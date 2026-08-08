//! The variables the class backing a component view puts in its scope.
//!
//! Blade merges a class component's public properties and its public
//! zero-argument methods into the data its view renders with, and Livewire
//! hands its view the component instance (as `$this`, `$_instance`, and
//! `$__livewire`) plus the component's public properties. None of that is
//! written in the template and no caller passes it, so a component body
//! that reads a member of its own class reports it undefined unless the
//! backing class is resolved and its members fed into the declaration
//! chain documented in [`super::signature`].
//!
//! The class is found the way Laravel finds it: over the class-component
//! namespaces a service provider registered, the default
//! `App\View\Components` convention (`components.alert` →
//! `App\View\Components\Alert`), and, for Livewire, the configured
//! `livewire.class_namespace` (`livewire.create-refund` →
//! `App\Livewire\CreateRefund`).
//!
//! Members declared on the framework base class are plumbing rather than
//! view data, so they are skipped, as is any method that requires an
//! argument: Blade only exposes the argument-less ones as variables.

use std::sync::Arc;

use crate::Backend;
use crate::atom::Atom;
use crate::php_type::PhpType;
use crate::types::{ClassInfo, Visibility};

use super::call_site_inference::InjectedVars;

/// The base class every Blade class component extends.
const COMPONENT_BASE: &str = "Illuminate\\View\\Component";

/// The base class every Livewire component extends.
const LIVEWIRE_BASE: &str = "Livewire\\Component";

/// Where Livewire looks for component classes when the application does
/// not configure `livewire.class_namespace`.
const LIVEWIRE_DEFAULT_NAMESPACE: &str = "App\\Livewire";

/// What Blade wraps an argument-less public method in before merging it
/// into the view data.
///
/// This is the runtime type, not the method's return type: the wrapper is
/// what makes both `{{ $total }}` and `{{ $total() }}` work, and naming
/// the return type instead would report the first of those as an error.
const INVOKABLE_VARIABLE: &str = "\\Illuminate\\View\\InvokableComponentVariable";

/// The names Livewire binds the component instance to in its view.
///
/// Livewire binds it to `$this` as well, which no declaration here can
/// cover: a variable arrives by being assigned in the virtual PHP's
/// prologue and pulled into the wrapper function with `global`, and PHP
/// allows neither for `$this`.
const LIVEWIRE_INSTANCE_VARS: [&str; 2] = ["_instance", "__livewire"];

impl Backend {
    /// The variables the backing class of a component view contributes,
    /// or an empty list when no view name resolves to one.
    ///
    /// Only the first name that resolves contributes: a template is one
    /// component, even when several view roots make it addressable by
    /// more than one name.
    pub(crate) fn blade_backing_class_vars(&self, view_names: &[String]) -> InjectedVars {
        for view_name in view_names {
            if let Some(class) = self.livewire_component_class(view_name) {
                return self.livewire_scope_vars(&class);
            }
            if let Some(class) = self.blade_component_class(view_name) {
                return self.component_scope_vars(&class);
            }
        }
        Vec::new()
    }

    /// The Blade component class a view name is the view of, over the
    /// registered class-component namespaces and then the default
    /// `App\View\Components` convention.
    fn blade_component_class(&self, view_name: &str) -> Option<Arc<ClassInfo>> {
        // The discovery index knows the class each tag name is backed by,
        // including the ones no name transform predicts — an index
        // component (`components.card` → `App\View\Components\Card\Card`)
        // is only findable by having seen the file.
        if let Some(tag) = super::component_tags::component_tag_for_view_name(view_name)
            && let Some(fqn) = self.blade_component_fqn(&tag)
            && let Some(class) = self.component_class_named(&fqn, COMPONENT_BASE)
        {
            return Some(class);
        }

        let namespaces = self
            .laravel_provider_resources
            .read()
            .class_component_namespaces
            .clone();
        for (prefix, namespace) in &namespaces {
            let Some(rest) = view_name
                .strip_prefix(prefix.as_str())
                .and_then(|rest| rest.strip_prefix("::"))
            else {
                continue;
            };
            // A package that keeps its component views under the same
            // `components.` convention the application uses addresses them
            // as `prefix::components.name`, but the class still sits
            // directly under the registered namespace.
            let candidates = [rest.strip_prefix("components."), Some(rest)];
            for candidate in candidates.into_iter().flatten() {
                if let Some(class) = self.component_class_at(namespace, candidate, COMPONENT_BASE) {
                    return Some(class);
                }
            }
        }

        let rest = view_name.strip_prefix("components.")?;
        let namespace = format!("{}View\\Components", self.application_namespace());
        self.component_class_at(&namespace, rest, COMPONENT_BASE)
    }

    /// The Livewire component class a `livewire.…` view name belongs to.
    fn livewire_component_class(&self, view_name: &str) -> Option<Arc<ClassInfo>> {
        let rest = view_name.strip_prefix("livewire.")?;
        if let Some(fqn) = self.livewire_component_fqn(rest)
            && let Some(class) = self.component_class_named(&fqn, LIVEWIRE_BASE)
        {
            return Some(class);
        }
        let namespace = self.livewire_class_namespace();
        self.component_class_at(&namespace, rest, LIVEWIRE_BASE)
    }

    /// Load `namespace\Component\Name` for the dotted tail of a view name,
    /// provided it exists and really is a component.
    fn component_class_at(
        &self,
        namespace: &str,
        dotted: &str,
        base: &str,
    ) -> Option<Arc<ClassInfo>> {
        let fqn = format!(
            "{}\\{}",
            namespace.trim_matches('\\'),
            class_name_for_view_tail(dotted)?
        );
        self.component_class_named(&fqn, base)
    }

    /// Load `fqn`, provided it exists and really is a component.
    fn component_class_named(&self, fqn: &str, base: &str) -> Option<Arc<ClassInfo>> {
        let fqn = fqn.trim_matches('\\');
        let class = self.find_or_load_class(fqn)?;
        // A miss can still land on a class of the same short name, which
        // would put a stranger's members in the template's scope.
        if !class.fqn().eq_ignore_ascii_case(fqn) {
            return None;
        }
        let loader = |name: &str| self.find_or_load_class(name);
        if !crate::type_engine::variable::forward_walk::is_subclass_of(fqn, base, &loader) {
            return None;
        }
        Some(class)
    }

    /// The application's root namespace, ending with `\` — the PSR-4 prefix
    /// mapped to `app/`, as Laravel's own `Application::getNamespace()`
    /// reads it from `composer.json`.
    pub(crate) fn application_namespace(&self) -> String {
        self.psr4_mappings()
            .read()
            .iter()
            .find(|mapping| {
                let dir = mapping
                    .base_path
                    .trim_start_matches("./")
                    .trim_end_matches('/');
                dir == "app"
            })
            .map(|mapping| mapping.prefix.clone())
            .unwrap_or_else(|| "App\\".to_string())
    }

    /// The namespace Livewire resolves component classes in.
    ///
    /// `config/livewire.php` is read straight from the workspace (through
    /// an open buffer when there is one) rather than through the cached
    /// config trees: this runs while a template is being opened, and that
    /// cache is built from the workspace index, which a keystroke must
    /// never wait on.
    pub(crate) fn livewire_class_namespace(&self) -> String {
        use crate::virtual_members::laravel::config_values::{ConfigValue, parse_config_tree};

        let Some(root) = self.workspace_root().read().clone() else {
            return LIVEWIRE_DEFAULT_NAMESPACE.to_string();
        };
        let path = root.join("config").join("livewire.php");
        let content = tower_lsp::lsp_types::Url::from_file_path(&path)
            .ok()
            .and_then(|uri| self.get_file_content(uri.as_str()));
        let namespace = content
            .as_deref()
            .and_then(parse_config_tree)
            .and_then(|tree| match tree.value_at(&["class_namespace"]) {
                Some(ConfigValue::Str(namespace)) => Some(namespace.clone()),
                _ => None,
            });
        // A single-quoted namespace keeps its doubled separators in source.
        namespace
            .map(|namespace| namespace.replace("\\\\", "\\"))
            .unwrap_or_else(|| LIVEWIRE_DEFAULT_NAMESPACE.to_string())
    }

    /// A Blade component's scope: its public properties and its public
    /// argument-less methods.
    fn component_scope_vars(&self, class: &ClassInfo) -> InjectedVars {
        self.class_member_vars(class, COMPONENT_BASE, true)
    }

    /// A Livewire component's scope: its public properties, plus the
    /// component instance under each of the names Livewire binds it to.
    ///
    /// Public methods are Livewire *actions* (what `wire:click` calls),
    /// not view data, so they are reached through the typed instance
    /// rather than being variables of their own.
    fn livewire_scope_vars(&self, class: &ClassInfo) -> InjectedVars {
        let mut vars = self.class_member_vars(class, LIVEWIRE_BASE, false);
        let instance_type = format!("\\{}", class.fqn());
        for name in LIVEWIRE_INSTANCE_VARS {
            vars.push((name.to_string(), instance_type.clone()));
        }
        vars
    }

    /// The public members `class` exposes to its view, skipping everything
    /// the framework base class declares.
    fn class_member_vars(
        &self,
        class: &ClassInfo,
        base: &str,
        include_methods: bool,
    ) -> InjectedVars {
        let loader = |name: &str| self.find_or_load_class(name);
        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            class,
            &loader,
            Some(&self.resolved_class_cache),
        );
        let framework_members = self.framework_member_names(base);
        let exposed = |name: &Atom| {
            !name.starts_with("__") && !framework_members.iter().any(|known| known == name)
        };

        let mut vars: InjectedVars = Vec::new();
        for property in resolved.properties.iter() {
            if property.is_static
                || property.visibility != Visibility::Public
                || !exposed(&property.name)
            {
                continue;
            }
            vars.push((
                property.name.to_string(),
                self.docblock_type(property.type_hint.as_ref()),
            ));
        }

        if !include_methods {
            return vars;
        }

        for method in resolved.methods.iter() {
            if method.is_static
                || method.is_abstract
                || method.visibility != Visibility::Public
                || !exposed(&method.name)
                // A method that takes an argument is handed to the view as a
                // plain closure, which no template can call without knowing
                // what to pass, so Blade users never read it as a variable.
                || method.parameters.iter().any(|param| param.is_required)
            {
                continue;
            }
            vars.push((method.name.to_string(), INVOKABLE_VARIABLE.to_string()));
        }

        vars
    }

    /// The member names the framework base class declares, which a
    /// component inherits but never exposes as view data.
    ///
    /// Laravel skips these by name (`Component::shouldIgnore()`), so a
    /// component that *overrides* `render()` hides it just the same.
    fn framework_member_names(&self, base: &str) -> Vec<Atom> {
        let Some(base_class) = self.find_or_load_class(base) else {
            return Vec::new();
        };
        let loader = |name: &str| self.find_or_load_class(name);
        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            &base_class,
            &loader,
            Some(&self.resolved_class_cache),
        );
        resolved
            .properties
            .iter()
            .map(|property| property.name)
            .chain(resolved.methods.iter().map(|method| method.name))
            .collect()
    }

    /// A member's type as a docblock string, with class names fully
    /// qualified so it resolves from the template's namespace-less scope.
    fn docblock_type(&self, ty: Option<&PhpType>) -> String {
        let Some(ty) = ty else {
            return "mixed".to_string();
        };
        ty.resolve_names(&|name: &str| match self.find_or_load_class(name) {
            Some(class) => format!("\\{}", class.fqn()),
            None => name.to_string(),
        })
        .to_string()
    }
}

/// The class-name tail a dotted view name maps to: `forms.date-picker`
/// becomes `Forms\DatePicker`, matching Laravel's own
/// `ComponentTagCompiler::formatClassName()`.
///
/// `None` when a segment holds a character a class name cannot, so the
/// lookup is skipped rather than built from garbage.
fn class_name_for_view_tail(dotted: &str) -> Option<String> {
    let mut out = String::with_capacity(dotted.len());
    for segment in dotted.split('.') {
        if segment.is_empty() {
            return None;
        }
        if !out.is_empty() {
            out.push('\\');
        }
        let mut capitalise_next = true;
        for ch in segment.chars() {
            if ch == '-' || ch == '_' {
                capitalise_next = true;
                continue;
            }
            if !ch.is_ascii_alphanumeric() {
                return None;
            }
            if capitalise_next {
                out.extend(ch.to_uppercase());
                capitalise_next = false;
            } else {
                out.push(ch);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_tail_becomes_a_studly_class_path() {
        assert_eq!(class_name_for_view_tail("alert").as_deref(), Some("Alert"));
        assert_eq!(
            class_name_for_view_tail("forms.date-picker").as_deref(),
            Some("Forms\\DatePicker")
        );
        assert_eq!(
            class_name_for_view_tail("create_refund").as_deref(),
            Some("CreateRefund")
        );
        // An already-camelCased segment keeps its inner capitals.
        assert_eq!(
            class_name_for_view_tail("userProfile").as_deref(),
            Some("UserProfile")
        );
    }

    #[test]
    fn a_tail_that_cannot_be_a_class_name_resolves_to_nothing() {
        assert_eq!(class_name_for_view_tail(""), None);
        assert_eq!(class_name_for_view_tail("alert."), None);
        assert_eq!(class_name_for_view_tail("mail::message"), None);
    }

    /// A package registers its component classes under a tag prefix, and
    /// the views it publishes under that prefix are backed by them.
    #[test]
    fn a_registered_namespace_backs_the_views_under_its_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let write = |relative: &str, content: &str| {
            let path = dir.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        };
        write(
            "composer.json",
            r#"{"autoload": {"psr-4": {
                "Nightshade\\": "packages/nightshade/src/",
                "Illuminate\\": "stubs/Illuminate/"
            }}}"#,
        );
        write(
            "stubs/Illuminate/View/Component.php",
            "<?php\nnamespace Illuminate\\View;\n\
             abstract class Component { public function render() {} }\n",
        );
        write(
            "packages/nightshade/src/Views/Components/Calendar.php",
            "<?php\nnamespace Nightshade\\Views\\Components;\n\
             use Illuminate\\View\\Component;\n\
             class Calendar extends Component {\n\
                 public string $month = '';\n\
                 public function render() {}\n\
             }\n",
        );

        let (mappings, _) = crate::composer::parse_composer_json(dir.path());
        let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);
        backend
            .laravel_provider_resources
            .write()
            .class_component_namespaces
            .push((
                "nightshade".to_string(),
                "Nightshade\\Views\\Components".to_string(),
            ));

        let declares_month = |view_name: &str| {
            backend
                .blade_backing_class_vars(&[view_name.to_string()])
                .iter()
                .any(|(name, ty)| name == "month" && ty == "string")
        };
        assert!(declares_month("nightshade::calendar"));
        // A package that keeps its component views under the same
        // `components.` convention the application uses.
        assert!(declares_month("nightshade::components.calendar"));
        assert!(!declares_month("nightshade::missing"));
    }
}
