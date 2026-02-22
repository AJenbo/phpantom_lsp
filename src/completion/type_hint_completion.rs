//! Type hint completion inside function / method definitions.
//!
//! When the cursor is at a type-hint position — e.g. a function parameter
//! type, a return type after `):`, a property type after a visibility
//! modifier, or a union/intersection/nullable modifier — this module
//! detects the context and provides PHP native scalar types alongside
//! class-name completions (but **not** constants or standalone functions,
//! which are invalid in type positions).
//!
//! Detection is intentionally conservative: we only fire when we can
//! confirm the cursor is inside a function/method *definition* (not a
//! call) or a property/promoted-parameter declaration.

use crate::completion::named_args::{find_enclosing_open_paren, position_to_char_offset};
use tower_lsp::lsp_types::Position;

/// PHP native types valid in type-hint positions (PHP 8.x).
///
/// These are offered before class-name results so that typing `str`
/// suggests `string` alongside any user-defined classes starting with
/// `str`.  The list deliberately omits PHPStan-only pseudo-types like
/// `class-string`, `positive-int`, `non-empty-string`, etc. that are
/// not valid in native PHP declarations.
pub(crate) const PHP_NATIVE_TYPES: &[&str] = &[
    "string", "int", "float", "bool", "array", "object", "mixed", "void", "null", "callable",
    "iterable", "never", "self", "static", "parent", "true", "false",
];

/// Context returned when the cursor is at a type-hint position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeHintContext {
    /// The partial identifier the user has typed so far (may be empty).
    pub partial: String,
}

/// Detect whether the cursor is at a type-hint position inside a
/// function/method definition, return-type declaration, or property
/// declaration.
///
/// Returns `Some(TypeHintContext)` with the partial text when the cursor
/// is eligible for type-hint completion, or `None` otherwise.
pub(crate) fn detect_type_hint_context(
    content: &str,
    position: Position,
) -> Option<TypeHintContext> {
    let chars: Vec<char> = content.chars().collect();
    let cursor = position_to_char_offset(&chars, position)?;

    // ── Extract partial identifier ──────────────────────────────────
    let mut partial_start = cursor;
    while partial_start > 0
        && (chars[partial_start - 1].is_alphanumeric()
            || chars[partial_start - 1] == '_'
            || chars[partial_start - 1] == '\\')
    {
        partial_start -= 1;
    }

    // Preceded by `$` → variable, not a type.
    if partial_start > 0 && chars[partial_start - 1] == '$' {
        return None;
    }

    // Preceded by `->` or `::` → member access, not a type.
    if partial_start >= 2 && chars[partial_start - 2] == '-' && chars[partial_start - 1] == '>' {
        return None;
    }
    if partial_start >= 2 && chars[partial_start - 2] == ':' && chars[partial_start - 1] == ':' {
        return None;
    }

    let partial: String = chars[partial_start..cursor].iter().collect();

    // ── Skip whitespace before the partial ──────────────────────────
    let before = skip_whitespace_backward(&chars, partial_start);

    if before == 0 {
        return None;
    }

    let prev_char = chars[before - 1];

    // ── Check: return type position  `):`  ──────────────────────────
    if prev_char == ':' && is_return_type_colon(&chars, before - 1) {
        return Some(TypeHintContext { partial });
    }

    // ── Check: inside function-definition parameter list ────────────
    if prev_char == '(' || prev_char == ',' {
        let open_paren = if prev_char == '(' {
            before - 1
        } else {
            find_enclosing_open_paren(&chars, before - 1)?
        };

        if is_function_definition_paren(&chars, open_paren) {
            if prev_char == ',' {
                // Verify the preceding parameter is complete (has a `$`
                // variable name) so we don't trigger mid-type.
                let text_between: String = chars[open_paren + 1..before - 1].iter().collect();
                if let Some(last_param) = text_between.rsplit(',').next() {
                    if !last_param.contains('$') {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            return Some(TypeHintContext { partial });
        }
    }

    // ── Check: type modifier `?`, `|`, `&` in a type context ────────
    if (prev_char == '?' || prev_char == '|' || prev_char == '&')
        && is_type_modifier_in_definition(&chars, before - 1)
    {
        return Some(TypeHintContext { partial });
    }

    // ── Check: after property / promoted-param modifier keyword ─────
    // This covers both class property declarations (`public int $x`)
    // and promoted constructor parameters (`private readonly string $y`).
    if prev_char.is_alphabetic() && is_after_modifier_keyword(&chars, before) {
        // Make sure the partial is NOT a keyword that forms part of a
        // function declaration (e.g. `public function`).  If the user
        // has typed `function` or `fn` we should not offer type hints.
        let partial_lower = partial.to_lowercase();
        if partial_lower == "function" || partial_lower == "fn" {
            return None;
        }
        return Some(TypeHintContext { partial });
    }

    None
}

// ─── Private helpers ────────────────────────────────────────────────────────

/// Skip whitespace (spaces, tabs, newlines) backward from `pos`
/// (exclusive) and return the new position.
fn skip_whitespace_backward(chars: &[char], pos: usize) -> usize {
    let mut i = pos;
    while i > 0 && chars[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i
}

/// Check whether the `(` at `paren_pos` belongs to a function or method
/// *definition* (as opposed to a function/method *call*).
///
/// Walks backward from just before the `(`, skipping whitespace and
/// the optional function name, then looks for `function` or `fn`.
fn is_function_definition_paren(chars: &[char], paren_pos: usize) -> bool {
    let mut i = paren_pos;

    // Skip whitespace before `(`
    while i > 0 && chars[i - 1].is_ascii_whitespace() {
        i -= 1;
    }

    // Walk backward through the identifier (function name — may be empty
    // for anonymous functions / closures).
    let name_end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }

    let name: String = chars[i..name_end].iter().collect();

    // Anonymous `function(` or arrow `fn(` — the name *is* the keyword.
    if name == "function" || name == "fn" {
        return true;
    }

    // Named function / method: skip whitespace before the name and check
    // for the `function` or `fn` keyword.
    let mut j = i;
    while j > 0 && chars[j - 1].is_ascii_whitespace() {
        j -= 1;
    }

    if check_keyword_ending_at(chars, j, "function") {
        return true;
    }
    if check_keyword_ending_at(chars, j, "fn") {
        return true;
    }

    false
}

/// Check whether the `:` at `colon_pos` is a return-type colon — i.e.
/// it is preceded (possibly with whitespace) by `)`.
///
/// Also verifies the `)` belongs to a function definition.
fn is_return_type_colon(chars: &[char], colon_pos: usize) -> bool {
    let i = skip_whitespace_backward(chars, colon_pos);
    if i == 0 || chars[i - 1] != ')' {
        return false;
    }

    // Find the matching `(` for this `)`.
    let close_paren = i - 1;
    if let Some(open_paren) = find_matching_paren_backward(chars, close_paren) {
        return is_function_definition_paren(chars, open_paren);
    }
    false
}

/// Find the matching `(` for the `)` at `close_pos`, respecting nesting
/// and string literals.
fn find_matching_paren_backward(chars: &[char], close_pos: usize) -> Option<usize> {
    if chars[close_pos] != ')' {
        return None;
    }
    let mut depth: u32 = 1;
    let mut i = close_pos;
    while i > 0 {
        i -= 1;
        match chars[i] {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            '\'' => {
                i = skip_string_literal_backward(chars, i, '\'');
            }
            '"' => {
                i = skip_string_literal_backward(chars, i, '"');
            }
            '{' | '[' | ';' => return None,
            _ => {}
        }
    }
    None
}

/// Skip backward past a string literal whose closing quote is at `end`.
fn skip_string_literal_backward(chars: &[char], end: usize, q: char) -> usize {
    if end == 0 {
        return 0;
    }
    let mut j = end - 1;
    while j > 0 {
        if chars[j] == q {
            let mut backslashes = 0u32;
            let mut k = j;
            while k > 0 && chars[k - 1] == '\\' {
                backslashes += 1;
                k -= 1;
            }
            if backslashes.is_multiple_of(2) {
                return j;
            }
        }
        j -= 1;
    }
    0
}

/// Check whether a type modifier (`?`, `|`, or `&`) at `mod_pos` sits
/// inside a function-definition type context.
///
/// Walks backward past existing type names and modifiers until it finds
/// the structural character that anchors the context (one of `(`, `,`,
/// `:` for return type, or a modifier keyword for property declarations).
fn is_type_modifier_in_definition(chars: &[char], mod_pos: usize) -> bool {
    // Walk backward past the preceding type name (if any) and any
    // further type modifiers, collecting context anchors.
    let mut i = mod_pos;

    // Allow chained modifiers:  `A|B|`  or `?A|`
    loop {
        // Skip whitespace
        i = skip_whitespace_backward(chars, i);

        if i == 0 {
            return false;
        }

        // Walk backward through type identifier
        let type_end = i;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        // Skip whitespace
        i = skip_whitespace_backward(chars, i);

        if i == 0 {
            return false;
        }

        let prev = chars[i - 1];

        match prev {
            // Another type modifier — keep walking.
            '?' | '|' | '&' => {
                i -= 1;
                continue;
            }
            '(' => {
                // Check if this paren is a function definition.
                return is_function_definition_paren(chars, i - 1);
            }
            ',' => {
                // Find enclosing paren and check if it's a function definition.
                if let Some(open) = find_enclosing_open_paren(chars, i - 1)
                    && is_function_definition_paren(chars, open)
                {
                    // Verify the previous param segment has a `$`
                    let between: String = chars[open + 1..i - 1].iter().collect();
                    if let Some(last_seg) = between.rsplit(',').next() {
                        return last_seg.contains('$');
                    }
                }
                return false;
            }
            ':' => {
                // Return type colon.
                return is_return_type_colon(chars, i - 1);
            }
            _ => {
                // Could be after a modifier keyword (property or promoted param).
                if prev.is_alphabetic() {
                    if is_after_modifier_keyword(chars, i) {
                        return true;
                    }
                    // Could also be: we already consumed the type name above
                    // and now the preceding text ends with a modifier.
                    // But if type_end == i we didn't consume anything, so this
                    // isn't a type context.
                    if type_end == i {
                        return false;
                    }
                    // We consumed a type name; check if what's before it is a
                    // modifier keyword (e.g. `public ?string|`).
                    return is_after_modifier_keyword(chars, i);
                }
                return false;
            }
        }
    }
}

/// Check whether the text ending at `pos` (exclusive) is one of the PHP
/// modifier keywords that can precede a type hint: `public`, `protected`,
/// `private`, `readonly`, or `static`.
///
/// Also handles chains like `private readonly` by recursively checking
/// the preceding token.
fn is_after_modifier_keyword(chars: &[char], pos: usize) -> bool {
    if pos == 0 {
        return false;
    }

    // Walk backward through alphabetic word.
    let word_end = pos;
    let mut start = pos;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }

    if start == word_end {
        return false;
    }

    let word: String = chars[start..word_end].iter().collect();

    match word.as_str() {
        "public" | "protected" | "private" | "readonly" | "static" => {
            // Ensure word boundary before the keyword.
            if start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                return false;
            }

            // Also handle `function` keyword before the modifier — this
            // means we're looking at a method declaration, not a property.
            // But that's fine: `function` can only appear before the name
            // and parens, so if we're here the cursor must be at a property
            // or promoted-param position.
            //
            // However, we do NOT want to trigger inside function
            // parameter lists that happen to have a modifier keyword
            // on a preceding line — the paren-based detection handles
            // those.  We check that we're NOT inside an unmatched `(`.
            // This avoids double-triggering for promoted constructor
            // params (the paren-based check already handles those).
            //
            // Actually, we DO want to trigger for promoted constructor
            // params like `__construct(private readonly |)` because the
            // modifier is the immediate predecessor.  The paren-based
            // detection would NOT fire there because `readonly` is not
            // `(` or `,`.  So this branch is the correct handler for
            // promoted params with modifiers.
            true
        }
        _ => false,
    }
}

/// Check whether a specific keyword ends at `pos` (exclusive) in `chars`,
/// with a word-boundary before it.
fn check_keyword_ending_at(chars: &[char], pos: usize, keyword: &str) -> bool {
    let kw_len = keyword.len();
    if pos < kw_len {
        return false;
    }
    let start = pos - kw_len;
    let candidate: String = chars[start..pos].iter().collect();
    if candidate != keyword {
        return false;
    }
    // Ensure word boundary before the keyword.
    if start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        return false;
    }
    true
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: detect at a given line/character.
    fn detect(content: &str, line: u32, character: u32) -> Option<TypeHintContext> {
        detect_type_hint_context(content, Position { line, character })
    }

    // ── Function parameter type hints ───────────────────────────────

    #[test]
    fn after_open_paren_in_function() {
        let src = "<?php\nfunction foo(Us) {}";
        let ctx = detect(src, 1, 15).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn empty_after_open_paren() {
        let src = "<?php\nfunction foo() {}";
        // cursor right after `(`
        let ctx = detect(src, 1, 13);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().partial, "");
    }

    #[test]
    fn after_comma_in_function_params() {
        let src = "<?php\nfunction foo(string $a, Us) {}";
        let ctx = detect(src, 1, 26).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn after_comma_empty_partial() {
        let src = "<?php\nfunction foo(string $a, ) {}";
        let ctx = detect(src, 1, 24);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().partial, "");
    }

    #[test]
    fn not_after_comma_incomplete_param() {
        // The first param has no $variable yet — the user is still typing
        // the type, so the comma doesn't indicate a new param type position.
        let src = "<?php\nfunction foo(string,) {}";
        let ctx = detect(src, 1, 20);
        assert!(ctx.is_none());
    }

    // ── Return type hints ───────────────────────────────────────────

    #[test]
    fn return_type_after_colon() {
        let src = "<?php\nfunction foo(): Us {}";
        let ctx = detect(src, 1, 18).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn return_type_empty() {
        let src = "<?php\nfunction foo():  {}";
        // cursor right after `: `
        let ctx = detect(src, 1, 16);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().partial, "");
    }

    // ── Nullable / union / intersection modifiers ───────────────────

    #[test]
    fn nullable_param_type() {
        let src = "<?php\nfunction foo(?Us) {}";
        let ctx = detect(src, 1, 16).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn union_param_type() {
        let src = "<?php\nfunction foo(string|Us) {}";
        let ctx = detect(src, 1, 22).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn intersection_param_type() {
        let src = "<?php\nfunction foo(A&Us) {}";
        let ctx = detect(src, 1, 17).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn union_return_type() {
        let src = "<?php\nfunction foo(): string|Us {}";
        let ctx = detect(src, 1, 25).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn nullable_return_type() {
        let src = "<?php\nfunction foo(): ?Us {}";
        let ctx = detect(src, 1, 19).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    // ── Method definitions ──────────────────────────────────────────

    #[test]
    fn method_param_type() {
        let src = "<?php\nclass Foo {\n    public function bar(Us) {}\n}";
        let ctx = detect(src, 2, 26).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn method_return_type() {
        let src = "<?php\nclass Foo {\n    public function bar(): Us {}\n}";
        let ctx = detect(src, 2, 29).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    // ── Property type hints ─────────────────────────────────────────

    #[test]
    fn property_after_public() {
        let src = "<?php\nclass Foo {\n    public Us\n}";
        let ctx = detect(src, 2, 13).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn property_after_private_readonly() {
        let src = "<?php\nclass Foo {\n    private readonly Us\n}";
        let ctx = detect(src, 2, 23).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn property_after_protected_static() {
        let src = "<?php\nclass Foo {\n    protected static Us\n}";
        let ctx = detect(src, 2, 23).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    // ── Promoted constructor parameters ─────────────────────────────

    #[test]
    fn promoted_param_after_modifier() {
        let src = "<?php\nclass Foo {\n    public function __construct(private Us) {}\n}";
        let ctx = detect(src, 2, 42).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn promoted_param_after_readonly() {
        let src = "<?php\nclass Foo {\n    public function __construct(private readonly Us) {}\n}";
        let ctx = detect(src, 2, 51).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    // ── Closures and arrow functions ────────────────────────────────

    #[test]
    fn closure_param_type() {
        let src = "<?php\n$f = function(Us) {};";
        let ctx = detect(src, 1, 16).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn arrow_fn_param_type() {
        let src = "<?php\n$f = fn(Us) => null;";
        let ctx = detect(src, 1, 10).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn closure_return_type() {
        let src = "<?php\n$f = function(): Us {};";
        let ctx = detect(src, 1, 19).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn arrow_fn_return_type() {
        let src = "<?php\n$f = fn(): Us => null;";
        let ctx = detect(src, 1, 13).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    // ── Multi-line function definitions ─────────────────────────────

    #[test]
    fn multiline_param_type() {
        let src = "<?php\nfunction foo(\n    string $a,\n    Us\n) {}";
        let ctx = detect(src, 3, 6).unwrap();
        assert_eq!(ctx.partial, "Us");
    }

    #[test]
    fn multiline_after_comma_empty() {
        let src = "<?php\nfunction foo(\n    string $a,\n    \n) {}";
        let ctx = detect(src, 3, 4);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().partial, "");
    }

    // ── Negative cases: should NOT detect ───────────────────────────

    #[test]
    fn not_in_function_call() {
        let src = "<?php\nfoo(Us);";
        let ctx = detect(src, 1, 6);
        assert!(ctx.is_none());
    }

    #[test]
    fn not_in_method_call() {
        let src = "<?php\n$obj->foo(Us);";
        let ctx = detect(src, 1, 13);
        assert!(ctx.is_none());
    }

    #[test]
    fn not_variable() {
        let src = "<?php\nfunction foo($us) {}";
        let ctx = detect(src, 1, 15);
        assert!(ctx.is_none());
    }

    #[test]
    fn not_member_access() {
        let src = "<?php\n$this->Us";
        let ctx = detect(src, 1, 10);
        assert!(ctx.is_none());
    }

    #[test]
    fn not_static_access() {
        let src = "<?php\nFoo::Us";
        let ctx = detect(src, 1, 8);
        assert!(ctx.is_none());
    }

    #[test]
    fn not_assignment() {
        let src = "<?php\n$x = Us;";
        let ctx = detect(src, 1, 7);
        assert!(ctx.is_none());
    }

    #[test]
    fn not_after_function_keyword() {
        // Typing the function name after `function` should not suggest types.
        let src = "<?php\npublic function Us";
        let ctx = detect(src, 1, 20);
        // `function` is not a modifier keyword, so this should not match.
        assert!(ctx.is_none());
    }

    #[test]
    fn partial_is_function_keyword_after_modifier() {
        // `public function` — the partial "function" should be filtered out
        // so we don't offer type hints when the user is typing the keyword.
        let src = "<?php\nclass Foo {\n    public function\n}";
        let ctx = detect(src, 2, 19);
        assert!(ctx.is_none());
    }

    // ── Native types constant ───────────────────────────────────────

    #[test]
    fn native_types_includes_common_types() {
        assert!(PHP_NATIVE_TYPES.contains(&"string"));
        assert!(PHP_NATIVE_TYPES.contains(&"int"));
        assert!(PHP_NATIVE_TYPES.contains(&"float"));
        assert!(PHP_NATIVE_TYPES.contains(&"bool"));
        assert!(PHP_NATIVE_TYPES.contains(&"array"));
        assert!(PHP_NATIVE_TYPES.contains(&"mixed"));
        assert!(PHP_NATIVE_TYPES.contains(&"void"));
        assert!(PHP_NATIVE_TYPES.contains(&"never"));
        assert!(PHP_NATIVE_TYPES.contains(&"callable"));
        assert!(PHP_NATIVE_TYPES.contains(&"self"));
        assert!(PHP_NATIVE_TYPES.contains(&"static"));
        assert!(PHP_NATIVE_TYPES.contains(&"null"));
        assert!(PHP_NATIVE_TYPES.contains(&"true"));
        assert!(PHP_NATIVE_TYPES.contains(&"false"));
    }

    #[test]
    fn native_types_excludes_phpstan_only() {
        assert!(!PHP_NATIVE_TYPES.contains(&"class-string"));
        assert!(!PHP_NATIVE_TYPES.contains(&"positive-int"));
        assert!(!PHP_NATIVE_TYPES.contains(&"non-empty-string"));
        assert!(!PHP_NATIVE_TYPES.contains(&"resource"));
    }
}
