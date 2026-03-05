//! Unknown class diagnostics.
//!
//! Walk the precomputed [`SymbolMap`] for a file and flag every
//! `ClassReference` that cannot be resolved through any of PHPantom's
//! resolution phases (use-map → local classes → same-namespace →
//! class_index → classmap → PSR-4 → stubs).
//!
//! Diagnostics use `Severity::Warning` because the code may still run
//! (e.g. the class exists but hasn't been indexed yet), but the user
//! benefits from knowing that PHPantom can't resolve it.
//!
//! The logic closely mirrors `collect_import_class_actions` in the
//! `code_actions::import_class` module — both need to determine whether
//! a class reference is unresolved.  The difference is that the code
//! action offers to *fix* it, while this diagnostic *reports* it.
//!
//! `ClassReference` spans that fall on `use` statement lines are skipped
//! because they are import declarations, not actual usages.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::symbol_map::SymbolKind;
use crate::types::ClassInfo;

use super::offset_range_to_lsp_range;

/// Diagnostic code used for unknown-class diagnostics so that code
/// actions can match on it.
pub(crate) const UNKNOWN_CLASS_CODE: &str = "unknown_class";

impl Backend {
    /// Collect unknown-class diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub fn collect_unknown_class_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Gather context under locks ──────────────────────────────────
        let symbol_map = {
            let maps = match self.symbol_maps.lock() {
                Ok(m) => m,
                Err(_) => return,
            };
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_use_map: HashMap<String, String> = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace: Option<String> = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        let local_classes: Vec<ClassInfo> = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        // ── Compute byte ranges of `use` statement lines ────────────────
        // ClassReference spans that fall on these lines are import
        // declarations, not actual usages — skip them.
        let use_line_ranges = compute_use_line_ranges(content);

        // ── Walk every symbol span ──────────────────────────────────────
        for span in &symbol_map.spans {
            // Skip spans on `use` statement lines — those are the import
            // declarations themselves, not references to resolve.
            if is_offset_in_use_ranges(span.start, &use_line_ranges) {
                continue;
            }

            let (ref_name, is_fqn) = match &span.kind {
                SymbolKind::ClassReference { name, is_fqn } => (name.as_str(), *is_fqn),
                _ => continue,
            };

            // Resolve the name to a fully-qualified form, then check
            // whether PHPantom can find the class.
            let fqn = if is_fqn {
                ref_name.strip_prefix('\\').unwrap_or(ref_name).to_string()
            } else {
                resolve_to_fqn(ref_name, &file_use_map, &file_namespace)
            };

            // ── Skip names that are always resolvable ───────────────────
            // `self`, `static`, `parent`, `$this` are context-dependent
            // keywords and should never trigger an unknown-class warning.
            if is_self_like(ref_name) {
                continue;
            }

            // ── Skip @template parameters ───────────────────────────────
            // Template type parameters (e.g. `TValue`, `TKey`) declared
            // via `@template` tags are not real classes — they are type
            // variables scoped to the class or method.  The symbol map
            // already tracks these with scope ranges, so we can check
            // whether the reference name matches an in-scope template def.
            if !is_fqn
                && !ref_name.contains('\\')
                && symbol_map.find_template_def(ref_name, span.start).is_some()
            {
                continue;
            }

            // ── Attempt resolution through all phases ───────────────────

            // 1. Local classes (same file)
            if local_classes.iter().any(|c| {
                c.name == ref_name
                    || match &file_namespace {
                        Some(ns) => format!("{}\\{}", ns, c.name) == fqn,
                        None => c.name == fqn,
                    }
            }) {
                continue;
            }

            // 2. find_or_load_class covers: class_index → ast_map →
            //    classmap → PSR-4 → stubs
            if self.find_or_load_class(&fqn).is_some() {
                continue;
            }

            // 3. For unqualified names without a use-map entry and without
            //    a namespace, try the raw name as a global class.
            if !is_fqn
                && !ref_name.contains('\\')
                && !file_use_map.contains_key(ref_name)
                && file_namespace.is_none()
                && self.find_or_load_class(ref_name).is_some()
            {
                continue;
            }

            // 4. Check the stub index directly (global built-in classes).
            if self.stub_index.contains_key(fqn.as_str()) {
                continue;
            }

            // ── Name is unresolved — emit diagnostic ────────────────────
            let range =
                match offset_range_to_lsp_range(content, span.start as usize, span.end as usize) {
                    Some(r) => r,
                    None => continue,
                };

            let message = if is_fqn || ref_name.contains('\\') {
                format!("Class '{}' not found", fqn)
            } else {
                format!("Class '{}' not found", ref_name)
            };

            out.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String(UNKNOWN_CLASS_CODE.to_string())),
                code_description: None,
                source: Some("phpantom".to_string()),
                message,
                related_information: None,
                tags: None,
                data: None,
            });
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// A byte range `[start, end)` representing a line in the source.
type ByteRange = (usize, usize);

/// Compute the byte ranges of all top-level `use` statement lines.
///
/// Returns a sorted list of `(line_start, line_end)` byte offset pairs.
/// Only matches lines whose trimmed content starts with `use ` and contains
/// a `;` — this excludes trait `use` statements inside class bodies because
/// those are indented and always occur after a `{`.
fn compute_use_line_ranges(content: &str) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut offset: usize = 0;

    for line in content.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("use ") && trimmed.contains(';') {
            ranges.push((offset, offset + line.len()));
        }
        offset += line.len() + 1; // +1 for '\n'
    }

    ranges
}

/// Check whether a byte offset falls within any of the given ranges.
fn is_offset_in_use_ranges(offset: u32, ranges: &[ByteRange]) -> bool {
    let offset = offset as usize;
    ranges
        .iter()
        .any(|&(start, end)| offset >= start && offset < end)
}

/// Resolve an unqualified/qualified class name to a fully-qualified name
/// using the use map and namespace context.
fn resolve_to_fqn(
    name: &str,
    use_map: &HashMap<String, String>,
    namespace: &Option<String>,
) -> String {
    // Fully qualified (leading backslash)
    if let Some(stripped) = name.strip_prefix('\\') {
        return stripped.to_string();
    }

    // Unqualified (no backslash) — check use map first
    if !name.contains('\\') {
        if let Some(fqn) = use_map.get(name) {
            return fqn.clone();
        }
        if let Some(ns) = namespace {
            return format!("{}\\{}", ns, name);
        }
        return name.to_string();
    }

    // Qualified (contains backslash, no leading backslash)
    let first_segment = name.split('\\').next().unwrap_or(name);
    if let Some(fqn_prefix) = use_map.get(first_segment) {
        let rest = &name[first_segment.len()..];
        return format!("{}{}", fqn_prefix, rest);
    }
    if let Some(ns) = namespace {
        return format!("{}\\{}", ns, name);
    }
    name.to_string()
}

/// Returns `true` for context-dependent keywords that resolve to the
/// enclosing class and should never be flagged as unknown.
fn is_self_like(name: &str) -> bool {
    matches!(name, "self" | "static" | "parent" | "$this")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Backend;

    /// Helper: parse a file and collect unknown-class diagnostics.
    fn collect(backend: &Backend, uri: &str, content: &str) -> Vec<Diagnostic> {
        backend.update_ast(uri, content);
        let mut out = Vec::new();
        backend.collect_unknown_class_diagnostics(uri, content, &mut out);
        out
    }

    // ── Basic detection ─────────────────────────────────────────────────

    #[test]
    fn flags_unknown_class_in_new_expression() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew UnknownThing();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            diags.iter().any(|d| d.message.contains("UnknownThing")),
            "expected diagnostic for UnknownThing, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flags_unknown_class_in_type_hint() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nfunction foo(MissingClass $x): void {}\n";

        let diags = collect(&backend, uri, content);
        assert!(
            diags.iter().any(|d| d.message.contains("MissingClass")),
            "expected diagnostic for MissingClass, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flags_unknown_fqn_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnew \\Some\\Missing\\FqnClass();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Some\\Missing\\FqnClass")),
            "expected diagnostic for FqnClass, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // ── No false positives ──────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_local_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nclass Foo {}\n\nnew Foo();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("Foo")),
            "should not flag local class Foo, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_diagnostic_for_imported_class() {
        let backend = Backend::new_test();

        // Register the dependency class in a separate file so that
        // find_or_load_class can resolve it via the class_index + ast_map.
        let dep_uri = "file:///vendor/laravel/Request.php";
        let dep_content = "<?php\nnamespace Illuminate\\Http;\n\nclass Request {}\n";
        backend.update_ast(dep_uri, dep_content);
        if let Ok(mut idx) = backend.class_index.lock() {
            idx.insert("Illuminate\\Http\\Request".to_string(), dep_uri.to_string());
        }

        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nuse Illuminate\\Http\\Request;\n\nnew Request();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("Request")),
            "should not flag imported class Request, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_diagnostic_for_self_static_parent() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = concat!(
            "<?php\n",
            "namespace App;\n",
            "class Base {}\n",
            "class Child extends Base {\n",
            "    public function foo(): self { return $this; }\n",
            "    public function bar(): static { return $this; }\n",
            "    public function baz(): void { parent::baz(); }\n",
            "}\n",
        );

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| {
                d.message.contains("'self'")
                    || d.message.contains("'static'")
                    || d.message.contains("'parent'")
            }),
            "should not flag self/static/parent, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_diagnostic_for_stub_class() {
        use std::collections::HashMap;

        let mut stubs = HashMap::new();
        stubs.insert(
            "Exception",
            "<?php\nclass Exception {\n    public function getMessage(): string {}\n}\n",
        );
        let backend = Backend::new_test_with_stubs(stubs);
        let uri = "file:///test.php";
        let content = "<?php\nnew \\Exception();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("Exception")),
            "should not flag stub class Exception, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_diagnostic_for_same_namespace_class() {
        let backend = Backend::new_test();
        let uri_dep = "file:///dep.php";
        let content_dep = "<?php\nnamespace App;\n\nclass Helper {}\n";
        backend.update_ast(uri_dep, content_dep);

        // Register in class_index so same-namespace lookup works.
        if let Ok(mut idx) = backend.class_index.lock() {
            idx.insert("App\\Helper".to_string(), uri_dep.to_string());
        }

        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew Helper();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("Helper")),
            "should not flag same-namespace class Helper, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // ── Diagnostic metadata ─────────────────────────────────────────────

    #[test]
    fn diagnostic_has_warning_severity() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew Ghost();\n";

        let diags = collect(&backend, uri, content);
        let ghost_diag = diags
            .iter()
            .find(|d| d.message.contains("Ghost"))
            .expect("should have diagnostic for Ghost");
        assert_eq!(ghost_diag.severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn diagnostic_has_code_and_source() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew Ghost();\n";

        let diags = collect(&backend, uri, content);
        let ghost_diag = diags
            .iter()
            .find(|d| d.message.contains("Ghost"))
            .expect("should have diagnostic for Ghost");
        assert_eq!(
            ghost_diag.code,
            Some(NumberOrString::String("unknown_class".to_string()))
        );
        assert_eq!(ghost_diag.source, Some("phpantom".to_string()));
    }

    #[test]
    fn diagnostic_range_covers_class_name() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        // "<?php\nnamespace App;\n\nnew Ghost();\n"
        //  line 3: "new Ghost();"
        //  "new " = 4 chars, "Ghost" starts at col 4, ends at col 9
        let content = "<?php\nnamespace App;\n\nnew Ghost();\n";

        let diags = collect(&backend, uri, content);
        let ghost_diag = diags
            .iter()
            .find(|d| d.message.contains("Ghost"))
            .expect("should have diagnostic for Ghost");

        // The range should be on line 3 and cover "Ghost" (5 chars).
        assert_eq!(ghost_diag.range.start.line, 3);
        assert_eq!(ghost_diag.range.end.line, 3);
        let width = ghost_diag.range.end.character - ghost_diag.range.start.character;
        assert_eq!(width, 5, "range should cover 'Ghost' (5 chars)");
    }

    // ── No diagnostic for global class without namespace ────────────────

    #[test]
    fn no_diagnostic_for_global_class_without_namespace() {
        let backend = Backend::new_test();
        let uri_dep = "file:///dep.php";
        let content_dep = "<?php\nclass GlobalHelper {}\n";
        backend.update_ast(uri_dep, content_dep);

        if let Ok(mut idx) = backend.class_index.lock() {
            idx.insert("GlobalHelper".to_string(), uri_dep.to_string());
        }

        let uri = "file:///test.php";
        let content = "<?php\nnew GlobalHelper();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("GlobalHelper")),
            "should not flag global class without namespace, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // ── Multiple unknown classes in one file ────────────────────────────

    // ── Template parameters ─────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_template_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = concat!(
            "<?php\n",
            "namespace App;\n",
            "\n",
            "/**\n",
            " * @template TValue\n",
            " * @template TKey\n",
            " */\n",
            "class Collection {\n",
            "    /**\n",
            "     * @param callable(TValue, TKey): mixed $callback\n",
            "     * @return TValue\n",
            "     */\n",
            "    public function first(callable $callback): mixed { return null; }\n",
            "}\n",
        );

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("TValue")),
            "should not flag @template param TValue, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(
            !diags.iter().any(|d| d.message.contains("TKey")),
            "should not flag @template param TKey, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_diagnostic_for_method_level_template() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = concat!(
            "<?php\n",
            "namespace App;\n",
            "\n",
            "class Util {\n",
            "    /**\n",
            "     * @template T\n",
            "     * @param T $value\n",
            "     * @return T\n",
            "     */\n",
            "    public function identity(mixed $value): mixed { return $value; }\n",
            "}\n",
        );

        let diags = collect(&backend, uri, content);
        assert!(
            !diags.iter().any(|d| d.message.contains("'T'")),
            "should not flag method-level @template param T, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    // ── Multiple unknown classes in one file ────────────────────────────

    #[test]
    fn flags_multiple_unknown_classes() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew Alpha();\nnew Beta();\n";

        let diags = collect(&backend, uri, content);
        assert!(
            diags.iter().any(|d| d.message.contains("Alpha")),
            "expected diagnostic for Alpha"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("Beta")),
            "expected diagnostic for Beta"
        );
    }
}
