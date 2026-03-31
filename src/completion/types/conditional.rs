/// PHPStan conditional return type resolution.
///
/// This module contains the free functions that resolve PHPStan conditional
/// return type annotations to concrete type strings.  These annotations
/// allow a function's return type to depend on the type or value of a
/// parameter at the call site.
///
/// Two resolution paths are supported:
///
/// - **AST-based** ([`resolve_conditional_with_args`]): used when the call
///   is an assignment (`$var = func(…)`) and we have the parsed
///   `ArgumentList` available.
/// - **Text-based** ([`resolve_conditional_with_text_args`]): used when the
///   call appears inline (e.g. `func(A::class)->method()`) and only the
///   raw argument text between parentheses is available.
/// - **No-args** ([`resolve_conditional_without_args`]): used when no
///   arguments were provided (or none were preserved); walks the
///   conditional tree taking the "null default" branch at each level.
use std::collections::HashMap;

use mago_syntax::ast::*;

use crate::php_type::PhpType;
use crate::types::ParameterInfo;

/// Callback that resolves a variable name (e.g. `"$requestType"`) to the
/// class names it holds as class-string values (e.g. from match expression
/// arms like `match (...) { 'a' => A::class, 'b' => B::class }`).
///
/// Returns an empty `Vec` when the variable cannot be resolved or does not
/// hold class-string values.
pub(crate) type VarClassStringResolver<'a> = Option<&'a dyn Fn(&str) -> Vec<String>>;

/// Split a call-expression subject into the call body and any textual
/// arguments.  Handles both `"app()"` → `("app", "")` and
/// `"app(A::class)"` → `("app", "A::class")`.
///
/// For method / static-method calls the arguments are currently not
/// preserved by the extractors, so they always arrive as `""`.
pub(crate) fn split_call_subject(subject: &str) -> Option<(&str, &str)> {
    // Subject must end with ')'.
    let inner = subject.strip_suffix(')')?;
    // Find the matching '(' for the stripped ')' by scanning backwards
    // and tracking balanced parentheses.  This correctly handles nested
    // calls inside the argument list (e.g. `Environment::get(self::country())`).
    let bytes = inner.as_bytes();
    let mut depth: u32 = 0;
    let mut open = None;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
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
    let call_body = &inner[..open];
    let args_text = inner[open + 1..].trim();
    if call_body.is_empty() {
        return None;
    }
    Some((call_body, args_text))
}

/// Resolve a conditional return type using **textual** arguments extracted
/// from the source code (e.g. `"SessionManager::class"`).
///
/// This is used when the call is made inline (not assigned to a variable)
/// and we therefore don't have an AST `ArgumentList` — only the raw text
/// between the parentheses.
pub(crate) fn resolve_conditional_with_text_args(
    conditional: &PhpType,
    params: &[ParameterInfo],
    text_args: &str,
    var_resolver: VarClassStringResolver<'_>,
    calling_class_name: Option<&str>,
) -> Option<String> {
    resolve_conditional_with_text_args_and_defaults(
        conditional,
        params,
        text_args,
        var_resolver,
        calling_class_name,
        None,
    )
}

/// Like [`resolve_conditional_with_text_args`], but also accepts optional
/// template parameter defaults from the owning class.
///
/// When the conditional's subject (e.g. `TAsync`) is not a method parameter
/// but a class-level template parameter with a default value, the default
/// is used to evaluate the condition.
pub fn resolve_conditional_with_text_args_and_defaults(
    conditional: &PhpType,
    params: &[ParameterInfo],
    text_args: &str,
    var_resolver: VarClassStringResolver<'_>,
    calling_class_name: Option<&str>,
    template_defaults: Option<&HashMap<String, String>>,
) -> Option<String> {
    match conditional {
        PhpType::Conditional {
            param,
            negated,
            condition,
            then_type,
            else_type,
        } => {
            // Check if the conditional subject is a template parameter
            // with a default value (not a method $parameter).
            let target = param.as_str();
            if !target.starts_with('$') {
                if let Some(resolved) = try_resolve_with_template_default(
                    target,
                    *negated,
                    condition,
                    then_type,
                    else_type,
                    template_defaults,
                ) {
                    return Some(resolved);
                }
            }

            // Find which parameter index corresponds to $param_name
            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);
            let is_variadic = params
                .get(param_idx)
                .map(|p| p.is_variadic)
                .unwrap_or(false);

            // Split the textual arguments by comma (at depth 0) and pick
            // the one at `param_idx`.
            let args = split_text_args(text_args);
            let arg_text = args.get(param_idx).map(|s| s.trim());

            match condition.as_ref() {
                PhpType::ClassString(_) => {
                    // For variadic class-string parameters, collect class
                    // names from ALL arguments at and after param_idx and
                    // form a union type (e.g. `A|B` from `A::class, B::class`).
                    if is_variadic {
                        let mut class_names: Vec<String> = Vec::new();
                        for arg in args.iter().skip(param_idx) {
                            let trimmed = arg.trim();
                            if let Some(class_name) = extract_class_name_from_text(trimmed) {
                                let class_name = resolve_self_keyword(&class_name, calling_class_name).unwrap_or(class_name);
                                if !class_names.contains(&class_name) {
                                    class_names.push(class_name);
                                }
                            } else if trimmed.starts_with('$')
                                && let Some(resolver) = var_resolver
                            {
                                for name in resolver(trimmed) {
                                    if !class_names.contains(&name) {
                                        class_names.push(name);
                                    }
                                }
                            }
                        }
                        if !class_names.is_empty() {
                            return Some(class_names.join("|"));
                        }
                        return resolve_conditional_with_text_args_and_defaults(
                            else_type,
                            params,
                            text_args,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        );
                    }

                    // Check if the argument text matches `X::class`
                    if let Some(arg) = arg_text
                        && let Some(class_name) = extract_class_name_from_text(arg)
                    {
                        let class_name = resolve_self_keyword(&class_name, calling_class_name).unwrap_or(class_name);
                        return Some(class_name);
                    }
                    // Check if the argument is a variable holding class-string
                    // value(s) (e.g. from a match expression).
                    if let Some(arg) = arg_text
                        && let trimmed = arg.trim()
                        && trimmed.starts_with('$')
                        && let Some(resolver) = var_resolver
                    {
                        let names = resolver(trimmed);
                        if !names.is_empty() {
                            return Some(names.join("|"));
                        }
                    }
                    // Argument isn't a ::class literal or resolvable variable → try else branch
                    resolve_conditional_with_text_args_and_defaults(else_type, params, text_args, var_resolver, calling_class_name, template_defaults)
                }
                PhpType::Named(s) if s == "null" => {
                    if arg_text.is_none() || arg_text == Some("") || arg_text == Some("null") {
                        // No argument provided or explicitly null → null branch
                        resolve_conditional_with_text_args_and_defaults(
                            then_type,
                            params,
                            text_args,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        )
                    } else {
                        // Argument was provided → not null
                        resolve_conditional_with_text_args_and_defaults(
                            else_type,
                            params,
                            text_args,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        )
                    }
                }
                PhpType::Literal(s) => {
                    // Strip quotes from the literal to get the expected value.
                    let expected = crate::util::unquote_php_string(s).unwrap_or(s);

                    // Check if the argument is a quoted string literal
                    // matching the expected value (e.g. `'foo'` or `"foo"`).
                    if let Some(arg) = arg_text {
                        let trimmed = arg.trim();
                        let arg_value = if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
                            || (trimmed.starts_with('"') && trimmed.ends_with('"'))
                        {
                            Some(&trimmed[1..trimmed.len() - 1])
                        } else {
                            None
                        };
                        if arg_value == Some(expected) {
                            return resolve_conditional_with_text_args_and_defaults(
                                then_type,
                                params,
                                text_args,
                                var_resolver,
                                calling_class_name,
                                template_defaults,
                            );
                        }
                    }
                    // Argument doesn't match the literal → else branch.
                    resolve_conditional_with_text_args_and_defaults(else_type, params, text_args, var_resolver, calling_class_name, template_defaults)
                }
                _ => {
                    // IsType equivalent: can't statically determine most
                    // conditions, but we handle `array` specially.
                    // Check if the condition mentions `array` and the
                    // argument is an array literal (starts with `[`).
                    if condition_includes_array(condition.as_ref())
                        && let Some(arg) = arg_text
                        && arg.trim_start().starts_with('[')
                    {
                        return resolve_conditional_with_text_args_and_defaults(
                            then_type,
                            params,
                            text_args,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        );
                    }
                    // Can't statically determine; fall through to else.
                    resolve_conditional_with_text_args_and_defaults(else_type, params, text_args, var_resolver, calling_class_name, template_defaults)
                }
            }
        }
        // Non-Conditional PhpType variant (replaces Concrete)
        other => {
            let ty = other.to_string();
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty)
        }
    }
}

/// Check whether a condition type includes `array` as one of its
/// union members.
///
/// Checks whether the PhpType is `Named("array")`, `Generic("array", _)`,
/// or a `Union` containing any such member.
fn condition_includes_array(condition: &PhpType) -> bool {
    match condition {
        PhpType::Named(s) if s == "array" => true,
        PhpType::Generic(s, _) if s == "array" => true,
        PhpType::Union(members) => members.iter().any(condition_includes_array),
        _ => false,
    }
}

/// Split a textual argument list by commas, respecting nested parentheses
/// so that `"foo(a, b), c"` splits into `["foo(a, b)", "c"]`.
pub(crate) fn split_text_args(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(&text[start..i]);
                start = i + 1; // skip the comma
            }
            _ => {}
        }
    }
    // Push the last segment (or the only one if there were no commas).
    if start <= text.len() {
        let last = &text[start..];
        if !last.trim().is_empty() {
            result.push(last);
        }
    }
    result
}

/// Extract a class name from textual `X::class` syntax.
///
/// Matches strings like `"SessionManager::class"`, `"\\App\\Foo::class"`,
/// returning the class name portion (`"SessionManager"`, `"\\App\\Foo"`).
/// If `name` is `"self"`, `"static"`, or `"parent"`, substitute the
/// calling-site class name so that the resolved type is concrete rather
/// than relative to the method-owner class.
fn resolve_self_keyword(name: &str, calling_class_name: Option<&str>) -> Option<String> {
    match name {
        "self" | "static" | "parent" => calling_class_name.map(|n| n.to_string()),
        _ => None,
    }
}

fn extract_class_name_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let name = trimmed.strip_suffix("::class")?;
    if name.is_empty() {
        return None;
    }
    // Validate that it looks like a class name (identifiers and backslashes).
    if name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
    {
        Some(name.to_string())
    } else {
        None
    }
}

/// Resolve a PHPStan conditional return type given AST-level call-site
/// arguments.
///
/// Walks the conditional tree and matches argument expressions against
/// the conditions:
///   - `class-string<T>`: checks if the positional argument is `X::class`
///     and returns `"X"`.
///   - `is null`: satisfied when no argument is provided (parameter has
///     a null default).
///   - `is SomeType`: not statically resolvable from AST; falls through
///     to the else branch.
pub(crate) fn resolve_conditional_with_args<'b>(
    conditional: &PhpType,
    params: &[ParameterInfo],
    argument_list: &ArgumentList<'b>,
    var_resolver: VarClassStringResolver<'_>,
    calling_class_name: Option<&str>,
) -> Option<String> {
    resolve_conditional_with_args_and_defaults(
        conditional,
        params,
        argument_list,
        var_resolver,
        calling_class_name,
        None,
    )
}

/// Like [`resolve_conditional_with_args`], but also accepts optional
/// template parameter defaults from the owning class.
pub fn resolve_conditional_with_args_and_defaults<'b>(
    conditional: &PhpType,
    params: &[ParameterInfo],
    argument_list: &ArgumentList<'b>,
    var_resolver: VarClassStringResolver<'_>,
    calling_class_name: Option<&str>,
    template_defaults: Option<&HashMap<String, String>>,
) -> Option<String> {
    match conditional {
        PhpType::Conditional {
            param,
            negated,
            condition,
            then_type,
            else_type,
        } => {
            // Check if the conditional subject is a template parameter
            // with a default value (not a method $parameter).
            let target = param.as_str();
            if !target.starts_with('$') {
                if let Some(resolved) = try_resolve_with_template_default(
                    target,
                    *negated,
                    condition,
                    then_type,
                    else_type,
                    template_defaults,
                ) {
                    return Some(resolved);
                }
            }

            // Find which parameter index corresponds to param
            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);

            // Extract param_name without $ prefix for named argument matching
            let param_name_without_dollar = param.strip_prefix('$').unwrap_or(param);

            // Get the actual argument expression (if provided)
            let arg_expr: Option<&Expression<'b>> = argument_list
                .arguments
                .iter()
                .nth(param_idx)
                .and_then(|arg| match arg {
                    Argument::Positional(pos) => Some(pos.value),
                    Argument::Named(named) => {
                        // Also match named arguments by param name
                        if named.name.value == param_name_without_dollar {
                            Some(named.value)
                        } else {
                            None
                        }
                    }
                });

            match condition.as_ref() {
                PhpType::ClassString(_) => {
                    // Check if the argument is `X::class`
                    if let Some(class_name) = arg_expr.and_then(extract_class_string_from_expr) {
                        let class_name = resolve_self_keyword(&class_name, calling_class_name).unwrap_or(class_name);
                        return Some(class_name);
                    }
                    // Check if the argument is a variable holding class-string
                    // value(s) (e.g. from a match expression).
                    if let Some(Expression::Variable(Variable::Direct(dv))) = arg_expr
                        && let Some(resolver) = var_resolver
                    {
                        let names = resolver(dv.name);
                        if !names.is_empty() {
                            return Some(names.join("|"));
                        }
                    }
                    // Argument isn't a ::class literal or resolvable variable → try else branch
                    resolve_conditional_with_args_and_defaults(else_type, params, argument_list, var_resolver, calling_class_name, template_defaults)
                }
                PhpType::Named(s) if s == "null" => {
                    if arg_expr.is_none() {
                        // No argument provided → param uses default (null)
                        resolve_conditional_with_args_and_defaults(
                            then_type,
                            params,
                            argument_list,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        )
                    } else {
                        // Argument was provided → not null
                        resolve_conditional_with_args_and_defaults(
                            else_type,
                            params,
                            argument_list,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        )
                    }
                }
                PhpType::Literal(s) => {
                    // Strip quotes from the literal to get the expected value.
                    let expected = crate::util::unquote_php_string(s).unwrap_or(s);

                    // Check if the argument is a string literal matching
                    // the expected value.
                    let matches = match arg_expr {
                        Some(Expression::Literal(Literal::String(lit_str))) => {
                            // `value` is the unquoted content; fall back
                            // to stripping quotes from `raw`.
                            let inner = lit_str.value.map(|v| v.to_string()).unwrap_or_else(|| {
                                crate::util::unquote_php_string(lit_str.raw)
                                    .unwrap_or(lit_str.raw)
                                    .to_string()
                            });
                            inner == *expected
                        }
                        _ => false,
                    };
                    if matches {
                        resolve_conditional_with_args_and_defaults(
                            then_type,
                            params,
                            argument_list,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        )
                    } else {
                        resolve_conditional_with_args_and_defaults(
                            else_type,
                            params,
                            argument_list,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        )
                    }
                }
                _ => {
                    // IsType equivalent
                    // Check if the condition mentions `array` and the
                    // argument is an array literal (`[...]`).
                    if condition_includes_array(condition.as_ref())
                        && let Some(Expression::Array(_)) = arg_expr
                    {
                        return resolve_conditional_with_args_and_defaults(
                            then_type,
                            params,
                            argument_list,
                            var_resolver,
                            calling_class_name,
                            template_defaults,
                        );
                    }
                    // We can't statically determine the type of an
                    // arbitrary expression; fall through to else.
                    resolve_conditional_with_args_and_defaults(else_type, params, argument_list, var_resolver, calling_class_name, template_defaults)
                }
            }
        }
        // Non-Conditional PhpType variant (replaces Concrete)
        other => {
            let ty = other.to_string();
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty)
        }
    }
}

/// Resolve a conditional return type **without** call-site arguments
/// (text-based path).  Walks the tree taking the "no argument / null
/// default" branch at each level.
pub(crate) fn resolve_conditional_without_args(
    conditional: &PhpType,
    params: &[ParameterInfo],
) -> Option<String> {
    resolve_conditional_without_args_and_defaults(conditional, params, None)
}

/// Like [`resolve_conditional_without_args`], but also accepts optional
/// template parameter defaults from the owning class.
///
/// When the conditional's subject (e.g. `TAsync`) is not a method parameter
/// but a class-level template parameter with a default value, the default
/// is used to evaluate the condition.  For example, given
/// `@template TAsync of bool = false` and a conditional
/// `(TAsync is false ? Response : PromiseInterface)`, this function
/// recognises `TAsync`'s default `false`, matches it against the `false`
/// condition, and returns `Response`.
pub fn resolve_conditional_without_args_and_defaults(
    conditional: &PhpType,
    params: &[ParameterInfo],
    template_defaults: Option<&HashMap<String, String>>,
) -> Option<String> {
    match conditional {
        PhpType::Conditional {
            param,
            negated,
            condition,
            then_type,
            else_type,
        } => {
            // Check if the conditional subject is a template parameter
            // with a default value (not a method $parameter).
            let target = param.as_str();
            if !target.starts_with('$') {
                if let Some(resolved) = try_resolve_with_template_default(
                    target,
                    *negated,
                    condition,
                    then_type,
                    else_type,
                    template_defaults,
                ) {
                    return Some(resolved);
                }
            }

            // Without arguments we check whether the parameter has a
            // null default — if so, the `is null` branch is taken.
            let param_info = params.iter().find(|p| p.name == target);
            let has_null_default = param_info.is_some_and(|p| !p.is_required);

            match condition.as_ref() {
                PhpType::Named(s) if s == "null" && has_null_default => {
                    resolve_conditional_without_args_and_defaults(then_type, params, template_defaults)
                }
                _ => {
                    // Try else branch
                    resolve_conditional_without_args_and_defaults(else_type, params, template_defaults)
                }
            }
        }
        // Non-Conditional PhpType variant (replaces Concrete)
        other => {
            let ty = other.to_string();
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty)
        }
    }
}

/// Try to resolve a conditional type using a template parameter's default value.
///
/// When a conditional references a template parameter (e.g. `TAsync`) rather
/// than a method parameter (e.g. `$param`), and the template parameter has a
/// default value, this function evaluates the condition against the default.
///
/// Handles conditions like:
///   - `TAsync is false` with default `false` → condition matches → then branch
///   - `TAsync is true`  with default `false` → condition doesn't match → else branch
///   - `TAsync is null`  with default `null`  → condition matches → then branch
///
/// Returns `None` when the template has no default or the condition cannot
/// be evaluated, allowing the caller to fall through to normal resolution.
fn try_resolve_with_template_default(
    template_name: &str,
    negated: bool,
    condition: &PhpType,
    then_type: &PhpType,
    else_type: &PhpType,
    template_defaults: Option<&HashMap<String, String>>,
) -> Option<String> {
    let defaults = template_defaults?;
    let default_value = defaults.get(template_name)?;

    // Determine whether the default value matches the condition.
    let condition_matches = match condition {
        // `TAsync is false` — check if the default is literally `false`
        PhpType::Named(s) if s == "false" => default_value == "false",
        // `TAsync is true` — check if the default is literally `true`
        PhpType::Named(s) if s == "true" => default_value == "true",
        // `TAsync is null` — check if the default is literally `null`
        PhpType::Named(s) if s == "null" => default_value == "null",
        // `TAsync is bool` — check if the default is `true` or `false`
        PhpType::Named(s) if s == "bool" => default_value == "true" || default_value == "false",
        // `TAsync is string` — check if the default looks like a string
        PhpType::Named(s) if s == "string" => {
            (default_value.starts_with('\'') && default_value.ends_with('\''))
                || (default_value.starts_with('"') && default_value.ends_with('"'))
        }
        // `TAsync is int` — check if the default is numeric
        PhpType::Named(s) if s == "int" || s == "integer" => default_value.parse::<i64>().is_ok(),
        // Literal string comparison: `TAsync is 'value'`
        PhpType::Literal(s) => {
            let expected = crate::util::unquote_php_string(s).unwrap_or(s);
            default_value == expected
        }
        // Named type comparison (general): check if the default matches the type name
        PhpType::Named(s) => default_value == s.as_str(),
        _ => return None,
    };

    let effective_match = if negated {
        !condition_matches
    } else {
        condition_matches
    };

    let branch = if effective_match { then_type } else { else_type };
    let ty = branch.to_string();
    if ty == "mixed" || ty == "void" || ty == "never" {
        return None;
    }
    Some(ty)
}

/// Extract the class name from an `X::class` expression.
///
/// Matches `Expression::Access(Access::ClassConstant(cca))` where the
/// constant selector is the identifier `class`.
pub(crate) fn extract_class_string_from_expr(expr: &Expression<'_>) -> Option<String> {
    if let Expression::Access(Access::ClassConstant(cca)) = expr
        && let ClassLikeConstantSelector::Identifier(ident) = &cca.constant
        && ident.value == "class"
    {
        // Extract the class name from the LHS
        return match cca.class {
            Expression::Identifier(class_ident) => Some(class_ident.value().to_string()),
            Expression::Self_(_) => Some("self".to_string()),
            Expression::Static(_) => Some("static".to_string()),
            Expression::Parent(_) => Some("parent".to_string()),
            _ => None,
        };
    }
    None
}
