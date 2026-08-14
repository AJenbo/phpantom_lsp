use super::*;
use std::collections::HashMap;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::argument::Argument;

use crate::atom::{Atom, atom, bytes_to_str};
use crate::php_type::{LiteralValue, PhpType, TypeKind};
use crate::type_engine::resolver::VarResolutionCtx;
use crate::type_engine::types::narrowing;
use crate::types::{MethodInfo, PropertyInfo, ResolvedType};

// ─── Completion-path ternary/match(true) narrowing ──────────────────────────

/// Walk an expression tree looking for a `match(true)` arm or ternary
/// `instanceof` branch that contains the cursor.  When found, apply
/// the appropriate narrowing to `scope` so that variable lookups see
/// the narrowed type.
///
/// This is the completion-path counterpart of
/// [`record_match_ternary_snapshots`], which records scope snapshots
/// for the diagnostic path.  Here we modify the live scope in-place
/// because the completion path only needs one variable's type at one
/// cursor position.
pub(crate) fn apply_cursor_ternary_narrowing<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let cursor = ctx.cursor_offset;
    let span = expr.span();
    if cursor < span.start.offset || cursor > span.end.offset {
        return;
    }

    match expr {
        Expression::Match(match_expr) if match_expr.expression.is_true() => {
            for arm in match_expr.arms.iter() {
                match arm {
                    MatchArm::Expression(expr_arm) => {
                        let arm_span = expr_arm.expression.span();
                        if cursor >= arm_span.start.offset && cursor <= arm_span.end.offset {
                            for condition in expr_arm.conditions.iter() {
                                apply_condition_narrowing(condition, scope, ctx);
                            }
                            // Recurse into the arm body for nested patterns.
                            apply_cursor_ternary_narrowing(expr_arm.expression, scope, ctx);
                            return;
                        }
                    }
                    MatchArm::Default(def_arm) => {
                        let arm_span = def_arm.expression.span();
                        if cursor >= arm_span.start.offset && cursor <= arm_span.end.offset {
                            apply_cursor_ternary_narrowing(def_arm.expression, scope, ctx);
                            return;
                        }
                    }
                }
            }
        }
        Expression::Conditional(conditional) => {
            // Check if the condition contains an instanceof check, a
            // member-existence proof
            // (`property_exists`/`method_exists`/`isset($x->prop)`), or a
            // null/false/truthiness guard (`$x !== null`, `isset($x)`, the
            // bare `$x` check) for any variable currently in scope.
            let has_narrowing = {
                let var_names: Vec<Atom> = scope.locals.keys().copied().collect();
                var_names.iter().any(|vn| {
                    narrowing::try_extract_instanceof(conditional.condition, vn).is_some()
                        || narrowing::try_extract_instanceof_with_negation(
                            conditional.condition,
                            vn,
                        )
                        .is_some()
                        || narrowing::try_extract_compound_or_instanceof(conditional.condition, vn)
                            .is_some()
                })
            } || condition_proves_member(conditional.condition, scope)
                || condition_proves_null_or_truthy(conditional.condition)
                || !assertion_alias_extractions(conditional.condition, scope).is_empty();
            if has_narrowing {
                if let Some(then_expr) = conditional.then {
                    let then_span = then_expr.span();
                    if cursor >= then_span.start.offset && cursor <= then_span.end.offset {
                        apply_condition_narrowing(conditional.condition, scope, ctx);
                        apply_cursor_ternary_narrowing(then_expr, scope, ctx);
                        return;
                    }
                }
                let else_span = conditional.r#else.span();
                if cursor >= else_span.start.offset && cursor <= else_span.end.offset {
                    apply_condition_narrowing_inverse(conditional.condition, scope, ctx);
                    apply_cursor_ternary_narrowing(conditional.r#else, scope, ctx);
                }
            } else {
                // No instanceof — just recurse for nested patterns.
                if let Some(then_expr) = conditional.then {
                    apply_cursor_ternary_narrowing(then_expr, scope, ctx);
                }
                apply_cursor_ternary_narrowing(conditional.r#else, scope, ctx);
            }
        }
        Expression::Assignment(assignment) => {
            apply_cursor_ternary_narrowing(assignment.rhs, scope, ctx);
        }
        Expression::Parenthesized(inner) => {
            apply_cursor_ternary_narrowing(inner.expression, scope, ctx);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => {
                    apply_cursor_ternary_narrowing(fc.function, scope, ctx);
                    &fc.argument_list
                }
                Call::Method(mc) => {
                    apply_cursor_ternary_narrowing(mc.object, scope, ctx);
                    &mc.argument_list
                }
                Call::NullSafeMethod(mc) => {
                    apply_cursor_ternary_narrowing(mc.object, scope, ctx);
                    &mc.argument_list
                }
                Call::StaticMethod(_) => return,
            };
            for arg in args.arguments.iter() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                apply_cursor_ternary_narrowing(arg_expr, scope, ctx);
            }
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            // `&&` chain: apply narrowing from LHS operands when the
            // cursor is in the RHS.  E.g. `$x instanceof Foo && $x->bar()`
            // narrows `$x` to `Foo` for the `$x->bar()` operand.
            let operands = collect_and_chain_operands(expr);
            if operands.len() >= 2 {
                let mut narrowed = false;
                for (i, operand) in operands.iter().enumerate() {
                    let op_span = operand.span();
                    if cursor >= op_span.start.offset && cursor <= op_span.end.offset {
                        // Cursor is inside this operand — apply
                        // narrowing from all preceding operands.
                        // (Already applied cumulatively in the loop.)
                        narrowed = true;
                        apply_cursor_ternary_narrowing(operand, scope, ctx);
                        break;
                    }
                    // Apply this operand's narrowing for subsequent operands.
                    if i < operands.len() - 1 {
                        apply_condition_narrowing(operand, scope, ctx);
                    }
                }
                if !narrowed {
                    // Cursor not inside any operand — just recurse.
                    apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
                    apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
                }
            } else {
                apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
                apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
            }
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::Or(_) | BinaryOperator::LowOr(_)
            ) =>
        {
            // `||` chain: the right operand executes only when the
            // preceding operands are false, so apply the *inverse*
            // narrowing from those operands when the cursor is in a
            // later operand.  E.g. `!$x instanceof Foo || $x->bar()`
            // narrows `$x` to `Foo` for the `$x->bar()` operand.
            let operands = collect_or_chain_operands(expr);
            if operands.len() >= 2 {
                let mut narrowed = false;
                for (i, operand) in operands.iter().enumerate() {
                    let op_span = operand.span();
                    if cursor >= op_span.start.offset && cursor <= op_span.end.offset {
                        narrowed = true;
                        apply_cursor_ternary_narrowing(operand, scope, ctx);
                        break;
                    }
                    // Apply this operand's inverse narrowing for the
                    // subsequent operands.
                    if i < operands.len() - 1 {
                        apply_condition_narrowing_inverse(operand, scope, ctx);
                    }
                }
                if !narrowed {
                    apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
                    apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
                }
            } else {
                apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
                apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
            }
        }
        Expression::Binary(bin) => {
            apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
            apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
        }
        // Non-`true` match expressions.  `match ($x::class)` still proves
        // which class the subject is in each arm.
        Expression::Match(match_expr) => {
            let subject_var = narrowing::match_class_subject_var(match_expr.expression);
            for arm in match_expr.arms.iter() {
                let arm_expr = match arm {
                    MatchArm::Expression(e) => e.expression,
                    MatchArm::Default(d) => d.expression,
                };
                let arm_span = arm_expr.span();
                if cursor < arm_span.start.offset || cursor > arm_span.end.offset {
                    continue;
                }
                if let (Some(var), MatchArm::Expression(expr_arm)) = (subject_var, arm) {
                    apply_class_match_arm_narrowing(var, expr_arm, scope, ctx);
                }
                apply_cursor_ternary_narrowing(arm_expr, scope, ctx);
                return;
            }
        }
        _ => {}
    }
}

// ─── Boolean variables that stand for a check ───────────────────────────────

/// Record the checks a boolean assignment carries.
///
/// `$isHtml = $raw instanceof HtmlString;` proves nothing on its own,
/// but `$isHtml` now stands for the check: wherever it is tested, `$raw`
/// narrows the same way the original expression would narrow it.
///
/// Only the `&&` conjuncts that are themselves `instanceof`-style checks
/// are recorded; anything else in the expression (`$flag`, a comparison,
/// a call) contributes no assertion and is skipped, which still leaves
/// the recorded conjuncts sound for a truthy test.
pub(crate) fn record_assertion_variable<'b>(
    lhs_name: &str,
    rhs: &'b Expression<'b>,
    scope: &mut ScopeState,
) {
    let mut checks: Vec<VarAssertion> = Vec::new();
    for operand in collect_and_chain_operands(rhs) {
        let mut subjects = collect_condition_var_names(operand);
        subjects.extend(collect_condition_property_keys(operand));
        for subject in subjects {
            // `$x = $x instanceof Foo` overwrites its own subject, so the
            // recorded check would describe a value that no longer exists.
            if subject == lhs_name {
                continue;
            }
            if let Some(extraction) =
                narrowing::try_extract_instanceof_with_negation(operand, &subject)
            {
                checks.push(VarAssertion {
                    subject: atom(&subject),
                    class_type: extraction.class_type,
                    negated: extraction.negated,
                    exact: extraction.exact,
                });
                break;
            }
        }
    }
    if !checks.is_empty() {
        scope.assertions.insert(atom(lhs_name), checks);
    }
}

/// Expand a bare boolean operand into the checks it stands for.
///
/// `$isHtml` and `!$isHtml` both resolve through the recorded check,
/// with the operand's own negation folded into the result, so the
/// callers below treat them exactly like the original `instanceof`
/// expression.
///
/// A boolean built from several conjuncts only proves its parts when it
/// is true: `!$ok` says one of them failed without saying which, so a
/// negated operand expands only a single-check boolean.
pub(in crate::type_engine) fn assertion_alias_extractions(
    expr: &Expression<'_>,
    scope: &ScopeState,
) -> Vec<(String, narrowing::InstanceofExtraction)> {
    if scope.assertions.is_empty() {
        return Vec::new();
    }

    let mut negated = false;
    let mut inner = expr;
    loop {
        match inner {
            Expression::Parenthesized(p) => inner = p.expression,
            Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
                negated = !negated;
                inner = prefix.operand;
            }
            _ => break,
        }
    }

    let Expression::Variable(Variable::Direct(dv)) = inner else {
        return Vec::new();
    };
    let Some(checks) = scope.assertions.get(&atom(bytes_to_str(dv.name))) else {
        return Vec::new();
    };
    if negated && checks.len() > 1 {
        return Vec::new();
    }

    checks
        .iter()
        .map(|c| {
            (
                c.subject.to_string(),
                narrowing::InstanceofExtraction {
                    class_type: c.class_type.clone(),
                    negated: c.negated != negated,
                    exact: c.exact,
                },
            )
        })
        .collect()
}

// ─── Narrowing helpers ──────────────────────────────────────────────────────

/// Narrow a `match ($x::class)` subject to the classes one arm names.
///
/// `match ($node::class) { ASTClass::class, ASTEnum::class => … }` proves
/// the subject is one of the listed classes inside that arm, exactly like
/// a chain of `instanceof` checks would — except the identity is exact, so
/// no subclass survives.
pub(crate) fn apply_class_match_arm_narrowing<'b>(
    subject_var: &str,
    expr_arm: &'b MatchExpressionArm<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let classes: Vec<PhpType> = expr_arm
        .conditions
        .iter()
        .filter_map(|c| narrowing::class_match_condition_class(c))
        .collect();
    if classes.is_empty() {
        return;
    }

    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };
    let var_ctx = build_var_ctx(subject_var, ctx, &scope_resolver);
    let union = narrowing::resolve_class_names_to_union(&classes, &var_ctx);
    if union.is_empty() {
        return;
    }
    scope.set(
        subject_var,
        union.into_iter().map(ResolvedType::from_class).collect(),
    );
}

/// Where one subject's accumulated classes came from while walking a
/// condition's `&&` operands, which decides whether they are
/// alternatives or members of an intersection.
#[derive(Default)]
struct Conjuncts {
    /// How many operands contributed a positive `instanceof` naming a
    /// single class.
    operands: usize,
    /// A `||` chain contributed, so at least one contribution is a set of
    /// alternatives rather than a class the value definitely is.
    saw_alternatives: bool,
}

impl Conjuncts {
    /// Whether the accumulated classes describe one value that is all of
    /// them at once.
    ///
    /// `$x instanceof A && $x instanceof B` proves both, so the value is
    /// `A&B`.  One operand on its own proves a single class, and a `||`
    /// chain proves only that the value is one of its members, so
    /// neither concludes an intersection.
    fn is_intersection(&self) -> bool {
        self.operands > 1 && !self.saw_alternatives
    }
}

/// Apply condition-based narrowing (instanceof, null check, type guard)
/// to the scope.  This narrows types for the "truthy" branch.
pub(crate) fn apply_condition_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Seed property access keys from conditions into the scope so that
    // narrowing functions can find and narrow them.
    seed_property_keys_into_scope(condition, scope, ctx);

    // Decompose `&&` chains so that `$x instanceof Foo && $x instanceof Bar`
    // applies both narrowings as a union (intersection semantics: the
    // variable satisfies both checks, so members from both types are
    // available).
    let operands = collect_and_chain_operands(condition);

    // First pass: collect all instanceof extractions per variable across
    // all `&&` operands.  This prevents later operands from overwriting
    // earlier ones when both narrow the same variable.
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };
    let mut var_names: Vec<String> = scope.locals.keys().map(|k| k.to_string()).collect();
    // Include variables from instanceof conditions that may not be in
    // scope yet (e.g. undeclared variables used in instanceof checks).
    for name in collect_condition_var_names(condition) {
        if !var_names.contains(&name) {
            var_names.push(name);
        }
    }
    // Include property access keys from conditions (e.g. `$a->foo`
    // from `$a->foo instanceof Foo`) so instanceof narrowing applies.
    for key in collect_condition_property_keys(condition) {
        if !var_names.contains(&key) {
            var_names.push(key);
        }
    }
    // Expand operands that are a bare boolean standing for a check
    // (`$isHtml` from `$isHtml = $raw instanceof HtmlString`) into the
    // check itself, and make sure its subject is narrowed below even
    // when the condition never names it.
    let alias_extractions: Vec<Vec<(String, narrowing::InstanceofExtraction)>> = operands
        .iter()
        .map(|operand| assertion_alias_extractions(operand, scope))
        .collect();
    for subject in alias_extractions.iter().flatten().map(|(s, _)| s) {
        if !var_names.contains(subject) {
            var_names.push(subject.clone());
        }
    }

    // Track which variables have been narrowed by instanceof across
    // `&&` operands so we can merge them, plus where each subject's
    // classes came from so the merge knows whether they are alternatives
    // or an intersection.
    let mut instanceof_results: HashMap<String, Vec<ResolvedType>> = HashMap::new();
    let mut conjuncts: HashMap<String, Conjuncts> = HashMap::new();

    for (op_idx, operand) in operands.iter().enumerate() {
        for var_name in &var_names {
            // Compound OR instanceof: `$x instanceof A || $x instanceof B`
            if let Some(classes) = narrowing::try_extract_compound_or_instanceof(operand, var_name)
                && !classes.is_empty()
            {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                let union = narrowing::resolve_class_names_to_union(&classes, &var_ctx);
                if !union.is_empty() {
                    let entry = instanceof_results.entry(var_name.clone()).or_default();
                    ResolvedType::extend_unique(
                        entry,
                        union.into_iter().map(ResolvedType::from_class).collect(),
                    );
                    conjuncts
                        .entry(var_name.clone())
                        .or_default()
                        .saw_alternatives = true;
                }
                continue;
            }

            // Single instanceof (including negated, is_a, get_class),
            // or a boolean that stands for one.
            if let Some(extraction) =
                narrowing::try_extract_instanceof_with_negation(operand, var_name).or_else(|| {
                    alias_extractions[op_idx]
                        .iter()
                        .find(|(subject, _)| subject == var_name)
                        .map(|(_, e)| e.clone())
                })
            {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                if extraction.negated {
                    // Negated instanceof: apply exclusion to the current
                    // scope immediately (each negation removes one type).
                    let mut results = scope.get(var_name).to_vec();
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_exclusion(
                            &extraction.class_type,
                            &var_ctx,
                            classes,
                        )
                    });
                    // Negated instanceof exclusion does NOT eliminate
                    // null — `!$x instanceof Foo` is true when $x is
                    // null, so null stays in the union.  No stripping.
                    if !results.is_empty() {
                        scope.set(var_name, results);
                    }
                } else {
                    // Positive instanceof: resolve and accumulate into
                    // the per-variable union.  For a single operand this
                    // produces `[Foo]`; for `&& instanceof Bar` it
                    // accumulates `[Foo, Bar]`.
                    let mut single = Vec::new();
                    ResolvedType::apply_narrowing(&mut single, |classes| {
                        narrowing::apply_instanceof_inclusion(
                            &extraction.class_type,
                            extraction.exact,
                            &var_ctx,
                            classes,
                        )
                    });
                    if !single.is_empty() {
                        let entry = instanceof_results.entry(var_name.clone()).or_default();
                        ResolvedType::extend_unique(entry, single);
                        conjuncts.entry(var_name.clone()).or_default().operands += 1;
                    } else {
                        // Target class is unresolvable — mark variable
                        // as empty so diagnostics suppress false positives.
                        instanceof_results.entry(var_name.clone()).or_default();
                    }
                }
            }
        }
    }

    // Apply the accumulated instanceof narrowing results to the scope.
    for (var_name, narrowed) in instanceof_results {
        // `$x instanceof A && $x instanceof B` proves both at once, so the
        // classes gathered across the operands are members of `A&B`
        // rather than alternatives a consumer may pick one of.
        let intersected = conjuncts
            .get(&var_name)
            .is_some_and(Conjuncts::is_intersection);
        commit_instanceof_narrowing(
            &var_name,
            narrowed,
            intersected,
            scope,
            ctx,
            &scope_resolver,
        );
    }

    // Type guard narrowing: `is_object($x)`, `is_array($x)`, etc.
    apply_type_guard_narrowing_truthy(condition, scope);

    // A check on `$x->prop` discriminates a union of objects when only
    // some of them declare a `prop` that could have passed it.
    apply_property_discriminant_narrowing(condition, scope, ctx, true);

    // `is_a($x, Class::class, true)` / `class_exists($x)` narrowing:
    // narrow a string-typed `$x` to `class-string<Class>` / `class-string`.
    apply_class_string_guard_narrowing(condition, scope, ctx, true);

    // Null narrowing: `if ($x !== null)` — remove null from scope.
    apply_null_narrowing_truthy(condition, scope, ctx);

    // A proof about a `?->` chain's value is a proof about its receivers.
    apply_nullsafe_receiver_narrowing(condition, scope, ctx, true);

    // @phpstan-assert-if-true / -if-false narrowing.
    apply_phpstan_assert_condition_narrowing(condition, scope, ctx, false);

    // in_array($var, $haystack, true) narrowing.
    apply_in_array_narrowing(condition, scope, ctx, false);

    // property_exists($var, 'name') / method_exists($var, 'name') narrowing.
    apply_member_exists_narrowing(condition, scope, false);
}

/// Write the outcome of a *successful* `instanceof` check on `var_name`
/// into the scope.
///
/// `narrowed` holds the classes the check proves the value is; the
/// variable's current types decide how they combine with what was already
/// known.  Both polarities of the check reach this: a positive
/// `instanceof` in a truthy branch, and the fall-through of a
/// `if (!$x instanceof T) { return; }` guard, which proves exactly the
/// same thing.  Keeping one implementation is what stops the guard form
/// from drifting back into *adding* `T` to the union instead of
/// filtering it down to `T`.
fn commit_instanceof_narrowing(
    var_name: &str,
    mut narrowed: Vec<ResolvedType>,
    intersected: bool,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    scope_resolver: &dyn Fn(&str) -> Vec<ResolvedType>,
) {
    if intersected {
        ResolvedType::tag_as_intersection(&mut narrowed);
    }
    if narrowed.is_empty() {
        // Empty narrowed list means the target was unresolvable.
        scope.locals.insert(atom(var_name), vec![]);
        return;
    }

    let existing = scope.get(var_name);
    if existing.is_empty() {
        // Untyped variable — instanceof provides the type.
        scope.set(var_name, narrowed);
        return;
    }

    // When the existing type says no more than `mixed` or `object`,
    // instanceof replaces it — there is no useful information to preserve
    // or intersect.  `null` members count as broad too: a successful check
    // rules them out, so `object|null` is as uninformative as bare
    // `object`.
    let is_broad_atom = |ty: &PhpType| {
        ty.is_null()
            || matches!(
                ty.kind(),
                TypeKind::Named(n) if n.eq_ignore_ascii_case("mixed") || n.eq_ignore_ascii_case("object")
            )
    };
    let all_broad = existing.iter().all(|rt| {
        rt.class_info.is_none()
            && match rt.type_string.non_null_type() {
                Some(non_null) => is_broad_atom(&non_null),
                None => is_broad_atom(&rt.type_string),
            }
    });
    if all_broad {
        scope.set(var_name, narrowed);
        return;
    }

    // Typed variable — filter the existing union to only types present in
    // the narrowed set.  This correctly handles both single instanceof
    // (`Dog|Cat` → `Dog`) and OR instanceof (`Dog|Cat|Other` → `Dog|Cat`).
    //
    // When the narrowed type is NOT in the existing union (e.g.
    // `MockInterface` narrowed to `MolliePayment`), this is an
    // intersection case — apply via apply_instanceof_inclusion which has
    // interface intersection logic.
    let narrowed_fqns: Vec<String> = narrowed
        .iter()
        .filter_map(|rt| rt.class_info.as_ref().map(|c| c.fqn().to_string()))
        .collect();

    // Try filtering: keep existing entries whose class is in the narrowed
    // set.  A kept entry's own type_string may still be the whole
    // pre-check union (a conditional return type resolves to one entry
    // naming a class and listing an array alternative beside it), so
    // restrict it to the narrowed classes as well.  Strip null on top
    // because a successful instanceof check guarantees the value is
    // non-null (e.g. `?Foo` → `Foo`).
    let survives = |name: &str| {
        narrowed.iter().any(|rt| {
            rt.class_info
                .as_ref()
                .is_some_and(|c| c.name == name || c.fqn() == name)
        })
    };
    let filtered: Vec<ResolvedType> = existing
        .iter()
        .filter(|rt| {
            rt.class_info
                .as_ref()
                .is_some_and(|c| narrowed_fqns.contains(&c.fqn().to_string()))
        })
        .map(|rt| {
            let mut rt = rt.clone();
            rt.restrict_type_string_to_classes(&survives);
            if let Some(non_null) = rt.type_string.non_null_type() {
                rt.type_string = non_null;
            }
            rt
        })
        .collect();

    if !filtered.is_empty() {
        // Filter matched — use the filtered results (preserves richer type
        // info from original resolution).  Also strip bare `null` entries:
        // a successful instanceof check guarantees non-null, so `null`
        // entries added by `from_classes_with_hint` must be removed.
        let mut filtered: Vec<ResolvedType> = filtered
            .into_iter()
            .filter(|rt| !rt.type_string.is_null())
            .collect();
        if intersected {
            ResolvedType::tag_as_intersection(&mut filtered);
        }
        if filtered.is_empty() {
            scope.set(var_name, narrowed);
        } else {
            scope.set(var_name, filtered);
        }
        return;
    }

    // No overlap between existing and narrowed types.  This is the
    // intersection case (e.g. MockInterface narrowed to MolliePayment).
    // Use apply_instanceof_inclusion which produces the intersection when
    // one side is an interface.
    let mut results = existing.to_vec();
    // Apply all narrowed classes as a single group by building a union type.
    let union_type = if narrowed_fqns.len() == 1 {
        PhpType::named(atom(&narrowed_fqns[0]))
    } else {
        PhpType::union(
            narrowed_fqns
                .iter()
                .map(|n| PhpType::named(atom(n)))
                .collect(),
        )
    };
    let var_ctx = build_var_ctx(var_name, ctx, scope_resolver);
    ResolvedType::apply_narrowing(&mut results, |classes| {
        narrowing::apply_instanceof_inclusion(&union_type, false, &var_ctx, classes)
    });
    // Instanceof guarantees non-null — strip bare `null` entries that were
    // preserved by `apply_narrowing`'s `None => true` rule.
    results.retain(|rt| !rt.type_string.is_null());
    // `apply_instanceof_inclusion` merging in an unrelated interface (the
    // branch this call site exists for) leaves both classes in `results` as
    // separate entries, which describe one value that is both at once.
    ResolvedType::tag_as_intersection(&mut results);
    if !results.is_empty() {
        scope.set(var_name, results);
    } else {
        // Fallback: use the narrowed types directly.
        scope.set(var_name, narrowed);
    }
}

/// Apply inverse narrowing for a single condition expression (not
/// decomposed).  Called by [`apply_condition_narrowing_inverse`] for
/// each operand in a `&&` chain, or for the whole condition when it
/// is not a chain.
pub(crate) fn apply_condition_narrowing_inverse_single<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Seed property access keys from conditions into the scope so that
    // narrowing functions can find and narrow them.
    seed_property_keys_into_scope(condition, scope, ctx);

    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };
    // Include variables from instanceof conditions that may not be in
    // scope yet (e.g. `if (!$foobar instanceof Foobar) { break; }`
    // where `$foobar` was never assigned).  After the guard clause,
    // `$foobar` must be `Foobar`.
    let mut var_names: Vec<String> = scope.locals.keys().map(|k| k.to_string()).collect();
    for name in collect_condition_var_names(condition) {
        if !var_names.contains(&name) {
            var_names.push(name);
        }
    }
    // Include property access keys from conditions (e.g. `$a->foo`
    // from `$a->foo instanceof Foo`) so instanceof narrowing applies.
    for key in collect_condition_property_keys(condition) {
        if !var_names.contains(&key) {
            var_names.push(key);
        }
    }
    // A bare boolean standing for a check inverts along with the rest of
    // the condition: `if (!$isHtml) { return; }` leaves `$raw` narrowed
    // to `HtmlString` after the guard.
    let alias_extractions = assertion_alias_extractions(condition, scope);
    for (subject, _) in &alias_extractions {
        if !var_names.contains(subject) {
            var_names.push(subject.clone());
        }
    }
    for var_name in &var_names {
        if let Some(classes) = narrowing::try_extract_compound_or_instanceof(condition, var_name)
            && !classes.is_empty()
        {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let mut results = scope.get(var_name).to_vec();
            for cls_type in &classes {
                ResolvedType::apply_narrowing(&mut results, |class_list| {
                    narrowing::apply_instanceof_exclusion(cls_type, &var_ctx, class_list)
                });
            }
            if !results.is_empty() {
                scope.set(var_name, results);
            }
            continue;
        }

        if let Some(extraction) =
            narrowing::try_extract_instanceof_with_negation(condition, var_name).or_else(|| {
                alias_extractions
                    .iter()
                    .find(|(subject, _)| subject == var_name)
                    .map(|(_, e)| e.clone())
            })
        {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            if extraction.negated {
                // Inverse of negated instanceof → positive instanceof,
                // which proves exactly what the truthy branch of the
                // un-negated check proves.  Resolve the asserted classes
                // on their own and hand them to the shared commit so the
                // existing union is *filtered* down to them rather than
                // extended with them.
                let mut narrowed = Vec::new();
                ResolvedType::apply_narrowing(&mut narrowed, |classes| {
                    narrowing::apply_instanceof_inclusion(
                        &extraction.class_type,
                        extraction.exact,
                        &var_ctx,
                        classes,
                    )
                });
                commit_instanceof_narrowing(var_name, narrowed, false, scope, ctx, &scope_resolver);
            } else {
                // Inverse of positive instanceof → exclusion.
                // Exclusion does NOT strip null (`!instanceof` is
                // true for null values).
                let mut results = scope.get(var_name).to_vec();
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_exclusion(&extraction.class_type, &var_ctx, classes)
                });
                if !results.is_empty() {
                    scope.set(var_name, results);
                }
            }
        }
    }

    // Inverse member-existence narrowing: after a guard clause like
    // `if (!property_exists($x, 'name')) { return; }`, the member is
    // known to exist.
    apply_member_exists_narrowing(condition, scope, true);

    // A union of objects the check on `$x->prop` could only have failed
    // for some of.  Callers hand this one operand at a time, so the
    // `&&` / `||` decomposition is already done.
    apply_property_discriminant_narrowing(condition, scope, ctx, false);
}

/// Apply inverse condition-based narrowing (for else branches and
/// guard clauses).
pub(crate) fn apply_condition_narrowing_inverse<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // De Morgan over `||`: NOT (A || B) = !A && !B.  Every operand's inverse
    // holds at the same time, so they apply sequentially to one scope.  This
    // is what makes the `if (!guard1 || !guard2) { return; }` idiom narrow
    // its fall-through by each conjunct.
    let or_operands = collect_or_chain_operands(condition);
    if or_operands.len() > 1 {
        for operand in &or_operands {
            // Recurse rather than calling the single-operand form directly,
            // so a nested `&&` inside one `||` operand is decomposed too.
            apply_condition_narrowing_inverse(operand, scope, ctx);
        }
        return;
    }

    // De Morgan over `&&`: NOT (A && B) = !A || !B.  The operands are
    // alternatives, not simultaneous facts, so each contributes one branch
    // of a union: narrow a clone per operand, then merge.
    let and_operands = collect_and_chain_operands(condition);
    if and_operands.len() > 1 {
        let base_scope = scope.clone();
        let mut branch_scopes: Vec<ScopeState> = Vec::new();
        for operand in &and_operands {
            let mut branch = base_scope.clone();
            apply_condition_narrowing_inverse(operand, &mut branch, ctx);
            branch_scopes.push(branch);
        }
        if let Some(first) = branch_scopes.first() {
            let mut merged = first.clone();
            for branch in &branch_scopes[1..] {
                merged.merge_branch(branch);
            }
            *scope = merged;
        }
        return;
    }

    apply_condition_narrowing_inverse_operand(condition, scope, ctx);
}

/// The variable overrides a condition establishes for one polarity, ready to
/// hand to [`VarResolutionCtx::with_match_arm_narrowing`].
///
/// This is how the expression resolvers (ternary arms, short-circuit
/// operands) get their narrowing: they run the *same* pipeline `if`/`else`
/// bodies use, over a scope seeded with just the subjects the condition
/// names, and keep whatever it changed. Every rule added to the pipeline
/// therefore reaches every expression position for free.
///
/// Returns an empty map when the condition narrows nothing, which callers
/// use to skip building a derived context at all.
pub(crate) fn condition_narrowing_overrides<'b>(
    condition: &'b Expression<'b>,
    truthy: bool,
    ctx: &VarResolutionCtx<'_>,
) -> HashMap<String, Vec<ResolvedType>> {
    let Some(resolver) = ctx.scope_var_resolver else {
        return HashMap::new();
    };

    let mut subjects: Vec<String> = Vec::new();
    collect_condition_subject_vars(condition, &mut subjects);
    for key in collect_condition_property_keys(condition) {
        if !subjects.contains(&key) {
            subjects.push(key);
        }
    }
    if subjects.is_empty() {
        return HashMap::new();
    }

    let mut scope = ScopeState::new();
    for subject in &subjects {
        let types = resolver(subject);
        if !types.is_empty() {
            scope.set(subject, types);
        }
    }
    if scope.locals.is_empty() {
        return HashMap::new();
    }

    let seeded = scope.locals.clone();
    let walk_ctx = ForwardWalkCtx::from_var_ctx(ctx);
    if truthy {
        apply_condition_narrowing(condition, &mut scope, &walk_ctx);
    } else {
        apply_condition_narrowing_inverse(condition, &mut scope, &walk_ctx);
    }

    scope
        .locals
        .into_iter()
        .filter(|(name, types)| {
            !types.is_empty()
                && seeded
                    .get(name)
                    .is_none_or(|before| narrowing_changed_types(before, types))
        })
        .map(|(name, types)| (name.to_string(), types))
        .collect()
}

/// Apply every inverse narrowing rule to a condition that is no longer a
/// `&&`/`||` chain.
///
/// [`apply_condition_narrowing_inverse`] does the De Morgan decomposition and
/// hands each leaf here, so every rule sees the operand it can actually match
/// instead of the compound expression wrapping it.
fn apply_condition_narrowing_inverse_operand<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    apply_condition_narrowing_inverse_single(condition, scope, ctx);

    // Inverse type guard narrowing: `if (is_object($x))` in else → exclude object.
    apply_type_guard_narrowing_inverse(condition, scope);

    // Inverse class-string guard narrowing: `if (!is_a($x, Class::class, true))`
    // guard clause → after it, `$x` is a class-string of `Class`.
    apply_class_string_guard_narrowing(condition, scope, ctx, false);

    // Inverse null narrowing: `if ($x === null)` after guard → remove null.
    apply_null_narrowing_inverse(condition, scope, ctx);

    // A proof about a `?->` chain's value is a proof about its receivers.
    apply_nullsafe_receiver_narrowing(condition, scope, ctx, false);

    // Inverse @phpstan-assert-if-true / -if-false narrowing.
    apply_phpstan_assert_condition_narrowing(condition, scope, ctx, true);

    // Inverse in_array narrowing: exclude the element type in the else branch.
    apply_in_array_narrowing(condition, scope, ctx, true);
}

/// Report whether `condition` contains a member-existence proof for any
/// variable currently in `scope`: `property_exists($x, 'name')`,
/// `method_exists($x, 'name')`, or `isset($x->name)` (all recognised by
/// [`narrowing::try_extract_member_exists_guard`]).
///
/// Ternary branch narrowing runs only for conditions that add information
/// the guarded branch relies on.  Like `instanceof`, these guards qualify:
/// the then-branch of `property_exists($x, 'p') ? $x->p : …` depends on the
/// proof that `$x->p` exists.
pub(crate) fn condition_proves_member(condition: &Expression<'_>, scope: &ScopeState) -> bool {
    let var_names: Vec<Atom> = scope.locals.keys().copied().collect();
    collect_and_chain_operands(condition).iter().any(|operand| {
        var_names
            .iter()
            .any(|vn| narrowing::try_extract_member_exists_guard(operand, vn.as_str()).is_some())
    })
}

/// Like [`condition_proves_member`], but for the null/false/truthiness
/// guards [`apply_null_narrowing_truthy`] recognises: `$x !== null`,
/// `isset($x)`, `!empty($x)`, `$x !== false`, and the bare `$x` truthy
/// check. The then-branch of `$x ? $x : $default` depends on the proof
/// that `$x` is truthy exactly as much as an `if ($x) { … }` body does.
pub(crate) fn condition_proves_null_or_truthy(condition: &Expression<'_>) -> bool {
    collect_and_chain_operands(condition).iter().any(|operand| {
        extract_non_null_check_var(operand).is_some()
            || extract_non_false_check_var(operand).is_some()
            || !extract_isset_vars(operand).is_empty()
            || !extract_not_isset_vars(operand).is_empty()
            || extract_null_equality_check_var(operand).is_some()
            || extract_not_empty_var(operand).is_some()
            || expr_to_var_name(operand)
                .or_else(|| narrowing::expr_to_subject_key(operand))
                .is_some()
    })
}

/// Apply `property_exists($var, 'name')` / `method_exists($var, 'name')`
/// narrowing to the scope.
///
/// In the branch where the guard holds, each class in the variable's
/// resolved union gains a virtual member of the guarded name (unknown
/// type), mirroring PHPStan's `object&hasProperty('name')` intersection.
/// Member access, completion, and hover inside the branch then treat the
/// member as present instead of reporting it unknown.
///
/// `inverted` is `false` for the truthy branch (a bare guard proves the
/// member exists) and `true` for the inverse path (else branch / after an
/// exiting guard clause), where the *negated* form proves it.
pub(crate) fn apply_member_exists_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    inverted: bool,
) {
    for operand in collect_and_chain_operands(condition) {
        let var_names: Vec<Atom> = scope.locals.keys().copied().collect();
        for var_name in &var_names {
            let Some((member, is_method, negated)) =
                narrowing::try_extract_member_exists_guard(operand, var_name)
            else {
                continue;
            };
            // Only the direction where the guard is known TRUE adds
            // information — "the member does not exist" removes nothing
            // we model.
            if negated != inverted {
                continue;
            }

            let mut results = scope.get(var_name).to_vec();
            let mut changed = false;
            for rt in &mut results {
                let Some(class_info) = &rt.class_info else {
                    continue;
                };
                // Skip when the member is already declared on the class
                // itself — nothing to add, and injecting an untyped
                // virtual member would shadow the declared type.  Only
                // own members are checked (resolving ancestors here
                // would be expensive); guarding a *statically declared
                // inherited* member with `property_exists` is rare, and
                // the cost is an unknown member type inside the branch,
                // never a false diagnostic.
                let already_present = if is_method {
                    class_info.get_method_ci(&member).is_some()
                } else {
                    class_info
                        .properties
                        .iter()
                        .any(|p| p.name.as_str() == member)
                };
                if already_present {
                    continue;
                }
                let mut narrowed = (**class_info).clone();
                if is_method {
                    narrowed
                        .methods
                        .push(Arc::new(MethodInfo::virtual_method(&member, None)));
                } else {
                    narrowed
                        .properties
                        .push(Arc::new(PropertyInfo::virtual_property(&member, None)));
                }
                rt.class_info = Some(Arc::new(narrowed));
                changed = true;
            }
            if changed {
                scope.set(var_name, results);
            }
        }
    }
}

/// Apply `in_array($var, $haystack, true)` narrowing.
///
/// When `inverted` is false (truthy branch / while body), the variable is
/// narrowed to the haystack's element type (inclusion).  When `inverted` is
/// true (else branch / guard clause inverse), the variable is narrowed by
/// excluding the element type.
pub(crate) fn apply_in_array_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inverted: bool,
) {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };

    // Unwrap parentheses and detect negation.
    let (inner, negated) = narrowing::unwrap_condition_negation(condition);

    // Check every variable in scope as the potential needle.
    let var_names: Vec<Atom> = scope.locals.keys().copied().collect();
    for var_name in &var_names {
        if let Some(haystack_expr) = narrowing::try_extract_in_array(inner, var_name) {
            // Resolve the haystack's type from the scope to extract the
            // element type.  This replaces the backward scanner's
            // `resolve_arg_raw_type` with a scope-based lookup.
            let element_type = resolve_in_array_element_type_fw(haystack_expr, scope, ctx);
            let element_type = match element_type {
                Some(et) => et,
                None => continue,
            };

            // Determine whether to include or exclude:
            // - truthy + positive  → include (var IS in haystack)
            // - truthy + negated   → exclude (var is NOT in haystack)
            // - inverse + positive → exclude
            // - inverse + negated  → include
            let should_exclude = inverted ^ negated;

            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let mut results = scope.get(var_name).to_vec();

            if should_exclude {
                // Skip exclusion when it would remove ALL type information.
                let would_remove_all = {
                    let mut test = results.clone();
                    ResolvedType::apply_narrowing(&mut test, |classes| {
                        narrowing::apply_instanceof_exclusion(&element_type, &var_ctx, classes)
                    });
                    test.is_empty()
                };
                if !would_remove_all {
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_exclusion(&element_type, &var_ctx, classes)
                    });
                }
            } else {
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_inclusion(&element_type, false, &var_ctx, classes)
                });
            }

            if !results.is_empty() {
                scope.set(var_name, results);
            }
        }
    }
}

/// Resolve the element type of a haystack expression for `in_array`
/// narrowing, using the forward walker's scope instead of the backward
/// scanner.
pub(crate) fn resolve_in_array_element_type_fw(
    haystack_expr: &Expression<'_>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<PhpType> {
    // If the haystack is a simple variable, look it up in the scope.
    if let Expression::Variable(Variable::Direct(dv)) = haystack_expr {
        let var_name = bytes_to_str(dv.name).to_string();
        let types = scope.get(&var_name);
        if !types.is_empty() {
            let joined = ResolvedType::types_joined(types);
            if let Some(elem) = joined.extract_element_type() {
                return Some(elem.clone());
            }
            // Try extracting value type for generic collections.
            if let Some(val) = joined.extract_value_type(true) {
                return Some(val.clone());
            }
        }
        // Fall back to docblock annotation.
        let offset = haystack_expr.span().start.offset as usize;
        let from_docblock =
            crate::docblock::find_iterable_raw_type_in_source(ctx.content, offset, &var_name)
                .map(|t| crate::util::resolve_php_type_names(&t, ctx.class_loader));
        if let Some(raw) = from_docblock
            && let Some(elem) = raw.extract_element_type()
        {
            return Some(elem.clone());
        }
        return None;
    }

    // For non-variable expressions (method calls, property access, etc.),
    // try resolving via the expression resolution pipeline.
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };
    let var_ctx = build_var_ctx("", ctx, &scope_resolver);
    let raw_type =
        crate::type_engine::variable::resolution::resolve_arg_raw_type(haystack_expr, &var_ctx);
    raw_type.and_then(|t| t.extract_element_type().cloned())
}

/// Apply `@phpstan-assert-if-true` / `@phpstan-assert-if-false` narrowing
/// from a function or static/instance method call used as a condition.
///
/// When `inverted` is false we are in the truthy branch (then-body or
/// while-body).  When `inverted` is true we are in the else branch or
/// applying guard-clause inverse narrowing.
pub(crate) fn apply_phpstan_assert_condition_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inverted: bool,
) {
    use crate::types::AssertionKind;

    // Unwrap parentheses and detect negation (`!func($var)`).
    let (func_call_expr, condition_negated) = narrowing::unwrap_condition_negation(condition);

    let call = match func_call_expr {
        Expression::Call(c) => c,
        _ => return,
    };

    // Determine whether the function returned true in this branch.
    let function_returned_true = !(inverted ^ condition_negated);

    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };

    // Try to extract assertion info from function calls and static method calls.
    match call {
        Call::Function(func_call) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => bytes_to_str(ident.value()).to_string(),
                _ => return,
            };
            let func_name_offset = func_call.function.span().start.offset;
            let func_info = match ctx.loaders.function_loader {
                Some(fl) => match fl(&func_name, func_name_offset) {
                    Some(fi) => fi,
                    None => return,
                },
                None => return,
            };
            if func_info.type_assertions.is_empty() {
                return;
            }
            for assertion in &func_info.type_assertions {
                let applies_positively = match assertion.kind {
                    AssertionKind::IfTrue => function_returned_true,
                    AssertionKind::IfFalse => !function_returned_true,
                    AssertionKind::Always => continue,
                };
                if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                    &func_call.argument_list,
                    &assertion.param_name,
                    &func_info.parameters,
                ) {
                    let should_exclude = assertion.negated ^ !applies_positively;
                    let var_ctx = build_var_ctx(&arg_var, ctx, &scope_resolver);
                    let mut results = scope.get(&arg_var).to_vec();
                    if should_exclude {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_exclusion(
                                &assertion.asserted_type,
                                &var_ctx,
                                classes,
                            )
                        });
                    } else {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_inclusion(
                                &assertion.asserted_type,
                                false,
                                &var_ctx,
                                classes,
                            )
                        });
                    }
                    if !results.is_empty() {
                        scope.set(&arg_var, results);
                    }
                }
            }
        }
        Call::StaticMethod(static_call) => {
            let method_name = match &static_call.method {
                ClassLikeMemberSelector::Identifier(ident) => bytes_to_str(ident.value).to_string(),
                _ => return,
            };
            // Resolve the receiver to a class, handling `self`, `static`,
            // `parent`, and subclass names.
            let receiver = match static_call.class {
                Expression::Identifier(ident) => {
                    let name = bytes_to_str(ident.value());
                    let fqn = crate::util::resolve_name_via_loader(name, ctx.class_loader);
                    (ctx.class_loader)(&fqn).or_else(|| (ctx.class_loader)(name))
                }
                Expression::Self_(_) | Expression::Static(_) => {
                    (ctx.class_loader)(&ctx.current_class.name)
                }
                Expression::Parent(_) => match ctx.current_class.parent_class.as_ref() {
                    Some(parent) => (ctx.class_loader)(parent),
                    None => return,
                },
                _ => return,
            };
            let class_info = match receiver {
                Some(ci) => ci,
                None => return,
            };
            // Search the trait/parent chain so assertions declared on an
            // ancestor (e.g. PHPUnit's `Assert`) are found.  Uses raw class
            // loads only, avoiding a full merge that would poison the shared
            // resolved-class cache mid-walk.
            let method = match narrowing::find_assertion_method_in_chain(
                &class_info,
                &method_name,
                ctx.class_loader,
                &mut Vec::new(),
                0,
            ) {
                Some(m) => m,
                None => return,
            };
            for assertion in &method.type_assertions {
                let applies_positively = match assertion.kind {
                    AssertionKind::IfTrue => function_returned_true,
                    AssertionKind::IfFalse => !function_returned_true,
                    AssertionKind::Always => continue,
                };
                if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                    &static_call.argument_list,
                    &assertion.param_name,
                    &method.parameters,
                ) {
                    let should_exclude = assertion.negated ^ !applies_positively;
                    // Resolve `self`/`static`/`$this` in the asserted type
                    // against the declaring class, not the enclosing class.
                    let resolved_assert_type = if assertion.asserted_type.contains_self_ref() {
                        assertion.asserted_type.replace_self(&class_info.fqn())
                    } else {
                        assertion.asserted_type.clone()
                    };
                    let var_ctx = build_var_ctx(&arg_var, ctx, &scope_resolver);
                    let mut results = scope.get(&arg_var).to_vec();
                    if should_exclude {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_exclusion(
                                &resolved_assert_type,
                                &var_ctx,
                                classes,
                            )
                        });
                    } else {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_inclusion(
                                &resolved_assert_type,
                                false,
                                &var_ctx,
                                classes,
                            )
                        });
                    }
                    if !results.is_empty() {
                        scope.set(&arg_var, results);
                    }
                }
            }
        }
        Call::Method(method_call) => {
            // Instance method: `$var->method()` with `@phpstan-assert-if-true Type $this`
            let receiver_var = match method_call.object {
                Expression::Variable(Variable::Direct(dv)) => bytes_to_str(dv.name).to_string(),
                _ => return,
            };
            let method_name = match &method_call.method {
                ClassLikeMemberSelector::Identifier(ident) => bytes_to_str(ident.value).to_string(),
                _ => return,
            };
            // Resolve the receiver's type to find the method's assertions.
            let receiver_types = scope.get(&receiver_var);
            if receiver_types.is_empty() {
                return;
            }
            // Collect assertions from all candidate classes.
            let mut to_apply: Vec<(crate::php_type::PhpType, bool, String)> = Vec::new();
            for rt in receiver_types {
                let receiver = match (ctx.class_loader)(&rt.type_string.to_string()) {
                    Some(ci) => ci,
                    None => {
                        continue;
                    }
                };
                // Search the trait/parent chain for the method's assertions
                // using raw class loads only (a full merge would poison the
                // shared resolved-class cache mid-walk).
                let method = match narrowing::find_assertion_method_in_chain(
                    &receiver,
                    &method_name,
                    ctx.class_loader,
                    &mut Vec::new(),
                    0,
                ) {
                    Some(m) => m,
                    None => continue,
                };
                for assertion in &method.type_assertions {
                    let applies_positively = match assertion.kind {
                        AssertionKind::IfTrue => function_returned_true,
                        AssertionKind::IfFalse => !function_returned_true,
                        AssertionKind::Always => continue,
                    };
                    let should_exclude = assertion.negated ^ !applies_positively;
                    // Resolve `self`/`static`/`$this` in the asserted type
                    // against the *declaring* class (e.g. `Decimal`), not the
                    // enclosing class (e.g. `Monetary`).  Without this,
                    // `@phpstan-assert-if-false self<true> $this` on
                    // `Decimal::isZero()` would narrow $denominator to
                    // `Monetary` instead of `Decimal`.
                    let resolved_type = if assertion.asserted_type.contains_self_ref() {
                        assertion.asserted_type.replace_self(&receiver.fqn())
                    } else {
                        assertion.asserted_type.clone()
                    };
                    if assertion.param_name == "$this" {
                        // Narrows the receiver variable itself.
                        to_apply.push((resolved_type, should_exclude, receiver_var.clone()));
                    } else if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                        &method_call.argument_list,
                        &assertion.param_name,
                        &method.parameters,
                    ) {
                        to_apply.push((resolved_type, should_exclude, arg_var));
                    }
                }
            }
            for (asserted_type, should_exclude, target_var) in to_apply {
                let var_ctx = build_var_ctx(&target_var, ctx, &scope_resolver);
                let mut results = scope.get(&target_var).to_vec();
                if should_exclude {
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_exclusion(&asserted_type, &var_ctx, classes)
                    });
                } else {
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_inclusion(
                            &asserted_type,
                            false,
                            &var_ctx,
                            classes,
                        )
                    });
                }
                if !results.is_empty() {
                    scope.set(&target_var, results);
                }
            }
        }
        _ => {}
    }
}

/// Build a [`VarResolutionCtx`] from a variable name and forward-walk context.
///
/// Shared helper used by the narrowing functions in this module to avoid
/// repeating the struct construction at every call site.
pub(crate) fn build_var_ctx<'a>(
    var_name: &'a str,
    ctx: &'a ForwardWalkCtx<'_>,
    scope_resolver: &'a dyn Fn(&str) -> Vec<ResolvedType>,
) -> VarResolutionCtx<'a> {
    VarResolutionCtx {
        var_name,
        current_class: ctx.current_class,
        all_classes: ctx.all_classes,
        content: ctx.content,
        cursor_offset: ctx.cursor_offset,
        class_loader: ctx.class_loader,
        backend: ctx.backend,
        loaders: ctx.loaders,
        resolved_class_cache: ctx.resolved_class_cache,
        enclosing_return_type: ctx.enclosing_return_type.clone(),
        top_level_scope: ctx.top_level_scope.clone(),
        branch_aware: false,
        match_arm_narrowing: HashMap::new(),
        scope_var_resolver: Some(scope_resolver),
    }
}

/// Apply type-guard narrowing in the truthy branch.
///
/// When `is_object($var)` (or `is_array`, `is_string`, etc.) appears
/// in a condition, narrow the variable's type.  For `mixed` variables,
/// this replaces `mixed` with the guard's canonical type (e.g. `object`).
/// For union types, it filters to only the members that match the guard.
///
/// Handles compound `&&` conditions by decomposing them into individual
/// operands and applying each type guard found.  For example,
/// `is_object($data) && property_exists($data, 'error_link')` applies
/// the `is_object` guard to `$data`.
pub(crate) fn apply_type_guard_narrowing_truthy(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
) {
    apply_type_guard_on_operands(condition, scope, true);
}

/// Apply type-guard narrowing in the inverse (else) branch.
///
/// When `is_object($var)` appears in a condition, the else branch
/// knows the variable is NOT an object — filter out object-like
/// members from the union type.
pub(crate) fn apply_type_guard_narrowing_inverse(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
) {
    apply_type_guard_on_operands(condition, scope, false);
}

/// Shared implementation for truthy and inverse type-guard narrowing.
///
/// Decomposes `&&` chains into individual operands and applies each
/// type guard found.  When `truthy` is `true`, applies inclusion
/// narrowing (then-body); when `false`, applies exclusion (else-body).
pub(crate) fn apply_type_guard_on_operands(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
    truthy: bool,
) {
    // Decompose `&&` chains so that `is_object($x) && is_string($y)`
    // applies both guards.
    let operands = collect_and_chain_operands(condition);
    let mut var_names: Vec<String> = scope.locals.keys().map(|k| k.to_string()).collect();
    // Include property access keys from conditions (e.g. `$a->foo`
    // from `is_string($a->foo)`) so they can be narrowed.
    for key in collect_condition_property_keys(condition) {
        if !var_names.contains(&key) {
            var_names.push(key);
        }
    }
    // Include plain variables the condition names but the scope has no
    // type for — a guard on a value read from an unknown source (a
    // `stdClass` property, an untyped array offset) is the only thing
    // that says what it is, so it must not be skipped for want of a
    // prior type.
    for name in collect_condition_var_names(condition) {
        if !var_names.contains(&name) {
            var_names.push(name);
        }
    }
    for operand in &operands {
        for var_name in &var_names {
            if let Some((kind, negated)) = narrowing::try_extract_type_guard(operand, var_name) {
                // When the guard is negated (e.g. `!is_object($x)`),
                // flip the inclusion/exclusion logic: the truthy branch
                // of a negated guard means the variable is NOT the
                // guarded type, and vice versa.
                let effective_truthy = if negated { !truthy } else { truthy };
                let mut results = scope.get(var_name).to_vec();
                if results.is_empty() {
                    // Nothing known about the subject.  A guard that
                    // holds still proves its type outright; one that
                    // fails only rules a type out, which says nothing
                    // on its own.
                    if effective_truthy {
                        scope.set(
                            var_name,
                            vec![ResolvedType::from_type_string(
                                narrowing::guard_kind_to_narrowed_type(kind),
                            )],
                        );
                    }
                    continue;
                }
                if effective_truthy {
                    narrowing::apply_type_guard_inclusion(kind, &mut results);
                } else {
                    narrowing::apply_type_guard_exclusion(kind, &mut results);
                }
                if !results.is_empty() {
                    scope.set(var_name, results);
                }
            }
        }
    }
}

/// Narrow a union of object types by a check on a property that only some
/// of its members could have passed.
///
/// `is_string($b->v)` on a `StrBox|IntBox` subject proves the value is a
/// `StrBox` when `IntBox::$v` is declared `int`: no `IntBox` reaches the
/// then-body.  An identity check against a literal (`$b->v === 'x'`)
/// discriminates the same way.  A member is only ever dropped when its
/// own declaration rules the check out, so a property whose type is
/// unknown, wide, or shared across the union leaves the subject alone.
pub(crate) fn apply_property_discriminant_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    truthy: bool,
) {
    // `&&` proves each of its operands where the body runs.  Its inverse
    // proves none of them on its own (`!(A && B)` leaves both open), so
    // the else branch only reads a condition that stands alone.
    let operands = if truthy {
        collect_and_chain_operands(condition)
    } else {
        vec![condition]
    };
    for operand in operands {
        if let Some(check) = extract_property_check(operand, truthy) {
            narrow_union_by_property_check(&check, scope, ctx);
        }
    }
}

/// A check on `subject`'s `property` that a union member may be unable
/// to pass.
struct PropertyCheck {
    subject: String,
    property: String,
    test: PropertyTest,
}

enum PropertyTest {
    /// The property passed (or, when `expect_match` is false, failed) a
    /// type guard such as `is_string()`.
    Guard {
        kind: narrowing::TypeGuardKind,
        expect_match: bool,
    },
    /// The property is identical (or, when `expect_equal` is false, not
    /// identical) to an exact value.
    Value {
        value: ExactValue,
        value_type: PhpType,
        expect_equal: bool,
    },
}

/// A value a comparison can pin a property to exactly.  Floats are left
/// out: `===` on them is a trap, and they are never written as a
/// discriminant.
#[derive(Debug, PartialEq)]
enum ExactValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Null,
}

impl PropertyTest {
    /// Report whether a member declaring `prop_type` for the property
    /// could have reached the branch this check guards.
    fn admits(&self, prop_type: &PhpType) -> bool {
        match self {
            PropertyTest::Guard { kind, expect_match } => {
                narrowing::guard_outcome_possible(prop_type, *kind, *expect_match)
            }
            PropertyTest::Value {
                value,
                value_type,
                expect_equal: true,
            } => property_can_equal(prop_type, value, value_type),
            PropertyTest::Value {
                value,
                expect_equal: false,
                ..
            } => !exact_value_of_type(prop_type).is_some_and(|own| own == *value),
        }
    }
}

/// Read the check a single condition operand makes about one property.
fn extract_property_check(operand: &Expression<'_>, truthy: bool) -> Option<PropertyCheck> {
    let (inner, negated) = narrowing::unwrap_condition_negation(operand);
    // Whether the branch being narrowed is the one where the check held.
    let holds = truthy != negated;

    match inner {
        // `is_string($b->v)` and the other type-guard functions.
        Expression::Call(Call::Function(_)) => {
            let key = collect_condition_property_keys(inner)
                .into_iter()
                .find(|k| k.contains("->"))?;
            let (kind, guard_negated) = narrowing::try_extract_type_guard(inner, &key)?;
            let (subject, property) = split_property_key(&key)?;
            Some(PropertyCheck {
                subject,
                property,
                test: PropertyTest::Guard {
                    kind,
                    expect_match: holds != guard_negated,
                },
            })
        }
        // `$b->v === 'x'` / `$b->v !== 'x'`.
        Expression::Binary(bin) => {
            let identical = match bin.operator {
                BinaryOperator::Identical(_) => true,
                BinaryOperator::NotIdentical(_) => false,
                _ => return None,
            };
            let (key, other) = property_key_operand(bin.lhs, bin.rhs)?;
            let (value, value_type) = exact_value_of_expr(other)?;
            let (subject, property) = split_property_key(&key)?;
            Some(PropertyCheck {
                subject,
                property,
                test: PropertyTest::Value {
                    value,
                    value_type,
                    expect_equal: holds == identical,
                },
            })
        }
        _ => None,
    }
}

/// Pick whichever side of a comparison is a property path, paired with
/// the other side.
fn property_key_operand<'b>(
    lhs: &'b Expression<'b>,
    rhs: &'b Expression<'b>,
) -> Option<(String, &'b Expression<'b>)> {
    for (candidate, other) in [(lhs, rhs), (rhs, lhs)] {
        if let Some(key) = narrowing::expr_to_subject_key(candidate)
            && key.contains("->")
        {
            return Some((key, other));
        }
    }
    None
}

/// Split `$b->v` into its subject (`$b`) and property name (`v`).
/// A call key (`$b->v()`) is not a property and is left out.
fn split_property_key(key: &str) -> Option<(String, String)> {
    let arrow = key.rfind("->")?;
    let property = &key[arrow + 2..];
    if property.is_empty() || !property.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some((key[..arrow].to_string(), property.to_string()))
}

/// Drop the members of the subject's union whose declaration of the
/// property rules the check out.
fn narrow_union_by_property_check(
    check: &PropertyCheck,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let entries = scope.get(&check.subject);
    // Nothing to discriminate between below two class-bearing members.
    if entries.iter().filter(|rt| rt.class_info.is_some()).count() < 2 {
        return;
    }

    let mut kept: Vec<ResolvedType> = Vec::with_capacity(entries.len());
    let mut dropped = false;
    for rt in entries {
        // Entries that name no class (a `null` alternative, a scalar the
        // subject may also hold) carry no property to read, so the check
        // says nothing about them.
        let admitted = match rt.class_info.as_ref() {
            Some(cls) => crate::inheritance::resolve_property_type_hint(
                cls,
                &check.property,
                ctx.class_loader,
            )
            .is_none_or(|hint| check.test.admits(&hint)),
            None => true,
        };
        if admitted {
            kept.push(rt.clone());
        } else {
            dropped = true;
        }
    }

    // Keeping nothing would mean the check can never pass — a claim the
    // subject's declared type is more likely wrong about than the code is.
    if dropped && kept.iter().any(|rt| rt.class_info.is_some()) {
        scope.set(&check.subject, kept);
    }
}

/// Report whether a property declared `prop_type` could be identical to
/// the compared value.
fn property_can_equal(prop_type: &PhpType, value: &ExactValue, value_type: &PhpType) -> bool {
    match prop_type.kind() {
        TypeKind::Union(members) => members
            .iter()
            .any(|m| property_can_equal(m, value, value_type)),
        TypeKind::Nullable(inner) => {
            *value == ExactValue::Null || property_can_equal(inner, value, value_type)
        }
        _ => match exact_value_of_type(prop_type) {
            Some(own) => own == *value,
            // Only a scalar declaration is precise enough to rule a value
            // out.  A class, a template parameter, or anything else the
            // subtype check cannot speak for keeps the member.
            None if is_scalar_declaration(prop_type) => value_type.is_subtype_of(prop_type),
            None => true,
        },
    }
}

/// Report whether a type pins its values to one scalar family, so that a
/// value outside it cannot be identical to anything the type holds.
fn is_scalar_declaration(ty: &PhpType) -> bool {
    ty.is_null()
        || ty.is_subtype_of(&PhpType::string())
        || ty.is_subtype_of(&PhpType::int())
        || ty.is_subtype_of(&PhpType::float())
        || ty.is_subtype_of(&PhpType::bool())
}

/// Read the single value a type is pinned to, when it has one.
fn exact_value_of_type(ty: &PhpType) -> Option<ExactValue> {
    if let Some(literal) = ty.as_literal() {
        return match literal {
            crate::php_type::LiteralValue::String(_) => literal
                .string_content()
                .map(|c| ExactValue::Str(c.into_owned())),
            crate::php_type::LiteralValue::Int(_) => literal.parse_i64().map(ExactValue::Int),
            crate::php_type::LiteralValue::Float(_) => None,
        };
    }
    match ty.kind() {
        TypeKind::Named(name) => match name.to_ascii_lowercase().as_str() {
            "true" => Some(ExactValue::Bool(true)),
            "false" => Some(ExactValue::Bool(false)),
            "null" => Some(ExactValue::Null),
            _ => None,
        },
        _ => None,
    }
}

/// Read the value a literal operand compares against, with the type that
/// value has.
fn exact_value_of_expr(expr: &Expression<'_>) -> Option<(ExactValue, PhpType)> {
    match expr {
        Expression::Parenthesized(paren) => exact_value_of_expr(paren.expression),
        Expression::Literal(Literal::String(string)) => {
            let raw = bytes_to_str(string.raw);
            let ty = PhpType::literal_string_raw(raw.to_string());
            let value = ty.as_literal()?.string_content()?.into_owned();
            Some((ExactValue::Str(value), ty))
        }
        Expression::Literal(Literal::Integer(integer)) => {
            let value = i64::try_from(integer.value?).ok()?;
            Some((
                ExactValue::Int(value),
                PhpType::literal_int(value.to_string()),
            ))
        }
        Expression::Literal(Literal::True(_)) => Some((ExactValue::Bool(true), PhpType::bool())),
        Expression::Literal(Literal::False(_)) => Some((ExactValue::Bool(false), PhpType::bool())),
        _ if is_null_expr(expr) => Some((ExactValue::Null, PhpType::null())),
        _ => None,
    }
}

/// Apply `is_a($x, Class::class, true)` / `class_exists($x)` (and the
/// other `*_exists()` forms) class-string narrowing.
///
/// When the guard's effective truth value is `true`, narrows string-like
/// (and `mixed`) entries in `$x`'s type to `class-string<Class>` (or
/// bare `class-string` for the generic `*_exists()` forms, which don't
/// name a specific class).  Negation is resolved by
/// `try_extract_class_string_guard`, so passing `truthy = false` here
/// from a guard-clause inverse correctly re-derives the truthy narrowing
/// for a negated condition (`if (!is_a(...)) { throw; }`).
///
/// Object-typed entries (with `class_info` set) are left untouched —
/// `is_a()`'s object side is already narrowed by the existing
/// instanceof-style handling, which operates independently on the
/// class-bearing entries.
pub(crate) fn apply_class_string_guard_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    truthy: bool,
) {
    let operands = collect_and_chain_operands(condition);
    let mut var_names: Vec<String> = scope.locals.keys().map(|k| k.to_string()).collect();
    for key in collect_condition_property_keys(condition) {
        if !var_names.contains(&key) {
            var_names.push(key);
        }
    }
    for operand in &operands {
        for var_name in &var_names {
            if let Some((target, negated)) =
                narrowing::try_extract_class_string_guard(operand, var_name)
            {
                let effective_truthy = if negated { !truthy } else { truthy };
                if !effective_truthy {
                    continue;
                }
                // Seed compound subject keys (`$arr['class']`, `$obj->prop`)
                // so a class-string guard on an array-index or property
                // subject narrows just like one on a plain variable.  An
                // untyped array index seeds as `mixed`, which the loop below
                // narrows to `class-string<Class>`.
                seed_synthetic_key_if_needed(var_name, scope, ctx);
                let mut results = scope.get(var_name).to_vec();
                if results.is_empty() {
                    continue;
                }
                let resolved_fqn = target
                    .as_deref()
                    .map(|name| crate::util::resolve_name_via_loader(name, ctx.class_loader));
                let class_string_type = match &resolved_fqn {
                    Some(fqn) => PhpType::parse(&format!("class-string<{}>", fqn)),
                    None => PhpType::parse("class-string"),
                };
                let mut changed = false;
                for rt in results.iter_mut() {
                    if rt.class_info.is_some() {
                        continue;
                    }
                    // Never widen a type that is already at least as
                    // specific as the guard's result. The generic
                    // `*_exists()` forms narrow to bare `class-string`; a
                    // variable already typed `class-string<Foo>` must keep
                    // its type argument rather than be downgraded (a bare
                    // `class-string` is a supertype, so `new $var` could no
                    // longer recover the concrete class).
                    if rt.type_string.is_subtype_of(&class_string_type) {
                        continue;
                    }
                    if rt.type_string.is_subtype_of(&PhpType::string()) || rt.type_string.is_mixed()
                    {
                        rt.type_string = class_string_type.clone();
                        changed = true;
                    }
                }
                if changed {
                    scope.set(var_name, results);
                }
            }
        }
    }
}

/// Apply null narrowing for the truthy branch.
///
/// Handles `$x !== null`, `$x != null`, `isset($x)`, `!empty($x)`,
/// `!is_null($x)`, and truthiness checks.
pub(crate) fn apply_null_narrowing_truthy<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Decompose `&&` chains so that `isset($a) && isset($b)` narrows
    // both variables, and `$x !== null && $y !== null` works too.
    let operands = collect_and_chain_operands(condition);
    if operands.len() > 1 {
        for operand in &operands {
            apply_null_narrowing_truthy(operand, scope, ctx);
        }
        return;
    }

    // Check for `$x !== null` or `$x != null` or `null !== $x` etc.
    if let Some(var_name) = extract_non_null_check_var(condition) {
        // For array access keys, narrow the shape on the base variable.
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_element(&var_name, base, key, scope, ctx);
        } else {
            seed_synthetic_key_if_needed(&var_name, scope, ctx);
            strip_null_from_scope(&var_name, scope);
        }
    }
    // Check for `$x !== false` or `false !== $x` — the truthy branch
    // rules out `false` alone, which is what the `T|false` handle idiom
    // (`fopen()`, `finfo_open()`, `strpos()`, …) is written to do.
    if let Some(var_name) = extract_non_false_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_false_from_scope(&var_name, scope);
    }
    // `isset($x)` — truthy branch means $x is not null: strip null.
    // Handles multiple args: `isset($a, $b)` strips null from both.
    for var_name in extract_isset_vars(condition) {
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_element(&var_name, base, key, scope, ctx);
        } else {
            seed_synthetic_key_if_needed(&var_name, scope, ctx);
            strip_null_from_scope(&var_name, scope);
        }
    }
    // `!isset($x)` — truthy branch means $x is null: narrow to null.
    for var_name in extract_not_isset_vars(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        narrow_to_null_in_scope(&var_name, scope);
    }
    // Check for `$x === null` or `$x == null` — narrow to null only.
    if let Some(var_name) = extract_null_equality_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        narrow_to_null_in_scope(&var_name, scope);
    }
    // `$x !== ''` / `$x !== []` — refine to the non-empty counterpart.
    // `$x === ''` proves the opposite and is handled by the inverse pass.
    if let Some((var_name, empty, non_empty)) = extract_empty_value_check(condition)
        && non_empty
    {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        refine_non_empty_in_scope(&var_name, empty, scope);
    }
    // `!empty($x)` — truthy branch means $x is non-empty (truthy):
    // strip null and false from the type.
    if let Some(var_name) = extract_not_empty_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_falsy_from_scope(&var_name, scope);
    }
    // Bare truthy check: `if ($x) { ... }` — $x is truthy in the
    // then-body, so strip null and false from its type.
    if let Some(var_name) =
        expr_to_var_name(condition).or_else(|| narrowing::expr_to_subject_key(condition))
    {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_falsy_from_scope(&var_name, scope);
    }
}

/// Apply inverse null narrowing (for guard clause: `if ($x === null) { return; }`).
pub(crate) fn apply_null_narrowing_inverse<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Decompose `||` chains: `if (A || B) { return; }` only falls
    // through to the rest of the function when every operand is false,
    // so each operand's own inverse narrowing holds on its own — the
    // same De Morgan reasoning `apply_condition_narrowing_inverse` uses
    // for instanceof checks, applied here to null/false checks.
    let or_operands = collect_or_chain_operands(condition);
    if or_operands.len() > 1 {
        for operand in &or_operands {
            apply_null_narrowing_inverse(operand, scope, ctx);
        }
        return;
    }

    // When the condition is `$x === null` (equality check for null),
    // the inverse (else/guard) means $x is NOT null.
    if let Some(var_name) = extract_null_equality_check_var(condition) {
        // For array access keys like `$a["test"]`, narrow the array
        // shape on the base variable directly rather than using a
        // synthetic scope entry.  This ensures the narrowed shape
        // survives scope merges.
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_element(&var_name, base, key, scope, ctx);
        } else {
            seed_synthetic_key_if_needed(&var_name, scope, ctx);
            strip_null_from_scope(&var_name, scope);
        }
    }
    // When the condition is `$x !== null`, the inverse (else/guard)
    // means $x IS null — narrow to null only.
    if let Some(var_name) = extract_non_null_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        narrow_to_null_in_scope(&var_name, scope);
    }
    // When the condition is `!$x` or `empty($x)`, the inverse means
    // $x is truthy — remove null.
    if let Some(var_name) = extract_falsy_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_null_from_scope(&var_name, scope);
    }
    // When the condition is `$x === false`, the inverse (else/guard)
    // means $x is NOT false — strip false only, mirroring the null
    // equality case above.
    if let Some(var_name) = extract_false_equality_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_false_from_scope(&var_name, scope);
    }
    // When the condition is `$x !== false`, the inverse (else/guard)
    // means $x IS false — narrow to false only.
    if let Some(var_name) = extract_non_false_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        narrow_to_false_in_scope(&var_name, scope);
    }
    // When the condition is `$x === ''` / `$x === []`, the inverse
    // (else/guard) means $x is non-empty.
    if let Some((var_name, empty, non_empty)) = extract_empty_value_check(condition)
        && !non_empty
    {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        refine_non_empty_in_scope(&var_name, empty, scope);
    }
    // When the condition is a bare `$x` (truthy check), the inverse means
    // $x is falsy.  For nullable types (`T|null`), narrow to null.
    // This handles `while ($a) { ... }` => after loop, $a is null.
    if let Some(var_name) = expr_to_var_name(condition) {
        narrow_to_null_in_scope(&var_name, scope);
    }
    // `isset($x)` — inverse (else) means $x was null: narrow to null.
    for var_name in extract_isset_vars(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        narrow_to_null_in_scope(&var_name, scope);
    }
    // `!isset($x)` — inverse (guard after `!isset` return) means $x
    // is not null: strip null.
    for var_name in extract_not_isset_vars(condition) {
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_element(&var_name, base, key, scope, ctx);
        } else {
            seed_synthetic_key_if_needed(&var_name, scope, ctx);
            strip_null_from_scope(&var_name, scope);
        }
    }
}

/// Narrow the receivers of every nullsafe chain the condition proves is
/// not `null`.
///
/// `if ($image?->file_id !== null)` can only be entered when `$image`
/// itself is not null: had it been, the chain would have short-circuited
/// to `null` and the comparison would have failed.  A truthy test on the
/// chain and an identity check against a non-null value carry the same
/// proof, and a chain of several `?->` links proves it for each receiver
/// along the way.
///
/// `truthy` is the polarity the caller establishes: `true` for an `if`
/// body, `false` for an else branch or the fall-through of a guard clause
/// that leaves the scope.
pub(crate) fn apply_nullsafe_receiver_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    truthy: bool,
) {
    let mut proven: Vec<&Expression<'_>> = Vec::new();
    collect_proven_non_null_exprs(condition, truthy, &mut proven);
    for expr in proven {
        let mut node = expr;
        while let Some(receiver) = nullsafe_receiver(node) {
            if let Some(key) = narrowing::expr_to_subject_key(receiver) {
                seed_synthetic_key_if_needed(&key, scope, ctx);
                strip_null_from_scope(&key, scope);
            }
            node = receiver;
        }
    }
}

/// The receiver a nullsafe access short-circuits on, when `expr` is one.
fn nullsafe_receiver<'b>(expr: &'b Expression<'b>) -> Option<&'b Expression<'b>> {
    match expr {
        Expression::Parenthesized(inner) => nullsafe_receiver(inner.expression),
        Expression::Access(Access::NullSafeProperty(pa)) => Some(pa.object),
        Expression::Call(Call::NullSafeMethod(mc)) => Some(mc.object),
        _ => None,
    }
}

/// Collect the expressions `condition` proves are not `null` under the
/// given polarity.
///
/// Callers filter the result for the shapes they can act on, so a bare
/// truthy test contributes its whole subject rather than nothing.
fn collect_proven_non_null_exprs<'b>(
    condition: &'b Expression<'b>,
    truthy: bool,
    out: &mut Vec<&'b Expression<'b>>,
) {
    match condition {
        Expression::Parenthesized(inner) => {
            collect_proven_non_null_exprs(inner.expression, truthy, out);
        }
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            collect_proven_non_null_exprs(prefix.operand, !truthy, out);
        }
        Expression::Binary(bin) => {
            // `A && B` proves both when true; `A || B` proves neither
            // operand held when false.  Either way each operand carries
            // the parent's polarity.
            let decomposes = match bin.operator {
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_) => truthy,
                BinaryOperator::Or(_) | BinaryOperator::LowOr(_) => !truthy,
                _ => false,
            };
            if decomposes {
                collect_proven_non_null_exprs(bin.lhs, truthy, out);
                collect_proven_non_null_exprs(bin.rhs, truthy, out);
                return;
            }

            let inequality = matches!(
                bin.operator,
                BinaryOperator::NotIdentical(_) | BinaryOperator::NotEqual(_)
            );
            let equality = matches!(
                bin.operator,
                BinaryOperator::Identical(_) | BinaryOperator::Equal(_)
            );
            if !inequality && !equality {
                return;
            }

            // `$x !== null` proves non-null when true, `$x === null` when
            // false.
            let proves_non_null = if inequality { truthy } else { !truthy };
            if is_null_expr(bin.rhs) {
                if proves_non_null {
                    out.push(bin.lhs);
                }
                return;
            }
            if is_null_expr(bin.lhs) {
                if proves_non_null {
                    out.push(bin.rhs);
                }
                return;
            }

            // A match against a value that is not null proves the other
            // side is not null either.  Only identity qualifies: `null ==
            // false` and `null == 0` are both true, so a loose comparison
            // against a falsy value proves nothing.
            if matches!(bin.operator, BinaryOperator::Identical(_)) && truthy {
                if exact_value_of_expr(bin.rhs).is_some_and(|(v, _)| v != ExactValue::Null) {
                    out.push(bin.lhs);
                }
                if exact_value_of_expr(bin.lhs).is_some_and(|(v, _)| v != ExactValue::Null) {
                    out.push(bin.rhs);
                }
            }
        }
        // A bare truthy test: anything truthy is non-null.
        _ if truthy => out.push(condition),
        _ => {}
    }
}

/// Extract variable name from `$x !== null` or `null !== $x` patterns.
pub(crate) fn extract_non_null_check_var(expr: &Expression<'_>) -> Option<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    match inner {
        Expression::Binary(bin) => {
            let is_not_identical = matches!(bin.operator, BinaryOperator::NotIdentical(_));
            let is_not_equal = matches!(bin.operator, BinaryOperator::NotEqual(_));
            let is_identical = matches!(bin.operator, BinaryOperator::Identical(_));
            let is_equal = matches!(bin.operator, BinaryOperator::Equal(_));

            // `$x !== null` or `null !== $x`
            if (is_not_identical || is_not_equal) && !negated
                || (is_identical || is_equal) && negated
            {
                if is_null_expr(bin.rhs) {
                    return expr_to_var_name(bin.lhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.lhs));
                }
                if is_null_expr(bin.lhs) {
                    return expr_to_var_name(bin.rhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.rhs));
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract all variable names from an `isset(…)` call (non-negated).
/// Handles simple variables (`$x`) and property/array access keys
/// (`$obj->prop`, `$arr["key"]`).  Returns an empty vec when the
/// expression is not an `isset()` call, or when it is negated.
pub(crate) fn extract_isset_vars(expr: &Expression<'_>) -> Vec<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    if negated {
        return vec![];
    }
    // `isset()` is a language construct, parsed as Expression::Construct(Construct::Isset).
    let Expression::Construct(Construct::Isset(isset)) = inner else {
        return vec![];
    };
    let mut vars = Vec::new();
    for value in isset.values.iter() {
        if let Some(name) =
            expr_to_var_name(value).or_else(|| narrowing::expr_to_subject_key(value))
        {
            vars.push(name);
        }
    }
    vars
}

/// Extract all variable names from a `!isset(…)` call (negated isset).
/// Returns an empty vec when the expression is not a negated `isset()`.
pub(crate) fn extract_not_isset_vars(expr: &Expression<'_>) -> Vec<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    if !negated {
        return vec![];
    }
    // `isset()` is a language construct, parsed as Expression::Construct(Construct::Isset).
    let Expression::Construct(Construct::Isset(isset)) = inner else {
        return vec![];
    };
    let mut vars = Vec::new();
    for value in isset.values.iter() {
        if let Some(name) =
            expr_to_var_name(value).or_else(|| narrowing::expr_to_subject_key(value))
        {
            vars.push(name);
        }
    }
    vars
}

/// Extract variable name from `$x === null` or `null === $x` patterns.
pub(crate) fn extract_null_equality_check_var(expr: &Expression<'_>) -> Option<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    match inner {
        Expression::Binary(bin) => {
            let is_identical = matches!(bin.operator, BinaryOperator::Identical(_));
            let is_equal = matches!(bin.operator, BinaryOperator::Equal(_));

            if (is_identical || is_equal) && !negated {
                if is_null_expr(bin.rhs) {
                    return expr_to_var_name(bin.lhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.lhs));
                }
                if is_null_expr(bin.lhs) {
                    return expr_to_var_name(bin.rhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.rhs));
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract variable name from `!empty($x)` (negated empty check).
pub(crate) fn extract_not_empty_var(expr: &Expression<'_>) -> Option<String> {
    if let Expression::UnaryPrefix(prefix) = expr
        && prefix.operator.is_not()
        && let Expression::Construct(Construct::Empty(empty)) = prefix.operand
    {
        return expr_to_var_name(empty.value);
    }
    None
}

/// Extract the subject of a falsy check: `!$x`, `empty($x)`.
///
/// A member path is as much a subject here as a bare variable is, so
/// `!$this->handle` names `$this->handle` — the guard-clause idiom
/// (`if (!$this->handle) { throw; }`) proves the same thing about a
/// property that it does about a local.
pub(crate) fn extract_falsy_check_var(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            expr_to_var_name(prefix.operand)
                .or_else(|| narrowing::expr_to_subject_key(prefix.operand))
        }
        // `empty($x)` — language construct, parsed as Expression::Construct(Construct::Empty).
        Expression::Construct(Construct::Empty(empty)) => {
            expr_to_var_name(empty.value).or_else(|| narrowing::expr_to_subject_key(empty.value))
        }
        _ => None,
    }
}

/// Extract variable name from `$x === false` or `false === $x` patterns.
///
/// Mirrors [`extract_null_equality_check_var`] but for `false` — needed
/// for the common "resource-like handle" idiom (`finfo_open()`,
/// `pg_connect()`, …) that returns `T|false` and is guarded with a
/// strict equality check rather than `!$x`/`empty($x)`.
pub(crate) fn extract_false_equality_check_var(expr: &Expression<'_>) -> Option<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    match inner {
        Expression::Binary(bin) => {
            let is_identical = matches!(bin.operator, BinaryOperator::Identical(_));
            let is_equal = matches!(bin.operator, BinaryOperator::Equal(_));

            if (is_identical || is_equal) && !negated {
                if is_false_expr(bin.rhs) {
                    return expr_to_var_name(bin.lhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.lhs));
                }
                if is_false_expr(bin.lhs) {
                    return expr_to_var_name(bin.rhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.rhs));
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract variable name from `$x !== false` or `false !== $x` patterns.
///
/// Mirrors [`extract_non_null_check_var`] but for `false`, which is what
/// the truthy branch of an `if`/`while` guarding a `T|false` return has
/// ruled out. The loose form (`$x != false`) rules out every falsy value,
/// so treating it as `false` alone is a subset of what it proves.
pub(crate) fn extract_non_false_check_var(expr: &Expression<'_>) -> Option<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    match inner {
        Expression::Binary(bin) => {
            let is_not_identical = matches!(bin.operator, BinaryOperator::NotIdentical(_));
            let is_not_equal = matches!(bin.operator, BinaryOperator::NotEqual(_));
            let is_identical = matches!(bin.operator, BinaryOperator::Identical(_));
            let is_equal = matches!(bin.operator, BinaryOperator::Equal(_));

            if (is_not_identical || is_not_equal) && !negated
                || (is_identical || is_equal) && negated
            {
                if is_false_expr(bin.rhs) {
                    return expr_to_var_name(bin.lhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.lhs));
                }
                if is_false_expr(bin.lhs) {
                    return expr_to_var_name(bin.rhs)
                        .or_else(|| narrowing::expr_to_subject_key(bin.rhs));
                }
            }
            None
        }
        _ => None,
    }
}

/// The empty value a condition compares a subject against.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmptyValue {
    String,
    Array,
}

/// Extract the subject of a strict comparison against an empty literal:
/// `$x !== ''`, `'' === $x`, `$x !== []`, and their negations.
///
/// Returns the subject key plus which empty value it was compared to, and
/// whether the comparison proves the subject is non-empty (`true`) or empty
/// (`false`).
///
/// Only the strict operators are recognised. `$x != ''` also rules out
/// `null`, and PHP 8 changed how `0 == ''` compares, so the loose form does
/// not map onto a single refinement.
pub(crate) fn extract_empty_value_check(
    expr: &Expression<'_>,
) -> Option<(String, EmptyValue, bool)> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    let Expression::Binary(bin) = inner else {
        return None;
    };
    let non_empty = match bin.operator {
        BinaryOperator::NotIdentical(_) => !negated,
        BinaryOperator::Identical(_) => negated,
        _ => return None,
    };
    let (subject, empty) = match (empty_literal_kind(bin.rhs), empty_literal_kind(bin.lhs)) {
        (Some(kind), _) => (bin.lhs, kind),
        (_, Some(kind)) => (bin.rhs, kind),
        _ => return None,
    };
    let name = expr_to_var_name(subject).or_else(|| narrowing::expr_to_subject_key(subject))?;
    Some((name, empty, non_empty))
}

/// Which empty literal an expression is, if any: `''`/`""` or `[]`/`array()`.
fn empty_literal_kind(expr: &Expression<'_>) -> Option<EmptyValue> {
    match expr {
        Expression::Parenthesized(paren) => empty_literal_kind(paren.expression),
        Expression::Literal(Literal::String(s)) => s
            .value
            .is_some_and(|value| value.is_empty())
            .then_some(EmptyValue::String),
        Expression::Array(array) => array.elements.is_empty().then_some(EmptyValue::Array),
        Expression::LegacyArray(array) => array.elements.is_empty().then_some(EmptyValue::Array),
        _ => None,
    }
}

/// Refine a variable's type to its non-empty counterpart.
///
/// `string` becomes `non-empty-string`, `array<K, V>` becomes
/// `non-empty-array<K, V>`, `list<T>` becomes `non-empty-list<T>`, and the
/// empty literal itself (`''`, `array{}`) drops out of a union. Members
/// outside the compared domain are left alone: `$x !== ''` on a
/// `string|array` says nothing about the array half.
pub(crate) fn refine_non_empty_in_scope(var_name: &str, empty: EmptyValue, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let refined: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| {
            rt.type_string = refine_non_empty_type(&rt.type_string, empty)?;
            Some(rt)
        })
        .collect();

    if !refined.is_empty() {
        scope.set(var_name, refined);
    }
}

/// Apply [`refine_non_empty_in_scope`]'s rule to one `PhpType`, returning
/// `None` when every member was the empty value being ruled out.
fn refine_non_empty_type(ty: &PhpType, empty: EmptyValue) -> Option<PhpType> {
    if let TypeKind::Union(members) = ty.kind() {
        let refined: Vec<PhpType> = members
            .iter()
            .filter_map(|member| refine_non_empty_type(member, empty))
            .collect();
        return match refined.len() {
            0 => None,
            1 => refined.into_iter().next(),
            _ => Some(PhpType::union(refined)),
        };
    }

    match empty {
        EmptyValue::String => {
            if ty
                .as_literal()
                .and_then(LiteralValue::string_content)
                .as_deref()
                == Some("")
            {
                return None;
            }
            match ty.kind() {
                TypeKind::Named(name) if name == "string" => {
                    Some(PhpType::named(atom("non-empty-string")))
                }
                _ => Some(ty.clone()),
            }
        }
        EmptyValue::Array => match ty.kind() {
            TypeKind::ArrayShape(entries) if entries.is_empty() => None,
            TypeKind::Named(name) if name == "array" => {
                Some(PhpType::named(atom("non-empty-array")))
            }
            TypeKind::Named(name) if name == "list" => Some(PhpType::named(atom("non-empty-list"))),
            TypeKind::Generic(generic) if generic.name == "array" => Some(PhpType::generic_atom(
                atom("non-empty-array"),
                generic.args.clone(),
            )),
            TypeKind::Generic(generic) if generic.name == "list" => Some(PhpType::generic_atom(
                atom("non-empty-list"),
                generic.args.clone(),
            )),
            _ => Some(ty.clone()),
        },
    }
}

/// Check if an expression is the `false` literal.
pub(crate) fn is_false_expr(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Literal(Literal::False(_)))
}

/// Check if an expression is `null`.
pub(crate) fn is_null_expr(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Literal(Literal::Null(_)) => true,
        Expression::ConstantAccess(ca) => {
            let name = ca.name.value();
            let clean = crate::util::strip_fqn_prefix(bytes_to_str(name));
            clean.eq_ignore_ascii_case("null")
        }
        _ => false,
    }
}

/// Extract a direct variable name from an expression.
///
/// An assignment stands for the variable it wrote, so the
/// assign-and-check idiom (`while (($line = fgets($h)) !== false)`,
/// `if ($row = next())`) resolves to `$line`/`$row` — the subject the
/// surrounding check narrows.  Parentheses are peeled on the way.
pub(crate) fn expr_to_var_name(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => Some(bytes_to_str(dv.name).to_string()),
        Expression::Parenthesized(paren) => expr_to_var_name(paren.expression),
        Expression::Assignment(assignment) if assignment.operator.is_assign() => {
            expr_to_var_name(assignment.lhs)
        }
        _ => None,
    }
}

/// Strip `null` from a variable's type in the scope.
/// Narrow a variable in scope to `null` only.
///
/// Used when a condition like `$x === null` is true: the variable must
/// be null.  Replaces the variable's type with `null` if it currently
/// contains a nullable type, or sets it to `null` if the variable has
/// any type at all.
pub(crate) fn narrow_to_null_in_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }
    // Check whether any existing type contains null (Nullable, Union
    // with null member, or bare null).  `non_null_type()` returns
    // `Some` for `?T` and `T|null` unions; `is_null()` catches bare
    // `null`.
    let has_null = types
        .iter()
        .any(|rt| rt.type_string.non_null_type().is_some() || rt.type_string.is_null());
    if has_null {
        scope.set(
            var_name,
            vec![ResolvedType::from_type_string(PhpType::null())],
        );
    }
}

/// Narrow a variable in scope to `false` only.
///
/// Mirrors [`narrow_to_null_in_scope`] but for `false`: used when a
/// condition like `$x !== false` is known to be false, so the variable
/// must be `false`.
pub(crate) fn narrow_to_false_in_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }
    let is_false = |t: &PhpType| matches!(t.kind(), TypeKind::Named(n) if n == "false");
    let has_false = types.iter().any(|rt| match rt.type_string.kind() {
        TypeKind::Union(members) => members.iter().any(is_false),
        _ => is_false(&rt.type_string),
    });
    if has_false {
        scope.set(
            var_name,
            vec![ResolvedType::from_type_string(PhpType::false_())],
        );
    }
}

pub(crate) fn strip_null_from_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let stripped: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| match rt.type_string.non_null_type() {
            Some(non_null) => {
                rt.type_string = non_null;
                Some(rt)
            }
            None if rt.type_string == PhpType::null() => None,
            None => Some(rt),
        })
        .collect();

    if !stripped.is_empty() {
        scope.set(var_name, stripped);
    }
}

/// Strip both `null` and `false` from a variable's type in the scope.
///
/// Used after falsy guard clauses (`if (!$var) { throw; }`) where the
/// variable is known to be truthy (non-null and non-false) after the guard.
pub(crate) fn strip_falsy_from_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let stripped: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| {
            rt.type_string = rt.type_string.truthy_type()?;
            Some(rt)
        })
        .collect();

    if !stripped.is_empty() {
        scope.set(var_name, stripped);
    }
}

/// Strip `false` (but not `null`) from a variable's type in the scope.
///
/// Used after a strict-equality guard clause (`if ($var === false) {
/// throw; }`) where only `false` was ruled out — unlike
/// [`strip_falsy_from_scope`], which also strips `null` for the broader
/// `!$var`/`empty($var)` idiom that guards against both.
pub(crate) fn strip_false_from_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let is_false = |t: &PhpType| matches!(t.kind(), TypeKind::Named(n) if n == "false");

    let stripped: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| {
            let ty = &rt.type_string;
            if is_false(ty) {
                return None;
            }
            if let TypeKind::Union(members) = ty.kind() {
                let non_false: Vec<PhpType> =
                    members.iter().filter(|m| !is_false(m)).cloned().collect();
                rt.type_string = match non_false.len() {
                    0 => return None,
                    1 => non_false.into_iter().next().unwrap(),
                    _ => PhpType::union(non_false),
                };
            }
            Some(rt)
        })
        .collect();

    if !stripped.is_empty() {
        scope.set(var_name, stripped);
    }
}

/// Split a single-level array access key like `$a["test"]` into base
/// variable and key name.  Returns `None` for non-array-access keys and
/// for multi-level access (`$a["x"]["y"]`), which this single-key
/// narrowing cannot represent and would otherwise mis-split.
pub(crate) fn split_array_access_key(key: &str) -> Option<(&str, &str)> {
    let bracket_pos = key.find("[\"")?;
    let base = &key[..bracket_pos];
    // The base must be a plain expression with no earlier array access.
    if base.contains('[') {
        return None;
    }
    let key_name = key[bracket_pos + 2..].strip_suffix("\"]")?;
    // A nested access leaves bracket characters inside the extracted key
    // (e.g. `x"]["y`); reject it rather than narrowing a bogus key.
    if key_name.contains('[') || key_name.contains(']') {
        return None;
    }
    Some((base, key_name))
}

/// Strip `null` from a specific array shape key on a variable.
///
/// Given variable `$a` typed as `array{test: ?int}` and key `"test"`,
/// rewrites the variable's type to `array{test: int}`.  This modifies
/// the base variable's type directly so the narrowed shape survives
/// scope merges (unlike synthetic scope entries which are stripped).
/// Remove `null` from an array element a check proved non-null.
///
/// A constant shape records each element's type inline, so the refinement
/// belongs on the base variable, where it survives scope merges.  A generic
/// `array<K, V|null>` has no per-key slot to refine — narrowing its value
/// type would wrongly claim every other key is non-null too — so the proof
/// is recorded on the synthetic `$a["k"]` scope key that offset reads
/// consult.
fn strip_null_from_array_element(
    access_key: &str,
    base_var: &str,
    key_name: &str,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    strip_null_from_array_shape_key(base_var, key_name, scope);
    seed_synthetic_key_if_needed(access_key, scope, ctx);
    strip_null_from_scope(access_key, scope);
}

pub(crate) fn strip_null_from_array_shape_key(
    base_var: &str,
    key_name: &str,
    scope: &mut ScopeState,
) {
    let types = scope.get(base_var).to_vec();
    if types.is_empty() {
        return;
    }
    let narrowed: Vec<ResolvedType> = types
        .into_iter()
        .map(|mut rt| {
            rt.type_string = strip_null_from_shape_key(&rt.type_string, key_name);
            rt
        })
        .collect();
    scope.set(base_var, narrowed);
}

/// Recursively strip `null` from a specific key in an array shape type.
pub(crate) fn strip_null_from_shape_key(
    ty: &crate::php_type::PhpType,
    key: &str,
) -> crate::php_type::PhpType {
    use crate::php_type::{PhpType, ShapeEntry, TypeKind};
    match ty.kind() {
        TypeKind::ArrayShape(entries) => {
            let new_entries: Vec<ShapeEntry> = entries
                .iter()
                .map(|e| {
                    if e.key.as_deref() == Some(key) {
                        let non_null = e
                            .value_type
                            .non_null_type()
                            .unwrap_or_else(|| e.value_type.clone());
                        ShapeEntry {
                            key: e.key.clone(),
                            value_type: non_null,
                            optional: false, // known to be present (was checked)
                        }
                    } else {
                        e.clone()
                    }
                })
                .collect();
            PhpType::array_shape(new_entries)
        }
        TypeKind::Nullable(inner) => {
            // `?array{test: ?int}` → `?array{test: int}`
            PhpType::nullable(strip_null_from_shape_key(inner, key))
        }
        TypeKind::Union(members) => {
            let new_members: Vec<PhpType> = members
                .iter()
                .map(|m| strip_null_from_shape_key(m, key))
                .collect();
            PhpType::union(new_members)
        }
        other => other.clone().into(),
    }
}

pub(crate) fn apply_guard_clause_null_narrowing<'b>(
    if_stmt: &'b If<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // When `if ($x === null) { return; }`, strip null from $x after.
    // When `if (!$x) { return; }`, strip null from $x after.
    if let Some(var_name) = extract_null_equality_check_var(if_stmt.condition) {
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_shape_key(base, key, scope);
        } else {
            seed_synthetic_key_if_needed(&var_name, scope, ctx);
            strip_null_from_scope(&var_name, scope);
        }
    }
    if let Some(var_name) = extract_falsy_check_var(if_stmt.condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_falsy_from_scope(&var_name, scope);
    }
    // When `if ($x === false) { throw; }`, strip only `false` from $x
    // after — the common "resource-like handle" idiom (`finfo_open()`,
    // `pg_connect()`, …) that returns `T|false`.
    if let Some(var_name) = extract_false_equality_check_var(if_stmt.condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_false_from_scope(&var_name, scope);
    }
    // `if (!isset($x)) { return; }` — after the guard, $x is not null.
    for var_name in extract_not_isset_vars(if_stmt.condition) {
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_shape_key(base, key, scope);
        } else {
            seed_synthetic_key_if_needed(&var_name, scope, ctx);
            strip_null_from_scope(&var_name, scope);
        }
    }
    // `if ($x !== null)` with return doesn't narrow after — the
    // remaining code is the null path.  This is handled by the
    // inverse narrowing in the guard clause logic.
}

/// Process assignment in a condition: `if ($x = expr())`
pub(crate) fn process_condition_assignment<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Direct assignment: `if ($x = expr())`
    if let Expression::Assignment(assignment) = condition
        && assignment.operator.is_assign()
        && let Expression::Variable(Variable::Direct(dv)) = assignment.lhs
    {
        let var_name = bytes_to_str(dv.name).to_string();
        let rhs_types = resolve_rhs_with_scope(assignment.rhs, scope, ctx);
        if !rhs_types.is_empty() {
            scope.set(&var_name, rhs_types);
        }
        return;
    }
    // Parenthesized conditions: `if (($x = expr()))`
    if let Expression::Parenthesized(inner) = condition {
        process_condition_assignment(inner.expression, scope, ctx);
        return;
    }
    // Negated (or otherwise unary-prefixed) conditions:
    //   `if (!$x = expr()) { return; }` — PHP parses this as
    //   `!($x = expr())`.  Recurse into the operand.
    if let Expression::UnaryPrefix(prefix) = condition {
        process_condition_assignment(prefix.operand, scope, ctx);
        return;
    }
    // Assignment inside a binary comparison or logical chain:
    //   `if (($x = expr()) !== null)`, `if (null !== ($x = expr()))`,
    //   `while (($x = next()) && $x->valid())`.  Recurse into both
    //   operands so the assignment on either side is seen.
    if let Expression::Binary(bin) = condition {
        process_condition_assignment(bin.lhs, scope, ctx);
        process_condition_assignment(bin.rhs, scope, ctx);
        return;
    }
    // Assignment wrapped in a call argument:
    //   `while (is_object($token = $tokenizer->next()))`.  Recurse
    //   into each argument value so the assignment is registered.
    if let Expression::Call(call) = condition {
        let arg_list = match call {
            Call::Function(fc) => &fc.argument_list,
            Call::Method(mc) => &mc.argument_list,
            Call::NullSafeMethod(mc) => &mc.argument_list,
            Call::StaticMethod(sc) => &sc.argument_list,
        };
        for arg in arg_list.arguments.iter() {
            let arg_expr = match arg {
                Argument::Positional(a) => a.value,
                Argument::Named(a) => a.value,
            };
            process_condition_assignment(arg_expr, scope, ctx);
        }
    }
}

/// Extract variable names referenced in instanceof / is_a / get_class
/// conditions.  This catches variables that are not yet in scope but
/// are used in guard clauses like `if (!$x instanceof Foo) { return; }`.
pub(crate) fn collect_condition_var_names(expr: &Expression<'_>) -> Vec<String> {
    let mut names = Vec::new();
    collect_condition_var_names_inner(expr, &mut names);
    names
}

/// Collect every variable a condition reads, in source order.
///
/// Unlike [`collect_condition_var_names`], which only picks out the subjects
/// of `instanceof`-shaped checks, this is the full set of candidates the
/// narrowing pipeline should consider — the equivalent of the `scope.locals`
/// key list `apply_condition_narrowing` walks when it has a live scope.
pub(crate) fn collect_condition_subject_vars(expr: &Expression<'_>, out: &mut Vec<String>) {
    let push = |name: String, out: &mut Vec<String>| {
        if !out.contains(&name) {
            out.push(name);
        }
    };
    match expr {
        Expression::Variable(Variable::Direct(dv)) => {
            push(bytes_to_str(dv.name).to_string(), out);
        }
        Expression::Parenthesized(inner) => collect_condition_subject_vars(inner.expression, out),
        Expression::UnaryPrefix(unary) => collect_condition_subject_vars(unary.operand, out),
        Expression::UnaryPostfix(unary) => collect_condition_subject_vars(unary.operand, out),
        Expression::Binary(bin) => {
            collect_condition_subject_vars(bin.lhs, out);
            collect_condition_subject_vars(bin.rhs, out);
        }
        Expression::Assignment(assignment) => {
            collect_condition_subject_vars(assignment.lhs, out);
            collect_condition_subject_vars(assignment.rhs, out);
        }
        Expression::Conditional(conditional) => {
            collect_condition_subject_vars(conditional.condition, out);
            if let Some(then) = conditional.then {
                collect_condition_subject_vars(then, out);
            }
            collect_condition_subject_vars(conditional.r#else, out);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => {
                    collect_condition_subject_vars(fc.function, out);
                    &fc.argument_list
                }
                Call::Method(mc) => {
                    collect_condition_subject_vars(mc.object, out);
                    &mc.argument_list
                }
                Call::NullSafeMethod(mc) => {
                    collect_condition_subject_vars(mc.object, out);
                    &mc.argument_list
                }
                Call::StaticMethod(sc) => &sc.argument_list,
            };
            for arg in args.arguments.iter() {
                collect_condition_subject_vars(arg.value(), out);
            }
        }
        Expression::Access(Access::Property(pa)) => collect_condition_subject_vars(pa.object, out),
        Expression::Access(Access::NullSafeProperty(pa)) => {
            collect_condition_subject_vars(pa.object, out);
        }
        Expression::ArrayAccess(aa) => {
            collect_condition_subject_vars(aa.array, out);
            collect_condition_subject_vars(aa.index, out);
        }
        Expression::Construct(Construct::Isset(isset)) => {
            for value in isset.values.iter() {
                collect_condition_subject_vars(value, out);
            }
        }
        Expression::Construct(Construct::Empty(empty)) => {
            collect_condition_subject_vars(empty.value, out);
        }
        _ => {}
    }
}

/// Whether a scope key is a synthetic property/array access key
/// (e.g. `$this->cache`, `$row["id"]`) rather than a plain variable.
pub(crate) fn is_synthetic_key(key: &str) -> bool {
    key.contains("->") || key.contains("[\"")
}

/// Remove synthetic property/array access keys from the scope.
/// Called after loop merges and other scope transitions where
/// condition-based narrowing no longer holds.
pub(crate) fn strip_synthetic_property_keys(scope: &mut ScopeState) {
    scope.locals.retain(|key, _| !is_synthetic_key(key));
    // A check on a property path is narrowing too, so a boolean that
    // stands for one is dropped alongside the key it describes.
    scope.assertions.retain(|_, checks| {
        checks.retain(|c| !is_synthetic_key(&c.subject));
        !checks.is_empty()
    });
}

/// Keep only the synthetic property/array access keys that *every*
/// surviving path out of a branching statement established a type for.
///
/// A key that only some paths carry is narrowing (or an assignment)
/// that holds inside one branch and says nothing about the others, so
/// the merged union would be an unsound claim about the program point
/// after the statement. A key every path carries is a genuine join:
/// each branch contributed its own truth, so the union is exactly the
/// type the property can have once the branches reconverge. That is
/// what makes the lazy-initialisation idiom resolve — the then-branch
/// assigns the concrete type and the implicit else path narrows to it
/// via the negated condition, so both agree.
pub(crate) fn retain_synthetic_keys_common_to_all(
    scope: &mut ScopeState,
    surviving: &[&ScopeState],
) {
    scope.locals.retain(|key, _| {
        !is_synthetic_key(key) || surviving.iter().all(|s| s.locals.contains_key(key))
    });
}

/// Seed a synthetic scope entry for a compound key (property access
/// or array access) if it isn't already present.  Simple variable
/// names (no `->` or `["`) are skipped since they are already tracked.
pub(crate) fn seed_synthetic_key_if_needed(
    key: &str,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Only seed compound keys (property access or array access).
    if !key.contains("->") && !key.contains("[\"") {
        return;
    }
    if scope.contains(key) {
        return;
    }

    let types = resolve_synthetic_key_type(key, scope, ctx);
    scope.set(key, types);
}

/// Resolve what a synthetic scope key promises, reading the scope but not
/// writing to it.
///
/// Dispatches on the key's *trailing* segment, because that is the access
/// that produces the key's type: `$a->items["0"]` is an array access whose
/// base is a property path, while `$a["0"]->items` is a property access
/// whose base is an array access.  Testing for `->` anywhere in the key
/// would route the former down the member path, which splits at the last
/// `->` and would look up a member literally named `items["0"]` — a name no
/// class declares, so a model with a magic `__get` answers it with `mixed`
/// and that bogus `mixed` becomes the authoritative type for the key.
fn resolve_synthetic_key_type(
    key: &str,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    if key.ends_with("\"]") {
        resolve_array_key_type(key, scope, ctx)
    } else if key.contains("->") {
        resolve_member_key_type(key, scope, ctx)
    } else {
        scope.get(key).to_vec()
    }
}

/// Resolve the element type an array-access key promises (`$a["k"]`,
/// `$a->items["0"]`, `$a["x"]["y"]`).
fn resolve_array_key_type(
    key: &str,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    // Split off the *last* bracket segment so a nested access resolves its
    // base (`$a["x"]` of `$a["x"]["y"]`) through the same dispatcher.
    let Some(bracket_pos) = key.rfind("[\"") else {
        return Vec::new();
    };
    let base_var = &key[..bracket_pos];
    let key_name = key[bracket_pos + 2..]
        .strip_suffix("\"]")
        .unwrap_or(&key[bracket_pos + 2..]);

    // Only the leading variable of a path is ever assigned in the scope, so
    // a compound base has to be resolved the same way this key is.  Each
    // step drops one segment, so the recursion is bounded by the number of
    // segments in the key.
    let resolved_base;
    let base_types: &[ResolvedType] = match scope.get(base_var) {
        [] if base_var.contains("->") || base_var.ends_with("\"]") => {
            resolved_base = resolve_synthetic_key_type(base_var, scope, ctx);
            &resolved_base
        }
        from_scope => from_scope,
    };
    if base_types.is_empty() {
        return Vec::new();
    }
    // Look up the array key's type.  Prefer a precise shape entry
    // (`array{class: Foo}`); fall back to the generic element type
    // (`array<string, Foo>` → `Foo`); and finally to `mixed` for an
    // untyped array (plain `array`).  Seeding the untyped case is
    // what lets assertion / class-string narrowing apply to an
    // array-index subject whose element type is otherwise unknown
    // (e.g. `assertInstanceOf(X::class, $arr['k'])`).
    let mut key_results: Vec<ResolvedType> = Vec::new();
    for rt in base_types {
        let element_type = rt
            .type_string
            .extract_shape_key_type(key_name)
            .or_else(|| rt.type_string.extract_value_type(false).cloned())
            .or_else(|| rt.type_string.is_array_like().then(PhpType::mixed));
        let Some(element_type) = element_type else {
            continue;
        };
        let resolved_classes = crate::type_engine::type_resolution::type_hint_to_classes_typed(
            &element_type,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if resolved_classes.is_empty() {
            ResolvedType::extend_unique(
                &mut key_results,
                vec![ResolvedType::from_type_string(element_type)],
            );
        } else {
            ResolvedType::extend_unique(
                &mut key_results,
                ResolvedType::from_classes_with_hint(resolved_classes, element_type),
            );
        }
    }
    key_results
}

/// Seed property/array-access subject keys that appear as arguments to a
/// call expression into the scope.
///
/// Used for assertion narrowing on non-variable subjects, e.g.
/// `assertInstanceOf(X::class, $view->component)` or a `@phpstan-assert`
/// helper invoked on `$arg->value`.  Each argument that resolves to a
/// compound subject key (property path or array access) is seeded with
/// its current type so the assertion narrowing loop can narrow it.
pub(crate) fn seed_assert_arg_subject_keys(
    expr: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    let Expression::Call(call) = expr else {
        return;
    };
    let argument_list = match call {
        Call::Function(fc) => &fc.argument_list,
        Call::Method(mc) => &mc.argument_list,
        Call::NullSafeMethod(mc) => &mc.argument_list,
        Call::StaticMethod(sc) => &sc.argument_list,
    };
    for arg in argument_list.arguments.iter() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        if let Some(key) = narrowing::expr_to_subject_key(arg_expr)
            && (key.contains("->") || key.contains("[\""))
        {
            seed_synthetic_key_if_needed(&key, scope, ctx);
        }
        // An argument that is itself a check (`assert($items[0] instanceof
        // Foo)`) carries its subject one level further down, so it is
        // seeded the same way an `if` condition's subject is.
        seed_property_keys_into_scope(arg_expr, scope, ctx);
    }
}

/// Collect property access keys (e.g. `$a->foo`) from conditions that
/// contain type guards or instanceof checks on property accesses.
/// These keys are injected into the scope so that narrowing applies.
pub(crate) fn collect_condition_property_keys(expr: &Expression<'_>) -> Vec<String> {
    let mut keys = Vec::new();
    collect_condition_property_keys_inner(expr, &mut keys);
    keys
}

pub(crate) fn collect_condition_property_keys_inner(expr: &Expression<'_>, keys: &mut Vec<String>) {
    match expr {
        // instanceof: `$a->foo instanceof Foo` or `$row["page"] instanceof Foo`
        Expression::Binary(bin) if bin.operator.is_instanceof() => {
            if let Some(key) = narrowing::expr_to_subject_key(bin.lhs)
                && (key.contains("->") || key.contains("[\""))
                && !keys.contains(&key)
            {
                keys.push(key);
            }
        }
        // Negation: `!is_string($a->foo)`, `!($a->foo instanceof Foo)`
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            collect_condition_property_keys_inner(prefix.operand, keys);
        }
        Expression::Parenthesized(p) => {
            collect_condition_property_keys_inner(p.expression, keys);
        }
        // Logical connectives
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_)
                    | BinaryOperator::LowAnd(_)
                    | BinaryOperator::Or(_)
                    | BinaryOperator::LowOr(_)
            ) =>
        {
            collect_condition_property_keys_inner(bin.lhs, keys);
            collect_condition_property_keys_inner(bin.rhs, keys);
        }
        // Type guard functions: `is_string($a->foo)`, `is_int($a->foo)`, etc.
        Expression::Call(Call::Function(func_call)) => {
            if let Expression::Identifier(ident) = func_call.function {
                let func_name = bytes_to_str(ident.value());
                let is_type_guard = matches!(
                    func_name,
                    "is_array"
                        | "is_string"
                        | "is_int"
                        | "is_integer"
                        | "is_long"
                        | "is_float"
                        | "is_double"
                        | "is_real"
                        | "is_bool"
                        | "is_object"
                        | "is_numeric"
                        | "is_callable"
                        | "is_null"
                        | "is_scalar"
                        | "is_a"
                        | "class_exists"
                        | "interface_exists"
                        | "enum_exists"
                        | "trait_exists"
                );
                if is_type_guard && let Some(first_arg) = func_call.argument_list.arguments.first()
                {
                    let arg_expr = match first_arg {
                        Argument::Positional(pos) => pos.value,
                        Argument::Named(named) => named.value,
                    };
                    if let Some(key) = narrowing::expr_to_subject_key(arg_expr)
                        && (key.contains("->") || key.contains("[\""))
                        && !keys.contains(&key)
                    {
                        keys.push(key);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Resolve the type of a property access key (e.g. `$a->foo`) or an
/// argument-less call key (`$a->foo()`) from the current scope and seed
/// it into the scope as a synthetic entry.  This allows subsequent
/// narrowing functions to find and narrow those expressions, and it is
/// what keeps a check against a wide type from discarding a narrower
/// declared one: seeded with `StringExpr`, an `instanceof Expr` guard
/// intersects down to `StringExpr` rather than replacing it.
pub(crate) fn seed_property_keys_into_scope(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let keys = collect_condition_property_keys(condition);
    if keys.is_empty() {
        return;
    }
    for key in &keys {
        // An array-access subject (`$items[0]`) is keyed and narrowed the
        // same way a property is, so both go through the seeder that knows
        // how to read an element type out of the base variable's type.
        // `seed_synthetic_key_if_needed` skips a key already seeded (e.g.
        // from a prior elseif condition).
        seed_synthetic_key_if_needed(key, scope, ctx);
    }
}

/// Resolve what a member key's declaration promises, reading the scope
/// but not writing to it.
fn resolve_member_key_type(
    key: &str,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    let (head, is_call) = match key.strip_suffix("()") {
        Some(head) => (head, true),
        None => (key, false),
    };
    let Some(arrow_pos) = head.rfind("->") else {
        return Vec::new();
    };
    let obj_var = &head[..arrow_pos];
    let member_name = &head[arrow_pos + 2..];

    // Resolve the object part's type from scope.  Only the leading
    // variable of a path is ever assigned there, so a deeper path
    // (`$this->holder` in `$this->holder->service`, or `$rows["0"]` in
    // `$rows["0"]->name`) has to be resolved the same way this key is.
    // Each step drops one segment, so the recursion is bounded by the
    // number of segments in the key.
    let resolved_prefix;
    let obj_types: &[ResolvedType] = match scope.get(obj_var) {
        [] if obj_var.contains("->") || obj_var.ends_with("\"]") => {
            resolved_prefix = resolve_synthetic_key_type(obj_var, scope, ctx);
            &resolved_prefix
        }
        from_scope => from_scope,
    };
    if obj_types.is_empty() {
        return Vec::new();
    }

    // Look up the member's type on the resolved class(es).
    let mut member_results: Vec<ResolvedType> = Vec::new();
    for rt in obj_types {
        let Some(ref cls) = rt.class_info else {
            continue;
        };
        let type_hint = if is_call {
            crate::inheritance::resolve_method_return_type(cls, member_name, ctx.class_loader)
        } else {
            crate::inheritance::resolve_property_type_hint(cls, member_name, ctx.class_loader)
        };
        let Some(hint) = type_hint else {
            continue;
        };
        let resolved_classes = crate::type_engine::type_resolution::type_hint_to_classes_typed(
            &hint,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if resolved_classes.is_empty() {
            // A return type that names nothing loadable is usually a
            // template parameter or a generic alias, answered better by
            // the call resolver at the use site, which substitutes from
            // the receiver.  Seeding the bare hint here would shadow that
            // with a type no class stands behind.  A hint built entirely
            // from keyword types (`string|false`, `bool|null`, ...) can
            // never be a template parameter or alias, though, so seeding
            // it is safe and lets scalar narrowing apply to call keys the
            // same way it already does for property keys.
            if is_call && !is_all_keyword_type(&hint) {
                continue;
            }
            ResolvedType::extend_unique(
                &mut member_results,
                vec![ResolvedType::from_type_string(hint)],
            );
        } else {
            ResolvedType::extend_unique(
                &mut member_results,
                ResolvedType::from_classes_with_hint(resolved_classes, hint),
            );
        }
    }

    member_results
}

/// Whether `hint` is built entirely from keyword types (`string`, `false`,
/// `null`, other scalars and pseudo-types) with no class-like name
/// anywhere in it.  Such a hint can never be a template parameter or a
/// generic alias, so it is safe to seed even when it names nothing
/// loadable.
fn is_all_keyword_type(hint: &PhpType) -> bool {
    match hint.kind() {
        TypeKind::Named(name) => crate::php_type::is_keyword_type(name),
        TypeKind::Nullable(inner) => is_all_keyword_type(inner),
        TypeKind::Union(members) => members.iter().all(is_all_keyword_type),
        TypeKind::Literal(_) => true,
        _ => false,
    }
}

pub(crate) fn collect_condition_var_names_inner(expr: &Expression<'_>, names: &mut Vec<String>) {
    match expr {
        Expression::Binary(bin) if bin.operator.is_instanceof() => {
            if let Expression::Variable(Variable::Direct(dv)) = bin.lhs {
                let name = bytes_to_str(dv.name).to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            collect_condition_var_names_inner(prefix.operand, names);
        }
        Expression::Parenthesized(p) => {
            collect_condition_var_names_inner(p.expression, names);
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_)
                    | BinaryOperator::LowAnd(_)
                    | BinaryOperator::Or(_)
                    | BinaryOperator::LowOr(_)
            ) =>
        {
            collect_condition_var_names_inner(bin.lhs, names);
            collect_condition_var_names_inner(bin.rhs, names);
        }
        // is_a($var, ...) and get_class($var) === ...
        Expression::Call(Call::Function(func_call)) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => bytes_to_str(ident.value()),
                _ => return,
            };
            if matches!(
                func_name,
                "is_a"
                    | "get_class"
                    | "class_exists"
                    | "interface_exists"
                    | "enum_exists"
                    | "trait_exists"
            ) && let Some(first_arg) = func_call.argument_list.arguments.first()
            {
                let arg_expr = match first_arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
                    let name = bytes_to_str(dv.name).to_string();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Check whether a statement exits via `break` or `continue` (loop-local
/// exit) rather than `return` or `throw` (function exit).
///
/// When an if-branch exits via `break`/`continue`, the variable
/// assignments made in that branch still flow to the post-loop scope.
/// The if-merge should include these branch scopes in the surviving
/// set so that the merged post-if scope reflects the assignments.
pub(crate) fn exits_via_loop_control(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::Break(_) | Statement::Continue(_) => true,
        Statement::Block(block) => block.statements.last().is_some_and(exits_via_loop_control),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::split_array_access_key;

    #[test]
    fn splits_single_level_string_key() {
        assert_eq!(split_array_access_key("$a[\"test\"]"), Some(("$a", "test")));
    }

    #[test]
    fn rejects_non_array_access() {
        assert_eq!(split_array_access_key("$a"), None);
    }

    #[test]
    fn rejects_nested_array_access() {
        // `$a["x"]["y"]` must not be mis-split into base `$a` and key
        // `x"]["y`; single-key narrowing cannot represent it.
        assert_eq!(split_array_access_key("$a[\"x\"][\"y\"]"), None);
    }

    #[test]
    fn rejects_base_with_earlier_access() {
        assert_eq!(split_array_access_key("$a[0][\"y\"]"), None);
    }
}
