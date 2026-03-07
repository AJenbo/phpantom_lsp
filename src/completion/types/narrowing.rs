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
///   - Guard clauses: `if (!$var instanceof Foo) { return; }` — narrows
///     after the if block when the body unconditionally exits via
///     `return`, `throw`, `continue`, or `break`.
///   - `in_array($var, $haystack, true)` — narrows `$var` to the
///     haystack's element type when the third argument is `true`.
use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::docblock;
use crate::types::{AssertionKind, ClassInfo};

use super::conditional::extract_class_string_from_expr;
use crate::completion::resolver::VarResolutionCtx;

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
        let mut union = Vec::new();
        for cls_name in &classes {
            let resolved = super::resolution::type_hint_to_classes(
                cls_name,
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
        let mut union = Vec::new();
        for cls_name in &classes {
            let resolved = super::resolution::type_hint_to_classes(
                cls_name,
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
        if !union.is_empty() {
            results.clear();
            *results = union;
        }
        return;
    }

    if let Some((cls_name, negated)) = try_extract_instanceof_with_negation(condition, ctx.var_name)
    {
        if negated {
            apply_instanceof_exclusion(&cls_name, ctx, results);
        } else {
            apply_instanceof_inclusion(&cls_name, ctx, results);
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

    if let Some((cls_name, negated)) = try_extract_instanceof_with_negation(condition, ctx.var_name)
    {
        // Flip the polarity: positive condition → exclude in else,
        // negated condition → include in else.
        if negated {
            apply_instanceof_inclusion(&cls_name, ctx, results);
        } else {
            apply_instanceof_exclusion(&cls_name, ctx, results);
        }
    }
}

/// Replace `results` with only the resolved classes for `cls_name`.
pub(in crate::completion) fn apply_instanceof_inclusion(
    cls_name: &str,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let narrowed = super::resolution::type_hint_to_classes(
        cls_name,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    if !narrowed.is_empty() {
        results.clear();
        for cls in narrowed {
            if !results.iter().any(|c| c.name == cls.name) {
                results.push(cls);
            }
        }
    }
}

/// Remove the resolved classes for `cls_name` from `results`.
pub(in crate::completion) fn apply_instanceof_exclusion(
    cls_name: &str,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    let excluded = super::resolution::type_hint_to_classes(
        cls_name,
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
            // LHS must be our variable
            let lhs_name = match bin.lhs {
                Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
                _ => return None,
            };
            if lhs_name != var_name {
                return None;
            }
            // RHS is the class name
            match bin.rhs {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
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
pub(in crate::completion) fn try_extract_instanceof_with_negation<'b>(
    expr: &'b Expression<'b>,
    var_name: &str,
) -> Option<(String, bool)> {
    match expr {
        Expression::Parenthesized(inner) => {
            try_extract_instanceof_with_negation(inner.expression, var_name)
        }
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            // `!expr` — recurse so that `!!expr` (double negation) and
            // deeper chains like `!!!expr` are handled correctly: each
            // `!` flips the negation flag.
            try_extract_instanceof_with_negation(prefix.operand, var_name)
                .map(|(cls, neg)| (cls, !neg))
        }
        _ => {
            try_extract_instanceof(expr, var_name)
                .map(|cls| (cls, false))
                .or_else(|| {
                    // `is_a($var, ClassName::class)` — equivalent to instanceof
                    try_extract_is_a(expr, var_name).map(|cls| (cls, false))
                })
                .or_else(|| {
                    // `get_class($var) === ClassName::class` or
                    // `$var::class === ClassName::class` — exact class match
                    try_extract_class_identity_check(expr, var_name)
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

/// Apply narrowing from `@phpstan-assert` / `@psalm-assert` annotations
/// on a function called as a standalone expression statement.
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
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value().to_string(),
            _ => return,
        };
        let func_info = match ctx.function_loader {
            Some(fl) => match fl(&func_name) {
                Some(fi) => fi,
                None => return,
            },
            None => return,
        };
        for assertion in &func_info.type_assertions {
            if assertion.kind != AssertionKind::Always {
                continue;
            }
            // Find the parameter index for this assertion
            if let Some(arg_var) = find_assertion_arg_variable(
                &func_call.argument_list,
                &assertion.param_name,
                &func_info.parameters,
            ) && arg_var == ctx.var_name
            {
                if assertion.negated {
                    apply_instanceof_exclusion(&assertion.asserted_type, ctx, results);
                } else {
                    apply_instanceof_inclusion(&assertion.asserted_type, ctx, results);
                }
            }
        }
    }
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

    let func_call = match func_call_expr {
        Expression::Call(Call::Function(fc)) => fc,
        _ => return,
    };
    let func_name = match func_call.function {
        Expression::Identifier(ident) => ident.value().to_string(),
        _ => return,
    };
    let func_info = match ctx.function_loader {
        Some(fl) => match fl(&func_name) {
            Some(fi) => fi,
            None => return,
        },
        None => return,
    };

    // Determine whether the function returned true in this branch.
    //
    // - then-body (inverted=false), no negation  → function returned true
    // - then-body (inverted=false), negated       → function returned false
    // - else-body (inverted=true),  no negation  → function returned false
    // - else-body (inverted=true),  negated       → function returned true
    let function_returned_true = !(inverted ^ condition_negated);

    for assertion in &func_info.type_assertions {
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
            &func_call.argument_list,
            &assertion.param_name,
            &func_info.parameters,
        ) && arg_var == ctx.var_name
        {
            // XOR the assertion's own negation with whether we're in the
            // opposite branch: positive + non-negated → include,
            // positive + negated → exclude, opposite + non-negated → exclude,
            // opposite + negated → include.
            let should_exclude = assertion.negated ^ !applies_positively;
            if should_exclude {
                apply_instanceof_exclusion(&assertion.asserted_type, ctx, results);
            } else {
                apply_instanceof_inclusion(&assertion.asserted_type, ctx, results);
            }
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
        let mut union = Vec::new();
        for cls_name in &classes {
            let resolved = super::resolution::type_hint_to_classes(
                cls_name,
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
        if !union.is_empty() {
            results.clear();
            *results = union;
        }
        return;
    }

    if let Some((cls_name, negated)) = try_extract_assert_instanceof(expr, ctx.var_name) {
        if negated {
            apply_instanceof_exclusion(&cls_name, ctx, results);
        } else {
            apply_instanceof_inclusion(&cls_name, ctx, results);
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
) -> Option<(String, bool)> {
    // Unwrap parenthesised wrapper on the whole expression
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value().to_string(),
            _ => return None,
        };
        if func_name != "assert" {
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
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value().to_string(),
            _ => return None,
        };
        if func_name != "assert" {
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
                if let Some((cls_name, negated)) =
                    try_extract_instanceof_with_negation(condition, ctx.var_name)
                {
                    if negated {
                        apply_instanceof_exclusion(&cls_name, ctx, results);
                    } else {
                        apply_instanceof_inclusion(&cls_name, ctx, results);
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
                if let Some((cls_name, negated)) =
                    try_extract_instanceof_with_negation(cond_expr.condition, ctx.var_name)
                {
                    if negated {
                        apply_instanceof_exclusion(&cls_name, ctx, results);
                    } else {
                        apply_instanceof_inclusion(&cls_name, ctx, results);
                    }
                }
            } else if in_else
                && let Some((cls_name, negated)) =
                    try_extract_instanceof_with_negation(cond_expr.condition, ctx.var_name)
            {
                // Flip polarity for the else branch.
                if negated {
                    apply_instanceof_inclusion(&cls_name, ctx, results);
                } else {
                    apply_instanceof_exclusion(&cls_name, ctx, results);
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

    // ── instanceof / is_a / get_class / ::class narrowing ──
    // The then-body exits, so subsequent code is the "else" — apply
    // the inverse of the condition.
    if let Some((cls_name, negated)) =
        try_extract_instanceof_with_negation(if_stmt.condition, ctx.var_name)
    {
        // Positive instanceof + exit → exclude after (var is NOT that class)
        // Negated instanceof + exit → include after (var IS that class)
        if negated {
            apply_instanceof_inclusion(&cls_name, ctx, results);
        } else {
            apply_instanceof_exclusion(&cls_name, ctx, results);
        }
    }

    // ── @phpstan-assert-if-true / @phpstan-assert-if-false ──
    // When a function with assert-if-true/false is the condition and
    // the then-body exits, the code after runs when the function
    // returned the opposite boolean — apply the inverse narrowing.
    let (func_call_expr, condition_negated) = unwrap_condition_negation(if_stmt.condition);

    if let Expression::Call(Call::Function(func_call)) = func_call_expr {
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value().to_string(),
            _ => return,
        };
        let func_info = match ctx.function_loader {
            Some(fl) => match fl(&func_name) {
                Some(fi) => fi,
                None => return,
            },
            None => return,
        };

        // The then-body exits, so we're in the "else" conceptually.
        // inverted=true, same logic as try_apply_assert_condition_narrowing
        let function_returned_true = condition_negated;

        for assertion in &func_info.type_assertions {
            let applies_positively = match assertion.kind {
                AssertionKind::IfTrue => function_returned_true,
                AssertionKind::IfFalse => !function_returned_true,
                AssertionKind::Always => continue,
            };

            if let Some(arg_var) = find_assertion_arg_variable(
                &func_call.argument_list,
                &assertion.param_name,
                &func_info.parameters,
            ) && arg_var == ctx.var_name
            {
                let should_exclude = assertion.negated ^ !applies_positively;
                if should_exclude {
                    apply_instanceof_exclusion(&assertion.asserted_type, ctx, results);
                } else {
                    apply_instanceof_inclusion(&assertion.asserted_type, ctx, results);
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
) -> Option<String> {
    let raw_type =
        crate::completion::variable::resolution::resolve_arg_raw_type(haystack_expr, ctx)?;
    docblock::types::extract_iterable_element_type(&raw_type)
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
            apply_instanceof_exclusion(&element_type, ctx, results);
        } else {
            apply_instanceof_inclusion(&element_type, ctx, results);
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
            apply_instanceof_inclusion(&element_type, ctx, results);
        } else {
            apply_instanceof_exclusion(&element_type, ctx, results);
        }
    }
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
            apply_instanceof_inclusion(&element_type, ctx, results);
        } else {
            apply_instanceof_exclusion(&element_type, ctx, results);
        }
    }
}
