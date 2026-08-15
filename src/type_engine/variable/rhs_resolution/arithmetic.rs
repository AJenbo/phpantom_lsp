/// Binary-operator result-type inference.
///
/// Shared by [`resolve_rhs_expression`](super::resolve_rhs_expression) and
/// the forward walker's compound-assignment handling (`+=`, `-=`, …), so a
/// fix here reaches every consumer that asks "what type does this operator
/// produce?" rather than being answered twice.
use mago_syntax::cst::binary::{Binary, BinaryOperator};

use crate::php_type::{PhpType, TypeKind, keyword_lowercase};
use crate::type_engine::resolver::VarResolutionCtx;
use crate::type_engine::types::const_fold::{self, BitwiseOp};
use crate::types::ResolvedType;

use super::resolve_rhs_expression;

/// The result type a binary operator produces, or `None` for an operator
/// this module does not classify (concatenation and `??` are handled by
/// their own dedicated callers before this is reached).
pub(super) fn resolve_binary_result_type<'b>(
    binary: &'b Binary<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    // Spaceship (<=>): always int (-1, 0, or 1).
    if matches!(binary.operator, BinaryOperator::Spaceship(_)) {
        return Some(vec![ResolvedType::from_type_string(PhpType::int())]);
    }

    // instanceof, comparison, logical: always bool.
    if binary.operator.is_instanceof()
        || binary.operator.is_comparison()
        || binary.operator.is_logical()
    {
        return Some(vec![ResolvedType::from_type_string(PhpType::bool())]);
    }

    // Modulo (%): always int.
    if matches!(binary.operator, BinaryOperator::Modulo(_)) {
        return Some(vec![ResolvedType::from_type_string(PhpType::int())]);
    }

    // Addition (+): PHP overloads this for array union vs numeric addition.
    if matches!(binary.operator, BinaryOperator::Addition(_)) {
        let lhs_types = resolve_rhs_expression(binary.lhs, ctx);
        let rhs_types = resolve_rhs_expression(binary.rhs, ctx);
        return Some(vec![ResolvedType::from_type_string(
            infer_addition_result_type(&lhs_types, &rhs_types),
        )]);
    }

    // Arithmetic: -, *, /, **.
    if matches!(
        binary.operator,
        BinaryOperator::Subtraction(_)
            | BinaryOperator::Multiplication(_)
            | BinaryOperator::Division(_)
            | BinaryOperator::Exponentiation(_)
    ) {
        let lhs_types = resolve_rhs_expression(binary.lhs, ctx);
        let rhs_types = resolve_rhs_expression(binary.rhs, ctx);
        let is_division = matches!(binary.operator, BinaryOperator::Division(_));
        return Some(vec![ResolvedType::from_type_string(
            infer_arithmetic_result_type(&lhs_types, &rhs_types, is_division),
        )]);
    }

    // Bitwise operators (&, |, ^, <<, >>).
    // When both operands are strings, PHP applies bitwise ops
    // character-by-character and returns a string.  Otherwise int — or the
    // exact value, when both operands are known integers.
    if let Some(op) = bitwise_op(&binary.operator) {
        let lhs_types = resolve_rhs_expression(binary.lhs, ctx);
        let rhs_types = resolve_rhs_expression(binary.rhs, ctx);
        // `&`, `|` and `^` are the ones PHP overloads for strings; a shift
        // always produces an int.
        if matches!(op, BitwiseOp::And | BitwiseOp::Or | BitwiseOp::Xor) {
            let both_strings = !lhs_types.is_empty()
                && !rhs_types.is_empty()
                && lhs_types
                    .iter()
                    .all(|rt| rt.type_string.is_subtype_of(&PhpType::string()))
                && rhs_types
                    .iter()
                    .all(|rt| rt.type_string.is_subtype_of(&PhpType::string()));
            if both_strings {
                return Some(vec![ResolvedType::from_type_string(PhpType::string())]);
            }
        }
        // A mask built from constants (`$flags = JSON_PRETTY_PRINT |
        // JSON_THROW_ON_ERROR`) keeps its value, so a call that is handed it
        // can still read the bits it sets.
        if let Some(value) = single_literal_int(&lhs_types)
            .zip(single_literal_int(&rhs_types))
            .and_then(|(lhs, rhs)| const_fold::apply_bitwise(op, lhs, rhs))
        {
            return Some(vec![ResolvedType::from_type_string(PhpType::literal_int(
                value.to_string(),
            ))]);
        }
        return Some(vec![ResolvedType::from_type_string(PhpType::int())]);
    }

    None
}

/// Classify a resolved operand as `int`, `float`, or unknown for arithmetic
/// type promotion.
///
/// Returns `Some(true)` for float, `Some(false)` for int/bool, `None` when
/// the type is mixed or otherwise ambiguous. Handles unions and nullable
/// types by classifying each member.
pub(crate) fn classify_numeric_operand(types: &[ResolvedType]) -> Option<bool> {
    if types.is_empty() {
        return None;
    }
    let mut saw_float = false;
    let mut saw_int = false;
    for rt in types {
        classify_php_type(&rt.type_string, &mut saw_float, &mut saw_int)?;
    }
    if saw_float && saw_int {
        // Both int-like and float-like members present (e.g. int|float
        // union) — the runtime result could be either, so return None to
        // fall back to the conservative int|float.
        None
    } else if saw_float {
        Some(true)
    } else if saw_int {
        Some(false)
    } else {
        None
    }
}

/// Recursively classify a `PhpType` as int-like or float-like.
///
/// Returns `None` (and short-circuits) if any member is ambiguous (mixed,
/// string, object, etc.). Updates `saw_float` and `saw_int` flags for known
/// numeric members. `null` members are ignored since they coerce to 0 in
/// arithmetic context.
fn classify_php_type(ty: &PhpType, saw_float: &mut bool, saw_int: &mut bool) -> Option<()> {
    // Defer to `is_int_subtype`/`is_float_subtype` so every PHPDoc int
    // refinement (`int<0,max>`, `positive-int`, …) and float spelling is
    // recognised, not just the bare `int`/`float` names.
    if ty.is_float_subtype() {
        *saw_float = true;
        return Some(());
    }
    if ty.is_int_subtype() {
        *saw_int = true;
        return Some(());
    }
    match ty.kind() {
        TypeKind::Named(n) => {
            let lower = keyword_lowercase(n);
            if lower == "bool" || lower == "boolean" || lower == "true" || lower == "false" {
                *saw_int = true;
            } else if lower == "numeric" || n == "number" {
                *saw_int = true;
                *saw_float = true;
            } else if lower == "null" {
                // null coerces to 0 (int) in arithmetic; ignore it so that
                // `int|null` classifies as int-like.
            } else {
                return None; // mixed, string, object, etc.
            }
            Some(())
        }
        TypeKind::Union(members) => {
            for member in members {
                classify_php_type(member, saw_float, saw_int)?;
            }
            Some(())
        }
        TypeKind::Nullable(inner) => {
            // ?T is T|null — classify the inner type, ignore null.
            classify_php_type(inner, saw_float, saw_int)
        }
        _ => None,
    }
}

/// Infer the result type of an arithmetic operation based on operand types,
/// following PHP's numeric type promotion rules.
///
/// - `int op int` → `int` (for `+`, `-`, `*`, `**`)
/// - `int op float` or `float op int` → `float`
/// - `float op float` → `float`
/// - `int / int` → `int|float` (division can produce either)
/// - Anything else → `int|float`
pub(crate) fn infer_arithmetic_result_type(
    lhs_types: &[ResolvedType],
    rhs_types: &[ResolvedType],
    is_division: bool,
) -> PhpType {
    let lhs = classify_numeric_operand(lhs_types);
    let rhs = classify_numeric_operand(rhs_types);
    match (lhs, rhs) {
        // Both are known int (not float): int op int.
        (Some(false), Some(false)) => {
            if is_division {
                // int / int can return float (e.g. 7/2 = 3.5).
                PhpType::union(vec![PhpType::int(), PhpType::float()])
            } else {
                PhpType::int()
            }
        }
        // At least one float, the other is known: result is float.
        (Some(true), Some(_)) | (Some(_), Some(true)) => PhpType::float(),
        // One or both operands are unknown: fall back to int|float.
        _ => PhpType::union(vec![PhpType::int(), PhpType::float()]),
    }
}

/// Infer the result type of `+` / `+=`, which PHP overloads for the array
/// union as well as numeric addition.
///
/// Two arrays union their keys, which
/// [`merge_array_plus`](super::super::resolution::merge_array_plus) works
/// out from whatever both sides know. Only a mix of an array and a number
/// has no meaningful result type: PHP raises a `TypeError` for it, so a bare
/// `array` stands in rather than a number the operation cannot produce.
pub(crate) fn infer_addition_result_type(
    lhs_types: &[ResolvedType],
    rhs_types: &[ResolvedType],
) -> PhpType {
    let lhs_is_array = lhs_types.iter().any(|rt| rt.type_string.is_array_like());
    let rhs_is_array = rhs_types.iter().any(|rt| rt.type_string.is_array_like());
    if rhs_is_array && (lhs_is_array || lhs_types.is_empty()) {
        // An operand with no tracked type still gets everything the array
        // beside it contributes: `+` only accepts arrays, so whatever it
        // held was one too.
        let lhs_type = if lhs_types.is_empty() {
            PhpType::array()
        } else {
            ResolvedType::types_joined(lhs_types)
        };
        return super::super::resolution::merge_array_plus(
            &lhs_type,
            &ResolvedType::types_joined(rhs_types),
        );
    }
    if lhs_is_array || rhs_is_array {
        return PhpType::array();
    }
    infer_arithmetic_result_type(lhs_types, rhs_types, false)
}

/// The bitwise operator `operator` is, or `None` for every other binary
/// operator.
fn bitwise_op(operator: &BinaryOperator<'_>) -> Option<BitwiseOp> {
    Some(match operator {
        BinaryOperator::BitwiseAnd(_) => BitwiseOp::And,
        BinaryOperator::BitwiseOr(_) => BitwiseOp::Or,
        BinaryOperator::BitwiseXor(_) => BitwiseOp::Xor,
        BinaryOperator::LeftShift(_) => BitwiseOp::LeftShift,
        BinaryOperator::RightShift(_) => BitwiseOp::RightShift,
        _ => return None,
    })
}

/// The integer an operand holds, when it resolved to exactly one literal
/// integer. An operand that could be several types is not a known value.
fn single_literal_int(types: &[ResolvedType]) -> Option<i64> {
    let [only] = types else {
        return None;
    };
    const_fold::literal_int_value(&only.type_string)
}
