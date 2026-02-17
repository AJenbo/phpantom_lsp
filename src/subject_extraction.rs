//! Shared subject-extraction helpers.
//!
//! This module contains free functions for extracting the expression
//! ("subject") to the left of an access operator (`->`, `?->`, `::`) in
//! a line of PHP source code.  These are used by both the **completion**
//! and **definition** subsystems so that the logic is defined once.
//!
//! All functions operate on a `&[char]` slice representing a single line
//! and work backwards from a given position.
//!
//! # Subjects
//!
//! A "subject" is the textual expression that precedes an operator.
//! Examples:
//!
//! | Source                        | Operator | Subject                 |
//! |------------------------------|----------|-------------------------|
//! | `$this->`                    | `->`     | `$this`                 |
//! | `$this->prop->`              | `->`     | `$this->prop`           |
//! | `app()->`                    | `->`     | `app()`                 |
//! | `app(A::class)->`            | `->`     | `app(A::class)`         |
//! | `$this->getService()->`      | `->`     | `$this->getService()`   |
//! | `ClassName::make()->`        | `->`     | `ClassName::make()`     |
//! | `new Foo()->`                | `->`     | `Foo`                   |
//! | `(new Foo())->`              | `->`     | `Foo`                   |
//! | `Status::Active->`           | `->`     | `Status::Active`        |
//! | `self::`                     | `::`     | `self`                  |
//! | `ClassName::`                | `::`     | `ClassName`             |
//! | `$var?->`                    | `?->`    | `$var`                  |

// ─── Character-level helpers ────────────────────────────────────────────────
//
// These were previously in `util.rs` but are only consumed by the
// subject-extraction logic in this module, so they live here now.

/// Skip backwards past a balanced parenthesised group `(…)` in a char slice.
///
/// `pos` must point one past the closing `)`.  Returns the index of the
/// opening `(`, or `None` if parens are unbalanced.
pub(crate) fn skip_balanced_parens_back(chars: &[char], pos: usize) -> Option<usize> {
    if pos == 0 || chars[pos - 1] != ')' {
        return None;
    }
    let mut depth: u32 = 0;
    let mut j = pos;
    while j > 0 {
        j -= 1;
        match chars[j] {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// Check if the `new` keyword (followed by whitespace) appears immediately
/// before the identifier starting at position `ident_start`.
///
/// Returns the class name (possibly with namespace) if `new` is found.
pub(crate) fn check_new_keyword_before(
    chars: &[char],
    ident_start: usize,
    class_name: &str,
) -> Option<String> {
    let mut j = ident_start;
    // Skip whitespace between `new` and the class name.
    while j > 0 && chars[j - 1] == ' ' {
        j -= 1;
    }
    // Check for the `new` keyword.
    if j >= 3 && chars[j - 3] == 'n' && chars[j - 2] == 'e' && chars[j - 1] == 'w' {
        // Verify word boundary before `new` (start of line, whitespace, `(`, etc.).
        let before_ok = j == 3 || {
            let prev = chars[j - 4];
            !prev.is_alphanumeric() && prev != '_'
        };
        if before_ok {
            // Strip leading `\` from FQN if present.
            let name = class_name.strip_prefix('\\').unwrap_or(class_name);
            return Some(name.to_string());
        }
    }
    None
}

/// Try to extract a class name from a parenthesized `new` expression:
/// `(new ClassName(...))`.
///
/// `open` is the position of the outer `(`, `close` is one past the
/// outer `)`.  The function looks inside for the pattern
/// `new ClassName(...)`.
pub(crate) fn extract_new_expression_inside_parens(
    chars: &[char],
    open: usize,
    close: usize,
) -> Option<String> {
    // Content is chars[open+1 .. close-1].
    let inner_start = open + 1;
    let inner_end = close - 1;
    if inner_start >= inner_end {
        return None;
    }

    // Skip whitespace inside the opening `(`.
    let mut k = inner_start;
    while k < inner_end && chars[k] == ' ' {
        k += 1;
    }

    // Check for `new` keyword.
    if k + 3 >= inner_end {
        return None;
    }
    if chars[k] != 'n' || chars[k + 1] != 'e' || chars[k + 2] != 'w' {
        return None;
    }
    k += 3;

    // Must be followed by whitespace.
    if k >= inner_end || chars[k] != ' ' {
        return None;
    }
    while k < inner_end && chars[k] == ' ' {
        k += 1;
    }

    // Read the class name (may include `\` for namespaces).
    let name_start = k;
    while k < inner_end && (chars[k].is_alphanumeric() || chars[k] == '_' || chars[k] == '\\') {
        k += 1;
    }
    if k == name_start {
        return None;
    }
    let class_name: String = chars[name_start..k].iter().collect();
    let name = class_name.strip_prefix('\\').unwrap_or(&class_name);
    Some(name.to_string())
}

// ─── Subject extraction ─────────────────────────────────────────────────────

/// Extract the subject expression before an arrow operator (`->`).
///
/// `chars` is the line as a char slice.  `arrow_pos` is the index of
/// the `-` character (i.e. `chars[arrow_pos] == '-'` and
/// `chars[arrow_pos + 1] == '>'`).
///
/// Handles:
///   - `$this->`, `$var->` (simple variable)
///   - `$this->prop->` (property chain)
///   - `$this?->prop->` (nullsafe property chain)
///   - `app()->` (function call)
///   - `$this->getService()->` (method call chain)
///   - `ClassName::make()->` (static method call)
///   - `new ClassName()->` (instantiation, PHP 8.4+)
///   - `(new ClassName())->` (parenthesized instantiation)
///   - `Status::Active->` (enum case access)
///   - `tryFrom($int)?->` (nullsafe after call)
pub(crate) fn extract_arrow_subject(chars: &[char], arrow_pos: usize) -> String {
    // Position just before the `->`
    let mut end = arrow_pos;

    // Skip whitespace
    let mut i = end;
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }

    // Skip the `?` of the nullsafe `?->` operator so that the rest
    // of the extraction logic sees the expression before the `?`
    // (e.g. the `)` of a call expression like `tryFrom($int)?->`,
    // or a simple variable like `$var?->`).
    if i > 0 && chars[i - 1] == '?' {
        i -= 1;
    }

    // Update `end` so the fallback `extract_simple_variable` at the
    // bottom of this function also starts from the correct position
    // (past any `?` and whitespace).
    end = i;

    // ── Function / method call or `new` expression: detect `)` ──
    // e.g. `app()->`, `$this->getService()->`, `Class::make()->`,
    //      `new Foo()->`, `(new Foo())->`
    if i > 0
        && chars[i - 1] == ')'
        && let Some(call_subject) = extract_call_subject(chars, i)
    {
        return call_subject;
    }

    // Try to read an identifier (property name if chained)
    let ident_end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }
    let ident_start = i;

    // Check whether this identifier is preceded by another `->` (chained access)
    if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
        // We have something like  `expr->ident->` — extract inner subject
        let inner_arrow = i - 2;
        let inner_subject = extract_simple_variable(chars, inner_arrow);
        if !inner_subject.is_empty() {
            let prop: String = chars[ident_start..ident_end].iter().collect();
            return format!("{}->{}", inner_subject, prop);
        }
    }

    // Check if preceded by `?->` (null-safe)
    if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
        let inner_arrow = i - 3;
        let inner_subject = extract_simple_variable(chars, inner_arrow);
        if !inner_subject.is_empty() {
            let prop: String = chars[ident_start..ident_end].iter().collect();
            return format!("{}?->{}", inner_subject, prop);
        }
    }

    // Check if preceded by `::` (enum case or static member access,
    // e.g. `Status::Active->`)
    if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
        let class_subject = extract_double_colon_subject(chars, i - 2);
        if !class_subject.is_empty() {
            let ident: String = chars[ident_start..ident_end].iter().collect();
            return format!("{}::{}", class_subject, ident);
        }
    }

    // Otherwise treat the whole thing as a simple variable like `$this` or `$var`
    extract_simple_variable(chars, end)
}

/// Extract the full call-expression subject when `)` appears before an
/// operator.
///
/// `paren_end` is the position one past the closing `)`.
///
/// Returns subjects such as:
///   - `"app()"` for a standalone function call without arguments
///   - `"app(A::class)"` for a function call with arguments (preserved)
///   - `"$this->getService()"` for an instance method call
///   - `"ClassName::make()"` for a static method call
///   - `"ClassName::make(Arg::class)"` for a static call with arguments
///   - `"ClassName"` for `new ClassName()` instantiation
pub(crate) fn extract_call_subject(chars: &[char], paren_end: usize) -> Option<String> {
    let open = skip_balanced_parens_back(chars, paren_end)?;
    if open == 0 {
        return None;
    }

    // Capture the argument text between the parentheses for later use
    // in conditional return-type resolution (e.g. `app(A::class)`).
    let args_text: String = chars[open + 1..paren_end - 1].iter().collect();
    let args_text = args_text.trim();

    // Read the function / method name before `(`
    let mut i = open;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\') {
        i -= 1;
    }
    if i == open {
        // No identifier before `(` — check if the contents inside the
        // balanced parens form a `(new ClassName(...))` expression.
        return extract_new_expression_inside_parens(chars, open, paren_end);
    }
    let func_name: String = chars[i..open].iter().collect();

    // ── `new ClassName()` instantiation ──
    // Check if the `new` keyword immediately precedes the class name.
    if let Some(class_name) = check_new_keyword_before(chars, i, &func_name) {
        return Some(class_name);
    }

    // Build the right-hand side of the call expression, preserving
    // arguments for conditional return-type resolution.
    let rhs = if args_text.is_empty() {
        format!("{}()", func_name)
    } else {
        format!("{}({})", func_name, args_text)
    };

    // Check what precedes the function name to determine the kind of
    // call expression.

    // Instance method call: `$this->method()` / `$var->method()` /
    // `app()->method()` (chained call expression)
    if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
        // First check if the LHS is itself a call expression ending
        // with `)` — e.g. `app()->make(...)` where we need to
        // recursively resolve `app()`.
        let arrow_pos = i - 2;
        let mut j = arrow_pos;
        while j > 0 && chars[j - 1] == ' ' {
            j -= 1;
        }
        if j > 0
            && chars[j - 1] == ')'
            && let Some(inner_call) = extract_call_subject(chars, j)
        {
            return Some(format!("{}->{}", inner_call, rhs));
        }
        let inner_subject = extract_simple_variable(chars, i - 2);
        if !inner_subject.is_empty() {
            return Some(format!("{}->{}", inner_subject, rhs));
        }
    }

    // Null-safe method call: `$var?->method()`
    if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
        let inner_subject = extract_simple_variable(chars, i - 3);
        if !inner_subject.is_empty() {
            return Some(format!("{}?->{}", inner_subject, rhs));
        }
    }

    // Static method call: `ClassName::method()` / `self::method()`
    if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
        let class_subject = extract_double_colon_subject(chars, i - 2);
        if !class_subject.is_empty() {
            return Some(format!("{}::{}", class_subject, rhs));
        }
    }

    // Standalone function call: preserve arguments for conditional
    // return-type resolution (e.g. `app(A::class)` instead of `app()`).
    Some(rhs)
}

/// Extract a simple `$variable` or bare identifier ending at position
/// `end` (exclusive).
///
/// Skips trailing whitespace, then walks backwards through identifier
/// characters.  If a `$` prefix is found, includes it (producing e.g.
/// `"$this"`, `"$var"`).  Otherwise returns whatever identifier was
/// collected (e.g. `"self"`, `"parent"`), which may be empty.
pub(crate) fn extract_simple_variable(chars: &[char], end: usize) -> String {
    let mut i = end;
    // skip whitespace
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }
    let var_end = i;
    // walk back through identifier chars
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }
    // expect `$` prefix
    if i > 0 && chars[i - 1] == '$' {
        i -= 1;
        chars[i..var_end].iter().collect()
    } else {
        // no `$` — return whatever we collected (may be empty)
        chars[i..var_end].iter().collect()
    }
}

/// Extract the identifier/keyword before `::`.
///
/// `colon_pos` is the index of the first `:` (i.e. `chars[colon_pos] == ':'`
/// and `chars[colon_pos + 1] == ':'`).
///
/// Handles `self::`, `static::`, `parent::`, `ClassName::`, `Foo\Bar::`,
/// and the edge case `$var::`.
pub(crate) fn extract_double_colon_subject(chars: &[char], colon_pos: usize) -> String {
    let mut i = colon_pos;
    // skip whitespace
    while i > 0 && chars[i - 1] == ' ' {
        i -= 1;
    }
    let end = i;
    // walk back through identifier chars (including `\` for namespaces)
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\') {
        i -= 1;
    }
    // Also accept `$` prefix for `$var::` edge case (variable class name)
    if i > 0 && chars[i - 1] == '$' {
        i -= 1;
    }
    chars[i..end].iter().collect()
}
