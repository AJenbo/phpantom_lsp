//! Eloquent Builder-as-static forwarding.
//!
//! Laravel's `Model::__callStatic()` delegates static calls to
//! `static::query()`, which returns an Eloquent Builder.  This module
//! loads the Builder class, fully resolves it (including `@mixin`
//! `Query\Builder` members), and converts each public instance method
//! into a static virtual method on the model.
//!
//! Return type mapping:
//! - `static`, `$this`, `self` → `\Illuminate\Database\Eloquent\Builder<ConcreteModel>`
//!   (the chain continues on the builder, not the model).
//! - Template parameters (e.g. `TModel`) → the concrete model class name.
//!
//! Methods whose name starts with `__` (magic methods) are skipped.

use std::collections::HashMap;
use std::sync::Arc;

use crate::inheritance::apply_substitution_to_conditional;
use crate::php_type::PhpType;
use crate::types::{ClassInfo, ELOQUENT_COLLECTION_FQN, MethodInfo, Visibility};
use crate::virtual_members::ResolvedClassCache;

use super::ELOQUENT_BUILDER_FQN;

/// Replace `\Illuminate\Database\Eloquent\Collection` with a custom
/// collection class in a [`PhpType`], preserving generic parameters.
pub(super) fn replace_eloquent_collection_typed(ty: &PhpType, custom_collection: &str) -> PhpType {
    replace_collection_in_type(ty, custom_collection)
}

/// Recursively walk a `PhpType` tree and replace any `Generic` whose
/// base name is the Eloquent Collection FQN with `custom_collection`.
fn replace_collection_in_type(ty: &PhpType, custom_collection: &str) -> PhpType {
    match ty {
        PhpType::Generic(name, args) if name == ELOQUENT_COLLECTION_FQN => {
            let new_args = args
                .iter()
                .map(|a| replace_collection_in_type(a, custom_collection))
                .collect();
            PhpType::Generic(custom_collection.to_string(), new_args)
        }
        PhpType::Generic(name, args) => {
            let new_args = args
                .iter()
                .map(|a| replace_collection_in_type(a, custom_collection))
                .collect();
            PhpType::Generic(name.clone(), new_args)
        }
        PhpType::Union(members) => PhpType::Union(
            members
                .iter()
                .map(|m| replace_collection_in_type(m, custom_collection))
                .collect(),
        ),
        PhpType::Intersection(members) => PhpType::Intersection(
            members
                .iter()
                .map(|m| replace_collection_in_type(m, custom_collection))
                .collect(),
        ),
        PhpType::Nullable(inner) => PhpType::Nullable(Box::new(replace_collection_in_type(
            inner,
            custom_collection,
        ))),
        PhpType::Array(inner) => PhpType::Array(Box::new(replace_collection_in_type(
            inner,
            custom_collection,
        ))),
        // Named types, scalars, etc. — no collection to replace.
        other => other.clone(),
    }
}

/// Build static virtual methods by forwarding Eloquent Builder's public
/// instance methods onto the model class.
///
/// Laravel's `Model::__callStatic()` delegates static calls to
/// `static::query()`, which returns a `Builder<static>`.  This function
/// loads the Builder class, fully resolves it (including `@mixin`
/// `Query\Builder` members), and converts each public instance method
/// into a static virtual method on the model.
///
/// Return type mapping:
/// - `static`, `$this`, `self` → `\Illuminate\Database\Eloquent\Builder<ConcreteModel>`
///   (the chain continues on the builder, not the model).
/// - Template parameters (e.g. `TModel`) → the concrete model class name.
///
/// Methods whose name starts with `__` (magic methods) are skipped.
pub(super) fn build_builder_forwarded_methods(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    _cache: Option<&ResolvedClassCache>,
) -> Vec<MethodInfo> {
    // Load the Eloquent Builder class.
    let builder_class = match class_loader(ELOQUENT_BUILDER_FQN) {
        Some(c) => c,
        None => return Vec::new(),
    };

    // Fully resolve Builder (own + traits + parents + virtual members
    // including @mixin Query\Builder).  This is safe because Builder
    // does not extend Model, so the LaravelModelProvider will not
    // recurse.
    //
    // Use the uncached variant here.  The cache is keyed by
    // (FQN, generic_args), but the base Builder resolved here has
    // empty generic args.  If we stored it in the cache, later code
    // paths that call `resolve_class_fully_cached` on a Builder
    // candidate (e.g. `build_union_completion_items`) would get this
    // pre-scope-injection version back instead of computing a fresh
    // resolution.  Scope methods are model-specific and injected at
    // a higher layer (`try_inject_builder_scopes` in type resolution),
    // so the base Builder must not be cached here.
    //
    // The top-level `resolve_class_fully_cached` call on the model
    // class already caches the final merged result (including these
    // forwarded methods), so the per-model cost is paid only once.
    let resolved_builder =
        crate::virtual_members::resolve_class_fully(&builder_class, class_loader);

    // Build a substitution map: TModel → concrete model class name,
    // and static/$this/self → Builder<ConcreteModel>.
    let builder_self_type = PhpType::Generic(
        ELOQUENT_BUILDER_FQN.to_string(),
        vec![PhpType::Named(class.name.clone())],
    );
    let mut subs = HashMap::new();
    for param in &builder_class.template_params {
        subs.insert(param.clone(), PhpType::Named(class.name.clone()));
    }
    subs.insert("static".to_string(), builder_self_type.clone());
    subs.insert("$this".to_string(), builder_self_type.clone());
    subs.insert("self".to_string(), builder_self_type);

    let mut methods = Vec::new();

    for method in &resolved_builder.methods {
        if method.visibility != Visibility::Public {
            continue;
        }
        // Skip magic methods (__construct, __call, etc.).
        if method.name.starts_with("__") {
            continue;
        }
        // Skip methods already present on the model (real methods,
        // scope methods, etc.).  The merge logic in
        // `merge_virtual_members` would also skip them, but filtering
        // here avoids unnecessary cloning and substitution work.
        if class
            .methods
            .iter()
            .any(|m| m.name == method.name && m.is_static)
        {
            continue;
        }

        let mut forwarded = method.clone();
        forwarded.is_static = true;

        // Apply template and self-type substitutions.
        if !subs.is_empty() {
            if let Some(ref mut ret) = forwarded.return_type {
                *ret = ret.substitute(&subs);
            }
            if let Some(ref mut cond) = forwarded.conditional_return {
                apply_substitution_to_conditional(cond, &subs);
            }
            for param in &mut forwarded.parameters {
                if let Some(ref mut hint) = param.type_hint {
                    *hint = hint.substitute(&subs);
                }
            }
        }

        // Replace Eloquent Collection with custom collection class.
        if let Some(coll) = class.laravel().and_then(|l| l.custom_collection.as_ref())
            && let Some(ref mut ret) = forwarded.return_type
        {
            *ret = replace_eloquent_collection_typed(ret, coll);
        }

        methods.push(forwarded);
    }

    methods
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
