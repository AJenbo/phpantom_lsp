/// Array literal inference, array function helpers, and generator yield
/// reverse-inference.
///
/// These are utility helpers that support the unified resolution pipeline
/// and the foreach/destructuring resolution module.  The raw-type
/// assignment pipeline that previously lived here has been deleted after
/// all callers were migrated to the unified resolver
/// (`resolve_variable_types` / `resolve_variable_types_branch_aware`).
use mago_span::HasSpan;
use mago_syntax::ast::*;

use super::{ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS};

use crate::docblock;
use crate::parser::extract_hint_string;
use crate::php_type::PhpType;
use crate::types::ClassInfo;

use crate::completion::resolver::VarResolutionCtx;

/// Infer the raw PHPStan-style type string for an array literal
/// (`[…]` or `array(…)`) by examining its keys and resolving value
/// elements by resolving each value expression.
pub(in crate::completion) fn infer_array_literal_raw_type<'b>(
    elements: impl Iterator<Item = &'b ArrayElement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    let mut types: Vec<String> = Vec::new();
    let mut has_string_keys = false;
    let mut shape_parts: Vec<String> = Vec::new();

    for elem in elements {
        match elem {
            ArrayElement::KeyValue(kv) => {
                has_string_keys = true;
                // Extract key text.
                let key_text = extract_array_key_text(kv.key);
                // Resolve value type.
                let value_type =
                    infer_element_type(kv.value, ctx).unwrap_or_else(|| "mixed".to_string());
                shape_parts.push(format!("{}: {}", key_text, value_type));
            }
            ArrayElement::Value(v) => {
                if let Some(t) = infer_element_type(v.value, ctx)
                    && !types.contains(&t)
                {
                    types.push(t);
                }
            }
            ArrayElement::Variadic(v) => {
                // Spread: `...$other` — try to resolve iterable element type.
                if let Some(raw) =
                    super::foreach_resolution::resolve_expression_type_string(v.value, ctx)
                    && let Some(elem) = crate::php_type::PhpType::parse(&raw)
                        .extract_value_type(true)
                        .map(|t| t.to_string())
                    && !types.contains(&elem)
                {
                    types.push(elem);
                }
            }
            ArrayElement::Missing(_) => {}
        }
    }

    if has_string_keys && !shape_parts.is_empty() {
        return Some(format!("array{{{}}}", shape_parts.join(", ")));
    }

    if types.is_empty() {
        return None;
    }

    let union = types.join("|");
    Some(format!("list<{}>", union))
}

/// Extract a string representation of an array key expression.
fn extract_array_key_text<'b>(key: &'b Expression<'b>) -> String {
    match key {
        Expression::Literal(Literal::String(s)) => {
            // `value` is the unquoted content; fall back to `raw` trimmed.
            s.value.map(|v| v.to_string()).unwrap_or_else(|| {
                crate::util::unquote_php_string(s.raw)
                    .unwrap_or(s.raw)
                    .to_string()
            })
        }
        Expression::Literal(Literal::Integer(i)) => i.raw.to_string(),
        _ => "mixed".to_string(),
    }
}

/// Infer the type of a single array element value expression.
fn infer_element_type<'b>(value: &'b Expression<'b>, ctx: &VarResolutionCtx<'_>) -> Option<String> {
    match value {
        // ── Scalar literals ──
        Expression::Literal(Literal::String(_)) => Some("string".to_string()),
        Expression::Literal(Literal::Integer(_)) => Some("int".to_string()),
        Expression::Literal(Literal::Float(_)) => Some("float".to_string()),
        Expression::Literal(Literal::True(_) | Literal::False(_)) => Some("bool".to_string()),
        Expression::Literal(Literal::Null(_)) => Some("null".to_string()),
        // ── Nested array literals ──
        Expression::Array(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx)
            .or_else(|| Some("array".to_string())),
        Expression::LegacyArray(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx)
            .or_else(|| Some("array".to_string())),
        // ── Object instantiation ──
        Expression::Instantiation(inst) => match inst.class {
            Expression::Identifier(ident) => Some(ident.value().to_string()),
            Expression::Self_(_) => Some(ctx.current_class.name.clone()),
            Expression::Static(_) => Some(ctx.current_class.name.clone()),
            _ => None,
        },
        Expression::Call(_) => {
            // Resolve call return type via the unified pipeline.
            super::foreach_resolution::resolve_expression_type_string(value, ctx)
        }
        Expression::Variable(Variable::Direct(dv)) => {
            let var_text = dv.name.to_string();
            let offset = value.span().start.offset as usize;
            // Try iterable docblock first (e.g. `@var list<User> $items`).
            if let Some(t) =
                docblock::find_iterable_raw_type_in_source(ctx.content, offset, &var_text)
            {
                return Some(t);
            }
            // Fall back to the full variable type resolution pipeline
            // (parameter type hints, @param docblocks, assignments,
            // foreach bindings, etc.).  This handles cases like
            // `string $trackingUserId` where the variable is a scalar
            // parameter, not an iterable.
            let current_class = ctx
                .all_classes
                .iter()
                .find(|c| c.name == ctx.current_class.name)
                .map(|c| c.as_ref());
            crate::hover::variable_type::resolve_variable_type_string(
                &var_text,
                ctx.content,
                offset as u32,
                current_class,
                ctx.all_classes,
                ctx.class_loader,
                crate::completion::resolver::Loaders::with_function(ctx.function_loader()),
            )
        }
        // ── Parenthesized ──
        Expression::Parenthesized(p) => infer_element_type(p.expression, ctx),
        // ── Property access, method calls on objects, etc. ──
        // Delegate to the unified pipeline which resolves property
        // type hints and method return types through the class
        // hierarchy.
        _ => super::foreach_resolution::resolve_expression_type_string(value, ctx),
    }
}

/// For known array functions, resolve the **raw output type** string
/// (e.g. `"list<User>"`) from the input arguments.
///
/// Used by foreach and destructuring resolution so that iterating over
/// `array_filter(...)` etc. preserves element types.
pub(in crate::completion) fn resolve_array_func_raw_type(
    func_name: &str,
    args: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    // Type-preserving functions: output array has same element type.
    if ARRAY_PRESERVING_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        let arr_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
        // If the raw type already has generic params, return it as-is
        // so downstream `PhpType::extract_value_type` can extract the
        // element type.  Otherwise it's a plain class name and we
        // can't infer element type.
        if crate::php_type::PhpType::parse(&raw)
            .extract_value_type(true)
            .is_some()
        {
            return Some(raw);
        }
    }

    // array_map: callback is first arg, array is second.
    // The callback's return type determines the output element type.
    if func_name.eq_ignore_ascii_case("array_map")
        && let Some(element_type) = extract_array_map_element_type(args, ctx)
    {
        return Some(format!("list<{}>", element_type));
    }

    // iterator_to_array: converts an iterator to an array, preserving
    // the value type.  `iterator_to_array($iter)` where `$iter` is
    // `Iterator<int, Foo>` produces `array<int, Foo>`.
    if func_name.eq_ignore_ascii_case("iterator_to_array") {
        let iter_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(iter_expr, ctx)?;
        if crate::php_type::PhpType::parse(&raw)
            .extract_value_type(true)
            .is_some()
        {
            return Some(raw);
        }
    }

    // Element-extracting functions: wrap element type in list<> so
    // it can be used as an iterable raw type.
    if ARRAY_ELEMENT_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        let arr_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
        if crate::php_type::PhpType::parse(&raw)
            .extract_value_type(true)
            .is_some()
        {
            return Some(raw);
        }
    }

    None
}

/// For known array functions, resolve the **element type** string
/// (e.g. `"User"`) for the output.
///
/// Used by `resolve_rhs_expression` so that `$item = array_pop($users)`
/// resolves `$item` to `User`.  This only covers true element-extracting
/// functions (array_pop, current, etc.) that return a single element.
///
/// Array-producing functions like `array_map` and `iterator_to_array`
/// are handled exclusively by [`resolve_array_func_raw_type`] which
/// preserves the container type (e.g. `list<User>`).  Returning the
/// element type here would lose the array wrapper and break downstream
/// consumers that need to walk bracket segments (e.g. `$result[0]->`).
pub(in crate::completion) fn resolve_array_func_element_type(
    func_name: &str,
    args: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    // Element-extracting functions: return the element type directly.
    if ARRAY_ELEMENT_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        let arr_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
        return crate::php_type::PhpType::parse(&raw)
            .extract_value_type(true)
            .map(|t| t.to_string());
    }

    None
}

/// Extract the raw text of a function/method argument list from source.
///
/// Returns the text between the parentheses (exclusive), trimmed.
/// For example, an argument list `($user, $role)` returns `"$user, $role"`.
pub(in crate::completion) fn extract_argument_text(
    argument_list: &mago_syntax::ast::ArgumentList<'_>,
    content: &str,
) -> String {
    let left = argument_list.left_parenthesis.span().end.offset as usize;
    let right = argument_list.right_parenthesis.span().start.offset as usize;
    if right > left && right <= content.len() {
        content[left..right].trim().to_string()
    } else {
        String::new()
    }
}

/// Extract the output element type for `array_map($callback, $array)`.
///
/// Strategy:
/// 1. If the callback (first arg) is a closure/arrow function with a
///    return type hint, use that.
/// 2. Otherwise, fall back to the **input array's** element type
///    (assumes the callback preserves type, which is a reasonable
///    default when no return type is declared).
fn extract_array_map_element_type(
    args: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    let callback_expr = super::resolution::first_arg_expr(args)?;

    // Try to get the callback's return type hint.
    let return_hint = match callback_expr {
        Expression::Closure(closure) => closure
            .return_type_hint
            .as_ref()
            .map(|rth| extract_hint_string(&rth.hint)),
        Expression::ArrowFunction(arrow) => arrow
            .return_type_hint
            .as_ref()
            .map(|rth| extract_hint_string(&rth.hint)),
        _ => None,
    };

    if let Some(hint) = return_hint {
        let parsed = crate::php_type::PhpType::parse(&hint);
        if let Some(name) = parsed.base_name() {
            return Some(name.to_string());
        }
    }

    // Fallback: use the input array's element type.
    let arr_expr = super::resolution::nth_arg_expr(args, 1)?;
    let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
    crate::php_type::PhpType::parse(&raw)
        .extract_value_type(true)
        .map(|t| t.to_string())
}

/// Reverse-infer a variable's type from `yield $var` statements when
/// the enclosing function declares `@return Generator<TKey, TValue, …>`.
///
/// Scans the source text around the cursor for `yield $varName`
/// patterns within the enclosing function body.  When found, extracts
/// the TValue (2nd generic parameter) from the Generator return type
/// and resolves it to `ClassInfo`.
///
/// This is a fallback used only when normal assignment-based resolution
/// produced no results — the developer is inside a generator body and
/// using a variable that is yielded but was not explicitly typed via
/// an assignment or parameter.
pub(in crate::completion) fn try_infer_from_generator_yield(
    return_type: &PhpType,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    // Only applies to Generator return types.
    let value_type = match return_type.extract_value_type(false) {
        Some(vt) => vt.to_string(),
        None => return vec![],
    };

    // Scan the source text for `yield $varName` or `yield $varName;`
    // within a reasonable window around the cursor.  We look at the
    // enclosing function body (everything between the outermost `{`
    // and `}` that contains the cursor).
    let var_name = ctx.var_name;
    let content = ctx.content;
    let cursor = ctx.cursor_offset as usize;

    // Find the enclosing function body boundaries by scanning backward
    // for the opening `{`.
    let search_before = content.get(..cursor).unwrap_or("");
    let mut brace_depth = 0i32;
    let mut body_start = None;
    for (i, ch) in search_before.char_indices().rev() {
        match ch {
            '}' => brace_depth += 1,
            '{' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    body_start = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    let start = match body_start {
        Some(s) => s,
        None => return vec![],
    };

    // Find the matching closing `}` by scanning forward from the
    // opening brace.
    let after_open = content.get(start..).unwrap_or("");
    let mut depth = 0i32;
    let mut body_end = content.len();
    for (i, ch) in after_open.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    body_end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }

    let body = content.get(start..body_end).unwrap_or("");

    // Look for `yield $varName` (not `yield from` or `yield $key => $varName`).
    // We check for simple patterns:
    //   - `yield $varName;`
    //   - `yield $varName `  (before semicolon or end of expression)
    let yield_pattern = format!("yield {}", var_name);
    let has_yield = body.contains(&yield_pattern);

    // Also check for `yield $key => $varName` pattern — the variable
    // is the value part in a key-value yield.
    let yield_pair_needle = format!("=> {}", var_name);
    let has_yield_pair = body.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.contains("yield ") && trimmed.contains(&yield_pair_needle)
    });

    if !has_yield && !has_yield_pair {
        return vec![];
    }

    crate::completion::type_resolution::type_hint_to_classes(
        &value_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    )
}
