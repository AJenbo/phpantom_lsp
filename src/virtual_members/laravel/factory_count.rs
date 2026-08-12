//! Count-conditional return types for Eloquent factory chains.
//!
//! `User::factory()->create()` builds a single `User`, but
//! `User::factory(3)->create()`, `User::factory()->count(3)->create()` and
//! `UserFactory::times(3)->create()` all build a
//! `Collection<int, User>`.  Laravel expresses both outcomes with one
//! `@return Collection<int, TModel>|TModel` annotation on
//! `Factory::create()`/`make()`, which is ambiguous at every call site.
//!
//! This module reads the count state off the receiver chain and picks the
//! branch the call actually produces, mirroring Larastan's conditional
//! return type extensions.  Only the syntactic chain is inspected, so a
//! factory that travels through a variable (`$factory = User::factory(); …`)
//! carries no count state we can see.  That case is left alone rather than
//! guessed at: narrowing it to one model would make `create()->first()` a
//! false positive whenever the variable did hold a count.

use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::{Argument, ArgumentList, Call, ClassLikeMemberSelector, Expression};

use crate::atom::{atom, bytes_to_str};
use crate::php_type::PhpType;
use crate::type_engine::conditional_resolution::split_text_args;
use crate::type_engine::resolver::ResolutionCtx;
use crate::type_engine::subject_expr::SubjectExpr;
use crate::types::{ClassInfo, ELOQUENT_COLLECTION_FQN, ResolvedType};

use super::factory::{extends_eloquent_factory, factory_model_type};

/// How many models a factory chain builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FactoryCount {
    /// No count was set, or a previously set count was cleared with
    /// `count(null)`.
    One,
    /// `count()`, `times()`, or a numeric `factory(3)` argument set a
    /// count, so the chain builds a collection.
    Many,
    /// The chain sets no count of its own and does not reach back to a
    /// head that settles the question, so either outcome is possible.
    Unknown,
}

/// Whether `name` is one of the `Factory` methods whose return type
/// depends on the chain's count state.
///
/// `createOne()`/`makeOne()` always build one model and
/// `createMany()`/`makeMany()` always build a collection, so neither is
/// count-conditional.
pub(crate) fn is_count_conditional_method(name: &str) -> bool {
    matches!(name, "create" | "createQuietly" | "make")
}

/// Read the count state off a factory receiver chain.
///
/// The chain is walked outermost-first, so the *last* count-setting call
/// wins — `User::factory(3)->count(null)` builds one model and
/// `User::factory()->count(2)` builds two.  Calls that are not
/// count-setting (`state()`, `hasPosts()`, `trashed()`, …) are stepped
/// over.  A static call settles the count either way, since it is the
/// head of the chain; anything else the walk reaches (a variable, a
/// property, `new UserFactory(…)` whose arguments the subject parser
/// does not keep) leaves it [`FactoryCount::Unknown`].
pub(crate) fn chain_count(receiver: &SubjectExpr) -> FactoryCount {
    // Every step descends into a strictly smaller sub-expression, so the
    // walk terminates on any chain the subject parser can produce.
    let mut current = receiver;
    loop {
        let SubjectExpr::CallExpr { callee, args_text } = current else {
            return FactoryCount::Unknown;
        };
        let args = split_text_args(args_text);
        let first_arg = args.first().map(|a| a.trim());
        match callee.as_ref() {
            SubjectExpr::MethodCall { base, method } => {
                if let Some(state) = instance_count_state(method, first_arg) {
                    return state;
                }
                current = base;
            }
            // A static call is the head of the chain: `Model::factory()`,
            // `UserFactory::new()`, `UserFactory::times(3)`.
            SubjectExpr::StaticMethodCall { method, .. } => {
                return static_count_state(method, first_arg);
            }
            _ => return FactoryCount::Unknown,
        }
    }
}

/// Read the count state off a factory receiver chain in its AST form.
///
/// The AST-walking resolution path (assignments, arguments, property and
/// return types) hands over an [`Expression`] rather than a parsed
/// [`SubjectExpr`], so the same outermost-first walk runs over the call
/// nodes directly.  Both walks share the per-call count rules below, so
/// the two paths cannot drift apart on what a chain builds.
pub(crate) fn chain_count_ast(receiver: &Expression<'_>, content: &str) -> FactoryCount {
    // Every step descends into a strictly smaller sub-expression, so the
    // walk terminates on any chain the parser can produce.
    let mut current = receiver;
    loop {
        let call = match current {
            Expression::Call(call) => call,
            Expression::Parenthesized(inner) => {
                current = inner.expression;
                continue;
            }
            _ => return FactoryCount::Unknown,
        };
        let (object, selector, argument_list) = match call {
            Call::Method(mc) => (Some(mc.object), &mc.method, &mc.argument_list),
            Call::NullSafeMethod(mc) => (Some(mc.object), &mc.method, &mc.argument_list),
            // A static call is the head of the chain: `Model::factory()`,
            // `UserFactory::new()`, `UserFactory::times(3)`.
            Call::StaticMethod(sc) => (None, &sc.method, &sc.argument_list),
            Call::Function(_) => return FactoryCount::Unknown,
        };
        // A computed call target (`$factory->$method()`) says nothing
        // about the count.
        let ClassLikeMemberSelector::Identifier(ident) = selector else {
            return FactoryCount::Unknown;
        };
        let method = bytes_to_str(ident.value);
        let first_arg = first_argument_text(argument_list, content);
        match object {
            Some(base) => {
                if let Some(state) = instance_count_state(method, first_arg) {
                    return state;
                }
                current = base;
            }
            None => return static_count_state(method, first_arg),
        }
    }
}

/// The source text of a call's first argument, for the count rules that
/// gate on how the argument was written.
fn first_argument_text<'c>(argument_list: &ArgumentList<'_>, content: &'c str) -> Option<&'c str> {
    let span = match argument_list.arguments.first()? {
        Argument::Positional(pos) => pos.value.span(),
        Argument::Named(named) => named.value.span(),
    };
    content
        .get(span.start.offset as usize..span.end.offset as usize)
        .map(str::trim)
}

/// Count state contributed by an instance call in the chain, or `None`
/// when the call does not touch the count.
fn instance_count_state(method: &str, first_arg: Option<&str>) -> Option<FactoryCount> {
    match method {
        // `count(?int $count)` — the only way to clear a count is to pass
        // a literal `null`.  A non-literal argument is assumed to be the
        // integer the parameter asks for.
        "count" => Some(match first_arg {
            None => FactoryCount::One,
            Some(arg) => {
                if arg.eq_ignore_ascii_case("null") {
                    FactoryCount::One
                } else {
                    FactoryCount::Many
                }
            }
        }),
        // `times(int $count)` cannot be given null.
        "times" => Some(FactoryCount::Many),
        _ => None,
    }
}

/// Count state contributed by the static call that opens the chain.
fn static_count_state(method: &str, first_arg: Option<&str>) -> FactoryCount {
    match method {
        // `Model::factory(…)` forwards its first argument to `count()`
        // only when it is numeric; an array or callable is state.
        "factory" => factory_argument_count(first_arg),
        "times" => FactoryCount::Many,
        // `Factory::new(array $attributes)` takes state, never a count.
        "new" => FactoryCount::One,
        // Some other static call handed back a factory, and whatever
        // count it was built with is not visible from here.
        _ => FactoryCount::Unknown,
    }
}

/// Count state set by `Model::factory(…)`'s first argument.
///
/// Laravel gates on `is_numeric($parameters[0])`, so the argument has to
/// be written out to settle the question: a numeric literal (`3`, and
/// `'3'`, which `is_numeric()` also accepts) sets a count, and a literal
/// it rejects (an array, a closure, `null`) is state.  A variable or a
/// call could be either, and guessing wrong would turn one model into a
/// collection or the reverse.
fn factory_argument_count(first_arg: Option<&str>) -> FactoryCount {
    let Some(first) = first_arg else {
        return FactoryCount::One;
    };
    if !is_decidable_literal(first) {
        return FactoryCount::Unknown;
    }
    let unquoted = crate::text_scan::unquote_php_string(first).unwrap_or(first);
    if !unquoted.is_empty() && unquoted.parse::<f64>().is_ok() {
        FactoryCount::Many
    } else {
        FactoryCount::One
    }
}

/// Whether an argument's spelling settles what `is_numeric()` would say
/// about it.
///
/// A literal does — a number, a quoted string, an array, a closure,
/// `null` — while a variable, a constant, or another call does not.
fn is_decidable_literal(arg: &str) -> bool {
    match arg.chars().next() {
        Some(c) if c.is_ascii_digit() || matches!(c, '\'' | '"' | '[' | '-' | '+' | '.') => true,
        Some(_) => {
            ["null", "true", "false"]
                .iter()
                .any(|k| arg.eq_ignore_ascii_case(k))
                || [
                    "array(",
                    "fn(",
                    "fn ",
                    "function(",
                    "function ",
                    "static ",
                    "new ",
                ]
                .iter()
                .any(|p| {
                    arg.get(..p.len())
                        .is_some_and(|head| head.eq_ignore_ascii_case(p))
                })
        }
        None => false,
    }
}

/// Resolve `create()` / `createQuietly()` / `make()` on an Eloquent
/// factory to the type the call-site chain actually builds.
///
/// Returns `None` — leaving the declared return type alone — when the
/// method is not count-conditional, the chain's count state is unknown,
/// the receiver is not a factory, the factory declares the method itself,
/// or the model type cannot be determined.
pub(crate) fn resolve_factory_count_return(
    receiver: &SubjectExpr,
    method_name: &str,
    owners: &[ResolvedType],
    ctx: &ResolutionCtx<'_>,
) -> Option<(Vec<Arc<ClassInfo>>, PhpType)> {
    if !is_count_conditional_method(method_name) {
        return None;
    }
    // Read the count off the chain before touching the class loader.  The
    // walk is pure syntax, and it rules out the great majority of the
    // `create()`/`make()` calls in a codebase — every builder that is not
    // a factory, and every factory reached through a variable — without a
    // hierarchy walk per owner.
    resolve_for_count(chain_count(receiver), method_name, owners, ctx)
}

/// [`resolve_factory_count_return`] for the AST-walking resolution path.
///
/// Assignments, arguments, property writes and returns reach method calls
/// as [`Expression`] nodes rather than parsed subject strings, so they
/// read the chain's count with [`chain_count_ast`] and share everything
/// downstream of it.
pub(crate) fn resolve_factory_count_return_ast(
    receiver: &Expression<'_>,
    method_name: &str,
    owners: &[ResolvedType],
    content: &str,
    ctx: &ResolutionCtx<'_>,
) -> Option<(Vec<Arc<ClassInfo>>, PhpType)> {
    if !is_count_conditional_method(method_name) {
        return None;
    }
    resolve_for_count(chain_count_ast(receiver, content), method_name, owners, ctx)
}

/// Pick the type a count-conditional factory call builds, given the count
/// state its receiver chain established.
fn resolve_for_count(
    count: FactoryCount,
    method_name: &str,
    owners: &[ResolvedType],
    ctx: &ResolutionCtx<'_>,
) -> Option<(Vec<Arc<ClassInfo>>, PhpType)> {
    if count == FactoryCount::Unknown {
        return None;
    }

    let factory = owners.iter().find_map(|rt| {
        rt.class_info
            .as_ref()
            .filter(|ci| extends_eloquent_factory(ci, ctx.class_loader))
    })?;

    // A factory that writes its own `create()`/`make()` keeps whatever it
    // declared — only the signature inherited from Laravel's `Factory`
    // (and the single-model stand-in PHPantom synthesizes for
    // convention-based factories) is ours to reinterpret.  The receiver
    // may already be a merged class, so the own-member check goes through
    // the loader, which hands back the class as parsed.
    let fqn = factory.fqn();
    if (ctx.class_loader)(fqn.as_str()).is_some_and(|raw| raw.get_method_ci(method_name).is_some())
    {
        return None;
    }

    let model = factory_model_type(factory, ctx.class_loader)?;

    // The call has to resolve to *some* inherited or synthesized method;
    // a factory with no `create()` at all gets no return type from us.
    let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
        factory,
        ctx.class_loader,
        ctx.resolved_class_cache,
    );
    merged.get_method_ci(method_name)?;

    let resolved = match count {
        FactoryCount::Many => {
            let collection = PhpType::generic(
                ELOQUENT_COLLECTION_FQN,
                vec![PhpType::named(atom("int")), model],
            );
            super::replace_eloquent_collections_in_type(&collection, ctx.class_loader)
                .unwrap_or(collection)
        }
        FactoryCount::One | FactoryCount::Unknown => model,
    };

    let classes = crate::type_engine::type_resolution::type_hint_to_classes_typed(
        &resolved,
        fqn.as_str(),
        ctx.all_classes,
        ctx.class_loader,
    );
    if classes.is_empty() {
        return None;
    }

    Some((classes, resolved))
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "factory_count_tests.rs"]
mod tests;
