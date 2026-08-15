use super::*;
use crate::atom::atom;
use crate::inheritance::resolve_class_with_inheritance;
use crate::php_type::PhpType;
use crate::test_fixtures::{make_class, make_method, no_loader};
use std::sync::Arc;

// ── model_to_factory_fqn tests ──────────────────────────────────────

#[test]
fn model_to_factory_standard() {
    assert_eq!(
        model_to_factory_fqn("App\\Models\\User"),
        "Database\\Factories\\UserFactory"
    );
}

#[test]
fn model_to_factory_subdirectory() {
    assert_eq!(
        model_to_factory_fqn("App\\Models\\Admin\\SuperUser"),
        "Database\\Factories\\Admin\\SuperUserFactory"
    );
}

#[test]
fn model_to_factory_no_models_segment() {
    assert_eq!(
        model_to_factory_fqn("App\\User"),
        "Database\\Factories\\UserFactory"
    );
}

#[test]
fn model_to_factory_bare_name() {
    assert_eq!(
        model_to_factory_fqn("User"),
        "Database\\Factories\\UserFactory"
    );
}

#[test]
fn model_to_factory_models_only_namespace() {
    assert_eq!(
        model_to_factory_fqn("Models\\Post"),
        "Database\\Factories\\PostFactory"
    );
}

// ── factory_to_model_fqn tests ──────────────────────────────────────

#[test]
fn factory_to_model_standard() {
    assert_eq!(
        factory_to_model_fqn("Database\\Factories\\UserFactory"),
        Some("App\\Models\\User".to_string())
    );
}

#[test]
fn factory_to_model_subdirectory() {
    assert_eq!(
        factory_to_model_fqn("Database\\Factories\\Admin\\SuperUserFactory"),
        Some("App\\Models\\Admin\\SuperUser".to_string())
    );
}

#[test]
fn factory_to_model_no_factory_suffix() {
    assert_eq!(
        factory_to_model_fqn("Database\\Factories\\UserBuilder"),
        None
    );
}

#[test]
fn factory_to_model_bare_factory() {
    // "Factory" alone has an empty model short name — should return None.
    assert_eq!(factory_to_model_fqn("Factory"), None);
}

// ── is_eloquent_factory / extends_eloquent_factory tests ────────────

#[test]
fn is_eloquent_factory_fqn() {
    assert!(is_eloquent_factory(FACTORY_FQN));
}

#[test]
fn is_eloquent_factory_rejects_unrelated() {
    assert!(!is_eloquent_factory("App\\Factories\\UserFactory"));
}

#[test]
fn extends_factory_direct() {
    let mut class = make_class("UserFactory");
    class.parent_class = Some(atom(FACTORY_FQN));
    assert!(extends_eloquent_factory(&class, &no_loader));
}

#[test]
fn extends_factory_indirect() {
    let mut class = make_class("UserFactory");
    class.parent_class = Some(atom("BaseFactory"));

    let mut base = make_class("BaseFactory");
    base.parent_class = Some(atom(FACTORY_FQN));

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "BaseFactory" {
            Some(Arc::new(base.clone()))
        } else {
            None
        }
    };
    assert!(extends_eloquent_factory(&class, &loader));
}

#[test]
fn does_not_extend_factory() {
    let class = make_class("SomeClass");
    assert!(!extends_eloquent_factory(&class, &no_loader));
}

// ── has_factory_extends_generic tests ────────────────────────────────

#[test]
fn has_factory_extends_generic_present() {
    let mut class = make_class("UserFactory");
    class.extends_generics = vec![(atom("Factory"), vec![PhpType::parse("User")])];
    assert!(has_factory_extends_generic(&class));
}

#[test]
fn has_factory_extends_generic_fqn() {
    let mut class = make_class("UserFactory");
    class.extends_generics = vec![(atom(FACTORY_FQN), vec![PhpType::parse("User")])];
    assert!(has_factory_extends_generic(&class));
}

#[test]
fn has_factory_extends_generic_not_present() {
    let class = make_class("UserFactory");
    assert!(!has_factory_extends_generic(&class));
}

#[test]
fn has_factory_extends_generic_empty_args() {
    let mut class = make_class("UserFactory");
    class.extends_generics = vec![(atom("Factory"), vec![])];
    assert!(!has_factory_extends_generic(&class));
}

fn generic_document_factory_classes() -> (ClassInfo, ClassInfo) {
    let mut document_factory = make_class("DocumentFactory");
    document_factory.file_namespace = Some(atom("Database\\Factories"));
    document_factory.parent_class = Some(atom(FACTORY_FQN));
    document_factory.template_params = vec![atom("TModel")];
    document_factory.template_param_bounds.insert(
        atom("TModel"),
        PhpType::named(atom("Illuminate\\Database\\Eloquent\\Model")),
    );
    document_factory.extends_generics =
        vec![(atom(FACTORY_FQN), vec![PhpType::named(atom("TModel"))])];

    let mut factory_base = make_class(FACTORY_FQN);
    factory_base.template_params = vec![atom("TModel")];
    factory_base
        .methods
        .push(Arc::new(make_method("makeOne", Some("TModel"))));

    (document_factory, factory_base)
}

#[test]
fn unbound_factory_generic_yields_to_declared_model() {
    let mut factory = make_class("DraftFactory");
    factory.file_namespace = Some(atom("Database\\Factories"));
    factory.parent_class = Some(atom("Database\\Factories\\DocumentFactory"));
    factory.laravel_mut().factory_model = Some(PhpType::named(atom("App\\Models\\Draft")));

    let (document_factory, factory_base) = generic_document_factory_classes();
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        match name {
            "Database\\Factories\\DocumentFactory" => Some(Arc::new(document_factory.clone())),
            FACTORY_FQN => Some(Arc::new(factory_base.clone())),
            _ => None,
        }
    };

    assert_eq!(
        factory_model_type(&factory, &loader).map(|model| model.to_string()),
        Some("App\\Models\\Draft".to_string())
    );

    let resolved = resolve_class_with_inheritance(&factory, &loader);
    assert_eq!(
        resolved
            .get_method_ci("makeOne")
            .and_then(|method| method.return_type_str())
            .as_deref(),
        Some("App\\Models\\Draft")
    );
}

#[test]
fn explicit_base_model_binding_remains_authoritative() {
    let mut factory = make_class("DraftFactory");
    factory.file_namespace = Some(atom("Database\\Factories"));
    factory.parent_class = Some(atom("Database\\Factories\\DocumentFactory"));
    factory.extends_generics = vec![(
        atom("Database\\Factories\\DocumentFactory"),
        vec![PhpType::named(atom(
            "Illuminate\\Database\\Eloquent\\Model",
        ))],
    )];
    factory.laravel_mut().factory_model = Some(PhpType::named(atom("App\\Models\\Draft")));

    let (document_factory, factory_base) = generic_document_factory_classes();
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        match name {
            "Database\\Factories\\DocumentFactory" => Some(Arc::new(document_factory.clone())),
            FACTORY_FQN => Some(Arc::new(factory_base.clone())),
            _ => None,
        }
    };

    assert_eq!(
        factory_model_type(&factory, &loader).map(|model| model.to_string()),
        Some("Illuminate\\Database\\Eloquent\\Model".to_string())
    );

    let resolved = resolve_class_with_inheritance(&factory, &loader);
    assert_eq!(
        resolved
            .get_method_ci("makeOne")
            .and_then(|method| method.return_type_str())
            .as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Model")
    );
}

// ── build_factory_model_methods tests ───────────────────────────────

#[test]
fn build_factory_model_methods_synthesizes_create_and_make() {
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let model = make_class("App\\Models\\User");
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let methods = build_factory_model_methods(&factory, &loader);
    assert_eq!(methods.len(), 2);

    let create = methods.iter().find(|m| m.name == "create").unwrap();
    assert!(!create.is_static);
    assert_eq!(
        create.return_type_str().as_deref(),
        Some("App\\Models\\User")
    );

    let make = methods.iter().find(|m| m.name == "make").unwrap();
    assert!(!make.is_static);
    assert_eq!(make.return_type_str().as_deref(), Some("App\\Models\\User"));
}

#[test]
fn build_factory_model_methods_returns_empty_when_model_missing() {
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let methods = build_factory_model_methods(&factory, &no_loader);
    assert!(methods.is_empty());
}

#[test]
fn build_factory_model_methods_returns_empty_for_non_factory_name() {
    let mut class = make_class("App\\Builders\\UserBuilder");
    class.parent_class = Some(atom(FACTORY_FQN));

    let methods = build_factory_model_methods(&class, &no_loader);
    assert!(methods.is_empty());
}

// ── LaravelFactoryProvider tests ────────────────────────────────────

#[test]
fn factory_provider_applies_to_factory_subclass() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == FACTORY_FQN {
            Some(Arc::new(make_class(FACTORY_FQN)))
        } else {
            None
        }
    };
    assert!(provider.applies_to(&factory, &loader));
}

#[test]
fn factory_provider_does_not_apply_to_factory_base_class() {
    let provider = LaravelFactoryProvider;
    let class = make_class(FACTORY_FQN);
    assert!(!provider.applies_to(&class, &no_loader));
}

#[test]
fn factory_provider_does_not_apply_when_extends_generic_present() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));
    factory.extends_generics = vec![(atom("Factory"), vec![PhpType::parse("User")])];

    assert!(!provider.applies_to(&factory, &no_loader));
}

#[test]
fn factory_provider_does_not_apply_to_non_factory() {
    let provider = LaravelFactoryProvider;
    let class = make_class("App\\Models\\User");
    assert!(!provider.applies_to(&class, &no_loader));
}

#[test]
fn factory_provider_synthesizes_create_and_make() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let model = make_class("App\\Models\\User");
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);
    assert_eq!(result.methods.len(), 2);

    let create = result.methods.iter().find(|m| m.name == "create").unwrap();
    assert_eq!(
        create.return_type_str().as_deref(),
        Some("App\\Models\\User")
    );
    assert!(!create.is_static);

    let make = result.methods.iter().find(|m| m.name == "make").unwrap();
    assert_eq!(make.return_type_str().as_deref(), Some("App\\Models\\User"));
    assert!(!make.is_static);
}

#[test]
fn factory_provider_empty_when_model_not_found() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let result = provider.provide(&factory, &no_loader, None);
    assert!(result.methods.is_empty());
}

// ── has{Rel}() / for{Rel}() / trashed() synthesis ──────────────────

#[test]
fn factory_provider_synthesizes_has_and_for_relationship_methods() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let mut model = make_class("App\\Models\\User");
    model
        .methods
        .push(Arc::new(make_method("posts", Some("HasMany<Post, $this>"))));
    model.methods.push(Arc::new(make_method(
        "author",
        Some("BelongsTo<User, $this>"),
    )));

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);

    // has{Relationship} for each relationship, returning the factory itself.
    let has_posts = result
        .methods
        .iter()
        .find(|m| m.name == "hasPosts")
        .unwrap();
    assert!(!has_posts.is_static);
    assert_eq!(has_posts.return_type_str().as_deref(), Some("static"));
    assert_eq!(has_posts.parameters.len(), 2);
    assert!(has_posts.parameters.iter().all(|p| !p.is_required));

    assert!(result.methods.iter().any(|m| m.name == "hasAuthor"));

    // for{Relationship} for each relationship, single optional $state param.
    let for_author = result
        .methods
        .iter()
        .find(|m| m.name == "forAuthor")
        .unwrap();
    assert!(!for_author.is_static);
    assert_eq!(for_author.return_type_str().as_deref(), Some("static"));
    assert_eq!(for_author.parameters.len(), 1);

    assert!(result.methods.iter().any(|m| m.name == "forPosts"));

    // create()/make() are still present alongside the relationship methods.
    assert!(result.methods.iter().any(|m| m.name == "create"));
    assert!(result.methods.iter().any(|m| m.name == "make"));
}

#[test]
fn factory_provider_uses_declared_model_for_relationship_methods() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\EditorialFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));
    factory.laravel_mut().factory_model = Some(PhpType::named(atom("App\\Models\\BlogAuthor")));

    let mut model = make_class("App\\Models\\BlogAuthor");
    model.methods.push(Arc::new(make_method(
        "posts",
        Some("HasMany<App\\Models\\BlogPost, $this>"),
    )));

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        (name == "App\\Models\\BlogAuthor").then(|| Arc::new(model.clone()))
    };

    let result = provider.provide(&factory, &loader, None);
    assert!(
        result
            .methods
            .iter()
            .any(|method| method.name == "hasPosts")
    );
    assert!(
        result
            .methods
            .iter()
            .any(|method| method.name == "forPosts")
    );
}

#[test]
fn factory_provider_ignores_non_relationship_methods() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let mut model = make_class("App\\Models\\User");
    model
        .methods
        .push(Arc::new(make_method("getName", Some("string"))));

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);
    assert!(!result.methods.iter().any(|m| m.name == "hasGetName"));
    assert!(!result.methods.iter().any(|m| m.name == "forGetName"));
}

#[test]
fn factory_provider_synthesizes_trashed_for_soft_deletes_model() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let mut model = make_class("App\\Models\\User");
    model.used_traits = vec![atom("Illuminate\\Database\\Eloquent\\SoftDeletes")];

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);
    let trashed = result.methods.iter().find(|m| m.name == "trashed").unwrap();
    assert!(!trashed.is_static);
    assert_eq!(trashed.return_type_str().as_deref(), Some("static"));
    assert!(trashed.parameters.is_empty());
}

#[test]
fn factory_provider_synthesizes_trashed_for_short_trait_name() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let mut model = make_class("App\\Models\\User");
    model.used_traits = vec![atom("SoftDeletes")];

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);
    assert!(result.methods.iter().any(|m| m.name == "trashed"));
}

#[test]
fn factory_provider_no_trashed_without_soft_deletes() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let model = make_class("App\\Models\\User");
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);
    assert!(!result.methods.iter().any(|m| m.name == "trashed"));
}

#[test]
fn factory_provider_trashed_from_soft_deletes_on_parent() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\UserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let mut model = make_class("App\\Models\\User");
    model.parent_class = Some(atom("App\\Models\\BaseModel"));

    let mut base = make_class("App\\Models\\BaseModel");
    base.used_traits = vec![atom("Illuminate\\Database\\Eloquent\\SoftDeletes")];

    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        match name {
            "App\\Models\\User" => Some(Arc::new(model.clone())),
            "App\\Models\\BaseModel" => Some(Arc::new(base.clone())),
            _ => None,
        }
    };

    let result = provider.provide(&factory, &loader, None);
    assert!(result.methods.iter().any(|m| m.name == "trashed"));
}

#[test]
fn factory_provider_subdirectory_convention() {
    let provider = LaravelFactoryProvider;
    let mut factory = make_class("Database\\Factories\\Admin\\SuperUserFactory");
    factory.parent_class = Some(atom(FACTORY_FQN));

    let model = make_class("App\\Models\\Admin\\SuperUser");
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\Admin\\SuperUser" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&factory, &loader, None);
    assert_eq!(result.methods.len(), 2);

    let create = result.methods.iter().find(|m| m.name == "create").unwrap();
    assert_eq!(
        create.return_type_str().as_deref(),
        Some("App\\Models\\Admin\\SuperUser")
    );
}
