//! Shared helpers for diagnostic collectors.
//!
//! Functions and types that are used by multiple diagnostic modules live
//! here to avoid duplication.

use std::collections::HashMap;
use std::sync::Arc;

use tower_lsp::lsp_types::*;

use crate::types::ClassInfo;

/// A byte range `[start, end)` representing a line in the source.
pub(crate) type ByteRange = (usize, usize);

/// Compute the byte ranges of all namespace-level `use` import lines.
///
/// Returns a sorted list of `(line_start, line_end)` byte offset pairs.
/// Only matches `use` lines at brace depth 0 (or depth 1 when inside a
/// `namespace Foo { … }` block).  Trait `use` statements inside class
/// bodies are at depth >= 1 (or >= 2 under a braced namespace) and are
/// excluded.
pub(crate) fn compute_use_line_ranges(content: &str) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut offset: usize = 0;
    // Track brace depth so we can distinguish namespace-level `use`
    // imports (depth 0, or depth 1 inside `namespace Foo { … }`) from
    // trait `use` statements inside class/trait/enum bodies (depth >= 1
    // or >= 2 under a braced namespace).
    let mut brace_depth: usize = 0;
    let mut namespace_brace_depth: Option<usize> = None;

    for line in content.split('\n') {
        // Update brace depth for braces on this line (crude but
        // sufficient — we only need an approximate depth to tell
        // top-level from class-body).  We skip braces inside strings
        // and comments only to the extent that single-line `//` and
        // `#` comments are trimmed, which covers the vast majority of
        // real-world PHP.
        let code = line.split("//").next().unwrap_or(line);
        let code = code.split('#').next().unwrap_or(code);

        let trimmed = line.trim_start();

        // Detect `namespace Foo {` so we know that depth 1 is still
        // "top-level" for use-import purposes.
        if trimmed.starts_with("namespace ") && code.contains('{') {
            // The opening brace on this line will bump brace_depth;
            // record that the namespace block starts at the *current*
            // depth (before the brace is counted).
            namespace_brace_depth = Some(brace_depth);
        }

        for ch in code.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth = brace_depth.saturating_sub(1);
                    // If we've closed the namespace block, clear the marker.
                    if namespace_brace_depth == Some(brace_depth) {
                        namespace_brace_depth = None;
                    }
                }
                _ => {}
            }
        }

        // A `use` line is a namespace import when it is at top-level
        // brace depth: depth 0 normally, or depth 1 when inside a
        // braced `namespace Foo { … }` block.
        let top_level_depth = namespace_brace_depth.map_or(0, |d| d + 1);
        if trimmed.starts_with("use ") && trimmed.contains(';') && brace_depth == top_level_depth {
            ranges.push((offset, offset + line.len()));
        }
        offset += line.len() + 1; // +1 for '\n'
    }

    ranges
}

/// Check whether a byte offset falls within any of the given ranges.
pub(crate) fn is_offset_in_ranges(offset: u32, ranges: &[ByteRange]) -> bool {
    let offset = offset as usize;
    ranges
        .iter()
        .any(|&(start, end)| offset >= start && offset < end)
}

/// Resolve an unqualified/qualified class name to a fully-qualified name
/// using the use map and namespace context.
///
/// This mirrors the logic in `Backend::resolve_class_name` but only
/// produces the FQN string without loading the class.
pub(crate) fn resolve_to_fqn(
    name: &str,
    use_map: &HashMap<String, String>,
    namespace: &Option<String>,
) -> String {
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

/// Find the innermost class whose body span contains `offset`.
///
/// Returns a reference to the `ClassInfo` with the smallest span that
/// encloses `offset`, including anonymous classes.  Used for
/// `$this`/`self`/`static` resolution inside diagnostic collectors.
pub(crate) fn find_innermost_enclosing_class(
    local_classes: &[Arc<ClassInfo>],
    offset: u32,
) -> Option<&ClassInfo> {
    local_classes
        .iter()
        .filter(|c| offset >= c.start_offset && offset <= c.end_offset)
        .min_by_key(|c| c.end_offset.saturating_sub(c.start_offset))
        .map(|c| c.as_ref())
}

/// Build a standard diagnostic with the common fields pre-filled.
///
/// Most diagnostic collectors build `Diagnostic` values with `source`
/// set to `"phpantom"` and the remaining optional fields set to `None`.
/// This helper reduces the boilerplate.
pub(crate) fn make_diagnostic(
    range: Range,
    severity: DiagnosticSeverity,
    code: &str,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: None,
        source: Some("phpantom".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}
