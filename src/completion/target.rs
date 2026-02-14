/// Completion target extraction.
///
/// This module contains the logic for detecting the access operator (`->` or
/// `::`) before the cursor and extracting the textual subject to its left
/// (e.g. `$this`, `self`, `$var`, `$this->prop`, `ClassName`).
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;
use crate::util::skip_balanced_parens_back;

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
    fn extract_arrow_subject(chars: &[char], arrow_pos: usize) -> String {
        // Position just before the `->`
        let end = arrow_pos;

        // Skip whitespace
        let mut i = end;
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }

        // ── Function / method call: detect `)` before the operator ──
        // e.g. `app()->`, `$this->getService()->`, `Class::make()->`
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
    ///   - `"app()"` for a standalone function call
    ///   - `"$this->getService()"` for an instance method call
    ///   - `"ClassName::make()"` for a static method call
    fn extract_call_subject(chars: &[char], paren_end: usize) -> Option<String> {
        let open = skip_balanced_parens_back(chars, paren_end)?;
        if open == 0 {
            return None;
        }

        // Read the function / method name before `(`
        let mut i = open;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }
        if i == open {
            // No identifier before `(` — can't resolve
            return None;
        }
        let func_name: String = chars[i..open].iter().collect();

        // Check what precedes the function name to determine the kind of
        // call expression.

        // Instance method call: `$this->method()` / `$var->method()`
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_subject = Self::extract_simple_variable(chars, i - 2);
            if !inner_subject.is_empty() {
                return Some(format!("{}->{}()", inner_subject, func_name));
            }
        }

        // Null-safe method call: `$var?->method()`
        if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_subject = Self::extract_simple_variable(chars, i - 3);
            if !inner_subject.is_empty() {
                return Some(format!("{}?->{}()", inner_subject, func_name));
            }
        }

        // Static method call: `ClassName::method()` / `self::method()`
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            let class_subject = Self::extract_double_colon_subject_raw(chars, i - 2);
            if !class_subject.is_empty() {
                return Some(format!("{}::{}()", class_subject, func_name));
            }
        }

        // Standalone function call: `app()`
        Some(format!("{}()", func_name))
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
