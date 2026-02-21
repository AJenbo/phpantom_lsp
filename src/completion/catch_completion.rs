//! Smart catch clause exception completion.
//!
//! When the cursor is inside `catch (|)` or `catch (Partial|)`, this
//! module analyses the corresponding `try` block to find all exception
//! types that can be thrown, and suggests them as completions.
//!
//! Sources of thrown exception types (in priority order):
//!   1. `throw new ExceptionType(…)` statements in the try block
//!   2. Inline `/** @throws ExceptionType */` annotations in the try block
//!   3. Propagated `@throws` from methods called in the try block
//!   4. `throw $this->method()` / `throw self::method()` return types
//!
//! The inline `/** @throws */` annotation is an escape hatch that lets
//! developers document exceptions from dependencies that don't have
//! `@throws` tags themselves.

use tower_lsp::lsp_types::*;

use super::comment_position::position_to_byte_offset;
use super::throws_analysis;

/// Information about the catch clause context at the cursor position.
#[derive(Debug)]
pub struct CatchContext {
    /// The partial class name the user has typed so far (may be empty).
    pub partial: String,
    /// Exception type names found in the corresponding try block.
    pub suggested_types: Vec<String>,
    /// Whether specific thrown types were discovered in the try block.
    /// When `false`, the caller should fall back to generic class
    /// completion instead of showing only `Throwable`.
    pub has_specific_types: bool,
}

/// Detect whether the cursor is inside a `catch (…)` clause's type
/// position, and if so, return a [`CatchContext`] with the try block's
/// thrown exception types.
///
/// Returns `None` if the cursor is not in a catch clause type position.
pub fn detect_catch_context(content: &str, position: Position) -> Option<CatchContext> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    // Walk backward from cursor to find the opening `(` of the catch clause,
    // collecting what's been typed so far.
    let (catch_paren_offset, partial, already_listed) = find_catch_paren(before_cursor)?;

    // From the `(` position, walk backward to find the `catch` keyword.
    let before_paren = &content[..catch_paren_offset];
    let trimmed = before_paren.trim_end();
    if !trimmed.ends_with("catch") {
        return None;
    }

    // Verify `catch` is a whole word
    let catch_end = trimmed.len();
    let catch_start = catch_end - 5;
    if catch_start > 0 {
        let prev_byte = trimmed.as_bytes()[catch_start - 1];
        if prev_byte.is_ascii_alphanumeric() || prev_byte == b'_' {
            return None;
        }
    }

    // Now find the matching try block by scanning backward from `catch`.
    let before_catch = trimmed[..catch_start].trim_end();

    // The text just before `catch` should be `}` (closing the try block
    // or a previous catch block).
    if !before_catch.ends_with('}') {
        return None;
    }

    // Find the try block: walk back through possible catch/finally blocks
    // to find the original `try {`.
    let try_body = find_try_block_body(content, before_catch)?;

    // Analyse the try block for thrown exception types.
    let mut suggested_types = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Direct `throw new ExceptionType(…)` statements
    let throws = throws_analysis::find_throw_statements(&try_body);
    for throw in &throws {
        let short_name = throw
            .type_name
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(&throw.type_name);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 2. Inline `/** @throws ExceptionType */` annotations
    let inline_throws = throws_analysis::find_inline_throws_annotations(&try_body);
    for exc_type in &inline_throws {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 3. Propagated @throws from called methods
    let propagated = throws_analysis::find_propagated_throws(&try_body, content);
    let propagated: Vec<String> = propagated.iter().map(|t| t.type_name.clone()).collect();
    for exc_type in &propagated {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 4. `throw $this->method()` / `throw self::method()` return types
    let throw_expr_types = throws_analysis::find_throw_expression_types(&try_body, content);
    let throw_expr_types: Vec<String> = throw_expr_types
        .iter()
        .map(|t| t.type_name.clone())
        .collect();
    for exc_type in &throw_expr_types {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // Track whether we found any specific thrown types before adding
    // the universal Throwable fallback.
    let has_specific_types = !suggested_types.is_empty();

    // Always offer \Throwable as a catch-all safety net
    if seen.insert("throwable".to_string()) {
        suggested_types.push("\\Throwable".to_string());
    }

    // Filter out types already listed in this catch clause
    let already_lower: Vec<String> = already_listed.iter().map(|s| s.to_lowercase()).collect();
    suggested_types.retain(|t| !already_lower.contains(&t.to_lowercase()));

    Some(CatchContext {
        partial,
        suggested_types,
        has_specific_types,
    })
}

/// Build LSP completion items from a [`CatchContext`].
///
/// Smart exception suggestions sort before any fallback items.
/// `\Throwable` is always offered but sorted last among the suggestions.
pub fn build_catch_completions(ctx: &CatchContext) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let partial_lower = ctx.partial.to_lowercase();

    for (idx, exc_type) in ctx.suggested_types.iter().enumerate() {
        let label = exc_type.trim_start_matches('\\');

        // Filter by the partial text the user has typed
        if !partial_lower.is_empty() && !label.to_lowercase().starts_with(&partial_lower) {
            continue;
        }

        // Sort \Throwable after specific exception types
        let sort_text = if exc_type.starts_with('\\') {
            format!("1_{:03}_{}", idx, label)
        } else {
            format!("0_{:03}_{}", idx, label)
        };

        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Exception thrown in try block".to_string()),
            sort_text: Some(sort_text),
            filter_text: Some(label.to_string()),
            ..CompletionItem::default()
        });
    }

    items
}

// ─── Internal helpers ───────────────────────────────────────────────────────

/// Walk backward from the cursor position to find the `(` of a catch clause.
///
/// Returns `(paren_byte_offset, partial_typed, already_listed_types)` or
/// `None` if no suitable `(` is found.
fn find_catch_paren(before_cursor: &str) -> Option<(usize, String, Vec<String>)> {
    let bytes = before_cursor.as_bytes();
    let mut pos = bytes.len();
    let mut depth = 0i32;

    // Walk backward collecting the text inside the parentheses
    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    // Found our opening paren
                    let inside = &before_cursor[pos + 1..];
                    let (partial, already_listed) = parse_catch_paren_content(inside);
                    return Some((pos, partial, already_listed));
                }
                depth -= 1;
            }
            // Stop at semicolons, opening braces — we've gone too far
            b';' | b'{' => return None,
            _ => {}
        }
    }

    None
}

/// Parse the content inside `catch (` up to the cursor.
///
/// For `catch (IOException | ` the partial is `""` and already_listed is
/// `["IOException"]`.
///
/// For `catch (IOEx` the partial is `"IOEx"` and already_listed is `[]`.
///
/// For `catch (IOException | Time` the partial is `"Time"` and
/// already_listed is `["IOException"]`.
fn parse_catch_paren_content(inside: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = inside.split('|').collect();
    let mut already_listed = Vec::new();

    if parts.len() <= 1 {
        // No `|` separator — everything is the partial
        let partial = inside.trim().trim_start_matches('\\').to_string();
        return (partial, already_listed);
    }

    // Everything except the last segment is an already-listed type
    for part in &parts[..parts.len() - 1] {
        let t = part.trim().trim_start_matches('\\');
        if !t.is_empty() {
            // Strip the variable name if present (shouldn't be before `|`, but be safe)
            let type_name = t.split_whitespace().next().unwrap_or(t);
            if !type_name.starts_with('$') {
                already_listed.push(type_name.to_string());
            }
        }
    }

    // The last segment is the partial
    let last = parts.last().unwrap_or(&"");
    let partial = last.trim().trim_start_matches('\\').to_string();

    (partial, already_listed)
}

/// Find the try block body by walking backward from the `}` that precedes
/// the `catch` keyword.
///
/// Handles chains like `try { … } catch (A $a) { … } catch (|)` by
/// walking back through previous catch (and finally) blocks to find the
/// original `try {`.
fn find_try_block_body(_content: &str, before_catch: &str) -> Option<String> {
    // `before_catch` ends with `}`. Find the matching `{`.
    let close_brace_offset = before_catch.len() - 1;

    // We need the absolute position in `content`. `before_catch` is a
    // prefix of `content` (after trimming), so we can use its length.
    // But actually, `before_catch` was derived by slicing `content`, so
    // we need to find where this `}` is in the full content.
    //
    // Walk backward to find the matching `{`.
    let block_open =
        crate::util::find_matching_backward(before_catch, close_brace_offset, b'{', b'}')?;

    // Now check what keyword precedes this block.
    let before_block = before_catch[..block_open].trim_end();

    // Check for `)` — this block was a catch block
    if before_block.ends_with(')') {
        // Skip the catch clause parentheses
        let close_paren = before_block.len() - 1;
        let open_paren =
            crate::util::find_matching_backward(before_block, close_paren, b'(', b')')?;
        let before_paren = before_block[..open_paren].trim_end();

        // Should be `catch`
        if before_paren.ends_with("catch") {
            let kw_start = before_paren.len() - 5;
            // Verify whole word
            if kw_start == 0
                || (!before_paren.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                    && before_paren.as_bytes()[kw_start - 1] != b'_')
            {
                // Must be preceded by `}` of the previous block
                let before_kw = before_paren[..kw_start].trim_end();
                if before_kw.ends_with('}') {
                    // Recurse to find the try block
                    return find_try_block_body(_content, before_kw);
                }
            }
        }
        return None;
    }

    // Check for `finally`
    if before_block.ends_with("finally") {
        let kw_start = before_block.len() - 7;
        if kw_start == 0
            || (!before_block.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                && before_block.as_bytes()[kw_start - 1] != b'_')
        {
            let before_kw = before_block[..kw_start].trim_end();
            if before_kw.ends_with('}') {
                return find_try_block_body(_content, before_kw);
            }
        }
        return None;
    }

    // Check for `try`
    if before_block.ends_with("try") {
        let kw_start = before_block.len() - 3;
        if kw_start == 0
            || (!before_block.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                && before_block.as_bytes()[kw_start - 1] != b'_')
        {
            // Found it! Extract the try block body.
            let body = &before_catch[block_open + 1..close_brace_offset];
            return Some(body.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::throws_analysis;

    #[test]
    fn test_find_method_throws_tags_with_private() {
        let content = concat!(
            "<?php\n",
            "class Foo {\n",
            "    /** @throws ValidationException */\n",
            "    private function riskyOperation(): void {}\n",
            "}\n",
        );
        let result = throws_analysis::find_method_throws_tags(content, "riskyOperation");
        assert_eq!(
            result,
            vec!["ValidationException"],
            "Should find @throws through 'private' modifier"
        );
    }

    #[test]
    fn test_find_method_throws_tags_with_protected_static() {
        let content = concat!(
            "<?php\n",
            "class Foo {\n",
            "    /** @throws RuntimeException */\n",
            "    protected static function dangerousCall(): void {}\n",
            "}\n",
        );
        let result = throws_analysis::find_method_throws_tags(content, "dangerousCall");
        assert_eq!(
            result,
            vec!["RuntimeException"],
            "Should find @throws through 'protected static' modifiers"
        );
    }

    #[test]
    fn test_find_method_throws_tags_without_modifier() {
        let content = concat!(
            "<?php\n",
            "/** @throws LogicException */\n",
            "function standalone(): void {}\n",
        );
        let result = throws_analysis::find_method_throws_tags(content, "standalone");
        assert_eq!(
            result,
            vec!["LogicException"],
            "Should find @throws on a standalone function (no modifier)"
        );
    }

    #[test]
    fn test_propagated_throws_with_visibility_in_catch() {
        // Full file content — cursor will be inside catch()
        //                                                    v cursor (line 5, char 17)
        // Line 0: <?php
        // Line 1: class Foo {
        // Line 2:     public function doStuff(): void {
        // Line 3:         try {
        // Line 4:             $this->riskyOperation();
        // Line 5:         } catch () {}
        // Line 6:     }
        // Line 7:
        // Line 8:     /** @throws ValidationException */
        // Line 9:     private function riskyOperation(): void {}
        // Line 10: }
        let full_content = concat!(
            "<?php\n",
            "class Foo {\n",
            "    public function doStuff(): void {\n",
            "        try {\n",
            "            $this->riskyOperation();\n",
            "        } catch () {}\n",
            "    }\n",
            "\n",
            "    /** @throws ValidationException */\n",
            "    private function riskyOperation(): void {}\n",
            "}\n",
        );

        // Character 17 is between `(` (char 16) and `)` (char 17) on line 5
        let pos = Position {
            line: 5,
            character: 17,
        };
        let ctx = detect_catch_context(full_content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"ValidationException".to_string()),
            "Should suggest ValidationException from propagated @throws on private method, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_propagated_throws_with_protected_static_in_catch() {
        let full_content = concat!(
            "<?php\n",
            "class Bar {\n",
            "    public function handle(): void {\n",
            "        try {\n",
            "            $this->dangerousCall();\n",
            "        } catch () {}\n",
            "    }\n",
            "\n",
            "    /** @throws RuntimeException */\n",
            "    protected static function dangerousCall(): void {}\n",
            "}\n",
        );

        let pos = Position {
            line: 5,
            character: 17,
        };
        let ctx = detect_catch_context(full_content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string()),
            "Should suggest RuntimeException through protected static modifier, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_find_inline_throws_annotations_in_catch() {
        let body = r#"
            /** @throws ModelNotFoundException */
            $model = SomeService::find($id);
            /** @throws \App\Exceptions\AuthException */
            $auth = doSomething();
        "#;
        let result = throws_analysis::find_inline_throws_annotations(body);
        // Raw names are returned; short-name extraction happens in detect_catch_context
        assert_eq!(
            result,
            vec!["ModelNotFoundException", "App\\Exceptions\\AuthException"]
        );
    }

    #[test]
    fn test_find_inline_throws_multiline_docblock_in_catch() {
        let body = r#"
            /**
             * @throws RuntimeException
             */
            doStuff();
        "#;
        let result = throws_analysis::find_inline_throws_annotations(body);
        assert_eq!(result, vec!["RuntimeException"]);
    }

    #[test]
    fn test_parse_catch_paren_content_empty() {
        let (partial, already) = parse_catch_paren_content("");
        assert_eq!(partial, "");
        assert!(already.is_empty());
    }

    #[test]
    fn test_parse_catch_paren_content_partial() {
        let (partial, already) = parse_catch_paren_content("IOEx");
        assert_eq!(partial, "IOEx");
        assert!(already.is_empty());
    }

    #[test]
    fn test_parse_catch_paren_content_multi_catch() {
        let (partial, already) = parse_catch_paren_content("IOException | ");
        assert_eq!(partial, "");
        assert_eq!(already, vec!["IOException"]);
    }

    #[test]
    fn test_parse_catch_paren_content_multi_catch_with_partial() {
        let (partial, already) = parse_catch_paren_content("IOException | Time");
        assert_eq!(partial, "Time");
        assert_eq!(already, vec!["IOException"]);
    }

    #[test]
    fn test_parse_catch_paren_content_three_types() {
        let (partial, already) = parse_catch_paren_content("IOException | TimeoutException | ");
        assert_eq!(partial, "");
        assert_eq!(already, vec!["IOException", "TimeoutException"]);
    }

    #[test]
    fn test_detect_catch_context_always_includes_throwable() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new RuntimeException('error');\n",
            "} catch (",
        );
        let pos = Position {
            line: 3,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos).unwrap();
        assert!(
            ctx.suggested_types.contains(&"\\Throwable".to_string()),
            "Should always include \\Throwable, got: {:?}",
            ctx.suggested_types
        );
        assert!(ctx.has_specific_types);
    }

    #[test]
    fn test_detect_catch_context_no_specific_types_sets_flag() {
        let content = concat!("<?php\n", "try {\n", "    doSomething();\n", "} catch (",);
        let pos = Position {
            line: 3,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos).unwrap();
        assert!(
            !ctx.has_specific_types,
            "Should have no specific types when try block has no throws"
        );
        // Throwable is still offered
        assert!(ctx.suggested_types.contains(&"\\Throwable".to_string()));
    }

    #[test]
    fn test_detect_catch_context_simple() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new RuntimeException('error');\n",
            "} catch (",
        );
        let pos = Position {
            line: 3,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string()),
            "Should suggest RuntimeException, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_detect_catch_context_with_inline_throws() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    /** @throws ModelNotFoundException */\n",
            "    $model = SomeService::find($id);\n",
            "} catch (",
        );
        let pos = Position {
            line: 4,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"ModelNotFoundException".to_string()),
            "Should suggest ModelNotFoundException from inline @throws, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_detect_catch_context_multi_throw() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new IOException('io');\n",
            "    throw new TimeoutException('timeout');\n",
            "} catch (",
        );
        let pos = Position {
            line: 4,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert!(ctx.suggested_types.contains(&"IOException".to_string()));
        assert!(
            ctx.suggested_types
                .contains(&"TimeoutException".to_string())
        );
    }

    #[test]
    fn test_detect_catch_context_second_catch() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new IOException('io');\n",
            "    throw new TimeoutException('timeout');\n",
            "} catch (IOException $e) {\n",
            "    // handled\n",
            "} catch (",
        );
        let pos = Position {
            line: 6,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some(), "Should detect second catch context");
        let ctx = ctx.unwrap();
        // Both types are in the try block
        assert!(ctx.suggested_types.contains(&"IOException".to_string()));
        assert!(
            ctx.suggested_types
                .contains(&"TimeoutException".to_string())
        );
    }

    #[test]
    fn test_detect_catch_context_partial_typed() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new RuntimeException('error');\n",
            "    throw new InvalidArgumentException('bad');\n",
            "} catch (Run",
        );
        let pos = Position {
            line: 4,
            character: 13,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.partial, "Run");
        // Both are suggested (filtering happens in build_catch_completions)
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string())
        );
        assert!(
            ctx.suggested_types
                .contains(&"InvalidArgumentException".to_string())
        );
    }

    #[test]
    fn test_detect_catch_context_not_catch() {
        let content = concat!("<?php\n", "function foo(",);
        let pos = Position {
            line: 1,
            character: 14,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_none(), "Should not detect catch context in function");
    }

    #[test]
    fn test_build_catch_completions_filters_by_partial() {
        let ctx = CatchContext {
            partial: "Run".to_string(),
            suggested_types: vec![
                "RuntimeException".to_string(),
                "InvalidArgumentException".to_string(),
            ],
            has_specific_types: true,
        };
        let items = build_catch_completions(&ctx);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "RuntimeException");
    }

    #[test]
    fn test_build_catch_completions_empty_partial_shows_all() {
        let ctx = CatchContext {
            partial: String::new(),
            suggested_types: vec![
                "RuntimeException".to_string(),
                "InvalidArgumentException".to_string(),
            ],
            has_specific_types: true,
        };
        let items = build_catch_completions(&ctx);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_detect_catch_context_multi_catch_pipe() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new IOException('io');\n",
            "    throw new TimeoutException('timeout');\n",
            "    throw new RuntimeException('rt');\n",
            "} catch (IOException | ",
        );
        let pos = Position {
            line: 5,
            character: 23,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        // IOException should be filtered out since it's already listed
        assert!(
            !ctx.suggested_types.contains(&"IOException".to_string()),
            "IOException should be filtered out"
        );
        assert!(
            ctx.suggested_types
                .contains(&"TimeoutException".to_string())
        );
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string())
        );
    }
}
