//! CLI analysis mode.
//!
//! Scans PHP files in a project and reports PHPantom's own diagnostics
//! (no PHPStan, no external tools) in a PHPStan-like table format.
//!
//! This is a debugging/coverage tool for PHPantom developers: run it
//! against a real codebase to find gaps in the type resolver.  It reuses
//! the same Backend initialization pipeline as the LSP server, so the
//! results match what a user would see in their editor.
//!
//! Only single Composer projects (root `composer.json`) are supported
//! for now.
//!
//! # Usage
//!
//! ```sh
//! phpantom_lsp analyse                     # scan entire project
//! phpantom_lsp analyse src/                # scan a subdirectory
//! phpantom_lsp analyse src/Foo.php         # scan a single file
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::config;

/// Severity filter for the analyse output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityFilter {
    /// Show all diagnostics (error, warning, information, hint).
    All,
    /// Show only errors and warnings.
    Warning,
    /// Show only errors.
    Error,
}

/// Options for the analyse command.
#[derive(Debug)]
pub struct AnalyseOptions {
    /// Workspace root (project directory containing composer.json).
    pub workspace_root: PathBuf,
    /// Optional path filter: only analyse files under this path.
    /// Can be a directory or a single file.
    pub path_filter: Option<PathBuf>,
    /// Minimum severity to report.
    pub severity_filter: SeverityFilter,
    /// Whether to output with ANSI colours.
    pub use_colour: bool,
}

/// A single diagnostic result for the analyse output.
struct FileDiagnostic {
    /// 1-based line number.
    line: u32,
    /// The diagnostic message.
    message: String,
    /// The diagnostic code (e.g. "unknown_class").
    identifier: Option<String>,
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
    // Create a headless Backend (no LSP client) and run the same init
    // pipeline as the LSP server.  With client=None the log/progress
    // calls are no-ops.
    let backend = Backend::new_test();
    *backend.workspace_root().write() = Some(root.to_path_buf());
    *backend.config.lock() = cfg.clone();

    let php_version = cfg
        .php
        .version
        .as_deref()
        .and_then(crate::types::PhpVersion::from_composer_constraint)
        .unwrap_or_else(|| composer::detect_php_version(root).unwrap_or_default());
    backend.set_php_version(php_version);

    backend.init_single_project(root, php_version, None).await;

    // ── 3. Locate user files (via PSR-4) and crop to path ───────────
    let files = discover_user_files(&backend, root, options.path_filter.as_deref());

    if files.is_empty() {
        eprintln!("No PHP files found.");
        return 0;
    }

    // ── 4. Collect diagnostics for every file (parallel) ────────────
    // N worker threads each steal the next file index from a shared
    // atomic counter.  This gives natural ~1-file batching with even
    // work distribution regardless of per-file cost variance.
    let file_count = files.len();
    let severity_filter = options.severity_filter;
    let use_colour = options.use_colour;
    let next_idx = AtomicUsize::new(0);
    let n_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut all_file_diagnostics: Vec<(String, Vec<FileDiagnostic>)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let backend = &backend;
                let next_idx = &next_idx;
                let files = &files;
                s.spawn(move || {
                    let mut results: Vec<(String, Vec<FileDiagnostic>)> = Vec::new();
                    loop {
                        let i = next_idx.fetch_add(1, Ordering::Relaxed);
                        if i >= file_count {
                            break;
                        }
                        if use_colour && i.is_multiple_of(20) {
                            eprint!("\r\x1b[2K {}", progress_bar(i + 1, file_count));
                        }

                        let file_path = &files[i];
                        let content = match std::fs::read_to_string(file_path) {
                            Ok(c) => c,
                            Err(_) => continue,
                        };

                        let uri = crate::util::path_to_uri(file_path);

                        backend
                            .open_files
                            .write()
                            .insert(uri.clone(), Arc::new(content.clone()));
                        backend.update_ast(&uri, &content);

                        let mut raw = Vec::new();
                        backend.collect_fast_diagnostics(&uri, &content, &mut raw);
                        backend.collect_slow_diagnostics(&uri, &content, &mut raw);

                        backend.open_files.write().remove(&uri);
                        backend.clear_file_maps(&uri);

                        let filtered: Vec<FileDiagnostic> = raw
                            .into_iter()
                            .filter_map(|d| {
                                let sev = d.severity.unwrap_or(DiagnosticSeverity::WARNING);
                                if !passes_severity_filter(sev, severity_filter) {
                                    return None;
                                }
                                let identifier = match &d.code {
                                    Some(NumberOrString::String(s)) => Some(s.clone()),
                                    _ => None,
                                };
                                Some(FileDiagnostic {
                                    line: d.range.start.line + 1,
                                    message: d.message,
                                    identifier,
                                })
                            })
                            .collect();

                        if !filtered.is_empty() {
                            let display_path = file_path
                                .strip_prefix(root)
                                .unwrap_or(file_path)
                                .to_string_lossy()
                                .to_string();
                            results.push((display_path, filtered));
                        }
                    }
                    results
                })
            })
            .collect();

        let mut merged: Vec<(String, Vec<FileDiagnostic>)> = Vec::new();
        for handle in handles {
            merged.extend(handle.join().unwrap_or_default());
        }
        merged
    });

    // Sort by path so output order is deterministic.
    all_file_diagnostics.sort_by(|a, b| a.0.cmp(&b.0));

    let total_errors: usize = all_file_diagnostics
        .iter()
        .map(|(_, diags)| diags.len())
        .sum();

    if use_colour {
        eprint!("\r\x1b[2K {}\n", progress_bar(file_count, file_count));
    }

    // ── 5. Render output ────────────────────────────────────────────
    if all_file_diagnostics.is_empty() {
        print_success_box(file_count, options.use_colour);
        return 0;
    }

    for (path, diagnostics) in &all_file_diagnostics {
        print_file_table(path, diagnostics, options.use_colour);
    }

    print_error_box(total_errors, file_count, options.use_colour);

    1
}

// ── File discovery ──────────────────────────────────────────────────────────

/// Discover user PHP files to analyse.
///
/// Walks each PSR-4 source directory from `composer.json` (these only
/// cover the project's own code, not vendor).  When `path_filter` is
/// provided the results are cropped to that file or directory.
fn discover_user_files(
    backend: &Backend,
    workspace_root: &Path,
    path_filter: Option<&Path>,
) -> Vec<PathBuf> {
    use ignore::WalkBuilder;

    // Resolve the path filter to an absolute path.
    let abs_filter = path_filter.map(|f| {
        if f.is_relative() {
            workspace_root.join(f)
        } else {
            f.to_path_buf()
        }
    });

    // Single-file short circuit.
    if let Some(ref resolved) = abs_filter
        && resolved.is_file()
    {
        return if resolved.extension().is_some_and(|ext| ext == "php") {
            vec![resolved.clone()]
        } else {
            Vec::new()
        };
    }

    // Collect the PSR-4 source directories as absolute paths.
    let psr4 = backend.psr4_mappings().read().clone();
    let mut source_dirs: Vec<PathBuf> = psr4
        .iter()
        .map(|m| {
            let p = Path::new(&m.base_path);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                workspace_root.join(p)
            }
        })
        .filter(|p| p.is_dir())
        .collect();

    source_dirs.sort();
    source_dirs.dedup();

    let vendor_dirs: Vec<PathBuf> = backend.vendor_dir_paths.lock().clone();

    let mut files: Vec<PathBuf> = Vec::new();

    for dir in &source_dirs {
        // If a directory filter is active and doesn't overlap with
        // this source dir, skip entirely.
        if let Some(ref fp) = abs_filter
            && fp.is_dir()
            && !dir.starts_with(fp)
            && !fp.starts_with(dir)
        {
            continue;
        }

        let skip_vendor = vendor_dirs.clone();
        let walker = WalkBuilder::new(dir)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .hidden(true)
            .parents(true)
            .ignore(true)
            .filter_entry(move |entry| {
                if entry.file_type().is_some_and(|ft| ft.is_dir())
                    && let Ok(canonical) = entry.path().canonicalize()
                    && skip_vendor.iter().any(|v| canonical.starts_with(v))
                {
                    return false;
                }
                true
            })
            .build();

        for entry in walker.flatten() {
            let path = entry.into_path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "php") {
                continue;
            }

            // Crop to the filter directory.
            if let Some(ref fp) = abs_filter
                && fp.is_dir()
                && !path.starts_with(fp)
            {
                continue;
            }

            files.push(path);
        }
    }

    files.sort();
    files.dedup();
    files
}

// ── Severity helpers ────────────────────────────────────────────────────────

fn passes_severity_filter(severity: DiagnosticSeverity, filter: SeverityFilter) -> bool {
    match filter {
        SeverityFilter::All => true,
        SeverityFilter::Warning => {
            matches!(
                severity,
                DiagnosticSeverity::ERROR | DiagnosticSeverity::WARNING
            )
        }
        SeverityFilter::Error => severity == DiagnosticSeverity::ERROR,
    }
}

// ── PHPStan-style table output ──────────────────────────────────────────────
//
// Mirrors Symfony Console's `Table` style used by PHPStan's
// `TableErrorFormatter` (see phpstan-src tests for exact spacing):
//
//  ------ -------------------------------------------
//   Line   src/Foo.php
//  ------ -------------------------------------------
//   15     Call to undefined method Bar::baz().
//          🪪  unknown_member
//   42     Access to property $qux on unknown class.
//          🪪  unknown_class
//  ------ -------------------------------------------

/// Print a file's diagnostics in the PHPStan table format.
fn print_file_table(path: &str, diagnostics: &[FileDiagnostic], use_colour: bool) {
    struct Row {
        line_str: String,
        lines: Vec<String>,
    }

    let mut rows: Vec<Row> = Vec::new();
    for diag in diagnostics {
        let mut message_lines = vec![diag.message.clone()];
        if let Some(ref id) = diag.identifier {
            message_lines.push(format!("\u{1faaa}  {id}"));
        }
        rows.push(Row {
            line_str: diag.line.to_string(),
            lines: message_lines,
        });
    }

    // Column widths.
    let line_col_w = rows
        .iter()
        .map(|r| r.line_str.len())
        .max()
        .unwrap_or(0)
        .max(4); // at least as wide as "Line"

    let msg_col_w = rows
        .iter()
        .flat_map(|r| r.lines.iter().map(|l| l.len()))
        .max()
        .unwrap_or(0)
        .max(path.len());

    let sep = format!(
        " {} {}",
        "-".repeat(line_col_w + 2),
        "-".repeat(msg_col_w + 2),
    );

    // Header.
    println!("{sep}");
    if use_colour {
        println!("  \x1b[32m{:>line_col_w$}\x1b[0m   \x1b[32m{path}\x1b[0m", "Line");
    } else {
        println!("  {:>line_col_w$}   {path}", "Line");
    }
    println!("{sep}");

    // Data rows.
    for row in &rows {
        for (i, msg_line) in row.lines.iter().enumerate() {
            if i == 0 {
                println!("  {:>line_col_w$}   {msg_line}", row.line_str);
            } else if use_colour {
                println!("  {:>line_col_w$}   \x1b[2m{msg_line}\x1b[0m", "");
            } else {
                println!("  {:>line_col_w$}   {msg_line}", "");
            }
        }
    }

    // Footer + blank line between files.
    println!("{sep}");
    println!();
}

/// Print the `[OK]` success box.
fn print_success_box(_file_count: usize, use_colour: bool) {
    let text = " [OK] No errors ";
    if use_colour {
        let pad = " ".repeat(text.len());
        println!();
        println!(" \x1b[30;42m{pad}\x1b[0m");
        println!(" \x1b[30;42m{text}\x1b[0m");
        println!(" \x1b[30;42m{pad}\x1b[0m");
        println!();
    } else {
        println!("{text}");
    }
}

/// Print the `[ERROR]` summary box.
fn print_error_box(total_errors: usize, _file_count: usize, use_colour: bool) {
    let label = if total_errors == 1 { "error" } else { "errors" };
    let text = format!(" [ERROR] Found {total_errors} {label} ");
    if use_colour {
        let pad = " ".repeat(text.len());
        println!();
        println!(" \x1b[97;41m{pad}\x1b[0m");
        println!(" \x1b[97;41m{text}\x1b[0m");
        println!(" \x1b[97;41m{pad}\x1b[0m");
        println!();
    } else {
        println!("{text}");
    }
}

// ── Progress bar ────────────────────────────────────────────────────────────

const BAR_WIDTH: usize = 28;

/// Render a PHPStan-style progress bar string:
/// ` 120/883 [▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░]  13%`
fn progress_bar(done: usize, total: usize) -> String {
    let pct = if total == 0 {
        100
    } else {
        (done * 100) / total
    };
    let filled = if total == 0 {
        BAR_WIDTH
    } else {
        (done * BAR_WIDTH) / total
    };
    let empty = BAR_WIDTH - filled;

    format!(
        " {done:>width$}/{total} [{bar_fill}{bar_empty}] {pct:>3}%",
        width = total.to_string().len(),
        bar_fill = "▓".repeat(filled),
        bar_empty = "░".repeat(empty),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_filter_all_passes_everything() {
        assert!(passes_severity_filter(
            DiagnosticSeverity::ERROR,
            SeverityFilter::All
        ));
        assert!(passes_severity_filter(
            DiagnosticSeverity::WARNING,
            SeverityFilter::All
        ));
        assert!(passes_severity_filter(
            DiagnosticSeverity::INFORMATION,
            SeverityFilter::All
        ));
        assert!(passes_severity_filter(
            DiagnosticSeverity::HINT,
            SeverityFilter::All
        ));
    }

    #[test]
    fn severity_filter_warning_blocks_info_and_hint() {
        assert!(passes_severity_filter(
            DiagnosticSeverity::ERROR,
            SeverityFilter::Warning
        ));
        assert!(passes_severity_filter(
            DiagnosticSeverity::WARNING,
            SeverityFilter::Warning
        ));
        assert!(!passes_severity_filter(
            DiagnosticSeverity::INFORMATION,
            SeverityFilter::Warning
        ));
        assert!(!passes_severity_filter(
            DiagnosticSeverity::HINT,
            SeverityFilter::Warning
        ));
    }

    #[test]
    fn severity_filter_error_only() {
        assert!(passes_severity_filter(
            DiagnosticSeverity::ERROR,
            SeverityFilter::Error
        ));
        assert!(!passes_severity_filter(
            DiagnosticSeverity::WARNING,
            SeverityFilter::Error
        ));
        assert!(!passes_severity_filter(
            DiagnosticSeverity::INFORMATION,
            SeverityFilter::Error
        ));
        assert!(!passes_severity_filter(
            DiagnosticSeverity::HINT,
            SeverityFilter::Error
        ));
    }
}
