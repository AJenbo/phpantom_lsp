/// Source-text scanning helpers that are **not** part of the deprecated
/// `extract_raw_type_from_assignment_text` pipeline.
///
/// These functions perform lightweight, targeted scans of raw PHP source
/// text for patterns that the AST-based walker cannot (or need not)
/// handle:
///
/// - **`extract_new_expression_class`** — parse `new ClassName(…)` from
///   a text fragment.
/// - **`extract_function_return_from_source`** — find a function's
///   `@return` type by scanning backward for its docblock.
/// - **`extract_closure_return_type_from_assignment`** — find a
///   closure/arrow-function's native return type hint from its
///   assignment.
/// - **`extract_first_class_callable_return_type`** — resolve the
///   return type of a first-class callable assignment like
///   `$fn = strlen(...)` or `$fn = $obj->method(...)`.
/// - **`try_chained_array_access_with_candidates`** /
///   **`walk_array_segments_and_resolve`** — walk bracket segments on
///   candidate `PhpType` values to resolve array access chains.
///
/// All functions in this module are free functions (not methods on
/// `Backend`).  Cross-module dependencies that previously used `Self::`
/// are called via their canonical module paths.
use std::sync::Arc;

use crate::docblock;
use crate::php_type::PhpType;
use crate::types::{BracketSegment, ClassInfo};
use crate::util::{find_semicolon_balanced, short_name};

use crate::completion::resolver::{Loaders, ResolutionCtx};

// ─── Source-text helpers ────────────────────────────────────────────────────

pub(in crate::completion) use crate::subject_expr::parse_new_expression_class as extract_new_expression_class;

/// Search backward in `content` for a function definition matching
/// `func_name` and extract its `@return` type from the docblock.
pub(in crate::completion) fn extract_function_return_from_source(
    func_name: &str,
    content: &str,
) -> Option<String> {
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
/// Handles:
/// - `$fn = function(…): ReturnType { … }`
/// - `$fn = function(…) use (…): ReturnType { … }`
/// - `$fn = fn(…): ReturnType => …`
///
/// Returns `None` if no closure/arrow-function assignment is found
/// or if there is no return type hint.
pub(in crate::completion) fn extract_closure_return_type_from_assignment(
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

/// Extract the return type annotation from a closure or arrow-function
/// literal passed as a call-site argument.
///
/// Unlike [`extract_closure_return_type_from_assignment`], this operates
/// on the raw argument text (e.g. the text between the call's parentheses
/// for one argument), not on a `$var = …` assignment context.
///
/// Handles:
/// - `fn(…): ReturnType => …`
/// - `function(…): ReturnType { … }`
/// - `function(…) use (…): ReturnType { … }`
///
/// Returns `None` if the text is not a closure/arrow-function or if
/// there is no return type hint.
pub(in crate::completion) fn extract_closure_return_type_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();

    let is_arrow = trimmed.starts_with("fn")
        && trimmed
            .get(2..2 + 1)
            .is_some_and(|c| c.starts_with('(') || c.starts_with(' ') || c.starts_with('\t'));
    let is_closure = trimmed.starts_with("function")
        && trimmed
            .get(8..)
            .is_some_and(|rest| rest.trim_start().starts_with('('));

    if !is_arrow && !is_closure {
        return None;
    }

    // Find the opening `(` of the parameter list.
    let paren_open = trimmed.find('(')?;
    // Find the matching `)` by tracking depth.
    let mut depth = 0i32;
    let mut paren_close = None;
    for (i, c) in trimmed[paren_open..].char_indices() {
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
    let after_paren = trimmed.get(paren_close + 1..)?.trim_start();

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

/// Extract the type annotation of the Nth parameter from a closure or
/// arrow-function literal.
///
/// Given `fn(User $u, int $count): void => ...` and `position = 0`,
/// returns `Some("User")`.  Given `position = 1`, returns `Some("int")`.
///
/// This is the contravariant counterpart of
/// [`extract_closure_return_type_from_text`]: when a docblock declares
/// `@param Closure(T): void $cb`, the template param `T` appears in the
/// callable's *parameter* list rather than its return type, so we need to
/// read the closure argument's parameter type hints to infer `T`.
///
/// Returns `None` if the text is not a closure/arrow-function, the
/// parameter at `position` does not exist, or the parameter has no type
/// hint.
pub(in crate::completion) fn extract_closure_param_type_from_text(
    text: &str,
    position: usize,
) -> Option<String> {
    let trimmed = text.trim();

    let is_arrow = trimmed.starts_with("fn")
        && trimmed
            .get(2..2 + 1)
            .is_some_and(|c| c.starts_with('(') || c.starts_with(' ') || c.starts_with('\t'));
    let is_closure = trimmed.starts_with("function")
        && trimmed
            .get(8..)
            .is_some_and(|rest| rest.trim_start().starts_with('('));

    if !is_arrow && !is_closure {
        return None;
    }

    // Find the opening `(` of the parameter list.
    let paren_open = trimmed.find('(')?;
    // Find the matching `)` by tracking depth.
    let mut depth = 0i32;
    let mut paren_close = None;
    for (i, c) in trimmed[paren_open..].char_indices() {
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

    // Extract the parameter list text between the parens.
    let params_text = trimmed.get(paren_open + 1..paren_close)?.trim();
    if params_text.is_empty() {
        return None;
    }

    // Split by commas at depth 0 (respecting nested parens/generics).
    let params = split_params_at_depth_zero(params_text);
    let param = params.get(position)?;
    let param = param.trim();
    if param.is_empty() {
        return None;
    }

    // A typed parameter looks like `TypeHint $name` or `?TypeHint $name`
    // or `TypeHint &$name` or `TypeHint ...$name`.
    // An untyped parameter is just `$name` or `&$name` or `...$name`.
    // We need to find the type hint, which is everything before the `$`
    // (or `&$` or `...$`).

    // Find the `$` that starts the variable name.
    let dollar = param.rfind('$')?;
    if dollar == 0 {
        // No type hint — the parameter is untyped.
        return None;
    }

    let before_dollar = param[..dollar].trim_end();
    // Strip trailing `&` or `...` (pass-by-reference or variadic).
    let before_dollar = before_dollar
        .strip_suffix("...")
        .or_else(|| before_dollar.strip_suffix('&'))
        .unwrap_or(before_dollar)
        .trim_end();

    if before_dollar.is_empty() {
        return None;
    }

    Some(before_dollar.to_string())
}

/// Split a parameter list string by commas at depth zero, respecting
/// nested parentheses and angle brackets.
fn split_params_at_depth_zero(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' | '<' | '[' => depth += 1,
            ')' | '>' | ']' => depth -= 1,
            ',' if depth == 0 => {
                result.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&text[start..]);
    result
}

/// Resolve the return type of a first-class callable assigned to
/// `var_name`.
///
/// Scans backward for `$var_name = callable_expr(...)` and resolves
/// the underlying function or method's return type.  Handles:
///
/// - `$fn = strlen(...)` (standalone function)
/// - `$fn = $this->method(...)` (instance method)
/// - `$fn = $obj->method(...)` (instance method on resolved variable)
/// - `$fn = ClassName::method(...)` (static method)
/// - `$fn = self::method(...)` / `static::method(...)`
///
/// Returns `None` if no first-class callable assignment is found or
/// the return type cannot be determined.
pub(in crate::completion) fn extract_first_class_callable_return_type(
    var_name: &str,
    rctx: &ResolutionCtx<'_>,
) -> Option<String> {
    let content = rctx.content;
    let cursor_offset = rctx.cursor_offset;
    let current_class = rctx.current_class;
    let all_classes = rctx.all_classes;
    let class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = rctx.class_loader;
    let function_loader = rctx.function_loader;
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
            // Resolve the variable's type via the unified pipeline.
            let default_class = ClassInfo::default();
            let effective_class = current_class.unwrap_or(&default_class);
            let resolved = crate::completion::variable::resolution::resolve_variable_types(
                lhs,
                effective_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                Loaders::with_function(function_loader),
            );
            crate::types::ResolvedType::into_classes(resolved)
                .into_iter()
                .next()
        } else {
            // Non-variable LHS (e.g. chained call) — delegate to
            // the general-purpose text resolver.
            resolve_lhs_to_class(lhs, current_class, all_classes, class_loader)
        };

        if let Some(cls) = owner {
            return crate::inheritance::resolve_method_return_type(&cls, method_name, class_loader)
                .map(|ret| ret.replace_self(&cls.name).to_string());
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
                .and_then(|p| class_loader(p).map(Arc::unwrap_or_clone))
        } else {
            let lookup = short_name(class_part);
            all_classes
                .iter()
                .find(|c| c.name == lookup)
                .map(|c| ClassInfo::clone(c))
                .or_else(|| class_loader(class_part).map(Arc::unwrap_or_clone))
        };

        if let Some(cls) = owner {
            return crate::inheritance::resolve_method_return_type(&cls, method_name, class_loader)
                .map(|ret| ret.replace_self(&cls.name).to_string());
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
        return func_info.return_type_str();
    }

    None
}

/// Resolve a chained array access, trying each candidate raw type
/// in order until one succeeds through the full segment walk.
///
/// Each candidate `PhpType` is fed through
/// `walk_array_segments_and_resolve`.  The first that resolves
/// through the segment walk and, if it produces a non-empty
/// `ClassInfo` set, returned immediately.  Returns `None` when no
/// candidate succeeds.
pub(in crate::completion) fn try_chained_array_access_with_candidates<'a>(
    candidates: impl Iterator<Item = PhpType> + 'a,
    segments: &[BracketSegment],
    current_class: Option<&ClassInfo>,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<Vec<ClassInfo>> {
    let current_class_name = current_class.map(|c| c.name.as_str()).unwrap_or("");

    for candidate in candidates {
        if let Some(result) = walk_array_segments_and_resolve(
            &candidate,
            segments,
            current_class_name,
            all_classes,
            class_loader,
        ) {
            return Some(result);
        }
    }

    None
}

/// Walk bracket segments on a `PhpType`, then resolve the resulting
/// type to `ClassInfo`.
///
/// Returns `Some(classes)` when the full segment chain resolves
/// successfully, or `None` when a segment cannot be applied (e.g.
/// the array shape does not contain the requested key).
fn walk_array_segments_and_resolve(
    base_type: &PhpType,
    segments: &[BracketSegment],
    current_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<Vec<ClassInfo>> {
    // Expand type aliases before walking segments.  The raw type may
    // be an alias name like `UserData` that resolves to
    // `array{name: string, pen: Pen}`.  Without expansion the
    // segment walk would fail to extract shape values.
    let mut current = if let PhpType::Named(_) = base_type {
        let name_str = base_type.to_string();
        if let Some(expanded) = crate::completion::type_resolution::resolve_type_alias(
            &name_str,
            current_class_name,
            all_classes,
            class_loader,
        ) {
            expanded
        } else {
            base_type.clone()
        }
    } else {
        base_type.clone()
    };

    for seg in segments {
        // Try pure-type extraction first (array shapes, generics).
        let extracted = match seg {
            BracketSegment::StringKey(key) => current
                .shape_value_type(key)
                .or_else(|| current.extract_value_type(true))
                .cloned(),
            BracketSegment::ElementAccess => current.extract_value_type(true).cloned(),
        };

        current = if let Some(t) = extracted {
            t
        } else {
            // Fallback: when the current type is a plain class name (e.g.
            // `Application`, `OpeningHours`), resolve the class and check
            // its iterable generics (`@extends`, `@implements`) for the
            // element type.  This handles bracket access on classes that
            // implement `ArrayAccess` with generic type parameters.
            let type_str = current.to_string();
            let class_element = crate::completion::type_resolution::type_hint_to_classes(
                &type_str,
                current_class_name,
                all_classes,
                class_loader,
            )
            .into_iter()
            .find_map(|cls| {
                let merged =
                    crate::virtual_members::resolve_class_fully(&cls, class_loader);
                crate::completion::variable::foreach_resolution::extract_iterable_element_type_from_class(
                    &merged,
                    class_loader,
                )
            });

            if let Some(element) = class_element {
                PhpType::parse(&element)
            } else {
                return None;
            }
        };

        // After each segment, the resulting type might itself be an
        // alias (e.g. a shape value defined as another alias).
        // Convert to string only for alias resolution.
        let type_str = current.to_string();
        if let Some(expanded) = crate::completion::type_resolution::resolve_type_alias(
            &type_str,
            current_class_name,
            all_classes,
            class_loader,
        ) {
            current = expanded;
        }
    }

    // Check whether the type has any class-like (non-scalar) component
    // worth resolving.
    if current.is_scalar() {
        return None;
    }

    let classes = crate::completion::type_resolution::type_hint_to_classes_typed(
        &current,
        current_class_name,
        all_classes,
        class_loader,
    );
    if classes.is_empty() {
        return None;
    }
    Some(classes)
}

// ── Internal helpers for `extract_first_class_callable_return_type` ──

/// Resolve the return type of a chained call expression from text.
///
/// Splits at the rightmost `->`, resolves the LHS to a `ClassInfo`,
/// then looks up the method's return type.  Used by
/// `extract_first_class_callable_return_type` for chained-call LHS
/// patterns.
fn resolve_raw_type_from_call_chain(
    callee: &str,
    _args_text: &str,
    current_class: Option<&ClassInfo>,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    // Split at the rightmost `->` to get the final method name and
    // the LHS expression that produces the owning object.
    let pos = callee.rfind("->")?;
    // Strip trailing `?` from LHS when the operator was `?->`
    let lhs = callee[..pos]
        .strip_suffix('?')
        .unwrap_or(&callee[..pos])
        .trim();
    let method_name = callee[pos + 2..].trim();

    // Resolve LHS to a class.
    let owner = resolve_lhs_to_class(lhs, current_class, all_classes, class_loader)?;
    crate::inheritance::resolve_method_return_type(&owner, method_name, class_loader)
        .map(|ret| ret.to_string())
}

/// Resolve the left-hand side of a chained expression to a `ClassInfo`.
///
/// Handles `$this` / `self` / `static`, `$this->prop`, `new Foo()`,
/// `(new Foo())`, and recursive chains.  Used by
/// `resolve_raw_type_from_call_chain` for the text-only path.
fn resolve_lhs_to_class(
    lhs: &str,
    current_class: Option<&ClassInfo>,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<ClassInfo> {
    // Trim whitespace so that multi-line call chains (where
    // `rfind("->")` leaves trailing newlines/spaces on the LHS)
    // are handled correctly by all downstream checks.
    let lhs = lhs.trim();

    // `$this` / `self` / `static`
    if lhs == "$this" || lhs == "self" || lhs == "static" {
        return current_class.cloned();
    }

    // `(new ClassName(...))` or `new ClassName(...)`
    if let Some(class_name) = extract_new_expression_class(lhs) {
        let lookup = short_name(&class_name);
        return all_classes
            .iter()
            .find(|c| c.name == lookup)
            .map(|c| ClassInfo::clone(c))
            .or_else(|| class_loader(&class_name).map(Arc::unwrap_or_clone));
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
        let ret_type = resolve_raw_type_from_call_chain(
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
                return crate::inheritance::resolve_method_return_type(owner, m, class_loader)
                    .map(|ret| ret.to_string());
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
                        .map(|c| ClassInfo::clone(c))
                        .or_else(|| class_loader(cls_part).map(Arc::unwrap_or_clone))
                };
                if let Some(cls) = resolved {
                    return crate::inheritance::resolve_method_return_type(
                        &cls,
                        m_part,
                        class_loader,
                    )
                    .map(|ret| ret.to_string());
                }
            }
            None
        })?;

        // `ret_type` is a type string — resolve it to ClassInfo.
        let parsed = PhpType::parse(&ret_type);
        let effective = parsed.non_null_type().unwrap_or_else(|| parsed.clone());
        if let Some(base) = effective.base_name() {
            let lookup = short_name(base);
            return all_classes
                .iter()
                .find(|c| c.name == lookup)
                .map(|c| ClassInfo::clone(c))
                .or_else(|| class_loader(base).map(Arc::unwrap_or_clone));
        }
    }

    // `$this->prop` — property access
    if let Some(prop) = lhs
        .strip_prefix("$this->")
        .or_else(|| lhs.strip_prefix("$this?->"))
        && prop.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        let owner = current_class?;
        let parsed = crate::inheritance::resolve_property_type_hint(owner, prop, class_loader)?;
        let effective = parsed.non_null_type().unwrap_or_else(|| parsed.clone());
        if let Some(base) = effective.base_name() {
            let lookup = short_name(base);
            return all_classes
                .iter()
                .find(|c| c.name == lookup)
                .map(|c| ClassInfo::clone(c))
                .or_else(|| class_loader(base).map(Arc::unwrap_or_clone));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_fn_with_return_type() {
        let text = "fn(Decimal $carry, Orderproduct $p): Decimal => $carry->add($p->price)";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some("Decimal".to_string())
        );
    }

    #[test]
    fn arrow_fn_without_return_type() {
        let text = "fn($carry, $p) => $carry + $p";
        assert_eq!(extract_closure_return_type_from_text(text), None);
    }

    #[test]
    fn closure_with_return_type() {
        let text = "function(Money $carry, LineItem $item): Money { return $carry; }";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some("Money".to_string())
        );
    }

    #[test]
    fn closure_with_use_and_return_type() {
        let text = "function(int $carry) use ($factor): Result { return new Result(); }";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some("Result".to_string())
        );
    }

    #[test]
    fn closure_without_return_type() {
        let text = "function($carry, $item) { return $carry; }";
        assert_eq!(extract_closure_return_type_from_text(text), None);
    }

    #[test]
    fn not_a_closure() {
        let text = "new Decimal('0')";
        assert_eq!(extract_closure_return_type_from_text(text), None);
    }

    #[test]
    fn arrow_fn_fqn_return_type() {
        let text = r"fn(\App\Models\User $u): \App\Models\User => $u";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some(r"\App\Models\User".to_string())
        );
    }

    #[test]
    fn arrow_fn_nullable_return_type() {
        let text = "fn(int $x): ?string => null";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some("?string".to_string())
        );
    }

    #[test]
    fn closure_with_nested_parens_in_params() {
        let text = "function(array $items = []): Collection { return new Collection(); }";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some("Collection".to_string())
        );
    }

    #[test]
    fn variable_is_not_a_closure() {
        let text = "$someVar";
        assert_eq!(extract_closure_return_type_from_text(text), None);
    }

    #[test]
    fn whitespace_around_text() {
        let text = "  fn(int $x): string => ''  ";
        assert_eq!(
            extract_closure_return_type_from_text(text),
            Some("string".to_string())
        );
    }

    // ── extract_closure_param_type_from_text tests ──────────────

    #[test]
    fn param_type_arrow_fn_first_param() {
        let text = "fn(User $u, int $count): void => doSomething($u)";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("User".to_string())
        );
    }

    #[test]
    fn param_type_arrow_fn_second_param() {
        let text = "fn(User $u, int $count): void => doSomething($u)";
        assert_eq!(
            extract_closure_param_type_from_text(text, 1),
            Some("int".to_string())
        );
    }

    #[test]
    fn param_type_closure_first_param() {
        let text = "function(Order $order): void { $order->process(); }";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("Order".to_string())
        );
    }

    #[test]
    fn param_type_untyped_param() {
        let text = "fn($item) => $item->process()";
        assert_eq!(extract_closure_param_type_from_text(text, 0), None);
    }

    #[test]
    fn param_type_out_of_bounds() {
        let text = "fn(User $u): void => doSomething($u)";
        assert_eq!(extract_closure_param_type_from_text(text, 5), None);
    }

    #[test]
    fn param_type_nullable() {
        let text = "fn(?string $name): void => trim($name)";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("?string".to_string())
        );
    }

    #[test]
    fn param_type_fqn() {
        let text = r"fn(\App\Models\User $u): void => $u->save()";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some(r"\App\Models\User".to_string())
        );
    }

    #[test]
    fn param_type_by_reference() {
        let text = "fn(int &$count): void => $count++";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("int".to_string())
        );
    }

    #[test]
    fn param_type_variadic() {
        let text = "fn(string ...$items): void => implode($items)";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("string".to_string())
        );
    }

    #[test]
    fn param_type_not_a_closure() {
        let text = "new Decimal('0')";
        assert_eq!(extract_closure_param_type_from_text(text, 0), None);
    }

    #[test]
    fn param_type_empty_params() {
        let text = "fn(): void => null";
        assert_eq!(extract_closure_param_type_from_text(text, 0), None);
    }

    #[test]
    fn param_type_closure_with_use_clause() {
        let text = "function(Product $p) use ($factor): void { $p->scale($factor); }";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("Product".to_string())
        );
    }

    #[test]
    fn param_type_whitespace_around() {
        let text = "  fn( User $u ): void => $u->save()  ";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("User".to_string())
        );
    }

    #[test]
    fn param_type_variable_is_not_a_closure() {
        let text = "$someVar";
        assert_eq!(extract_closure_param_type_from_text(text, 0), None);
    }

    #[test]
    fn param_type_mixed_typed_and_untyped() {
        let text = "fn(User $u, $count, string $label): void => null";
        assert_eq!(
            extract_closure_param_type_from_text(text, 0),
            Some("User".to_string())
        );
        assert_eq!(extract_closure_param_type_from_text(text, 1), None);
        assert_eq!(
            extract_closure_param_type_from_text(text, 2),
            Some("string".to_string())
        );
    }
}
