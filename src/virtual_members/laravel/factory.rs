//! Laravel Eloquent Factory virtual member provider.
//!
//! Synthesizes `create()` and `make()` methods for factory classes that
//! extend `Illuminate\Database\Eloquent\Factories\Factory` but do not
//! already have `@extends Factory<Model>` generics.  The model type is
//! derived from the naming convention (e.g.
//! `Database\Factories\UserFactory` → `App\Models\User`).

use crate::types::{ClassInfo, MethodInfo};
use std::sync::Arc;

use super::helpers::walks_parent_chain;

use super::super::{VirtualMemberProvider, VirtualMembers};

/// The fully-qualified name of the `Factory` base class.
const FACTORY_FQN: &str = "Illuminate\\Database\\Eloquent\\Factories\\Factory";

/// Derive the conventional factory FQN from a model FQN.
///
/// Follows Laravel's default convention:
/// - `App\Models\User` → `Database\Factories\UserFactory`
/// - `App\Models\Admin\SuperUser` → `Database\Factories\Admin\SuperUserFactory`
///
/// The rule: strip the `Models\` segment from the namespace, replace
/// the root with `Database\Factories\`, and append `Factory` to the
/// class short name.
pub(crate) fn model_to_factory_fqn(model_fqn: &str) -> String {
    // Split into namespace + short name.
    let (ns, short) = match model_fqn.rsplit_once('\\') {
        Some((ns, short)) => (ns, short),
        None => return format!("Database\\Factories\\{model_fqn}Factory"),
    };

    // Check for `X\Models\Sub` pattern → `Database\Factories\Sub`
    if let Some((_prefix, suffix)) = ns.split_once("\\Models\\") {
        return format!("Database\\Factories\\{suffix}\\{short}Factory");
    }

    // Check for `X\Models` pattern (model directly in Models namespace)
    if ns.ends_with("\\Models") || ns == "Models" {
        return format!("Database\\Factories\\{short}Factory");
    }

    // No `Models` segment — put factory in `Database\Factories`
    format!("Database\\Factories\\{short}Factory")
}

/// Derive the conventional model FQN from a factory FQN.
///
/// Reverse of [`model_to_factory_fqn`]:
/// - `Database\Factories\UserFactory` → `App\Models\User`
/// - `Database\Factories\Admin\SuperUserFactory` → `App\Models\Admin\SuperUser`
pub(crate) fn factory_to_model_fqn(factory_fqn: &str) -> Option<String> {
    // The short name must end with `Factory`.
    let short = factory_fqn.rsplit('\\').next().unwrap_or(factory_fqn);
    let model_short = short.strip_suffix("Factory")?;
    if model_short.is_empty() {
        return None;
    }

    // Extract the namespace after `Database\Factories\`.
    let ns = factory_fqn
        .rsplit_once('\\')
        .map(|(ns, _)| ns)
        .unwrap_or("");

    let sub_ns = if let Some(after) = ns.strip_prefix("Database\\Factories\\") {
        Some(after)
    } else if ns == "Database\\Factories" {
        None
    } else {
        // Not in the standard factory namespace — still try to strip
        // any `Factories` segment.
        None
    };

    match sub_ns {
        Some(sub) => Some(format!("App\\Models\\{sub}\\{model_short}")),
        None => Some(format!("App\\Models\\{model_short}")),
    }
}

/// Determine whether `class_name` is the Eloquent Factory base class.
fn is_eloquent_factory(class_name: &str) -> bool {
    class_name == FACTORY_FQN
}

/// Walk the parent chain of `class` looking for
/// `Illuminate\Database\Eloquent\Factories\Factory`.
///
/// Returns `true` if the class itself is `Factory` or any ancestor is.
fn extends_eloquent_factory(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    walks_parent_chain(class, class_loader, is_eloquent_factory)
}

/// Check whether a factory class already has `@extends Factory<Model>`
/// that would let the generics system resolve `TModel`.
fn has_factory_extends_generic(class: &ClassInfo) -> bool {
    class.extends_generics.iter().any(|(name, args)| {
        let short = name.rsplit('\\').next().unwrap_or(name);
        short == "Factory" && !args.is_empty()
    })
}

/// Build virtual `create()` and `make()` methods for a factory class
/// that does not have `@extends Factory<Model>`.
///
/// The model type is derived from the naming convention (e.g.
/// `Database\Factories\UserFactory` → `App\Models\User`).
fn build_factory_model_methods(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<MethodInfo> {
    let model_fqn = match factory_to_model_fqn(&class.name) {
        Some(fqn) => fqn,
        None => return Vec::new(),
    };

    // Verify the model class actually exists.
    if class_loader(&model_fqn).is_none() {
        return Vec::new();
    }

    let model_type = model_fqn.to_string();

    vec![
        MethodInfo::virtual_method("create", Some(&model_type)),
        MethodInfo::virtual_method("make", Some(&model_type)),
    ]
}

/// Virtual member provider for Laravel Eloquent factories.
///
/// When a class extends `Illuminate\Database\Eloquent\Factories\Factory`
/// (directly or through an intermediate parent) and does not already
/// have `@extends Factory<Model>` generics, this provider synthesizes
/// `create()` and `make()` methods that return the model type derived
/// from the naming convention.
pub struct LaravelFactoryProvider;

impl VirtualMemberProvider for LaravelFactoryProvider {
    /// Returns `true` if the class extends
    /// `Illuminate\Database\Eloquent\Factories\Factory` and does not
    /// already have `@extends Factory<Model>` generics.
    fn applies_to(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> bool {
        !is_eloquent_factory(&class.name)
            && !has_factory_extends_generic(class)
            && extends_eloquent_factory(class, class_loader)
    }

    /// Synthesize `create()` and `make()` methods that return the model
    /// type derived from the naming convention.
    fn provide(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        _cache: Option<&crate::virtual_members::ResolvedClassCache>,
    ) -> VirtualMembers {
        let methods = build_factory_model_methods(class, class_loader);
        VirtualMembers {
            methods,
            properties: Vec::new(),
            constants: Vec::new(),
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
