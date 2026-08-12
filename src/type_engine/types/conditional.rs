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
///   conditional tree deciding each condition against the parameter's
///   declared default.
///
/// All three decide an omitted argument by that declared default, since that
/// is the value the parameter takes at runtime.
use crate::atom::atom;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use mago_syntax::cst::*;

use crate::php_type::{LiteralValue, PhpType, TypeKind};
use crate::types::{ClassInfo, ParameterInfo};

/// Groups template-related context for conditional return type resolution.
///
/// This bundles the class-level template defaults and the method/function-level
/// template parameter names into a single value, keeping function signatures
/// under clippy's 7-argument limit.
pub struct TemplateContext<'a> {
    /// Class-level template parameter defaults (e.g. from `@template TAsync = false`).
    pub defaults: Option<&'a HashMap<String, PhpType>>,
    /// Method/function-level `@template` parameter names.
    /// Used to distinguish template parameters (e.g. `T`) from concrete class
    /// names (e.g. `FormFlowTypeInterface`) in `class-string<Bound>` conditions.
    pub params: &'a [crate::atom::Atom],
    /// Optional resolver mapping an argument's source text to its resolved
    /// [`PhpType`].
    ///
    /// Used to evaluate `is <Type>` conditions (e.g. `$subject is string`)
    /// when the argument is not a syntactic literal but an expression whose
    /// type can still be resolved (a method-call chain like `$obj->toHtml()`,
    /// a property access, a variable, etc.). Without it, such arguments fall
    /// through to the else branch even when their real type would satisfy the
    /// condition.
    pub arg_type_resolver: ArgTypeResolver<'a>,
}

/// Callback that resolves an argument's source text (e.g. `"$obj->toHtml()"`)
/// to its [`PhpType`], or `None` when the type cannot be determined.
pub type ArgTypeResolver<'a> = Option<&'a dyn Fn(&str) -> Option<PhpType>>;

impl<'a> TemplateContext<'a> {
    pub fn with_params(params: &'a [crate::atom::Atom]) -> Self {
        Self {
            defaults: None,
            params,
            arg_type_resolver: None,
        }
    }
}

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
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    tpl: &TemplateContext<'_>,
) -> Option<PhpType> {
    resolve_conditional_with_text_args_and_defaults(
        conditional,
        params,
        text_args,
        var_resolver,
        calling_class_name,
        class_loader,
        tpl,
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
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    tpl: &TemplateContext<'_>,
) -> Option<PhpType> {
    match conditional.kind() {
        TypeKind::Conditional(cond) => {
            let (param, negated, condition, then_type, else_type) = (
                &cond.param,
                &cond.negated,
                &cond.condition,
                &cond.then_type,
                &cond.else_type,
            );
            // Check if the conditional subject is a template parameter
            // with a default value (not a method $parameter).
            let target = param.as_str();
            if !target.starts_with('$')
                && let Some(resolved) = try_resolve_with_template_default(
                    target,
                    *negated,
                    condition,
                    then_type,
                    else_type,
                    tpl.defaults,
                )
            {
                return Some(resolved);
            }

            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);
            let is_variadic = params
                .get(param_idx)
                .map(|p| p.is_variadic)
                .unwrap_or(false);

            // Split the textual arguments by comma (at depth 0), then bind
            // them to parameters by PHP's rules so a named argument resolves
            // to the parameter it targets rather than its ordinal slot.
            let args = split_text_args(text_args);
            let bound_text = crate::call_args::bind_text_args_to_params(params, &args);
            let arg_text_owned = bound_text.get(param_idx).cloned().flatten();
            // An omitted argument takes the parameter's declared default at
            // runtime, so the condition is decided against that default
            // rather than being left undecidable.
            let arg_text = arg_text_owned
                .as_deref()
                .or_else(|| param_default_text(params.get(param_idx)));

            if matches!(condition.kind(), TypeKind::ClassString(_)) {
                // Extract the bound type from `class-string<Bound>`, if any.
                // When a bound is present AND resolves to a real class (not
                // a template parameter like `T`), the conditional checks
                // whether the argument class is a subtype of the bound
                // (e.g. `$type is class-string<FormFlowTypeInterface>`).
                //
                // When the bound is a template parameter (e.g. `T` from
                // `@template T of object`), any `::class` literal satisfies
                // the condition and the template param is substituted with
                // the concrete class name — this is the existing behavior.
                //
                // We distinguish the two by checking whether the bound name
                // is one of the method/function's own `@template` parameter
                // names (`tpl.params`), not by resolving it as a class:
                // `class_loader` resolution below determines whether a
                // *concrete* bound exists in the project, but a bare
                // template name like `T` would never appear there anyway,
                // so membership in `tpl.params` is the only reliable signal
                // for which case we're in.
                let class_string_bound_name: Option<&str> = match condition.kind() {
                    TypeKind::ClassString(Some(inner)) => match inner.kind() {
                        TypeKind::Named(name) => Some(name.as_str()),
                        _ => None,
                    },
                    _ => None,
                };

                let bound_is_template = class_string_bound_name
                    .is_some_and(|name| tpl.params.iter().any(|tp| tp.as_str() == name));

                // For concrete bounds, try to resolve the bound class.
                // `None` = no bound or template param (permissive)
                // `Some(resolved)` = concrete class (strict subtype check)
                // `Some(sentinel)` = unresolvable concrete name (always
                //   fails the subtype check, forcing the else branch)
                let concrete_class_string_bound: Option<String> = if bound_is_template {
                    None
                } else {
                    class_string_bound_name.map(|name| {
                        let resolved = crate::util::resolve_name_via_loader(name, class_loader);
                        if class_loader(&resolved).is_some() {
                            resolved
                        } else {
                            // Concrete class that can't be resolved
                            // (cross-file name resolution failure).
                            // Use a sentinel that will never match,
                            // forcing the else branch.  This is the
                            // safe default: we can't verify the
                            // subtype relationship, so we fall back
                            // to the broader return type.
                            "!!unresolvable_bound!!".to_string()
                        }
                    })
                };

                // Helper: check whether a resolved class name satisfies the
                // concrete class-string bound.  Returns `true` when there is
                // no concrete bound (bare `class-string` or template param
                // bound — any class satisfies it) or the class is a subtype
                // of the bound.
                let satisfies_bound = |resolved_name: &str| -> bool {
                    match concrete_class_string_bound {
                        None => true,
                        Some(ref bound) => crate::class_lookup::is_subtype_of_names(
                            resolved_name,
                            bound,
                            class_loader,
                        ),
                    }
                };

                // Helper: choose the correct branch based on whether the
                // bound is satisfied and the `negated` flag.
                let choose_branch = |bound_satisfied: bool| -> Option<PhpType> {
                    let take_then = bound_satisfied ^ *negated;
                    resolve_conditional_with_text_args_and_defaults(
                        if take_then { then_type } else { else_type },
                        params,
                        text_args,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    )
                };

                // Helper: when the class-string bound is itself a template
                // parameter (not a concrete class to subtype-check), the
                // condition is definitionally satisfied and `then_type`
                // must have the template substituted with the resolved
                // class(es) rather than being discarded wholesale — this
                // preserves surrounding structure like `T&MockInterface`
                // instead of collapsing it to bare `T`.
                let substitute_bound = |resolved_ty: PhpType| -> PhpType {
                    match class_string_bound_name {
                        Some(bound_name) if bound_is_template => {
                            let subs = std::collections::HashMap::from([(
                                bound_name.to_string(),
                                resolved_ty,
                            )]);
                            then_type.substitute(&subs)
                        }
                        _ => resolved_ty,
                    }
                };

                // For variadic class-string parameters, collect class
                // names from ALL arguments at and after param_idx and
                // form a union type (e.g. `A|B` from `A::class, B::class`).
                if is_variadic {
                    let mut class_names: Vec<String> = Vec::new();
                    for arg in args.iter().skip(param_idx) {
                        let trimmed = arg.trim();
                        if let Some(class_name) = extract_class_name_from_text(trimmed) {
                            let class_name = resolve_self_keyword(&class_name, calling_class_name)
                                .unwrap_or(class_name);
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
                        let class_names: Vec<String> = class_names
                            .into_iter()
                            .map(|n| crate::util::resolve_name_via_loader(&n, class_loader))
                            .collect();

                        // When a bound exists, check all collected classes.
                        // If any fails the bound check, fall through to the
                        // else branch rather than returning a wrong type.
                        let all_satisfy = class_names.iter().all(|n| satisfies_bound(n));
                        if !all_satisfy {
                            return choose_branch(false);
                        }

                        let ty = if class_names.len() == 1 {
                            PhpType::named(atom(&class_names.into_iter().next().unwrap()))
                        } else {
                            PhpType::union(
                                class_names
                                    .into_iter()
                                    .map(|n| PhpType::named(atom(n.as_ref())))
                                    .collect(),
                            )
                        };
                        return Some(substitute_bound(ty));
                    }
                    return resolve_conditional_with_text_args_and_defaults(
                        else_type,
                        params,
                        text_args,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    );
                }

                // Check if the argument text matches `X::class`.  An omitted
                // argument already reads back as its `Foo::class` default, so
                // `app()` resolves the same as `app(Foo::class)`.
                let class_name = arg_text.and_then(extract_class_name_from_text).or_else(|| {
                    arg_text.and_then(|text| class_named_by_quoted_string(text, class_loader))
                });
                if let Some(class_name) = class_name {
                    let class_name =
                        resolve_self_keyword(&class_name, calling_class_name).unwrap_or(class_name);
                    let resolved = crate::util::resolve_name_via_loader(&class_name, class_loader);

                    // When a bound exists, verify the class is a subtype
                    // before taking the then-branch.  E.g. for
                    // `($type is class-string<FormFlowTypeInterface> ? FormFlowInterface : FormInterface)`
                    // with `ImageUploadFormType::class`: if `ImageUploadFormType`
                    // does NOT implement `FormFlowTypeInterface`, return
                    // `FormInterface` (else branch), not the class name.
                    if concrete_class_string_bound.is_some() {
                        return choose_branch(satisfies_bound(&resolved));
                    }

                    return Some(substitute_bound(PhpType::named(atom(&resolved))));
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
                        let names: Vec<String> = names
                            .into_iter()
                            .map(|n| crate::util::resolve_name_via_loader(&n, class_loader))
                            .collect();

                        // When a bound exists, check all resolved names.
                        if concrete_class_string_bound.is_some() {
                            let all_satisfy = names.iter().all(|n| satisfies_bound(n));
                            return choose_branch(all_satisfy);
                        }

                        let ty = if names.len() == 1 {
                            PhpType::named(atom(&names.into_iter().next().unwrap()))
                        } else {
                            PhpType::union(
                                names
                                    .into_iter()
                                    .map(|n| PhpType::named(atom(n.as_ref())))
                                    .collect(),
                            )
                        };
                        return Some(substitute_bound(ty));
                    }
                }
                // Argument isn't a ::class literal or resolvable variable → try else branch
                resolve_conditional_with_text_args_and_defaults(
                    else_type,
                    params,
                    text_args,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            } else if condition.is_null() {
                // The null (`then`) branch is taken when the argument is
                // absent (the parameter falls back to its null default) or
                // when `null` is passed explicitly. This mirrors the AST
                // path's rule so the same call resolves identically through
                // the inline-text and AST resolution paths.
                let arg_is_null = arg_text.is_none_or(|t| {
                    let t = t.trim();
                    t.is_empty() || t.eq_ignore_ascii_case("null")
                });
                if arg_is_null {
                    // No argument provided or explicitly null → null branch
                    resolve_conditional_with_text_args_and_defaults(
                        then_type,
                        params,
                        text_args,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    )
                } else {
                    // Argument was provided → not null
                    resolve_conditional_with_text_args_and_defaults(
                        else_type,
                        params,
                        text_args,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    )
                }
            } else if matches!(condition.kind(), TypeKind::Literal(_)) {
                // Value condition (`$format is 0`, `$value is ''`). The
                // argument decides it only when it is itself a literal; any
                // other expression leaves the broader else branch.
                let matched = arg_text
                    .and_then(|arg| condition_result_from_text(condition, arg))
                    .unwrap_or(false);
                let take_then = matched ^ *negated;
                resolve_conditional_with_text_args_and_defaults(
                    if take_then { then_type } else { else_type },
                    params,
                    text_args,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            } else if let Some((cond_class, cond_const)) = class_const_condition_parts(condition) {
                // Class-constant condition (e.g. `$mode is PDO::FETCH_ASSOC`).
                // Take the then-branch when the bound argument is the same
                // class constant.
                let matched = arg_text
                    .and_then(|arg| arg.trim().rsplit_once("::"))
                    .is_some_and(|(arg_class, arg_const)| {
                        class_const_matches(
                            cond_class,
                            cond_const,
                            arg_class.trim(),
                            arg_const.trim(),
                            calling_class_name,
                        )
                    });
                let take_then = matched ^ *negated;
                resolve_conditional_with_text_args_and_defaults(
                    if take_then { then_type } else { else_type },
                    params,
                    text_args,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            } else {
                // IsType equivalent (`$x is string`, `$x is array|string`,
                // …). Decide the branch in three ways, in order of
                // confidence: (1) from the argument's syntactic form (a
                // literal), (2) from the argument's resolved type when a
                // resolver is available (e.g. a method-call chain that
                // returns `string`). When neither is conclusive, fall
                // through to the else branch as before.
                let decided = arg_text.and_then(|arg| {
                    condition_result_from_text(condition, arg).or_else(|| {
                        tpl.arg_type_resolver
                            .and_then(|resolve| resolve(arg))
                            .and_then(|arg_ty| type_condition_result(&arg_ty, condition))
                    })
                });
                let branch = match decided {
                    Some(satisfied) => {
                        if satisfied ^ *negated {
                            then_type
                        } else {
                            else_type
                        }
                    }
                    None => {
                        // The condition is genuinely undecidable: the argument
                        // is an expression whose type we could not pin down.
                        // When a resolver was available (i.e. a real
                        // resolution context, not a bare completion lookup),
                        // the true result is one of the two branches, so
                        // return their union rather than committing to the
                        // else branch — otherwise `Str::replace(…, $x->y())`
                        // would resolve to `string[]` and falsely flag a
                        // `string` argument.
                        if arg_text.is_some() && tpl.arg_type_resolver.is_some() {
                            let resolve_branch = |b| {
                                resolve_conditional_with_text_args_and_defaults(
                                    b,
                                    params,
                                    text_args,
                                    var_resolver,
                                    calling_class_name,
                                    class_loader,
                                    tpl,
                                )
                            };
                            return union_branch_types(
                                resolve_branch(then_type),
                                resolve_branch(else_type),
                            );
                        }
                        else_type
                    }
                };
                resolve_conditional_with_text_args_and_defaults(
                    branch,
                    params,
                    text_args,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            }
        }
        _ => {
            if conditional.is_uninformative_return() {
                return None;
            }
            Some(conditional.clone())
        }
    }
}

/// The source text of a literal argument expression, or `None` when the
/// argument is any other expression (a variable, a call, a class constant).
fn literal_expr_text<'a>(expr: &Expression<'a>) -> Option<&'a str> {
    match expr {
        Expression::Literal(Literal::String(s)) => Some(crate::atom::bytes_to_str(s.raw)),
        Expression::Literal(Literal::Integer(i)) => Some(crate::atom::bytes_to_str(i.raw)),
        Expression::Literal(Literal::Float(f)) => Some(crate::atom::bytes_to_str(f.raw)),
        Expression::Literal(Literal::True(_)) => Some("true"),
        Expression::Literal(Literal::False(_)) => Some("false"),
        Expression::Literal(Literal::Null(_)) => Some("null"),
        _ => None,
    }
}

/// The syntactic form of an AST argument, with `arg_text` covering the literal
/// and omitted-argument cases (see [`literal_expr_text`] and
/// [`param_default_text`]).
///
/// Array literals have no source text to classify but are still a known form,
/// and an interpolated string (`"a$b"`) is a string however its parts resolve.
fn ast_arg_form(arg_expr: Option<&Expression<'_>>, arg_text: Option<&str>) -> ArgForm {
    match arg_expr {
        Some(Expression::Array(_) | Expression::LegacyArray(_)) => ArgForm::ArrayLit,
        Some(Expression::CompositeString(_)) => ArgForm::StringLit,
        _ => arg_text.map_or(ArgForm::Unknown, classify_arg_form),
    }
}

/// Checks whether the argument text is a quoted string literal.
fn arg_is_string_literal(arg: &str) -> bool {
    let t = arg.trim();
    (t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"'))
}

/// Checks whether the argument text is an integer literal, in any of the
/// notations PHP accepts (`10`, `-10`, `0x0a`, `0b1010`, `012`, `1_0`).
fn arg_is_int_literal(arg: &str) -> bool {
    crate::php_type::parse_php_int_literal(arg.trim()).is_some()
}

/// Checks whether the argument text is a float literal.
fn arg_is_float_literal(arg: &str) -> bool {
    let t = arg.trim();
    let t = t.strip_prefix('-').unwrap_or(t);
    t.contains('.') && t.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// The syntactic form of an argument's source text, as far as it can be
/// classified without resolving its type. Used to decide `is <Type>`
/// conditions against literal arguments.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArgForm {
    StringLit,
    IntLit,
    FloatLit,
    BoolLit,
    Null,
    ArrayLit,
    /// Any expression whose type cannot be read from its syntax alone
    /// (variables, property/method chains, function calls, closures, …).
    Unknown,
}

/// Classify the syntactic form of an argument's source text.
fn classify_arg_form(arg: &str) -> ArgForm {
    let t = arg.trim();
    if t.is_empty() {
        return ArgForm::Unknown;
    }
    if arg_is_string_literal(t) {
        return ArgForm::StringLit;
    }
    if arg_is_int_literal(t) {
        return ArgForm::IntLit;
    }
    if arg_is_float_literal(t) {
        return ArgForm::FloatLit;
    }
    if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false") {
        return ArgForm::BoolLit;
    }
    if t.eq_ignore_ascii_case("null") {
        return ArgForm::Null;
    }
    if t.starts_with('[') || t.to_ascii_lowercase().starts_with("array(") {
        return ArgForm::ArrayLit;
    }
    ArgForm::Unknown
}

/// The literal value an argument's source text denotes, when the text is a
/// literal at all (`0`, `-1`, `1.5`, `'json'`).
fn literal_value_from_text(arg_text: &str) -> Option<LiteralValue> {
    let t = arg_text.trim();
    match classify_arg_form(t) {
        ArgForm::StringLit => Some(LiteralValue::string_raw(t)),
        ArgForm::IntLit => Some(LiteralValue::int(t)),
        ArgForm::FloatLit => Some(LiteralValue::float(t)),
        _ => None,
    }
}

/// The source text of a parameter's declared default value.
///
/// An omitted argument takes this value at runtime, so a conditional keyed on
/// that parameter is decided against the default rather than being treated as
/// undecidable.
fn param_default_text(param: Option<&ParameterInfo>) -> Option<&str> {
    param?.default_value.as_deref()
}

/// Decide a conditional's condition against an argument's source text.
///
/// A value condition (`$format is 0`) is settled by comparing literal values,
/// so `0x1` satisfies `is 1` and `"foo"` satisfies `is 'foo'`. A type condition
/// (`$x is string`, `$x is array|string`) is settled by the argument's
/// syntactic form. Returns `None` when the text settles neither, which happens
/// whenever the argument is an expression rather than a literal.
fn condition_result_from_text(condition: &PhpType, arg_text: &str) -> Option<bool> {
    match condition.kind() {
        TypeKind::Union(members) => combine_union_results(
            members
                .iter()
                .map(|member| condition_result_from_text(member, arg_text)),
        ),
        TypeKind::Literal(expected) => literal_value_from_text(arg_text)
            .map(|actual| crate::php_type::literals_equal(expected, &actual)),
        _ => condition_result_from_form(condition, classify_arg_form(arg_text)),
    }
}

/// Decide a type condition (`$x is string`, `$x is array|string`) from the
/// argument's syntactic form.
///
/// A literal argument has a fully known type, so the answer is conclusive;
/// [`ArgForm::Unknown`] yields `None` for the caller to handle. Union
/// conditions are satisfied when any member is.
fn condition_result_from_form(condition: &PhpType, form: ArgForm) -> Option<bool> {
    if form == ArgForm::Unknown {
        return None;
    }
    match condition.kind() {
        TypeKind::Union(members) => combine_union_results(
            members
                .iter()
                .map(|member| condition_result_from_form(member, form)),
        ),
        _ => Some(scalar_condition_matches_form(condition, form)),
    }
}

/// Combine the per-member results of a union condition: it is satisfied when
/// any member is, and refuted only when every member is refuted.
fn combine_union_results(results: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut any_true = false;
    let mut all_false = true;
    for result in results {
        match result {
            Some(true) => {
                any_true = true;
                all_false = false;
            }
            Some(false) => {}
            None => all_false = false,
        }
    }
    if any_true {
        Some(true)
    } else if all_false {
        Some(false)
    } else {
        None
    }
}

/// Whether a single (non-union) type condition is satisfied by a literal
/// argument form. A literal always has a fully known type, so this is
/// conclusive (unlike [`ArgForm::Unknown`], which the caller handles).
fn scalar_condition_matches_form(condition: &PhpType, form: ArgForm) -> bool {
    match form {
        ArgForm::StringLit => condition.is_string_type(),
        ArgForm::IntLit => condition.is_int(),
        ArgForm::FloatLit => condition.is_float(),
        ArgForm::BoolLit => condition.is_bool(),
        ArgForm::Null => condition.is_null(),
        ArgForm::ArrayLit => condition.is_array_like(),
        ArgForm::Unknown => false,
    }
}

/// The broad runtime category a resolved type belongs to, used to decide
/// mutually-exclusive `is <scalar>` conditions. Returns `None` when the type
/// cannot be placed in a single category (so the condition stays undecided).
fn type_category(t: &PhpType) -> Option<&'static str> {
    if t.is_string_subtype() {
        Some("string")
    } else if t.is_int_subtype() {
        Some("int")
    } else if t.is_float_subtype() {
        Some("float")
    } else if t.is_bool() {
        Some("bool")
    } else if t.is_null() {
        Some("null")
    } else if t.is_array_like() {
        Some("array")
    } else if matches!(t.kind(), TypeKind::Callable(_)) || t.base_name().is_some() {
        // A closure/callable or any class instance — not a scalar or array.
        Some("object")
    } else {
        None
    }
}

/// The category named by a (non-union) `is <Type>` condition, or `None` when
/// the condition is not a plain scalar/array type we can categorise.
fn condition_category(condition: &PhpType) -> Option<&'static str> {
    if condition.is_string_type() {
        Some("string")
    } else if condition.is_int() {
        Some("int")
    } else if condition.is_float() {
        Some("float")
    } else if condition.is_bool() {
        Some("bool")
    } else if condition.is_null() {
        Some("null")
    } else if condition.is_array_like() {
        Some("array")
    } else {
        None
    }
}

/// Decide whether an argument of resolved type `arg_ty` satisfies a type
/// `condition`.
///
/// Returns `Some(true)` when the resolved type clearly satisfies the
/// condition, `Some(false)` when it clearly cannot (a mutually-exclusive
/// category, e.g. a `Closure` argument against `array|string`), and `None`
/// when it cannot be proven either way (`mixed`, an unresolved type, or a
/// mixed union). The caller uses `None` to fall back to a union of both
/// branches rather than committing to the wrong one.
fn type_condition_result(arg_ty: &PhpType, condition: &PhpType) -> Option<bool> {
    if arg_ty.is_mixed() || arg_ty.is_untyped() {
        return None;
    }
    // Condition union (`array|string`): satisfied when any member matches,
    // refuted only when every member is refuted.
    if let TypeKind::Union(members) = condition.kind() {
        return combine_union_results(
            members
                .iter()
                .map(|member| type_condition_result(arg_ty, member)),
        );
    }
    // Argument union: satisfied only when every member matches, refuted only
    // when every member is refuted, otherwise indeterminate.
    if let TypeKind::Union(members) = arg_ty.kind() {
        let results: Vec<Option<bool>> = members
            .iter()
            .map(|m| type_condition_result(m, condition))
            .collect();
        return if results.iter().all(|r| *r == Some(true)) {
            Some(true)
        } else if results.iter().all(|r| *r == Some(false)) {
            Some(false)
        } else {
            None
        };
    }
    match (type_category(arg_ty), condition_category(condition)) {
        (Some(arg_cat), Some(cond_cat)) => Some(arg_cat == cond_cat),
        _ => None,
    }
}

/// Union two optional branch types produced by an undecidable conditional,
/// dropping duplicates and any uninformative branch.
fn union_branch_types(a: Option<PhpType>, b: Option<PhpType>) -> Option<PhpType> {
    let mut members: Vec<PhpType> = Vec::new();
    let mut push = |ty: PhpType| match ty.kind() {
        TypeKind::Union(inner) => {
            for m in inner {
                if !members.contains(m) {
                    members.push(m.clone());
                }
            }
        }
        _ => {
            if !members.contains(&ty) {
                members.push(ty);
            }
        }
    };
    if let Some(t) = a {
        push(t);
    }
    if let Some(t) = b {
        push(t);
    }
    match members.len() {
        0 => None,
        1 => members.into_iter().next(),
        _ => Some(PhpType::union(members)),
    }
}

/// Recursively evaluate any nested [`TypeKind::Conditional`] nodes inside a
/// type against textual call-site arguments, replacing each with the type of
/// its winning branch.
///
/// A method's return type can embed a conditional inside a generic wrapper,
/// e.g. Laravel's `Collection::groupBy` returns
/// `static<($groupBy is array|string ? array-key : …), static<…>>`. The
/// top-level conditional resolvers only handle a return type that is *itself*
/// a conditional; this walker reaches conditionals nested inside `Generic`,
/// `Union`, `Array`, and shape positions so they never survive raw into a
/// resolved variable type (where they would later be compared against a
/// call argument and printed unevaluated).
///
/// When a conditional cannot be resolved to an informative type it collapses
/// to `mixed` rather than remaining a raw conditional.
pub fn evaluate_nested_conditionals_text(
    ty: &PhpType,
    params: &[ParameterInfo],
    text_args: &str,
    var_resolver: VarClassStringResolver<'_>,
    calling_class_name: Option<&str>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    tpl: &TemplateContext<'_>,
) -> PhpType {
    let recurse = |inner: &PhpType| {
        evaluate_nested_conditionals_text(
            inner,
            params,
            text_args,
            var_resolver,
            calling_class_name,
            class_loader,
            tpl,
        )
    };
    match ty.kind() {
        TypeKind::Conditional(_) => {
            let resolved = resolve_conditional_with_text_args_and_defaults(
                ty,
                params,
                text_args,
                var_resolver,
                calling_class_name,
                class_loader,
                tpl,
            )
            .unwrap_or_else(|| PhpType::named(atom("mixed")));
            // Template-default resolution hands back the winning branch
            // without recursing into it, so the branch may itself be a
            // conditional (e.g. `TGroupKey is \UnitEnum ? … : (TGroupKey is
            // \Stringable ? … : …)`). Keep collapsing until none remains.
            // The `resolved != *ty` guard prevents a non-terminating loop on
            // a conditional that cannot be reduced further; such a residual
            // conditional collapses to `mixed` rather than surviving raw.
            if !resolved.contains_conditional() {
                resolved
            } else if resolved != *ty {
                recurse(&resolved)
            } else {
                PhpType::named(atom("mixed"))
            }
        }
        TypeKind::Generic(g) => PhpType::generic_atom(g.name, g.args.iter().map(recurse).collect()),
        TypeKind::Union(members) => PhpType::union(members.iter().map(recurse).collect()),
        TypeKind::Intersection(members) => {
            PhpType::intersection(members.iter().map(recurse).collect())
        }
        TypeKind::Nullable(inner) => PhpType::nullable(recurse(inner)),
        TypeKind::Array(inner) => PhpType::array_of(recurse(inner)),
        TypeKind::ArrayShape(entries) => PhpType::array_shape(
            entries
                .iter()
                .map(|e| crate::php_type::ShapeEntry {
                    key: e.key.clone(),
                    value_type: recurse(&e.value_type),
                    optional: e.optional,
                })
                .collect(),
        ),
        TypeKind::ObjectShape(entries) => PhpType::object_shape(
            entries
                .iter()
                .map(|e| crate::php_type::ShapeEntry {
                    key: e.key.clone(),
                    value_type: recurse(&e.value_type),
                    optional: e.optional,
                })
                .collect(),
        ),
        other => other.clone().into(),
    }
}

/// Split a textual argument list by commas, respecting nested brackets
/// so that `"foo(a, b), c"` splits into `["foo(a, b)", "c"]`.
///
/// Braces count as brackets too, so a closure argument written out in full
/// (`function ($a) { return [$a, 1]; }, $rest`) stays one argument.
pub fn split_text_args(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_was_backslash = false;

    for (i, ch) in text.char_indices() {
        if prev_was_backslash {
            prev_was_backslash = false;
            continue;
        }
        match ch {
            '\\'
                // Only treat as escape if inside a quote
                if in_single_quote || in_double_quote =>
            {
                prev_was_backslash = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '(' | '[' | '{' if !in_single_quote && !in_double_quote => {
                depth += 1;
            }
            ')' | ']' | '}' if !in_single_quote && !in_double_quote => {
                depth = depth.saturating_sub(1);
            }
            ',' if depth == 0 && !in_single_quote && !in_double_quote => {
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

/// If `name` is `"self"`, `"static"`, or `"parent"`, substitute the
/// calling-site class name so that the resolved type is concrete rather
/// than relative to the method-owner class. Returns `None` for any other
/// name.
fn resolve_self_keyword(name: &str, calling_class_name: Option<&str>) -> Option<String> {
    match name {
        "self" | "static" | "parent" => calling_class_name.map(|n| n.to_string()),
        _ => None,
    }
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
        Some(name.to_string())
    } else {
        None
    }
}

/// If `condition` is a class-constant reference such as `PDO::FETCH_ASSOC`,
/// return its `(class, constant)` parts.
///
/// PHPStan conditional return types use class constants as the compared
/// type, e.g. `@return ($mode is PDO::FETCH_ASSOC ? ... : ...)`. The type
/// parser represents such a member reference as a `Raw`/`Named` variant
/// whose payload contains `::`. Wildcard members (`Foo::*`, used by
/// `int-mask-of<Foo::*>`) and the `::class` pseudo-constant are not real
/// class constants and return `None`.
fn class_const_condition_parts(condition: &PhpType) -> Option<(&str, &str)> {
    let raw = match condition.kind() {
        TypeKind::Raw(s) => s,
        TypeKind::Named(s) => s.as_str(),
        _ => return None,
    };
    let (class, member) = raw.rsplit_once("::")?;
    if class.is_empty() || member.is_empty() || member.contains('*') || member == "class" {
        return None;
    }
    Some((class, member))
}

/// Whether an argument referring to the class constant `(arg_class,
/// arg_const)` matches the condition's `(cond_class, cond_const)`.
///
/// Constant names must match exactly; class names are compared on their
/// short (namespace-stripped) form, case-insensitively, which mirrors
/// PHP's case-insensitive class-name resolution. `self`/`static`/`parent`
/// in the argument are resolved against `calling_class_name` first.
fn class_const_matches(
    cond_class: &str,
    cond_const: &str,
    arg_class: &str,
    arg_const: &str,
    calling_class_name: Option<&str>,
) -> bool {
    if arg_const != cond_const {
        return false;
    }
    let arg_class = resolve_self_keyword(arg_class, calling_class_name)
        .unwrap_or_else(|| arg_class.to_string());
    let cond_short = crate::util::short_name(cond_class.trim_start_matches('\\'));
    let arg_short = crate::util::short_name(arg_class.trim_start_matches('\\'));
    arg_short.eq_ignore_ascii_case(cond_short)
}

/// Extract the `(class, constant)` parts from a class-constant access
/// expression such as `PDO::FETCH_ASSOC`.
///
/// Unlike [`extract_class_string_from_expr`], this rejects the `::class`
/// pseudo-constant and only matches identifier selectors (not dynamic
/// `::{$expr}` constant fetches).
fn extract_class_const_from_expr(expr: &Expression<'_>) -> Option<(String, String)> {
    if let Expression::Access(Access::ClassConstant(cca)) = expr
        && let ClassLikeConstantSelector::Identifier(ident) = &cca.constant
        && ident.value != b"class"
    {
        let class = match cca.class {
            Expression::Identifier(class_ident) => {
                crate::atom::bytes_to_str(class_ident.value()).to_string()
            }
            Expression::Self_(_) => "self".to_string(),
            Expression::Static(_) => "static".to_string(),
            Expression::Parent(_) => "parent".to_string(),
            _ => return None,
        };
        return Some((class, crate::atom::bytes_to_str(ident.value).to_string()));
    }
    None
}

/// Resolve a PHPStan conditional return type given AST-level call-site
/// arguments.
///
/// Walks the conditional tree and matches argument expressions against
/// the conditions:
///   - `class-string<T>`: checks if the positional argument is `X::class`
///     and returns `"X"`.
///   - `is null`: satisfied when `null` is passed or the omitted argument
///     defaults to null.
///   - `is 0` / `is 'json'`: satisfied when the argument is a literal of the
///     same value.
///   - `is SomeType`: satisfied when the argument is a literal of that type;
///     any other expression falls through to the else branch.
pub(crate) fn resolve_conditional_with_args<'b>(
    conditional: &PhpType,
    params: &[ParameterInfo],
    argument_list: &ArgumentList<'b>,
    var_resolver: VarClassStringResolver<'_>,
    calling_class_name: Option<&str>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    tpl: &TemplateContext<'_>,
) -> Option<PhpType> {
    resolve_conditional_with_args_and_defaults(
        conditional,
        params,
        argument_list,
        var_resolver,
        calling_class_name,
        class_loader,
        tpl,
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
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    tpl: &TemplateContext<'_>,
) -> Option<PhpType> {
    match conditional.kind() {
        TypeKind::Conditional(cond) => {
            let (param, negated, condition, then_type, else_type) = (
                &cond.param,
                &cond.negated,
                &cond.condition,
                &cond.then_type,
                &cond.else_type,
            );
            // Check if the conditional subject is a template parameter
            // with a default value (not a method $parameter).
            let target = param.as_str();
            if !target.starts_with('$')
                && let Some(resolved) = try_resolve_with_template_default(
                    target,
                    *negated,
                    condition,
                    then_type,
                    else_type,
                    tpl.defaults,
                )
            {
                return Some(resolved);
            }

            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);

            // Bind arguments to parameters following PHP's rules (positional
            // fill in order, named fill by name) so a named argument in an
            // earlier slot does not shadow the parameter the conditional
            // refers to.
            let arg_expr: Option<&Expression<'b>> =
                crate::call_args::bind_args_to_params(params, argument_list)
                    .get(param_idx)
                    .copied()
                    .flatten();

            // The argument's source text, for the conditions that are decided
            // by a literal value rather than by the expression's shape. A
            // literal argument supplies its own text; an omitted one takes the
            // parameter's declared default, which is what it evaluates to at
            // runtime.
            let arg_text: Option<&str> = match arg_expr {
                Some(expr) => literal_expr_text(expr),
                None => param_default_text(params.get(param_idx)),
            };

            if matches!(condition.kind(), TypeKind::ClassString(_)) {
                // Extract the bound from `class-string<Bound>` and determine
                // whether it is a template parameter or a concrete class.
                // Mirrors the logic in resolve_conditional_with_text_args_and_defaults.
                let class_string_bound_name: Option<&str> = match condition.kind() {
                    TypeKind::ClassString(Some(inner)) => match inner.kind() {
                        TypeKind::Named(name) => Some(name.as_str()),
                        _ => None,
                    },
                    _ => None,
                };

                let bound_is_template = class_string_bound_name
                    .is_some_and(|name| tpl.params.iter().any(|tp| tp.as_str() == name));

                let concrete_class_string_bound: Option<String> = if bound_is_template {
                    None
                } else {
                    class_string_bound_name.map(|name| {
                        let resolved = crate::util::resolve_name_via_loader(name, class_loader);
                        if class_loader(&resolved).is_some() {
                            resolved
                        } else {
                            "!!unresolvable_bound!!".to_string()
                        }
                    })
                };

                let satisfies_bound = |resolved_name: &str| -> bool {
                    match concrete_class_string_bound {
                        None => true,
                        Some(ref bound) => crate::class_lookup::is_subtype_of_names(
                            resolved_name,
                            bound,
                            class_loader,
                        ),
                    }
                };

                let choose_branch = |bound_satisfied: bool| -> Option<PhpType> {
                    let take_then = bound_satisfied ^ *negated;
                    resolve_conditional_with_args_and_defaults(
                        if take_then { then_type } else { else_type },
                        params,
                        argument_list,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    )
                };

                // See the matching helper in resolve_conditional_with_text_args_and_defaults:
                // a template-parameter bound must have `then_type` substituted rather
                // than be replaced outright, so surrounding structure like
                // `T&MockInterface` survives.
                let substitute_bound = |resolved_ty: PhpType| -> PhpType {
                    match class_string_bound_name {
                        Some(bound_name) if bound_is_template => {
                            let subs = std::collections::HashMap::from([(
                                bound_name.to_string(),
                                resolved_ty,
                            )]);
                            then_type.substitute(&subs)
                        }
                        _ => resolved_ty,
                    }
                };

                // Check if the argument is `X::class`.  When the argument is
                // omitted entirely, its declared default supplies the name so
                // `app()` resolves the same as `app(Foo::class)`.
                let class_name = arg_expr
                    .and_then(extract_class_string_from_expr)
                    .or_else(|| {
                        arg_expr
                            .and_then(string_literal_content)
                            .and_then(|name| class_named_by_string(name, class_loader))
                    })
                    .or_else(|| {
                        arg_text.and_then(|text| {
                            extract_class_name_from_text(text)
                                .or_else(|| class_named_by_quoted_string(text, class_loader))
                        })
                    });
                if let Some(class_name) = class_name {
                    let class_name =
                        resolve_self_keyword(&class_name, calling_class_name).unwrap_or(class_name);
                    let resolved = crate::util::resolve_name_via_loader(&class_name, class_loader);

                    if concrete_class_string_bound.is_some() {
                        return choose_branch(satisfies_bound(&resolved));
                    }

                    return Some(substitute_bound(PhpType::named(atom(&resolved))));
                }
                // Check if the argument is a variable holding class-string
                // value(s) (e.g. from a match expression).
                if let Some(Expression::Variable(Variable::Direct(dv))) = arg_expr
                    && let Some(resolver) = var_resolver
                {
                    let names = resolver(crate::atom::bytes_to_str(dv.name));
                    if !names.is_empty() {
                        let names: Vec<String> = names
                            .into_iter()
                            .map(|n| crate::util::resolve_name_via_loader(&n, class_loader))
                            .collect();

                        if concrete_class_string_bound.is_some() {
                            let all_satisfy = names.iter().all(|n| satisfies_bound(n));
                            return choose_branch(all_satisfy);
                        }

                        let ty = if names.len() == 1 {
                            PhpType::named(atom(&names.into_iter().next().unwrap()))
                        } else {
                            PhpType::union(
                                names
                                    .into_iter()
                                    .map(|n| PhpType::named(atom(n.as_ref())))
                                    .collect(),
                            )
                        };
                        return Some(substitute_bound(ty));
                    }
                }
                // Argument isn't a ::class literal or resolvable variable → try else branch
                resolve_conditional_with_args_and_defaults(
                    else_type,
                    params,
                    argument_list,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            } else if condition.is_null() {
                // The null (`then`) branch is taken when the argument is
                // absent and defaults to null, or when `null` is passed
                // explicitly. An absent argument with a non-null default is
                // that default, so it takes the else branch.
                let arg_is_null = match arg_expr {
                    None => arg_text.is_none_or(|t| t.trim().eq_ignore_ascii_case("null")),
                    Some(expr) => matches!(expr, Expression::Literal(Literal::Null(_))),
                };
                if arg_is_null {
                    // No argument provided or explicitly null → null branch
                    resolve_conditional_with_args_and_defaults(
                        then_type,
                        params,
                        argument_list,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    )
                } else {
                    // Argument was provided and not null → else branch
                    resolve_conditional_with_args_and_defaults(
                        else_type,
                        params,
                        argument_list,
                        var_resolver,
                        calling_class_name,
                        class_loader,
                        tpl,
                    )
                }
            } else if matches!(condition.kind(), TypeKind::Literal(_)) {
                // Value condition (`$format is 0`, `$value is ''`), decided by
                // comparing the argument's literal value against the expected
                // one. Any non-literal argument leaves the else branch.
                let matched = arg_text
                    .and_then(|text| condition_result_from_text(condition, text))
                    .unwrap_or(false);
                let take_then = matched ^ *negated;
                resolve_conditional_with_args_and_defaults(
                    if take_then { then_type } else { else_type },
                    params,
                    argument_list,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            } else if let Some((cond_class, cond_const)) = class_const_condition_parts(condition) {
                // Class-constant condition (e.g. `$mode is PDO::FETCH_ASSOC`).
                let matched = arg_expr
                    .and_then(extract_class_const_from_expr)
                    .is_some_and(|(arg_class, arg_const)| {
                        class_const_matches(
                            cond_class,
                            cond_const,
                            &arg_class,
                            &arg_const,
                            calling_class_name,
                        )
                    });
                let take_then = matched ^ *negated;
                resolve_conditional_with_args_and_defaults(
                    if take_then { then_type } else { else_type },
                    params,
                    argument_list,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            } else {
                // IsType equivalent (`$x is string`, `$x is array|string`, …),
                // decided from the argument's syntactic form. Anything whose
                // type we can't read from its syntax alone is undecidable and
                // falls through to the else branch.
                let matched =
                    condition_result_from_form(condition, ast_arg_form(arg_expr, arg_text))
                        .unwrap_or(false);
                let take_then = matched ^ *negated;
                resolve_conditional_with_args_and_defaults(
                    if take_then { then_type } else { else_type },
                    params,
                    argument_list,
                    var_resolver,
                    calling_class_name,
                    class_loader,
                    tpl,
                )
            }
        }
        _ => {
            if conditional.is_uninformative_return() {
                return None;
            }
            Some(conditional.clone())
        }
    }
}

/// Resolve a conditional return type **without** call-site arguments
/// (text-based path).  Walks the tree taking the "no argument / null
/// default" branch at each level.
pub(crate) fn resolve_conditional_without_args(
    conditional: &PhpType,
    params: &[ParameterInfo],
) -> Option<PhpType> {
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
    template_defaults: Option<&HashMap<String, PhpType>>,
) -> Option<PhpType> {
    match conditional.kind() {
        TypeKind::Conditional(cond) => {
            let (param, negated, condition, then_type, else_type) = (
                &cond.param,
                &cond.negated,
                &cond.condition,
                &cond.then_type,
                &cond.else_type,
            );
            // Check if the conditional subject is a template parameter
            // with a default value (not a method $parameter).
            let target = param.as_str();
            if !target.starts_with('$')
                && let Some(resolved) = try_resolve_with_template_default(
                    target,
                    *negated,
                    condition,
                    then_type,
                    else_type,
                    template_defaults,
                )
            {
                return Some(resolved);
            }

            // Every parameter takes its declared default, so a default the
            // condition can be decided against settles the branch.
            let param_info = params.iter().find(|p| p.name == target);
            if let Some(default_text) = param_info.and_then(|p| p.default_value.as_deref())
                && let Some(matched) = condition_result_from_text(condition, default_text)
            {
                let branch = if matched ^ *negated {
                    then_type
                } else {
                    else_type
                };
                return resolve_conditional_without_args_and_defaults(
                    branch,
                    params,
                    template_defaults,
                );
            }

            // Otherwise fall back to whether the parameter is optional at all:
            // an omitted optional argument is most often defaulted to null.
            let has_null_default = param_info.is_some_and(|p| !p.is_required);

            if condition.is_null() && has_null_default {
                resolve_conditional_without_args_and_defaults(then_type, params, template_defaults)
            } else {
                // Try else branch
                resolve_conditional_without_args_and_defaults(else_type, params, template_defaults)
            }
        }
        _ => {
            if conditional.is_uninformative_return() {
                return None;
            }
            Some(conditional.clone())
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
    template_defaults: Option<&HashMap<String, PhpType>>,
) -> Option<PhpType> {
    let defaults = template_defaults?;
    let default_value = defaults.get(template_name)?;

    // Determine whether the default value matches the condition.
    let condition_matches = if condition.is_false() {
        default_value.is_false()
    } else if condition.is_true() {
        default_value.is_true()
    } else if condition.is_null() {
        default_value.is_null()
    } else if condition.is_bool() {
        default_value.is_true() || default_value.is_false()
    } else if condition.is_string_type() {
        default_value.is_string_literal()
    } else if condition.is_int() {
        default_value.is_int_literal()
    } else if let TypeKind::Literal(lit) = condition.kind() {
        let expected = lit
            .string_content()
            .map(Cow::into_owned)
            .unwrap_or_else(|| lit.as_raw());
        match default_value.kind() {
            TypeKind::Literal(dv) => {
                dv.string_content()
                    .map(Cow::into_owned)
                    .unwrap_or_else(|| dv.as_raw())
                    == expected
            }
            TypeKind::Named(dv) => dv == &expected,
            _ => false,
        }
    } else if let TypeKind::Named(s) = condition.kind() {
        match default_value.kind() {
            TypeKind::Named(dv) => dv == s,
            _ => false,
        }
    } else {
        return None;
    };

    let effective_match = if negated {
        !condition_matches
    } else {
        condition_matches
    };

    let branch = if effective_match {
        then_type
    } else {
        else_type
    };
    if branch.is_uninformative_return() {
        return None;
    }
    Some(branch.clone())
}

/// Collapse a conditional return type using known template-parameter values.
///
/// When the conditional's subject is a template parameter (not a method
/// `$parameter`) whose value is known from `values` — for example
/// `@template TAsync of bool = false` supplying `TAsync => false`, or an
/// explicit `@mixin Foo<false>` generic argument — the condition can be
/// evaluated and the conditional replaced by the winning branch.
///
/// Returns `None` when the conditional's subject is a runtime parameter,
/// is absent from `values`, or the winning branch is uninformative.
pub fn resolve_conditional_from_values(
    conditional: &PhpType,
    values: &HashMap<String, PhpType>,
) -> Option<PhpType> {
    if let TypeKind::Conditional(cond) = conditional.kind()
        && !cond.param.starts_with('$')
    {
        return try_resolve_with_template_default(
            &cond.param,
            cond.negated,
            &cond.condition,
            &cond.then_type,
            &cond.else_type,
            Some(values),
        );
    }
    None
}

/// The class a plain string argument names, when it names one at all.
///
/// PHP accepts a string wherever a `class-string` is expected, so
/// `$container->make('App\Services\Clock')` is as much a class-string call as
/// `make(Clock::class)` is; in a Laravel project the loader also answers for
/// the container keys service providers bind (`make('sentry')`).  The class
/// has to actually resolve: an ordinary string argument must keep falling
/// through to the conditional's else branch rather than being read as the name
/// of a class that does not exist.
fn class_named_by_string(
    name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    class_loader(name).map(|cls| cls.fqn().to_string())
}

/// [`class_named_by_string`] for the text-argument path, where the argument is
/// still the quoted source text.
fn class_named_by_quoted_string(
    arg_text: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    let name = crate::util::unescape_php_string_literal(arg_text.trim())?;
    class_named_by_string(&name, class_loader)
}

/// The content of a string-literal expression, without its quotes.
fn string_literal_content<'a>(expr: &Expression<'a>) -> Option<&'a str> {
    match expr {
        Expression::Literal(literal::Literal::String(s)) => s.value.map(crate::atom::bytes_to_str),
        _ => None,
    }
}

/// Extract the class name from an `X::class` expression.
///
/// Matches `Expression::Access(Access::ClassConstant(cca))` where the
/// constant selector is the identifier `class`.
pub(crate) fn extract_class_string_from_expr(expr: &Expression<'_>) -> Option<String> {
    if let Expression::Access(Access::ClassConstant(cca)) = expr
        && let ClassLikeConstantSelector::Identifier(ident) = &cca.constant
        && ident.value == b"class"
    {
        // Extract the class name from the LHS
        return match cca.class {
            Expression::Identifier(class_ident) => {
                Some(crate::atom::bytes_to_str(class_ident.value()).to_string())
            }
            Expression::Self_(_) => Some("self".to_string()),
            Expression::Static(_) => Some("static".to_string()),
            Expression::Parent(_) => Some("parent".to_string()),
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single required parameter with the given name (including `$`).
    fn param(name: &str) -> ParameterInfo {
        ParameterInfo {
            name: crate::atom::atom(name),
            is_required: true,
            type_hint: None,
            native_type_hint: None,
            description: None,
            default_value: None,
            is_variadic: false,
            is_reference: false,
            closure_this_type: None,
        }
    }

    /// An optional parameter with the given declared default value.
    fn param_with_default(name: &str, default: &str) -> ParameterInfo {
        ParameterInfo {
            is_required: false,
            default_value: Some(default.to_string()),
            ..param(name)
        }
    }

    /// Resolve a `str_word_count`-style conditional keyed on the value of a
    /// `$format` parameter that defaults to `0`.
    fn resolve_word_count(text_args: &str) -> Option<String> {
        let cond = PhpType::parse(
            "($format is 0 ? int : ($format is 1 ? list<string> : ($format is 2 ? array<int, string> : list<string>|int)))",
        );
        let params = [param("$string"), param_with_default("$format", "0")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let tpl = TemplateContext::with_params(&[]);
        resolve_conditional_with_text_args_and_defaults(
            &cond, &params, text_args, None, None, loader, &tpl,
        )
        .map(|t| t.to_string())
    }

    /// An omitted argument takes the parameter's declared default, so the
    /// condition on `$format` is decided by that `0` rather than left open.
    #[test]
    fn omitted_argument_is_decided_by_its_declared_default() {
        assert_eq!(resolve_word_count("$text").as_deref(), Some("int"));
    }

    /// A value condition compares literal values, so an explicit int argument
    /// picks its own branch.
    #[test]
    fn int_literal_argument_matches_value_condition() {
        assert_eq!(
            resolve_word_count("$text, 1").as_deref(),
            Some("list<string>")
        );
        assert_eq!(
            resolve_word_count("$text, 2").as_deref(),
            Some("array<int, string>")
        );
        // Comparison is by value, not by spelling.
        assert_eq!(
            resolve_word_count("$text, 0x1").as_deref(),
            Some("list<string>")
        );
    }

    /// A named argument reaches the parameter it targets rather than the slot
    /// it sits in.
    #[test]
    fn named_argument_matches_value_condition() {
        assert_eq!(
            resolve_word_count("$text, format: 1").as_deref(),
            Some("list<string>")
        );
    }

    /// An argument that is not a literal decides nothing, so the conditional
    /// collapses to its final else branch — every value the call could return.
    #[test]
    fn non_literal_argument_leaves_value_condition_undecided() {
        assert_eq!(
            resolve_word_count("$text, $format").as_deref(),
            Some("list<string>|int")
        );
    }

    /// With no arguments at all, the no-args path decides the condition from
    /// the declared default the same way the text path does.
    #[test]
    fn no_args_path_is_decided_by_declared_default() {
        let cond = PhpType::parse("($format is 0 ? int : list<string>)");
        let params = [param_with_default("$format", "0")];
        assert_eq!(
            resolve_conditional_without_args(&cond, &params).map(|t| t.to_string()),
            Some("int".to_string())
        );

        // A non-null default no longer reads as null just because the
        // parameter is optional.
        let nullable = PhpType::parse("($format is null ? int : list<string>)");
        assert_eq!(
            resolve_conditional_without_args(&nullable, &params).map(|t| t.to_string()),
            Some("list<string>".to_string())
        );
    }

    /// Resolve a `PDOStatement::fetch`-style conditional keyed on the fetch
    /// mode class constant, returning the resolved type's display string.
    fn resolve_fetch(text_args: &str) -> Option<String> {
        // ($mode is PDO::FETCH_OBJ ? \stdClass|false
        //   : ($mode is PDO::FETCH_ASSOC ? array<string, mixed>|false : mixed))
        let cond = PhpType::parse(
            "($mode is PDO::FETCH_OBJ ? \\stdClass|false : ($mode is PDO::FETCH_ASSOC ? array<string, mixed>|false : mixed))",
        );
        let params = [param("$mode")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let tpl = TemplateContext::with_params(&[]);
        resolve_conditional_with_text_args_and_defaults(
            &cond, &params, text_args, None, None, loader, &tpl,
        )
        .map(|t| t.to_string())
    }

    #[test]
    fn class_const_condition_selects_matching_branch() {
        assert!(
            resolve_fetch("\\PDO::FETCH_OBJ")
                .unwrap()
                .contains("stdClass")
        );
        assert!(
            resolve_fetch("\\PDO::FETCH_ASSOC")
                .unwrap()
                .contains("array")
        );
    }

    #[test]
    fn class_const_condition_ignores_leading_backslash() {
        // Argument without a leading backslash still matches the condition.
        assert!(
            resolve_fetch("PDO::FETCH_OBJ")
                .unwrap()
                .contains("stdClass")
        );
    }

    #[test]
    fn class_const_condition_unlisted_mode_falls_through() {
        // A mode with no dedicated branch reaches the `mixed` else branch.
        // `mixed` is informative (a value of unknown type), so it flows
        // through rather than yielding no resolved type.
        assert_eq!(
            resolve_fetch("\\PDO::FETCH_COLUMN").as_deref(),
            Some("mixed")
        );
    }

    #[test]
    fn class_const_condition_requires_matching_class() {
        // The same constant name on a different class must not match the
        // `PDO::FETCH_OBJ` branch — it falls through past every PDO branch to
        // the `mixed` else, which flows through as `mixed`.
        assert_eq!(resolve_fetch("Other::FETCH_OBJ").as_deref(), Some("mixed"));
        // A different class whose constant matches an inner branch also fails.
        assert_eq!(
            resolve_fetch("Other::FETCH_ASSOC").as_deref(),
            Some("mixed")
        );
    }

    #[test]
    fn class_const_condition_parts_rejects_non_constants() {
        assert!(class_const_condition_parts(&PhpType::named(atom("string"))).is_none());
        assert!(class_const_condition_parts(&PhpType::raw("Foo::*")).is_none());
        assert!(class_const_condition_parts(&PhpType::raw("Foo::class")).is_none());
        assert_eq!(
            class_const_condition_parts(&PhpType::raw("PDO::FETCH_OBJ")),
            Some(("PDO", "FETCH_OBJ"))
        );
    }

    /// A `$x is (array|string)` condition with a string-literal argument
    /// takes the then-branch, mirroring Laravel's `keyBy`/`groupBy` key type.
    #[test]
    fn union_scalar_condition_with_string_literal_takes_then() {
        let cond = PhpType::parse("($key is (array|string) ? array-key : ObjectKey)");
        let params = [param("$key")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let tpl = TemplateContext::with_params(&[]);
        let resolved = resolve_conditional_with_text_args_and_defaults(
            &cond, &params, "'field'", None, None, loader, &tpl,
        )
        .map(|t| t.to_string());
        assert_eq!(resolved.as_deref(), Some("array-key"));
    }

    /// An `is string` condition whose argument is a non-literal expression
    /// (a method-call chain) is decided from the argument's resolved type
    /// via the resolver, taking the then-branch when it is a string.
    #[test]
    fn is_string_condition_uses_resolved_arg_type() {
        let cond = PhpType::parse("($subject is string ? string : list<string>)");
        let params = [param("$subject")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let resolver = |t: &str| {
            if t == "$obj->toHtml()" {
                Some(PhpType::named(atom("string")))
            } else {
                None
            }
        };
        let tpl = TemplateContext {
            defaults: None,
            params: &[],
            arg_type_resolver: Some(&resolver),
        };
        let resolved = resolve_conditional_with_text_args_and_defaults(
            &cond,
            &params,
            "$obj->toHtml()",
            None,
            None,
            loader,
            &tpl,
        )
        .map(|t| t.to_string());
        assert_eq!(resolved.as_deref(), Some("string"));
    }

    #[test]
    fn scalar_conditions_classify_resolved_int_and_float_literals() {
        let params = [param("$value")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;

        for (condition_type, literal, expected) in [
            ("int", PhpType::literal_int("1"), "IntBranch"),
            ("float", PhpType::literal_float("1.5"), "FloatBranch"),
        ] {
            let cond = PhpType::parse(&format!(
                "($value is {condition_type} ? {expected} : ElseBranch)"
            ));
            let resolver = |_: &str| Some(literal.clone());
            let tpl = TemplateContext {
                defaults: None,
                params: &[],
                arg_type_resolver: Some(&resolver),
            };
            let resolved = resolve_conditional_with_text_args_and_defaults(
                &cond, &params, "$value", None, None, loader, &tpl,
            );
            assert_eq!(
                resolved.as_ref().map(ToString::to_string).as_deref(),
                Some(expected)
            );
        }
    }

    /// When the argument's type cannot be resolved (e.g. a magic property
    /// chain that resolves to `mixed`) an `is string` condition is genuinely
    /// undecidable, so the result is the union of both branches rather than a
    /// commitment to the else branch.
    #[test]
    fn undecidable_is_string_condition_unions_branches() {
        let cond = PhpType::parse("($subject is string ? string : list<string>)");
        let params = [param("$subject")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        // Resolver that cannot resolve the argument (returns None).
        let resolver = |_: &str| None;
        let tpl = TemplateContext {
            defaults: None,
            params: &[],
            arg_type_resolver: Some(&resolver),
        };
        let resolved = resolve_conditional_with_text_args_and_defaults(
            &cond,
            &params,
            "$obj->magic->toHtml()",
            None,
            None,
            loader,
            &tpl,
        )
        .map(|t| t.to_string());
        assert_eq!(resolved.as_deref(), Some("string|list<string>"));
    }

    /// An argument whose resolved type is mutually exclusive with the
    /// condition (a closure against `array|string`) takes the else branch,
    /// not the union — this keeps `Collection::groupBy(fn …)` resolving to
    /// its non-array key type.
    #[test]
    fn closure_arg_refutes_array_or_string_condition() {
        let cond = PhpType::parse("($groupBy is (array|string) ? array-key : Value)");
        let params = [param("$groupBy")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let resolver = |_: &str| Some(PhpType::named(atom("Closure")));
        let tpl = TemplateContext {
            defaults: None,
            params: &[],
            arg_type_resolver: Some(&resolver),
        };
        let resolved = resolve_conditional_with_text_args_and_defaults(
            &cond,
            &params,
            "fn($x) => $x->value",
            None,
            None,
            loader,
            &tpl,
        )
        .map(|t| t.to_string());
        assert_eq!(resolved.as_deref(), Some("Value"));
    }

    /// Without a resolver an unclassifiable `is string` argument falls
    /// through to the else branch (unchanged conservative default).
    #[test]
    fn is_string_condition_without_resolver_falls_to_else() {
        let cond = PhpType::parse("($subject is string ? string : list<string>)");
        let params = [param("$subject")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let tpl = TemplateContext::with_params(&[]);
        let resolved = resolve_conditional_with_text_args_and_defaults(
            &cond,
            &params,
            "$obj->toHtml()",
            None,
            None,
            loader,
            &tpl,
        )
        .map(|t| t.to_string());
        assert_eq!(resolved.as_deref(), Some("list<string>"));
    }

    /// A conditional nested inside a generic wrapper is collapsed against
    /// the call arguments rather than surviving raw.
    #[test]
    fn nested_conditional_in_generic_is_evaluated() {
        let ty =
            PhpType::parse("Collection<($key is (array|string) ? array-key : ObjectKey), Value>");
        assert!(ty.contains_conditional());
        let params = [param("$key")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        let tpl = TemplateContext::with_params(&[]);
        let evaluated =
            evaluate_nested_conditionals_text(&ty, &params, "'field'", None, None, loader, &tpl);
        assert!(!evaluated.contains_conditional());
        assert_eq!(evaluated.to_string(), "Collection<array-key, Value>");
    }

    /// A multi-level conditional whose branches are resolved via a template
    /// default (as in Laravel's `groupBy`) collapses fully — the template
    /// default resolver returns a branch that is itself a conditional, and
    /// the evaluator must keep collapsing rather than leaving a raw residue.
    #[test]
    fn nested_template_default_conditional_collapses_fully() {
        // static<($g is array|string ? array-key
        //   : (T is \UnitEnum ? array-key : (T is \Stringable ? string : T))), V>
        let ty = PhpType::parse(
            "Collection<($g is (array|string) ? array-key : (T is \\UnitEnum ? array-key : (T is \\Stringable ? string : T))), V>",
        );
        let params = [param("$g")];
        let loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>> = &|_| None;
        // The closure return type binds T to a nullable string key. As in the
        // real call path, template substitution runs before the evaluator, so
        // every `T` in a branch position is already concrete; only the `T is
        // …` condition subjects (carried in the conditional's `param` field)
        // remain and are evaluated via the template defaults.
        let mut defaults = HashMap::new();
        defaults.insert(
            "T".to_string(),
            PhpType::union(vec![
                PhpType::named(atom("string")),
                PhpType::named(atom("null")),
            ]),
        );
        let ty = ty.substitute(&defaults);
        assert!(ty.contains_conditional());
        let resolver = |_: &str| Some(PhpType::named(atom("Closure")));
        let tpl = TemplateContext {
            defaults: Some(&defaults),
            params: &[crate::atom::atom("T")],
            arg_type_resolver: Some(&resolver),
        };
        let evaluated = evaluate_nested_conditionals_text(
            &ty,
            &params,
            "fn($x) => $x->value",
            None,
            None,
            loader,
            &tpl,
        );
        assert!(
            !evaluated.contains_conditional(),
            "residual conditional left in {evaluated}"
        );
        assert_eq!(evaluated.to_string(), "Collection<string|null, V>");
    }
}
