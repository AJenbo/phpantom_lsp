/// Type narrowing for variable resolution.
///
/// This module contains the logic for narrowing a variable's type based on
/// runtime checks that appear before the cursor position.  Supported
/// patterns include:
///
///   - `if ($var instanceof ClassName)` — narrows inside the then-body
///   - `if (!$var instanceof ClassName)` — narrows inside the else-body
///   - `is_a($var, ClassName::class)` — equivalent to instanceof
///   - `get_class($var) === ClassName::class` — exact class identity check
///   - `$var::class === ClassName::class` — exact class identity check
///   - `assert($var instanceof ClassName)` — unconditional narrowing
///   - `@phpstan-assert` / `@psalm-assert` — custom type guard functions
///   - `match(true) { $var instanceof Foo => … }` — match-arm narrowing
///   - `$var instanceof Foo ? $var->method() : …` — ternary narrowing
///   - `$var instanceof Foo && $var->method()` — inline `&&` narrowing
///     (the RHS of `&&` sees the narrowed type from the LHS)
///   - Guard clauses: `if (!$var instanceof Foo) { return; }` — narrows
///     after the if block when the body unconditionally exits via
///     `return`, `throw`, `continue`, or `break`.
///   - `in_array($var, $haystack, true)` — narrows `$var` to the
///     haystack's element type when the third argument is `true`.
///   - `is_array($var)` — narrows to only the array-like members of a
///     union type, preserving generic element types from PHPDoc.
///   - `is_string($var)`, `is_int($var)`, `is_bool($var)`, etc. —
///     narrows to the corresponding scalar type.
use crate::php_type::PhpType;
use crate::types::{AssertionKind, ClassInfo, ParameterInfo, ResolvedType, TypeAssertion};
use mago_span::HasSpan;
use mago_syntax::ast::*;

use super::conditional::extract_class_string_from_expr;
use crate::completion::resolver::VarResolutionCtx;

/// Resolve a list of class name strings into a deduplicated `Vec<ClassInfo>`.
///
/// This is a shared helper for the compound instanceof/assert narrowing
/// patterns that produce a union of classes from multiple branches.
/// Uses `PhpType::Named` directly instead of `PhpType::parse` since
/// the class names are always simple identifiers extracted from
/// instanceof expressions.
fn resolve_class_names_to_union(classes: &[String], ctx: &VarResolutionCtx<'_>) -> Vec<ClassInfo> {
    let mut union = Vec::new();
    for cls_name in classes {
        let ty = PhpType::Named(cls_name.clone());
        let resolved = super::resolution::type_hint_to_classes_typed(
            &ty,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        for cls in resolved {
            if !union.iter().any(|c: &ClassInfo| c.name == cls.name) {
                union.push(cls);
            }
        }
    }
    union
}

/// Convert an AST expression to a subject key string for narrowing comparison.
///
/// Handles:
/// - `$var` → `"$var"`
/// - `$this->prop` → `"$this->prop"`
/// - `$this?->prop` → `"$this->prop"` (null-safe normalised)
///
/// Returns `None` for expressions that are not supported as narrowing subjects.
pub(in crate::completion) fn expr_to_subject_key(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => Some(dv.name.to_string()),
        Expression::Access(Access::Property(pa)) => {
            let obj = expr_to_subject_key(pa.object)?;
            if let ClassLikeMemberSelector::Identifier(ident) = &pa.property {
                Some(format!("{}->{}", obj, ident.value))
            } else {
                None
            }
        }
        Expression::Access(Access::NullSafeProperty(pa)) => {
            let obj = expr_to_subject_key(pa.object)?;
            if let ClassLikeMemberSelector::Identifier(ident) = &pa.property {
                Some(format!("{}->{}", obj, ident.value))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if `condition` is `$var instanceof ClassName` (possibly
/// parenthesised or negated) where the variable matches `ctx.var_name`.
///
/// If the cursor falls inside `body_span`:
///   - positive match → narrow `results` to only the instanceof class
///   - negated match (`!($var instanceof ClassName)`) → *exclude* the
///     class from the current candidates
pub(in crate::completion) fn try_apply_instanceof_narrowing(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }

    // ── Compound OR: `$x instanceof A || $x instanceof B` ──────────
    // Each branch that matches adds its class to the results (union).
    // This also handles untyped variables: if `results` is empty and
    // both branches match, the variable becomes `A|B`.
    //
    // We resolve all classes first and then replace `results` in one
    // shot, because `apply_instanceof_inclusion` clears results on
    // each call (correct for single-class narrowing, but wrong when
    // building a union from multiple OR branches).
    if let Some(classes) = try_extract_compound_or_instanceof(condition, ctx.var_name)
        && !classes.is_empty()
    {
        let union = resolve_class_names_to_union(&classes, ctx);
        if !union.is_empty() {
            results.clear();
            *results = union;
        }
        return;
    }

    // ── Compound AND: `$x instanceof A && $x instanceof B` ─────────
    // Both branches must hold, so each narrows further.  In practice
    // this means the variable is the intersection.  Since PHPantom
    // uses union-completion semantics, we add all matched classes.
    if let Some(classes) = try_extract_compound_and_instanceof(condition, ctx.var_name)
        && !classes.is_empty()
    {
        let union = resolve_class_names_to_union(&classes, ctx);
        if !union.is_empty() {
            results.clear();
            *results = union;
        }
        return;
    }

    if let Some(extraction) = try_extract_instanceof_with_negation(condition, ctx.var_name) {
        if extraction.negated {
            apply_instanceof_exclusion(&extraction.class_name, ctx, results);
        } else {
            apply_instanceof_inclusion(&extraction.class_name, extraction.exact, ctx, results);
        }
    }
}

/// Inverse of `try_apply_instanceof_narrowing` — used for the `else`
/// branch of an `if ($var instanceof ClassName)` check.
///
/// A positive instanceof in the condition means the variable is NOT
/// that class inside the else body (→ exclude), and vice-versa for a
/// negated condition (→ include only that class).
pub(in crate::completion) fn try_apply_instanceof_narrowing_inverse(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }

    // ── Compound OR inverse: after `if ($x instanceof A || $x instanceof B) { exit; }` ──
    // In the else branch, $x is neither A nor B → exclude both.
    if let Some(classes) = try_extract_compound_or_instanceof(condition, ctx.var_name)
        && !classes.is_empty()
    {
        for cls_name in &classes {
            apply_instanceof_exclusion(cls_name, ctx, results);
        }
        return;
    }

    // ── Compound AND inverse: after `if ($x instanceof A && $x instanceof B) { exit; }` ──
    // In the else branch, at least one doesn't hold.  Since we can't
    // precisely model "not (A and B)", we don't narrow.  Fall through.

    if let Some(extraction) = try_extract_instanceof_with_negation(condition, ctx.var_name) {
        // Flip the polarity: positive condition → exclude in else,
        // negated condition → include in else.
        if extraction.negated {
            apply_instanceof_inclusion(&extraction.class_name, extraction.exact, ctx, results);
        } else {
            apply_instanceof_exclusion(&extraction.class_name, ctx, results);
        }
    }
}

/// Replace `results` with only the resolved classes for `cls_name`.
/// Narrow `results` to include only classes matching `cls_name`.
///
/// When `exact` is `false` (the common `instanceof` / `is_a()` case),
/// existing results that are already subtypes of the narrowing class are
/// kept as-is because they are more specific and already satisfy the
/// check.  For example, if results = `[Zoo]` and we narrow to
/// `ZooBase`, `Zoo extends ZooBase` means `Zoo` is already more specific
/// so it is preserved.
///
/// When `exact` is `true` (`get_class($x) === Foo::class` or
/// `$x::class === Foo::class`), the variable is narrowed to exactly
/// that class regardless of the current results.
pub(in crate::completion) fn apply_instanceof_inclusion(
    cls_name: &str,
    exact: bool,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let parsed = PhpType::parse(cls_name);
    apply_instanceof_inclusion_typed(&parsed, exact, ctx, results);
}

/// Like [`apply_instanceof_inclusion`], but accepts a [`PhpType`] directly,
/// avoiding a string round-trip when the caller already has a parsed type.
pub(in crate::completion) fn apply_instanceof_inclusion_typed(
    ty: &PhpType,
    exact: bool,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let narrowed = super::resolution::type_hint_to_classes_typed(
        ty,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    if narrowed.is_empty() {
        // The instanceof target class could not be resolved (e.g. it
        // lives inside a phar that we cannot index).  The developer
        // wrote an explicit instanceof guard, so they clearly expect
        // the variable to have that type in this branch.  Rather than
        // keeping the un-narrowed type (which would cause false-
        // positive "unknown member" diagnostics for members that only
        // exist on the unresolvable subclass), clear the results so
        // the variable appears untyped.  Untyped subjects are
        // suppressed by the diagnostic engine, eliminating the false
        // positives without losing any information we actually had.
        results.clear();
        return;
    }

    // For non-exact checks (instanceof / is_a), keep existing results
    // that are already subtypes of the narrowing class.  For example,
    // if results = [Zoo] and we narrow to ZooBase, Zoo extends ZooBase
    // so Zoo is already more specific — keep it.
    if !exact {
        let already_subtypes: Vec<ClassInfo> = results
            .iter()
            .filter(|r| {
                narrowed
                    .iter()
                    .any(|n| crate::util::is_subtype_of_names(&r.fqn(), &n.fqn(), ctx.class_loader))
            })
            .cloned()
            .collect();

        if !already_subtypes.is_empty() {
            // All kept results are already subtypes of the narrowing
            // class, so the instanceof check is satisfied without
            // widening.
            *results = already_subtypes;
            return;
        }
    }

    // When the narrowed class is a subtype of (i.e. more specific than)
    // an existing result, replace with the narrowed type.  For example,
    // results = [Animal] narrowed to Dog (Dog extends Animal) → [Dog].
    if !exact {
        let narrowed_is_more_specific = narrowed.iter().any(|n| {
            results
                .iter()
                .any(|r| crate::util::is_subtype_of_names(&n.fqn(), &r.fqn(), ctx.class_loader))
        });

        if !narrowed_is_more_specific && results.len() == 1 {
            // Neither direction holds — the types are unrelated.
            // This only makes sense as an intersection when the
            // variable has a single definite type (not a union from
            // conditional branches) and at least one side is an
            // interface, because a concrete object can implement an
            // interface without it appearing in the declared class
            // hierarchy (e.g. mock objects, dynamic proxies).
            //
            // When `results` is a union (len > 1) the instanceof
            // filters the union rather than intersecting, so we fall
            // through to the replacement path below.
            let any_interface = narrowed
                .iter()
                .chain(results.iter())
                .any(|c| c.kind == crate::types::ClassLikeKind::Interface);

            if any_interface {
                // Keep both (intersection semantics) so that members
                // from all types are available.
                for cls in narrowed {
                    if !results.iter().any(|c| c.fqn() == cls.fqn()) {
                        results.push(cls);
                    }
                }
                return;
            }
        }
    }

    // Exact identity check, or narrowed type is more specific —
    // replace with the narrowed type.
    results.clear();
    for cls in narrowed {
        if !results.iter().any(|c| c.name == cls.name) {
            results.push(cls);
        }
    }
}

/// Remove the resolved classes for `cls_name` from `results`.
pub(in crate::completion) fn apply_instanceof_exclusion(
    cls_name: &str,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let parsed = PhpType::parse(cls_name);
    apply_instanceof_exclusion_typed(&parsed, ctx, results);
}

/// Like [`apply_instanceof_exclusion`], but accepts a [`PhpType`] directly.
pub(in crate::completion) fn apply_instanceof_exclusion_typed(
    ty: &PhpType,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let excluded = super::resolution::type_hint_to_classes_typed(
        ty,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    if !excluded.is_empty() {
        results.retain(|r| !excluded.iter().any(|e| e.name == r.name));
    }
}

/// If `expr` is `$var instanceof ClassName` and the variable name
/// matches `var_name`, return the class name.
///
/// Handles parenthesised expressions recursively so that
/// `($var instanceof Foo)` also works.
pub(in crate::completion) fn try_extract_instanceof<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<String> {
    match expr {
        Expression::Parenthesized(inner) => try_extract_instanceof(inner.expression, var_name),
        Expression::Binary(bin) if bin.operator.is_instanceof() => {
            // LHS must be our variable or property access
            let lhs_name = expr_to_subject_key(bin.lhs)?;
            if lhs_name != var_name {
                return None;
            }
            // RHS is the class name
            match bin.rhs {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                Expression::Self_(_) => Some("self".to_string()),
                Expression::Static(_) => Some("static".to_string()),
                Expression::Parent(_) => Some("parent".to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Like `try_extract_instanceof` but also detects negation.
///
/// Returns `Some((class_name, negated))` where `negated` is `true`
/// when the expression is `!($var instanceof ClassName)` or
/// `!$var instanceof ClassName` (PHP precedence: `instanceof` binds
/// tighter than `!`, so both forms are equivalent).
///
/// Also handles:
///   - `is_a($var, ClassName::class)` — treated as equivalent to instanceof
///   - `get_class($var) === ClassName::class` or `==` — exact class match
///   - `$var::class === ClassName::class` or `==` — exact class match
///
/// Handles arbitrary parenthesisation.
/// Result of extracting an instanceof-style check from an expression.
///
/// - `class_name`: the class being checked against
/// - `negated`: `true` when the check is negated (e.g. `!($x instanceof Foo)`)
/// - `exact`: `true` for exact class identity checks (`get_class($x) === Foo::class`,
///   `$x::class === Foo::class`) where subclasses should NOT be preserved.
///   `false` for `instanceof` / `is_a()` checks where a more-specific subtype
///   in the current results should be kept.
pub(in crate::completion) struct InstanceofExtraction {
    pub class_name: String,
    pub negated: bool,
    pub exact: bool,
}

pub(in crate::completion) fn try_extract_instanceof_with_negation<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<InstanceofExtraction> {
    match expr {
        Expression::Parenthesized(inner) => {
            try_extract_instanceof_with_negation(inner.expression, var_name)
        }
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            // `!expr` — recurse so that `!!expr` (double negation) and
            // deeper chains like `!!!expr` are handled correctly: each
            // `!` flips the negation flag.
            try_extract_instanceof_with_negation(prefix.operand, var_name).map(|mut e| {
                e.negated = !e.negated;
                e
            })
        }
        _ => {
            try_extract_instanceof(expr, var_name)
                .map(|cls| InstanceofExtraction {
                    class_name: cls,
                    negated: false,
                    exact: false,
                })
                .or_else(|| {
                    // `is_a($var, ClassName::class)` — equivalent to instanceof
                    try_extract_is_a(expr, var_name).map(|cls| InstanceofExtraction {
                        class_name: cls,
                        negated: false,
                        exact: false,
                    })
                })
                .or_else(|| {
                    // `get_class($var) === ClassName::class` or
                    // `$var::class === ClassName::class` — exact class match
                    try_extract_class_identity_check(expr, var_name).map(|(cls, neg)| {
                        InstanceofExtraction {
                            class_name: cls,
                            negated: neg,
                            exact: true,
                        }
                    })
                })
        }
    }
}

/// Detect `is_a($var, ClassName::class)` — semantically equivalent to
/// `$var instanceof ClassName`.
///
/// Returns the class name if the pattern matches.
fn try_extract_is_a<'b>(expr: &'b Expression<'b>, var_name: &str) -> Option<String> {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value(),
            _ => return None,
        };
        if func_name != "is_a" {
            return None;
        }
        let args: Vec<_> = func_call.argument_list.arguments.iter().collect();
        if args.len() < 2 {
            return None;
        }
        // First argument must be our variable
        let first_expr = match &args[0] {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        let first_var = match first_expr {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => return None,
        };
        if first_var != var_name {
            return None;
        }
        // Second argument should be ClassName::class
        let second_expr = match &args[1] {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        extract_class_string_from_expr(second_expr)
    } else {
        None
    }
}

/// Detect `get_class($var) === ClassName::class` (or `==`) and
/// `$var::class === ClassName::class` (or `==`).
///
/// Returns `Some((class_name, negated))` where `negated` is `true`
/// for `!==` and `!=` operators.
fn try_extract_class_identity_check<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<(String, bool)> {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Binary(bin) = expr {
        let negated = match &bin.operator {
            BinaryOperator::Identical(_) | BinaryOperator::Equal(_) => false,
            BinaryOperator::NotIdentical(_) | BinaryOperator::NotEqual(_) => true,
            _ => return None,
        };
        // Try both orders: class-check == ClassName::class and
        // ClassName::class == class-check
        if let Some(cls) = match_class_identity_pair(bin.lhs, bin.rhs, var_name) {
            return Some((cls, negated));
        }
        if let Some(cls) = match_class_identity_pair(bin.rhs, bin.lhs, var_name) {
            return Some((cls, negated));
        }
    }
    None
}

/// Helper for `try_extract_class_identity_check`.
///
/// Checks if `lhs` is a class-identity expression for `var_name`
/// (`get_class($var)` or `$var::class`) and `rhs` is a
/// `ClassName::class` constant.
fn match_class_identity_pair<'b>(
    lhs: &'b Expression<'b>,
    rhs: &'b Expression<'b>,
    var_name: &str,
) -> Option<String> {
    let is_class_of_var =
        is_get_class_of_var(lhs, var_name) || is_var_class_constant(lhs, var_name);
    if !is_class_of_var {
        return None;
    }
    extract_class_string_from_expr(rhs)
}

/// Check if `expr` is `get_class($var)` where the variable matches.
fn is_get_class_of_var(expr: &Expression<'_>, var_name: &str) -> bool {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value(),
            _ => return false,
        };
        if func_name != "get_class" {
            return false;
        }
        if let Some(first_arg) = func_call.argument_list.arguments.iter().next() {
            let arg_expr = match first_arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
                return dv.name == var_name;
            }
        }
    }
    false
}

/// Check if `expr` is `$var::class` where the variable matches.
fn is_var_class_constant(expr: &Expression<'_>, var_name: &str) -> bool {
    if let Expression::Access(Access::ClassConstant(cca)) = expr {
        // The class part must be our variable
        if let Expression::Variable(Variable::Direct(dv)) = cca.class {
            if dv.name != var_name {
                return false;
            }
            // The constant selector must be `class`
            if let ClassLikeConstantSelector::Identifier(ident) = &cca.constant {
                return ident.value == "class";
            }
        }
    }
    false
}

/// Resolved assertion metadata extracted from a function call or static
/// method call expression.
///
/// Produced by [`extract_call_assertions`] so that callers can apply
/// narrowing logic uniformly regardless of whether the call is
/// `myFunc($x)` or `Assert::check($x)`.
struct CallAssertionInfo<'a> {
    /// The `@phpstan-assert` / `@psalm-assert` annotations on the callee.
    assertions: &'a [TypeAssertion],
    /// The callee's parameter list (used to map assertion `$param` names
    /// to positional argument indices).
    parameters: &'a [ParameterInfo],
    /// The call-site argument list.
    argument_list: &'a ArgumentList<'a>,
    /// Template parameter names from the callee's `@template` tags.
    template_params: &'a [String],
    /// Template parameter → parameter name bindings (e.g. `("T", "$class")`).
    template_bindings: &'a [(String, String)],
}

/// Try to extract assertion metadata from a call expression.
///
/// Handles two call forms:
///   - `Call::Function(func_call)` — standalone function call, resolved
///     through `ctx.function_loader`.
///   - `Call::StaticMethod(static_call)` — static method call like
///     `Assert::instanceOf(…)`, resolved through `ctx.class_loader`.
///
/// Returns `None` when the call is not one of these forms, or when the
/// callee cannot be resolved.
fn extract_call_assertions<'a>(
    call: &'a Call<'a>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<CallAssertionInfo<'a>> {
    match call {
        Call::Function(func_call) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return None,
            };
            let func_info = ctx.function_loader()?(&func_name)?;
            if func_info.type_assertions.is_empty() {
                return None;
            }
            // SAFETY: We leak the FunctionInfo to get a stable reference.
            // This is acceptable because narrowing runs once per completion
            // request and the allocation is small.
            let func_info = Box::leak(Box::new(func_info));
            Some(CallAssertionInfo {
                assertions: &func_info.type_assertions,
                parameters: &func_info.parameters,
                argument_list: &func_call.argument_list,
                template_params: &func_info.template_params,
                template_bindings: &func_info.template_bindings,
            })
        }
        Call::StaticMethod(static_call) => {
            let class_name = match static_call.class {
                Expression::Identifier(ident) => ident.value().to_string(),
                Expression::Self_(_) | Expression::Static(_) => ctx.current_class.name.clone(),
                _ => return None,
            };
            let method_name = match &static_call.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
                _ => return None,
            };
            let class_info = (ctx.class_loader)(&class_name)?;
            let method = class_info
                .methods
                .iter()
                .find(|m| m.name == method_name && m.is_static)?
                .clone();
            if method.type_assertions.is_empty() {
                return None;
            }
            // Leak MethodInfo to get a stable reference for the duration
            // of this narrowing pass.
            let method = Box::leak(Box::new(method));
            Some(CallAssertionInfo {
                assertions: &method.type_assertions,
                parameters: &method.parameters,
                argument_list: &static_call.argument_list,
                template_params: &method.template_params,
                template_bindings: &method.template_bindings,
            })
        }
        _ => None,
    }
}

/// Apply narrowing from `@phpstan-assert` / `@psalm-assert` annotations
/// on a function or static method called as a standalone expression statement.
///
/// Only `AssertionKind::Always` assertions are applied here — the
/// `IfTrue` / `IfFalse` variants are handled by
/// `try_apply_assert_condition_narrowing`.
pub(in crate::completion) fn try_apply_custom_assert_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    let call = match expr {
        Expression::Call(c) => c,
        _ => return,
    };
    let info = match extract_call_assertions(call, ctx) {
        Some(info) => info,
        None => return,
    };
    for assertion in info.assertions {
        if assertion.kind != AssertionKind::Always {
            continue;
        }
        if let Some(arg_var) =
            find_assertion_arg_variable(info.argument_list, &assertion.param_name, info.parameters)
            && arg_var == ctx.var_name
        {
            // Resolve the asserted type.  When the type is a template
            // parameter (e.g. `ExpectedType` from `@phpstan-assert
            // ExpectedType $actual`), substitute it using the call-site
            // argument bound via `class-string<T>`.
            let effective_type =
                resolve_assertion_template_type(&assertion.asserted_type, &info, ctx);

            if assertion.negated {
                apply_instanceof_exclusion_typed(&effective_type, ctx, results);
            } else {
                apply_instanceof_inclusion_typed(&effective_type, false, ctx, results);
            }
        }
    }
}

/// If `asserted_type` is a template parameter name, resolve it to a
/// concrete type using the call-site arguments and template bindings.
///
/// For example, given:
///   `@template ExpectedType of object`
///   `@param class-string<ExpectedType> $expected`
///   `@phpstan-assert ExpectedType $actual`
///   Call: `Assert::assertFoobar(Foobar::class, $obj)`
///
/// The asserted type `ExpectedType` is resolved to `Foobar` by:
///   1. Finding `ExpectedType` in `template_params`
///   2. Looking up its binding: `("ExpectedType", "$expected")`
///   3. Finding positional index of `$expected` in `parameters`
///   4. Reading the call-site argument at that index: `Foobar::class`
///   5. Extracting the class name `Foobar`
///
/// Returns the original type unchanged when it is not a template param
/// or when the concrete type cannot be determined.
fn resolve_assertion_template_type(
    asserted_type: &PhpType,
    info: &CallAssertionInfo<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> PhpType {
    // Check if the asserted type is a template parameter.
    let tpl_name = match asserted_type {
        PhpType::Named(n) if info.template_params.iter().any(|t| t == n) => n.as_str(),
        _ => return asserted_type.clone(),
    };

    // Find the parameter name that binds this template param.
    let bound_param = info
        .template_bindings
        .iter()
        .find(|(tpl, _)| tpl == tpl_name)
        .map(|(_, param)| param.as_str());

    let bound_param = match bound_param {
        Some(p) => p,
        None => return asserted_type.clone(),
    };

    // Find the positional index of that parameter.
    let param_idx = match info.parameters.iter().position(|p| p.name == bound_param) {
        Some(idx) => idx,
        None => return asserted_type.clone(),
    };

    // Get the call-site argument at that position.
    let arg_expr = match info.argument_list.arguments.iter().nth(param_idx) {
        Some(Argument::Positional(pos)) => pos.value,
        Some(Argument::Named(named)) => named.value,
        None => return asserted_type.clone(),
    };

    // Try to extract a class name from the argument expression.
    if let Some(class_name) = extract_class_string_from_expr(arg_expr) {
        return PhpType::Named(class_name);
    }

    // Try to resolve a variable argument's class-string type.
    if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
        let var_name = dv.name.to_string();
        let targets =
            crate::completion::variable::class_string_resolution::resolve_class_string_targets(
                &var_name,
                ctx.current_class,
                ctx.all_classes,
                ctx.content,
                ctx.cursor_offset,
                ctx.class_loader,
            );
        if let Some(first) = targets.into_iter().next() {
            return PhpType::Named(first.name);
        }
    }

    asserted_type.clone()
}

/// Apply narrowing from `@phpstan-assert-if-true` / `-if-false`
/// annotations on a function call used as an `if` / `while` condition.
///
/// * `inverted == false` → we're in the then-body (or while-body):
///   apply `IfTrue` assertions (and `IfFalse` if the condition is negated).
/// * `inverted == true` → we're in the else-body:
///   apply `IfFalse` assertions (and `IfTrue` if the condition is negated).
pub(in crate::completion) fn try_apply_assert_condition_narrowing(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    inverted: bool,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }

    // Unwrap parentheses and detect negation (`!func($var)`)
    let (func_call_expr, condition_negated) = unwrap_condition_negation(condition);

    let call = match func_call_expr {
        Expression::Call(c) => c,
        _ => return,
    };

    // Determine whether the function returned true in this branch.
    //
    // - then-body (inverted=false), no negation  → function returned true
    // - then-body (inverted=false), negated       → function returned false
    // - else-body (inverted=true),  no negation  → function returned false
    // - else-body (inverted=true),  negated       → function returned true
    let function_returned_true = !(inverted ^ condition_negated);

    if let Some(info) = extract_call_assertions(call, ctx) {
        for assertion in info.assertions {
            // Determine if this assertion's condition is satisfied in this
            // branch.  IfTrue assertions apply positively when the function
            // returned true; IfFalse assertions apply positively when the
            // function returned false.  In the opposite branch, we apply
            // the *inverse* (exclude instead of include, and vice-versa).
            let applies_positively = match assertion.kind {
                AssertionKind::IfTrue => function_returned_true,
                AssertionKind::IfFalse => !function_returned_true,
                AssertionKind::Always => continue, // handled elsewhere
            };

            if let Some(arg_var) = find_assertion_arg_variable(
                info.argument_list,
                &assertion.param_name,
                info.parameters,
            ) && arg_var == ctx.var_name
            {
                // XOR the assertion's own negation with whether we're in the
                // opposite branch: positive + non-negated → include,
                // positive + negated → exclude, opposite + non-negated → exclude,
                // opposite + negated → include.
                let should_exclude = assertion.negated ^ !applies_positively;
                if should_exclude {
                    apply_instanceof_exclusion_typed(&assertion.asserted_type, ctx, results);
                } else {
                    apply_instanceof_inclusion_typed(&assertion.asserted_type, false, ctx, results);
                }
            }
        }
    } else {
        // Handle instance method calls: `$var->method()` where the method
        // carries `@phpstan-assert-if-true $this` (or the -if-false / psalm
        // variants).  The receiver variable itself is the assertion subject.
        apply_this_assert_condition_narrowing(call, ctx, results, function_returned_true);
    }
}

/// Apply `@phpstan-assert-if-true $this` / `@phpstan-assert-if-false $this`
/// narrowing for instance method calls used as an `if` condition.
///
/// Handles the pattern:
/// ```php
/// if ($app->isTestApp()) {
///     $app->testMethod(); // $app narrowed to TestApplication
/// }
/// ```
/// where `isTestApp()` is annotated with
/// `@phpstan-assert-if-true \TestApplication $this`.
///
/// The receiver's current known types (from `results`) are searched for a
/// matching method with `$this` assertions.  Assertions are collected first
/// and applied after the iteration to avoid borrow-check conflicts.
fn apply_this_assert_condition_narrowing(
    call: &Call<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    function_returned_true: bool,
) {
    // Extract receiver expression and method name from the call.
    let (receiver, method_name) = match call {
        Call::Method(mc) => (
            mc.object,
            match &mc.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value,
                _ => return,
            },
        ),
        Call::NullSafeMethod(mc) => (
            mc.object,
            match &mc.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value,
                _ => return,
            },
        ),
        _ => return,
    };

    // The receiver must be the variable we are currently narrowing.
    let receiver_key = match expr_to_subject_key(receiver) {
        Some(k) => k,
        None => return,
    };
    if receiver_key != ctx.var_name {
        return;
    }

    // Collect (asserted_type, should_exclude) pairs from every current
    // candidate class that declares the method with a $this assertion.
    // We collect before mutating `results` to avoid borrow conflicts.
    let mut to_apply: Vec<(PhpType, bool)> = Vec::new();
    for class_info in results.iter() {
        let method = class_info
            .methods
            .iter()
            .find(|m| m.name == method_name && !m.is_static);
        if let Some(method) = method {
            for assertion in &method.type_assertions {
                if assertion.param_name != "$this" {
                    continue;
                }
                let applies_positively = match assertion.kind {
                    AssertionKind::IfTrue => function_returned_true,
                    AssertionKind::IfFalse => !function_returned_true,
                    AssertionKind::Always => continue,
                };
                let should_exclude = assertion.negated ^ !applies_positively;
                to_apply.push((assertion.asserted_type.clone(), should_exclude));
            }
        }
    }

    for (asserted_type, should_exclude) in to_apply {
        if should_exclude {
            apply_instanceof_exclusion_typed(&asserted_type, ctx, results);
        } else {
            apply_instanceof_inclusion_typed(&asserted_type, false, ctx, results);
        }
    }
}

/// Unwrap parentheses and a single `!` prefix from a condition,
/// returning `(inner_expr, negated)`.
pub(in crate::completion) fn unwrap_condition_negation<'b>(
    expr: &'b Expression<'b>,
) -> (&'b Expression<'b>, bool) {
    match expr {
        Expression::Parenthesized(inner) => unwrap_condition_negation(inner.expression),
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            let (inner, already_negated) = unwrap_condition_negation(prefix.operand);
            (inner, !already_negated)
        }
        _ => (expr, false),
    }
}

/// Given a function's argument list and a parameter name (with `$`
/// prefix), find the variable name passed at that parameter's position.
///
/// Returns `Some("$varName")` if the argument at the matching position
/// is a simple direct variable.
pub(in crate::completion) fn find_assertion_arg_variable(
    argument_list: &ArgumentList<'_>,
    param_name: &str,
    parameters: &[crate::types::ParameterInfo],
) -> Option<String> {
    // Find the parameter index
    let param_idx = parameters.iter().position(|p| p.name == param_name)?;

    // Get the argument at that position
    let arg = argument_list.arguments.iter().nth(param_idx)?;
    let arg_expr = match arg {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    };

    // The argument must be a simple variable
    match arg_expr {
        Expression::Variable(Variable::Direct(dv)) => Some(dv.name.to_string()),
        _ => None,
    }
}

/// If `expr` is `assert($var instanceof ClassName)` (or the negated
/// form `assert(!$var instanceof ClassName)`), narrow or exclude
/// `results` accordingly.
///
/// Unlike `if`-based narrowing which is scoped to the block body,
/// `assert()` narrows unconditionally for all subsequent code in the
/// same scope — the statement being before the cursor is already
/// guaranteed by the caller.
pub(in crate::completion) fn try_apply_assert_instanceof_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    // ── Compound OR inside assert: `assert($x instanceof A || $x instanceof B)` ──
    if let Some(classes) = try_extract_assert_compound_or_instanceof(expr, ctx.var_name)
        && !classes.is_empty()
    {
        let union = resolve_class_names_to_union(&classes, ctx);
        if !union.is_empty() {
            results.clear();
            *results = union;
        }
        return;
    }

    if let Some(extraction) = try_extract_assert_instanceof(expr, ctx.var_name) {
        if extraction.negated {
            apply_instanceof_exclusion(&extraction.class_name, ctx, results);
        } else {
            apply_instanceof_inclusion(&extraction.class_name, extraction.exact, ctx, results);
        }
    }
}

/// If `expr` is `assert($var instanceof ClassName)` (or the negated
/// form), return `Some((class_name, negated))`.
///
/// Supports parenthesised inner expressions and the function name
/// `assert`.
fn try_extract_assert_instanceof<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<InstanceofExtraction> {
    // Unwrap parenthesised wrapper on the whole expression
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name_raw = match func_call.function {
            Expression::Identifier(ident) => ident.value(),
            _ => return None,
        };
        let func_name = func_name_raw.strip_prefix('\\').unwrap_or(func_name_raw);
        if !func_name.eq_ignore_ascii_case("assert") {
            return None;
        }
        // The first argument should be the instanceof expression
        // (possibly negated), or is_a / class-identity check
        if let Some(first_arg) = func_call.argument_list.arguments.iter().next() {
            let arg_expr = match first_arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            return try_extract_instanceof_with_negation(arg_expr, var_name);
        }
    }
    None
}

/// Extract compound OR instanceof class names from inside an `assert()` call.
///
/// For `assert($x instanceof A || $x instanceof B)`, returns
/// `Some(["A", "B"])`.  Returns `None` if the expression is not an
/// `assert()` call whose argument is a compound OR of instanceof checks.
fn try_extract_assert_compound_or_instanceof<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<Vec<String>> {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name_raw = match func_call.function {
            Expression::Identifier(ident) => ident.value(),
            _ => return None,
        };
        let func_name = func_name_raw.strip_prefix('\\').unwrap_or(func_name_raw);
        if !func_name.eq_ignore_ascii_case("assert") {
            return None;
        }
        if let Some(first_arg) = func_call.argument_list.arguments.iter().next() {
            let arg_expr = match first_arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            return try_extract_compound_or_instanceof(arg_expr, var_name);
        }
    }
    None
}

/// Check if the cursor is inside a `match (true)` arm whose
/// condition is `$var instanceof ClassName` and, if so, narrow
/// the results for the arm body.
///
/// Supports patterns like:
/// ```text
/// match (true) {
///     $value instanceof AdminUser => $value->doAdmin(),
///     //                             ^cursor here
/// };
/// ```
pub(in crate::completion) fn try_apply_match_true_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    // Unwrap parenthesised wrapper
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    let match_expr = match expr {
        Expression::Match(m) => m,
        // Also handle `$var = match(true) { … }`
        Expression::Assignment(a) => {
            if let Expression::Match(m) = a.rhs {
                m
            } else {
                return;
            }
        }
        _ => return,
    };
    // The subject must be `true` for instanceof conditions to make sense
    if !match_expr.expression.is_true() {
        return;
    }
    for arm in match_expr.arms.iter() {
        if let MatchArm::Expression(expr_arm) = arm {
            let body_span = expr_arm.expression.span();
            if ctx.cursor_offset < body_span.start.offset
                || ctx.cursor_offset > body_span.end.offset
            {
                continue;
            }
            // Check each condition in this arm (comma-separated)
            for condition in expr_arm.conditions.iter() {
                if let Some(extraction) =
                    try_extract_instanceof_with_negation(condition, ctx.var_name)
                {
                    if extraction.negated {
                        apply_instanceof_exclusion(&extraction.class_name, ctx, results);
                    } else {
                        apply_instanceof_inclusion(
                            &extraction.class_name,
                            extraction.exact,
                            ctx,
                            results,
                        );
                    }
                }
            }
        }
    }
}

/// Apply `instanceof` narrowing inside ternary (`?:`) expressions.
///
/// When the cursor falls inside a ternary whose condition is
/// `$var instanceof ClassName`:
///   - **then-branch** → narrow to `ClassName`
///   - **else-branch** → exclude `ClassName`
///
/// Negated conditions (`!$var instanceof Foo ? … : …`) flip the
/// polarity, just like `if`/`else`.
///
/// The function recursively walks the expression tree so that nested
/// ternaries and ternaries buried inside assignments, function
/// arguments, etc. are all handled.
pub(in crate::completion) fn try_apply_ternary_instanceof_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    match expr {
        Expression::Conditional(cond_expr) => {
            // Determine which branch (if any) the cursor is inside.
            let in_then = cond_expr.then.is_some_and(|then_expr| {
                let span = then_expr.span();
                ctx.cursor_offset >= span.start.offset && ctx.cursor_offset <= span.end.offset
            });
            let in_else = {
                let span = cond_expr.r#else.span();
                ctx.cursor_offset >= span.start.offset && ctx.cursor_offset <= span.end.offset
            };

            if in_then {
                if let Some(extraction) =
                    try_extract_instanceof_with_negation(cond_expr.condition, ctx.var_name)
                {
                    if extraction.negated {
                        apply_instanceof_exclusion(&extraction.class_name, ctx, results);
                    } else {
                        apply_instanceof_inclusion(
                            &extraction.class_name,
                            extraction.exact,
                            ctx,
                            results,
                        );
                    }
                }
            } else if in_else
                && let Some(extraction) =
                    try_extract_instanceof_with_negation(cond_expr.condition, ctx.var_name)
            {
                // Flip polarity for the else branch.
                if extraction.negated {
                    apply_instanceof_inclusion(
                        &extraction.class_name,
                        extraction.exact,
                        ctx,
                        results,
                    );
                } else {
                    apply_instanceof_exclusion(&extraction.class_name, ctx, results);
                }
            }

            // Recurse into whichever branch contains the cursor so
            // that nested ternaries are also narrowed.
            if let Some(then_expr) = cond_expr.then {
                try_apply_ternary_instanceof_narrowing(then_expr, ctx, results);
            }
            try_apply_ternary_instanceof_narrowing(cond_expr.r#else, ctx, results);
        }
        // Recurse through common wrapper expressions so ternaries
        // buried inside assignments, parentheses, binary ops, etc.
        // are still discovered.
        Expression::Parenthesized(inner) => {
            try_apply_ternary_instanceof_narrowing(inner.expression, ctx, results);
        }
        Expression::Assignment(assign) => {
            try_apply_ternary_instanceof_narrowing(assign.rhs, ctx, results);
        }
        Expression::Binary(bin) => {
            try_apply_ternary_instanceof_narrowing(bin.lhs, ctx, results);
            try_apply_ternary_instanceof_narrowing(bin.rhs, ctx, results);
        }
        Expression::UnaryPrefix(prefix) => {
            try_apply_ternary_instanceof_narrowing(prefix.operand, ctx, results);
        }
        Expression::UnaryPostfix(postfix) => {
            try_apply_ternary_instanceof_narrowing(postfix.operand, ctx, results);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => &fc.argument_list.arguments,
                Call::Method(mc) => &mc.argument_list.arguments,
                Call::NullSafeMethod(mc) => &mc.argument_list.arguments,
                Call::StaticMethod(sc) => &sc.argument_list.arguments,
            };
            for arg in args.iter() {
                let arg_expr = match arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                try_apply_ternary_instanceof_narrowing(arg_expr, ctx, results);
            }
        }
        _ => {}
    }
}

// ── Inline `&&` short-circuit narrowing ─────────────────────────
//
// In PHP, the right-hand side of `&&` is only evaluated when the
// left-hand side is truthy.  Therefore, if the LHS is an
// `instanceof` check, the RHS can see the narrowed type.
//
// Example:
//   if ($req instanceof LaravelRequest && $req->fullUrlIs('…'))
//                                          ^^^ cursor here
// The cursor is inside the *condition* (not the if-body), but
// `$req` should resolve to `LaravelRequest` because the LHS of
// `&&` guarantees it.
//
// This function recursively walks an expression tree and, when it
// finds a `&&` whose RHS contains the cursor, applies any
// instanceof narrowing from the LHS.

/// Apply instanceof narrowing from the left-hand side of `&&` when the
/// cursor falls inside the right-hand side.
///
/// This handles short-circuit semantics: in `A && B`, `B` is only
/// evaluated when `A` is truthy, so any `instanceof` check in `A`
/// narrows the variable's type within `B`.
///
/// The function recurses into the expression tree so that chained
/// `&&` expressions (`A && B && C`) and nested sub-expressions are
/// handled correctly.
pub(in crate::completion) fn try_apply_inline_and_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    match expr {
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            let rhs_span = bin.rhs.span();
            let cursor_in_rhs = ctx.cursor_offset >= rhs_span.start.offset
                && ctx.cursor_offset <= rhs_span.end.offset;

            if cursor_in_rhs {
                // The cursor is in the RHS — apply all instanceof
                // checks found in the LHS.  For chained `&&` the
                // LHS is itself a `&&`, so we collect recursively.
                apply_and_lhs_narrowing(bin.lhs, ctx, results);
            }

            // Recurse into both sides so that deeply nested `&&`
            // chains are handled (e.g. `A && B && C` parses as
            // `(A && B) && C`; when the cursor is in C we need to
            // also recurse into the LHS `(A && B)` for the case
            // where the cursor is in B).
            try_apply_inline_and_narrowing(bin.lhs, ctx, results);
            try_apply_inline_and_narrowing(bin.rhs, ctx, results);
        }
        // Recurse through common wrapper expressions so `&&` nodes
        // buried inside ternaries, assignments, other binary ops, etc.
        // are still discovered.
        Expression::Parenthesized(inner) => {
            try_apply_inline_and_narrowing(inner.expression, ctx, results);
        }
        Expression::Conditional(cond_expr) => {
            try_apply_inline_and_narrowing(cond_expr.condition, ctx, results);
            if let Some(then_expr) = cond_expr.then {
                try_apply_inline_and_narrowing(then_expr, ctx, results);
            }
            try_apply_inline_and_narrowing(cond_expr.r#else, ctx, results);
        }
        Expression::Assignment(assign) => {
            try_apply_inline_and_narrowing(assign.rhs, ctx, results);
        }
        Expression::Binary(bin) => {
            try_apply_inline_and_narrowing(bin.lhs, ctx, results);
            try_apply_inline_and_narrowing(bin.rhs, ctx, results);
        }
        Expression::UnaryPrefix(prefix) => {
            try_apply_inline_and_narrowing(prefix.operand, ctx, results);
        }
        Expression::UnaryPostfix(postfix) => {
            try_apply_inline_and_narrowing(postfix.operand, ctx, results);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => &fc.argument_list.arguments,
                Call::Method(mc) => &mc.argument_list.arguments,
                Call::NullSafeMethod(mc) => &mc.argument_list.arguments,
                Call::StaticMethod(sc) => &sc.argument_list.arguments,
            };
            for arg in args.iter() {
                let arg_expr = match arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                try_apply_inline_and_narrowing(arg_expr, ctx, results);
            }
        }
        _ => {}
    }
}

/// Collect and apply all instanceof narrowing checks from the LHS of a
/// `&&` expression.
///
/// When the LHS is itself a chain of `&&` (e.g. `A && B`), both `A`
/// and `B` contribute narrowing.  This function recurses through `&&`
/// nodes and applies each instanceof extraction it finds.
fn apply_and_lhs_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    match expr {
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            // Both sides of the inner `&&` are known-true.
            apply_and_lhs_narrowing(bin.lhs, ctx, results);
            apply_and_lhs_narrowing(bin.rhs, ctx, results);
        }
        Expression::Parenthesized(inner) => {
            apply_and_lhs_narrowing(inner.expression, ctx, results);
        }
        _ => {
            // Try to extract a single instanceof / is_a / class-identity check.
            if let Some(extraction) = try_extract_instanceof_with_negation(expr, ctx.var_name) {
                if extraction.negated {
                    apply_instanceof_exclusion(&extraction.class_name, ctx, results);
                } else {
                    apply_instanceof_inclusion(
                        &extraction.class_name,
                        extraction.exact,
                        ctx,
                        results,
                    );
                }
                return;
            }

            // Try type-guard functions (`is_object`, `is_array`, etc.).
            // When `is_object($var)` appears in the LHS of `&&` and
            // the variable has no resolved type yet, inject a synthetic
            // `stdClass` so that downstream member-access diagnostics
            // are suppressed (stdClass permits arbitrary properties).
            if let Some((kind, negated)) = try_extract_type_guard(expr, ctx.var_name)
                && !negated
                && kind == TypeGuardKind::Object
                && results.is_empty()
            {
                results.push(ClassInfo {
                    name: "stdClass".to_string(),
                    ..ClassInfo::default()
                });
            }
        }
    }
}

// ── Inline && null narrowing ─────────────────────────────────────

/// Apply null-check narrowing from `&&` short-circuit semantics.
///
/// This is the `ResolvedType`-level counterpart of
/// [`try_apply_inline_and_narrowing`] (which handles `instanceof`).
/// When the cursor is inside the RHS of `$var !== null && $var->…`,
/// the `!== null` check on the LHS guarantees the variable is
/// non-null in the RHS, so we strip `null` from the resolved types.
///
/// Works on `Vec<ResolvedType>` directly because `null` is not a
/// class and would be missed by class-level narrowing.
pub(in crate::completion) fn try_apply_inline_and_null_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    match expr {
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            let rhs_span = bin.rhs.span();
            let cursor_in_rhs = ctx.cursor_offset >= rhs_span.start.offset
                && ctx.cursor_offset <= rhs_span.end.offset;

            if cursor_in_rhs {
                apply_and_lhs_null_narrowing(bin.lhs, ctx, results);
            }

            // Recurse into both sides so that deeply nested `&&`
            // chains are handled.
            try_apply_inline_and_null_narrowing(bin.lhs, ctx, results);
            try_apply_inline_and_null_narrowing(bin.rhs, ctx, results);
        }
        // Recurse through common wrapper expressions so `&&` nodes
        // buried inside ternaries, assignments, other binary ops, etc.
        // are still discovered.
        Expression::Parenthesized(inner) => {
            try_apply_inline_and_null_narrowing(inner.expression, ctx, results);
        }
        Expression::Conditional(cond_expr) => {
            try_apply_inline_and_null_narrowing(cond_expr.condition, ctx, results);
            if let Some(then_expr) = cond_expr.then {
                try_apply_inline_and_null_narrowing(then_expr, ctx, results);
            }
            try_apply_inline_and_null_narrowing(cond_expr.r#else, ctx, results);
        }
        Expression::Assignment(assign) => {
            try_apply_inline_and_null_narrowing(assign.rhs, ctx, results);
        }
        Expression::Binary(bin) => {
            try_apply_inline_and_null_narrowing(bin.lhs, ctx, results);
            try_apply_inline_and_null_narrowing(bin.rhs, ctx, results);
        }
        Expression::UnaryPrefix(prefix) => {
            try_apply_inline_and_null_narrowing(prefix.operand, ctx, results);
        }
        Expression::UnaryPostfix(postfix) => {
            try_apply_inline_and_null_narrowing(postfix.operand, ctx, results);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => &fc.argument_list.arguments,
                Call::Method(mc) => &mc.argument_list.arguments,
                Call::NullSafeMethod(mc) => &mc.argument_list.arguments,
                Call::StaticMethod(sc) => &sc.argument_list.arguments,
            };
            for arg in args.iter() {
                let arg_expr = match arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                try_apply_inline_and_null_narrowing(arg_expr, ctx, results);
            }
        }
        _ => {}
    }
}

/// Collect and apply all null-check narrowing from the LHS of a `&&`
/// expression.
///
/// When the LHS contains `$var !== null` (or `!is_null($var)`, or a
/// bare truthy `$var`), and the variable matches `ctx.var_name`, we
/// remove `null` from `results`.
fn apply_and_lhs_null_narrowing(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    match expr {
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            // Both sides of the inner `&&` are known-true.
            apply_and_lhs_null_narrowing(bin.lhs, ctx, results);
            apply_and_lhs_null_narrowing(bin.rhs, ctx, results);
        }
        Expression::Parenthesized(inner) => {
            apply_and_lhs_null_narrowing(inner.expression, ctx, results);
        }
        _ => {
            // `try_extract_null_check` returns `Some(true)` when the
            // expression checks that the variable is NOT null (e.g.
            // `$var !== null`, `!is_null($var)`, bare `$var`).
            if let Some(true) = try_extract_null_check(expr, ctx.var_name) {
                // The LHS proved the variable is non-null → remove null.
                results.retain(|rt| !rt.type_string.is_null());
                for rt in results.iter_mut() {
                    if let Some(non_null) = rt.type_string.non_null_type() {
                        rt.type_string = non_null;
                    }
                }
            }
        }
    }
}

// ── Guard clause narrowing (early return / throw) ────────────────

/// Check whether a statement unconditionally exits the current scope.
///
/// A statement unconditionally exits if every code path through it
/// ends with `return`, `throw`, `continue`, or `break`.  This is used
/// to detect guard clause patterns like:
///
/// ```text
/// if (!$var instanceof Foo) {
///     return;
/// }
/// // $var is Foo here
/// ```
pub(in crate::completion) fn statement_unconditionally_exits(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::Return(_) => true,
        Statement::Continue(_) => true,
        Statement::Break(_) => true,
        // `throw new …;` is parsed as an expression statement
        // containing a Throw expression.
        Statement::Expression(es) => matches!(es.expression, Expression::Throw(_)),
        // A block exits if its last statement exits.
        Statement::Block(block) => block
            .statements
            .last()
            .is_some_and(statement_unconditionally_exits),
        // An if/else exits if ALL branches exist and ALL exit.
        Statement::If(if_stmt) => if_body_unconditionally_exits(&if_stmt.body),
        _ => false,
    }
}

/// Check whether an `if` body (including all branches) unconditionally
/// exits.  This requires:
///   - The then-body exits, AND
///   - All elseif bodies exit, AND
///   - An else clause exists and exits.
fn if_body_unconditionally_exits(body: &IfBody<'_>) -> bool {
    match body {
        IfBody::Statement(stmt_body) => {
            // Then-body must exit
            if !statement_unconditionally_exits(stmt_body.statement) {
                return false;
            }
            // All elseif bodies must exit
            if !stmt_body
                .else_if_clauses
                .iter()
                .all(|ei| statement_unconditionally_exits(ei.statement))
            {
                return false;
            }
            // Else must exist and exit
            stmt_body
                .else_clause
                .as_ref()
                .is_some_and(|ec| statement_unconditionally_exits(ec.statement))
        }
        IfBody::ColonDelimited(colon_body) => {
            // Then-body: last statement must exit
            if !colon_body
                .statements
                .last()
                .is_some_and(statement_unconditionally_exits)
            {
                return false;
            }
            // All elseif bodies must exit
            if !colon_body.else_if_clauses.iter().all(|ei| {
                ei.statements
                    .last()
                    .is_some_and(statement_unconditionally_exits)
            }) {
                return false;
            }
            // Else must exist and exit
            colon_body.else_clause.as_ref().is_some_and(|ec| {
                ec.statements
                    .last()
                    .is_some_and(statement_unconditionally_exits)
            })
        }
    }
}

/// Check whether an `if` body's then-branch unconditionally exits.
/// Used for guard clause detection where we only need the then-body
/// to exit (no else clause required).
fn then_body_unconditionally_exits(body: &IfBody<'_>) -> bool {
    match body {
        IfBody::Statement(stmt_body) => statement_unconditionally_exits(stmt_body.statement),
        IfBody::ColonDelimited(colon_body) => colon_body
            .statements
            .last()
            .is_some_and(statement_unconditionally_exits),
    }
}

/// Apply guard clause narrowing after an `if` statement whose
/// then-body unconditionally exits (return/throw/continue/break)
/// and which has no else/elseif clauses.
///
/// When a guard clause like:
/// ```text
/// if (!$var instanceof Foo) { return; }
/// ```
/// appears before the cursor, the code after it can only be reached
/// when the condition was *false* — so we apply the inverse narrowing.
///
/// This handles:
///   - `instanceof` / `is_a()` / `get_class()` / `::class` checks
///   - `@phpstan-assert-if-true` / `@phpstan-assert-if-false` guards
pub(in crate::completion) fn apply_guard_clause_narrowing(
    if_stmt: &If<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    // Only applies when the then-body exits and there are no
    // elseif/else branches (simple guard clause pattern).
    if !then_body_unconditionally_exits(&if_stmt.body) {
        return;
    }
    if if_stmt.body.has_else_clause() || if_stmt.body.has_else_if_clauses() {
        return;
    }

    // ── Compound OR guard clause ────────────────────────────────────
    // `if ($x instanceof A || $x instanceof B) { return; }`
    // After the if, $x is neither A nor B → exclude both.
    if let Some(classes) = try_extract_compound_or_instanceof(if_stmt.condition, ctx.var_name)
        && !classes.is_empty()
    {
        for cls_name in &classes {
            apply_instanceof_exclusion(cls_name, ctx, results);
        }
        return;
    }

    // ── Compound negated AND guard clause ───────────────────────────
    // `if (!$x instanceof A && !$x instanceof B) { return; }`
    // The then-body exits when $x is neither A nor B.  After the if,
    // the condition was false, so $x IS instanceof A or B → include both.
    if let Some(classes) =
        try_extract_compound_negated_and_instanceof(if_stmt.condition, ctx.var_name)
        && !classes.is_empty()
    {
        let union = resolve_class_names_to_union(&classes, ctx);
        if !union.is_empty() {
            results.clear();
            *results = union;
        }
        return;
    }

    // ── instanceof / is_a / get_class / ::class narrowing ──
    // The then-body exits, so subsequent code is the "else" — apply
    // the inverse of the condition.
    if let Some(extraction) = try_extract_instanceof_with_negation(if_stmt.condition, ctx.var_name)
    {
        // Positive instanceof + exit → exclude after (var is NOT that class)
        // Negated instanceof + exit → include after (var IS that class)
        if extraction.negated {
            apply_instanceof_inclusion(&extraction.class_name, extraction.exact, ctx, results);
        } else {
            apply_instanceof_exclusion(&extraction.class_name, ctx, results);
        }
    }

    // ── @phpstan-assert-if-true / @phpstan-assert-if-false ──
    // When a function or static method with assert-if-true/false is the
    // condition and the then-body exits, the code after runs when the
    // callee returned the opposite boolean — apply the inverse narrowing.
    let (func_call_expr, condition_negated) = unwrap_condition_negation(if_stmt.condition);

    if let Expression::Call(call) = func_call_expr
        && let Some(info) = extract_call_assertions(call, ctx)
    {
        // The then-body exits, so we're in the "else" conceptually.
        // inverted=true, same logic as try_apply_assert_condition_narrowing
        let function_returned_true = condition_negated;

        for assertion in info.assertions {
            let applies_positively = match assertion.kind {
                AssertionKind::IfTrue => function_returned_true,
                AssertionKind::IfFalse => !function_returned_true,
                AssertionKind::Always => continue,
            };

            if let Some(arg_var) = find_assertion_arg_variable(
                info.argument_list,
                &assertion.param_name,
                info.parameters,
            ) && arg_var == ctx.var_name
            {
                let should_exclude = assertion.negated ^ !applies_positively;
                if should_exclude {
                    apply_instanceof_exclusion_typed(&assertion.asserted_type, ctx, results);
                } else {
                    apply_instanceof_inclusion_typed(&assertion.asserted_type, false, ctx, results);
                }
            }
        }
    }
}

// ── Compound instanceof helpers ─────────────────────────────────

/// Extract all instanceof class names from a compound `||` condition.
///
/// For `$x instanceof A || $x instanceof B || $x instanceof C`,
/// returns `Some(["A", "B", "C"])`.  Returns `None` if the expression
/// is not a chain of `||`-connected instanceof checks on `var_name`.
fn try_extract_compound_or_instanceof<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<Vec<String>> {
    match expr {
        Expression::Parenthesized(inner) => {
            try_extract_compound_or_instanceof(inner.expression, var_name)
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::Or(_) | BinaryOperator::LowOr(_)
            ) =>
        {
            let mut classes = Vec::new();
            collect_or_instanceof_classes(expr, var_name, &mut classes);
            if classes.is_empty() {
                None
            } else {
                Some(classes)
            }
        }
        _ => None,
    }
}

/// Recursively walk a tree of `||` binary expressions, collecting
/// instanceof class names for `var_name`.
fn collect_or_instanceof_classes<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
    out: &mut Vec<String>,
) {
    match expr {
        Expression::Parenthesized(inner) => {
            collect_or_instanceof_classes(inner.expression, var_name, out);
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::Or(_) | BinaryOperator::LowOr(_)
            ) =>
        {
            collect_or_instanceof_classes(bin.lhs, var_name, out);
            collect_or_instanceof_classes(bin.rhs, var_name, out);
        }
        _ => {
            if let Some(cls_name) = try_extract_instanceof(expr, var_name)
                && !out.contains(&cls_name)
            {
                out.push(cls_name);
            }
        }
    }
}

/// Extract all instanceof class names from a compound `&&` condition.
///
/// For `$x instanceof A && $x instanceof B`, returns `Some(["A", "B"])`.
/// Returns `None` if the expression is not a chain of `&&`-connected
/// instanceof checks on `var_name`.
fn try_extract_compound_and_instanceof<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<Vec<String>> {
    match expr {
        Expression::Parenthesized(inner) => {
            try_extract_compound_and_instanceof(inner.expression, var_name)
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            let mut classes = Vec::new();
            collect_and_instanceof_classes(expr, var_name, &mut classes);
            if classes.is_empty() {
                None
            } else {
                Some(classes)
            }
        }
        _ => None,
    }
}

/// Recursively walk a tree of `&&` binary expressions, collecting
/// instanceof class names for `var_name`.
fn collect_and_instanceof_classes<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
    out: &mut Vec<String>,
) {
    match expr {
        Expression::Parenthesized(inner) => {
            collect_and_instanceof_classes(inner.expression, var_name, out);
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            collect_and_instanceof_classes(bin.lhs, var_name, out);
            collect_and_instanceof_classes(bin.rhs, var_name, out);
        }
        _ => {
            if let Some(cls_name) = try_extract_instanceof(expr, var_name)
                && !out.contains(&cls_name)
            {
                out.push(cls_name);
            }
        }
    }
}

/// Detect a compound `&&` of negated `instanceof` checks for `var_name`.
///
/// Matches patterns like `!$x instanceof A && !$x instanceof B`.
/// Returns the list of class names when every leaf of the `&&` tree is
/// a negated instanceof for the same variable.  Returns `None` when the
/// pattern does not match.
fn try_extract_compound_negated_and_instanceof<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<Vec<String>> {
    match expr {
        Expression::Parenthesized(inner) => {
            try_extract_compound_negated_and_instanceof(inner.expression, var_name)
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            let mut classes = Vec::new();
            if collect_negated_and_instanceof_classes(expr, var_name, &mut classes)
                && !classes.is_empty()
            {
                Some(classes)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Recursively walk a tree of `&&` binary expressions, collecting
/// instanceof class names from negated instanceof leaves.
///
/// Returns `true` when every leaf successfully matched `!$var instanceof Class`.
fn collect_negated_and_instanceof_classes<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
    out: &mut Vec<String>,
) -> bool {
    match expr {
        Expression::Parenthesized(inner) => {
            collect_negated_and_instanceof_classes(inner.expression, var_name, out)
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            collect_negated_and_instanceof_classes(bin.lhs, var_name, out)
                && collect_negated_and_instanceof_classes(bin.rhs, var_name, out)
        }
        _ => {
            // Each leaf must be a negated instanceof for the target variable.
            if let Some(extraction) = try_extract_instanceof_with_negation(expr, var_name)
                && extraction.negated
            {
                if !out.contains(&extraction.class_name) {
                    out.push(extraction.class_name);
                }
                true
            } else {
                false
            }
        }
    }
}

// ── in_array strict-mode narrowing ───────────────────────────────

/// Extract the haystack expression from an
/// `in_array($needle, $haystack, true)` call where the needle
/// matches `var_name`.
///
/// Returns `Some(haystack_expr)` when:
///   - The function name is `in_array`
///   - The first argument is a simple `$variable` matching `var_name`
///   - There are at least 3 arguments and the third is the literal `true`
///
/// The caller is responsible for resolving the haystack expression's
/// iterable element type.
fn try_extract_in_array<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<&'b Expression<'b>> {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    let func_call = match expr {
        Expression::Call(Call::Function(fc)) => fc,
        _ => return None,
    };
    let name = match func_call.function {
        Expression::Identifier(ident) => ident.value(),
        _ => return None,
    };
    if name != "in_array" {
        return None;
    }
    let args: Vec<_> = func_call.argument_list.arguments.iter().collect();
    if args.len() < 3 {
        return None;
    }

    // Third argument must be the literal `true` (strict mode).
    let third_expr = match &args[2] {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    };
    if !third_expr.is_true() {
        return None;
    }

    // First argument must be our variable.
    let first_expr = match &args[0] {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    };
    let needle_var = match first_expr {
        Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
        _ => return None,
    };
    if needle_var != var_name {
        return None;
    }

    // Second argument is the haystack expression.
    let second_expr = match &args[1] {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    };
    Some(second_expr)
}

/// Resolve the haystack expression's iterable element type and return
/// it as a type string suitable for [`apply_instanceof_inclusion`].
///
/// Handles `$variable` (via docblock + assignment chasing) as well as
/// method calls, property access, and other expressions supported by
/// [`resolve_arg_raw_type`](crate::completion::variable::resolution::resolve_arg_raw_type).
fn resolve_in_array_element_type(
    haystack_expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<PhpType> {
    let raw_type =
        crate::completion::variable::resolution::resolve_arg_raw_type(haystack_expr, ctx)?;
    raw_type.extract_element_type().cloned()
}

/// Apply `in_array($var, $haystack, true)` narrowing when the call
/// appears as an `if` / `while` condition and the cursor is inside
/// the then-body.
///
/// Narrows `$var` to the haystack's element type.  Also handles
/// negated conditions (`!in_array(…)`) by excluding instead.
pub(in crate::completion) fn try_apply_in_array_narrowing(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }
    let (inner, negated) = unwrap_condition_negation(condition);
    if let Some(haystack_expr) = try_extract_in_array(inner, ctx.var_name)
        && let Some(element_type) = resolve_in_array_element_type(haystack_expr, ctx)
    {
        if negated {
            if !results.is_empty() && would_exclude_all_results(&element_type, results, ctx) {
                return;
            }
            apply_instanceof_exclusion_typed(&element_type, ctx, results);
        } else {
            apply_instanceof_inclusion_typed(&element_type, false, ctx, results);
        }
    }
}

/// Inverse of [`try_apply_in_array_narrowing`] -- used for the `else`
/// branch of an `if (in_array(…))` check.
///
/// A positive `in_array` in the condition means the variable is NOT
/// one of the haystack's element types inside the else body (exclude),
/// and vice-versa for a negated condition (include).
pub(in crate::completion) fn try_apply_in_array_narrowing_inverse(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }
    let (inner, negated) = unwrap_condition_negation(condition);
    if let Some(haystack_expr) = try_extract_in_array(inner, ctx.var_name)
        && let Some(element_type) = resolve_in_array_element_type(haystack_expr, ctx)
    {
        // Flip polarity for the else branch.
        if negated {
            apply_instanceof_inclusion_typed(&element_type, false, ctx, results);
        } else {
            if !results.is_empty() && would_exclude_all_results(&element_type, results, ctx) {
                return;
            }
            apply_instanceof_exclusion_typed(&element_type, ctx, results);
        }
    }
}

/// Check whether excluding `element_type` from `results` would remove
/// every entry, leaving the variable completely untyped.
///
/// `in_array($var, $haystack)` checks whether a *value* is present in
/// the array, not whether the *type* matches.  When the haystack's
/// element type is the same as (or a supertype of) every entry in
/// `results`, exclusion would incorrectly wipe out all type information.
/// For example, `in_array($item, $exclude)` where both `$item` and
/// `$exclude`'s elements are `BackedEnum` should not remove
/// `BackedEnum` from the resolved types — the variable is still a
/// `BackedEnum`, just not one of the excluded values.
fn would_exclude_all_results(
    element_type: &PhpType,
    results: &[ClassInfo],
    ctx: &VarResolutionCtx<'_>,
) -> bool {
    let excluded = super::resolution::type_hint_to_classes_typed(
        element_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    if excluded.is_empty() {
        return false;
    }
    results
        .iter()
        .all(|r| excluded.iter().any(|e| e.name == r.name))
}

/// Apply `in_array` guard clause narrowing after an `if` statement
/// whose then-body unconditionally exits.
///
/// When a guard clause like:
/// ```text
/// if (!in_array($var, $haystack, true)) { return; }
/// ```
/// appears before the cursor, the code after it can only be reached
/// when the condition was *false* -- so we apply the inverse narrowing.
pub(in crate::completion) fn apply_guard_clause_in_array_narrowing(
    if_stmt: &If<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    if !then_body_unconditionally_exits(&if_stmt.body) {
        return;
    }
    if if_stmt.body.has_else_clause() || if_stmt.body.has_else_if_clauses() {
        return;
    }

    let (inner, condition_negated) = unwrap_condition_negation(if_stmt.condition);
    if let Some(haystack_expr) = try_extract_in_array(inner, ctx.var_name)
        && let Some(element_type) = resolve_in_array_element_type(haystack_expr, ctx)
    {
        // The then-body exits, so subsequent code is the "else".
        // Positive in_array + exit → exclude (var is NOT in haystack)
        // Negated in_array + exit → include (var IS in haystack)
        if condition_negated {
            apply_instanceof_inclusion_typed(&element_type, false, ctx, results);
        } else {
            // Skip exclusion when it would remove ALL type information.
            // `in_array($item, $exclude)` where both `$item` and the
            // elements of `$exclude` are the same type (e.g. both are
            // `BackedEnum`) only narrows which *value* of that type the
            // variable holds, not the type itself.  Excluding would
            // incorrectly wipe out the variable's type entirely.
            if !results.is_empty() && would_exclude_all_results(&element_type, results, ctx) {
                return;
            }
            apply_instanceof_exclusion_typed(&element_type, ctx, results);
        }
    }
}

// ── Null / falsy guard clause narrowing ─────────────────────────────────

/// Detect whether `condition` is a null/falsy check on `var_name`.
///
/// Recognised patterns (after unwrapping negation and parentheses):
///   - `$var` (bare variable used as boolean — falsy check)
///   - `$var === null` / `null === $var`
///   - `$var == null`  / `null == $var`
///   - `is_null($var)`
///   - `empty($var)`
///
/// Returns `Some(negated)` where `negated` is `true` when the original
/// condition was negated (e.g. `!$var`, `$var !== null`).
fn try_extract_null_check(condition: &Expression<'_>, var_name: &str) -> Option<bool> {
    let (inner, negated) = unwrap_condition_negation(condition);

    match inner {
        // Bare variable used as boolean condition:
        //   `if ($var) { return; }` → then-body runs when truthy (NOT null)
        //   `if (!$var) { return; }` → then-body runs when falsy (IS null)
        // The caller interprets `false` as "checks for null/falsy" and
        // `true` as "checks for NOT null".  A bare `$var` is a truthy
        // check (negated=false → NOT null → return true), and `!$var`
        // is a falsy check (negated=true → IS null → return false).
        Expression::Variable(Variable::Direct(dv)) if dv.name == var_name => Some(!negated),

        // Assignment used as a condition:
        //   `if ($var = expr())` → truthy when assigned value is truthy
        //   `if (!($var = expr()))` → truthy when assigned value is falsy
        // This is equivalent to a bare variable truthy check.
        Expression::Assignment(assign)
            if assign.operator.is_assign()
                && matches!(
                    assign.lhs,
                    Expression::Variable(Variable::Direct(dv)) if dv.name == var_name
                ) =>
        {
            Some(!negated)
        }

        // Parenthesized assignment used as a condition:
        //   `if (($var = expr()))` — same as above but wrapped in parens.
        Expression::Parenthesized(inner) => try_extract_null_check(inner.expression, var_name),

        // `$var === null` / `$var == null` / `$var !== null` / `$var != null`
        Expression::Binary(bin) => {
            let is_identity = matches!(
                bin.operator,
                BinaryOperator::Identical(_) | BinaryOperator::NotIdentical(_)
            );
            let is_equality = matches!(
                bin.operator,
                BinaryOperator::Equal(_) | BinaryOperator::NotEqual(_)
            );
            if !is_identity && !is_equality {
                return None;
            }

            // NotIdentical / NotEqual flip the negation sense.
            let op_negates = matches!(
                bin.operator,
                BinaryOperator::NotIdentical(_) | BinaryOperator::NotEqual(_)
            );

            let var_side = if is_null_literal(bin.rhs) {
                bin.lhs
            } else if is_null_literal(bin.lhs) {
                bin.rhs
            } else {
                return None;
            };

            if is_var_or_assignment_to(var_side, var_name) {
                return Some(negated ^ op_negates);
            }
            None
        }

        // `is_null($var)` / `empty($var)`
        Expression::Call(Call::Function(fc)) => {
            let func_name = match &fc.function {
                Expression::Identifier(ident) => ident.value(),
                _ => return None,
            };
            if func_name != "is_null" && func_name != "empty" {
                return None;
            }
            let args = &fc.argument_list.arguments;
            if args.len() != 1 {
                return None;
            }
            if let Some(Argument::Positional(pos)) = args.first()
                && let Expression::Variable(Variable::Direct(dv)) = pos.value
                && dv.name == var_name
            {
                return Some(negated);
            }
            None
        }

        _ => None,
    }
}

/// Check whether an expression is a direct variable reference to
/// `var_name`, or an assignment (possibly parenthesized) whose LHS
/// is that variable.
///
/// This lets null-check extraction recognise patterns like
/// `($x = expr()) !== null` in addition to `$x !== null`.
fn is_var_or_assignment_to(expr: &Expression<'_>, var_name: &str) -> bool {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => dv.name == var_name,
        Expression::Assignment(assign)
            if assign.operator.is_assign()
                && matches!(
                    assign.lhs,
                    Expression::Variable(Variable::Direct(dv)) if dv.name == var_name
                ) =>
        {
            true
        }
        Expression::Parenthesized(inner) => is_var_or_assignment_to(inner.expression, var_name),
        _ => false,
    }
}

/// Returns `true` when the expression is the literal `null`.
fn is_null_literal(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Literal(Literal::Null(_)))
}

/// Apply null guard clause narrowing after an `if` statement whose
/// then-body unconditionally exits.
///
/// When a guard clause like:
/// ```text
/// if (!$var) { continue; }
/// if ($var === null) { return; }
/// ```
/// appears before the cursor, the code after it can only be reached
/// when the variable is non-null.  This removes `null` entries from
/// the resolved type list.
///
/// Operates directly on `Vec<ResolvedType>` because `null` is not a
/// class and would be missed by class-level narrowing.
pub(in crate::completion) fn apply_guard_clause_null_narrowing(
    if_stmt: &If<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if !then_body_unconditionally_exits(&if_stmt.body) {
        return;
    }
    if if_stmt.body.has_else_clause() || if_stmt.body.has_else_if_clauses() {
        return;
    }

    if let Some(condition_checks_null) = try_extract_null_check(if_stmt.condition, ctx.var_name) {
        // condition_checks_null == false means the condition is "is null/falsy"
        //   → then-body exits when null → after: non-null → remove null
        // condition_checks_null == true means the condition is "is NOT null"
        //   → then-body exits when non-null → after: null → keep only null
        if !condition_checks_null {
            // Remove null entries from the resolved types.
            results.retain(|rt| !rt.type_string.is_null());
            // Also strip `null` from union types (e.g. `Foo|null` → `Foo`).
            for rt in results.iter_mut() {
                if let Some(non_null) = rt.type_string.non_null_type() {
                    rt.type_string = non_null;
                }
            }
        }
        // The "exits when non-null" case (condition_checks_null == true)
        // is unusual and not worth handling — the code after would only
        // see null, which has no class members to complete on.
    }
}

/// Apply null narrowing inside the then-body of an `if` / `elseif`
/// whose condition proves the variable is non-null.
///
/// Handles:
///   - Simple: `if ($var !== null) { … }`
///   - Compound `&&`: `if ($a !== null && $b !== null && …) { … }`
///     — each operand that checks non-null for `ctx.var_name`
///     contributes narrowing.
///   - `!is_null($var)`, bare truthy `$var`
///
/// This is the if-body counterpart of the inline `&&` narrowing
/// ([`try_apply_inline_and_null_narrowing`]) and the guard-clause
/// narrowing ([`apply_guard_clause_null_narrowing`]).
pub(in crate::completion) fn try_apply_if_body_null_narrowing(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }

    if condition_checks_non_null(condition, ctx.var_name) {
        results.retain(|rt| !rt.type_string.is_null());
        for rt in results.iter_mut() {
            if let Some(non_null) = rt.type_string.non_null_type() {
                rt.type_string = non_null;
            }
        }
    }
}

/// Apply inverse null narrowing inside the else-body of an `if`
/// whose condition proves the variable is non-null.
///
/// When the condition is a simple `$var !== null` (not compound `&&`),
/// the else-body knows the variable IS null.  This is only useful for
/// stripping non-null types, but we include it for completeness so
/// that hover shows `null` rather than `Carbon|null` in the else
/// branch.
///
/// For compound `&&` conditions we cannot safely invert — any single
/// operand being false enters the else branch, so we skip those.
pub(in crate::completion) fn try_apply_if_body_null_narrowing_inverse(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }

    // Only invert simple (non-compound) null checks.
    // `if ($var !== null) { … } else { ← $var is null here }`
    if let Some(true) = try_extract_null_check(condition, ctx.var_name) {
        // The condition proved NOT null, so the else-body sees null.
        // Keep only null entries.
        results.retain(|rt| rt.type_string.is_null());
    } else if let Some(false) = try_extract_null_check(condition, ctx.var_name) {
        // The condition proved IS null (e.g. `$var === null`), so the
        // else-body sees non-null.
        results.retain(|rt| !rt.type_string.is_null());
        for rt in results.iter_mut() {
            if let Some(non_null) = rt.type_string.non_null_type() {
                rt.type_string = non_null;
            }
        }
    }
}

/// Check whether `condition` proves that `var_name` is non-null.
///
/// Returns `true` when the condition (or any operand in a `&&` chain)
/// is a non-null check for the given variable.
fn condition_checks_non_null(condition: &Expression<'_>, var_name: &str) -> bool {
    match condition {
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            // In `A && B`, both A and B are true — if either checks
            // non-null, the variable is non-null.
            condition_checks_non_null(bin.lhs, var_name)
                || condition_checks_non_null(bin.rhs, var_name)
        }
        Expression::Parenthesized(inner) => condition_checks_non_null(inner.expression, var_name),
        _ => matches!(try_extract_null_check(condition, var_name), Some(true)),
    }
}

// ── PHP type-guard function narrowing (`is_array`, `is_string`, …) ──────

/// The category of a PHP type-checking function like `is_array`, `is_string`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeGuardKind {
    Array,
    String,
    Int,
    Float,
    Bool,
    Object,
    Numeric,
    Callable,
}

/// Return the canonical `PhpType` that a type-guard narrows `mixed` to.
///
/// When a variable has type `mixed` and a type-guard like `is_object()`
/// succeeds, the variable should narrow to `object` (not stay `mixed`
/// and not become empty).  This function maps each guard kind to the
/// PHP type it asserts.
fn guard_kind_to_narrowed_type(kind: TypeGuardKind) -> PhpType {
    match kind {
        TypeGuardKind::Array => PhpType::array(),
        TypeGuardKind::String => PhpType::string(),
        TypeGuardKind::Int => PhpType::int(),
        TypeGuardKind::Float => PhpType::float(),
        TypeGuardKind::Bool => PhpType::bool(),
        TypeGuardKind::Object => PhpType::object(),
        TypeGuardKind::Numeric => PhpType::numeric(),
        TypeGuardKind::Callable => PhpType::callable(),
    }
}

/// Try to extract a type-guard function call on a variable.
///
/// Matches `is_array($var)`, `is_string($var)`, etc. (with optional
/// parenthesisation and negation).
///
/// Returns `Some((kind, negated))` when the expression is a recognised
/// type-guard call on `var_name`.
fn try_extract_type_guard(expr: &Expression<'_>, var_name: &str) -> Option<(TypeGuardKind, bool)> {
    match expr {
        Expression::Parenthesized(inner) => try_extract_type_guard(inner.expression, var_name),
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            try_extract_type_guard(prefix.operand, var_name).map(|(kind, neg)| (kind, !neg))
        }
        Expression::Call(Call::Function(fc)) => {
            let func_name = match &fc.function {
                Expression::Identifier(ident) => ident.value(),
                _ => return None,
            };
            let kind = match func_name {
                "is_array" => TypeGuardKind::Array,
                "is_string" => TypeGuardKind::String,
                "is_int" | "is_integer" | "is_long" => TypeGuardKind::Int,
                "is_float" | "is_double" | "is_real" => TypeGuardKind::Float,
                "is_bool" => TypeGuardKind::Bool,
                "is_object" => TypeGuardKind::Object,
                "is_numeric" => TypeGuardKind::Numeric,
                "is_callable" => TypeGuardKind::Callable,
                _ => return None,
            };
            let args = &fc.argument_list.arguments;
            if args.len() != 1 {
                return None;
            }
            let arg_expr = match args.first() {
                Some(Argument::Positional(pos)) => pos.value,
                Some(Argument::Named(named)) => named.value,
                _ => return None,
            };
            let arg_name = expr_to_subject_key(arg_expr)?;
            if arg_name != var_name {
                return None;
            }
            Some((kind, false))
        }
        _ => None,
    }
}

/// Check whether a `PhpType` matches a given type-guard kind.
///
/// For `TypeGuardKind::Array`, returns `true` for array-like types
/// (`array`, `list<T>`, `T[]`, `array{…}`, `iterable`, etc.).
fn type_matches_guard(ty: &PhpType, kind: TypeGuardKind) -> bool {
    match kind {
        TypeGuardKind::Array => ty.is_array_like(),
        TypeGuardKind::String => ty.is_subtype_of(&PhpType::string()),
        TypeGuardKind::Int => ty.is_subtype_of(&PhpType::int()),
        TypeGuardKind::Float => ty.is_subtype_of(&PhpType::float()),
        TypeGuardKind::Bool => ty.is_subtype_of(&PhpType::bool()),
        TypeGuardKind::Numeric => ty.is_subtype_of(&PhpType::numeric()),
        TypeGuardKind::Callable => ty.is_callable(),
        TypeGuardKind::Object => ty.is_object_like(),
    }
}

/// Narrow `results` to only the union members that match the given
/// type-guard kind.
///
/// For example, when `kind` is `Array` and the type string is
/// `null|list<Request>|Request`, the result is narrowed to
/// `list<Request>`.
fn apply_type_guard_inclusion(kind: TypeGuardKind, results: &mut Vec<ResolvedType>) {
    for rt in results.iter_mut() {
        let filtered = filter_type_by_guard(&rt.type_string, kind, true);
        if let Some(narrowed) = filtered {
            rt.type_string = narrowed;
            // If the narrowed type no longer matches the class_info's
            // class (e.g. narrowed from `Request|list<Request>` to
            // `list<Request>`), clear the class_info.
            if let Some(ref ci) = rt.class_info {
                let ci_type = PhpType::Named(ci.name.clone());
                if !type_matches_guard(&ci_type, kind) {
                    rt.class_info = None;
                }
            }
        }
    }
    // Remove entries that became empty (no union member matched).
    results.retain(|rt| !matches!(&rt.type_string, PhpType::Named(s) if s == "__empty"));
}

/// Narrow `results` to only the union members that do NOT match the
/// given type-guard kind (inverse / else-body narrowing).
fn apply_type_guard_exclusion(kind: TypeGuardKind, results: &mut Vec<ResolvedType>) {
    for rt in results.iter_mut() {
        let filtered = filter_type_by_guard(&rt.type_string, kind, false);
        if let Some(narrowed) = filtered {
            rt.type_string = narrowed;
        }
    }
    results.retain(|rt| !matches!(&rt.type_string, PhpType::Named(s) if s == "__empty"));
}

/// Filter a `PhpType` to keep only members that match (or don't match)
/// the given type-guard kind.
///
/// When `keep_matching` is `true`, keeps only members where
/// `type_matches_guard` returns `true` (then-body semantics).
/// When `false`, keeps only members where it returns `false`
/// (else-body semantics).
///
/// Returns `None` when no filtering is needed (non-union type that
/// already satisfies the predicate).  Returns `Some(Named("__empty"))`
/// when all members are filtered out.
fn filter_type_by_guard(ty: &PhpType, kind: TypeGuardKind, keep_matching: bool) -> Option<PhpType> {
    match ty {
        PhpType::Union(members) => {
            let filtered: Vec<PhpType> = members
                .iter()
                .filter(|m| type_matches_guard(m, kind) == keep_matching)
                .cloned()
                .collect();
            if filtered.len() == members.len() {
                // Nothing was filtered out.
                None
            } else if filtered.is_empty() {
                Some(PhpType::Named("__empty".to_string()))
            } else if filtered.len() == 1 {
                Some(filtered.into_iter().next().unwrap())
            } else {
                Some(PhpType::Union(filtered))
            }
        }
        PhpType::Nullable(inner) => {
            // `?T` is `T|null`.  For `is_array`, null doesn't match,
            // so we keep only the inner type (if it matches) or only
            // null (if it doesn't).
            let inner_matches = type_matches_guard(inner, kind);
            let null_matches = type_matches_guard(&PhpType::null(), kind);
            match (
                inner_matches == keep_matching,
                null_matches == keep_matching,
            ) {
                (true, true) => None, // keep both → no change
                (true, false) => Some(inner.as_ref().clone()),
                (false, true) => Some(PhpType::null()),
                (false, false) => Some(PhpType::Named("__empty".to_string())),
            }
        }
        other => {
            // `mixed` includes all types.  When narrowing in the
            // then-body (`keep_matching = true`), replace `mixed`
            // with the canonical type for the guard kind (e.g.
            // `is_object($mixed)` → `object`).  In the else-body
            // (`keep_matching = false`), `mixed` minus one kind is
            // still effectively `mixed`, so leave it unchanged.
            if other.is_mixed() {
                return if keep_matching {
                    Some(guard_kind_to_narrowed_type(kind))
                } else {
                    None // mixed minus one kind ≈ mixed
                };
            }
            // Non-union type: if it matches the predicate, keep it.
            if type_matches_guard(other, kind) == keep_matching {
                None // no change needed
            } else {
                Some(PhpType::Named("__empty".to_string()))
            }
        }
    }
}

/// Apply type-guard narrowing (`is_array`, `is_string`, etc.) when the
/// cursor is inside the then-body of an `if` whose condition is a
/// type-guard call on the resolved variable.
///
/// Works on `Vec<ResolvedType>` directly (like null narrowing) because
/// the narrowing modifies `type_string` rather than `class_info`.
pub(in crate::completion) fn try_apply_type_guard_narrowing(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }

    // ── Compound `&&` / `and` ───────────────────────────────────────
    // `if (is_object($x) && is_string($y))` — both sides are true
    // inside the then-body.  Decompose and apply each guard found.
    if let Expression::Binary(bin) = condition
        && matches!(
            bin.operator,
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
        )
    {
        try_apply_type_guard_narrowing(bin.lhs, body_span, ctx, results);
        try_apply_type_guard_narrowing(bin.rhs, body_span, ctx, results);
        return;
    }

    if let Some((kind, negated)) = try_extract_type_guard(condition, ctx.var_name) {
        if negated {
            apply_type_guard_exclusion(kind, results);
        } else {
            apply_type_guard_inclusion(kind, results);
        }
    }
}

/// Inverse of [`try_apply_type_guard_narrowing`] — used for the `else`
/// branch of an `if (is_array($var))` check.
pub(in crate::completion) fn try_apply_type_guard_narrowing_inverse(
    condition: &Expression<'_>,
    body_span: mago_span::Span,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
        return;
    }
    if let Some((kind, negated)) = try_extract_type_guard(condition, ctx.var_name) {
        // Flip polarity for else-body.
        if negated {
            apply_type_guard_inclusion(kind, results);
        } else {
            apply_type_guard_exclusion(kind, results);
        }
    }
}

/// Apply type-guard narrowing after a guard clause (`if (is_array($x))
/// { return; }`).  The then-body exits, so code after the if sees the
/// inverse narrowing.
pub(in crate::completion) fn apply_guard_clause_type_guard_narrowing(
    if_stmt: &If<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if !then_body_unconditionally_exits(&if_stmt.body) {
        return;
    }
    if if_stmt.body.has_else_clause() || if_stmt.body.has_else_if_clauses() {
        return;
    }
    let (inner, condition_negated) = unwrap_condition_negation(if_stmt.condition);
    if let Some((kind, negated)) = try_extract_type_guard(inner, ctx.var_name) {
        let effectively_negated = negated ^ condition_negated;
        // Then-body exits, so post-if sees the inverse.
        if effectively_negated {
            // Guard was `!is_array($x) { return; }` → after if, $x IS array.
            apply_type_guard_inclusion(kind, results);
        } else {
            // Guard was `is_array($x) { return; }` → after if, $x is NOT array.
            apply_type_guard_exclusion(kind, results);
        }
    }
}
