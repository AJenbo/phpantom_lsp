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
///
/// A conjunct may itself be an `||` chain over one subject
/// (`$shouldCheck = $n instanceof Function_ || $n instanceof ClassMethod`),
/// which proves the subject is one of the listed classes exactly as the
/// same chain written straight into an `if` does.
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
            if let Some(mut classes) = or_chain_alternatives(operand, &subject) {
                let class_type = classes.remove(0);
                checks.push(VarAssertion {
                    subject: atom(&subject),
                    class_type,
                    alternatives: classes,
                    negated: false,
                    exact: false,
                    allow_string: false,
                });
                break;
            }
            if let Some(extraction) =
                narrowing::try_extract_instanceof_with_negation(operand, &subject)
            {
                checks.push(VarAssertion {
                    subject: atom(&subject),
                    class_type: extraction.class_type,
                    alternatives: Vec::new(),
                    negated: extraction.negated,
                    exact: extraction.exact,
                    allow_string: extraction.allow_string,
                });
                break;
            }
        }
    }
    if !checks.is_empty() {
        scope.assertions.insert(atom(lhs_name), checks);
    }
}

/// The classes an `||` chain over `subject` proves it is one of.
///
/// Every leg has to be a positive check on the same subject: a leg about
/// some other value, or a negated one, leaves the disjunction proving
/// nothing about `subject`, and collecting only the legs that do match
/// would claim a narrowing the chain never made.
fn or_chain_alternatives(expr: &Expression<'_>, subject: &str) -> Option<Vec<PhpType>> {
    fn walk(expr: &Expression<'_>, subject: &str, out: &mut Vec<PhpType>) -> bool {
        match expr {
            Expression::Parenthesized(inner) => walk(inner.expression, subject, out),
            Expression::Binary(bin)
                if matches!(
                    bin.operator,
                    BinaryOperator::Or(_) | BinaryOperator::LowOr(_)
                ) =>
            {
                walk(bin.lhs, subject, out) && walk(bin.rhs, subject, out)
            }
            _ => match narrowing::try_extract_instanceof_with_negation(expr, subject) {
                Some(extraction) if !extraction.negated => {
                    if !out.contains(&extraction.class_type) {
                        out.push(extraction.class_type);
                    }
                    true
                }
                _ => false,
            },
        }
    }

    let is_or_chain = matches!(
        narrowing::fold_negation_pairs(expr),
        Expression::Binary(bin) if matches!(bin.operator, BinaryOperator::Or(_) | BinaryOperator::LowOr(_))
    );
    if !is_or_chain {
        return None;
    }
    let mut classes = Vec::new();
    (walk(expr, subject, &mut classes) && classes.len() > 1).then_some(classes)
}

/// Every class an alias check allows, its own first.
fn alias_classes(alias: &AliasExtraction) -> Vec<PhpType> {
    let mut classes = Vec::with_capacity(alias.alternatives.len() + 1);
    classes.push(alias.extraction.class_type.clone());
    classes.extend(alias.alternatives.iter().cloned());
    classes
}

/// One check a boolean stands for, expanded for the condition testing it.
pub(in crate::type_engine) struct AliasExtraction {
    /// Scope key the check narrows.
    pub subject: String,
    /// The check itself, with the operand's own negation folded in.
    pub extraction: narrowing::InstanceofExtraction,
    /// The further classes an `||` chain allows.  Empty for a check that
    /// names a single class.
    pub alternatives: Vec<PhpType>,
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
) -> Vec<AliasExtraction> {
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
        .map(|c| AliasExtraction {
            subject: c.subject.to_string(),
            extraction: narrowing::InstanceofExtraction {
                class_type: c.class_type.clone(),
                negated: c.negated != negated,
                exact: c.exact,
                allow_string: c.allow_string,
            },
            alternatives: c.alternatives.clone(),
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
    /// At least one contributing operand was `is_a($x, C::class, true)` —
    /// a string alternative on the subject must survive the narrowing.
    allow_string: bool,
    /// At least one contributing operand pinned the class exactly
    /// (`get_class($x) === C::class`), so a subclass of `C` does not pass.
    exact: bool,
}

/// What a condition's `instanceof`-style checks concluded about one
/// subject, beyond the classes themselves.
#[derive(Default, Clone, Copy)]
struct CheckShape {
    /// The classes describe one value that is all of them at once.
    intersected: bool,
    /// A string alternative on the subject must survive the narrowing.
    allow_string: bool,
    /// The classes are exact identities rather than subtype bounds.
    exact: bool,
}

impl Conjuncts {
    /// Whether the accumulated classes describe one value that is all of
    /// them at once.
    ///
    /// `$x instanceof A && $x instanceof B` proves both, so the value is
    /// `A&B`.  One operand on its own proves a single class, so it does
    /// not conclude an intersection.
    fn is_intersection(&self) -> bool {
        self.operands > 1
    }
}

/// The chain inside a `!` that negates a whole `&&` / `||` expression.
///
/// A negation over a chain says the opposite of everything the chain says,
/// so the pass that reads it is the *other* polarity's: the truthy branch
/// of `if (!(A || B))` is the inverse of `A || B`, and the fall-through of
/// `if (!(A || B)) { return; }` is its truthy narrowing.  Handing the
/// chain over is what lets each operand be examined at all — the negated
/// disjunction as a whole matches no extractor, so without this the
/// widest idiom for "one of these two types" narrows nothing.
///
/// Only chains delegate.  A `!` over a single check is a shape the
/// extractors recognise in place, and routing it through the opposite
/// pass would change which commit path it takes.
fn negated_logical_chain<'b>(expr: &'b Expression<'b>) -> Option<&'b Expression<'b>> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    let is_chain = matches!(
        inner,
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_)
                    | BinaryOperator::LowAnd(_)
                    | BinaryOperator::Or(_)
                    | BinaryOperator::LowOr(_)
            )
    );
    (negated && is_chain).then_some(inner)
}

/// Apply condition-based narrowing (instanceof, null check, type guard)
/// to the scope.  This narrows types for the "truthy" branch.
pub(crate) fn apply_condition_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // `!(!$x)` says exactly what `$x` says, so cancel the pair before any
    // extractor looks at it.  The chain collectors fold each operand of an
    // `&&` / `||` the same way.
    let condition = narrowing::fold_negation_pairs(condition);

    // A `!` over a logical chain proves what the *inverse* pass proves
    // about the chain itself.
    if let Some(inner) = negated_logical_chain(condition) {
        apply_condition_narrowing_inverse(inner, scope, ctx);
        return;
    }

    // Seed property access keys from conditions into the scope so that
    // narrowing functions can find and narrow them.
    seed_property_keys_into_scope(condition, scope, ctx);

    // Decompose `&&` chains so that `$x instanceof Foo && $x instanceof Bar`
    // applies both narrowings as a union (intersection semantics: the
    // variable satisfies both checks, so members from both types are
    // available).
    //
    // An operand that is itself a disjunction is held back from that
    // decomposition: entering the branch proves the disjunction, not any
    // one of its legs, and every pass below narrows what it is handed as
    // though it had held.  The join at the end of this function owns them
    // instead, so a leg's own conclusion never reaches the branch body
    // unless every other leg proves it too.
    let (disjunctions, operands): (Vec<_>, Vec<_>) = collect_and_chain_operands(condition)
        .into_iter()
        .partition(|operand| collect_or_chain_operands(unwrap_parens(operand)).len() > 1);

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
    let alias_extractions: Vec<Vec<AliasExtraction>> = operands
        .iter()
        .map(|operand| assertion_alias_extractions(operand, scope))
        .collect();
    for subject in alias_extractions.iter().flatten().map(|a| &a.subject) {
        if !var_names.contains(subject) {
            var_names.push(subject.clone());
        }
    }

    let mut pinned = commit_chain_instanceof(&operands, &alias_extractions, &var_names, scope, ctx);

    // A key read through a receiver the same chain narrows is only
    // resolvable once that narrowing has landed:
    // `$expr instanceof FuncCall && !$expr->name instanceof Name` cannot
    // look `name` up on `FuncCall` until the first operand has proved that
    // is what `$expr` is.  Seeding is left until here for those keys, and
    // the extraction runs again over them alone — the subjects the first
    // run committed are already narrowed and must not be narrowed twice.
    let late_keys: Vec<String> = collect_condition_property_keys(condition)
        .into_iter()
        .filter(|key| !scope.contains(key))
        .collect();
    if !late_keys.is_empty() {
        for key in &late_keys {
            seed_synthetic_key_if_needed(key, scope, ctx);
        }
        let seeded: Vec<String> = late_keys
            .into_iter()
            .filter(|key| scope.contains(key))
            .collect();
        if !seeded.is_empty() {
            pinned.extend(commit_chain_instanceof(
                &operands,
                &alias_extractions,
                &seeded,
                scope,
                ctx,
            ));
        }
    }

    // The passes below still read the whole condition, disjunctions and
    // all.  Each of them either decomposes `&&` and stops at an operand it
    // does not recognise, or reads the condition as one expression — so a
    // disjunction is opaque to them and none of a leg's conclusions can
    // escape through them.  A new pass that looks *inside* an `||` belongs
    // in the leg walk below, not here.

    // Type guard narrowing: `is_object($x)`, `is_array($x)`, etc.
    apply_type_guard_narrowing_truthy(condition, scope, ctx);

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

    // array_key_exists('k', $arr) narrowing on an optional shape key.
    apply_array_key_exists_narrowing(condition, scope, ctx, false);

    // `if (preg_match(…, $matches))` — the body runs on a successful match,
    // so `$matches` has the keys the pattern describes.
    apply_preg_match_narrowing(condition, scope, ctx, true);

    // Each disjunction the chain held back proves only that one of its
    // legs held.  Splitting it re-uses the branch join, so the scope ends
    // up carrying both the union of what the legs prove and the record of
    // which leg proved what.
    apply_disjunct_operand_narrowing(&disjunctions, &pinned, scope, ctx);

    // Whatever the passes above proved about one value's null, they
    // proved about every value whose null it stands for.  Last, so it
    // sees the narrowed state rather than the state on the way in.
    apply_non_null_implication_narrowing(scope, ctx);
}

/// Narrow through an `&&` operand that is itself an `||` chain.
///
/// Entering the branch proves the disjunction as a whole, not any one leg,
/// so the scope inside it is the join of the scopes the legs would each
/// produce — exactly the shape [`ScopeState::merge_branch`] already
/// handles for an `if`/`else`.  Going through the join is what records
/// which leg proved what, so a check further down that rules the other
/// legs out recovers the surviving leg's conclusions:
///
/// ```php
/// if ($n->keyVar === null || ($n->keyVar instanceof Variable && is_string($n->keyVar->name))) {
///     $name = $n->keyVar instanceof Variable ? $n->keyVar->name : null;   // string|null
/// }
/// ```
///
/// The join is the *only* treatment a disjunction gets: the passes that
/// run before it are handed the chain's plain conjuncts alone, so what
/// they leave behind is the base every leg starts from.  That ordering is
/// what keeps a leg's own conclusion out of the branch body — reading
/// `$v instanceof Variable || $flag` as though the `instanceof` had held
/// dropped the `null` the guard never ruled out.
///
/// `pinned` names the subjects a conjunct already narrowed to a definite
/// class.  Those the join leaves alone: `$b instanceof Generic && ($cls
/// === Generic::class || $b instanceof Template)` proves `$b` is a
/// `Generic`, and a leg naming an unrelated class replaces rather than
/// intersects, so joining the legs would answer `Generic|Template` and
/// lose what the conjunct established.
fn apply_disjunct_operand_narrowing<'b>(
    disjunctions: &[&'b Expression<'b>],
    pinned: &[String],
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    for operand in disjunctions {
        let legs = collect_or_chain_operands(unwrap_parens(operand));
        if legs.len() < 2 {
            continue;
        }

        let leg_scopes: Vec<ScopeState> = legs
            .into_iter()
            .map(|leg| {
                let mut leg_scope = scope.clone();
                apply_condition_narrowing(leg, &mut leg_scope, ctx);
                drop_leg_answers_the_base_rules_out(&mut leg_scope, scope);
                leg_scope
            })
            .collect();

        let mut joined = leg_scopes[0].clone();
        for leg_scope in &leg_scopes[1..] {
            joined.merge_branch(leg_scope);
        }
        // A path that could not read a member path at all did not narrow
        // it, and adopting the one leg that could would claim its answer
        // for the whole disjunction.
        let refs: Vec<&ScopeState> = leg_scopes.iter().collect();
        retain_synthetic_keys_common_to_all(&mut joined, &refs);
        for subject in pinned {
            let key = atom(subject);
            if let Some(types) = scope.locals.get(&key) {
                joined.locals.insert(key, types.clone());
            }
        }
        *scope = joined;
    }
}

/// Undo what a leg concluded about a subject the branch already knew
/// something else about.
///
/// The legs start from what the chain's conjuncts — and everything above
/// the `if` — proved, and a leg can only refine that. When a leg lands on
/// a type the base rules out, the leg describes a run that cannot happen:
/// the `is_null($price)` half of `is_null($price) || $price->isZero()` on
/// a `$price` a guard already proved non-null. Joining the `null` it
/// wrote back in would hand the branch body the very type the guard
/// removed, so the base's answer stands for that subject instead.
fn drop_leg_answers_the_base_rules_out(leg: &mut ScopeState, base: &ScopeState) {
    for (name, base_types) in &base.locals {
        let Some(leg_types) = leg.locals.get(name) else {
            continue;
        };
        if types_are_disjoint(leg_types, base_types) {
            leg.locals.insert(*name, base_types.clone());
        }
    }
}

/// Narrow every subject the `&&` chain's operands prove an
/// `instanceof`-style check about, and write the result to the scope.
///
/// Collecting across all operands before committing any of them is what
/// keeps a later operand from overwriting an earlier one when both narrow
/// the same subject: `$x instanceof Foo && $x instanceof Bar` has to end
/// up as `Foo&Bar`, not as whichever operand was looked at last.
///
/// Returns the subjects an operand pinned to a definite class, which is
/// what [`apply_disjunct_operand_narrowing`] reads to know whose type a
/// disjunction further along the chain must not widen back.
fn commit_chain_instanceof<'b>(
    operands: &[&'b Expression<'b>],
    alias_extractions: &[Vec<AliasExtraction>],
    var_names: &[String],
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<String> {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };

    // Track which variables have been narrowed by instanceof across
    // `&&` operands so we can merge them, plus where each subject's
    // classes came from so the merge knows whether they are alternatives
    // or an intersection.
    //
    // Operands that prove a single class and operands that prove a set of
    // alternatives are kept apart, because `&&` intersects what its
    // operands prove and these two cannot be intersected by unioning them
    // into one list.  `$b instanceof A && ($cls === A::class || $b
    // instanceof B)` proves `$b` is an `A`; merging `B` in as a peer would
    // answer `A|B` and lose the very thing the first operand established.
    let mut instanceof_results: HashMap<String, Vec<ResolvedType>> = HashMap::new();
    let mut alternative_results: HashMap<String, Vec<ResolvedType>> = HashMap::new();
    let mut conjuncts: HashMap<String, Conjuncts> = HashMap::new();

    for (op_idx, operand) in operands.iter().enumerate() {
        for var_name in var_names {
            // Compound OR instanceof: `$x instanceof A || $x instanceof B`
            if let Some(classes) = narrowing::try_extract_compound_or_instanceof(operand, var_name)
                && !classes.is_empty()
            {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                let union = narrowing::resolve_class_names_to_union(&classes, &var_ctx);
                if !union.is_empty() {
                    let entry = alternative_results.entry(var_name.clone()).or_default();
                    ResolvedType::extend_unique(
                        entry,
                        union.into_iter().map(ResolvedType::from_class).collect(),
                    );
                }
                continue;
            }

            // The same disjunction reached through a boolean that stands
            // for it (`$isNode = $n instanceof Stmt || $n instanceof Expr;
            // if ($isNode)`).
            if let Some(alias) = alias_extractions[op_idx]
                .iter()
                .find(|a| a.subject == *var_name && !a.alternatives.is_empty())
            {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                let classes = alias_classes(alias);
                if alias.extraction.negated {
                    // `!$isNode` — no leg of the chain held, so every one
                    // of them is excluded.
                    let mut results = scope.get(var_name).to_vec();
                    for cls in &classes {
                        ResolvedType::apply_narrowing(&mut results, |class_list| {
                            narrowing::apply_instanceof_exclusion(cls, &var_ctx, class_list)
                        });
                        scope.record_exclusion(var_name, cls);
                    }
                    if !results.is_empty() {
                        scope.set(var_name, results);
                    }
                } else {
                    let union = narrowing::resolve_class_names_to_union(&classes, &var_ctx);
                    if !union.is_empty() {
                        let entry = alternative_results.entry(var_name.clone()).or_default();
                        ResolvedType::extend_unique(
                            entry,
                            union.into_iter().map(ResolvedType::from_class).collect(),
                        );
                    }
                }
                continue;
            }

            // `$x instanceof $stmtClass` — the right-hand side is a value
            // holding the class name, so what it narrows to is whatever
            // that value's type says the class is.
            if let Some((rhs, negated)) =
                narrowing::try_extract_dynamic_instanceof(operand, var_name)
            {
                let targets = dynamic_instanceof_targets(rhs, scope, ctx);
                if !targets.is_empty() {
                    let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                    if negated {
                        let mut results = scope.get(var_name).to_vec();
                        for target in &targets {
                            ResolvedType::apply_narrowing(&mut results, |classes| {
                                narrowing::apply_instanceof_exclusion(target, &var_ctx, classes)
                            });
                            scope.record_exclusion(var_name, target);
                        }
                        if !results.is_empty() {
                            scope.set(var_name, results);
                        }
                    } else {
                        let mut resolved = Vec::new();
                        for target in &targets {
                            let mut single = Vec::new();
                            ResolvedType::apply_narrowing(&mut single, |classes| {
                                narrowing::apply_instanceof_inclusion(
                                    target, false, &var_ctx, classes,
                                )
                            });
                            ResolvedType::extend_unique(&mut resolved, single);
                        }
                        // An operand that names no loadable class (an
                        // unsubstituted `@template T` is the common one)
                        // proves nothing, so the subject is left as it
                        // was rather than emptied.
                        if !resolved.is_empty() {
                            if targets.len() > 1 {
                                ResolvedType::extend_unique(
                                    alternative_results.entry(var_name.clone()).or_default(),
                                    resolved,
                                );
                            } else {
                                ResolvedType::extend_unique(
                                    instanceof_results.entry(var_name.clone()).or_default(),
                                    resolved,
                                );
                                conjuncts.entry(var_name.clone()).or_default().operands += 1;
                            }
                        }
                    }
                    continue;
                }
            }

            // Single instanceof (including negated, is_a, get_class),
            // or a boolean that stands for one.
            if let Some(extraction) =
                narrowing::try_extract_instanceof_with_negation(operand, var_name).or_else(|| {
                    alias_extractions[op_idx]
                        .iter()
                        .find(|a| a.subject == *var_name)
                        .map(|a| a.extraction.clone())
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
                    scope.record_exclusion(var_name, &extraction.class_type);
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
                        let c = conjuncts.entry(var_name.clone()).or_default();
                        c.operands += 1;
                        c.allow_string |= extraction.allow_string;
                        c.exact |= extraction.exact;
                    } else {
                        // Target class is unresolvable — mark variable
                        // as empty so diagnostics suppress false positives.
                        instanceof_results.entry(var_name.clone()).or_default();
                    }
                }
            }
        }
    }

    // A subject an operand proved a definite class for keeps that class:
    // every alternative the rest of the chain allows is one the definite
    // check already has to admit, so the definite class bounds them all
    // and is the safe answer.  Alternatives narrow the subject only when
    // nothing in the chain pinned it down.
    for (var_name, alternatives) in alternative_results {
        let entry = instanceof_results.entry(var_name).or_default();
        if entry.is_empty() {
            *entry = alternatives;
        }
    }

    // Apply the accumulated instanceof narrowing results to the scope.
    for (var_name, narrowed) in instanceof_results {
        // `$x instanceof A && $x instanceof B` proves both at once, so the
        // classes gathered across the operands are members of `A&B`
        // rather than alternatives a consumer may pick one of.
        let shape = CheckShape {
            intersected: conjuncts
                .get(&var_name)
                .is_some_and(Conjuncts::is_intersection),
            allow_string: conjuncts.get(&var_name).is_some_and(|c| c.allow_string),
            exact: conjuncts.get(&var_name).is_some_and(|c| c.exact),
        };
        commit_instanceof_narrowing(&var_name, narrowed, shape, scope, ctx, &scope_resolver);
    }

    conjuncts.into_keys().collect()
}

/// Narrow the `$matches` out-parameter of a `preg_match`/`preg_match_all`
/// call that a condition tests the outcome of.
///
/// The seeding at the call site writes what the call leaves either way
/// (`array{0: string, 1: string}|array{}`), because that is all it can know
/// there. A branch that runs only on a successful match knows the array has
/// the pattern's keys, and one that runs only on a failed match knows it is
/// empty. That is what keeps a group read inside the guard a plain `string`,
/// while the same read on a line either outcome reaches carries the `null` a
/// missing key yields.
pub(crate) fn apply_preg_match_narrowing(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    truthy: bool,
) {
    // `preg_match(…, $m) && $m[1] === 'x'` proves the match for the whole
    // conjunction; an `||` operand proves nothing, and is left whole by the
    // decomposition below.
    for operand in collect_and_chain_operands(condition) {
        // The condition either makes the call itself, or names a variable
        // holding a result the walk recorded at the assignment.  Both say
        // the same thing about the out-parameter.
        let Some((matches_var, matched, matches_all, matched_when_true)) =
            crate::type_engine::regex_shape::preg_condition(operand)
                .and_then(|(call, matched_when_true)| {
                    let matched = preg_matched_type(&call, scope, ctx)?;
                    Some((
                        atom(call.matches_var),
                        matched,
                        call.matches_all,
                        matched_when_true,
                    ))
                })
                .or_else(|| {
                    let (outcome, matched_when_true) = preg_outcome_alias(operand, scope)?;
                    Some((
                        outcome.matches_var,
                        outcome.matched,
                        outcome.matches_all,
                        matched_when_true,
                    ))
                })
        else {
            continue;
        };
        let narrowed = if matched_when_true == truthy {
            matched
        } else {
            crate::type_engine::regex_shape::no_match_type(&matched, matches_all).unwrap_or(matched)
        };
        scope.set(&matches_var, vec![ResolvedType::from_type_string(narrowed)]);
    }
}

/// Record the `preg_match` outcome a variable's value is.
///
/// `$ok = preg_match('/(\d+)/', $s, $m);` proves nothing on its own, but
/// `$ok` now stands for the match: wherever it is tested, `$m` narrows the
/// same way testing the call would narrow it.  The comparison spellings
/// (`$ok = preg_match(…) === 1`) come along for free, since the condition
/// reader handles them and this reuses it.
pub(crate) fn record_preg_outcome<'b>(
    lhs_name: &str,
    rhs: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let Some((call, matched_when_true)) = crate::type_engine::regex_shape::preg_condition(rhs)
    else {
        return;
    };
    // `$m = preg_match(…, $m)` overwrites the very array the outcome would
    // describe, so there is nothing left for it to narrow.
    if call.matches_var == lhs_name {
        return;
    }
    let Some(matched) = preg_matched_type(&call, scope, ctx) else {
        return;
    };
    // `$failed = !preg_match(…, $m)` holds the negation, which the reader
    // folds into `matched_when_true`.  Storing the positive shape and
    // flipping the sense here keeps the lookup side free of polarity.
    let matched = if matched_when_true {
        matched
    } else {
        match crate::type_engine::regex_shape::no_match_type(&matched, call.matches_all) {
            Some(no_match) => no_match,
            None => return,
        }
    };
    scope.preg_outcomes.insert(
        atom(lhs_name),
        PregOutcome {
            matches_var: atom(call.matches_var),
            matched,
            matches_all: call.matches_all,
        },
    );
}

/// Expand a bare boolean operand into the `preg_match` outcome it stands
/// for.
///
/// `$ok` and `!$ok` both resolve through the recorded outcome, with the
/// operand's own negation folded into the result, so the caller treats
/// them exactly like the call the outcome came from.
fn preg_outcome_alias(expr: &Expression<'_>, scope: &ScopeState) -> Option<(PregOutcome, bool)> {
    if scope.preg_outcomes.is_empty() {
        return None;
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
        return None;
    };
    let outcome = scope.preg_outcomes.get(&atom(bytes_to_str(dv.name)))?;
    Some((outcome.clone(), !negated))
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
/// The classes `$subject instanceof <value>` narrows to, when the
/// right-hand side is a value rather than a literal class name.
///
/// A `class-string<T>` names `T`; a union of them names each `T` as an
/// alternative; and an object-typed right-hand side stands for its own
/// class, which is what `$node instanceof $other` checks.  A bare
/// `class-string` or plain `string` names nothing, so the check proves
/// nothing and the subject is left alone.
fn dynamic_instanceof_targets(
    rhs: &Expression<'_>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };
    let var_ctx = build_var_ctx("", ctx, &scope_resolver);
    let Some(resolved) =
        crate::type_engine::variable::resolution::resolve_arg_raw_type(rhs, &var_ctx)
    else {
        return Vec::new();
    };

    let mut targets = Vec::new();
    for member in resolved.union_members() {
        let target = match member.kind() {
            TypeKind::ClassString(Some(inner)) | TypeKind::InterfaceString(Some(inner)) => {
                inner.clone()
            }
            // An object-typed operand stands for its own class.  A
            // keyword type (`string`, `object`, `mixed`) names no class,
            // so the check proves nothing about the subject.
            TypeKind::Named(_) | TypeKind::Generic { .. } if !member.is_keyword() => member.clone(),
            _ => continue,
        };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    targets
}

fn commit_instanceof_narrowing(
    var_name: &str,
    mut narrowed: Vec<ResolvedType>,
    shape: CheckShape,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    scope_resolver: &dyn Fn(&str) -> Vec<ResolvedType>,
) {
    let CheckShape {
        intersected,
        allow_string,
        exact,
    } = shape;
    if intersected {
        ResolvedType::tag_as_intersection(&mut narrowed);
    }
    if narrowed.is_empty() {
        // Empty narrowed list means the target was unresolvable.
        scope.set_untyped(var_name);
        return;
    }

    let existing = scope.get(var_name);
    if existing.is_empty() {
        // Untyped variable — instanceof provides the type.
        scope.set(var_name, narrowed);
        return;
    }

    // `is_a($x, C::class, true)` also passes when `$x` is a
    // `class-string<C>`, so a string alternative already in the subject's
    // type must survive every path below that would otherwise replace the
    // whole union with just the checked class. The string half can be a
    // whole entry (`string`) or one member of a broader union carried by a
    // single entry (`object|string`), so pull it out at the union-member
    // level rather than requiring the whole entry to be string-only.
    // `apply_class_string_guard_narrowing` (run right after this pass)
    // turns the preserved string alternative into `class-string<C>`.
    let string_alt: Vec<ResolvedType> = if allow_string {
        existing
            .iter()
            .filter(|rt| rt.class_info.is_none())
            .filter_map(|rt| {
                let stringy: Vec<PhpType> = rt
                    .type_string
                    .union_members()
                    .into_iter()
                    .filter(|m| m.is_subtype_of(&PhpType::string()))
                    .cloned()
                    .collect();
                match stringy.len() {
                    0 => None,
                    1 => Some(ResolvedType::from_type_string(
                        stringy.into_iter().next().unwrap(),
                    )),
                    _ => Some(ResolvedType::from_type_string(PhpType::union(stringy))),
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let with_string_alt = |mut result: Vec<ResolvedType>| -> Vec<ResolvedType> {
        if !string_alt.is_empty() {
            ResolvedType::extend_unique(&mut result, string_alt.clone());
        }
        result
    };

    // A successful check rules out every alternative that cannot be an
    // object, and an `object`/`mixed` alternative says nothing the checked
    // class does not already say.  When that leaves no class alternative
    // to filter down to, the check's conclusion is the whole answer:
    // `object|string` (the shape a route parameter arrives as) becomes the
    // checked class instead of growing it as one more alternative.  `null`
    // counts as ruled out too, so `?object` is as uninformative as bare
    // `object`.
    //
    // A subject with no object alternative at all is deliberately left to
    // the passes that run after this one: `is_a($s, C::class, true)` on a
    // `string` proves a `class-string<C>`, not a `C`.
    let is_broad_object = |ty: &PhpType| {
        matches!(
            ty.kind(),
            TypeKind::Named(n) if n.eq_ignore_ascii_case("mixed") || n.eq_ignore_ascii_case("object")
        )
    };
    let mut names_class = false;
    let mut broad_object = false;
    for rt in existing {
        if rt.class_info.is_some() {
            names_class = true;
            break;
        }
        for member in rt.type_string.union_members() {
            if !member.top_level_class_names().is_empty() {
                names_class = true;
                break;
            }
            let bare = member.non_null_type();
            if is_broad_object(bare.as_ref().unwrap_or(member)) {
                broad_object = true;
            }
        }
        if names_class {
            break;
        }
    }
    if !names_class && broad_object {
        scope.set(var_name, with_string_alt(narrowed));
        return;
    }

    // The other half of that rule: a subject that can only hold a string
    // has no object alternative for the checked class to filter down to,
    // so `is_a($s, C::class, true)` proves a `class-string<C>` and
    // nothing more.  Leave it to `apply_class_string_guard_narrowing`,
    // which runs right after this pass — adding `C` here would put an
    // object alternative into a value that is definitely a string.
    if allow_string && !string_alt.is_empty() && !names_class {
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
    // set, or is a subtype of one — a `VarLikeIdentifier` alternative
    // passes `instanceof Identifier` and stays as itself, being the more
    // specific of the two.  An exact check
    // (`get_class($x) === Identifier::class`) pins the identity instead,
    // and there the subclass does not pass.
    //
    // A kept entry's own type_string may still be the whole pre-check union
    // (a conditional return type resolves to one entry naming a class and
    // listing an array alternative beside it), so restrict it to the
    // surviving classes as well.  Strip null on top because a successful
    // instanceof check guarantees the value is non-null (`?Foo` → `Foo`).
    let passes_check = |fqn: &str| {
        narrowed_fqns.iter().any(|n| {
            n == fqn
                || (!exact && crate::class_lookup::is_subtype_of_names(fqn, n, ctx.class_loader))
        })
    };
    let survives = |name: &str| {
        narrowed.iter().any(|rt| {
            rt.class_info
                .as_ref()
                .is_some_and(|c| c.name == name || c.fqn() == name)
        }) || passes_check(name)
    };
    let filtered: Vec<ResolvedType> = existing
        .iter()
        .filter(|rt| {
            rt.class_info
                .as_ref()
                .is_some_and(|c| passes_check(&c.fqn()))
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
            scope.set(var_name, with_string_alt(narrowed));
        } else {
            scope.set(var_name, with_string_alt(filtered));
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
    // separate entries, which describe one value that is both at once —
    // recognisable by a class surviving that the check never named.  When
    // the inclusion instead *replaced* the subject's class (the checked
    // classes are the more specific ones), the result is those classes as
    // the check handed them over, and a `||` chain hands over
    // alternatives, not an intersection.
    let kept_unchecked_class = results.iter().any(|rt| {
        rt.class_info
            .as_ref()
            .is_some_and(|c| !narrowed_fqns.iter().any(|n| n == c.fqn()))
    });
    if intersected || kept_unchecked_class {
        ResolvedType::tag_as_intersection(&mut results);
    }
    if !results.is_empty() {
        scope.set(var_name, with_string_alt(results));
    } else {
        // Fallback: use the narrowed types directly.
        scope.set(var_name, with_string_alt(narrowed));
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
    for a in &alias_extractions {
        if !var_names.contains(&a.subject) {
            var_names.push(a.subject.clone());
        }
    }
    for var_name in &var_names {
        // The inverse of a disjunction the condition names, or of one a
        // boolean stands for: neither leg held, so every class it lists
        // is excluded.
        let alias_or = alias_extractions
            .iter()
            .find(|a| a.subject == *var_name && !a.alternatives.is_empty());
        if let Some(alias) = alias_or.filter(|a| !a.extraction.negated) {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let had_types = !scope.get(var_name).is_empty();
            let mut results = scope.get(var_name).to_vec();
            for cls in alias_classes(alias) {
                ResolvedType::apply_narrowing(&mut results, |class_list| {
                    narrowing::apply_instanceof_exclusion(&cls, &var_ctx, class_list)
                });
                scope.record_exclusion(var_name, &cls);
            }
            if !results.is_empty() {
                scope.set(var_name, results);
            } else if had_types {
                scope.unreachable = true;
            }
            continue;
        }
        // The inverse of `!$isNode`: the chain did hold, so the subject
        // is one of the classes it lists.
        if let Some(alias) = alias_or {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let union = narrowing::resolve_class_names_to_union(&alias_classes(alias), &var_ctx);
            if !union.is_empty() {
                commit_instanceof_narrowing(
                    var_name,
                    union.into_iter().map(ResolvedType::from_class).collect(),
                    CheckShape::default(),
                    scope,
                    ctx,
                    &scope_resolver,
                );
            }
            continue;
        }

        if let Some(classes) = narrowing::try_extract_compound_or_instanceof(condition, var_name)
            && !classes.is_empty()
        {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let had_types = !scope.get(var_name).is_empty();
            let mut results = scope.get(var_name).to_vec();
            for cls_type in &classes {
                ResolvedType::apply_narrowing(&mut results, |class_list| {
                    narrowing::apply_instanceof_exclusion(cls_type, &var_ctx, class_list)
                });
                scope.record_exclusion(var_name, cls_type);
            }
            if !results.is_empty() {
                scope.set(var_name, results);
            } else if had_types {
                scope.unreachable = true;
            }
            continue;
        }

        // `if (!$x instanceof $stmtClass) { continue; }` — the
        // fall-through proves the dynamic check held.
        if let Some((rhs, negated)) = narrowing::try_extract_dynamic_instanceof(condition, var_name)
        {
            let targets = dynamic_instanceof_targets(rhs, scope, ctx);
            if !targets.is_empty() {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                if negated {
                    let mut narrowed = Vec::new();
                    for target in &targets {
                        let mut single = Vec::new();
                        ResolvedType::apply_narrowing(&mut single, |classes| {
                            narrowing::apply_instanceof_inclusion(target, false, &var_ctx, classes)
                        });
                        ResolvedType::extend_unique(&mut narrowed, single);
                    }
                    // An operand that names no loadable class proves
                    // nothing; committing an empty result would instead
                    // mark the subject unresolvable.
                    if !narrowed.is_empty() {
                        commit_instanceof_narrowing(
                            var_name,
                            narrowed,
                            CheckShape::default(),
                            scope,
                            ctx,
                            &scope_resolver,
                        );
                    }
                } else {
                    let had_types = !scope.get(var_name).is_empty();
                    let mut results = scope.get(var_name).to_vec();
                    for target in &targets {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_exclusion(target, &var_ctx, classes)
                        });
                        scope.record_exclusion(var_name, target);
                    }
                    if !results.is_empty() {
                        scope.set(var_name, results);
                    } else if had_types {
                        scope.unreachable = true;
                    }
                }
                continue;
            }
        }

        if let Some(extraction) =
            narrowing::try_extract_instanceof_with_negation(condition, var_name).or_else(|| {
                alias_extractions
                    .iter()
                    .find(|a| a.subject == *var_name)
                    .map(|a| a.extraction.clone())
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
                commit_instanceof_narrowing(
                    var_name,
                    narrowed,
                    CheckShape {
                        allow_string: extraction.allow_string,
                        exact: extraction.exact,
                        ..CheckShape::default()
                    },
                    scope,
                    ctx,
                    &scope_resolver,
                );
            } else {
                // Inverse of positive instanceof → exclusion.
                // Exclusion does NOT strip null (`!instanceof` is
                // true for null values).
                let had_types = !scope.get(var_name).is_empty();
                let mut results = scope.get(var_name).to_vec();
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_exclusion(&extraction.class_type, &var_ctx, classes)
                });
                scope.record_exclusion(var_name, &extraction.class_type);
                if !results.is_empty() {
                    scope.set(var_name, results);
                } else if had_types {
                    // Every alternative the variable had was excluded, so
                    // nothing can reach this path: `$v` was already an
                    // `AbstractNode` and this is the else of
                    // `if ($v instanceof AbstractNode)`.
                    scope.unreachable = true;
                }
            }
        }
    }

    // Inverse member-existence narrowing: after a guard clause like
    // `if (!property_exists($x, 'name')) { return; }`, the member is
    // known to exist.
    apply_member_exists_narrowing(condition, scope, true);

    // Inverse `array_key_exists` narrowing: after
    // `if (!array_key_exists('k', $arr)) { return; }` the key is present.
    apply_array_key_exists_narrowing(condition, scope, ctx, true);

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
    // As in the truthy pass: `!(!$x)` is `$x`, so cancel the pair first.
    let condition = narrowing::fold_negation_pairs(condition);

    // The mirror of the truthy pass: the fall-through of
    // `if (!($t instanceof CallableType || $t instanceof ClosureType)) { return; }`
    // is what the chain itself proves, so hand it to the truthy pass.
    if let Some(inner) = negated_logical_chain(condition) {
        apply_condition_narrowing(inner, scope, ctx);
        return;
    }

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
            // A synthetic key only one branch established is not a fact
            // about the merge: the other branches say nothing about it, so
            // the declared type still stands.  Without this,
            // `!($n === 'self' && $s->isInClass())` left
            // `$s->getClassReflection()` narrowed to the `null` that one
            // alternative implies, and a sibling `elseif` that proves the
            // opposite could no longer widen it back.
            let branch_refs: Vec<&ScopeState> = branch_scopes.iter().collect();
            retain_synthetic_keys_common_to_all(&mut merged, &branch_refs);
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
    condition_arm_narrowing(condition, truthy, ctx).0
}

/// [`condition_narrowing_overrides`] plus whether the arm can run at all.
///
/// The flag is `true` when the condition rules out every value one of its
/// subjects could hold, which makes the arm dead code: the else of
/// `$acc === null ? seed($x) : $acc->merge($x)` on the run where `$acc` is
/// still exactly `null`. An arm that cannot run contributes no type, so a
/// caller that unions the arms must leave it out rather than fold in the
/// unresolvable receiver it would have had.
pub(crate) fn condition_arm_narrowing<'b>(
    condition: &'b Expression<'b>,
    truthy: bool,
    ctx: &VarResolutionCtx<'_>,
) -> (HashMap<String, Vec<ResolvedType>>, bool) {
    let Some(resolver) = ctx.scope_var_resolver else {
        return (HashMap::new(), false);
    };

    let mut subjects: Vec<String> = Vec::new();
    collect_condition_subject_vars(condition, &mut subjects);
    for key in collect_condition_property_keys(condition) {
        if !subjects.contains(&key) {
            subjects.push(key);
        }
    }
    if subjects.is_empty() {
        return (HashMap::new(), false);
    }

    let mut scope = ScopeState::new();
    // A condition that tests a boolean standing for an earlier check
    // (`$isFoo ? … : …`) names only the boolean, so the check's own
    // subject has to be seeded before there is anything to narrow.
    if let Some(proofs) = ctx.scope_proofs.filter(|p| !p.is_empty()) {
        scope.adopt_proofs(&proofs);
        let holders: Vec<Atom> = subjects.iter().map(|s| atom(s)).collect();
        for holder in &holders {
            proofs.subjects_of(holder, &mut subjects);
        }
    }
    for subject in &subjects {
        let types = resolver(subject);
        if !types.is_empty() {
            scope.set(subject, types);
        }
    }
    if scope.locals.is_empty() {
        return (HashMap::new(), false);
    }

    let seeded = scope.locals.clone();
    let walk_ctx = ForwardWalkCtx::from_var_ctx(ctx);
    if truthy {
        apply_condition_narrowing(condition, &mut scope, &walk_ctx);
    } else {
        apply_condition_narrowing_inverse(condition, &mut scope, &walk_ctx);
    }

    let impossible = scope.unreachable;
    let overrides = scope
        .locals
        .into_iter()
        .filter(|(name, types)| {
            !types.is_empty()
                && seeded
                    .get(name)
                    .is_none_or(|before| narrowing_changed_types(before, types))
        })
        .map(|(name, types)| (name.to_string(), types))
        .collect();
    (overrides, impossible)
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
    apply_type_guard_narrowing_inverse(condition, scope, ctx);

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

    // Inverse `preg_match` narrowing: the else branch (and the fall-through of
    // an `if (!preg_match(…, $matches)) { return; }` guard) knows the opposite
    // outcome of the one the condition tests for.
    apply_preg_match_narrowing(condition, scope, ctx, false);

    // Whatever the passes above proved about one value's null, they
    // proved about every value whose null it stands for.  Last, so it
    // sees the narrowed state rather than the state on the way in.
    apply_non_null_implication_narrowing(scope, ctx);
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

/// Apply `array_key_exists('k', $arr)` narrowing to an array shape's
/// optional key.
///
/// A shape's optional key reads as `T|null`, because the key may be
/// absent.  `array_key_exists` proves it is present, so the read is
/// plain `T` — the same refinement `isset()` gets, minus the extra proof
/// that the value itself is not null:
///
/// ```php
/// /** @param array{a?: array<int,string>} $shape */
/// if (array_key_exists('a', $shape)) {
///     return $shape['a']; // array<int,string>, not ?array<int,string>
/// }
/// ```
///
/// `inverted` selects the polarity the caller establishes, so an
/// `if (!array_key_exists('a', $shape)) { return; }` guard refines its
/// fall-through.  Only the direction that proves the key *present* adds
/// information: an absent key is not something the shape can record.
pub(crate) fn apply_array_key_exists_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inverted: bool,
) {
    for operand in collect_and_chain_operands(condition) {
        let Some((base_key, key_name, negated)) = array_key_exists_target(operand) else {
            continue;
        };
        if negated != inverted {
            continue;
        }
        // A property or static-property subject (`$this->excludePaths`)
        // is not a tracked local, so its shape has to be brought into
        // the scope before it can be refined.
        seed_synthetic_key_if_needed(&base_key, scope, ctx);
        mark_array_shape_key_present(&base_key, &key_name, scope);
        // Seed the element key after the shape is refined, so an offset
        // read consulting it sees the present-key type rather than the
        // optional one it would have resolved a moment earlier.
        seed_synthetic_key_if_needed(&format!("{base_key}[\"{key_name}\"]"), scope, ctx);
    }
}

/// Extract the `(base subject key, key name, negated)` of an
/// `array_key_exists('k', $arr)` check, unwrapping parentheses and a
/// leading `!`.
fn array_key_exists_target(expr: &Expression<'_>) -> Option<(String, String, bool)> {
    match expr {
        Expression::Parenthesized(inner) => array_key_exists_target(inner.expression),
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            array_key_exists_target(prefix.operand)
                .map(|(base, key, negated)| (base, key, !negated))
        }
        Expression::Call(Call::Function(call)) => {
            let Expression::Identifier(ident) = call.function else {
                return None;
            };
            if bytes_to_str(ident.value()).trim_start_matches('\\') != "array_key_exists" {
                return None;
            }
            let args: Vec<_> = call.argument_list.arguments.iter().collect();
            if args.len() < 2 {
                return None;
            }
            let key_name = narrowing::string_literal_value(narrowing::argument_value(args[0]))?;
            let base_key = narrowing::expr_to_subject_key(narrowing::argument_value(args[1]))?;
            Some((base_key, key_name, false))
        }
        _ => None,
    }
}

/// Mark one key of an array shape as present, leaving its value type
/// alone.
///
/// The sibling [`strip_null_from_array_shape_key`] also drops `null` from
/// the value, which is what `isset()` proves.  `array_key_exists` proves
/// only presence, so a shape entry declared `?T` stays `?T`.
fn mark_array_shape_key_present(base_var: &str, key_name: &str, scope: &mut ScopeState) {
    let types = scope.get(base_var).to_vec();
    if types.is_empty() {
        return;
    }
    let narrowed: Vec<ResolvedType> = types
        .into_iter()
        .map(|mut rt| {
            rt.type_string = mark_shape_key_present(&rt.type_string, key_name);
            rt
        })
        .collect();
    scope.set(base_var, narrowed);
}

/// Recursively clear the `optional` flag on one key of an array shape.
fn mark_shape_key_present(ty: &crate::php_type::PhpType, key: &str) -> crate::php_type::PhpType {
    use crate::php_type::{PhpType, ShapeEntry, TypeKind};
    match ty.kind() {
        TypeKind::ArrayShape(entries) => {
            let new_entries: Vec<ShapeEntry> = entries
                .iter()
                .map(|e| {
                    if e.key.as_deref() == Some(key) {
                        ShapeEntry {
                            key: e.key.clone(),
                            value_type: e.value_type.clone(),
                            optional: false,
                        }
                    } else {
                        e.clone()
                    }
                })
                .collect();
            PhpType::array_shape(new_entries)
        }
        TypeKind::Nullable(inner) => PhpType::nullable(mark_shape_key_present(inner, key)),
        TypeKind::Union(members) => PhpType::union(
            members
                .iter()
                .map(|m| mark_shape_key_present(m, key))
                .collect(),
        ),
        other => other.clone().into(),
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
                // `apply_narrowing` only reaches the class layer, so a
                // needle with no class behind it comes back untouched.
                // Strict equality is a proof about the value, so failing
                // it rules out every alternative the haystack's elements
                // account for: `!in_array($doc, [null, ''], true)` leaves
                // a `?string` needle holding `string`.
                for rt in results.iter_mut() {
                    if rt.class_info.is_some() {
                        continue;
                    }
                    let mut narrowed = rt.type_string.clone();
                    for member in element_type.union_members() {
                        match strip_literal_from_type(&narrowed, member) {
                            Some(kept) => narrowed = kept,
                            // Excluding everything would leave the needle
                            // with no type at all, which says less than
                            // the type it came in with.
                            None => continue,
                        }
                    }
                    rt.type_string = narrowed;
                }
            } else {
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_inclusion(&element_type, false, &var_ctx, classes)
                });
                // `apply_narrowing` works on the class layer, so a needle
                // with no class behind it (`?string`, `int|string`) comes
                // back untouched.  Strict equality against the haystack's
                // elements is a proof about the value, so it narrows the
                // declared type too.
                for rt in results.iter_mut() {
                    if rt.class_info.is_none()
                        && let Some(narrowed) =
                            narrow_value_by_element(&rt.type_string, &element_type)
                    {
                        rt.type_string = narrowed;
                    }
                }
            }

            if !results.is_empty() {
                scope.set(var_name, results);
            }
        }
    }
}

/// The needle's type once a strict `in_array` has proved it equals one of
/// the haystack's elements, or `None` when that proves nothing new.
///
/// Every alternative the needle could hold that no element could equal is
/// gone: `?string` against a `list<string>` haystack keeps only `string`,
/// which is what makes the `if (!in_array(…)) { abort(); }` gate leave a
/// definite value behind it.  Where the elements are *narrower* than the
/// alternative they match, the alternative is replaced by them, so a
/// constant list of literals narrows a `string` needle to exactly the
/// values the list names.
///
/// A needle typed `mixed`, or a haystack whose element type is unknown,
/// proves nothing worth recording — narrowing `mixed` to the element type
/// would claim the haystack is exhaustive over a type nothing constrains.
fn narrow_value_by_element(needle: &PhpType, element: &PhpType) -> Option<PhpType> {
    if needle.is_mixed() || element.is_mixed() || needle.is_untyped() || element.is_untyped() {
        return None;
    }
    let element_members = element.union_members();
    let mut kept: Vec<PhpType> = Vec::new();
    for member in needle.union_members() {
        let matching: Vec<PhpType> = element_members
            .iter()
            .filter(|e| e.is_subtype_of(member))
            .map(|e| (*e).clone())
            .collect();
        if !matching.is_empty() {
            for m in matching {
                if !kept.contains(&m) {
                    kept.push(m);
                }
            }
        } else if member.is_subtype_of(element) && !kept.contains(member) {
            kept.push(member.clone());
        }
    }
    if kept.is_empty() {
        return None;
    }
    let narrowed = PhpType::union(kept);
    (narrowed != *needle).then_some(narrowed)
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
    // A constant list (`self::APPROVED`, `[Status::ACTIVE, …]`) resolves to
    // a shape rather than a parameterised array, and its elements are the
    // shape's values.
    raw_type.and_then(|t| t.iterable_element_type())
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

    // `$name === 'static' && $scope->isInClass()` proves both operands in
    // the truthy branch, so each one's own tags apply.  Written as one
    // expression the chain is not a call at all, and the extractors below
    // would find nothing to read.  The inverse path needs no equivalent:
    // `apply_condition_narrowing_inverse` does the De Morgan
    // decomposition and hands this function a single operand.
    if !inverted {
        let operands = collect_and_chain_operands(condition);
        if operands.len() > 1 {
            for operand in &operands {
                apply_phpstan_assert_condition_narrowing(operand, scope, ctx, inverted);
            }
            return;
        }
    }

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
                // The branch this tag does not name is reached by negating
                // what it promises, which only holds for the subtype form:
                // `-if-true Foo` failing means the value was not a `Foo`.
                // The equality form promises a comparison instead, and a
                // failed comparison rules nothing out, so it stays a one-way
                // implication.  Laravel's `filled()` carries
                // `@phpstan-assert-if-false !=numeric|bool`, and inverting
                // that made every filled value look like `numeric|bool`.
                if !applies_positively && assertion.is_equality {
                    continue;
                }
                if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                    &func_call.argument_list,
                    &assertion.param_name,
                    &func_info.parameters,
                ) {
                    let should_exclude = assertion.negated ^ !applies_positively;
                    apply_assertion_to_key(
                        &arg_var,
                        &assertion.asserted_type,
                        should_exclude,
                        scope,
                        ctx,
                        &scope_resolver,
                    );
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
            let (method, declaring_fqn) = match narrowing::find_assertion_method_in_chain(
                &class_info,
                &method_name,
                ctx.class_loader,
                &mut Vec::new(),
                0,
            ) {
                Some(found) => found,
                None => return,
            };
            let declaring_namespace = namespace_of_fqn(&declaring_fqn);
            for assertion in &method.type_assertions {
                let applies_positively = match assertion.kind {
                    AssertionKind::IfTrue => function_returned_true,
                    AssertionKind::IfFalse => !function_returned_true,
                    AssertionKind::Always => continue,
                };
                if !applies_positively && assertion.is_equality {
                    continue;
                }
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
                        qualify_assertion_type(
                            &assertion.asserted_type,
                            declaring_namespace.as_deref(),
                            ctx,
                        )
                    };
                    apply_assertion_to_key(
                        &arg_var,
                        &resolved_assert_type,
                        should_exclude,
                        scope,
                        ctx,
                        &scope_resolver,
                    );
                }
            }
        }
        Call::Method(method_call) => {
            // Instance method: `$var->method()` with `@phpstan-assert-if-true Type $this`.
            // The receiver is any subject the scope can key, not just a
            // bare local: `$this->scope->isInClass()` is the same promise
            // about the same value, and the guard is written that way
            // wherever the scope is held in a property.
            let Some(receiver_var) = narrowing::expr_to_subject_key(method_call.object) else {
                return;
            };
            let method_name = match &method_call.method {
                ClassLikeMemberSelector::Identifier(ident) => bytes_to_str(ident.value).to_string(),
                _ => return,
            };
            // A compound receiver is not a tracked local, so its type has
            // to be brought into the scope before it can be read.
            seed_synthetic_key_if_needed(&receiver_var, scope, ctx);
            // Resolve the receiver's type to find the method's assertions.
            let receiver_types = scope.get(&receiver_var).to_vec();
            if receiver_types.is_empty() {
                return;
            }
            // Collect assertions from all candidate classes.
            let mut to_apply: Vec<(crate::php_type::PhpType, bool, String)> = Vec::new();
            for rt in &receiver_types {
                // An intersection-typed receiver carries one entry per
                // member, so the class each entry names is looked up
                // rather than the joined `A&B` text, which is not a class.
                let receiver = match resolve_receiver_class(rt, ctx) {
                    Some(ci) => ci,
                    None => {
                        continue;
                    }
                };
                // Search the trait/parent chain for the method's assertions
                // using raw class loads only (a full merge would poison the
                // shared resolved-class cache mid-walk).
                let (method, declaring_fqn) = match narrowing::find_assertion_method_in_chain(
                    &receiver,
                    &method_name,
                    ctx.class_loader,
                    &mut Vec::new(),
                    0,
                ) {
                    Some(found) => found,
                    None => continue,
                };
                let declaring_namespace = namespace_of_fqn(&declaring_fqn);
                for assertion in &method.type_assertions {
                    let applies_positively = match assertion.kind {
                        AssertionKind::IfTrue => function_returned_true,
                        AssertionKind::IfFalse => !function_returned_true,
                        AssertionKind::Always => continue,
                    };
                    if !applies_positively && assertion.is_equality {
                        continue;
                    }
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
                        qualify_assertion_type(
                            &assertion.asserted_type,
                            declaring_namespace.as_deref(),
                            ctx,
                        )
                    };
                    if assertion.param_name == "$this" {
                        // Narrows the receiver variable itself.
                        to_apply.push((resolved_type, should_exclude, receiver_var.clone()));
                    } else if let Some(member) = assertion.param_name.strip_prefix("$this->") {
                        // `@phpstan-assert-if-true !null
                        // $this->getTraitReflection()` on `isInTrait()` is a
                        // promise about a member read off the *receiver*, so
                        // at the call site the subject is that same read
                        // through the variable the call was written on.
                        to_apply.push((
                            resolved_type,
                            should_exclude,
                            format!("{receiver_var}->{member}"),
                        ));
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
                apply_assertion_to_key(
                    &target_var,
                    &asserted_type,
                    should_exclude,
                    scope,
                    ctx,
                    &scope_resolver,
                );
            }
        }
        _ => {}
    }
}

/// The class a resolved receiver entry stands for, for looking up the
/// method whose docblock carries the assertion.
///
/// The entry's own `class_info` is the authority when it has one.  An
/// entry that only carries a type string is looked up by that text,
/// except that an intersection (`A&B`) is not a class name — each member
/// is tried in turn, since the assertion may be declared on any of them.
fn resolve_receiver_class(
    rt: &ResolvedType,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<Arc<crate::types::ClassInfo>> {
    if let Some(ci) = &rt.class_info {
        return Some(Arc::clone(ci));
    }
    if let TypeKind::Intersection(members) = rt.type_string.kind() {
        return members
            .iter()
            .find_map(|member| (ctx.class_loader)(&member.to_string()));
    }
    (ctx.class_loader)(&rt.type_string.to_string())
}

/// Qualify the unqualified class names in an assertion's type against
/// the namespace of the file that declared the tag.
///
/// `@phpstan-assert-if-true TestMethod $this` on an interface in
/// `PHPUnit\Event\Code` names `PHPUnit\Event\Code\TestMethod`, the way
/// PHP resolves every other unqualified name in that file.  The call
/// site's namespace has nothing to do with it, and the short-name index
/// that would otherwise find the class covers the project's own files
/// only — so a tag a vendor package declares on itself resolved to
/// nothing at all.
///
/// A name that does not resolve to a class under the declaring namespace
/// is left alone, so an already-qualified name and the short-name
/// fallback both keep working.
fn qualify_assertion_type(
    asserted: &PhpType,
    declaring_namespace: Option<&str>,
    ctx: &ForwardWalkCtx<'_>,
) -> PhpType {
    let Some(namespace) = declaring_namespace.filter(|ns| !ns.is_empty()) else {
        return asserted.clone();
    };
    asserted.resolve_names(&|name| {
        if name.contains('\\') {
            return name.to_string();
        }
        let qualified = format!("{}\\{}", namespace, name);
        if (ctx.class_loader)(&qualified).is_some() {
            qualified
        } else {
            name.to_string()
        }
    })
}

/// The namespace part of a fully-qualified class name, or `None` for a
/// class in the global namespace.
fn namespace_of_fqn(fqn: &str) -> Option<String> {
    let trimmed = fqn.trim_start_matches('\\');
    trimmed
        .rfind('\\')
        .map(|pos| trimmed[..pos].to_string())
        .filter(|ns| !ns.is_empty())
}

/// Apply one `@phpstan-assert-if-true` / `-if-false` conclusion to the
/// scope entry for `target`.
///
/// `target` is a scope key rather than a plain variable name: an
/// assertion whose subject is written `$this->getClassReflection()`
/// resolves to a member path off the receiver, which the scope tracks
/// under its own key once it has been seeded.
///
/// A class-named assertion narrows through the `instanceof` machinery. A
/// scalar or pseudo-type one (`!null`, `string`, `array` — PHPUnit's
/// `assertIsString` and every `!null` promise) names no class at all, so
/// that machinery would exclude nothing and include nothing; those are
/// routed through the same type guards the matching `is_*()` check uses.
fn apply_assertion_to_key(
    target: &str,
    asserted_type: &PhpType,
    should_exclude: bool,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    scope_resolver: &dyn Fn(&str) -> Vec<ResolvedType>,
) {
    seed_synthetic_key_if_needed(target, scope, ctx);
    let mut results = scope.get(target).to_vec();
    if results.is_empty() {
        return;
    }

    if let Some(kind) = narrowing::scalar_assert_guard_kind(asserted_type) {
        if should_exclude {
            narrowing::apply_type_guard_exclusion(kind, &mut results, Some(ctx.class_loader));
        } else {
            narrowing::apply_type_guard_inclusion(kind, &mut results, Some(ctx.class_loader));
        }
    } else if should_exclude && matches!(asserted_type.kind(), TypeKind::Union(_)) {
        // Ruling out a union rules out every member, so each one narrows on
        // its own.  That is what lets `!=null|''` (Laravel's `filled()`)
        // strip the null: the whole union names no class, so handing it to
        // the class machinery below resolved nothing and narrowed nothing.
        //
        // Only exclusion decomposes this way.  Narrowing *to* each member in
        // turn would leave the subject as the last member alone rather than
        // the union, so an included union stays whole.
        let var_ctx = build_var_ctx(target, ctx, scope_resolver);
        for member in asserted_type.union_members() {
            if let Some(kind) = narrowing::scalar_assert_guard_kind(member) {
                narrowing::apply_type_guard_exclusion(kind, &mut results, Some(ctx.class_loader));
            } else {
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_exclusion(member, &var_ctx, classes)
                });
            }
        }
    } else {
        let var_ctx = build_var_ctx(target, ctx, scope_resolver);
        if should_exclude {
            ResolvedType::apply_narrowing(&mut results, |classes| {
                narrowing::apply_instanceof_exclusion(asserted_type, &var_ctx, classes)
            });
        } else {
            ResolvedType::apply_narrowing(&mut results, |classes| {
                narrowing::apply_instanceof_inclusion(asserted_type, false, &var_ctx, classes)
            });
        }
    }

    if !results.is_empty() {
        scope.set(target, results);
    }
}

/// Build a [`VarResolutionCtx`] from a variable name and forward-walk context.
///
/// Shared helper used by the narrowing functions in this module to avoid
/// repeating the struct construction at every call site.  It carries no
/// scope proofs: the callers use it to resolve the class names a check
/// mentions, which is a question about declarations rather than about
/// what the values flowing through the scope have been proven to be.
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
        scope_proofs: None,
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
    ctx: &ForwardWalkCtx<'_>,
) {
    apply_type_guard_on_operands(condition, scope, true, ctx);
}

/// Apply type-guard narrowing in the inverse (else) branch.
///
/// When `is_object($var)` appears in a condition, the else branch
/// knows the variable is NOT an object — filter out object-like
/// members from the union type.
pub(crate) fn apply_type_guard_narrowing_inverse(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    apply_type_guard_on_operands(condition, scope, false, ctx);
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
    ctx: &ForwardWalkCtx<'_>,
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
                    narrowing::apply_type_guard_inclusion(
                        kind,
                        &mut results,
                        Some(ctx.class_loader),
                    );
                } else {
                    narrowing::apply_type_guard_exclusion(
                        kind,
                        &mut results,
                        Some(ctx.class_loader),
                    );
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
    fn admits(
        &self,
        prop_type: &PhpType,
        class_loader: &dyn Fn(&str) -> Option<Arc<crate::types::ClassInfo>>,
    ) -> bool {
        match self {
            PropertyTest::Guard { kind, expect_match } => narrowing::guard_outcome_possible(
                prop_type,
                *kind,
                *expect_match,
                Some(class_loader),
            ),
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
            .is_none_or(|hint| check.test.admits(&hint, ctx.class_loader)),
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
    // `$x !== ''` / `$x !== []` — refine to the non-empty counterpart, and
    // `$x === ''` to the empty one.
    if let Some((var_name, empty, non_empty)) = extract_empty_value_check(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        if non_empty {
            refine_non_empty_in_scope(&var_name, empty, scope);
        } else {
            refine_empty_in_scope(&var_name, empty, scope);
        }
    }
    // `count($x) > 0` — the counted subject has entries, which is what a
    // `foreach` guarded this way reads to know its body runs.  `count($x)
    // === 0` says the subject is the empty array.
    if let Some((var_name, non_empty, _)) = extract_count_emptiness_check(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        if non_empty {
            refine_non_empty_in_scope(&var_name, EmptyValue::Array, scope);
        } else {
            refine_empty_in_scope(&var_name, EmptyValue::Array, scope);
        }
    }
    // `$x === 0` / `$x !== []` and the rest of the strict comparisons
    // against a written-out value: the equal branch holds that value, the
    // unequal one holds everything else the subject could be.
    apply_literal_identity_narrowing(condition, scope, ctx, true);
    // `$x === Land::Be` — the subject holds whatever the constant holds,
    // so a constant that cannot be null leaves no null in the subject.
    if let Some((var_name, constant)) = extract_class_constant_identity(condition, true) {
        strip_null_by_constant_identity(&var_name, constant, scope, ctx);
    }
    // `$x === $y` — the same reasoning for any comparand whose own type
    // rules out null.
    apply_identity_comparison_null_narrowing(condition, scope, ctx, true);
    // `!empty($x)` — truthy branch means $x is non-empty (truthy):
    // strip null and false from the type.
    if let Some(var_name) = extract_not_empty_var(condition) {
        // `!empty($arr['key'])` says of the key what `isset` does — it is
        // there — so the shape drops its `?` as well as the falsy half of
        // its value type.
        if let Some((base, key)) = split_array_access_key(&var_name) {
            strip_null_from_array_shape_key(base, key, scope);
        }
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
    // $x is truthy — remove every falsy member, not just `null`.  This
    // is the same proof the fall-through of `if (!$x) { return; }`
    // carries, so it strips the same set: an else branch that only lost
    // `null` leaves `false` in a `string|false` union to reappear at the
    // merge, undoing a reassignment the taken branch made to repair it.
    if let Some(var_name) = extract_falsy_check_var(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        strip_falsy_from_scope(&var_name, scope);
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
    // The else branch of a strict comparison against a written-out value
    // establishes the opposite of what the body did.
    apply_literal_identity_narrowing(condition, scope, ctx, false);
    // When the condition is `$x !== Land::Be`, the inverse (else/guard)
    // means the subject is that constant, so it holds whatever the
    // constant holds.
    if let Some((var_name, constant)) = extract_class_constant_identity(condition, false) {
        strip_null_by_constant_identity(&var_name, constant, scope, ctx);
    }
    // When the condition is `$x !== $y`, the inverse (else/guard) means
    // the two were identical, so a comparand that cannot be null leaves
    // no null in the subject.
    apply_identity_comparison_null_narrowing(condition, scope, ctx, false);
    // When the condition is `$x === ''` / `$x === []`, the inverse
    // (else/guard) means $x is non-empty; when it is `$x !== ''`, the
    // inverse means $x is exactly the empty value.
    if let Some((var_name, empty, non_empty)) = extract_empty_value_check(condition) {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        if non_empty {
            refine_empty_in_scope(&var_name, empty, scope);
        } else {
            refine_non_empty_in_scope(&var_name, empty, scope);
        }
    }
    // When the condition is `count($x) === 0`, the inverse (else, or the
    // fall-through of a guard that threw) means the subject has entries;
    // when it is `count($x) > 0`, the inverse means it has none.  A bound
    // further out (`count($x) > 1`) proves neither on the way past.
    if let Some((var_name, non_empty, complement_exact)) = extract_count_emptiness_check(condition)
        && complement_exact
    {
        seed_synthetic_key_if_needed(&var_name, scope, ctx);
        if non_empty {
            refine_empty_in_scope(&var_name, EmptyValue::Array, scope);
        } else {
            refine_non_empty_in_scope(&var_name, EmptyValue::Array, scope);
        }
    }
    // When the condition is a bare `$x` (truthy check), the inverse means
    // $x is falsy: every member that could not have been drops out, which
    // is what leaves `while ($a) { … }` with a `null` below it.  A `bool`
    // keeps its `false` half rather than staying whole, and that is what
    // lets a branch the flag guards be recognised again where the flag is
    // re-tested.
    if let Some(var_name) = expr_to_var_name(condition) {
        narrow_to_falsy_in_scope(&var_name, scope);
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

    let mut keys: Vec<String> = Vec::new();
    for expr in proven {
        for key in nullsafe_receiver_keys(expr) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }

    // A chain compared identical to a value that cannot be null held a
    // non-null value itself, which is the same proof about its receivers.
    let mut compared: Vec<(&Expression<'_>, &Expression<'_>)> = Vec::new();
    collect_identity_comparisons(condition, truthy, &mut compared);
    for (chain, comparand) in compared {
        let chain_keys: Vec<String> = nullsafe_receiver_keys(chain)
            .into_iter()
            .filter(|key| !keys.contains(key))
            .collect();
        // Resolving the comparand is the expensive half, so it happens
        // only once the cheap half has found receivers still to prove.
        if chain_keys.is_empty() || expr_accepts_null(comparand, scope, ctx) {
            continue;
        }
        keys.extend(chain_keys);
    }

    for key in keys {
        seed_synthetic_key_if_needed(&key, scope, ctx);
        strip_null_from_scope(&key, scope);
    }
}

/// Strip `null` from a subject that an identity comparison has matched
/// against a value that cannot be `null`.
///
/// `$a === $b` holding means both sides carried the same value, so a
/// nullable `$a` compared identical to a definitely-non-null `$b` holds
/// no null in that branch:
///
/// ```php
/// $name = $context->getName(); // ?string
/// if ($name === $node->name) {  // $node->name is string
///     takesString($name);       // string
/// }
/// ```
///
/// Only identity qualifies: `null == 0` and `null == false` are both
/// true, so a loose comparison proves nothing.
fn apply_identity_comparison_null_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    truthy: bool,
) {
    let mut compared: Vec<(&Expression<'_>, &Expression<'_>)> = Vec::new();
    collect_identity_comparisons(condition, truthy, &mut compared);

    for (subject, comparand) in compared {
        let Some(key) =
            expr_to_var_name(subject).or_else(|| narrowing::expr_to_subject_key(subject))
        else {
            continue;
        };
        // The cheap half first: with no null to rule out there is
        // nothing to narrow, and resolving the comparand costs a full
        // type resolution.
        if !scope_value_is_nullable(&key, scope) || expr_accepts_null(comparand, scope, ctx) {
            continue;
        }
        strip_null_from_scope(&key, scope);
    }
}

/// Whether the value `expr` evaluates to could be `null`.
///
/// An expression that resolves to nothing counts as nullable: an unknown
/// value is not a proof.
fn expr_accepts_null(expr: &Expression<'_>, scope: &ScopeState, ctx: &ForwardWalkCtx<'_>) -> bool {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(&atom(vn)).cloned().unwrap_or_default()
    };
    let var_ctx = build_var_ctx("", ctx, &scope_resolver);
    crate::type_engine::variable::resolution::resolve_arg_raw_type(expr, &var_ctx)
        .is_none_or(|ty| ty.accepts_null())
}

/// Collect the `(subject, comparand)` pairs of every identity comparison
/// the condition proves held, in both operand orders.
///
/// Whichever side a caller cares about, the identity holding means both
/// sides carried the same value, so a proof about one is a proof about
/// the other.  `A === B` holding and `A !== B` failing are the same
/// proof, which is why the `truthy` flag flips for `!==`.
fn collect_identity_comparisons<'b>(
    condition: &'b Expression<'b>,
    truthy: bool,
    out: &mut Vec<(&'b Expression<'b>, &'b Expression<'b>)>,
) {
    match condition {
        Expression::Parenthesized(inner) => {
            collect_identity_comparisons(inner.expression, truthy, out);
        }
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            collect_identity_comparisons(prefix.operand, !truthy, out);
        }
        Expression::Binary(bin) => {
            // `A && B` proves both when true; `A || B` proves neither
            // operand held when false.
            let decomposes = match bin.operator {
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_) => truthy,
                BinaryOperator::Or(_) | BinaryOperator::LowOr(_) => !truthy,
                _ => false,
            };
            if decomposes {
                collect_identity_comparisons(bin.lhs, truthy, out);
                collect_identity_comparisons(bin.rhs, truthy, out);
                return;
            }
            // Only identity qualifies: `null == false` and `null == 0` are
            // both true, so a loose comparison proves nothing.
            let holds = match bin.operator {
                BinaryOperator::Identical(_) => truthy,
                BinaryOperator::NotIdentical(_) => !truthy,
                _ => false,
            };
            if holds {
                out.push((bin.lhs, bin.rhs));
                out.push((bin.rhs, bin.lhs));
            }
        }
        _ => {}
    }
}

/// The scope keys of every receiver a nullsafe chain short-circuits on,
/// outermost first.  Empty when `expr` holds no `?->` link.
///
/// The walk peels plain `->` links too, because a `?->` short-circuits the
/// rest of the chain written after it: `$a?->b()->c()` is `null` whenever
/// `$a` is, so a proof about the whole chain is a proof about `$a`.  Only
/// the `?->` receivers are recorded — a plain link's receiver is one the
/// chain would have thrown on, not short-circuited.
fn nullsafe_receiver_keys(expr: &Expression<'_>) -> Vec<String> {
    let mut keys = Vec::new();
    let mut node = expr;
    while let Some((receiver, nullsafe)) = chain_receiver(node) {
        if nullsafe && let Some(key) = narrowing::expr_to_subject_key(receiver) {
            keys.push(key);
        }
        node = receiver;
    }
    keys
}

/// Record what the expression assigned to `lhs_name` proves about the
/// other keys it read, for a guard that arrives later and only names the
/// result.
///
/// Two shapes carry such a proof. A `?->` chain's null stands for its
/// receivers': `$period = $agreement?->latestPeriod();` followed by
/// `if (!$period instanceof Period) { return; }` proves `$agreement` is
/// not null past the guard — the chain would have short-circuited to
/// `null` otherwise. And a plain copy of a member path holds the very
/// same value, so `$cacheKey = $this->cacheKey;` makes a later
/// `$cacheKey !== null` a proof about `$this->cacheKey` too. Either way
/// the guard's condition never names the other key, so the link has to be
/// recorded where it is written.
///
/// Nothing is recorded when the assigned value is not nullable: without a
/// `null` to rule out, "not null" is not evidence that any guard ran.
pub(crate) fn record_nullsafe_origin<'b>(
    lhs_name: &str,
    rhs: &'b Expression<'b>,
    scope: &mut ScopeState,
) {
    if !scope_value_is_nullable(lhs_name, scope) {
        return;
    }
    let mut implied = nullsafe_receiver_keys(rhs);
    // A copy reads the value; a call only reads the receiver, and what it
    // returns is its own value, not the receiver's.
    if let Some(key) = narrowing::expr_to_subject_key(rhs)
        && narrowing::is_member_path_key(&key)
        && !narrowing::is_call_key(&key)
    {
        implied.push(key);
    }
    // A path rooted at the variable being written names a different value
    // after the write than it did before it: `$a = $a->a;` leaves `$a->a`
    // meaning the *new* `$a`'s property, which the assignment proves
    // nothing about. Recording it would undo the invalidation the write
    // just performed.
    let implied: Vec<Atom> = implied
        .iter()
        .filter(|key| key.as_str() != lhs_name && !narrowing::key_reads_variable(key, lhs_name))
        .map(|key| atom(key))
        .collect();
    scope.record_non_null_implication(lhs_name, implied);
}

/// Carry a proof about one value's null back to every value whose null it
/// stands for: the receivers a `?->` chain would have short-circuited on,
/// and the variables a branch wrote alongside it.
///
/// Runs after the condition's own narrowing has landed, so what it reads
/// is the guarded state: a holder that is no longer nullable is one the
/// condition ruled the null out of, whichever shape the guard was written
/// in (`instanceof`, `!== null`, a bare truthy test, an assertion helper).
fn apply_non_null_implication_narrowing(scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    if !scope.non_null_implications.is_empty() {
        let proven: Vec<Atom> = scope
            .non_null_implications
            .iter()
            .filter(|(holder, _)| !scope_value_is_nullable(holder, scope))
            .flat_map(|(_, implieds)| implieds.iter().copied())
            .collect();
        for implied in proven {
            seed_synthetic_key_if_needed(&implied, scope, ctx);
            strip_null_from_scope(&implied, scope);
        }
    }

    if scope.implied_narrowings.is_empty() {
        return;
    }
    let proven: Vec<(Atom, Vec<ResolvedType>)> = scope
        .implied_narrowings
        .iter()
        .flat_map(|(holder, proofs)| {
            proofs
                .iter()
                .filter(|proof| trigger_holds(proof, holder, scope))
                .map(|proof| (proof.key, proof.types.clone()))
        })
        .collect();
    for (key, types) in proven {
        seed_synthetic_key_if_needed(&key, scope, ctx);
        if recorded_narrows_current(&types, scope.get(&key), ctx) {
            scope.set(&key, types);
        }
    }
}

/// Whether the scope now shows the holder to have taken the branch that
/// recorded `proof`.
///
/// A [`ProofTrigger::NonNull`] asks only that the holder's `null` is gone,
/// which is what a `!== null` test below the join establishes. A
/// [`ProofTrigger::Within`] asks that every type the holder can still be
/// is one the branch's own value could have been — the join only recorded
/// the proof because a holder inside that value cannot have come down the
/// other path. A [`ProofTrigger::Outside`] is the complement: nothing the
/// holder can still be is a value the *other* path left, so that path is
/// the one that did not run.
fn trigger_holds(proof: &ImpliedNarrowing, holder: &Atom, scope: &ScopeState) -> bool {
    let held = scope.get(holder);
    match &proof.trigger {
        ProofTrigger::NonNull => !scope_value_is_nullable(holder, scope),
        ProofTrigger::Within(trigger) => {
            !held.is_empty()
                && held.iter().all(|current| {
                    trigger
                        .iter()
                        .any(|want| current.type_string.is_subtype_of(&want.type_string))
                })
        }
        // Conservatively: two classes only count as contradicting each
        // other when the loader has been consulted, which
        // `types_are_disjoint` declines to guess at.  An `A` that is not
        // *spelled* `B` may still be one.
        ProofTrigger::Outside(trigger) => types_are_disjoint(held, trigger),
    }
}

/// Whether a recorded proof still refines what the scope knows.
///
/// The proof describes the value the key held on the branch that ran, so
/// it can only ever add to what is known — never replace it.  A guard
/// closer to the read has already narrowed further (the `&&` chain that
/// re-proves `$node->dim instanceof FuncCall` right where it is used),
/// and overwriting that with the coarser branch type would lose the more
/// specific answer.  An entry with no type is the one exception: unknown
/// is the top of the lattice, so anything recorded is narrower.
///
/// Which is also why a recorded `mixed` refines nothing.  It sits at the
/// same top of the lattice a missing entry does, so a branch that left the
/// key `mixed` beside a class the scope also holds says nothing the scope
/// does not already say — and applying it would drop that class, which is
/// the half every member lookup needs.
fn recorded_narrows_current(
    recorded: &[ResolvedType],
    current: &[ResolvedType],
    ctx: &ForwardWalkCtx<'_>,
) -> bool {
    if recorded.iter().any(|r| r.type_string.is_mixed()) {
        return false;
    }
    if current.is_empty() {
        return true;
    }
    let within = |narrow: &ResolvedType, wide: &ResolvedType| {
        if narrow.type_string == wide.type_string
            || narrow.type_string.is_subtype_of(&wide.type_string)
        {
            return true;
        }
        match (
            narrow.type_string.unwrap_nullable().class_name(),
            wide.type_string.unwrap_nullable().class_name(),
        ) {
            (Some(child), Some(parent)) => is_subclass_of(child, parent, ctx.class_loader),
            _ => false,
        }
    };
    let identical = recorded.len() == current.len()
        && recorded
            .iter()
            .all(|r| current.iter().any(|c| c.type_string == r.type_string));
    !identical
        && recorded
            .iter()
            .all(|r| current.iter().any(|c| within(r, c)))
}

/// Whether the scope's entry for `key` still admits `null`.
///
/// An entry with no types admits everything, so it counts as nullable:
/// an unknown value is not a proof.
fn scope_value_is_nullable(key: &str, scope: &ScopeState) -> bool {
    let types = scope.get(key);
    types.is_empty()
        || types
            .iter()
            .any(|rt| rt.type_string.non_null_type().is_some() || rt.type_string == PhpType::null())
}

/// The receiver one link of a member chain is applied to, paired with
/// whether that link is the nullsafe `?->`.
fn chain_receiver<'b>(expr: &'b Expression<'b>) -> Option<(&'b Expression<'b>, bool)> {
    match expr {
        Expression::Parenthesized(inner) => chain_receiver(inner.expression),
        Expression::Access(Access::NullSafeProperty(pa)) => Some((pa.object, true)),
        Expression::Call(Call::NullSafeMethod(mc)) => Some((mc.object, true)),
        Expression::Access(Access::Property(pa)) => Some((pa.object, false)),
        Expression::Call(Call::Method(mc)) => Some((mc.object, false)),
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

/// Extract the subject of an identity comparison against a class
/// constant, paired with the constant expression itself.
///
/// `proves_equal` selects which polarity the caller is narrowing: `true`
/// for the branch where the subject *is* the constant (the truthy side of
/// `$x === C`, the guard fall-through of `$x !== C`), `false` for the
/// branch where it is not.
///
/// An enum case is the case that matters — `$land === Land::Be` is how
/// enum code is written — but every class constant carries the same
/// proof, so the constant's own type decides what the comparison rules
/// out rather than the syntax.
fn extract_class_constant_identity<'b>(
    expr: &'b Expression<'b>,
    proves_equal: bool,
) -> Option<(String, &'b Expression<'b>)> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    let Expression::Binary(bin) = inner else {
        return None;
    };
    let identical = match bin.operator {
        BinaryOperator::Identical(_) => true,
        BinaryOperator::NotIdentical(_) => false,
        _ => return None,
    };
    // Whether "subject is the constant" is what holds when the written
    // condition is true; the caller says which of the two branches it is
    // narrowing.
    if (identical != negated) != proves_equal {
        return None;
    }
    for (candidate, other) in [(bin.rhs, bin.lhs), (bin.lhs, bin.rhs)] {
        if !matches!(candidate, Expression::Access(Access::ClassConstant(_))) {
            continue;
        }
        if let Some(name) =
            expr_to_var_name(other).or_else(|| narrowing::expr_to_subject_key(other))
        {
            return Some((name, candidate));
        }
    }
    None
}

/// Strip `null` from `var_name` when the constant it was proven identical
/// to cannot be null.
///
/// Identity is only ever true between two values of the same type, so a
/// constant that holds no null leaves none in the subject. A constant
/// that does — `const NONE = null;`, or one whose type we cannot read —
/// proves nothing and is left alone.
fn strip_null_by_constant_identity(
    var_name: &str,
    constant: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    if scope.get(var_name).is_empty() {
        seed_synthetic_key_if_needed(var_name, scope, ctx);
        if scope.get(var_name).is_empty() {
            return;
        }
    }
    let constant_types = super::assignment::resolve_rhs_with_scope(constant, scope, ctx);
    if constant_types.is_empty() {
        return;
    }
    if constant_types
        .iter()
        .any(|rt| type_admits_null(&rt.type_string))
    {
        return;
    }
    strip_null_from_scope(var_name, scope);
}

/// Whether a type has a `null` among the values it describes.
fn type_admits_null(ty: &PhpType) -> bool {
    match ty.kind() {
        TypeKind::Nullable(_) => true,
        TypeKind::Union(members) => members.iter().any(type_admits_null),
        _ => ty.is_null() || ty.is_mixed(),
    }
}

/// Extract variable name from `!empty($x)` (negated empty check).
///
/// A member path or an array offset is as much a subject as a bare variable,
/// the same way it is for `isset($x)` and `empty($x)`: `!empty($row['name'])`
/// proves that entry is there and truthy.
pub(crate) fn extract_not_empty_var(expr: &Expression<'_>) -> Option<String> {
    if let Expression::UnaryPrefix(prefix) = expr
        && prefix.operator.is_not()
        && let Expression::Construct(Construct::Empty(empty)) = prefix.operand
    {
        return expr_to_var_name(empty.value)
            .or_else(|| narrowing::expr_to_subject_key(empty.value));
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

/// Extract the subject of a `count()`/`sizeof()` comparison against an
/// integer literal, plus whether the condition holds exactly when the
/// subject has entries.
///
/// `count($x) > 0`, `count($x) !== 0` and `count($x) >= 1` all prove the
/// subject is non-empty; `count($x) === 0` and `count($x) < 1` prove the
/// opposite, which is what the inverse branch of the same `if` (or the
/// fall-through of `if (count($x) === 0) { throw; }`) reads. A bound that
/// neither proves — `count($x) < 5` says nothing, and `count($x) > -1` is
/// vacuous — yields `None`.
///
/// The second flag says whether the *other* branch proves the opposite.
/// `count($x) > 1` proves the subject non-empty where it holds, but
/// falling through it leaves one entry exactly as possible as none, so
/// nothing may be concluded there.
///
/// The literal may be written on either side, and a leading `!` flips the
/// answer.
fn extract_count_emptiness_check(expr: &Expression<'_>) -> Option<(String, bool, bool)> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    let Expression::Binary(bin) = inner else {
        return None;
    };

    // `count($x) OP n`, or `n OP count($x)` with the comparison flipped so
    // the subject is always on the left.
    let (subject, comparison, bound) =
        match (count_call_subject(bin.lhs), count_call_subject(bin.rhs)) {
            (Some(subject), _) => (
                subject,
                Comparison::of(&bin.operator)?,
                count_bound(bin.rhs)?,
            ),
            (_, Some(subject)) => (
                subject,
                Comparison::of(&bin.operator)?.flipped(),
                count_bound(bin.lhs)?,
            ),
            _ => return None,
        };

    // `count()` never returns a negative number, so a bound below zero
    // makes the comparison say nothing about the subject either way.
    let non_empty = match comparison {
        Comparison::Greater if bound >= 0 => true,
        Comparison::GreaterOrEqual if bound >= 1 => true,
        Comparison::NotEqual if bound == 0 => true,
        Comparison::Equal if bound == 0 => false,
        Comparison::Less if bound == 1 => false,
        Comparison::LessOrEqual if bound == 0 => false,
        _ => return None,
    };

    // The comparison splits emptiness in two only when its boundary sits
    // at "has at least one entry".  Every arm that proves emptiness does;
    // of the ones that prove the opposite, only the three that mean
    // `count($x) >= 1` do.
    let complement_exact = !non_empty
        || matches!(
            (comparison, bound),
            (Comparison::Greater, 0) | (Comparison::GreaterOrEqual, 1) | (Comparison::NotEqual, 0)
        );

    Some((subject, non_empty != negated, complement_exact))
}

/// The comparison a `count()` check is written with, reduced to the six
/// orderings so the subject can be moved to the left of it.
#[derive(Clone, Copy)]
enum Comparison {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
}

impl Comparison {
    fn of(operator: &BinaryOperator<'_>) -> Option<Comparison> {
        Some(match operator {
            BinaryOperator::LessThan(_) => Comparison::Less,
            BinaryOperator::LessThanOrEqual(_) => Comparison::LessOrEqual,
            BinaryOperator::GreaterThan(_) => Comparison::Greater,
            BinaryOperator::GreaterThanOrEqual(_) => Comparison::GreaterOrEqual,
            BinaryOperator::Equal(_) | BinaryOperator::Identical(_) => Comparison::Equal,
            BinaryOperator::NotEqual(_) | BinaryOperator::NotIdentical(_) => Comparison::NotEqual,
            _ => return None,
        })
    }

    /// The comparison that holds when its two operands swap places.
    fn flipped(self) -> Comparison {
        match self {
            Comparison::Less => Comparison::Greater,
            Comparison::LessOrEqual => Comparison::GreaterOrEqual,
            Comparison::Greater => Comparison::Less,
            Comparison::GreaterOrEqual => Comparison::LessOrEqual,
            Comparison::Equal => Comparison::Equal,
            Comparison::NotEqual => Comparison::NotEqual,
        }
    }
}

/// The subject of a `count($x)`/`sizeof($x)` call, as a narrowing key.
fn count_call_subject(expr: &Expression<'_>) -> Option<String> {
    let Expression::Call(Call::Function(call)) = unwrap_parens(expr) else {
        return None;
    };
    let Expression::Identifier(ident) = call.function else {
        return None;
    };
    let name = crate::util::strip_fqn_prefix(bytes_to_str(ident.value())).to_ascii_lowercase();
    if name != "count" && name != "sizeof" {
        return None;
    }
    // `count($x, COUNT_RECURSIVE)` still counts the top level's entries,
    // so the second argument does not change what a zero/non-zero result
    // proves about the subject.
    let first = call.argument_list.arguments.first()?;
    let arg = match first {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    };
    expr_to_var_name(arg).or_else(|| narrowing::expr_to_subject_key(arg))
}

/// The bound a `count()` comparison is written against: a plain decimal
/// integer literal, optionally negated. A hexadecimal, octal, or
/// separator-laden literal is not worth decoding for the handful of bounds
/// this reads.
fn count_bound(expr: &Expression<'_>) -> Option<i64> {
    match unwrap_parens(expr) {
        Expression::Literal(Literal::Integer(lit)) => bytes_to_str(lit.raw).parse().ok(),
        Expression::UnaryPrefix(prefix)
            if matches!(prefix.operator, UnaryPrefixOperator::Negation(_)) =>
        {
            count_bound(prefix.operand).map(|value| -value)
        }
        _ => None,
    }
}

/// Extract the subject of a strict comparison against a literal value:
/// `$x === 0`, `0.0 === $x`, `$x !== []`, and their negations.
///
/// Returns the subject key, the literal's own type, and whether the
/// branch being narrowed is the one where the two were equal.
///
/// Only `===`/`!==` are read. The loose operators compare across types
/// (`0 == ''` changed meaning in PHP 8), so they do not pin the subject
/// to the literal's type the way strict identity does.
pub(crate) fn extract_literal_identity_check(
    expr: &Expression<'_>,
) -> Option<(String, PhpType, bool)> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    let Expression::Binary(bin) = inner else {
        return None;
    };
    let equal = match bin.operator {
        BinaryOperator::Identical(_) => !negated,
        BinaryOperator::NotIdentical(_) => negated,
        _ => return None,
    };
    let (subject, literal) = match (
        literal_comparand_type(bin.rhs),
        literal_comparand_type(bin.lhs),
    ) {
        (Some(ty), _) => (bin.lhs, ty),
        (_, Some(ty)) => (bin.rhs, ty),
        _ => return None,
    };
    let name = expr_to_var_name(subject).or_else(|| narrowing::expr_to_subject_key(subject))?;
    Some((name, literal, equal))
}

/// The type of the literal an expression writes, for the comparands that
/// pin a subject to one exact value.
///
/// A `-1` is a unary minus over a literal rather than a literal of its
/// own, so the sign is folded back in; anything else that is not written
/// out as a value in the source has no literal type.
fn literal_comparand_type(expr: &Expression<'_>) -> Option<PhpType> {
    match expr {
        Expression::Parenthesized(paren) => literal_comparand_type(paren.expression),
        Expression::UnaryPrefix(prefix) => {
            let negated = match prefix.operator {
                UnaryPrefixOperator::Negation(_) => true,
                UnaryPrefixOperator::Plus(_) => false,
                _ => return None,
            };
            let inner = literal_comparand_type(prefix.operand)?;
            if !negated {
                return Some(inner);
            }
            match inner.as_literal()? {
                LiteralValue::Int(raw) => Some(PhpType::literal_int(format!("-{raw}"))),
                LiteralValue::Float(raw) => Some(PhpType::literal_float(format!("-{raw}"))),
                _ => None,
            }
        }
        Expression::Literal(Literal::Integer(int)) => int
            .value
            .map(|value| PhpType::literal_int(value.to_string())),
        Expression::Literal(Literal::Float(float)) => {
            Some(PhpType::literal_float(float.value.into_inner().to_string()))
        }
        Expression::Literal(Literal::String(string)) => string
            .value
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(PhpType::literal_string_value),
        Expression::Literal(Literal::True(_)) => Some(PhpType::true_()),
        Expression::Literal(Literal::False(_)) => Some(PhpType::false_()),
        _ if is_null_expr(expr) => Some(PhpType::null()),
        Expression::Array(array) => array.elements.is_empty().then(|| PhpType::parse("array{}")),
        Expression::LegacyArray(array) => {
            array.elements.is_empty().then(|| PhpType::parse("array{}"))
        }
        _ => None,
    }
}

/// Apply what a strict comparison against a literal proves about the
/// subject, in whichever direction the branch establishes.
///
/// The equal branch pins the subject to the literal, but only when the
/// type it carries has room for it: a comparison no alternative could
/// satisfy describes a branch that cannot run, which is not this
/// function's business to decide. The unequal branch drops every
/// alternative the literal covers, which is what lets a discriminant
/// (`if ($this->state === 'notLoaded') { … }`) leave its sentinel behind
/// in the branch that ruled it out.
fn apply_literal_identity_narrowing(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    truthy: bool,
) {
    let Some((var_name, literal, equal)) = extract_literal_identity_check(condition) else {
        return;
    };
    let equal = equal == truthy;
    seed_synthetic_key_if_needed(&var_name, scope, ctx);
    let types = scope.get(&var_name).to_vec();
    if types.is_empty() {
        return;
    }

    if equal {
        let admits = types.iter().any(|rt| {
            rt.type_string
                .union_members()
                .iter()
                .any(|member| literal.is_subtype_of(member))
        });
        if admits {
            scope.set(&var_name, vec![ResolvedType::from_type_string(literal)]);
        }
        return;
    }

    let kept: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| {
            rt.type_string = strip_literal_from_type(&rt.type_string, &literal)?;
            Some(rt)
        })
        .collect();
    // Nothing is recorded when the subtraction empties the type: the
    // remaining alternatives are what the branch runs on, and "no type at
    // all" is not one of them.
    if !kept.is_empty() {
        scope.set(&var_name, kept);
    }
}

/// Remove every alternative of `ty` that the literal `excluded` covers,
/// returning `None` when that leaves nothing.
///
/// Only alternatives the literal fully accounts for go: `'notLoaded'`
/// drops out of `bool|'notLoaded'|null`, while a bare `string` stays put
/// in `string|null` — ruling out one of its values does not rule out the
/// type.
fn strip_literal_from_type(ty: &PhpType, excluded: &PhpType) -> Option<PhpType> {
    if let TypeKind::Union(members) = ty.kind() {
        let kept: Vec<PhpType> = members
            .iter()
            .filter_map(|member| strip_literal_from_type(member, excluded))
            .collect();
        return match kept.len() {
            0 => None,
            1 => kept.into_iter().next(),
            _ => Some(PhpType::union(kept)),
        };
    }
    if let TypeKind::Nullable(inner) = ty.kind() {
        if excluded.is_null() {
            return Some(inner.clone());
        }
        let kept = strip_literal_from_type(inner, excluded)?;
        return Some(PhpType::nullable(kept));
    }
    (!ty.is_subtype_of(excluded)).then(|| ty.clone())
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
            _ => Some(ty.non_empty_array_form()),
        },
    }
}

/// Refine a variable's type to its empty counterpart.
///
/// The mirror of [`refine_non_empty_in_scope`]: `array<K, V>` becomes
/// `array{}` and `string` becomes `''`, while a member that promises at
/// least one entry (`non-empty-array`, a shape with a required key) drops
/// out of a union because no empty value could have been it. Members
/// outside the compared domain are left alone, exactly as the non-empty
/// side leaves them: `count($x) === 0` on a `Countable|array` says nothing
/// about the object half.
pub(crate) fn refine_empty_in_scope(var_name: &str, empty: EmptyValue, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let refined: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| {
            rt.type_string = refine_empty_type(&rt.type_string, empty)?;
            Some(rt)
        })
        .collect();

    if !refined.is_empty() {
        scope.set(var_name, refined);
    }
}

/// Apply [`refine_empty_in_scope`]'s rule to one `PhpType`, returning
/// `None` when the type cannot hold the empty value at all.
fn refine_empty_type(ty: &PhpType, empty: EmptyValue) -> Option<PhpType> {
    if let TypeKind::Union(members) = ty.kind() {
        let refined: Vec<PhpType> = members
            .iter()
            .filter_map(|member| refine_empty_type(member, empty))
            .collect();
        return match refined.len() {
            0 => None,
            1 => refined.into_iter().next(),
            _ => Some(PhpType::union(refined)),
        };
    }

    match empty {
        EmptyValue::String => {
            if let Some(content) = ty.as_literal().and_then(LiteralValue::string_content) {
                return content.is_empty().then(|| ty.clone());
            }
            match ty.kind() {
                TypeKind::Named(name) if name == "non-empty-string" => None,
                TypeKind::Named(name) if name == "string" => {
                    Some(PhpType::literal_string_value(""))
                }
                _ => Some(ty.clone()),
            }
        }
        EmptyValue::Array => {
            if !ty.is_array_like() {
                return Some(ty.clone());
            }
            if ty.is_provably_non_empty() {
                return None;
            }
            Some(PhpType::array_shape(Vec::new()))
        }
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
    // A `null` among the values the variable could hold is what makes
    // replacing them all with `null` sound.  A union that has none says
    // the opposite — `bool|string` came out of a falsy branch as `null`
    // while `non_null_type()` stood in for this check, because a union
    // with nothing to strip still has a non-null part.
    fn holds_null(ty: &PhpType) -> bool {
        match ty.kind() {
            TypeKind::Nullable(_) => true,
            TypeKind::Union(members) => members.iter().any(holds_null),
            _ => ty.is_null(),
        }
    }
    if types.iter().any(|rt| holds_null(&rt.type_string)) {
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

/// Keep only what a variable could hold and still be falsy.
///
/// The mirror of [`strip_falsy_from_scope`], which is what the branch a
/// truthy test *enters* applies. Both go through the same rule
/// ([`PhpType::falsy_type`] / [`PhpType::truthy_type`]), so the two halves
/// of one `if` agree about what the condition split.
///
/// A `bool` is the member that matters: keeping it whole on the path that
/// skipped the branch makes the two paths look like they could hold the
/// same value, so a join has nothing to key what the branch filled
/// against and re-testing the flag below recovers nothing:
///
/// ```php
/// $a = null;
/// if ($isI) { $a = makeA(); }
/// if ($isI) { $a->go(); }   // needs the skipped path to say `false`
/// ```
///
/// A variable nothing falsy could have been is left as it was rather than
/// emptied: the branch is dead, and saying so is the reachability
/// question rather than this one.
pub(crate) fn narrow_to_falsy_in_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let falsy: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| {
            let falsy = rt.type_string.falsy_type()?;
            // An object is truthy, so a falsy `?Customer` is a `null` that
            // no longer names a class.  Leaving the resolved class beside
            // it makes the entry read as a `Customer` to everything that
            // consults the class rather than the type, and a join with the
            // branch's own `Customer` then collapses the pair to whichever
            // type string came last.
            let dropped_class = falsy.unwrap_nullable().class_name()
                != rt.type_string.unwrap_nullable().class_name();
            if dropped_class {
                rt.class_info = None;
            }
            rt.type_string = falsy;
            Some(rt)
        })
        .collect();

    if !falsy.is_empty() {
        scope.set(var_name, falsy);
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
    } else {
        // Everything the variable could hold was `null`, so a path that
        // proves it is not null cannot run.  Saying so keeps the dead
        // path's end state out of the join instead of letting the
        // impossible `null` receiver there erase what the live paths
        // knew — which is what a first-iteration `if ($acc === null)`
        // seed/merge accumulator depends on.
        scope.unreachable = true;
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

/// Whether a scope key is a synthetic key for an expression
/// (`$this->cache`, `$row["id"]`, `mb_strpos($s, $m)`) rather than a
/// plain variable.
pub(crate) fn is_synthetic_key(key: &str) -> bool {
    narrowing::is_member_path_key(key) || narrowing::is_call_key(key)
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
    // Only seed compound keys (property access, array access, or a
    // static property).
    if !narrowing::is_member_path_key(key) {
        return;
    }
    // A call key that carries arguments cannot be re-resolved from its
    // text — the arguments decide the return type.  Those are seeded from
    // the expression they were built from, by
    // [`seed_call_subject_keys`].
    if narrowing::is_call_key_with_arguments(key) {
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
    if key.ends_with(']') {
        resolve_array_key_type(key, scope, ctx)
    } else if key.contains("->") {
        resolve_member_key_type(key, scope, ctx)
    } else if let Some((class_key, prop_name)) = split_static_property_key(key) {
        resolve_static_property_key_type(class_key, prop_name, ctx)
    } else {
        scope.get(key).to_vec()
    }
}

/// Split `self::$repo` into the class side (`self`) and the property name
/// (`repo`), or `None` when the key is not a static property path.
///
/// Only the trailing segment counts, so `self::$repo->name` (a property
/// read *through* a static property) is not one: it is a member path whose
/// base happens to be static, and `resolve_member_key_type` splits it.
fn split_static_property_key(key: &str) -> Option<(&str, &str)> {
    let pos = key.rfind("::$")?;
    let prop = &key[pos + 3..];
    if prop.is_empty() || prop.contains(|c: char| !(c.is_alphanumeric() || c == '_')) {
        return None;
    }
    Some((&key[..pos], prop))
}

/// Resolve what a static property's declaration promises.
///
/// `self`/`static` name the class the walk is inside; anything else is a
/// written class name resolved through the same path a type hint takes,
/// so an imported short name resolves like it does everywhere else.
fn resolve_static_property_key_type(
    class_key: &str,
    prop_name: &str,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    // `self`/`static` name the class the walk is already inside, so its
    // own `ClassInfo` is the owner. Looking the name up in the index
    // instead would miss a class the index does not carry — the document
    // being edited is exactly that case.
    if matches!(class_key, "self" | "static") {
        return static_property_hint_of(ctx.current_class, prop_name, ctx);
    }

    let class_name = match class_key {
        "parent" => match ctx.current_class.parent_class {
            Some(parent) => parent.to_string(),
            None => return Vec::new(),
        },
        other => other.to_string(),
    };
    let owners = crate::type_engine::type_resolution::type_hint_to_classes_typed(
        &PhpType::named(crate::atom::atom(&class_name)),
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    for owner in &owners {
        let resolved = static_property_hint_of(owner, prop_name, ctx);
        if !resolved.is_empty() {
            return resolved;
        }
    }
    Vec::new()
}

/// What `owner`'s declaration of `prop_name` promises.
fn static_property_hint_of(
    owner: &crate::types::ClassInfo,
    prop_name: &str,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    let Some(hint) =
        crate::inheritance::resolve_property_type_hint(owner, prop_name, ctx.class_loader)
    else {
        return Vec::new();
    };
    let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
        &hint,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    match resolved.is_empty() {
        true => vec![ResolvedType::from_type_string(hint)],
        false => ResolvedType::from_classes_with_hint(resolved, hint),
    }
}

/// Resolve the element type an array-access key promises (`$a["k"]`,
/// `$a->items["0"]`, `$a["x"]["y"]`, `$a["x"][$i]`).
fn resolve_array_key_type(
    key: &str,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    // Split off the *last* bracket segment so a nested access resolves its
    // base (`$a["x"]` of `$a["x"]["y"]`) through the same dispatcher.
    let Some((base_var, segment)) = narrowing::split_trailing_bracket(key) else {
        return Vec::new();
    };
    // A variable index names no shape entry, so only the container's
    // element type describes it.
    let key_name = narrowing::bracket_segment_literal(segment);

    // Only the leading variable of a path is ever assigned in the scope, so
    // a compound base has to be resolved the same way this key is.  Each
    // step drops one segment, so the recursion is bounded by the number of
    // segments in the key.
    // A static property is one of those bases: nothing ever assigns
    // `self::$cache` a scope entry, so without this the whole key comes
    // back unresolved and `isset(self::$cache[$k])` proves nothing about
    // the read it guards.
    let resolved_base;
    let base_types: &[ResolvedType] = match scope.get(base_var) {
        [] if base_var.contains("->")
            || base_var.ends_with(']')
            || split_static_property_key(base_var).is_some() =>
        {
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
        let element_type = key_name
            .and_then(|name| rt.type_string.extract_shape_key_type(name))
            .or_else(|| rt.type_string.extract_value_type(false).cloned())
            // An empty shape has no entry any key could address, so the
            // read is a guaranteed miss and yields `null` — the same
            // answer the offset-read path gives.  Widening to `mixed`
            // here would leave an `isset($a['k'])` branch claiming the
            // key could be anything, because `mixed` survives the null
            // strip that the check's whole purpose is to license.
            .or_else(|| rt.type_string.is_empty_array_shape().then(PhpType::null))
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
            && narrowing::is_member_path_key(&key)
        {
            seed_synthetic_key_if_needed(&key, scope, ctx);
        }
        // An argument that is itself a check (`assert($items[0] instanceof
        // Foo)`) carries its subject one level further down, so it is
        // seeded the same way an `if` condition's subject is.
        seed_property_keys_into_scope(arg_expr, scope, ctx);
    }

    // A tag can also name a path the call site never spells out —
    // `@phpstan-assert bool $this->resolved` on a method with no
    // arguments at all — so the tags themselves are read for subjects to
    // seed, not just the arguments.
    let snapshot = scope.locals.clone();
    let resolver =
        |vn: &str| -> Vec<ResolvedType> { snapshot.get(&atom(vn)).cloned().unwrap_or_default() };
    let var_ctx = build_var_ctx("", ctx, &resolver);
    let Some(info) = narrowing::extract_call_assertions(call, &var_ctx) else {
        return;
    };
    let keys: Vec<String> = info
        .assertions
        .iter()
        .filter_map(|assertion| narrowing::assertion_subject_key(&assertion.param_name, &info))
        .filter(|key| narrowing::is_member_path_key(key))
        .collect();
    for key in keys {
        seed_synthetic_key_if_needed(&key, scope, ctx);
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
                && narrowing::is_member_path_key(&key)
                && !keys.contains(&key)
            {
                keys.push(key);
            }
        }
        // Class identity: `get_class($a->foo) === Foo::class`, and the
        // `$a->foo::class` spelling of the same question, on either side
        // of the comparison.
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::Identical(_)
                    | BinaryOperator::Equal(_)
                    | BinaryOperator::NotIdentical(_)
                    | BinaryOperator::NotEqual(_)
            ) =>
        {
            for side in [bin.lhs, bin.rhs] {
                if let Some(key) = narrowing::class_identity_subject_key(side)
                    && narrowing::is_member_path_key(&key)
                    && !keys.contains(&key)
                {
                    keys.push(key);
                }
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
                        // A strict `in_array` proves its needle is one of
                        // the haystack's elements, so the needle is a
                        // subject the branch narrows like any other.
                        | "in_array"
                );
                if is_type_guard && let Some(first_arg) = func_call.argument_list.arguments.first()
                {
                    let arg_expr = match first_arg {
                        Argument::Positional(pos) => pos.value,
                        Argument::Named(named) => named.value,
                    };
                    if let Some(key) = narrowing::expr_to_subject_key(arg_expr)
                        && narrowing::is_member_path_key(&key)
                        && !keys.contains(&key)
                    {
                        keys.push(key);
                    }
                }
            }
        }
        // A bare truthy test names its subject and nothing else:
        // `$article->alt ? $article->alt : $article->title` and
        // `if ($row->id)` both prove the path is truthy, and no operator
        // is present for the arms above to match on.
        Expression::Access(
            Access::Property(_) | Access::NullSafeProperty(_) | Access::StaticProperty(_),
        )
        | Expression::ArrayAccess(_) => {
            if let Some(key) = narrowing::expr_to_subject_key(expr)
                && narrowing::is_member_path_key(&key)
                && !keys.contains(&key)
            {
                keys.push(key);
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
    seed_call_subject_keys(condition, scope, ctx);

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

/// Seed the scope with the current type of every call the condition
/// tests, keyed under the call's own written form.
///
/// This is the counterpart of [`seed_synthetic_key_if_needed`] for the
/// calls that seeder cannot answer: `mb_strpos($slug, $marker)` cannot be
/// re-resolved from its key text the way `$this->handle` can, because the
/// arguments are what decide the return type, and `currentUser()` has no
/// receiver path to walk.  Resolving them here, from the expression, puts
/// the un-narrowed type in scope so the guard that follows has something
/// to narrow — and every later occurrence of the same call text then reads
/// the narrowed entry instead of asking the callee what it returns.
pub(crate) fn seed_call_subject_keys(
    condition: &Expression<'_>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    match condition {
        Expression::Parenthesized(paren) => seed_call_subject_keys(paren.expression, scope, ctx),
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            seed_call_subject_keys(prefix.operand, scope, ctx)
        }
        Expression::Binary(bin) if bin.operator.is_instanceof() => {
            seed_call_subject(bin.lhs, scope, ctx)
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
            seed_call_subject_keys(bin.lhs, scope, ctx);
            seed_call_subject_keys(bin.rhs, scope, ctx);
        }
        // A comparison names its subject on whichever side is not the
        // value being compared against, and the narrowing extractors read
        // both, so both are offered here too.
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::Identical(_)
                    | BinaryOperator::NotIdentical(_)
                    | BinaryOperator::Equal(_)
                    | BinaryOperator::NotEqual(_)
            ) =>
        {
            seed_call_subject(bin.lhs, scope, ctx);
            seed_call_subject(bin.rhs, scope, ctx);
        }
        // A bare truthy test on a call, and the argument of a type guard
        // or assertion helper written around one.
        Expression::Call(call) => {
            let argument_list = match call {
                Call::Function(fc) => &fc.argument_list,
                Call::Method(mc) => &mc.argument_list,
                Call::NullSafeMethod(mc) => &mc.argument_list,
                Call::StaticMethod(sc) => &sc.argument_list,
            };
            for argument in argument_list.arguments.iter() {
                seed_call_subject(narrowing::argument_value(argument), scope, ctx);
            }
            seed_call_subject(condition, scope, ctx);
        }
        Expression::Construct(Construct::Isset(isset)) => {
            for value in isset.values.iter() {
                seed_call_subject(value, scope, ctx);
            }
        }
        Expression::Construct(Construct::Empty(empty)) => {
            seed_call_subject(empty.value, scope, ctx)
        }
        _ => {}
    }
}

/// Seed one expression under its call key, when it has one and the scope
/// does not already carry it.
fn seed_call_subject(expr: &Expression<'_>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    let Some(key) = narrowing::expr_to_subject_key(expr) else {
        return;
    };
    if !seeds_from_expression(&key) || scope.contains(&key) {
        return;
    }
    let types = super::assignment::resolve_rhs_with_scope(expr, scope, ctx);
    scope.set(&key, types);
}

/// Whether a call key has to be seeded from the expression it was built
/// from rather than re-resolved from its own text by
/// [`resolve_synthetic_key_type`].
///
/// A key carrying arguments never re-resolves: the arguments are what
/// decide the return type, and the key text is all that survives. A bare
/// `currentUser()` or `Holder::make()` names everything needed to resolve
/// it, but the text-based resolver only knows how to walk a receiver path,
/// which neither has. What it does own is `$h->get()`, whose receiver the
/// walker's scope already holds, so that shape is left to it.
fn seeds_from_expression(key: &str) -> bool {
    narrowing::is_call_key(key)
        && (narrowing::is_call_key_with_arguments(key) || !narrowing::is_member_path_key(key))
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
        [] if narrowing::is_member_path_key(obj_var) => {
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
        // `self` / `static` / `$this` in the member's declared type name
        // the class the member was read off, not the class the reading
        // code happens to sit in.  `Scope::getParentScope(): ?self` seeded
        // against the enclosing class made a member lookup on the result
        // report a method missing from a class the code never mentions.
        let hint = if hint.contains_self_ref() {
            hint.resolve_self_refs(&cls.fqn(), cls.parent_class.as_ref().map(|p| p.as_str()))
        } else {
            hint
        };
        let resolved_classes = crate::type_engine::type_resolution::type_hint_to_classes_typed(
            &hint,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if resolved_classes.is_empty() {
            if is_call && !hint_says_what_it_says(&hint, ctx.class_loader) {
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

/// Whether a declared type that resolved to no class still means exactly
/// what it is written as, so seeding it as a call key's type is safe.
///
/// Two very different things resolve to no class.  One is a name standing
/// in for a type the declaration cannot resolve on its own — a `@template`
/// parameter, or an alias imported from elsewhere.  The call resolver at
/// the use site answers those properly by substituting from the receiver,
/// so seeding the unsubstituted name here would shadow a better answer
/// with a type no class stands behind.  The other is a type that is not a
/// name at all (`string|false`, `list<int>`, `array{a: int}`,
/// `int<0, max>`): nothing substitutes into it, so seeding it is what lets
/// a guard on a call key narrow the same way one on a property key
/// already does.
///
/// The test that separates them is whether every name the type mentions is
/// one we can account for: a keyword, or a class the loader knows.
/// `array<string, Foo>` passes, `T[]` does not.  An unevaluated type
/// operator (`key-of<X>`, `X[K]`) is excluded whatever it names, since its
/// meaning is still waiting on the operand.
fn hint_says_what_it_says(
    hint: &PhpType,
    class_loader: &dyn Fn(&str) -> Option<std::sync::Arc<crate::types::ClassInfo>>,
) -> bool {
    !hint.contains_unevaluated_operator() && every_name_is_known(hint, class_loader)
}

/// Whether every class-like name in `hint` is a keyword or loads.
fn every_name_is_known(
    hint: &PhpType,
    class_loader: &dyn Fn(&str) -> Option<std::sync::Arc<crate::types::ClassInfo>>,
) -> bool {
    let known = |name: &str| crate::php_type::is_keyword_type(name) || class_loader(name).is_some();
    match hint.kind() {
        TypeKind::Named(name) | TypeKind::StaticType(name) | TypeKind::ThisType(name) => {
            known(name)
        }
        TypeKind::Nullable(inner)
        | TypeKind::Array(inner)
        | TypeKind::ClassString(Some(inner))
        | TypeKind::InterfaceString(Some(inner)) => every_name_is_known(inner, class_loader),
        TypeKind::Union(members) | TypeKind::Intersection(members) => members
            .iter()
            .all(|member| every_name_is_known(member, class_loader)),
        TypeKind::Generic(generic) => {
            known(&generic.name)
                && generic
                    .args
                    .iter()
                    .all(|arg| every_name_is_known(arg, class_loader))
        }
        TypeKind::ArrayShape(entries) | TypeKind::ObjectShape(entries) => entries
            .iter()
            .all(|entry| every_name_is_known(&entry.value_type, class_loader)),
        TypeKind::Callable(callable) => {
            known(&callable.kind)
                && callable
                    .params
                    .iter()
                    .all(|param| every_name_is_known(&param.type_hint, class_loader))
                && callable
                    .return_type
                    .as_ref()
                    .is_none_or(|ret| every_name_is_known(ret, class_loader))
        }
        TypeKind::Literal(_)
        | TypeKind::IntRange(..)
        | TypeKind::ClassString(None)
        | TypeKind::InterfaceString(None) => true,
        // A conditional return type is decided by the arguments, and a
        // `Raw` node is text we could not parse; neither says anything a
        // scope key can hold.
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
                crate::util::strip_fqn_prefix(func_name)
                    .to_ascii_lowercase()
                    .as_str(),
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
