/// Completion target extraction.
///
/// This module contains the logic for detecting the access operator (`->` or
/// `::`) before the cursor and extracting the textual subject to its left
/// (e.g. `$this`, `self`, `$var`, `$this->prop`, `ClassName`).
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;
use crate::util::{
    check_new_keyword_before, extract_new_expression_inside_parens, skip_balanced_parens_back,
};

impl Backend {
    /// Detect the access operator before the cursor position and extract
    /// both the `AccessKind` and the textual subject to its left.
    ///
    /// Returns `None` when no `->` or `::` is found (i.e. `AccessKind::Other`).
    pub fn extract_completion_target(
        content: &str,
        position: Position,
    ) -> Option<CompletionTarget> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards past any partial identifier the user may have typed
        let mut i = col;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        // Detect operator
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let subject = Self::extract_arrow_subject(&chars, i - 2);
            Some(CompletionTarget {
                access_kind: AccessKind::Arrow,
                subject,
            })
        } else if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            let subject = Self::extract_double_colon_subject(&chars, i - 2);
            Some(CompletionTarget {
                access_kind: AccessKind::DoubleColon,
                subject,
            })
        } else {
            None
        }
    }

    /// Kept for backward-compat with existing tests that call it directly.
    pub fn detect_access_kind(content: &str, position: Position) -> AccessKind {
        match Self::extract_completion_target(content, position) {
            Some(ct) => ct.access_kind,
            None => AccessKind::Other,
        }
    }

    /// Extract the subject expression before `->` (the arrow sits at
    /// `chars[arrow_pos]` = `-`, `chars[arrow_pos+1]` = `>`).
    ///
    /// Handles:
    ///   - `$this->`, `$var->` (simple variable)
    ///   - `$this->prop->` (property chain)
    ///   - `app()->` (function call)
    ///   - `$this->getService()->` (method call chain)
    ///   - `ClassName::make()->` (static method call)
    ///   - `new ClassName()->` (instantiation, PHP 8.4+)
    ///   - `(new ClassName())->` (parenthesized instantiation)
    fn extract_arrow_subject(chars: &[char], arrow_pos: usize) -> String {
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
            && let Some(call_subject) = Self::extract_call_subject(chars, i)
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
            let inner_subject = Self::extract_simple_variable(chars, inner_arrow);
            if !inner_subject.is_empty() {
                let prop: String = chars[ident_start..ident_end].iter().collect();
                return format!("{}->{}", inner_subject, prop);
            }
        }

        // Check if preceded by `?->` (null-safe)
        if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_arrow = i - 3;
            let inner_subject = Self::extract_simple_variable(chars, inner_arrow);
            if !inner_subject.is_empty() {
                let prop: String = chars[ident_start..ident_end].iter().collect();
                return format!("{}?->{}", inner_subject, prop);
            }
        }

        // Otherwise treat the whole thing as a simple variable like `$this` or `$var`
        Self::extract_simple_variable(chars, end)
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
    ///   - `"ClassName"` for `new ClassName()` instantiation
    fn extract_call_subject(chars: &[char], paren_end: usize) -> Option<String> {
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
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
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
                && let Some(inner_call) = Self::extract_call_subject(chars, j)
            {
                return Some(format!("{}->{}", inner_call, rhs));
            }
            let inner_subject = Self::extract_simple_variable(chars, i - 2);
            if !inner_subject.is_empty() {
                return Some(format!("{}->{}", inner_subject, rhs));
            }
        }

        // Null-safe method call: `$var?->method()`
        if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_subject = Self::extract_simple_variable(chars, i - 3);
            if !inner_subject.is_empty() {
                return Some(format!("{}?->{}", inner_subject, rhs));
            }
        }

        // Static method call: `ClassName::method()` / `self::method()`
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            let class_subject = Self::extract_double_colon_subject_raw(chars, i - 2);
            if !class_subject.is_empty() {
                return Some(format!("{}::{}()", class_subject, func_name));
            }
        }

        // Standalone function call: preserve arguments for conditional
        // return-type resolution (e.g. `app(A::class)` instead of `app()`).
        Some(rhs)
    }

    /// Raw helper: extract identifier/keyword before `::` without going
    /// through the public `extract_double_colon_subject` API.
    fn extract_double_colon_subject_raw(chars: &[char], colon_pos: usize) -> String {
        let mut i = colon_pos;
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        let end = i;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }
        if i > 0 && chars[i - 1] == '$' {
            i -= 1;
        }
        chars[i..end].iter().collect()
    }

    /// Extract a simple `$variable` ending at position `end` (exclusive).
    fn extract_simple_variable(chars: &[char], end: usize) -> String {
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
    /// Handles `self::`, `static::`, `parent::`, `ClassName::`, `Foo\Bar::`.
    fn extract_double_colon_subject(chars: &[char], colon_pos: usize) -> String {
        let mut i = colon_pos;
        // skip whitespace
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        let end = i;
        // walk back through identifier chars (including `\` for namespaces)
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }
        // Also accept `$` prefix for `$var::` edge case (variable class name)
        if i > 0 && chars[i - 1] == '$' {
            i -= 1;
        }
        chars[i..end].iter().collect()
    }
}
