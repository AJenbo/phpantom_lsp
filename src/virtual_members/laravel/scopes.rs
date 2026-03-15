//! Eloquent scope detection, name transformation, and builder scope
//! synthesis.
//!
//! This module handles both convention-based (`scopeX`) and
//! attribute-based (`#[Scope]`, Laravel 11+) scope methods, building
//! virtual instance and static methods with the `scope` prefix stripped
//! and the first `$query` parameter removed.

use std::collections::HashMap;
use std::sync::Arc;

use crate::inheritance::apply_substitution;
use crate::types::{ClassInfo, MethodInfo};

use super::helpers::extends_eloquent_model;

/// The default return type for scope methods that don't declare a return
/// type or return `void`.
const DEFAULT_SCOPE_RETURN_TYPE: &str = "Illuminate\\Database\\Eloquent\\Builder<static>";

/// Determine whether a method is an Eloquent scope.
///
/// Scopes are methods whose name starts with `scope` (case-sensitive)
/// and have at least five characters (the prefix plus at least one
/// character for the scope name).  For example, `scopeActive` is a
/// scope, but `scope` alone is not.
///
/// Also returns `true` for methods decorated with `#[Scope]`
/// (Laravel 11+), regardless of their name.
pub(super) fn is_scope_method(method: &MethodInfo) -> bool {
    method.has_scope_attribute || (method.name.starts_with("scope") && method.name.len() > 5)
}

/// Returns `true` when the method uses the `#[Scope]` attribute
/// rather than the `scopeX` naming convention.
pub(super) fn is_attribute_scope(method: &MethodInfo) -> bool {
    method.has_scope_attribute
}

/// Transform a scope method name into the public-facing scope name.
///
/// For `scopeX`-style methods, strips the `scope` prefix and
/// lowercases the first character: `scopeActive` → `active`.
///
/// For `#[Scope]`-attributed methods, returns the method's own name
/// unchanged (it is already the public-facing name).
pub(super) fn scope_name_for(method: &MethodInfo) -> String {
    if is_attribute_scope(method) {
        method.name.clone()
    } else {
        scope_name(&method.name)
    }
}

/// Transform a `scopeX` method name into the public-facing scope name.
///
/// Strips the `scope` prefix and lowercases the first character:
/// `scopeActive` → `active`, `scopeVerified` → `verified`.
pub(super) fn scope_name(method_name: &str) -> String {
    let after_prefix = &method_name[5..]; // skip "scope"
    let mut chars = after_prefix.chars();
    match chars.next() {
        Some(c) => {
            let lower: String = c.to_lowercase().collect();
            format!("{lower}{}", chars.as_str())
        }
        None => String::new(),
    }
}

/// Determine the return type for a synthesized scope method.
///
/// Uses the scope method's declared return type.  If the return type is
/// `void` or absent, defaults to
/// `\Illuminate\Database\Eloquent\Builder<static>`.
pub(super) fn scope_return_type(method: &MethodInfo) -> String {
    match method.return_type.as_deref() {
        Some("void") | None => DEFAULT_SCOPE_RETURN_TYPE.to_string(),
        Some(rt) => rt.to_string(),
    }
}

/// Build virtual methods for a scope method.
///
/// Returns two `MethodInfo` values: one static and one instance.  Both
/// have the `scope` prefix stripped (or keep the original name for
/// `#[Scope]`-attributed methods), and the first `$query` parameter
/// removed.  This makes scope methods accessible via both
/// `User::active()` (static) and `$user->active()` (instance).
pub(super) fn build_scope_methods(method: &MethodInfo) -> [MethodInfo; 2] {
    let name = scope_name_for(method);
    let return_type = Some(scope_return_type(method));

    // Strip the first parameter ($query / $builder) that Laravel injects.
    let parameters: Vec<_> = if method.parameters.is_empty() {
        Vec::new()
    } else {
        method.parameters[1..].to_vec()
    };

    let instance_method = MethodInfo {
        parameters: parameters.clone(),
        deprecation_message: method.deprecation_message.clone(),
        ..MethodInfo::virtual_method(&name, return_type.as_deref())
    };

    let static_method = MethodInfo {
        parameters,
        is_static: true,
        deprecation_message: method.deprecation_message.clone(),
        ..MethodInfo::virtual_method(&name, return_type.as_deref())
    };

    [instance_method, static_method]
}

/// Inject scope methods from a concrete model onto a resolved Builder.
///
/// When a type resolves to `Builder<User>`, the generic substitution
/// replaces `TModel` with `User` but does not add `User`'s scope
/// methods.  This function loads the concrete model, scans for scope
/// methods, and returns them as **instance** methods on the Builder so
/// that `$query->active()` and `Brand::where(...)->isActive()` both
/// resolve.
///
/// Return types are mapped so that `static` (from the default scope
/// return type `Builder<static>`) becomes `Builder<ConcreteModel>`,
/// keeping the chain on the Builder rather than jumping to the model.
pub fn build_scope_methods_for_builder(
    model_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<MethodInfo> {
    let model_class = match class_loader(model_name) {
        Some(c) => c,
        None => return Vec::new(),
    };

    // Only synthesize scopes for actual Eloquent models.
    if !extends_eloquent_model(&model_class, class_loader) {
        return Vec::new();
    }

    // Resolve the model with inheritance (traits + parent chain) but
    // WITHOUT virtual member providers.  Virtual providers transform
    // #[Scope] methods into their public-facing form (replacing the
    // original), which makes them invisible to `is_scope_method`.
    // Using the pre-provider resolution preserves the raw methods.
    let resolved_model =
        crate::inheritance::resolve_class_with_inheritance(&model_class, class_loader);

    // Build a substitution map so that `static`, `$this`, and `self`
    // in scope return types resolve to the concrete model name.
    // The default scope return type is `\...\Builder<static>` where
    // `static` means the model, so substituting `static` → `User`
    // produces `\...\Builder<User>`, keeping the chain on the builder.
    let mut subs = HashMap::new();
    subs.insert("static".to_string(), model_name.to_string());
    subs.insert("$this".to_string(), model_name.to_string());
    subs.insert("self".to_string(), model_name.to_string());

    let mut methods = Vec::new();

    for method in &resolved_model.methods {
        if !is_scope_method(method) {
            continue;
        }

        // Build an instance method (scopes are called as instance
        // methods on Builder, not static).  For `#[Scope]`-attributed
        // methods the name is used as-is; for `scopeX` methods the
        // prefix is stripped.
        let [instance_method, _static_method] = build_scope_methods(method);

        let mut m = instance_method;

        // Apply substitutions to the return type.
        if let Some(ref mut ret) = m.return_type {
            *ret = apply_substitution(ret, &subs).into_owned();
        }

        methods.push(m);
    }

    methods
}

#[cfg(test)]
#[path = "scopes_tests.rs"]
mod tests;
