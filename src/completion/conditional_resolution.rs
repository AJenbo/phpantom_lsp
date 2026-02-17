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
use mago_syntax::ast::*;

use crate::types::{ConditionalReturnType, ParamCondition, ParameterInfo};

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
    conditional: &ConditionalReturnType,
    params: &[ParameterInfo],
    text_args: &str,
) -> Option<String> {
    match conditional {
        ConditionalReturnType::Concrete(ty) => {
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty.clone())
        }
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            // Find which parameter index corresponds to $param_name
            let target = format!("${}", param_name);
            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);

            // Split the textual arguments by comma (at depth 0) and pick
            // the one at `param_idx`.
            let args = split_text_args(text_args);
            let arg_text = args.get(param_idx).map(|s| s.trim());

            match condition {
                ParamCondition::ClassString => {
                    // Check if the argument text matches `X::class`
                    if let Some(arg) = arg_text
                        && let Some(class_name) = extract_class_name_from_text(arg)
                    {
                        return Some(class_name);
                    }
                    // Argument isn't a ::class literal → try else branch
                    resolve_conditional_with_text_args(else_type, params, text_args)
                }
                ParamCondition::IsNull => {
                    if arg_text.is_none() || arg_text == Some("") || arg_text == Some("null") {
                        // No argument provided or explicitly null → null branch
                        resolve_conditional_with_text_args(then_type, params, text_args)
                    } else {
                        // Argument was provided → not null
                        resolve_conditional_with_text_args(else_type, params, text_args)
                    }
                }
                ParamCondition::IsType(_) => {
                    // Can't statically determine; fall through to else.
                    resolve_conditional_with_text_args(else_type, params, text_args)
                }
            }
        }
    }
}

/// Split a textual argument list by commas, respecting nested parentheses
/// so that `"foo(a, b), c"` splits into `["foo(a, b)", "c"]`.
fn split_text_args(text: &str) -> Vec<&str> {
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
        Some(name.strip_prefix('\\').unwrap_or(name).to_string())
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
    conditional: &ConditionalReturnType,
    params: &[ParameterInfo],
    argument_list: &ArgumentList<'b>,
) -> Option<String> {
    match conditional {
        ConditionalReturnType::Concrete(ty) => {
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty.clone())
        }
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            // Find which parameter index corresponds to $param_name
            let target = format!("${}", param_name);
            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);

            // Get the actual argument expression (if provided)
            let arg_expr: Option<&Expression<'b>> = argument_list
                .arguments
                .iter()
                .nth(param_idx)
                .and_then(|arg| match arg {
                    Argument::Positional(pos) => Some(pos.value),
                    Argument::Named(named) => {
                        // Also match named arguments by param name
                        if named.name.value == param_name.as_str() {
                            Some(named.value)
                        } else {
                            None
                        }
                    }
                });

            match condition {
                ParamCondition::ClassString => {
                    // Check if the argument is `X::class`
                    if let Some(class_name) = arg_expr.and_then(extract_class_string_from_expr) {
                        return Some(class_name);
                    }
                    // Argument isn't a ::class literal → try else branch
                    resolve_conditional_with_args(else_type, params, argument_list)
                }
                ParamCondition::IsNull => {
                    if arg_expr.is_none() {
                        // No argument provided → param uses default (null)
                        resolve_conditional_with_args(then_type, params, argument_list)
                    } else {
                        // Argument was provided → not null
                        resolve_conditional_with_args(else_type, params, argument_list)
                    }
                }
                ParamCondition::IsType(_) => {
                    // We can't statically determine the type of an
                    // arbitrary expression; fall through to else.
                    resolve_conditional_with_args(else_type, params, argument_list)
                }
            }
        }
    }
}

/// Resolve a conditional return type **without** call-site arguments
/// (text-based path).  Walks the tree taking the "no argument / null
/// default" branch at each level.
pub(crate) fn resolve_conditional_without_args(
    conditional: &ConditionalReturnType,
    params: &[ParameterInfo],
) -> Option<String> {
    match conditional {
        ConditionalReturnType::Concrete(ty) => {
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty.clone())
        }
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            // Without arguments we check whether the parameter has a
            // null default — if so, the `is null` branch is taken.
            let target = format!("${}", param_name);
            let param = params.iter().find(|p| p.name == target);
            let has_null_default = param.is_some_and(|p| !p.is_required);

            match condition {
                ParamCondition::IsNull if has_null_default => {
                    resolve_conditional_without_args(then_type, params)
                }
                _ => {
                    // Try else branch
                    resolve_conditional_without_args(else_type, params)
                }
            }
        }
    }
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
