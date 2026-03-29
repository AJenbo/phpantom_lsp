//! "Add `#[\ReturnTypeWillChange]`" code action for PHPStan
//! `method.tentativeReturnType`.
//!
//! When PHPStan reports that a method has a tentative return type and
//! suggests using `#[\ReturnTypeWillChange]` to suppress the error,
//! this code action offers to insert the attribute on the line above
//! the method declaration, with correct indentation.
//!
//! **Trigger:** A PHPStan diagnostic with identifier
//! `method.tentativeReturnType` overlaps the cursor.
//!
//! **Code action kind:** `quickfix`.
//!
//! ## Two-phase resolve
//!
//! Phase 1 (`collect_add_return_type_will_change_actions`) validates
//! and emits a lightweight `CodeAction` with a `data` payload but no
//! `edit`.  Phase 2 (`resolve_add_return_type_will_change`) computes
//! the workspace edit on demand when the user picks the action.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::code_actions::{CodeActionData, make_code_action_data};
use crate::util::ranges_overlap;

/// The PHPStan identifier we match on.
const TENTATIVE_RETURN_TYPE_ID: &str = "method.tentativeReturnType";

/// The attribute to insert (always FQN — `ReturnTypeWillChange` lives
/// in the global namespace and has no short-form import convention).
const ATTRIBUTE_TEXT: &str = "#[\\ReturnTypeWillChange]";

impl Backend {
    /// Collect "Add `#[\\ReturnTypeWillChange]`" code actions for
    /// PHPStan `method.tentativeReturnType` diagnostics.
    ///
    /// **Phase 1**: validates the action is applicable and emits a
    /// lightweight `CodeAction` with a `data` payload but **no `edit`**.
    /// The edit is computed lazily in
    /// [`resolve_add_return_type_will_change`](Self::resolve_add_return_type_will_change).
    pub(crate) fn collect_add_return_type_will_change_actions(
        &self,
        uri: &str,
        content: &str,
        params: &CodeActionParams,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        let phpstan_diags: Vec<Diagnostic> = {
            let cache = self.phpstan_last_diags.lock();
            cache.get(uri).cloned().unwrap_or_default()
        };

        for diag in &phpstan_diags {
            if !ranges_overlap(&diag.range, &params.range) {
                continue;
            }

            let identifier = match &diag.code {
                Some(NumberOrString::String(s)) => s.as_str(),
                _ => continue,
            };

            if identifier != TENTATIVE_RETURN_TYPE_ID {
                continue;
            }

            let diag_line = diag.range.start.line as usize;

            let Some(insertion) = find_method_insertion_point(content, diag_line) else {
                continue;
            };

            // If the attribute is already present (user added it
            // manually since PHPStan last ran), skip.
            if already_has_return_type_will_change(content, &insertion) {
                continue;
            }

            let method_name = extract_method_name(&diag.message).unwrap_or("method");
            let title = format!("Add {} to {}", ATTRIBUTE_TEXT, method_name);

            let extra = serde_json::json!({
                "diagnostic_message": diag.message,
                "diagnostic_line": diag.range.start.line,
                "diagnostic_code": TENTATIVE_RETURN_TYPE_ID,
            });

            let data =
                make_code_action_data("phpstan.addReturnTypeWillChange", uri, &params.range, extra);

            out.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diag.clone()]),
                edit: None,
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(data),
            }));
        }
    }

    /// Resolve the "Add `#[\\ReturnTypeWillChange]`" code action by
    /// computing the full workspace edit.
    ///
    /// **Phase 2**: called from
    /// [`resolve_code_action`](Self::resolve_code_action) when the user
    /// picks this action.
    pub(crate) fn resolve_add_return_type_will_change(
        &self,
        data: &CodeActionData,
        content: &str,
    ) -> Option<WorkspaceEdit> {
        let uri = &data.uri;
        let diag_line = data.extra.get("diagnostic_line")?.as_u64()? as usize;

        let insertion = find_method_insertion_point(content, diag_line)?;

        // If the attribute was added since the action was offered, bail.
        if already_has_return_type_will_change(content, &insertion) {
            return None;
        }

        // Build the text edit: insert `#[\ReturnTypeWillChange]\n<indent>`
        // at the insertion point (before any existing attributes or
        // modifiers).
        let insert_text = format!("{}{}\n", insertion.indent, ATTRIBUTE_TEXT);
        let insert_pos = byte_offset_to_lsp(content, insertion.insert_offset);

        let edits = vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: insert_text,
        }];

        let doc_uri: Url = uri.parse().ok()?;
        let mut changes = HashMap::new();
        changes.insert(doc_uri, edits);

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Information about where to insert an attribute above a method.
struct InsertionPoint {
    /// The byte offset where the attribute line should be inserted.
    /// This is the start of the line containing the first token of
    /// the method declaration (attribute, modifier, or `function`).
    insert_offset: usize,
    /// The indentation whitespace of the method declaration line.
    indent: String,
    /// The byte offset of the start of the first attribute list (if
    /// any), or the start of the first modifier / `function` keyword.
    first_token_offset: usize,
    /// The byte offset just past the end of the last attribute list
    /// before the modifiers/function keyword.  If no attributes exist,
    /// this equals `first_token_offset`.
    attrs_end_offset: usize,
}

/// Extract the method name from a PHPStan `method.tentativeReturnType`
/// message.
///
/// Expected format:
/// - `"Return type (int) of method Foo::count() should be covariant
///    with return type (int) of method Countable::count()"`
///
/// We extract just the short method name from the first `::name()`
/// occurrence.
fn extract_method_name(message: &str) -> Option<&str> {
    // Find `method <Class>::<name>()` — the first occurrence is the
    // overriding method.
    let marker = "method ";
    let pos = message.find(marker)?;
    let after = &message[pos + marker.len()..];
    let paren_pos = after.find('(')?;
    let class_and_name = &after[..paren_pos];
    let name = class_and_name.rsplit("::").next()?;
    if name.is_empty() {
        return None;
    }
    Some(name)
}

/// Find the insertion point for an attribute on a method whose PHPStan
/// diagnostic is on `diag_line`.
///
/// Walks backward from the diagnostic line to find:
/// 1. The `function` keyword on or before the diagnostic line
/// 2. Any modifiers (`public`, `static`, etc.) before `function`
/// 3. Any attribute lists (`#[...]`) before the modifiers
///
/// The insertion point is the start of the line containing the
/// earliest attribute list, or the start of the line containing the
/// first modifier/`function` keyword if no attributes exist.
fn find_method_insertion_point(content: &str, diag_line: usize) -> Option<InsertionPoint> {
    let lines: Vec<&str> = content.lines().collect();
    if diag_line >= lines.len() {
        return None;
    }

    // Find the `function` keyword on or near the diagnostic line.
    let search_start = diag_line.min(lines.len().saturating_sub(1));
    let mut func_line = None;
    for i in (search_start.saturating_sub(5)..=search_start).rev() {
        if contains_function_keyword(lines[i]) {
            func_line = Some(i);
            break;
        }
    }
    let func_line = func_line?;

    // Walk backward from the function line past modifier keywords.
    let mut first_decl_line = func_line;
    let mut check_line = func_line;
    loop {
        if check_line == 0 {
            break;
        }
        let prev = check_line - 1;
        let prev_trimmed = lines[prev].trim();
        if prev_trimmed.is_empty() {
            break;
        }
        if is_modifier_line(prev_trimmed) {
            first_decl_line = prev;
            check_line = prev;
            continue;
        }
        break;
    }

    // Walk backward from `first_decl_line` to find attribute lists.
    let mut first_attr_line = first_decl_line;
    let mut check_line = first_decl_line;
    loop {
        if check_line == 0 {
            break;
        }
        let prev = check_line - 1;
        let prev_trimmed = lines[prev].trim();
        if prev_trimmed.is_empty() {
            break;
        }
        if is_attribute_line(prev_trimmed) {
            first_attr_line = prev;
            check_line = prev;
            continue;
        }
        break;
    }

    let target_line = first_attr_line;
    let insert_offset = line_byte_offset(content, target_line);

    let indent: String = lines[func_line]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();

    let first_token_offset = insert_offset;

    let attrs_end_offset = if first_attr_line < first_decl_line {
        line_byte_offset(content, first_decl_line)
    } else {
        first_token_offset
    };

    Some(InsertionPoint {
        insert_offset,
        indent,
        first_token_offset,
        attrs_end_offset,
    })
}

/// Check if the method already has a `#[ReturnTypeWillChange]` or
/// `#[\ReturnTypeWillChange]` attribute.
fn already_has_return_type_will_change(content: &str, insertion: &InsertionPoint) -> bool {
    // Check existing attribute lines above the method.
    if insertion.attrs_end_offset > insertion.first_token_offset {
        let attr_region = &content[insertion.first_token_offset..insertion.attrs_end_offset];
        for line in attr_region.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[") && contains_rtwc_attr(trimmed) {
                return true;
            }
        }
    }

    // Also check a few lines above the insertion point in case the
    // attribute is on its own line before other attributes.
    let lines: Vec<&str> = content.lines().collect();
    let insert_line = content[..insertion.insert_offset]
        .chars()
        .filter(|&c| c == '\n')
        .count();

    let search_start = insert_line.saturating_sub(3);
    for i in search_start..=insert_line {
        if i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("#[") && contains_rtwc_attr(trimmed) {
                return true;
            }
        }
    }

    false
}

/// Check if a `#[...]` line contains `ReturnTypeWillChange` as an
/// attribute name.
fn contains_rtwc_attr(line: &str) -> bool {
    let bytes = line.as_bytes();
    let target = b"ReturnTypeWillChange";
    let target_len = target.len();

    let mut i = 0;
    while i + target_len <= bytes.len() {
        if &bytes[i..i + target_len] == target {
            let ok_before = if i == 0 {
                false
            } else {
                let prev = bytes[i - 1];
                prev == b'[' || prev == b'\\' || prev == b',' || prev == b' ' || prev == b'\t'
            };
            let ok_after = if i + target_len >= bytes.len() {
                true
            } else {
                let next = bytes[i + target_len];
                next == b']' || next == b',' || next == b'(' || next == b' ' || next == b'\t'
            };
            if ok_before && ok_after {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Check if a line contains the `function` keyword as a standalone word.
fn contains_function_keyword(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(pos) = trimmed.find("function") else {
        return false;
    };
    let before_ok = pos == 0 || trimmed.as_bytes()[pos - 1].is_ascii_whitespace();
    let after_pos = pos + "function".len();
    let after_ok = after_pos >= trimmed.len()
        || (!trimmed.as_bytes()[after_pos].is_ascii_alphanumeric()
            && trimmed.as_bytes()[after_pos] != b'_');
    before_ok && after_ok
}

/// Check if a trimmed line starts with a PHP modifier keyword.
fn is_modifier_line(trimmed: &str) -> bool {
    let modifiers = [
        "public",
        "protected",
        "private",
        "static",
        "abstract",
        "final",
        "readonly",
    ];
    modifiers.iter().any(|kw| {
        trimmed.starts_with(kw)
            && trimmed[kw.len()..].starts_with(|c: char| c.is_whitespace() || c == '\0')
    })
}

/// Check if a trimmed line is a PHP attribute line (`#[...]`).
fn is_attribute_line(trimmed: &str) -> bool {
    trimmed.starts_with("#[")
}

/// Compute the byte offset of the start of the given line number
/// (0-based).
fn line_byte_offset(content: &str, line: usize) -> usize {
    let mut offset = 0;
    for (i, l) in content.lines().enumerate() {
        if i == line {
            return offset;
        }
        offset += l.len() + 1; // +1 for newline
    }
    content.len()
}

/// Convert a byte offset to an LSP `Position`.
fn byte_offset_to_lsp(content: &str, offset: usize) -> Position {
    let before = &content[..offset.min(content.len())];
    let line = before.chars().filter(|&c| c == '\n').count() as u32;
    let last_newline = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let character = content[last_newline..offset].chars().count() as u32;
    Position { line, character }
}

/// Check whether a `method.tentativeReturnType` diagnostic is stale
/// by verifying that the `#[\ReturnTypeWillChange]` attribute is now
/// present near the diagnostic line.
pub(crate) fn is_add_return_type_will_change_stale(content: &str, diag_line: usize) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if diag_line >= lines.len() {
        return false;
    }

    // Search backward from the diagnostic line for the attribute.
    let search_start = diag_line.saturating_sub(10);
    for i in (search_start..=diag_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("#[") && contains_rtwc_attr(trimmed) {
            return true;
        }
    }
    false
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_method_name ─────────────────────────────────────────

    #[test]
    fn extracts_method_name_from_tentative_message() {
        let msg = "Return type (int) of method Foo::count() should be covariant with return type (int) of method Countable::count()";
        assert_eq!(extract_method_name(msg), Some("count"));
    }

    #[test]
    fn extracts_method_name_with_namespace() {
        let msg = "Return type (array) of method App\\Collection::toArray() should be covariant with return type (array) of method ArrayAccess::toArray()";
        assert_eq!(extract_method_name(msg), Some("toArray"));
    }

    #[test]
    fn returns_none_for_unrelated_message() {
        let msg = "Some other PHPStan error about something.";
        assert_eq!(extract_method_name(msg), None);
    }

    // ── contains_rtwc_attr ──────────────────────────────────────────

    #[test]
    fn finds_rtwc_simple() {
        assert!(contains_rtwc_attr("#[ReturnTypeWillChange]"));
    }

    #[test]
    fn finds_rtwc_with_backslash() {
        assert!(contains_rtwc_attr("#[\\ReturnTypeWillChange]"));
    }

    #[test]
    fn finds_rtwc_in_list() {
        assert!(contains_rtwc_attr("#[ReturnTypeWillChange, Deprecated]"));
        assert!(contains_rtwc_attr("#[Deprecated, ReturnTypeWillChange]"));
        assert!(contains_rtwc_attr("#[Deprecated, \\ReturnTypeWillChange]"));
    }

    #[test]
    fn does_not_match_partial() {
        assert!(!contains_rtwc_attr("#[ReturnTypeWillChangeSomething]"));
        assert!(!contains_rtwc_attr("#[MyReturnTypeWillChange]"));
    }

    // ── find_method_insertion_point ──────────────────────────────────

    #[test]
    fn finds_insertion_for_simple_method() {
        let content = "<?php\nclass Foo {\n    public function bar(): void {}\n}\n";
        let line = 2;
        let ins = find_method_insertion_point(content, line).unwrap();
        assert_eq!(ins.indent, "    ");
        let expected_offset = content.find("    public function").unwrap();
        assert_eq!(ins.insert_offset, expected_offset);
    }

    #[test]
    fn finds_insertion_for_method_with_existing_attributes() {
        let content =
            "<?php\nclass Foo {\n    #[Route('/bar')]\n    public function bar(): void {}\n}\n";
        let line = 3;
        let ins = find_method_insertion_point(content, line).unwrap();
        assert_eq!(ins.indent, "    ");
        let expected_offset = content.find("    #[Route").unwrap();
        assert_eq!(ins.insert_offset, expected_offset);
    }

    #[test]
    fn finds_insertion_with_multiple_attributes() {
        let content = "<?php\nclass Foo {\n    #[Route('/bar')]\n    #[Deprecated]\n    public function bar(): void {}\n}\n";
        let line = 4;
        let ins = find_method_insertion_point(content, line).unwrap();
        let expected_offset = content.find("    #[Route").unwrap();
        assert_eq!(ins.insert_offset, expected_offset);
    }

    // ── already_has_return_type_will_change ──────────────────────────

    #[test]
    fn detects_existing_rtwc() {
        let content = "<?php\nclass Foo {\n    #[\\ReturnTypeWillChange]\n    public function count(): int {}\n}\n";
        let line = 3;
        let ins = find_method_insertion_point(content, line).unwrap();
        assert!(already_has_return_type_will_change(content, &ins));
    }

    #[test]
    fn no_rtwc_when_absent() {
        let content = "<?php\nclass Foo {\n    public function count(): int {}\n}\n";
        let line = 2;
        let ins = find_method_insertion_point(content, line).unwrap();
        assert!(!already_has_return_type_will_change(content, &ins));
    }

    #[test]
    fn detects_rtwc_without_backslash() {
        let content = "<?php\nclass Foo {\n    #[ReturnTypeWillChange]\n    public function count(): int {}\n}\n";
        let line = 3;
        let ins = find_method_insertion_point(content, line).unwrap();
        assert!(already_has_return_type_will_change(content, &ins));
    }

    // ── is_add_return_type_will_change_stale ─────────────────────────

    #[test]
    fn stale_when_rtwc_present() {
        let content = "<?php\nclass Foo {\n    #[\\ReturnTypeWillChange]\n    public function count(): int {}\n}\n";
        assert!(is_add_return_type_will_change_stale(content, 3));
    }

    #[test]
    fn not_stale_when_rtwc_absent() {
        let content = "<?php\nclass Foo {\n    public function count(): int {}\n}\n";
        assert!(!is_add_return_type_will_change_stale(content, 2));
    }

    // ── Integration: build edit text ────────────────────────────────

    #[test]
    fn builds_correct_rtwc_text() {
        let content = "<?php\nclass Foo {\n    public function count(): int {}\n}\n";
        let line = 2;
        let ins = find_method_insertion_point(content, line).unwrap();
        let insert_text = format!("{}{}\n", ins.indent, ATTRIBUTE_TEXT);
        assert_eq!(insert_text, "    #[\\ReturnTypeWillChange]\n");
    }

    #[test]
    fn builds_correct_rtwc_text_nested() {
        let content = "<?php\nclass Foo {\n        protected function count(): int {}\n}\n";
        let line = 2;
        let ins = find_method_insertion_point(content, line).unwrap();
        let insert_text = format!("{}{}\n", ins.indent, ATTRIBUTE_TEXT);
        assert_eq!(insert_text, "        #[\\ReturnTypeWillChange]\n");
    }

    // ── Integration: full code action via Backend ───────────────────

    #[test]
    fn offers_add_rtwc_action() {
        let backend = crate::Backend::defaults();
        let uri = "file:///test.php";
        let content = r#"<?php
class MyCollection implements \Countable {
    public function count(): int { return 0; }
}
"#;
        backend.update_ast(uri, content);
        backend
            .open_files
            .write()
            .insert(uri.to_string(), std::sync::Arc::new(content.to_string()));

        let diag = Diagnostic {
            range: Range {
                start: Position::new(2, 0),
                end: Position::new(2, 80),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(TENTATIVE_RETURN_TYPE_ID.to_string())),
            source: Some("PHPStan".to_string()),
            message: "Return type (int) of method MyCollection::count() should be covariant with return type (int) of method Countable::count()\nMake it covariant, or use the #[\\ReturnTypeWillChange] attribute to temporarily suppress the error.".to_string(),
            ..Default::default()
        };
        {
            let mut cache = backend.phpstan_last_diags().lock();
            cache.entry(uri.to_string()).or_default().push(diag);
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(2, 4),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let rtwc_action = actions.iter().find_map(|a| match a {
            CodeActionOrCommand::CodeAction(ca) if ca.title.contains("ReturnTypeWillChange") => {
                Some(ca)
            }
            _ => None,
        });

        assert!(
            rtwc_action.is_some(),
            "should offer Add #[\\ReturnTypeWillChange] action"
        );

        let action = rtwc_action.unwrap();
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        assert_eq!(action.is_preferred, Some(true));
        assert!(
            action.title.contains("count"),
            "title should mention method name: {}",
            action.title
        );

        // Phase 1: edit should be None, data should be Some.
        assert!(action.edit.is_none(), "Phase 1 should not compute the edit");
        assert!(
            action.data.is_some(),
            "Phase 1 should set data for deferred resolve"
        );

        // Phase 2: resolve the action to get the edit.
        let (resolved, _republish) = backend.resolve_code_action(action.clone());
        let edit = resolved
            .edit
            .as_ref()
            .expect("resolve should produce an edit");
        let changes = edit.changes.as_ref().unwrap();
        let edits: Vec<&TextEdit> = changes.values().flat_map(|v| v.iter()).collect();
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.contains("#[\\ReturnTypeWillChange]"));
    }

    #[test]
    fn no_action_when_rtwc_already_present() {
        let backend = crate::Backend::defaults();
        let uri = "file:///test.php";
        let content = r#"<?php
class MyCollection implements \Countable {
    #[\ReturnTypeWillChange]
    public function count(): int { return 0; }
}
"#;
        backend.update_ast(uri, content);

        let diag = Diagnostic {
            range: Range {
                start: Position::new(3, 0),
                end: Position::new(3, 80),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(TENTATIVE_RETURN_TYPE_ID.to_string())),
            source: Some("PHPStan".to_string()),
            message: "Return type (int) of method MyCollection::count() should be covariant with return type (int) of method Countable::count()".to_string(),
            ..Default::default()
        };
        {
            let mut cache = backend.phpstan_last_diags().lock();
            cache.entry(uri.to_string()).or_default().push(diag);
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(3, 4),
                end: Position::new(3, 4),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let rtwc_action = actions.iter().find_map(|a| match a {
            CodeActionOrCommand::CodeAction(ca) if ca.title.contains("ReturnTypeWillChange") => {
                Some(ca)
            }
            _ => None,
        });

        assert!(
            rtwc_action.is_none(),
            "should NOT offer action when #[\\ReturnTypeWillChange] already present"
        );
    }

    #[test]
    fn no_action_for_other_identifiers() {
        let backend = crate::Backend::defaults();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}
}
"#;
        backend.update_ast(uri, content);

        let diag = Diagnostic {
            range: Range {
                start: Position::new(2, 0),
                end: Position::new(2, 80),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("return.unusedType".to_string())),
            source: Some("PHPStan".to_string()),
            message: "Some other error.".to_string(),
            ..Default::default()
        };
        {
            let mut cache = backend.phpstan_last_diags().lock();
            cache.entry(uri.to_string()).or_default().push(diag);
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(2, 4),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let rtwc_action = actions.iter().find_map(|a| match a {
            CodeActionOrCommand::CodeAction(ca) if ca.title.contains("ReturnTypeWillChange") => {
                Some(ca)
            }
            _ => None,
        });

        assert!(
            rtwc_action.is_none(),
            "should NOT offer action for non-tentativeReturnType identifiers"
        );
    }

    #[test]
    fn inserts_before_existing_attributes() {
        let backend = crate::Backend::defaults();
        let uri = "file:///test.php";
        let content = r#"<?php
class MyCollection implements \Countable {
    #[SomeAttr]
    public function count(): int { return 0; }
}
"#;
        backend.update_ast(uri, content);
        backend
            .open_files
            .write()
            .insert(uri.to_string(), std::sync::Arc::new(content.to_string()));

        let diag = Diagnostic {
            range: Range {
                start: Position::new(3, 0),
                end: Position::new(3, 80),
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(TENTATIVE_RETURN_TYPE_ID.to_string())),
            source: Some("PHPStan".to_string()),
            message: "Return type (int) of method MyCollection::count() should be covariant with return type (int) of method Countable::count()".to_string(),
            ..Default::default()
        };
        {
            let mut cache = backend.phpstan_last_diags().lock();
            cache.entry(uri.to_string()).or_default().push(diag);
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(3, 4),
                end: Position::new(3, 4),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let action = actions
            .iter()
            .find_map(|a| match a {
                CodeActionOrCommand::CodeAction(ca)
                    if ca.title.contains("ReturnTypeWillChange") =>
                {
                    Some(ca)
                }
                _ => None,
            })
            .expect("should offer action");

        // Phase 2: resolve to get the edit.
        let (resolved, _) = backend.resolve_code_action(action.clone());
        let edit = resolved.edit.as_ref().expect("resolve should produce edit");
        let changes = edit.changes.as_ref().unwrap();
        let edits: Vec<&TextEdit> = changes.values().flat_map(|v| v.iter()).collect();

        // The insertion position should be before the `#[SomeAttr]`
        // line (line 2), not before the `public function` line.
        assert_eq!(
            edits[0].range.start.line, 2,
            "should insert before existing attributes"
        );
        assert!(edits[0].new_text.contains("#[\\ReturnTypeWillChange]"));
    }
}
