//! Named argument completion for PHP 8.0+ syntax.
//!
//! When the cursor is inside the parentheses of a function or method call,
//! this module detects the call context and offers parameter names as
//! completion items with a trailing `:` so the user can quickly write
//! named arguments like `foo(paramName: $value)`.
//!
//! ## Supported call forms
//!
//! - Standalone functions: `foo(|)`
//! - Instance methods: `$this->method(|)`, `$var->method(|)`
//! - Static methods: `ClassName::method(|)`, `self::method(|)`
//! - Constructors: `new ClassName(|)`
//!
//! ## Smart features
//!
//! - Already-used named arguments are excluded from suggestions
//! - Positional arguments are counted to skip leading parameters
//! - The user's partial prefix is used for filtering

use tower_lsp::lsp_types::*;

// ─── Context ────────────────────────────────────────────────────────────────

/// Information about a named-argument completion context.
#[derive(Debug, Clone)]
pub struct NamedArgContext {
    /// The call expression in a format suitable for resolution:
    /// - `"functionName"` for standalone functions
    /// - `"$this->method"` or `"$var->method"` for instance methods
    /// - `"ClassName::method"` or `"self::method"` for static methods
    /// - `"new ClassName"` for constructor calls
    pub call_expression: String,
    /// Parameter names already specified as named arguments in this call.
    pub existing_named_args: Vec<String>,
    /// Number of positional (non-named) arguments before the cursor.
    pub positional_count: usize,
    /// The partial identifier prefix the user has typed (e.g. `"na"` from `foo(na|)`).
    pub prefix: String,
}

// ─── Detection ──────────────────────────────────────────────────────────────

/// Detect whether the cursor is inside a function/method call and extract
/// the context needed for named-argument completion.
///
/// Returns `None` if the cursor is not at an eligible position (e.g. after
/// `$`, `->`, `::`, or inside a string/comment).
pub fn detect_named_arg_context(content: &str, position: Position) -> Option<NamedArgContext> {
    let chars: Vec<char> = content.chars().collect();
    let cursor = position_to_char_offset(&chars, position)?;

    // ── Check eligibility at cursor ─────────────────────────────────
    // Walk backward from cursor through identifier chars to find the
    // start of the current "word".
    let mut word_start = cursor;
    while word_start > 0
        && (chars[word_start - 1].is_alphanumeric() || chars[word_start - 1] == '_')
    {
        word_start -= 1;
    }

    // If preceded by `$`, this is a variable — not a named arg.
    if word_start > 0 && chars[word_start - 1] == '$' {
        return None;
    }

    // If preceded by `->` or `::`, member completion handles this.
    if word_start >= 2 && chars[word_start - 2] == '-' && chars[word_start - 1] == '>' {
        return None;
    }
    if word_start >= 2 && chars[word_start - 2] == ':' && chars[word_start - 1] == ':' {
        return None;
    }

    let prefix: String = chars[word_start..cursor].iter().collect();

    // ── Find enclosing open paren ───────────────────────────────────
    let open_paren = find_enclosing_open_paren(&chars, word_start)?;

    // ── Extract call expression before `(` ──────────────────────────
    let call_expr = extract_call_expression(&chars, open_paren)?;
    if call_expr.is_empty() {
        return None;
    }

    // ── Parse arguments between `(` and cursor ──────────────────────
    let args_text: String = chars[open_paren + 1..word_start].iter().collect();
    let (existing_named, positional_count) = parse_existing_args(&args_text);

    Some(NamedArgContext {
        call_expression: call_expr,
        existing_named_args: existing_named,
        positional_count,
        prefix,
    })
}

/// Convert an LSP `Position` (line/character) to a character offset into
/// the char array.
fn position_to_char_offset(chars: &[char], position: Position) -> Option<usize> {
    let target_line = position.line as usize;
    let target_col = position.character as usize;
    let mut line = 0usize;
    let mut col = 0usize;

    for (i, &ch) in chars.iter().enumerate() {
        if line == target_line && col == target_col {
            return Some(i);
        }
        if ch == '\n' {
            // If we're at the target line and the target column is at or
            // past the end of the line, clamp to end-of-line.
            if line == target_line {
                return Some(i);
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    // Cursor at very end of content
    if line == target_line && col == target_col {
        return Some(chars.len());
    }
    // Target column past end of last line (no trailing newline)
    if line == target_line {
        return Some(chars.len());
    }

    None
}

/// Walk backward from `start` (exclusive) to find the unmatched `(` that
/// encloses the cursor.
///
/// Skips balanced `(…)` pairs and string literals.  Returns `None` if no
/// enclosing `(` is found (cursor is not inside call parens).
fn find_enclosing_open_paren(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    let mut depth: i32 = 0;

    while i > 0 {
        i -= 1;
        match chars[i] {
            ')' => depth += 1,
            '(' => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    // Found unmatched `(` — this is the call's open paren.
                    return Some(i);
                }
            }
            // Skip single-quoted strings backwards
            '\'' => {
                i = skip_string_backward(chars, i, '\'');
            }
            // Skip double-quoted strings backwards
            '"' => {
                i = skip_string_backward(chars, i, '"');
            }
            // If we hit `{` or `[` without a matching `}` or `]`, we've
            // left the expression context — stop searching.
            '{' | '[' => return None,
            // If we hit `;` we've gone past a statement boundary.
            ';' => return None,
            _ => {}
        }
    }

    None
}

/// Skip backward past a string literal ending at position `end` (which
/// points to the closing quote character `q`).
///
/// Returns the position of the opening quote, or 0 if not found.
fn skip_string_backward(chars: &[char], end: usize, q: char) -> usize {
    if end == 0 {
        return 0;
    }
    let mut j = end - 1;
    while j > 0 {
        if chars[j] == q {
            // Check it's not escaped
            let mut backslashes = 0u32;
            let mut k = j;
            while k > 0 && chars[k - 1] == '\\' {
                backslashes += 1;
                k -= 1;
            }
            if backslashes.is_multiple_of(2) {
                // Not escaped — this is the opening quote
                return j;
            }
        }
        j -= 1;
    }
    0
}

/// Extract the call expression that precedes the opening paren at `open`.
///
/// Handles:
/// - `foo(` → `"foo"`
/// - `$this->method(` → `"$this->method"`
/// - `$var->method(` → `"$var->method"`
/// - `ClassName::method(` → `"ClassName::method"`
/// - `self::method(` / `static::method(` / `parent::method(` → as-is
/// - `new ClassName(` → `"new ClassName"`
/// - `(new Foo())->method(` → `"$this->method"` etc. — simplified
fn extract_call_expression(chars: &[char], open: usize) -> Option<String> {
    if open == 0 {
        return None;
    }

    let mut i = open;

    // Skip whitespace before `(`
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }

    if i == 0 {
        return None;
    }

    // ── If preceded by `)`, this is a chained call like `foo()->bar(`.
    // We won't try to resolve through call chains for named args — the
    // complexity is high and the user can rely on member completion.
    // But we DO need to handle `(new Foo)(` — skip for now.
    if chars[i - 1] == ')' {
        return None;
    }

    // ── Read the identifier (function/method name) ──────────────────
    let ident_end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\') {
        i -= 1;
    }
    if i == ident_end {
        return None;
    }
    let ident: String = chars[i..ident_end].iter().collect();

    // ── Check what precedes the identifier ──────────────────────────

    // Instance method: `->method(`
    if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
        let subject = extract_subject_before_arrow(chars, i - 2);
        if !subject.is_empty() {
            return Some(format!("{}->{}", subject, ident));
        }
        return None;
    }

    // Null-safe method: `?->method(`
    if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
        let subject = extract_subject_before_arrow(chars, i - 3);
        if !subject.is_empty() {
            return Some(format!("{}->{}", subject, ident));
        }
        return None;
    }

    // Static method: `::method(`
    if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
        let class_name = extract_class_name_backward(chars, i - 2);
        if !class_name.is_empty() {
            return Some(format!("{}::{}", class_name, ident));
        }
        return None;
    }

    // Constructor: `new ClassName(`
    // Skip whitespace and check for `new` keyword.
    let mut j = i;
    while j > 0 && chars[j - 1] == ' ' {
        j -= 1;
    }
    if j >= 3 && chars[j - 3] == 'n' && chars[j - 2] == 'e' && chars[j - 1] == 'w' {
        // Verify word boundary before `new`
        let before_ok = j == 3 || { !chars[j - 4].is_alphanumeric() && chars[j - 4] != '_' };
        if before_ok {
            return Some(format!("new {}", ident));
        }
    }

    // Standalone function call: `foo(`
    Some(ident)
}

/// Extract the subject before `->` for method calls.
///
/// `arrow_pos` points to the `-` of `->`.
/// Handles `$this`, `$var`, and simple variable names.
fn extract_subject_before_arrow(chars: &[char], arrow_pos: usize) -> String {
    let mut i = arrow_pos;
    // Skip whitespace
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }

    // Check for `)` — chained call, skip for now
    if i > 0 && chars[i - 1] == ')' {
        return String::new();
    }

    // Read identifier (property or variable name without `$`)
    let end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }

    // Check for `$` prefix (variable)
    if i > 0 && chars[i - 1] == '$' {
        i -= 1;
        return chars[i..end].iter().collect();
    }

    // Could be a chained property: `$this->prop->method(` — just return
    // the identifier; resolution in server.rs will handle it.
    chars[i..end].iter().collect()
}

/// Extract a class name (possibly namespace-qualified) before `::`.
///
/// `colon_pos` points to the first `:` of `::`.
fn extract_class_name_backward(chars: &[char], colon_pos: usize) -> String {
    let mut i = colon_pos;
    // Skip whitespace
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }
    let end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\') {
        i -= 1;
    }
    chars[i..end].iter().collect()
}

/// Parse the arguments text between `(` and the cursor to determine:
/// - Which parameter names have already been used as named arguments
/// - How many positional (non-named) arguments precede the cursor
///
/// Returns `(existing_named_args, positional_count)`.
fn parse_existing_args(args_text: &str) -> (Vec<String>, usize) {
    let mut named = Vec::new();
    let mut positional = 0usize;

    // Split by commas at the top level (respecting nested parens/strings)
    let args = split_args_top_level(args_text);

    for arg in &args {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if this argument is a named argument: `name: value`
        // Named args look like `identifier:` (but NOT `::`)
        if let Some(name) = extract_named_arg_name(trimmed) {
            named.push(name);
        } else {
            positional += 1;
        }
    }

    (named, positional)
}

/// Split argument text by commas at the top level (depth 0), respecting
/// nested parentheses and string literals.
fn split_args_top_level(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '(' | '[' => {
                depth += 1;
                current.push(chars[i]);
            }
            ')' | ']' => {
                depth -= 1;
                current.push(chars[i]);
            }
            ',' if depth == 0 => {
                args.push(std::mem::take(&mut current));
            }
            '\'' | '"' => {
                let q = chars[i];
                current.push(q);
                i += 1;
                while i < chars.len() {
                    current.push(chars[i]);
                    if chars[i] == q {
                        // Check for escaping
                        let mut backslashes = 0u32;
                        let mut k = current.len() - 1;
                        while k > 0 && current.as_bytes()[k - 1] == b'\\' {
                            backslashes += 1;
                            k -= 1;
                        }
                        if backslashes.is_multiple_of(2) {
                            break;
                        }
                    }
                    i += 1;
                }
            }
            _ => current.push(chars[i]),
        }
        i += 1;
    }

    // Don't push the last segment — it's the argument currently being typed
    // and is handled separately as the prefix.
    // Actually, we DO want to push it if it has content, because parse_existing_args
    // needs to count it. But the caller already stripped the prefix from args_text,
    // so the last segment here (if any) is a complete previous argument.
    if !current.trim().is_empty() {
        args.push(current);
    }

    args
}

/// If `arg` looks like a named argument (`name: ...`), return the name.
/// Returns `None` for positional arguments.
fn extract_named_arg_name(arg: &str) -> Option<String> {
    // Look for `identifier:` at the start (but not `::`)
    let chars: Vec<char> = arg.chars().collect();
    let mut i = 0;

    // Skip whitespace
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }

    // Read identifier
    let start = i;
    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }

    if i == start {
        return None;
    }

    // Must be followed by `:` (but not `::`)
    if i < chars.len() && chars[i] == ':' {
        // Check it's not `::`
        if i + 1 < chars.len() && chars[i + 1] == ':' {
            return None;
        }
        let name: String = chars[start..i].iter().collect();
        return Some(name);
    }

    None
}

// ─── Completion Builder ─────────────────────────────────────────────────────

/// Build named-argument completion items from a list of parameters.
///
/// Parameters that have already been used as named arguments or that are
/// covered by positional arguments are excluded.
pub fn build_named_arg_completions(
    ctx: &NamedArgContext,
    parameters: &[crate::types::ParameterInfo],
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let prefix_lower = ctx.prefix.to_lowercase();

    for (idx, param) in parameters.iter().enumerate() {
        // The parameter name in PHP includes `$`, but named args use the
        // bare name: `$name` → `name:`
        let bare_name = param.name.strip_prefix('$').unwrap_or(&param.name);

        // Skip parameters covered by positional arguments
        if idx < ctx.positional_count {
            continue;
        }

        // Skip parameters already specified as named arguments
        if ctx.existing_named_args.iter().any(|n| n == bare_name) {
            continue;
        }

        // Apply prefix filter
        if !bare_name.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }

        // Build the label showing type info
        let label = if let Some(ref th) = param.type_hint {
            format!("{}: {}", bare_name, th)
        } else {
            format!("{}:", bare_name)
        };

        // Insert text: `name: ` (bare name + colon + space)
        let insert = format!("{}: ", bare_name);

        let detail = if param.is_variadic {
            Some("Named argument (variadic)".to_string())
        } else if !param.is_required {
            Some("Named argument (optional)".to_string())
        } else {
            Some("Named argument".to_string())
        };

        items.push(CompletionItem {
            label,
            kind: Some(CompletionItemKind::VARIABLE),
            detail,
            insert_text: Some(insert),
            filter_text: Some(bare_name.to_string()),
            sort_text: Some(format!("0_{:03}", idx)),
            ..CompletionItem::default()
        });
    }

    items
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ParameterInfo;

    // ── position_to_char_offset ─────────────────────────────────────

    #[test]
    fn char_offset_first_line() {
        let content = "<?php\nfoo()\n";
        let chars: Vec<char> = content.chars().collect();
        let pos = Position {
            line: 1,
            character: 3,
        };
        // "foo" starts at offset 6 (after "<?php\n"), character 3 = '('
        assert_eq!(position_to_char_offset(&chars, pos), Some(9));
    }

    #[test]
    fn char_offset_end_of_line() {
        let content = "<?php\nfoo()\n";
        let chars: Vec<char> = content.chars().collect();
        let pos = Position {
            line: 1,
            character: 5,
        };
        assert_eq!(position_to_char_offset(&chars, pos), Some(11));
    }

    // ── find_enclosing_open_paren ───────────────────────────────────

    #[test]
    fn finds_open_paren_simple() {
        let chars: Vec<char> = "foo(".chars().collect();
        assert_eq!(find_enclosing_open_paren(&chars, 4), Some(3));
    }

    #[test]
    fn finds_open_paren_with_args() {
        let chars: Vec<char> = "foo($x, ".chars().collect();
        assert_eq!(find_enclosing_open_paren(&chars, 8), Some(3));
    }

    #[test]
    fn skips_nested_parens() {
        let chars: Vec<char> = "foo(bar(1), ".chars().collect();
        assert_eq!(find_enclosing_open_paren(&chars, 12), Some(3));
    }

    #[test]
    fn none_outside_parens() {
        let chars: Vec<char> = "foo();".chars().collect();
        // After the `)` and `;`
        assert_eq!(find_enclosing_open_paren(&chars, 6), None);
    }

    #[test]
    fn stops_at_semicolon() {
        let chars: Vec<char> = "$x = 1; foo(".chars().collect();
        // Searching from after `foo(`, should find `(` at position 11
        assert_eq!(find_enclosing_open_paren(&chars, 12), Some(11));
    }

    #[test]
    fn skips_single_quoted_string() {
        let chars: Vec<char> = "foo('(', ".chars().collect();
        assert_eq!(find_enclosing_open_paren(&chars, 9), Some(3));
    }

    #[test]
    fn skips_double_quoted_string() {
        let chars: Vec<char> = "foo(\"(\", ".chars().collect();
        assert_eq!(find_enclosing_open_paren(&chars, 9), Some(3));
    }

    // ── extract_call_expression ─────────────────────────────────────

    #[test]
    fn call_expr_standalone_function() {
        let chars: Vec<char> = "foo(".chars().collect();
        assert_eq!(extract_call_expression(&chars, 3), Some("foo".to_string()));
    }

    #[test]
    fn call_expr_namespaced_function() {
        let chars: Vec<char> = "App\\Helper\\foo(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 14),
            Some("App\\Helper\\foo".to_string())
        );
    }

    #[test]
    fn call_expr_instance_method() {
        let chars: Vec<char> = "$this->method(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 13),
            Some("$this->method".to_string())
        );
    }

    #[test]
    fn call_expr_variable_method() {
        let chars: Vec<char> = "$service->handle(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 16),
            Some("$service->handle".to_string())
        );
    }

    #[test]
    fn call_expr_static_method() {
        let chars: Vec<char> = "Cache::get(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 10),
            Some("Cache::get".to_string())
        );
    }

    #[test]
    fn call_expr_self_method() {
        let chars: Vec<char> = "self::create(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 12),
            Some("self::create".to_string())
        );
    }

    #[test]
    fn call_expr_parent_method() {
        let chars: Vec<char> = "parent::__construct(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 19),
            Some("parent::__construct".to_string())
        );
    }

    #[test]
    fn call_expr_constructor() {
        let chars: Vec<char> = "new UserService(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 15),
            Some("new UserService".to_string())
        );
    }

    #[test]
    fn call_expr_constructor_with_extra_space() {
        let chars: Vec<char> = "new  Foo(".chars().collect();
        assert_eq!(
            extract_call_expression(&chars, 8),
            Some("new Foo".to_string())
        );
    }

    #[test]
    fn call_expr_none_for_chained_call() {
        let chars: Vec<char> = "foo()->bar(".chars().collect();
        // The `(` at index 10 follows `bar`, but before `bar` is `)->` preceded by `)`
        // We don't support chained-call resolution for named args
        assert_eq!(extract_call_expression(&chars, 10), None);
    }

    // ── extract_named_arg_name ──────────────────────────────────────

    #[test]
    fn named_arg_simple() {
        assert_eq!(
            extract_named_arg_name("name: $value"),
            Some("name".to_string())
        );
    }

    #[test]
    fn named_arg_with_whitespace() {
        assert_eq!(extract_named_arg_name("  age: 42"), Some("age".to_string()));
    }

    #[test]
    fn positional_arg_variable() {
        assert_eq!(extract_named_arg_name("$value"), None);
    }

    #[test]
    fn positional_arg_number() {
        assert_eq!(extract_named_arg_name("42"), None);
    }

    #[test]
    fn not_named_arg_double_colon() {
        assert_eq!(extract_named_arg_name("Foo::class"), None);
    }

    #[test]
    fn not_named_arg_string() {
        assert_eq!(extract_named_arg_name("'hello'"), None);
    }

    // ── parse_existing_args ─────────────────────────────────────────

    #[test]
    fn no_args() {
        let (named, pos) = parse_existing_args("");
        assert!(named.is_empty());
        assert_eq!(pos, 0);
    }

    #[test]
    fn one_positional() {
        let (named, pos) = parse_existing_args("$x, ");
        assert!(named.is_empty());
        assert_eq!(pos, 1);
    }

    #[test]
    fn two_positional() {
        let (named, pos) = parse_existing_args("$x, $y, ");
        assert!(named.is_empty());
        assert_eq!(pos, 2);
    }

    #[test]
    fn one_named() {
        let (named, pos) = parse_existing_args("name: $x, ");
        assert_eq!(named, vec!["name"]);
        assert_eq!(pos, 0);
    }

    #[test]
    fn mixed_positional_and_named() {
        let (named, pos) = parse_existing_args("$x, name: $y, ");
        assert_eq!(named, vec!["name"]);
        assert_eq!(pos, 1);
    }

    #[test]
    fn multiple_named() {
        let (named, pos) = parse_existing_args("name: 'John', age: 30, ");
        assert_eq!(named, vec!["name", "age"]);
        assert_eq!(pos, 0);
    }

    #[test]
    fn nested_call_in_arg() {
        let (named, pos) = parse_existing_args("getName($obj), ");
        assert!(named.is_empty());
        assert_eq!(pos, 1);
    }

    // ── detect_named_arg_context ────────────────────────────────────

    #[test]
    fn context_simple_function() {
        let content = "<?php\nfoo(";
        let pos = Position {
            line: 1,
            character: 4,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some(), "Should detect context inside foo(");
        let ctx = ctx.unwrap();
        assert_eq!(ctx.call_expression, "foo");
        assert!(ctx.existing_named_args.is_empty());
        assert_eq!(ctx.positional_count, 0);
        assert_eq!(ctx.prefix, "");
    }

    #[test]
    fn context_with_prefix() {
        let content = "<?php\nfoo(na";
        let pos = Position {
            line: 1,
            character: 6,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.call_expression, "foo");
        assert_eq!(ctx.prefix, "na");
    }

    #[test]
    fn context_after_positional() {
        let content = "<?php\nfoo($x, ";
        let pos = Position {
            line: 1,
            character: 8,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.call_expression, "foo");
        assert_eq!(ctx.positional_count, 1);
    }

    #[test]
    fn context_after_named_arg() {
        let content = "<?php\nfoo(name: $x, ";
        let pos = Position {
            line: 1,
            character: 15,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.call_expression, "foo");
        assert_eq!(ctx.existing_named_args, vec!["name"]);
        assert_eq!(ctx.positional_count, 0);
    }

    #[test]
    fn context_method_call() {
        let content = "<?php\n$this->method(";
        let pos = Position {
            line: 1,
            character: 16,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().call_expression, "$this->method");
    }

    #[test]
    fn context_static_call() {
        let content = "<?php\nCache::get(";
        let pos = Position {
            line: 1,
            character: 11,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().call_expression, "Cache::get");
    }

    #[test]
    fn context_constructor() {
        let content = "<?php\nnew Foo(";
        let pos = Position {
            line: 1,
            character: 8,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().call_expression, "new Foo");
    }

    #[test]
    fn no_context_typing_variable() {
        let content = "<?php\nfoo($va";
        let pos = Position {
            line: 1,
            character: 7,
        };
        // Preceded by `$` — should return None
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_none(), "Should not trigger for variable names");
    }

    #[test]
    fn no_context_after_arrow() {
        let content = "<?php\nfoo($this->pr";
        let pos = Position {
            line: 1,
            character: 14,
        };
        // Preceded by `->` — should return None
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_none(), "Should not trigger after ->");
    }

    #[test]
    fn no_context_outside_parens() {
        let content = "<?php\nfoo();";
        let pos = Position {
            line: 1,
            character: 6,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_none(), "Should not trigger outside parens");
    }

    #[test]
    fn context_multiline() {
        let content = "<?php\nfoo(\n    $x,\n    ";
        let pos = Position {
            line: 3,
            character: 4,
        };
        let ctx = detect_named_arg_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.call_expression, "foo");
        assert_eq!(ctx.positional_count, 1);
    }

    // ── build_named_arg_completions ─────────────────────────────────

    fn make_param(name: &str, type_hint: Option<&str>, required: bool) -> ParameterInfo {
        ParameterInfo {
            name: format!("${}", name),
            is_required: required,
            type_hint: type_hint.map(|s| s.to_string()),
            is_variadic: false,
            is_reference: false,
        }
    }

    #[test]
    fn completions_all_params() {
        let params = vec![
            make_param("name", Some("string"), true),
            make_param("age", Some("int"), true),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "name: string");
        assert_eq!(items[0].insert_text.as_deref(), Some("name: "));
        assert_eq!(items[0].filter_text.as_deref(), Some("name"));
        assert_eq!(items[1].label, "age: int");
        assert_eq!(items[1].insert_text.as_deref(), Some("age: "));
    }

    #[test]
    fn completions_skip_positional() {
        let params = vec![
            make_param("name", Some("string"), true),
            make_param("age", Some("int"), true),
            make_param("email", Some("string"), false),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 1, // first param covered by positional
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].filter_text.as_deref(), Some("age"));
        assert_eq!(items[1].filter_text.as_deref(), Some("email"));
    }

    #[test]
    fn completions_skip_named() {
        let params = vec![
            make_param("name", Some("string"), true),
            make_param("age", Some("int"), true),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec!["name".to_string()],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].filter_text.as_deref(), Some("age"));
    }

    #[test]
    fn completions_filter_by_prefix() {
        let params = vec![
            make_param("name", Some("string"), true),
            make_param("notify", Some("bool"), false),
            make_param("age", Some("int"), true),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: "na".to_string(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].filter_text.as_deref(), Some("name"));
    }

    #[test]
    fn completions_untyped_param() {
        let params = vec![make_param("data", None, true)];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "data:");
    }

    #[test]
    fn completions_optional_detail() {
        let params = vec![
            make_param("name", Some("string"), true),
            make_param("age", Some("int"), false),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items[0].detail.as_deref(), Some("Named argument"));
        assert_eq!(
            items[1].detail.as_deref(),
            Some("Named argument (optional)")
        );
    }

    #[test]
    fn completions_variadic_detail() {
        let params = vec![ParameterInfo {
            name: "$items".to_string(),
            is_required: true,
            type_hint: Some("string".to_string()),
            is_variadic: true,
            is_reference: false,
        }];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].detail.as_deref(),
            Some("Named argument (variadic)")
        );
    }

    #[test]
    fn completions_have_variable_kind() {
        let params = vec![make_param("x", Some("int"), true)];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
    }

    #[test]
    fn completions_empty_when_all_used() {
        let params = vec![
            make_param("x", Some("int"), true),
            make_param("y", Some("int"), true),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec!["x".to_string(), "y".to_string()],
            positional_count: 0,
            prefix: String::new(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert!(items.is_empty());
    }

    #[test]
    fn completions_prefix_case_insensitive() {
        let params = vec![
            make_param("Name", Some("string"), true),
            make_param("age", Some("int"), true),
        ];
        let ctx = NamedArgContext {
            call_expression: "foo".to_string(),
            existing_named_args: vec![],
            positional_count: 0,
            prefix: "na".to_string(),
        };

        let items = build_named_arg_completions(&ctx, &params);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].filter_text.as_deref(), Some("Name"));
    }
}
