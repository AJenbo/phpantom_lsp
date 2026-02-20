/// Type resolution for completion subjects.
///
/// This module contains the core entry points for resolving a completion
/// subject (e.g. `$this`, `self`, `static`, `$var`, `$this->prop`,
/// `ClassName`) to a concrete `ClassInfo` so that the correct completion
/// items can be offered.
///
/// The resolution logic is split across several sibling modules:
///
/// - [`super::variable_resolution`]: Variable type resolution via
///   assignment scanning and parameter type hints.
/// - [`super::type_narrowing`]: instanceof / assert / custom type guard
///   narrowing.
/// - [`super::closure_resolution`]: Closure and arrow-function parameter
///   resolution.
/// - [`crate::inheritance`]: Class inheritance merging (traits, mixins,
///   parent chain).
/// - [`super::conditional_resolution`]: PHPStan conditional return type
///   resolution at call sites.
use std::collections::HashMap;

use crate::Backend;
use crate::docblock;
use crate::docblock::types::{
    parse_generic_args, split_intersection_depth0, split_union_depth0, strip_generics,
};
use crate::inheritance::{apply_generic_args, apply_substitution};
use crate::types::*;

use super::conditional_resolution::{
    resolve_conditional_with_text_args, resolve_conditional_without_args, split_call_subject,
    split_text_args,
};

/// A single bracket segment in a chained array access subject.
///
/// Used by [`parse_bracket_segments`] to decompose subjects like
/// `$response['items'][]` into structured parts.
#[derive(Debug, Clone)]
enum BracketSegment {
    /// A string-key access, e.g. `['items']` → `StringKey("items")`.
    StringKey(String),
    /// A numeric / variable index access, e.g. `[0]` or `[$i]` → `ElementAccess`.
    ElementAccess,
}

/// Result of parsing a chained array access subject.
#[derive(Debug)]
struct BracketSubject {
    /// The base variable (e.g. `"$response"`).
    base_var: String,
    /// The bracket segments in left-to-right order.
    segments: Vec<BracketSegment>,
}

/// Parse a subject like `$var['key'][]` into its base variable and
/// bracket segments.
///
/// Returns `None` if the subject doesn't start with `$` or has no `[`.
fn parse_bracket_segments(subject: &str) -> Option<BracketSubject> {
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

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

/// Bundles the common parameters threaded through variable-type resolution.
///
/// Introducing this struct avoids passing 7–10 individual arguments to
/// every helper in the resolution chain, which keeps clippy happy and
/// makes call-sites much easier to read.
pub(super) struct VarResolutionCtx<'a> {
    pub var_name: &'a str,
    pub current_class: &'a ClassInfo,
    pub all_classes: &'a [ClassInfo],
    pub content: &'a str,
    pub cursor_offset: u32,
    pub class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    pub function_loader: FunctionLoaderFn<'a>,
}

/// Bundles the common parameters threaded through call-expression
/// return-type resolution.
///
/// This keeps the argument count of [`resolve_call_return_types`] under
/// clippy's `too_many_arguments` threshold.
pub(super) struct CallResolutionCtx<'a> {
    pub current_class: Option<&'a ClassInfo>,
    pub all_classes: &'a [ClassInfo],
    pub content: &'a str,
    pub cursor_offset: u32,
    pub class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    pub function_loader: FunctionLoaderFn<'a>,
}

/// Split a subject string at the **last** `->` or `?->` operator,
/// returning `(base, property_name)`.
///
/// Only splits at depth 0 (i.e. arrows inside balanced parentheses are
/// ignored).  Returns `None` if no arrow is found at depth 0.
///
/// # Examples
///
/// - `"$user->address"` → `Some(("$user", "address"))`
/// - `"$user->address->city"` → `Some(("$user->address", "city"))`
/// - `"$user?->address"` → `Some(("$user", "address"))`
fn split_last_arrow(subject: &str) -> Option<(&str, &str)> {
    let bytes = subject.as_bytes();
    let mut depth = 0i32;
    let mut last_arrow: Option<(usize, usize)> = None; // (start_of_arrow, start_of_prop)

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'-' if depth == 0 && i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                // Check for `?->`: the char before `-` might be `?`
                let arrow_start = if i > 0 && bytes[i - 1] == b'?' {
                    i - 1
                } else {
                    i
                };
                let prop_start = i + 2; // skip `->`
                last_arrow = Some((arrow_start, prop_start));
                i += 2; // skip past `->`
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    let (arrow_start, prop_start) = last_arrow?;
    if prop_start >= subject.len() {
        return None;
    }
    let base = &subject[..arrow_start];
    let prop = &subject[prop_start..];
    if base.is_empty() || prop.is_empty() {
        return None;
    }
    Some((base, prop))
}

impl Backend {
    /// Determine which class (if any) the completion subject refers to.
    ///
    /// `current_class` is the class the cursor is inside (if any).
    /// `all_classes` is every class we know about in the current file.
    /// `content` + `cursor_offset` are used for variable-type resolution.
    /// `class_loader` is a fallback that can search across files / load
    /// classes on demand (e.g. via PSR-4).
    ///
    /// Returns an owned `ClassInfo` if the type could be resolved.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_target_class(
        subject: &str,
        access_kind: AccessKind,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<ClassInfo> {
        Self::resolve_target_classes(
            subject,
            access_kind,
            current_class,
            all_classes,
            content,
            cursor_offset,
            class_loader,
            function_loader,
        )
        .into_iter()
        .next()
    }

    /// Like `resolve_target_class`, but returns **all** candidate types.
    ///
    /// When a variable is assigned different types in conditional branches
    /// (e.g. an `if` block reassigns `$thing`), this returns every possible
    /// type so the caller can try each one when looking up members.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_target_classes(
        subject: &str,
        _access_kind: AccessKind,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        // ── Keywords that always mean "current class" ──
        if subject == "$this" || subject == "self" || subject == "static" {
            return current_class.cloned().into_iter().collect();
        }

        // ── `parent::` — resolve to the current class's parent ──
        if subject == "parent" {
            if let Some(cc) = current_class
                && let Some(ref parent_name) = cc.parent_class
            {
                // Try local lookup first
                let lookup = parent_name.rsplit('\\').next().unwrap_or(parent_name);
                if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                    return vec![cls.clone()];
                }
                // Fall back to cross-file / PSR-4
                return class_loader(parent_name).into_iter().collect();
            }
            return vec![];
        }

        // ── Enum case / static member access: `ClassName::CaseName` ──
        // When an enum case or static member is used with `->`, resolve to
        // the class/enum itself (e.g. `Status::Active->label()` → `Status`).
        if !subject.starts_with('$')
            && subject.contains("::")
            && !subject.ends_with(')')
            && let Some((class_part, _case_part)) = subject.split_once("::")
        {
            let lookup = class_part.rsplit('\\').next().unwrap_or(class_part);
            if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                return vec![cls.clone()];
            }
            return class_loader(class_part).into_iter().collect();
        }

        // ── Bare class name (for `::` or `->` from `new ClassName()`) ──
        if !subject.starts_with('$')
            && !subject.contains("->")
            && !subject.contains("::")
            && !subject.ends_with(')')
        {
            let lookup = subject.rsplit('\\').next().unwrap_or(subject);
            if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                return vec![cls.clone()];
            }
            // Try cross-file / PSR-4 with the full subject
            return class_loader(subject).into_iter().collect();
        }

        // ── Call expression: subject ends with ")" ──
        // Handles function calls (`app()`, `app(A::class)`),
        // method calls (`$this->getService()`),
        // and static method calls (`ClassName::make()`).
        if subject.ends_with(')')
            && let Some((call_body, args_text)) = split_call_subject(subject)
        {
            let ctx = CallResolutionCtx {
                current_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
            };
            return Self::resolve_call_return_types(call_body, args_text, &ctx);
        }

        // ── Property-chain: $this->prop  or  $this?->prop ──
        if let Some(prop_name) = subject
            .strip_prefix("$this->")
            .or_else(|| subject.strip_prefix("$this?->"))
        {
            if let Some(cc) = current_class {
                let resolved =
                    Self::resolve_property_types(prop_name, cc, all_classes, class_loader);
                if !resolved.is_empty() {
                    return resolved;
                }
            }
            return vec![];
        }

        // ── Property chain on non-`$this` variable: `$var->prop`, `$var->prop->sub` ──
        // When the subject starts with `$`, contains `->` (or `?->`), and
        // does not start with `$this->`, split at the last arrow to get
        // the base expression and the trailing property name, then
        // recursively resolve the base and look up the property type.
        if subject.starts_with('$')
            && !subject.starts_with("$this->")
            && !subject.starts_with("$this?->")
            && !subject.ends_with(')')
            && let Some((base, prop_name)) = split_last_arrow(subject)
        {
            let base_classes = Self::resolve_target_classes(
                base,
                _access_kind,
                current_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
            );
            let mut results = Vec::new();
            for cls in &base_classes {
                let resolved =
                    Self::resolve_property_types(prop_name, cls, all_classes, class_loader);
                ClassInfo::extend_unique(&mut results, resolved);
            }
            if !results.is_empty() {
                return results;
            }
            // If property lookup failed, don't fall through to the
            // bare `$var` branch — the subject is clearly a chain.
            return vec![];
        }

        // ── Chained array access: `$var['key'][]`, `$var['a']['b']` ──
        // When the subject has multiple bracket segments (e.g. from
        // `$response['items'][0]->`), walk through each segment to
        // resolve the final type.  This handles combinations of array
        // shape key lookups and generic element extraction.
        if subject.starts_with('$') && subject.contains('[') {
            let segments = parse_bracket_segments(subject);
            if let Some(ref segs) = segments {
                let resolved = Self::resolve_chained_array_access(
                    &segs.base_var,
                    &segs.segments,
                    content,
                    cursor_offset,
                    current_class,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }

        // ── Variable like `$var` — resolve via assignments / parameter hints ──
        if subject.starts_with('$') {
            // When the cursor is inside a class, use the enclosing class
            // for `self`/`static` resolution in type hints.  When in
            // top-level code (`current_class` is `None`), use a dummy
            // empty class so that assignment scanning still works.
            let dummy_class;
            let effective_class = match current_class {
                Some(cc) => cc,
                None => {
                    dummy_class = ClassInfo::default();
                    &dummy_class
                }
            };
            return Self::resolve_variable_types(
                subject,
                effective_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
            );
        }

        vec![]
    }

    /// Text-based assignment following for raw type extraction.
    ///
    /// Scans backward from `cursor_offset` for `$var = expr;`, then
    /// extracts the raw return type from the RHS expression.  This is
    /// used as a fallback when no `@var` / `@param` annotation is found.
    fn extract_raw_type_from_assignment_text(
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
        let semi_pos = Self::find_semicolon_balanced(remaining)?;
        let rhs_text = remaining[..semi_pos].trim();

        // ── Array literal — `[…]` or `array(…)` ────────────────────
        // Check this BEFORE the function-call case because `array(…)`
        // ends with `)` and would otherwise be mistaken for a call.
        // Also scan for incremental `$var['key'] = expr;` assignments
        // and push-style `$var[] = expr;` assignments.
        let base_entries = super::array_shape::parse_array_literal_entries(rhs_text);

        // Extract spread element types from the array literal (e.g.
        // `[...$users, ...$admins]` → resolve each spread variable's
        // iterable element type via docblock annotation).
        let spread_types = super::array_shape::extract_spread_expressions(rhs_text)
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
        let incremental = super::array_shape::collect_incremental_key_assignments(
            base_var,
            content,
            after_assign,
            cursor_offset,
        );

        // Scan for push-style `$var[] = expr;` assignments.
        let mut push_types = super::array_shape::collect_push_assignments(
            base_var,
            content,
            after_assign,
            cursor_offset,
        );

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
            if let Some(list_type) =
                super::array_shape::build_list_type_from_push_types(&push_types)
            {
                return Some(list_type);
            }
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
                let merged = Self::resolve_class_with_inheritance(owner, class_loader);
                return merged
                    .methods
                    .iter()
                    .find(|m| m.name == method_name)
                    .and_then(|m| m.return_type.clone());
            }

            // Static call: `ClassName::methodName(…)`
            if let Some((class_part, method_part)) = callee.rsplit_once("::") {
                let resolved_class = if class_part == "self" || class_part == "static" {
                    current_class.cloned()
                } else {
                    class_loader(class_part)
                };
                if let Some(cls) = resolved_class {
                    let merged = Self::resolve_class_with_inheritance(&cls, class_loader);
                    return merged
                        .methods
                        .iter()
                        .find(|m| m.name == method_part)
                        .and_then(|m| m.return_type.clone());
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
            let merged = Self::resolve_class_with_inheritance(owner, class_loader);
            return merged
                .properties
                .iter()
                .find(|p| p.name == prop_name)
                .and_then(|p| p.type_hint.clone());
        }

        None
    }

    /// Known array functions whose output preserves the input array's
    /// element type.
    const TEXT_ARRAY_PRESERVING_FUNCS: &'static [&'static str] = &[
        "array_filter",
        "array_values",
        "array_unique",
        "array_reverse",
        "array_slice",
        "array_splice",
        "array_chunk",
        "array_diff",
        "array_intersect",
        "array_merge",
    ];

    /// Known array functions that extract a single element (the element
    /// type is the output type, not wrapped in an array).
    const TEXT_ARRAY_ELEMENT_FUNCS: &'static [&'static str] = &[
        "array_pop",
        "array_shift",
        "current",
        "end",
        "reset",
        "next",
        "prev",
    ];

    /// Text-based resolution for known array functions.
    ///
    /// Given a function name and its argument text, extract the first
    /// variable argument and look up its iterable raw type from docblock
    /// annotations.  For type-preserving functions the raw type is returned
    /// as-is; for element-extracting functions the element type is returned.
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
        let is_preserving = Self::TEXT_ARRAY_PRESERVING_FUNCS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(func_name));
        let is_element = Self::TEXT_ARRAY_ELEMENT_FUNCS
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
            let merged = Self::resolve_class_with_inheritance(owner, class_loader);
            return merged
                .properties
                .iter()
                .find(|p| p.name == prop_name)
                .and_then(|p| p.type_hint.clone());
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
    fn extract_new_expression_class(s: &str) -> Option<String> {
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
    /// don't have a `function_loader` or full `CallResolutionCtx`, only
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
        let merged = Self::resolve_class_with_inheritance(&owner, class_loader);
        merged
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .and_then(|m| m.return_type.clone())
    }

    /// Resolve a text-based LHS expression (the part before `->method`)
    /// to a single `ClassInfo`.
    ///
    /// Handles `$this`, `$this->prop`, `ClassName::method()`,
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
            let lookup = class_name.rsplit('\\').next().unwrap_or(&class_name);
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
                    let merged = Self::resolve_class_with_inheritance(owner, class_loader);
                    return merged
                        .methods
                        .iter()
                        .find(|mi| mi.name == m)
                        .and_then(|mi| mi.return_type.clone());
                }
                // `ClassName::method`
                if let Some((cls_part, m_part)) = inner_callee.rsplit_once("::") {
                    let resolved = if cls_part == "self" || cls_part == "static" {
                        current_class.cloned()
                    } else {
                        let lookup = cls_part.rsplit('\\').next().unwrap_or(cls_part);
                        all_classes
                            .iter()
                            .find(|c| c.name == lookup)
                            .cloned()
                            .or_else(|| class_loader(cls_part))
                    };
                    if let Some(cls) = resolved {
                        let merged = Self::resolve_class_with_inheritance(&cls, class_loader);
                        return merged
                            .methods
                            .iter()
                            .find(|mi| mi.name == m_part)
                            .and_then(|mi| mi.return_type.clone());
                    }
                }
                None
            })?;

            // `ret_type` is a type string — resolve it to ClassInfo.
            let clean = crate::docblock::types::clean_type(&ret_type);
            let lookup = clean.rsplit('\\').next().unwrap_or(&clean);
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
            let merged = Self::resolve_class_with_inheritance(owner, class_loader);
            let type_str = merged
                .properties
                .iter()
                .find(|p| p.name == prop)
                .and_then(|p| p.type_hint.clone())?;
            let clean = crate::docblock::types::clean_type(&type_str);
            let lookup = clean.rsplit('\\').next().unwrap_or(&clean);
            return all_classes
                .iter()
                .find(|c| c.name == lookup)
                .cloned()
                .or_else(|| class_loader(&clean));
        }

        None
    }

    /// Find `;` in `s`, respecting `()`, `[]`, `{}`, and string nesting.
    fn find_semicolon_balanced(s: &str) -> Option<usize> {
        let mut depth_paren = 0i32;
        let mut depth_bracket = 0i32;
        let mut depth_brace = 0i32;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut prev_char = '\0';

        for (i, ch) in s.char_indices() {
            if in_single_quote {
                if ch == '\'' && prev_char != '\\' {
                    in_single_quote = false;
                }
                prev_char = ch;
                continue;
            }
            if in_double_quote {
                if ch == '"' && prev_char != '\\' {
                    in_double_quote = false;
                }
                prev_char = ch;
                continue;
            }
            match ch {
                '\'' => in_single_quote = true,
                '"' => in_double_quote = true,
                '(' => depth_paren += 1,
                ')' => depth_paren -= 1,
                '[' => depth_bracket += 1,
                ']' => depth_bracket -= 1,
                '{' => depth_brace += 1,
                '}' => depth_brace -= 1,
                ';' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                    return Some(i);
                }
                _ => {}
            }
            prev_char = ch;
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

    /// Resolve a call expression to the class of its return type.
    ///
    /// `call_body` is the subject without the trailing `()`, for example:
    ///   - `"app"` for a standalone function call
    ///   - `"$this->getService"` for an instance method call
    ///   - `"ClassName::make"` for a static method call
    ///
    /// The return type string is extracted from the function / method
    /// definition and then resolved to a `ClassInfo` via `class_loader`.
    ///
    /// Returns all candidate classes when the return type is a union
    /// (e.g. `A|B`).
    /// Resolve the element type of an array/list variable accessed with `[]`.
    ///
    /// Given a base variable name like `$admins`, searches backward from
    /// Resolve a chained array access subject like `$var['key'][]`.
    ///
    /// Walks through each bracket segment in order:
    /// - `BracketSegment::StringKey(k)` → extract the value type for key
    ///   `k` from an array shape annotation.
    /// - `BracketSegment::ElementAccess` → extract the generic element
    ///   type (e.g. `list<User>` → `User`).
    ///
    /// Returns the resolved `ClassInfo` for the final type.
    fn resolve_chained_array_access(
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

    pub(super) fn resolve_call_return_types(
        call_body: &str,
        text_args: &str,
        ctx: &CallResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        let current_class = ctx.current_class;
        let all_classes = ctx.all_classes;
        let class_loader = ctx.class_loader;
        let function_loader = ctx.function_loader;
        // ── Instance method call: $this->method / $var->method ──
        if let Some(pos) = call_body.rfind("->") {
            let lhs = &call_body[..pos];
            let method_name = &call_body[pos + 2..];

            // Resolve the left-hand side to a class (recursively handles
            // $this, $var, property chains, nested calls, etc.)
            //
            // IMPORTANT: the `ends_with(')')` check must come before the
            // `$this->` property-chain check, otherwise an LHS like
            // `$this->getFactory()` would be misinterpreted as a property
            // access on `getFactory()` instead of a method call.
            let lhs_classes: Vec<ClassInfo> = if lhs == "$this" || lhs == "self" || lhs == "static"
            {
                current_class.cloned().into_iter().collect()
            } else if let Some(class_name) = Self::extract_new_expression_class(lhs) {
                // Parenthesized (or bare) `new` expression:
                //   `(new Builder())`, `(new Builder)`, `new Builder()`
                // Resolve the class name to a ClassInfo.
                let lookup = class_name.rsplit('\\').next().unwrap_or(&class_name);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(&class_name))
                    .into_iter()
                    .collect()
            } else if lhs.ends_with(')') {
                // LHS is itself a call expression (e.g. `app()` in
                // `app()->make(…)`, or `$this->getFactory()` in
                // `$this->getFactory()->create(…)`).
                // Recursively resolve it.
                if let Some((inner_body, inner_args)) = split_call_subject(lhs) {
                    Self::resolve_call_return_types(inner_body, inner_args, ctx)
                } else {
                    vec![]
                }
            } else if let Some(prop) = lhs
                .strip_prefix("$this->")
                .or_else(|| lhs.strip_prefix("$this?->"))
            {
                current_class
                    .map(|cc| Self::resolve_property_types(prop, cc, all_classes, class_loader))
                    .unwrap_or_default()
            } else if lhs.starts_with('$') {
                // Bare variable like `$profile` — resolve its type via
                // assignment scanning so that chains like
                // `$profile->getUser()->getEmail()` work in both
                // class-method and top-level contexts.
                Self::resolve_target_classes(
                    lhs,
                    AccessKind::Arrow,
                    ctx.current_class,
                    ctx.all_classes,
                    ctx.content,
                    ctx.cursor_offset,
                    ctx.class_loader,
                    ctx.function_loader,
                )
            } else {
                // Unknown LHS form — skip
                vec![]
            };

            let mut results = Vec::new();
            for owner in &lhs_classes {
                // Build template substitution map when the method has
                // method-level @template params and we have arguments.
                let template_subs = if !text_args.is_empty() {
                    Self::build_method_template_subs(
                        owner,
                        method_name,
                        text_args,
                        ctx,
                        class_loader,
                    )
                } else {
                    HashMap::new()
                };
                results.extend(Self::resolve_method_return_types_with_args(
                    owner,
                    method_name,
                    text_args,
                    all_classes,
                    class_loader,
                    &template_subs,
                ));
            }
            return results;
        }

        // ── Static method call: ClassName::method / self::method ──
        if let Some(pos) = call_body.rfind("::") {
            let class_part = &call_body[..pos];
            let method_name = &call_body[pos + 2..];

            let owner_class = if class_part == "self" || class_part == "static" {
                current_class.cloned()
            } else if class_part == "parent" {
                current_class
                    .and_then(|cc| cc.parent_class.as_ref())
                    .and_then(|p| class_loader(p))
            } else {
                // Bare class name
                let lookup = class_part.rsplit('\\').next().unwrap_or(class_part);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(class_part))
            };

            if let Some(ref owner) = owner_class {
                let template_subs = if !text_args.is_empty() {
                    Self::build_method_template_subs(
                        owner,
                        method_name,
                        text_args,
                        ctx,
                        class_loader,
                    )
                } else {
                    HashMap::new()
                };
                return Self::resolve_method_return_types_with_args(
                    owner,
                    method_name,
                    text_args,
                    all_classes,
                    class_loader,
                    &template_subs,
                );
            }
            return vec![];
        }

        // ── Standalone function call: app / myHelper ──
        if let Some(fl) = function_loader
            && let Some(func_info) = fl(call_body)
        {
            // If the function has a conditional return type, try to resolve
            // it using any textual arguments we preserved from the call site
            // (e.g. `app(SessionManager::class)` → text_args = "SessionManager::class").
            if let Some(ref cond) = func_info.conditional_return {
                let resolved_type = if !text_args.is_empty() {
                    resolve_conditional_with_text_args(cond, &func_info.parameters, text_args)
                } else {
                    resolve_conditional_without_args(cond, &func_info.parameters)
                };
                if let Some(ref ty) = resolved_type {
                    let classes = Self::type_hint_to_classes(ty, "", all_classes, class_loader);
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }
            if let Some(ref ret) = func_info.return_type {
                return Self::type_hint_to_classes(ret, "", all_classes, class_loader);
            }
        }

        // ── Variable invocation: $fn() ──────────────────────────────────
        // When the call body is a bare variable (e.g. `$fn`), the variable
        // holds a closure or callable.  Resolve the variable's type
        // annotation and extract the callable return type, or look for a
        // closure/arrow-function literal assignment and extract the native
        // return type hint from the source text.
        if call_body.starts_with('$') {
            let content = ctx.content;
            let cursor_offset = ctx.cursor_offset;

            // 1. Try docblock annotation: `@var Closure(): User $fn` or
            //    `@param callable(int): Response $fn`.
            if let Some(raw_type) = crate::docblock::find_iterable_raw_type_in_source(
                content,
                cursor_offset as usize,
                call_body,
            ) && let Some(ret) = crate::docblock::extract_callable_return_type(&raw_type)
            {
                let classes = Self::type_hint_to_classes(&ret, "", all_classes, class_loader);
                if !classes.is_empty() {
                    return classes;
                }
            }

            // 2. Scan backward for a closure/arrow-function literal
            //    assignment: `$fn = function(): User { … }` or
            //    `$fn = fn(): User => …`.  Extract the native return
            //    type hint from the source text.
            if let Some(ret) =
                Self::extract_closure_return_type_from_assignment(call_body, content, cursor_offset)
            {
                let classes = Self::type_hint_to_classes(&ret, "", all_classes, class_loader);
                if !classes.is_empty() {
                    return classes;
                }
            }
        }

        vec![]
    }

    /// Resolve a method call's return type, taking into account PHPStan
    /// conditional return types when `text_args` is provided, and
    /// method-level `@template` substitutions when `template_subs` is
    /// non-empty.
    ///
    /// This is the workhorse behind both `resolve_method_return_types`
    /// (which passes `""`) and the inline call-chain path (which passes
    /// the raw argument text from the source, e.g. `"CurrentCart::class"`).
    pub(super) fn resolve_method_return_types_with_args(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        template_subs: &HashMap<String, String>,
    ) -> Vec<ClassInfo> {
        // Helper: try to resolve a method's conditional return type, falling
        // back to template-substituted return type, then plain return type.
        let resolve_method = |method: &MethodInfo| -> Vec<ClassInfo> {
            // Try conditional return type first (PHPStan syntax)
            if let Some(ref cond) = method.conditional_return {
                let resolved_type = if !text_args.is_empty() {
                    resolve_conditional_with_text_args(cond, &method.parameters, text_args)
                } else {
                    resolve_conditional_without_args(cond, &method.parameters)
                };
                if let Some(ref ty) = resolved_type {
                    let classes =
                        Self::type_hint_to_classes(ty, &class_info.name, all_classes, class_loader);
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }

            // Try method-level @template substitution on the return type.
            // This handles the general case where the return type references
            // a template param (e.g. `@return Collection<T>`) and we have
            // resolved bindings from the call-site arguments.
            if !template_subs.is_empty()
                && let Some(ref ret) = method.return_type
            {
                let substituted = apply_substitution(ret, template_subs);
                if substituted != *ret {
                    let classes = Self::type_hint_to_classes(
                        &substituted,
                        &class_info.name,
                        all_classes,
                        class_loader,
                    );
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }

            // Fall back to plain return type
            if let Some(ref ret) = method.return_type {
                return Self::type_hint_to_classes(
                    ret,
                    &class_info.name,
                    all_classes,
                    class_loader,
                );
            }
            vec![]
        };

        // First check the class itself
        if let Some(method) = class_info.methods.iter().find(|m| m.name == method_name) {
            return resolve_method(method);
        }

        // Walk up the inheritance chain
        let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
        if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
            return resolve_method(method);
        }

        vec![]
    }

    /// Build a template substitution map for a method-level `@template` call.
    ///
    /// Finds the method on the class (or inherited), checks for template
    /// params and bindings, resolves argument types from `text_args` using
    /// the call resolution context, and returns a `HashMap` mapping template
    /// parameter names to their resolved concrete types.
    ///
    /// Returns an empty map if the method has no template params, no
    /// bindings, or if argument types cannot be resolved.
    pub(super) fn build_method_template_subs(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
        ctx: &CallResolutionCtx<'_>,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> HashMap<String, String> {
        // Find the method — first on the class directly, then via inheritance.
        let method = class_info
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .cloned()
            .or_else(|| {
                let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
                merged.methods.into_iter().find(|m| m.name == method_name)
            });

        let method = match method {
            Some(m) if !m.template_params.is_empty() && !m.template_bindings.is_empty() => m,
            _ => return HashMap::new(),
        };

        let args = split_text_args(text_args);
        let mut subs = HashMap::new();

        for (tpl_name, param_name) in &method.template_bindings {
            // Find the parameter index for this binding.
            let param_idx = match method.parameters.iter().position(|p| p.name == *param_name) {
                Some(idx) => idx,
                None => continue,
            };

            // Get the corresponding argument text.
            let arg_text = match args.get(param_idx) {
                Some(text) => text.trim(),
                None => continue,
            };

            // Try to resolve the argument text to a type name.
            if let Some(type_name) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                subs.insert(tpl_name.clone(), type_name);
            }
        }

        subs
    }

    /// Resolve an argument text string to a type name.
    ///
    /// Handles common patterns:
    /// - `ClassName::class` → `ClassName`
    /// - `new ClassName(…)` → `ClassName`
    /// - `$this` / `self` / `static` → current class name
    /// - `$this->prop` → property type
    /// - `$var` → variable type via assignment scanning
    fn resolve_arg_text_to_type(arg_text: &str, ctx: &CallResolutionCtx<'_>) -> Option<String> {
        let trimmed = arg_text.trim();

        // ClassName::class → ClassName
        if let Some(name) = trimmed.strip_suffix("::class")
            && !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
        {
            return Some(name.strip_prefix('\\').unwrap_or(name).to_string());
        }

        // new ClassName(…) → ClassName
        if let Some(class_name) = Self::extract_new_expression_class(trimmed) {
            return Some(class_name);
        }

        // $this / self / static → current class
        if trimmed == "$this" || trimmed == "self" || trimmed == "static" {
            return ctx.current_class.map(|c| c.name.clone());
        }

        // $this->prop → property type
        if let Some(prop) = trimmed
            .strip_prefix("$this->")
            .or_else(|| trimmed.strip_prefix("$this?->"))
            && prop.chars().all(|c| c.is_alphanumeric() || c == '_')
            && let Some(owner) = ctx.current_class
        {
            let types =
                Self::resolve_property_types(prop, owner, ctx.all_classes, ctx.class_loader);
            if let Some(first) = types.first() {
                return Some(first.name.clone());
            }
        }

        // $var → resolve variable type
        if trimmed.starts_with('$') {
            let classes = Self::resolve_target_classes(
                trimmed,
                crate::types::AccessKind::Arrow,
                ctx.current_class,
                ctx.all_classes,
                ctx.content,
                ctx.cursor_offset,
                ctx.class_loader,
                ctx.function_loader,
            );
            if let Some(first) = classes.first() {
                return Some(first.name.clone());
            }
        }

        None
    }

    /// Look up a property's type hint and resolve all candidate classes.
    ///
    /// When the type hint is a union (e.g. `A|B`), every resolvable part
    /// is returned.
    pub(crate) fn resolve_property_types(
        prop_name: &str,
        class_info: &ClassInfo,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        // Resolve inheritance so that inherited (and generic-substituted)
        // properties are visible.  For example, if `ConfigWrapper extends
        // Wrapper<Config>` and `Wrapper` has `/** @var T */ public $value`,
        // the merged class will have `$value` with type `Config`.
        let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
        let prop = match merged.properties.iter().find(|p| p.name == prop_name) {
            Some(p) => p,
            None => return vec![],
        };
        let type_hint = match prop.type_hint.as_deref() {
            Some(h) => h,
            None => return vec![],
        };
        Self::type_hint_to_classes(type_hint, &class_info.name, all_classes, class_loader)
    }

    /// Map a type-hint string to all matching `ClassInfo` values.
    ///
    /// Handles:
    ///   - Nullable types: `?Foo` → strips `?`, resolves `Foo`
    ///   - Union types: `A|B|C` → resolves each part independently
    ///     (respects `<…>` nesting so `Collection<int|string>` is not split)
    ///   - Intersection types: `A&B` → resolves each part independently
    ///   - Generic types: `Collection<int, User>` → resolves `Collection`,
    ///     then applies generic substitution (`TKey→int`, `TValue→User`)
    ///   - `self` / `static` / `$this` → owning class
    ///   - Scalar/built-in types (`int`, `string`, `bool`, `float`, `array`,
    ///     `void`, `null`, `mixed`, `never`, `object`, `callable`, `iterable`,
    ///     `false`, `true`) → skipped (not class types)
    ///
    /// Each resolvable class-like part is returned as a separate entry.
    pub(crate) fn type_hint_to_classes(
        type_hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        Self::type_hint_to_classes_depth(type_hint, owning_class_name, all_classes, class_loader, 0)
    }

    /// Inner implementation of [`type_hint_to_classes`] with a recursion
    /// depth guard to prevent infinite loops from circular type aliases.
    fn type_hint_to_classes_depth(
        type_hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: u8,
    ) -> Vec<ClassInfo> {
        // Guard against circular / deeply nested type alias resolution.
        const MAX_ALIAS_DEPTH: u8 = 10;
        if depth > MAX_ALIAS_DEPTH {
            return vec![];
        }

        let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);

        // Strip surrounding parentheses that appear in DNF types like `(A&B)|C`.
        let hint = hint
            .strip_prefix('(')
            .and_then(|h| h.strip_suffix(')'))
            .unwrap_or(hint);

        // ── Type alias resolution ──────────────────────────────────────
        // Check if `hint` is a type alias defined on the owning class
        // (via `@phpstan-type` / `@psalm-type` / `@phpstan-import-type`).
        // If so, expand the alias and resolve the underlying definition.
        //
        // This runs before union/intersection splitting because the alias
        // itself may expand to a union or intersection type.
        if let Some(alias_def) =
            Self::resolve_type_alias(hint, owning_class_name, all_classes, class_loader)
        {
            return Self::type_hint_to_classes_depth(
                &alias_def,
                owning_class_name,
                all_classes,
                class_loader,
                depth + 1,
            );
        }

        // ── Union type: split on `|` at depth 0, respecting `<…>` nesting ──
        let union_parts = split_union_depth0(hint);
        if union_parts.len() > 1 {
            let mut results = Vec::new();
            for part in union_parts {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                // Recursively resolve each part (handles self/static, scalars,
                // intersection components, etc.)
                let resolved = Self::type_hint_to_classes_depth(
                    part,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }
            return results;
        }

        // ── Intersection type: split on `&` at depth 0 and resolve each part ──
        // `User&JsonSerializable` means the value satisfies *all* listed
        // types, so completions should include members from every part.
        // Uses depth-aware splitting so that `&` inside `{…}` or `<…>`
        // (e.g. `object{foo: A&B}`) is not treated as a top-level split.
        let intersection_parts = split_intersection_depth0(hint);
        if intersection_parts.len() > 1 {
            let mut results = Vec::new();
            for part in intersection_parts {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let resolved = Self::type_hint_to_classes_depth(
                    part,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }
            return results;
        }

        // ── Object shape: `object{foo: int, bar: string}` ──────────────
        // Synthesise a ClassInfo with public properties from the shape
        // entries so that `$var->foo` resolves through normal property
        // resolution.  Object shape properties are read-only.
        if docblock::types::is_object_shape(hint)
            && let Some(entries) = docblock::parse_object_shape(hint)
        {
            let properties = entries
                .into_iter()
                .map(|e| PropertyInfo {
                    name: e.key,
                    type_hint: Some(e.value_type),
                    is_static: false,
                    visibility: Visibility::Public,
                    is_deprecated: false,
                })
                .collect();

            let synthetic = ClassInfo {
                name: "__object_shape".to_string(),
                properties,
                ..ClassInfo::default()
            };
            return vec![synthetic];
        }

        // self / static / $this always refer to the owning class.
        // In docblocks `@return $this` means "the instance the method is
        // called on" — identical to `static` for inheritance, but when the
        // method comes from a `@mixin` the return type is rewritten to the
        // mixin class name during merge (see `merge_mixins_into_recursive`).
        if hint == "self" || hint == "static" || hint == "$this" {
            return all_classes
                .iter()
                .find(|c| c.name == owning_class_name)
                .cloned()
                .or_else(|| class_loader(owning_class_name))
                .into_iter()
                .collect();
        }

        // ── Parse generic arguments (if any) ──
        // `Collection<int, User>` → base_hint = `Collection`, generic_args = ["int", "User"]
        // `Foo`                   → base_hint = `Foo`,        generic_args = []
        let (base_hint, generic_args) = parse_generic_args(hint);

        // For class lookup, strip any remaining generics from the base
        // (should already be clean, but defensive) and use the short name.
        let base_clean = strip_generics(base_hint.strip_prefix('\\').unwrap_or(base_hint));
        let lookup = base_clean.rsplit('\\').next().unwrap_or(&base_clean);

        // Try local (current-file) lookup by last segment
        let found = all_classes
            .iter()
            .find(|c| c.name == lookup)
            .cloned()
            .or_else(|| class_loader(base_hint));

        match found {
            Some(cls) => {
                // Apply generic substitution if the type hint carried
                // generic arguments and the class has template parameters.
                if !generic_args.is_empty() && !cls.template_params.is_empty() {
                    vec![apply_generic_args(&cls, &generic_args)]
                } else {
                    vec![cls]
                }
            }
            None => vec![],
        }
    }

    /// Look up a type alias by name in the owning class's `type_aliases`.
    ///
    /// Returns the expanded type definition string if `hint` is a known
    /// alias, or `None` if it is not.
    ///
    /// For imported aliases (`from:ClassName:OriginalName`), the source
    /// class is loaded and the original alias is resolved from its
    /// `type_aliases` map.
    fn resolve_type_alias(
        hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        // Only bare identifiers (no `<`, `{`, `|`, `&`, `?`, `\`) can be
        // type aliases.  Skip anything that looks like a complex type
        // expression to avoid false matches.
        if hint.contains('<')
            || hint.contains('{')
            || hint.contains('|')
            || hint.contains('&')
            || hint.contains('?')
            || hint.contains('\\')
            || hint.contains('$')
        {
            return None;
        }

        // Find the owning class to check its type_aliases.
        let owning_class = all_classes.iter().find(|c| c.name == owning_class_name);

        if let Some(cls) = owning_class
            && let Some(def) = cls.type_aliases.get(hint)
        {
            // Handle imported type aliases: `from:ClassName:OriginalName`
            if let Some(import_ref) = def.strip_prefix("from:") {
                return Self::resolve_imported_type_alias(import_ref, all_classes, class_loader);
            }
            return Some(def.clone());
        }

        // Also check all classes in the file — the type alias might be
        // referenced from a method inside a different class that uses the
        // owning class's return type.  This is rare but handles the case
        // where the owning class name is empty (top-level code) or when
        // the type is used in a context where the owning class is not the
        // declaring class.
        for cls in all_classes {
            if cls.name == owning_class_name {
                continue; // Already checked above.
            }
            if let Some(def) = cls.type_aliases.get(hint) {
                if let Some(import_ref) = def.strip_prefix("from:") {
                    return Self::resolve_imported_type_alias(
                        import_ref,
                        all_classes,
                        class_loader,
                    );
                }
                return Some(def.clone());
            }
        }

        None
    }

    /// Resolve an imported type alias reference (`ClassName:OriginalName`).
    ///
    /// Loads the source class and looks up the original alias in its
    /// `type_aliases` map.
    fn resolve_imported_type_alias(
        import_ref: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let (source_class_name, original_name) = import_ref.split_once(':')?;

        // Try to find the source class.
        let lookup = source_class_name
            .rsplit('\\')
            .next()
            .unwrap_or(source_class_name);
        let source_class = all_classes
            .iter()
            .find(|c| c.name == lookup)
            .cloned()
            .or_else(|| class_loader(source_class_name));

        let source_class = source_class?;
        let def = source_class.type_aliases.get(original_name)?;

        // Don't follow nested imports — just return the definition.
        if def.starts_with("from:") {
            return None;
        }

        Some(def.clone())
    }
}
