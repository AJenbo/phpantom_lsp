/// Text-based type resolution helpers.
///
/// These functions resolve types by scanning raw source text rather than
/// working with parsed AST structures.  They are used as a lightweight
/// fallback when no `@var` / `@param` annotation is available and the
/// caller needs to infer a variable's type from its assignment RHS.
///
/// Extracted from [`super::resolver`] to keep that module focused on the
/// higher-level entry points (`resolve_target_classes`,
/// `resolve_call_return_types`, `type_hint_to_classes`, etc.).
///
/// The main entry point is
/// [`Backend::extract_raw_type_from_assignment_text`], which scans
/// backward from the cursor for `$var = …;` and extracts the raw type
/// string from the RHS expression.  It delegates to specialised helpers
/// for array literals, function/method calls, property access, chained
/// calls, and closure literals.
use crate::Backend;
use crate::docblock;
use crate::types::ClassInfo;
use crate::util::{
    ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS, find_semicolon_balanced, short_name,
};

use super::resolver::FunctionLoaderFn;

use super::array_shape::{
    build_list_type_from_push_types, collect_incremental_key_assignments, collect_push_assignments,
    extract_spread_expressions, parse_array_literal_entries,
};
use super::conditional_resolution::split_call_subject;

// ─── Chained array access types ─────────────────────────────────────────────

/// A single bracket segment in a chained array access subject.
///
/// Used by [`parse_bracket_segments`] to decompose subjects like
/// `$response['items'][]` into structured parts.
#[derive(Debug, Clone)]
pub(super) enum BracketSegment {
    /// A string-key access, e.g. `['items']` → `StringKey("items")`.
    StringKey(String),
    /// A numeric / variable index access, e.g. `[0]` or `[$i]` → `ElementAccess`.
    ElementAccess,
}

/// Result of parsing a chained array access subject.
#[derive(Debug)]
pub(super) struct BracketSubject {
    /// The base variable (e.g. `"$response"`).
    pub base_var: String,
    /// The bracket segments in left-to-right order.
    pub segments: Vec<BracketSegment>,
}

/// Parse a subject like `$var['key'][]` into its base variable and
/// bracket segments.
///
/// Returns `None` if the subject doesn't start with `$` or has no `[`.
pub(super) fn parse_bracket_segments(subject: &str) -> Option<BracketSubject> {
    if !subject.starts_with('$') || !subject.contains('[') {
        return None;
    }

    let first_bracket = subject.find('[')?;
    let base_var = subject[..first_bracket].to_string();
    if base_var.len() < 2 {
        return None;
    }

    let mut segments = Vec::new();
    let mut rest = &subject[first_bracket..];

    while rest.starts_with('[') {
        // Find the matching `]`.
        let close = rest.find(']')?;
        let inner = rest[1..close].trim();

        if let Some(key) = inner
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .or_else(|| inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
        {
            segments.push(BracketSegment::StringKey(key.to_string()));
        } else {
            segments.push(BracketSegment::ElementAccess);
        }

        rest = &rest[close + 1..];
    }

    if segments.is_empty() {
        return None;
    }

    Some(BracketSubject { base_var, segments })
}

/// Replace `self`, `static`, and `$this` tokens in a type string with
/// the concrete class name.
///
/// This is needed when a method's return type is extracted in a context
/// where the owning class is known but the caller will resolve the type
/// string without that class context (e.g. first-class callable return
/// types passed through with `owning_class_name = ""`).
fn replace_self_references(type_str: &str, class_name: &str) -> String {
    // Fast path: no substitution needed.
    if !type_str.contains("self") && !type_str.contains("static") && !type_str.contains("$this") {
        return type_str.to_string();
    }

    // Split on `|` (union) boundaries, replace each token that is
    // exactly `self`, `static`, or `$this` (with optional leading `?`).
    type_str
        .split('|')
        .map(|part| {
            let trimmed = part.trim();
            let (prefix, base) = if let Some(rest) = trimmed.strip_prefix('?') {
                ("?", rest)
            } else {
                ("", trimmed)
            };
            if base == "self" || base == "static" || base == "$this" {
                format!("{}{}", prefix, class_name)
            } else {
                trimmed.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("|")
}

// ─── impl Backend — text-based resolution methods ───────────────────────────

impl Backend {
    /// Scan backward from `cursor_offset` for `$base_var = …;` and
    /// extract the raw type string from the RHS expression.
    ///
    /// This is the text-based counterpart of the AST-driven variable
    /// resolution in [`super::variable_resolution`].  It handles:
    ///
    /// - Array literals (`[…]` / `array(…)`) with incremental key
    ///   assignments and push-style `$var[] = expr;`
    /// - Function calls (`functionName(…)`)
    /// - Method calls (`$this->methodName(…)`)
    /// - Static calls (`ClassName::methodName(…)`)
    /// - Chained calls (`$this->getRepo()->findAll()`)
    /// - `new ClassName(…)` expressions
    /// - Property access (`$this->propName`)
    /// - Known array functions (`array_filter`, `array_map`, etc.)
    pub(super) fn extract_raw_type_from_assignment_text(
        base_var: &str,
        content: &str,
        cursor_offset: usize,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let search_area = content.get(..cursor_offset)?;

        // Find the most recent assignment to this variable.
        let assign_pattern = format!("{} = ", base_var);
        let assign_pos = search_area.rfind(&assign_pattern)?;
        let rhs_start = assign_pos + assign_pattern.len();

        // Extract the RHS up to the next `;`
        let remaining = &content[rhs_start..];
        let semi_pos = find_semicolon_balanced(remaining)?;
        let rhs_text = remaining[..semi_pos].trim();

        // ── Array literal — `[…]` or `array(…)` ────────────────────
        // Check this BEFORE the function-call case because `array(…)`
        // ends with `)` and would otherwise be mistaken for a call.
        // Also scan for incremental `$var['key'] = expr;` assignments
        // and push-style `$var[] = expr;` assignments.
        let base_entries = parse_array_literal_entries(rhs_text);

        // Extract spread element types from the array literal (e.g.
        // `[...$users, ...$admins]` → resolve each spread variable's
        // iterable element type via docblock annotation).
        let spread_types = extract_spread_expressions(rhs_text)
            .unwrap_or_default()
            .iter()
            .filter_map(|expr| {
                if !expr.starts_with('$') {
                    return None;
                }
                // Try docblock annotation first (@var / @param).
                let raw =
                    crate::docblock::find_iterable_raw_type_in_source(content, cursor_offset, expr)
                        .or_else(|| {
                            // Fall back to resolving through assignment.
                            Self::extract_raw_type_from_assignment_text(
                                expr,
                                content,
                                cursor_offset,
                                current_class,
                                all_classes,
                                class_loader,
                            )
                        })?;
                crate::docblock::extract_iterable_element_type(&raw)
            })
            .collect::<Vec<_>>();

        let after_assign = rhs_start + semi_pos + 1; // past the `;`
        let incremental =
            collect_incremental_key_assignments(base_var, content, after_assign, cursor_offset);

        // Scan for push-style `$var[] = expr;` assignments.
        let mut push_types =
            collect_push_assignments(base_var, content, after_assign, cursor_offset);

        // Merge spread element types into push types so they participate
        // in the `list<…>` inference.
        push_types.extend(spread_types);

        if base_entries.is_some() || !incremental.is_empty() || !push_types.is_empty() {
            let mut entries: Vec<(String, String)> = base_entries.unwrap_or_default();
            // Merge incremental assignments — later assignments for the
            // same key override earlier ones.
            for (k, v) in incremental {
                if let Some(existing) = entries.iter_mut().find(|(ek, _)| *ek == k) {
                    existing.1 = v;
                } else {
                    entries.push((k, v));
                }
            }
            // If there are string-keyed entries, prefer the array shape.
            if !entries.is_empty() {
                let shape_parts: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                return Some(format!("array{{{}}}", shape_parts.join(", ")));
            }
            // No string-keyed entries — try push-style list inference.
            if let Some(list_type) = build_list_type_from_push_types(&push_types) {
                return Some(list_type);
            }
        }

        // ── First-class callable — `funcName(...)` / `$obj->method(...)` ──
        // Detect before the call expression branch because `fn(...)`
        // ends with `)` and would otherwise be resolved as a call.
        if rhs_text.ends_with("(...)") && rhs_text.len() > 4 {
            return Some("Closure".to_string());
        }

        // RHS is a call expression — extract the return type.
        //
        // Use backward paren scanning (like `split_call_subject`) so that
        // chained calls like `$this->getRepo()->findAll()` correctly
        // identify `findAll` as the outermost call, not `getRepo`.
        if rhs_text.ends_with(')') {
            let (callee, _args_text) = split_call_subject(rhs_text)?;

            // ── Chained call: callee contains `->` or `::` beyond a
            // single-level access ────────────────────────────────────
            // When the callee itself is a chain (e.g.
            // `$this->getRepo()->findAll`), delegate to
            // `resolve_raw_type_from_call_chain` which walks the full
            // chain recursively.
            let is_chain = callee.contains("->") && {
                if let Some(rest) = callee
                    .strip_prefix("$this->")
                    .or_else(|| callee.strip_prefix("$this?->"))
                {
                    rest.contains("->") || rest.contains("::")
                } else {
                    true
                }
            };
            let is_static_chain = !callee.contains("->") && callee.contains("::") && {
                let first_dc = callee.find("::").unwrap_or(0);
                callee[first_dc + 2..].contains("::") || callee[first_dc + 2..].contains("->")
            };

            if is_chain || is_static_chain {
                return Self::resolve_raw_type_from_call_chain(
                    callee,
                    _args_text,
                    current_class,
                    all_classes,
                    class_loader,
                );
            }

            // ── `(new ClassName(…))` or `new ClassName(…)` ──────────
            if let Some(class_name) = Self::extract_new_expression_class(rhs_text) {
                return Some(class_name);
            }

            // Method call: `$this->methodName(…)`
            if let Some(method_name) = callee.strip_prefix("$this->") {
                let owner = current_class?;
                return Self::resolve_method_return_type(owner, method_name, class_loader);
            }

            // Static call: `ClassName::methodName(…)`
            if let Some((class_part, method_part)) = callee.rsplit_once("::") {
                let resolved_class = if class_part == "self" || class_part == "static" {
                    current_class.cloned()
                } else {
                    class_loader(class_part)
                };
                if let Some(cls) = resolved_class {
                    return Self::resolve_method_return_type(&cls, method_part, class_loader);
                }
            }

            // ── Known array functions — preserve element type ───────
            if let Some(raw) = Self::resolve_array_func_raw_type_from_text(
                callee,
                _args_text,
                content,
                assign_pos,
                current_class,
                all_classes,
                class_loader,
            ) {
                return Some(raw);
            }

            // Standalone function call — search all classes for a matching
            // global function.  Since we don't have `function_loader` here,
            // search backward in the source for a `@return` in the
            // function's docblock.
            return Self::extract_function_return_from_source(callee, content);
        }

        // RHS is a property access: `$this->propName`
        if let Some(prop_name) = rhs_text.strip_prefix("$this->")
            && prop_name.chars().all(|c| c.is_alphanumeric() || c == '_')
            && let Some(owner) = current_class
        {
            return Self::resolve_property_type_hint(owner, prop_name, class_loader);
        }

        None
    }

    /// Resolve the raw return type of a known array function from text.
    ///
    /// This is the text-based counterpart of
    /// `variable_resolution::resolve_array_func_raw_type` and is used by
    /// `extract_raw_type_from_assignment_text` which operates on source
    /// text rather than the AST.
    fn resolve_array_func_raw_type_from_text(
        func_name: &str,
        args_text: &str,
        content: &str,
        before_offset: usize,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let is_preserving = ARRAY_PRESERVING_FUNCS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(func_name));
        let is_element = ARRAY_ELEMENT_FUNCS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(func_name));
        let is_array_map = func_name.eq_ignore_ascii_case("array_map");

        if !is_preserving && !is_element && !is_array_map {
            return None;
        }

        // For array_map the array is the second argument; for everything
        // else it's the first.
        let arg_index = if is_array_map { 1 } else { 0 };

        // Try to resolve the raw iterable type from the nth argument.
        // First try plain `$variable` with docblock lookup, then try
        // `$this->prop` via the enclosing class's property type hints,
        // and finally try `$variable` assigned from a method call.
        let raw = Self::resolve_nth_arg_raw_type(
            args_text,
            arg_index,
            content,
            before_offset,
            current_class,
            all_classes,
            class_loader,
        )?;

        // Make sure the raw type actually carries generic/array info.
        docblock::types::extract_generic_value_type(&raw)?;

        if is_preserving || is_array_map {
            // Return the full raw type so downstream callers can extract
            // the element type via `extract_generic_value_type`.
            Some(raw)
        } else {
            // Element-extracting: return just the element type.
            docblock::types::extract_generic_value_type(&raw)
        }
    }

    /// Resolve the raw iterable type of the nth argument in a text-based
    /// argument list.
    ///
    /// Tries multiple strategies in order:
    /// 1. Plain `$variable` → docblock `@var` / `@param` lookup
    /// 2. `$this->prop` → property type hint from the enclosing class
    /// 3. Plain `$variable` → chase its assignment to extract the raw type
    fn resolve_nth_arg_raw_type(
        args_text: &str,
        n: usize,
        content: &str,
        before_offset: usize,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let arg_text = Self::extract_nth_arg_text(args_text, n)?;

        // Strategy 1: plain `$variable` with @var / @param annotation.
        if let Some(var_name) = Self::extract_plain_variable(&arg_text) {
            if let Some(raw) =
                docblock::find_iterable_raw_type_in_source(content, before_offset, &var_name)
            {
                return Some(raw);
            }
            // Strategy 3: chase the variable's assignment to extract raw type.
            if let Some(raw) = Self::extract_raw_type_from_assignment_text(
                &var_name,
                content,
                before_offset,
                current_class,
                all_classes,
                class_loader,
            ) {
                return Some(raw);
            }
        }

        // Strategy 2: `$this->prop` — resolve via the enclosing class.
        if let Some(prop_name) = arg_text
            .strip_prefix("$this->")
            .or_else(|| arg_text.strip_prefix("$this?->"))
            && prop_name.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            let owner = current_class?;
            return Self::resolve_property_type_hint(owner, prop_name, class_loader);
        }

        None
    }

    /// Extract the nth (0-based) argument text from a comma-separated
    /// argument text string.
    ///
    /// Returns the raw trimmed argument text, which may be a plain
    /// variable, a property access, a function call, etc.  Respects
    /// nested parentheses and brackets so that commas inside sub-
    /// expressions are not treated as argument separators.
    fn extract_nth_arg_text(args_text: &str, n: usize) -> Option<String> {
        let trimmed = args_text.trim();
        let mut depth = 0i32;
        let mut arg_start = 0usize;
        let mut arg_index = 0usize;

        let bytes = trimmed.as_bytes();
        for (i, &ch) in bytes.iter().enumerate() {
            match ch {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => {
                    if arg_index == n {
                        let arg = trimmed[arg_start..i].trim();
                        if !arg.is_empty() {
                            return Some(arg.to_string());
                        }
                        return None;
                    }
                    arg_index += 1;
                    arg_start = i + 1;
                }
                _ => {}
            }
        }

        // Last (or only) argument.
        if arg_index == n {
            let arg = trimmed[arg_start..].trim();
            if !arg.is_empty() {
                return Some(arg.to_string());
            }
        }

        None
    }

    /// If `text` is a plain variable reference (`$foo`), return it.
    /// Returns `None` for expressions like `$foo->bar`, `func()`, etc.
    fn extract_plain_variable(text: &str) -> Option<String> {
        let text = text.trim();
        if text.starts_with('$')
            && text.len() > 1
            && text[1..].chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            Some(text.to_string())
        } else {
            None
        }
    }

    /// Extract the class name from a `new` expression, handling both
    /// parenthesized and bare forms:
    ///
    /// - `(new Builder())`  → `Some("Builder")`
    /// - `(new Builder)`    → `Some("Builder")`
    /// - `new Builder()`    → `Some("Builder")`
    /// - `(new \App\Builder())` → `Some("App\\Builder")`
    /// - `$this->foo()`     → `None`
    pub(super) fn extract_new_expression_class(s: &str) -> Option<String> {
        // Strip balanced outer parentheses.
        let inner = if s.starts_with('(') && s.ends_with(')') {
            &s[1..s.len() - 1]
        } else {
            s
        };
        let rest = inner.trim().strip_prefix("new ")?;
        let rest = rest.trim_start();
        // The class name runs until `(`, whitespace, or end-of-string.
        let end = rest
            .find(|c: char| c == '(' || c.is_whitespace())
            .unwrap_or(rest.len());
        let class_name = rest[..end].trim_start_matches('\\');
        if class_name.is_empty()
            || !class_name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
        {
            return None;
        }
        Some(class_name.to_string())
    }

    /// Resolve a chained call expression to a raw type string, walking
    /// the chain from left to right.
    ///
    /// This is used by `extract_raw_type_from_assignment_text` where we
    /// don't have a `function_loader` or full `ResolutionCtx`, only
    /// `class_loader`.  Handles:
    ///
    /// - `$this->getRepo()->findAll` + args → return type of `findAll`
    /// - `(new Builder())->build` + args → return type of `build`
    /// - `Factory::create()->process` + args → return type of `process`
    fn resolve_raw_type_from_call_chain(
        callee: &str,
        _args_text: &str,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        // Split at the rightmost `->` to get the final method name and
        // the LHS expression that produces the owning object.
        let pos = callee.rfind("->")?;
        let lhs = &callee[..pos];
        let method_name = &callee[pos + 2..];

        // Resolve LHS to a class.
        let owner = Self::resolve_lhs_to_class(lhs, current_class, all_classes, class_loader)?;
        Self::resolve_method_return_type(&owner, method_name, class_loader)
    }

    /// Resolve the left-hand side of a chained expression to a `ClassInfo`.
    ///
    /// Handles `$this` / `self` / `static`, `$this->prop`, `new Foo()`,
    /// `(new Foo())`, and recursive chains.  Used by
    /// `resolve_raw_type_from_call_chain` for the text-only path.
    fn resolve_lhs_to_class(
        lhs: &str,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        // `$this` / `self` / `static`
        if lhs == "$this" || lhs == "self" || lhs == "static" {
            return current_class.cloned();
        }

        // `(new ClassName(...))` or `new ClassName(...)`
        if let Some(class_name) = Self::extract_new_expression_class(lhs) {
            let lookup = short_name(&class_name);
            return all_classes
                .iter()
                .find(|c| c.name == lookup)
                .cloned()
                .or_else(|| class_loader(&class_name));
        }

        // LHS ends with `)` — it's a call expression.  Recurse.
        if lhs.ends_with(')') {
            let inner = lhs.strip_suffix(')')?;
            // Find matching open paren.
            let mut depth = 0u32;
            let mut open = None;
            for (i, b) in inner.bytes().enumerate().rev() {
                match b {
                    b')' => depth += 1,
                    b'(' => {
                        if depth == 0 {
                            open = Some(i);
                            break;
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            let open = open?;
            let inner_callee = &inner[..open];
            let inner_args = inner[open + 1..].trim();

            // Inner callee may itself be a chain — recurse.
            let ret_type = Self::resolve_raw_type_from_call_chain(
                inner_callee,
                inner_args,
                current_class,
                all_classes,
                class_loader,
            )
            .or_else(|| {
                // Single-level: `$this->method`
                if let Some(m) = inner_callee
                    .strip_prefix("$this->")
                    .or_else(|| inner_callee.strip_prefix("$this?->"))
                {
                    let owner = current_class?;
                    return Self::resolve_method_return_type(owner, m, class_loader);
                }
                // `ClassName::method`
                if let Some((cls_part, m_part)) = inner_callee.rsplit_once("::") {
                    let resolved = if cls_part == "self" || cls_part == "static" {
                        current_class.cloned()
                    } else {
                        let lookup = short_name(cls_part);
                        all_classes
                            .iter()
                            .find(|c| c.name == lookup)
                            .cloned()
                            .or_else(|| class_loader(cls_part))
                    };
                    if let Some(cls) = resolved {
                        return Self::resolve_method_return_type(&cls, m_part, class_loader);
                    }
                }
                None
            })?;

            // `ret_type` is a type string — resolve it to ClassInfo.
            let clean = crate::docblock::types::clean_type(&ret_type);
            let lookup = short_name(&clean);
            return all_classes
                .iter()
                .find(|c| c.name == lookup)
                .cloned()
                .or_else(|| class_loader(&clean));
        }

        // `$this->prop` — property access
        if let Some(prop) = lhs
            .strip_prefix("$this->")
            .or_else(|| lhs.strip_prefix("$this?->"))
            && prop.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            let owner = current_class?;
            let type_str = Self::resolve_property_type_hint(owner, prop, class_loader)?;
            let clean = crate::docblock::types::clean_type(&type_str);
            let lookup = short_name(&clean);
            return all_classes
                .iter()
                .find(|c| c.name == lookup)
                .cloned()
                .or_else(|| class_loader(&clean));
        }

        None
    }

    /// Search backward in `content` for a function definition matching
    /// `func_name` and extract its `@return` type from the docblock.
    fn extract_function_return_from_source(func_name: &str, content: &str) -> Option<String> {
        // Look for `function funcName(` in the source.
        let pattern = format!("function {}(", func_name);
        let func_pos = content.find(&pattern)?;

        // Search backward from the function definition for a docblock.
        let before = content.get(..func_pos)?;
        let trimmed = before.trim_end();
        if !trimmed.ends_with("*/") {
            return None;
        }
        let open_pos = trimmed.rfind("/**")?;
        let docblock = &trimmed[open_pos..];

        docblock::extract_return_type(docblock)
    }

    /// Scan backward through `content` for a closure or arrow-function
    /// literal assigned to `var_name` and extract the native return type
    /// hint from the source text.
    ///
    /// Matches patterns like:
    ///   - `$fn = function(): User { … }`
    ///   - `$fn = fn(): User => …`
    ///   - `$fn = function(): ?Response { … }`
    ///
    /// Returns the return type string (e.g. `"User"`, `"?Response"`) or
    /// `None` if no closure assignment is found or it has no return type.
    pub(super) fn extract_closure_return_type_from_assignment(
        var_name: &str,
        content: &str,
        cursor_offset: u32,
    ) -> Option<String> {
        let search_area = content.get(..cursor_offset as usize)?;

        // Look for `$fn = function` or `$fn = fn` assignment.
        let assign_prefix = format!("{} = ", var_name);
        let assign_pos = search_area.rfind(&assign_prefix)?;
        let rhs_start = assign_pos + assign_prefix.len();
        let rhs = search_area.get(rhs_start..)?.trim_start();

        // Match `function(…): ReturnType` or `fn(…): ReturnType => …`
        let is_closure = rhs.starts_with("function") && rhs[8..].trim_start().starts_with('(');
        let is_arrow = rhs.starts_with("fn") && rhs[2..].trim_start().starts_with('(');

        if !is_closure && !is_arrow {
            return None;
        }

        // Find the opening `(` of the parameter list.
        let paren_open = rhs.find('(')?;
        // Find the matching `)` by tracking depth.
        let mut depth = 0i32;
        let mut paren_close = None;
        for (i, c) in rhs[paren_open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        paren_close = Some(paren_open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let paren_close = paren_close?;

        // After `)`, look for `: ReturnType`.
        let after_paren = rhs.get(paren_close + 1..)?.trim_start();
        // For closures there may be a `use (…)` clause before the return type.
        let after_use = if after_paren.starts_with("use") {
            let use_paren = after_paren.find('(')?;
            let mut udepth = 0i32;
            let mut use_close = None;
            for (i, c) in after_paren[use_paren..].char_indices() {
                match c {
                    '(' => udepth += 1,
                    ')' => {
                        udepth -= 1;
                        if udepth == 0 {
                            use_close = Some(use_paren + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            after_paren.get(use_close? + 1..)?.trim_start()
        } else {
            after_paren
        };

        // Expect `: ReturnType`
        let after_colon = after_use.strip_prefix(':')?.trim_start();
        if after_colon.is_empty() {
            return None;
        }

        // Extract the return type token — stop at `{`, `=>`, or whitespace.
        let end = after_colon
            .find(|c: char| c == '{' || c == '=' || c.is_whitespace())
            .unwrap_or(after_colon.len());
        let ret_type = after_colon[..end].trim();
        if ret_type.is_empty() {
            return None;
        }

        Some(ret_type.to_string())
    }

    /// Scan backward through `content` for a first-class callable
    /// assignment to `var_name` and resolve the underlying
    /// function/method's return type.
    ///
    /// Matches patterns like:
    ///   - `$fn = strlen(...)`            → look up `strlen` return type
    ///   - `$fn = $this->method(...)`     → look up method return type
    ///   - `$fn = $obj->method(...)`      → resolve `$obj`, look up method
    ///   - `$fn = ClassName::method(...)` → look up static method return type
    ///
    /// Returns the return type string (e.g. `"int"`, `"User"`) or `None`
    /// if no first-class callable assignment is found or the return type
    /// cannot be determined.
    pub(super) fn extract_first_class_callable_return_type(
        var_name: &str,
        content: &str,
        cursor_offset: u32,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<String> {
        let search_area = content.get(..cursor_offset as usize)?;

        // Look for `$fn = ` assignment.
        let assign_prefix = format!("{} = ", var_name);
        let assign_pos = search_area.rfind(&assign_prefix)?;
        let rhs_start = assign_pos + assign_prefix.len();

        // Extract the RHS up to the next `;`
        let remaining = &content[rhs_start..];
        let semi_pos = find_semicolon_balanced(remaining)?;
        let rhs_text = remaining[..semi_pos].trim();

        // Must end with `(...)` — the first-class callable marker.
        let callable_text = rhs_text.strip_suffix("(...)")?.trim_end();
        if callable_text.is_empty() {
            return None;
        }

        // ── Instance method: `$this->method` or `$obj->method` ──────
        if let Some(pos) = callable_text.rfind("->") {
            let lhs = callable_text[..pos].trim_end_matches('?');
            let method_name = &callable_text[pos + 2..];

            let owner = if lhs == "$this" || lhs == "self" || lhs == "static" {
                current_class.cloned()
            } else if lhs.starts_with('$') {
                // Bare variable LHS like `$factory->create(...)`.
                // Resolve the variable's type via assignment scanning,
                // then look up the resulting class.
                Self::extract_raw_type_from_assignment_text(
                    lhs,
                    content,
                    cursor_offset as usize,
                    current_class,
                    all_classes,
                    class_loader,
                )
                .and_then(|raw| {
                    let clean = crate::docblock::types::clean_type(&raw);
                    let lookup = short_name(&clean);
                    all_classes
                        .iter()
                        .find(|c| c.name == lookup)
                        .cloned()
                        .or_else(|| class_loader(&clean))
                })
            } else {
                // Non-variable LHS (e.g. chained call) — delegate to
                // the general-purpose text resolver.
                Self::resolve_lhs_to_class(lhs, current_class, all_classes, class_loader)
            };

            if let Some(cls) = owner {
                return Self::resolve_method_return_type(&cls, method_name, class_loader)
                    .map(|ret| replace_self_references(&ret, &cls.name));
            }
            return None;
        }

        // ── Static method: `ClassName::method` / `self::method` ─────
        if let Some(pos) = callable_text.rfind("::") {
            let class_part = &callable_text[..pos];
            let method_name = &callable_text[pos + 2..];

            let owner = if class_part == "self" || class_part == "static" {
                current_class.cloned()
            } else if class_part == "parent" {
                current_class
                    .and_then(|cc| cc.parent_class.as_ref())
                    .and_then(|p| class_loader(p))
            } else {
                let lookup = short_name(class_part);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(class_part))
            };

            if let Some(cls) = owner {
                return Self::resolve_method_return_type(&cls, method_name, class_loader)
                    .map(|ret| replace_self_references(&ret, &cls.name));
            }
            return None;
        }

        // ── Plain function: `strlen`, `array_map`, etc. ─────────────
        if callable_text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
            && !callable_text.starts_with('$')
        {
            let func_info = function_loader?(callable_text)?;
            return func_info.return_type;
        }

        None
    }

    /// Resolve a chained array access subject like `$var['key'][]`.
    ///
    /// Walks through each bracket segment in order:
    /// - `BracketSegment::StringKey(k)` → extract the value type for key
    ///   `k` from an array shape annotation.
    /// - `BracketSegment::ElementAccess` → extract the generic element
    ///   type (e.g. `list<User>` → `User`).
    ///
    /// Returns the resolved `ClassInfo` for the final type.
    pub(super) fn resolve_chained_array_access(
        base_var: &str,
        segments: &[BracketSegment],
        content: &str,
        cursor_offset: u32,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        let current_class_name = current_class.map(|c| c.name.as_str()).unwrap_or("");

        // 1. Resolve the raw type annotation for the base variable.
        let raw_type =
            docblock::find_iterable_raw_type_in_source(content, cursor_offset as usize, base_var)
                .or_else(|| {
                    Self::extract_raw_type_from_assignment_text(
                        base_var,
                        content,
                        cursor_offset as usize,
                        current_class,
                        all_classes,
                        class_loader,
                    )
                });

        let mut current_type = match raw_type {
            Some(t) => t,
            None => return vec![],
        };

        // 2. Walk through each bracket segment to refine the type.
        for seg in segments {
            match seg {
                BracketSegment::StringKey(key) => {
                    // Array shape key lookup: array{key: Type} → Type
                    current_type =
                        match docblock::extract_array_shape_value_type(&current_type, key) {
                            Some(t) => t,
                            None => return vec![],
                        };
                }
                BracketSegment::ElementAccess => {
                    // Generic element extraction: list<User> → User
                    current_type = match docblock::types::extract_generic_value_type(&current_type)
                    {
                        Some(t) => t,
                        None => return vec![],
                    };
                }
            }
        }

        // 3. Resolve the final type string to ClassInfo.
        let cleaned = docblock::clean_type(&current_type);
        let base_name = docblock::types::strip_generics(&cleaned);
        if base_name.is_empty() || docblock::types::is_scalar(&base_name) {
            return vec![];
        }

        Self::type_hint_to_classes(&cleaned, current_class_name, all_classes, class_loader)
    }
}
