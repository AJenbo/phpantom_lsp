/// Return-type rules for the array-producing and element-extracting
/// standard library functions.
///
/// The rules themselves live here, independent of where the call's
/// arguments come from.  Two callers reach a call expression from
/// opposite directions and both need the same answers:
///
/// * the AST walker, when the call is an assignment right-hand side and
///   a parsed `ArgumentList` is in hand (see
///   [`super::raw_type_inference`]), and
/// * the text-driven call resolver, when the call appears inline (as a
///   subject, an array-access base, or another call's argument) and only
///   the raw argument source text is available (see
///   `type_engine::call_resolution`).
///
/// [`ArrayFuncArgs`] is the seam between the two: each caller answers
/// the handful of questions the rules ask about an argument, and the
/// rules stay in one place so a fix to `array_map`'s element type
/// reaches every consumer.
use crate::php_type::{PhpType, TypeKind, is_array_like_name};

use super::{ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS};

/// Argument access needed by the array-function return-type rules.
pub(in crate::type_engine) trait ArrayFuncArgs {
    /// The unflattened type of the argument at `index` (e.g.
    /// `list<User>`), or `None` when it cannot be resolved.
    fn arg_raw_type(&self, index: usize) -> Option<PhpType>;

    /// Whether the argument at `index` is the literal `false`.
    fn is_false_literal(&self, index: usize) -> bool;

    /// Whether an argument was written at `index`.
    ///
    /// Distinguishes an omitted argument from one that is present but
    /// unresolvable, which [`arg_raw_type`](Self::arg_raw_type) cannot:
    /// both come back as `None`. `array_filter($a)` and
    /// `array_filter($a, $cb)` differ only in this.
    fn has_arg(&self, index: usize) -> bool;

    /// The return type declared on the closure or arrow function at
    /// `index`.  `None` when the argument is not an inline function or
    /// declares no return type.
    fn callback_declared_return_type(&self, index: usize) -> Option<PhpType>;

    /// The return type inferred from the body of the closure or arrow
    /// function at `index`, with its first parameter seeded to
    /// `param_type`.
    fn callback_inferred_return_type(&self, index: usize, param_type: &PhpType) -> Option<PhpType>;

    /// The argument at `index` written as a bare constant name or integer
    /// literal (`ARRAY_FILTER_USE_KEY`, `2`), with any namespace prefix
    /// stripped.  `None` for any other expression.
    fn arg_atom_text(&self, index: usize) -> Option<String>;

    /// `subject` narrowed to the values that make the closure or arrow
    /// function at `index` accept them through its `param_index`th
    /// parameter.  `None` when the callback asserts nothing about it.
    fn callback_param_narrowing(
        &self,
        index: usize,
        param_index: usize,
        subject: &PhpType,
    ) -> Option<PhpType>;
}

/// For known array-producing functions, resolve the **raw output type**
/// (e.g. `list<User>`) from the input arguments.
///
/// Element-extracting functions are handled by
/// [`array_func_element_type`], which the caller consults first.
pub(in crate::type_engine) fn array_func_raw_type(
    func_name: &str,
    args: &dyn ArrayFuncArgs,
) -> Option<PhpType> {
    // Type-preserving functions: output array has same element type.
    if ARRAY_PRESERVING_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        // Every one of these rearranges the array (reorders, renumbers,
        // drops or chunks entries), so a constant shape does not survive
        // the call and is generalized to the container it describes.
        let raw = args.arg_raw_type(0)?.generalized_array();
        // Only a parameterised iterable carries an element type worth
        // preserving; a bare `array`/`iterable` is a `Named` kind with no
        // value argument to extract, so the rule declines and the stub's
        // own return type stands.
        //
        // `skip_scalar` must stay off here. It asks "is the element
        // non-scalar", which is not the question: a `list<string>` is
        // every bit as worth preserving as a `list<User>`, and answering
        // it with `true` silently dropped every scalar-element array back
        // to a bare `array`.
        if raw.extract_value_type(false).is_some() {
            // `array_filter($a)` with no callback keeps exactly the
            // members that survive a truthiness test, so the element type
            // drops `null`, `false`, `0`, `''` and friends. With a
            // callback the kept members are whatever it approves of, which
            // says nothing about their type.
            if func_name.eq_ignore_ascii_case("array_filter") {
                if !args.has_arg(1) {
                    return Some(filter_element_type(&raw).unwrap_or(raw));
                }
                // A callback handed the key decides which keys survive, so
                // what it asserts about them describes the result.
                if let Some(narrowed) = filter_key_type(&raw, args) {
                    return Some(narrowed);
                }
            }
            return Some(raw);
        }
    }

    // array_map: callback is first arg, array is second.
    // The callback's return type determines the output element type.
    if func_name.eq_ignore_ascii_case("array_map")
        && let Some(element_type) = array_map_element_type(args)
    {
        return Some(PhpType::list(element_type.widen_scalar_literals()));
    }

    // iterator_to_array: converts an iterator to an array, preserving
    // key and value types.  `iterator_to_array($iter)` where `$iter`
    // is `Iterator<int, Foo>` produces `array<int, Foo>`.  When only
    // a value type is available (single generic param), produces
    // `list<Foo>`.
    if func_name.eq_ignore_ascii_case("iterator_to_array") {
        let raw = args.arg_raw_type(0)?;
        let val = raw
            .iterable_element_type()
            .map(|value| value.widen_scalar_literals());
        // `preserve_keys: false` renumbers the result, so the key type
        // the iterator declared no longer describes it.
        if args.is_false_literal(1) {
            return Some(val.map_or_else(PhpType::array, PhpType::list));
        }
        let key = raw
            .iterable_key_type()
            .and_then(|key| super::resolution::normalize_array_key_type(&key));
        return match (key, val) {
            (Some(k), Some(v)) => Some(PhpType::generic_array(k, v)),
            (None, Some(v)) => Some(PhpType::list(v)),
            _ => Some(PhpType::array()),
        };
    }

    None
}

/// For known array functions, resolve the **element type**
/// (e.g. `User`) of the output.
///
/// This only covers true element-extracting functions (`array_pop`,
/// `current`, …) that return a single element.  Array-producing
/// functions like `array_map` and `iterator_to_array` are handled
/// exclusively by [`array_func_raw_type`], which preserves the
/// container type (e.g. `list<User>`).  Returning the element type here
/// would lose the array wrapper and break downstream consumers that
/// need to walk bracket segments (e.g. `$result[0]->`).
pub(in crate::type_engine) fn array_func_element_type(
    func_name: &str,
    args: &dyn ArrayFuncArgs,
) -> Option<PhpType> {
    if ARRAY_ELEMENT_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        // A scalar element is the honest answer for `array_pop(list<string>)`
        // just as `User` is for `list<User>`, so the element type is read
        // without `skip_scalar`.
        return args.arg_raw_type(0)?.iterable_element_type();
    }

    // `array_sum`/`array_product` are declared `int|float` because the
    // result follows PHP's numeric promotion, but an all-`int` array can
    // only sum to an `int`. The element type decides it, which is why this
    // cannot be expressed as a `@template` on the stub: `array<TValue>` with
    // `@return TValue` would answer `string` for `array_sum(list<string>)`
    // rather than the `int|float` PHP actually produces.
    if matches!(func_name, "array_sum" | "array_product") {
        let element = args.arg_raw_type(0)?.iterable_element_type()?;
        let members: Vec<&PhpType> = match element.kind() {
            TypeKind::Union(m) => m.iter().collect(),
            _ => vec![&element],
        };
        let (int_ty, float_ty) = (PhpType::int(), PhpType::float());
        let all_int = members.iter().all(|m| m.is_subtype_of(&int_ty));
        if all_int {
            return Some(int_ty);
        }
        // `int` is a subtype of `float` here (PHP widens it silently), so
        // a member has to be tested against both to tell `list<float>`
        // apart from `list<int|float>` — the latter really can sum to
        // either and keeps the declared union.
        if members
            .iter()
            .all(|m| m.is_subtype_of(&float_ty) && !m.is_subtype_of(&int_ty))
        {
            return Some(float_ty);
        }
        return None;
    }

    None
}

/// Rebuild an iterable type with its element narrowed to the members that
/// pass a truthiness test.
///
/// Returns `None` when the element type has no falsy member to remove (so
/// the caller keeps the type it already has) or when every member is falsy,
/// since `array<string, never>` is a worse answer than the original.
fn filter_element_type(raw: &PhpType) -> Option<PhpType> {
    let element = raw.extract_value_type(false)?;
    let truthy = element.truthy_type()?;
    if truthy == *element {
        return None;
    }
    match raw.kind() {
        TypeKind::Array(_) => Some(PhpType::array_of(truthy)),
        TypeKind::Generic(g) if !g.args.is_empty() => {
            let mut args = g.args.clone();
            // Same `<TKey, TValue>` convention `extract_value_type` reads:
            // the value is the second argument when there are two or more,
            // and the lone argument otherwise (`list<V>`).
            let value_idx = if args.len() >= 2 { 1 } else { args.len() - 1 };
            args[value_idx] = truthy;
            Some(PhpType::generic_atom(g.name, args))
        }
        _ => None,
    }
}

/// Rebuild an `array_filter` result with its key type narrowed to what
/// the callback asserts about the key it was handed.
///
/// Returns `None` unless the call runs in one of the two modes that pass
/// the key (`ARRAY_FILTER_USE_KEY`, `ARRAY_FILTER_USE_BOTH`), the callback
/// proves something about it, and the input carries a key type the proof
/// can narrow.
fn filter_key_type(raw: &PhpType, args: &dyn ArrayFuncArgs) -> Option<PhpType> {
    let param_index = filter_key_param_index(args)?;
    let key = raw.iterable_key_type()?;
    let narrowed = args.callback_param_narrowing(1, param_index, &key)?;
    // A callback that admits every key it could receive (`is_int($k) ||
    // is_string($k)`) leaves nothing to say, and answering with the
    // rebuilt union would only reorder its members.
    if key.is_subtype_of(&narrowed) {
        return None;
    }
    let value = raw.extract_value_type(false)?.clone();
    // A filter can drop every entry, so the result is a plain `array`
    // whatever refinement (`non-empty-array`, `list`) the input carried.
    match raw.kind() {
        TypeKind::Array(_) => Some(PhpType::generic_array(narrowed, value)),
        TypeKind::Generic(g) if is_array_like_name(g.name.as_str()) => {
            Some(PhpType::generic_array(narrowed, value))
        }
        _ => None,
    }
}

/// Which of the callback's parameters receives the key, from
/// `array_filter`'s mode argument.
///
/// The default mode passes only the value, so the callback says nothing
/// about the keys and this returns `None`.
fn filter_key_param_index(args: &dyn ArrayFuncArgs) -> Option<usize> {
    match args.arg_atom_text(2)?.as_str() {
        "ARRAY_FILTER_USE_KEY" | "2" => Some(0),
        // `ARRAY_FILTER_USE_BOTH` passes the value first and the key
        // second.
        "ARRAY_FILTER_USE_BOTH" | "1" => Some(1),
        _ => None,
    }
}

/// Extract the output element type for `array_map($callback, $array)`.
///
/// Strategy:
/// 1. If the callback (first arg) is a closure/arrow function with a
///    return type hint, use that.
/// 2. Otherwise infer it from the callback body, with the callback's
///    first parameter seeded to the input array's element type.
/// 3. Otherwise assume the callback passes its element through.
fn array_map_element_type(args: &dyn ArrayFuncArgs) -> Option<PhpType> {
    if let Some(declared) = args.callback_declared_return_type(0)
        && !declared.is_untyped()
    {
        return Some(declared);
    }

    let input_element = args.arg_raw_type(1)?.iterable_element_type()?;

    if let Some(inferred) = args.callback_inferred_return_type(0, &input_element) {
        return Some(inferred);
    }

    // Final fallback: assume the callback passes its element through. That
    // only holds for element types a callback is unlikely to convert; a
    // scalar element says nothing about the result, and `array_map('intval',
    // $strings)` would be reported as `list<string>` on the strength of an
    // input the callback exists to change.
    (!input_element.is_scalar_leaf()).then_some(input_element)
}
