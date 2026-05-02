//! CLI analysis mode.
//!
//! Scans PHP files in a project and reports PHPantom's own diagnostics
//! (no PHPStan, no external tools) in a PHPStan-like table format.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tower_lsp::lsp_types::*;

use crate::parser::with_parse_cache;
use crate::virtual_members::with_active_resolved_class_cache;

use crate::Backend;
use crate::composer;
use crate::config;
use crate::types::ClassInfo;

/// Output format for the analysis report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable table (default).
    #[default]
    Table,
    /// Machine-readable JSON.
    Json,
    /// GitHub Actions workflow command format.
    Github,
}

/// Options for the `analyze` command.
#[derive(Debug)]
pub struct AnalyseOptions {
    pub workspace_root: PathBuf,
    pub path_filter: Option<String>,
    pub severity_filter: DiagnosticSeverity,
    pub output_format: OutputFormat,
    pub use_colour: bool,
}

/// A diagnostic finding for a single file.
#[derive(Debug, serde::Serialize)]
pub struct FileDiagnostic {
    /// The location of the issue.
    pub range: Range,
    /// The human-readable description.
    pub message: String,
    /// The diagnostic code (e.g. "unknown_class").
    pub identifier: Option<String>,
    /// The diagnostic severity.
    pub severity: DiagnosticSeverity,
}

/// Run the analyse command and return the process exit code.
///
/// Returns `0` when no diagnostics are found, `1` when diagnostics exist.
pub async fn run(options: AnalyseOptions) -> i32 {
    let root = &options.workspace_root;

    if !root.join("composer.json").is_file() {
        eprintln!("Error: no composer.json found in {}", root.display());
        eprintln!("The analyse command currently only supports single Composer projects.");
        return 1;
    }

    // ── 1. Load config ──────────────────────────────────────────────
    let cfg = match config::load_config(root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to load .phpantom.toml: {e}");
            config::Config::default()
        }
    };

    // ── 2. Index project ────────────────────────────────────────────
    let backend = Backend::new_headless();
    *backend.workspace_root().write() = Some(root.to_path_buf());
    *backend.config.lock() = cfg.clone();

    let composer_package = composer::read_composer_package(root);

    let php_version = cfg
        .php
        .version
        .as_deref()
        .and_then(crate::types::PhpVersion::from_composer_constraint)
        .unwrap_or_else(|| {
            composer_package
                .as_ref()
                .and_then(composer::detect_php_version_from_package)
                .unwrap_or_default()
        });
    backend.set_php_version(php_version);

    backend
        .init_single_project(root, php_version, composer_package, None)
        .await;

    // ── 3. Locate user files (via PSR-4) and crop to path ───────────
    let filter_path = options.path_filter.as_deref().map(Path::new);
    let files = discover_user_files(&backend, root, filter_path);

    if files.is_empty() {
        eprintln!("No PHP files found.");
        return 0;
    }

    // ── 4. Two-phase parallel analysis ──────────────────────────────
    let file_count = files.len();
    let severity_filter = options.severity_filter;
    let use_colour = options.use_colour;
    let output_format = options.output_format;
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // ── Phase 1: Parse all files (parallel) ─────────────────────────
    if use_colour && output_format == OutputFormat::Table {
        eprint!("\r\x1b[2K {}", progress_bar(0, file_count));
    }
    let next_idx = AtomicUsize::new(0);

    let file_data: Vec<Option<(String, String)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let backend = &backend;
                let next_idx = &next_idx;
                let files = &files;
                std::thread::Builder::new()
                    .name("index-worker".into())
                    .stack_size(32 * 1024 * 1024)
                    .spawn_scoped(s, move || {
                        let mut entries: Vec<(usize, String, String)> = Vec::new();
                        loop {
                            let i = next_idx.fetch_add(1, Ordering::Relaxed);
                            if i >= file_count {
                                break;
                            }

                            let file_path = &files[i];
                            let content = match std::fs::read_to_string(file_path) {
                                Ok(c) => c,
                                Err(_) => continue,
                            };

                            let uri = crate::util::path_to_uri(file_path);
                            backend.update_ast(&uri, &content);
                            entries.push((i, uri, content));
                        }
                        entries
                    })
                    .expect("failed to spawn index-worker thread")
            })
            .collect();

        let mut indexed: Vec<Option<(String, String)>> = (0..file_count).map(|_| None).collect();
        for handle in handles {
            let entries = handle.join().expect("index-worker thread panicked");
            for entry in entries {
                indexed[entry.0] = Some((entry.1, entry.2));
            }
        }
        indexed
    });

    // ── Phase 1.5: Eager class population ───────────────────────────
    let sorted_fqns = {
        let ast_map = backend.ast_map.read();
        crate::toposort::toposort_from_ast_map(&ast_map)
    };
    std::thread::scope(|s| {
        let backend = &backend;
        let sorted_fqns = &sorted_fqns;
        std::thread::Builder::new()
            .name("eager-populate".into())
            .stack_size(32 * 1024 * 1024)
            .spawn_scoped(s, move || {
                let class_loader =
                    |name: &str| -> Option<Arc<ClassInfo>> { backend.find_or_load_class(name) };
                crate::virtual_members::populate_from_sorted(
                    sorted_fqns,
                    &backend.resolved_class_cache,
                    &class_loader,
                );
            })
            .expect("failed to spawn eager-population thread");
    });

    // ── Phase 2: Collect diagnostics (parallel) ─────────────────────
    let next_idx = AtomicUsize::new(0);
    let done_count = AtomicUsize::new(0);

    let mut all_file_diagnostics: Vec<(String, Vec<FileDiagnostic>)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let backend = &backend;
                let next_idx = &next_idx;
                let done_count = &done_count;
                let file_data = &file_data;
                std::thread::Builder::new()
                    .name("diag-worker".into())
                    .stack_size(32 * 1024 * 1024)
                    .spawn_scoped(s, move || {
                        let mut results: Vec<(String, Vec<FileDiagnostic>)> = Vec::new();
                        loop {
                            let i = next_idx.fetch_add(1, Ordering::Relaxed);
                            if i >= file_count {
                                break;
                            }
                            let (uri, content) = match &file_data[i] {
                                Some(pair) => (&pair.0, &pair.1),
                                None => continue,
                            };

                            let _parse_guard = with_parse_cache(content);
                            let _cache_guard =
                                with_active_resolved_class_cache(&backend.resolved_class_cache);
                            let _chain_guard =
                                crate::completion::resolver::with_chain_resolution_cache();
                            let _callable_guard =
                                crate::completion::call_resolution::with_callable_target_cache();
                            let _body_infer_guard = backend.activate_body_return_inferrer();

                            let _scope_guard =
                            crate::completion::variable::forward_walk::with_diagnostic_scope_cache(
                            );
                            let scope_t0 = Instant::now();
                            {
                                let file_ctx = backend.file_context(uri);
                                let class_loader = backend.class_loader(&file_ctx);
                                let function_loader_cl = backend.function_loader(&file_ctx);
                                let constant_loader_cl = backend.constant_loader();
                                let loaders = crate::completion::resolver::Loaders {
                                    function_loader: Some(&function_loader_cl),
                                    constant_loader: Some(&constant_loader_cl),
                                };
                                crate::completion::variable::forward_walk::build_diagnostic_scopes(
                                    content,
                                    &file_ctx.classes,
                                    &class_loader,
                                    loaders,
                                    Some(&backend.resolved_class_cache),
                                );
                            }
                            let scope_elapsed = scope_t0.elapsed();

                            let mut raw = Vec::new();

                            #[cfg(debug_assertions)]
                            {
                                const FILE_TIMEOUT: Duration = Duration::from_secs(60);
                                type CollectFn = dyn Fn(&Backend, &str, &str, &mut Vec<Diagnostic>);
                                let file_start = Instant::now();
                                let deadline = file_start + FILE_TIMEOUT;
                                let mut timings = Vec::new();
                                let mut timed_out = false;
                                timings.push((scope_elapsed, "scope"));

                                timings.push({
                                    let t0 = Instant::now();
                                    backend.collect_fast_diagnostics(uri, content, &mut raw);
                                    (t0.elapsed(), "fast")
                                });

                                let collectors: &[(&str, &CollectFn)] = &[
                                (
                                    "unknown_class",
                                    &|b: &Backend, u: &str, c: &str, o: &mut Vec<Diagnostic>| {
                                        b.collect_unknown_class_diagnostics(u, c, o)
                                    },
                                ),
                                ("unknown_member", &|b, u, c, o| {
                                    b.collect_unknown_member_diagnostics(u, c, o)
                                }),
                                ("unknown_function", &|b, u, c, o| {
                                    b.collect_unknown_function_diagnostics(u, c, o)
                                }),
                                ("argument_count", &|b, u, c, o| {
                                    b.collect_argument_count_diagnostics(u, c, o)
                                }),
                                ("type_error", &|b, u, c, o| {
                                    b.collect_type_error_diagnostics(u, c, o)
                                }),
                                ("implementation", &|b, u, c, o| {
                                    b.collect_implementation_error_diagnostics(u, c, o)
                                }),
                                ("deprecated", &|b, u, c, o| {
                                    b.collect_deprecated_diagnostics(u, c, o)
                                }),
                                ("undefined_variable", &|b, u, c, o| {
                                    b.collect_undefined_variable_diagnostics(u, c, o)
                                }),
                                ("invalid_class_kind", &|b, u, c, o| {
                                    b.collect_invalid_class_kind_diagnostics(u, c, o)
                                }),
                            ];

                                for (name, collect_fn) in collectors {
                                    if Instant::now() >= deadline {
                                        timed_out = true;
                                        break;
                                    }
                                    let t0 = Instant::now();
                                    collect_fn(backend, uri, content, &mut raw);
                                    let elapsed = t0.elapsed();
                                    timings.push((elapsed, name));
                                }

                                if timed_out {
                                    eprintln!("\nWarning: diagnostics timed out on {}", uri);
                                }
                            }

                            #[cfg(not(debug_assertions))]
                            {
                                backend.collect_fast_diagnostics(uri, content, &mut raw);
                                backend.collect_slow_diagnostics(uri, content, &mut raw);
                            }

                            if !raw.is_empty() {
                                let mut file_results = Vec::new();
                                for d in raw {
                                    if d.severity <= Some(severity_filter) {
                                        file_results.push(FileDiagnostic {
                                            range: d.range,
                                            message: d.message,
                                            identifier: match d.code {
                                                Some(NumberOrString::String(s)) => Some(s),
                                                _ => None,
                                            },
                                            severity: d
                                                .severity
                                                .unwrap_or(DiagnosticSeverity::ERROR),
                                        });
                                    }
                                }
                                if !file_results.is_empty() {
                                    results.push((uri.clone(), file_results));
                                }
                            }

                            let done = done_count.fetch_add(1, Ordering::Relaxed) + 1;
                            if use_colour
                                && output_format == OutputFormat::Table
                                && done.is_multiple_of(10)
                            {
                                eprint!("\r\x1b[2K {}", progress_bar(done, file_count));
                            }
                        }
                        results
                    })
                    .expect("failed to spawn diag-worker thread")
            })
            .collect();

        let mut all = Vec::new();
        for h in handles {
            let res = h.join().expect("diag-worker panicked");
            all.extend(res);
        }
        all
    });

    if use_colour && output_format == OutputFormat::Table {
        eprint!("\r\x1b[2K {}", progress_bar(file_count, file_count));
        eprintln!();
    }

    if all_file_diagnostics.is_empty() {
        if output_format == OutputFormat::Table {
            println!("No issues found.");
        } else {
            println!("[]");
        }
        return 0;
    }

    if output_format == OutputFormat::Table {
        print_report_table(&mut all_file_diagnostics, root, use_colour);
    } else if output_format == OutputFormat::Github {
        print_report_github(&mut all_file_diagnostics, root);
    } else {
        println!("{}", serde_json::to_string(&all_file_diagnostics).unwrap());
    }

    1
}

pub fn discover_user_files(backend: &Backend, root: &Path, filter: Option<&Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let abs_filter = filter.map(|f| root.join(f).canonicalize().unwrap_or(root.join(f)));

    let dirs_to_walk = if let Some(ref fp) = abs_filter
        && fp.is_dir()
    {
        vec![fp.clone()]
    } else if let Some(ref fp) = abs_filter
        && fp.is_file()
    {
        return vec![fp.clone()];
    } else {
        let mappings = backend.psr4_mappings().read();
        mappings
            .iter()
            .map(|m| PathBuf::from(&m.base_path))
            .collect()
    };

    for dir in &dirs_to_walk {
        if let Some(ref fp) = abs_filter
            && fp.is_dir()
            && !fp.starts_with(dir)
            && !dir.starts_with(fp)
        {
            continue;
        }

        let walker = ignore::WalkBuilder::new(dir)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "php") {
                if let Some(ref fp) = abs_filter
                    && !path.starts_with(fp)
                {
                    continue;
                }
                files.push(path.to_path_buf());
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

fn progress_bar(done: usize, total: usize) -> String {
    let width = 40;
    let progress = if total > 0 {
        (done as f64 / total as f64 * width as f64) as usize
    } else {
        width
    };
    let percent = if total > 0 {
        (done as f64 / total as f64 * 100.0) as usize
    } else {
        100
    };

    let filled = "=".repeat(progress);
    let empty = " ".repeat(width - progress);
    format!("[{}{}] {}% ({}/{})", filled, empty, percent, done, total)
}

fn print_report_table(
    all_diagnostics: &mut [(String, Vec<FileDiagnostic>)],
    root: &Path,
    use_colour: bool,
) {
    all_diagnostics.sort_by(|a, b| a.0.cmp(&b.0));

    let mut total_errors = 0;
    let mut total_warnings = 0;

    for (uri, diagnostics) in all_diagnostics {
        let path = crate::util::uri_to_path(uri);
        let rel_path = path
            .as_ref()
            .and_then(|p| p.strip_prefix(root).ok())
            .unwrap_or(path.as_deref().unwrap_or(Path::new(uri)));

        println!();
        if use_colour {
            println!("\x1b[1;37m{}\x1b[0m", rel_path.display());
            println!(
                "\x1b[1;37m{}\x1b[0m",
                "-".repeat(rel_path.to_string_lossy().len())
            );
        } else {
            println!("{}", rel_path.display());
            println!("{}", "-".repeat(rel_path.to_string_lossy().len()));
        }

        for d in diagnostics {
            let line = d.range.start.line + 1;
            let col = d.range.start.character + 1;
            let (sev_label, sev_colour) = match d.severity {
                DiagnosticSeverity::ERROR => {
                    total_errors += 1;
                    ("Error", "\x1b[1;31m")
                }
                DiagnosticSeverity::WARNING => {
                    total_warnings += 1;
                    ("Warning", "\x1b[1;33m")
                }
                _ => ("Issue", "\x1b[1;37m"),
            };

            let id_part = d
                .identifier
                .as_ref()
                .map(|id| format!(" [{}]", id))
                .unwrap_or_default();

            if use_colour {
                println!(
                    " {:>4}:{:<3}  {}{:<7}\x1b[0m  {}{}",
                    line, col, sev_colour, sev_label, d.message, id_part
                );
            } else {
                println!(
                    " {:>4}:{:<3}  {:<7}  {}{}",
                    line, col, sev_label, d.message, id_part
                );
            }
        }
    }

    println!();
    if use_colour {
        if total_errors > 0 {
            println!("\x1b[1;31mFound {} errors\x1b[0m", total_errors);
        }
        if total_warnings > 0 {
            println!("\x1b[1;33mFound {} warnings\x1b[0m", total_warnings);
        }
        if total_errors == 0 && total_warnings == 0 {
            println!("\x1b[1;32mNo issues found.\x1b[0m");
        }
    } else {
        println!("Found {} errors, {} warnings", total_errors, total_warnings);
    }
}

fn print_report_github(all_diagnostics: &mut [(String, Vec<FileDiagnostic>)], root: &Path) {
    all_diagnostics.sort_by(|a, b| a.0.cmp(&b.0));

    for (uri, diagnostics) in all_diagnostics {
        let path = crate::util::uri_to_path(uri);
        let rel_path = path
            .as_ref()
            .and_then(|p| p.strip_prefix(root).ok())
            .unwrap_or(path.as_deref().unwrap_or(Path::new(uri)));

        for d in diagnostics {
            let line = d.range.start.line + 1;
            let col = d.range.start.character + 1;
            let level = match d.severity {
                DiagnosticSeverity::ERROR => "error",
                _ => "warning",
            };
            let id_part = d
                .identifier
                .as_ref()
                .map(|id| format!(" [{}]", id))
                .unwrap_or_default();

            let message = format_github_message(&format!("{}{}", d.message, id_part));
            println!(
                "::{} file={},line={},col={}::{}",
                level,
                rel_path.display(),
                line,
                col,
                message
            );
        }
    }
}

pub fn format_github_message(message: &str) -> String {
    message
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

pub fn json_escape(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_filter_error_only() {
        let mut raw = vec![
            Diagnostic {
                severity: Some(DiagnosticSeverity::ERROR),
                message: "err".to_string(),
                ..Default::default()
            },
            Diagnostic {
                severity: Some(DiagnosticSeverity::WARNING),
                message: "warn".to_string(),
                ..Default::default()
            },
        ];
        raw.retain(|d| d.severity <= Some(DiagnosticSeverity::ERROR));
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].message, "err");
    }

    #[test]
    fn severity_filter_all_passes_everything() {
        let mut raw = vec![
            Diagnostic {
                severity: Some(DiagnosticSeverity::ERROR),
                message: "err".to_string(),
                ..Default::default()
            },
            Diagnostic {
                severity: Some(DiagnosticSeverity::HINT),
                message: "hint".to_string(),
                ..Default::default()
            },
        ];
        raw.retain(|d| d.severity <= Some(DiagnosticSeverity::HINT));
        assert_eq!(raw.len(), 2);
    }

    #[test]
    fn severity_filter_warning_blocks_info_and_hint() {
        let mut raw = vec![
            Diagnostic {
                severity: Some(DiagnosticSeverity::WARNING),
                message: "warn".to_string(),
                ..Default::default()
            },
            Diagnostic {
                severity: Some(DiagnosticSeverity::INFORMATION),
                message: "info".to_string(),
                ..Default::default()
            },
        ];
        raw.retain(|d| d.severity <= Some(DiagnosticSeverity::WARNING));
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].message, "warn");
    }

    #[test]
    fn github_annotation_format() {
        let msg = "Hello\nWorld % 100%";
        let formatted = format_github_message(msg);
        assert_eq!(formatted, "Hello%0AWorld %25 100%25");
    }

    #[test]
    fn json_escape_basic() {
        assert_eq!(json_escape("hello"), "\"hello\"");
    }

    #[test]
    fn json_escape_special_chars() {
        assert_eq!(json_escape("line\nfeed"), "\"line\\nfeed\"");
        assert_eq!(json_escape("quote\"mark"), "\"quote\\\"mark\"");
    }

    #[test]
    fn json_output_empty() {
        let diags: Vec<(String, Vec<FileDiagnostic>)> = Vec::new();
        let json = serde_json::to_string(&diags).unwrap();
        assert_eq!(json, "[]");
    }
}
