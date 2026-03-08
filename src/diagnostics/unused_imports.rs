//! Unused `use` statement dimming.
//!
//! After `update_ast`, compare each `use` declaration against all symbol
//! references in the file.  Any import alias that has zero references
//! gets a diagnostic with `Severity::Hint` and `DiagnosticTag::Unnecessary`,
//! which editors render as dimmed text.
//!
//! We only check class-level `use` imports (not trait `use` inside class
//! bodies, and not `use function` / `use const` — those are a follow-up).

use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::symbol_map::SymbolKind;

use super::offset_range_to_lsp_range;

impl Backend {
    /// Collect unused-import diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller publishes them via
    /// `textDocument/publishDiagnostics`.
    pub fn collect_unused_import_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Gather the file's use map (short name → FQN) ────────────────
        let file_use_map: HashMap<String, String> = match self.use_map.read().get(uri) {
            Some(map) => map.clone(),
            None => return,
        };

        if file_use_map.is_empty() {
            return;
        }

        // ── Gather the symbol map ───────────────────────────────────────
        let symbol_map = match self.symbol_maps.read().get(uri) {
            Some(sm) => sm.clone(),
            None => return,
        };

        // ── Compute byte ranges of `use` statement lines ────────────────
        // We need to exclude ClassReference spans that are part of `use`
        // statements themselves — those are the *import declarations*, not
        // actual usages of the imported name.
        let use_line_ranges = compute_use_line_ranges(content);

        // ── Also compute byte ranges of class/interface/trait/enum
        //    declaration lines so the content safety-net doesn't count
        //    a class declaration bearing the same short name as a usage. ──
        let decl_line_ranges = compute_declaration_line_ranges(content);

        // ── Collect all referenced short names from the symbol map ──────
        //
        // A `use Foo\Bar;` import is considered "used" if `Bar` appears as:
        //   - A ClassReference name (type hint, new, extends, implements, catch, etc.)
        //   - A MemberAccess subject_text for static access (`Bar::method()`)
        //   - A FunctionCall name that matches the alias (unlikely for class
        //     imports, but covers edge cases)
        //   - The subject_text in any context that matches the short name
        //
        // We also check docblock type references, which are already emitted
        // as ClassReference spans by the symbol map extraction.
        let mut referenced_aliases: HashSet<String> = HashSet::new();

        for span in &symbol_map.spans {
            // Skip spans that fall on `use` statement lines — those are
            // the import declarations, not actual usage sites.
            if is_offset_in_ranges(span.start, &use_line_ranges) {
                continue;
            }

            match &span.kind {
                SymbolKind::ClassReference { name, .. } => {
                    // The name may be fully qualified, partially qualified,
                    // or unqualified.  We need to check if the first segment
                    // (or the whole name for unqualified) matches a use alias.
                    let first_segment = extract_first_segment(name);
                    if file_use_map.contains_key(first_segment) {
                        referenced_aliases.insert(first_segment.to_string());
                    }
                }

                SymbolKind::MemberAccess {
                    subject_text,
                    is_static: true,
                    ..
                } => {
                    // Static access: `Foo::bar()` — subject_text is `"Foo"`
                    let trimmed = subject_text.trim();
                    if !trimmed.starts_with('$')
                        && trimmed != "self"
                        && trimmed != "static"
                        && trimmed != "parent"
                    {
                        let first_segment = extract_first_segment(trimmed);
                        if file_use_map.contains_key(first_segment) {
                            referenced_aliases.insert(first_segment.to_string());
                        }
                    }
                }

                SymbolKind::FunctionCall { name } => {
                    // In rare cases a use alias might match a function call
                    // pattern (e.g. `use function` — but we don't track those
                    // in the use_map currently).  Still check the first segment.
                    let first_segment = extract_first_segment(name);
                    if file_use_map.contains_key(first_segment) {
                        referenced_aliases.insert(first_segment.to_string());
                    }
                }

                SymbolKind::ConstantReference { name } => {
                    let first_segment = extract_first_segment(name);
                    if file_use_map.contains_key(first_segment) {
                        referenced_aliases.insert(first_segment.to_string());
                    }
                }

                _ => {}
            }
        }

        // Filter to only aliases the symbol map didn't find.
        let unused_aliases: Vec<&String> = file_use_map
            .keys()
            .filter(|alias| !referenced_aliases.contains(alias.as_str()))
            .collect();

        if unused_aliases.is_empty() {
            return;
        }

        // ── Safety-net: scan raw content for missed references ──────────
        //
        // For each still-unused alias, scan the raw content for the alias
        // appearing as an identifier outside of `use` statement and class
        // declaration lines.  This catches references in attributes,
        // annotations, or other contexts the symbol map might have missed.
        //
        // This avoids false positives for edge cases.
        // ── Find use statement positions in the source ──────────────────
        for alias in &unused_aliases {
            let fqn = match file_use_map.get(alias.as_str()) {
                Some(f) => f,
                None => continue,
            };

            // Double-check: scan content for the alias appearing as an
            // identifier outside of `use` statements and class declarations.
            if alias_is_referenced_in_content(
                content,
                alias,
                fqn,
                &use_line_ranges,
                &decl_line_ranges,
            ) {
                continue;
            }

            // Find the `use` statement line that imports this FQN.
            if let Some(range) = find_use_statement_range(content, alias, fqn) {
                out.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::HINT),
                    code: None,
                    code_description: None,
                    source: Some("phpantom".to_string()),
                    message: format!("Unused import '{}'", fqn),
                    related_information: None,
                    tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                    data: None,
                });
            }
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

/// Compute the byte ranges of class / interface / trait / enum declaration
/// lines.
///
/// These lines contain the declared name as an identifier, which could
/// collide with an import alias of the same short name.  We exclude them
/// from the content safety-net scan.
fn compute_declaration_line_ranges(content: &str) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut offset: usize = 0;

    for line in content.split('\n') {
        let trimmed = line.trim_start();
        if (trimmed.starts_with("class ")
            || trimmed.starts_with("interface ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("abstract class ")
            || trimmed.starts_with("final class ")
            || trimmed.starts_with("readonly class ")
            || trimmed.starts_with("final readonly class ")
            || trimmed.starts_with("readonly final class "))
            // Quick sanity: actual declarations, not comments/strings
            && !trimmed.starts_with("//")
        {
            ranges.push((offset, offset + line.len()));
        }
        offset += line.len() + 1;
    }

    ranges
}

/// Check whether a byte offset falls within any of the given ranges.
fn is_offset_in_ranges(offset: u32, ranges: &[ByteRange]) -> bool {
    let offset = offset as usize;
    ranges
        .iter()
        .any(|&(start, end)| offset >= start && offset < end)
}

/// Extract the first segment of a potentially qualified name.
///
/// - `"Foo"` → `"Foo"`
/// - `"Foo\\Bar"` → `"Foo"`
/// - `"\\Foo\\Bar"` → (skip leading backslash) `"Foo"`
fn extract_first_segment(name: &str) -> &str {
    let name = name.strip_prefix('\\').unwrap_or(name);
    name.split('\\').next().unwrap_or(name)
}

/// Check whether an alias name appears as an identifier reference in the
/// file content outside of `use` statements and class declarations.
///
/// This is a simple heuristic safety-net to reduce false positives.  It
/// looks for the alias name preceded and followed by a non-identifier
/// character (word boundary simulation), skipping occurrences on `use`
/// statement lines and class declaration lines.
fn alias_is_referenced_in_content(
    content: &str,
    alias: &str,
    _fqn: &str,
    use_ranges: &[ByteRange],
    decl_ranges: &[ByteRange],
) -> bool {
    let alias_bytes = alias.as_bytes();
    let content_bytes = content.as_bytes();
    let alias_len = alias_bytes.len();

    if alias_len == 0 {
        return false;
    }

    let mut search_from = 0;
    while search_from + alias_len <= content_bytes.len() {
        // Find the next occurrence of the alias string
        let pos = match content[search_from..].find(alias) {
            Some(p) => search_from + p,
            None => break,
        };

        // Check word boundaries
        let before_ok = if pos == 0 {
            true
        } else {
            !is_ident_char(content_bytes[pos - 1])
        };

        let after_ok = if pos + alias_len >= content_bytes.len() {
            true
        } else {
            !is_ident_char(content_bytes[pos + alias_len])
        };

        if before_ok && after_ok {
            // Skip if this occurrence falls on a `use` statement line.
            if is_offset_in_ranges(pos as u32, use_ranges) {
                search_from = pos + alias_len;
                continue;
            }

            // Skip if this occurrence falls on a class/interface/trait/enum
            // declaration line (the declared name matches the alias).
            if is_offset_in_ranges(pos as u32, decl_ranges) {
                search_from = pos + alias_len;
                continue;
            }

            // Skip occurrences inside single-line comments.
            let line_start = content[..pos].rfind('\n').map_or(0, |p| p + 1);
            let line_prefix = &content[line_start..pos];
            if line_prefix.contains("//")
                || line_prefix.trim_start().starts_with('*')
                || line_prefix.trim_start().starts_with("/**")
            {
                search_from = pos + alias_len;
                continue;
            }

            // Skip occurrences inside string literals — simple heuristic:
            // if there's an odd number of unescaped quotes before the match
            // on the same line, it's likely inside a string.  This isn't
            // perfect but avoids the most common false positives.

            // Found a real reference outside excluded lines
            return true;
        }

        search_from = pos + 1;
    }

    false
}

/// Check whether a byte is a valid PHP identifier character.
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\\' || b > 0x7F
}

/// Find the source range of the `use` statement that imports a given FQN
/// (or alias).
///
/// Scans the file content line by line for a `use` statement that contains
/// the FQN.  Returns the LSP range covering the entire `use` line.
///
/// For group imports (`use Foo\{Bar, Baz}`), if only one member is unused,
/// we highlight just the unused member name within the group.  If the entire
/// group is unused, we highlight the whole statement.
fn find_use_statement_range(content: &str, alias: &str, fqn: &str) -> Option<Range> {
    // The FQN's last segment or the alias — what appears in the `use` line.
    let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);
    let has_alias = short_name != alias;

    let mut byte_offset: usize = 0;

    for line in content.split('\n') {
        let trimmed = line.trim_start();
        let leading_ws = line.len() - trimmed.len();

        if trimmed.starts_with("use ") && trimmed.contains(';') {
            // Check if this use statement imports our FQN
            let is_match = if has_alias {
                // `use Foo\Bar as Alias;`
                trimmed.contains(fqn) && trimmed.contains(&format!("as {}", alias))
            } else if trimmed.contains('{') {
                // Group import: `use Foo\{Bar, Baz};`
                // Check if the FQN prefix matches and the short name is in the group
                is_group_import_match(trimmed, fqn, short_name)
            } else {
                // Simple: `use Foo\Bar;`
                trimmed.contains(fqn)
            };

            if is_match {
                // For group imports, try to highlight just the unused member
                if trimmed.contains('{')
                    && !has_alias
                    && let Some(member_range) =
                        find_group_member_range(content, byte_offset, line, short_name)
                {
                    return Some(member_range);
                }

                // Highlight the entire use statement line
                let line_start = byte_offset + leading_ws;
                let line_end = byte_offset + line.len();
                return offset_range_to_lsp_range(content, line_start, line_end);
            }
        }

        // +1 for the '\n' that split() consumed
        byte_offset += line.len() + 1;
    }

    None
}

/// Check if a group import line (`use Foo\{Bar, Baz};`) contains the
/// given FQN.
fn is_group_import_match(line: &str, fqn: &str, short_name: &str) -> bool {
    // Extract the prefix from `use Prefix\{...};`
    if let Some(brace_pos) = line.find('{') {
        let prefix_part = line["use ".len()..brace_pos].trim().trim_end_matches('\\');
        let expected_prefix = if let Some(prefix_end) = fqn.rfind('\\') {
            &fqn[..prefix_end]
        } else {
            return false;
        };

        if prefix_part == expected_prefix {
            // Check if short_name is in the group
            if let Some(close_brace) = line.find('}') {
                let group_content = &line[brace_pos + 1..close_brace];
                return group_content
                    .split(',')
                    .any(|item| item.trim() == short_name);
            }
        }
    }
    false
}

/// Find the range of a specific member within a group import.
///
/// For `use Foo\{Bar, Baz};` where `Bar` is unused, returns the range
/// covering just `Bar` (plus trailing comma/space if appropriate).
fn find_group_member_range(
    content: &str,
    line_byte_offset: usize,
    line: &str,
    short_name: &str,
) -> Option<Range> {
    let brace_pos = line.find('{')?;
    let close_brace = line.find('}')?;
    let group_content = &line[brace_pos + 1..close_brace];

    // Find the member's position within the group
    let members: Vec<&str> = group_content.split(',').collect();
    let member_count = members.len();

    let mut group_offset = brace_pos + 1; // offset within line, after '{'
    for (i, member) in members.iter().enumerate() {
        let trimmed = member.trim();
        if trimmed == short_name {
            // Found the member.  Calculate its byte range in content.
            let member_start_in_line = group_offset + member.find(trimmed).unwrap_or(0);
            let member_end_in_line = member_start_in_line + trimmed.len();

            // If this is the only member, highlight the whole use line
            if member_count == 1 {
                return None; // fall back to highlighting the whole line
            }

            let abs_start = line_byte_offset + member_start_in_line;
            let abs_end = line_byte_offset + member_end_in_line;

            return offset_range_to_lsp_range(content, abs_start, abs_end);
        }
        // Move past this member + the comma
        group_offset += member.len();
        if i < member_count - 1 {
            group_offset += 1; // for the comma
        }
    }

    None
}
