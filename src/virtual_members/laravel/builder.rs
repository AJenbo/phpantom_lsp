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

use crate::inheritance::{apply_substitution, apply_substitution_to_conditional};
use crate::types::{ClassInfo, ELOQUENT_COLLECTION_FQN, MethodInfo, Visibility};
use crate::virtual_members::ResolvedClassCache;

use super::ELOQUENT_BUILDER_FQN;

/// Replace `\Illuminate\Database\Eloquent\Collection` with a custom
/// collection class in a type string, preserving generic parameters.
pub(super) fn replace_eloquent_collection(type_str: &str, custom_collection: &str) -> String {
    type_str.replace(ELOQUENT_COLLECTION_FQN, custom_collection)
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
    let builder_self_type = format!("{ELOQUENT_BUILDER_FQN}<{}>", class.name);
    let mut subs = HashMap::new();
    for param in &builder_class.template_params {
        subs.insert(param.clone(), class.name.clone());
    }
    subs.insert("static".to_string(), builder_self_type.clone());
    subs.insert("$this".to_string(), builder_self_type.clone());
    subs.insert("self".to_string(), builder_self_type.clone());

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
                *ret = apply_substitution(ret, &subs).into_owned();
            }
            if let Some(ref mut cond) = forwarded.conditional_return {
                apply_substitution_to_conditional(cond, &subs);
            }
            for param in &mut forwarded.parameters {
                if let Some(ref mut hint) = param.type_hint {
                    *hint = apply_substitution(hint, &subs).into_owned();
                }
            }
        }

        // Replace Eloquent Collection with custom collection class.
        if let Some(coll) = class.laravel().and_then(|l| l.custom_collection.as_ref())
            && let Some(ref mut ret) = forwarded.return_type
        {
            *ret = replace_eloquent_collection(ret, coll);
        }

        methods.push(forwarded);
    }

    methods
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
