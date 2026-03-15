use super::*;
use crate::test_fixtures::{
    make_class, make_method, make_method_with_params, make_param, no_loader,
};
use crate::types::{MethodInfo, Visibility};
use std::sync::Arc;

// ── applies_to ──────────────────────────────────────────────────────

#[test]
fn applies_to_model_subclass() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    assert!(provider.applies_to(&user, &loader));
}

#[test]
fn does_not_apply_to_non_model() {
    let provider = LaravelModelProvider;
    let service = make_class("App\\Services\\UserService");
    assert!(!provider.applies_to(&service, &no_loader));
}

// ── provide: relationship properties ────────────────────────────────

#[test]
fn synthesizes_has_many_property() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "posts")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Collection<Post>")
    );
    assert_eq!(rel_prop.visibility, Visibility::Public);
    assert!(!rel_prop.is_static);
    // Also produces posts_count
    assert!(result.properties.iter().any(|p| p.name == "posts_count"));
}

#[test]
fn synthesizes_has_one_property() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("profile", Some("HasOne<Profile, $this>")));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "profile")
        .unwrap();
    assert_eq!(rel_prop.type_hint.as_deref(), Some("Profile"));
}

#[test]
fn synthesizes_belongs_to_property() {
    let provider = LaravelModelProvider;
    let mut post = make_class("App\\Models\\Post");
    post.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    post.methods
        .push(make_method("author", Some("BelongsTo<User, $this>")));

    let result = provider.provide(&post, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "author")
        .unwrap();
    assert_eq!(rel_prop.type_hint.as_deref(), Some("User"));
}

#[test]
fn synthesizes_morph_to_property() {
    let provider = LaravelModelProvider;
    let mut comment = make_class("App\\Models\\Comment");
    comment.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    comment
        .methods
        .push(make_method("commentable", Some("MorphTo")));

    let result = provider.provide(&comment, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "commentable")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Model")
    );
    // MorphTo also gets a _count property
    assert!(
        result
            .properties
            .iter()
            .any(|p| p.name == "commentable_count")
    );
}

#[test]
fn synthesizes_belongs_to_many_property() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("roles", Some("BelongsToMany<Role, $this>")));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "roles")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Collection<Role>")
    );
}

#[test]
fn synthesizes_multiple_relationship_properties() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));
    user.methods
        .push(make_method("profile", Some("HasOne<Profile, $this>")));
    user.methods
        .push(make_method("roles", Some("BelongsToMany<Role, $this>")));

    let result = provider.provide(&user, &no_loader, None);
    // 3 relationship properties + 3 _count properties = 6
    assert_eq!(result.properties.len(), 6);

    let names: Vec<&str> = result.properties.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"posts"));
    assert!(names.contains(&"profile"));
    assert!(names.contains(&"roles"));
    assert!(names.contains(&"posts_count"));
    assert!(names.contains(&"profile_count"));
    assert!(names.contains(&"roles_count"));
}

#[test]
fn skips_non_relationship_methods() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("getFullName", Some("string")));
    user.methods.push(make_method("save", Some("bool")));
    user.methods.push(make_method("toArray", Some("array")));

    let result = provider.provide(&user, &no_loader, None);
    assert!(result.properties.is_empty());
}

#[test]
fn skips_methods_without_return_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method("posts", None));

    let result = provider.provide(&user, &no_loader, None);
    assert!(result.properties.is_empty());
}

#[test]
fn handles_fqn_relationship_return_types() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method(
        "posts",
        Some("Illuminate\\Database\\Eloquent\\Relations\\HasMany<Post, $this>"),
    ));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "posts")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Collection<Post>")
    );
    assert!(result.properties.iter().any(|p| p.name == "posts_count"));
}

#[test]
fn relationship_without_generics_and_singular_produces_nothing() {
    // A singular relationship without generics has no TRelated,
    // so we cannot determine the relationship property type.
    // However, a _count property is still produced.
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method("profile", Some("HasOne")));

    let result = provider.provide(&user, &no_loader, None);
    assert!(
        !result.properties.iter().any(|p| p.name == "profile"),
        "Singular relationship without generics should not produce a relationship property"
    );
    // But a _count property is still valid
    let count_prop = result.properties.iter().find(|p| p.name == "profile_count");
    assert!(
        count_prop.is_some(),
        "Even without generics, a _count property should be produced"
    );
    assert_eq!(count_prop.unwrap().type_hint.as_deref(), Some("int"));
}

#[test]
fn collection_relationship_without_generics_uses_model_fallback() {
    // A collection relationship without generics defaults to
    // Collection<Model>.
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method("posts", Some("HasMany")));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "posts")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Collection<Illuminate\\Database\\Eloquent\\Model>")
    );
    assert!(result.properties.iter().any(|p| p.name == "posts_count"));
}

#[test]
fn relationships_produce_no_virtual_methods_or_constants() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));

    let result = provider.provide(&user, &no_loader, None);
    assert!(
        result.methods.is_empty(),
        "Relationship methods should not produce virtual methods"
    );
    assert!(result.constants.is_empty());
}

#[test]
fn provides_fqn_related_type_in_collection() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method(
        "posts",
        Some("HasMany<\\App\\Models\\Post, $this>"),
    ));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "posts")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Collection<\\App\\Models\\Post>")
    );
    assert!(result.properties.iter().any(|p| p.name == "posts_count"));
}

#[test]
fn provides_fqn_related_type_singular() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method(
        "profile",
        Some("HasOne<\\App\\Models\\Profile, $this>"),
    ));

    let result = provider.provide(&user, &no_loader, None);
    let rel_prop = result
        .properties
        .iter()
        .find(|p| p.name == "profile")
        .unwrap();
    assert_eq!(
        rel_prop.type_hint.as_deref(),
        Some("\\App\\Models\\Profile")
    );
}

// ── provide: scope methods (integration) ────────────────────────────

#[test]
fn synthesizes_scope_as_both_static_and_instance() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param(
            "$query",
            Some("\\Illuminate\\Database\\Eloquent\\Builder"),
            true,
        )],
    ));

    let result = provider.provide(&user, &no_loader, None);
    assert_eq!(result.methods.len(), 2, "Expected both static and instance");

    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    assert_eq!(instance.name, "active");
    assert!(instance.parameters.is_empty());
    assert_eq!(
        instance.return_type.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Builder<static>")
    );

    let static_m = result.methods.iter().find(|m| m.is_static).unwrap();
    assert_eq!(static_m.name, "active");
    assert!(static_m.parameters.is_empty());
    assert_eq!(
        static_m.return_type.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Builder<static>")
    );
}

#[test]
fn synthesizes_scope_with_extra_params() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method_with_params(
        "scopeOfType",
        Some("void"),
        vec![
            make_param(
                "$query",
                Some("\\Illuminate\\Database\\Eloquent\\Builder"),
                true,
            ),
            make_param("$type", Some("string"), true),
        ],
    ));

    let result = provider.provide(&user, &no_loader, None);
    assert_eq!(result.methods.len(), 2);

    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    assert_eq!(instance.name, "ofType");
    assert_eq!(instance.parameters.len(), 1);
    assert_eq!(instance.parameters[0].name, "$type");
    assert_eq!(instance.parameters[0].type_hint.as_deref(), Some("string"));
}

#[test]
fn synthesizes_multiple_scopes() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));
    user.methods.push(make_method_with_params(
        "scopeVerified",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);
    // 2 scopes × 2 variants (static + instance) = 4
    assert_eq!(result.methods.len(), 4);

    let names: Vec<&str> = result
        .methods
        .iter()
        .filter(|m| !m.is_static)
        .map(|m| m.name.as_str())
        .collect();
    assert!(names.contains(&"active"));
    assert!(names.contains(&"verified"));
}

#[test]
fn scope_and_relationship_coexist() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);
    // posts + posts_count = 2 properties
    assert_eq!(result.properties.len(), 2);
    assert!(result.properties.iter().any(|p| p.name == "posts"));
    assert!(result.properties.iter().any(|p| p.name == "posts_count"));
    assert_eq!(
        result.methods.len(),
        2,
        "Two scope methods (static + instance)"
    );
    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    assert_eq!(instance.name, "active");
}

#[test]
fn scope_method_not_treated_as_relationship() {
    // scopeActive's return type is "void", not a relationship type.
    // It should be treated as a scope, not produce a property.
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);
    assert!(
        result.properties.is_empty(),
        "Scope methods should not produce relationship properties"
    );
    assert_eq!(result.methods.len(), 2);
}

#[test]
fn scope_with_custom_return_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("\\App\\Builders\\UserBuilder"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);
    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    assert_eq!(
        instance.return_type.as_deref(),
        Some("\\App\\Builders\\UserBuilder")
    );
}

// ── provide: #[Scope] attribute (integration) ───────────────────────

#[test]
fn synthesizes_scope_attribute_as_both_static_and_instance() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let mut scope_method = make_method_with_params(
        "active",
        Some("void"),
        vec![make_param(
            "$query",
            Some("\\Illuminate\\Database\\Eloquent\\Builder"),
            true,
        )],
    );
    scope_method.has_scope_attribute = true;
    user.methods.push(scope_method);

    let result = provider.provide(&user, &no_loader, None);
    assert_eq!(result.methods.len(), 2, "Expected both static and instance");

    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    let static_m = result.methods.iter().find(|m| m.is_static).unwrap();

    assert_eq!(instance.name, "active");
    assert_eq!(static_m.name, "active");
    assert!(instance.parameters.is_empty());
    assert!(static_m.parameters.is_empty());
}

#[test]
fn synthesizes_scope_attribute_with_extra_params() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let mut scope_method = make_method_with_params(
        "ofType",
        Some("void"),
        vec![
            make_param(
                "$query",
                Some("\\Illuminate\\Database\\Eloquent\\Builder"),
                true,
            ),
            make_param("$type", Some("string"), true),
        ],
    );
    scope_method.has_scope_attribute = true;
    user.methods.push(scope_method);

    let result = provider.provide(&user, &no_loader, None);
    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    assert_eq!(instance.name, "ofType");
    assert_eq!(instance.parameters.len(), 1);
    assert_eq!(instance.parameters[0].name, "$type");
}

#[test]
fn scope_attribute_and_convention_scope_coexist() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    // Convention scope
    user.methods.push(make_method_with_params(
        "scopeVerified",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));
    // Attribute scope
    let mut scope_method = make_method_with_params(
        "active",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    );
    scope_method.has_scope_attribute = true;
    user.methods.push(scope_method);

    let result = provider.provide(&user, &no_loader, None);
    // 2 methods per scope × 2 scopes = 4
    let scope_methods: Vec<_> = result
        .methods
        .iter()
        .filter(|m| m.name == "verified" || m.name == "active")
        .collect();
    assert_eq!(scope_methods.len(), 4);
}

#[test]
fn scope_attribute_and_relationship_coexist() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));

    let mut scope_method = make_method_with_params(
        "active",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    );
    scope_method.has_scope_attribute = true;
    user.methods.push(scope_method);

    let result = provider.provide(&user, &no_loader, None);
    assert!(
        !result.properties.is_empty(),
        "Should have relationship properties"
    );
    assert!(
        result.methods.iter().any(|m| m.name == "active"),
        "Should have scope method"
    );
}

#[test]
fn scope_attribute_with_custom_return_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let mut scope_method = make_method_with_params(
        "active",
        Some("\\App\\Builders\\UserBuilder"),
        vec![make_param("$query", Some("Builder"), true)],
    );
    scope_method.has_scope_attribute = true;
    user.methods.push(scope_method);

    let result = provider.provide(&user, &no_loader, None);
    let instance = result.methods.iter().find(|m| !m.is_static).unwrap();
    assert_eq!(
        instance.return_type.as_deref(),
        Some("\\App\\Builders\\UserBuilder")
    );
}

#[test]
fn scope_attribute_not_treated_as_relationship() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let mut scope_method = make_method_with_params(
        "active",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    );
    scope_method.has_scope_attribute = true;
    user.methods.push(scope_method);

    let result = provider.provide(&user, &no_loader, None);
    assert!(
        result.properties.is_empty(),
        "Scope attribute methods should not produce relationship properties"
    );
    assert_eq!(result.methods.len(), 2);
}

// ── Builder-as-static forwarding (integration tests) ────────────────

/// Helper: create a minimal Builder class with template params and methods.
fn make_builder(methods: Vec<MethodInfo>) -> ClassInfo {
    let mut builder = make_class(ELOQUENT_BUILDER_FQN);
    builder.template_params = vec!["TModel".to_string()];
    builder.methods = methods;
    builder
}

#[test]
fn provide_includes_builder_forwarded_methods() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let builder = make_builder(vec![
        make_method("where", Some("static")),
        make_method(
            "get",
            Some("\\Illuminate\\Database\\Eloquent\\Collection<int, TModel>"),
        ),
    ]);

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_BUILDER_FQN {
            Some(Arc::new(builder.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);

    let static_methods: Vec<&str> = result
        .methods
        .iter()
        .filter(|m| m.is_static)
        .map(|m| m.name.as_str())
        .collect();
    assert!(
        static_methods.contains(&"where"),
        "Builder's where() should be forwarded as static, got: {:?}",
        static_methods
    );
    assert!(
        static_methods.contains(&"get"),
        "Builder's get() should be forwarded as static, got: {:?}",
        static_methods
    );
}

#[test]
fn provide_scope_beats_builder_method_with_same_name() {
    // If the model has a scopeWhere method AND Builder has a where
    // method, both produce static methods named "where". The scope's
    // version is added first, and merge_virtual_members would
    // deduplicate. But within the provider itself, the scope method
    // is added first, and build_builder_forwarded_methods skips
    // methods already on the class. However, scope methods are added
    // to the `methods` vec, not to the class itself, so the builder
    // dedup is based on class.methods (real methods + inherited).
    // The merge_virtual_members in mod.rs handles the final dedup.
    //
    // Here we just verify that both are produced (the dedup happens
    // at the merge layer).
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method_with_params(
        "scopeWhere",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let builder = make_builder(vec![make_method("where", Some("static"))]);

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_BUILDER_FQN {
            Some(Arc::new(builder.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);

    // Scope produces both static and instance "where".
    // Builder forwarding also produces a static "where".
    // merge_virtual_members will keep the first (scope) static one.
    let static_wheres: Vec<_> = result
        .methods
        .iter()
        .filter(|m| m.name == "where" && m.is_static)
        .collect();
    assert!(
        !static_wheres.is_empty(),
        "At least one static 'where' should exist from scope"
    );
    // The scope version has the default builder return type.
    assert_eq!(
        static_wheres[0].return_type.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Builder<static>"),
        "First static 'where' should be from the scope (added first)"
    );
}

// ── provide: accessor integration ───────────────────────────────────

#[test]
fn synthesizes_legacy_accessor_property() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("getFullNameAttribute", Some("string")));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "full_name");
    assert!(
        prop.is_some(),
        "Legacy accessor getFullNameAttribute should produce property full_name, got: {:?}",
        result
            .properties
            .iter()
            .map(|p| &p.name)
            .collect::<Vec<_>>()
    );
    assert_eq!(prop.unwrap().type_hint.as_deref(), Some("string"));
    assert!(!prop.unwrap().is_static);
}

#[test]
fn synthesizes_modern_accessor_property() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method(
        "fullName",
        Some("Illuminate\\Database\\Eloquent\\Casts\\Attribute"),
    ));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "full_name");
    assert!(
        prop.is_some(),
        "Modern accessor fullName() returning Attribute should produce property full_name, got: {:?}",
        result
            .properties
            .iter()
            .map(|p| &p.name)
            .collect::<Vec<_>>()
    );
    assert_eq!(prop.unwrap().type_hint.as_deref(), Some("mixed"));
}

#[test]
fn synthesizes_modern_accessor_property_with_generic_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods.push(make_method(
        "fullName",
        Some("Illuminate\\Database\\Eloquent\\Casts\\Attribute<string, never>"),
    ));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "full_name");
    assert!(
        prop.is_some(),
        "Modern accessor fullName() returning Attribute<string, never> should produce property full_name",
    );
    assert_eq!(
        prop.unwrap().type_hint.as_deref(),
        Some("string"),
        "Should extract first generic arg as the property type"
    );
}

#[test]
fn synthesizes_modern_accessor_property_short_name_generic() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("age", Some("Attribute<int>")));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "age");
    assert!(prop.is_some());
    assert_eq!(prop.unwrap().type_hint.as_deref(), Some("int"));
}

#[test]
fn accessor_and_relationship_coexist() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("getFullNameAttribute", Some("string")));
    user.methods.push(make_method(
        "posts",
        Some("HasMany<App\\Models\\Post, $this>"),
    ));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop_names: Vec<_> = result.properties.iter().map(|p| p.name.as_str()).collect();
    assert!(
        prop_names.contains(&"full_name"),
        "Should have accessor property"
    );
    assert!(
        prop_names.contains(&"posts"),
        "Should have relationship property"
    );
}

#[test]
fn get_attribute_method_not_treated_as_accessor() {
    // getAttribute() is a real Eloquent method, not an accessor.
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("getAttribute", Some("mixed")));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    // getAttribute should not produce any virtual property.
    assert!(
        result.properties.is_empty(),
        "getAttribute() should not be treated as a legacy accessor, got: {:?}",
        result
            .properties
            .iter()
            .map(|p| &p.name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn accessor_scope_and_relationship_all_coexist() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    user.methods
        .push(make_method("getFullNameAttribute", Some("string")));
    user.methods.push(make_method(
        "firstName",
        Some("Illuminate\\Database\\Eloquent\\Casts\\Attribute"),
    ));
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));
    user.methods.push(make_method(
        "posts",
        Some("HasMany<App\\Models\\Post, $this>"),
    ));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop_names: Vec<_> = result.properties.iter().map(|p| p.name.as_str()).collect();
    assert!(
        prop_names.contains(&"full_name"),
        "Legacy accessor property"
    );
    assert!(
        prop_names.contains(&"first_name"),
        "Modern accessor property"
    );
    assert!(prop_names.contains(&"posts"), "Relationship property");

    let method_names: Vec<_> = result.methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"active"), "Scope method");
}

#[test]
fn legacy_accessor_preserves_deprecated() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    let mut accessor = make_method("getOldNameAttribute", Some("string"));
    accessor.deprecation_message = Some("Use newName instead".into());
    user.methods.push(accessor);

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "old_name");
    assert!(prop.is_some());
    assert!(
        prop.unwrap().deprecation_message.is_some(),
        "Deprecated flag should be preserved"
    );
}

// ── Synthesize body-inferred relationship properties (uses functions from relationships submodule) ──

#[test]
fn synthesizes_property_from_body_inferred_has_many() {
    let provider = LaravelModelProvider;
    let mut user = make_class("App\\Models\\User");
    user.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    // Method with no @return annotation — return_type is set by
    // the parser from body inference.
    user.methods
        .push(make_method("posts", Some("HasMany<Post>")));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&user, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "posts");
    assert!(
        prop.is_some(),
        "Body-inferred HasMany<Post> should produce a 'posts' property"
    );
}

#[test]
fn synthesizes_property_from_body_inferred_morph_to() {
    let provider = LaravelModelProvider;
    let mut comment = make_class("App\\Models\\Comment");
    comment.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());

    // morphTo inferred from body — bare name, no generics.
    comment
        .methods
        .push(make_method("commentable", Some("MorphTo")));

    let model = make_class("Illuminate\\Database\\Eloquent\\Model");
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Illuminate\\Database\\Eloquent\\Model" {
            Some(Arc::new(model.clone()))
        } else {
            None
        }
    };

    let result = provider.provide(&comment, &loader, None);
    let prop = result.properties.iter().find(|p| p.name == "commentable");
    assert!(
        prop.is_some(),
        "Body-inferred MorphTo should produce a 'commentable' property"
    );
}

// ── Cast type mapping tests ─────────────────────────────────────────

// ── Cast property synthesis tests ───────────────────────────────────

#[test]
fn synthesizes_cast_properties() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![
        ("is_admin".to_string(), "boolean".to_string()),
        ("created_at".to_string(), "datetime".to_string()),
        ("options".to_string(), "array".to_string()),
    ];

    let result = provider.provide(&user, &no_loader, None);

    let is_admin = result.properties.iter().find(|p| p.name == "is_admin");
    assert!(is_admin.is_some(), "should produce is_admin property");
    assert_eq!(is_admin.unwrap().type_hint.as_deref(), Some("bool"));

    let created_at = result.properties.iter().find(|p| p.name == "created_at");
    assert!(created_at.is_some(), "should produce created_at property");
    assert_eq!(
        created_at.unwrap().type_hint.as_deref(),
        Some("Carbon\\Carbon")
    );

    let options = result.properties.iter().find(|p| p.name == "options");
    assert!(options.is_some(), "should produce options property");
    assert_eq!(options.unwrap().type_hint.as_deref(), Some("array"));
}

#[test]
fn cast_properties_are_public_and_not_static() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("is_admin".to_string(), "boolean".to_string())];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result
        .properties
        .iter()
        .find(|p| p.name == "is_admin")
        .unwrap();
    assert_eq!(prop.visibility, Visibility::Public);
    assert!(!prop.is_static);
    assert!(prop.deprecation_message.is_none());
}

#[test]
fn cast_properties_coexist_with_relationships_and_scopes() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("is_admin".to_string(), "boolean".to_string())];
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);

    // Cast property
    assert!(result.properties.iter().any(|p| p.name == "is_admin"));
    // Relationship property
    assert!(result.properties.iter().any(|p| p.name == "posts"));
    // Scope methods
    assert!(
        result
            .methods
            .iter()
            .any(|m| m.name == "active" && !m.is_static)
    );
    assert!(
        result
            .methods
            .iter()
            .any(|m| m.name == "active" && m.is_static)
    );
}

#[test]
fn cast_properties_coexist_with_accessors() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("is_admin".to_string(), "boolean".to_string())];
    user.methods
        .push(make_method("getFullNameAttribute", Some("string")));
    user.methods.push(make_method(
        "avatarUrl",
        Some("Illuminate\\Database\\Eloquent\\Casts\\Attribute"),
    ));

    let result = provider.provide(&user, &no_loader, None);

    // Cast property
    assert!(result.properties.iter().any(|p| p.name == "is_admin"));
    // Legacy accessor
    assert!(result.properties.iter().any(|p| p.name == "full_name"));
    // Modern accessor
    assert!(result.properties.iter().any(|p| p.name == "avatar_url"));
}

#[test]
fn empty_casts_produces_no_properties() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = Vec::new();

    let result = provider.provide(&user, &no_loader, None);
    assert!(result.properties.is_empty());
}

#[test]
fn cast_decimal_with_precision_synthesizes_float() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("price".to_string(), "decimal:2".to_string())];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result
        .properties
        .iter()
        .find(|p| p.name == "price")
        .unwrap();
    assert_eq!(prop.type_hint.as_deref(), Some("float"));
}

// ── Attribute default property synthesis tests ───────────────────────

#[test]
fn synthesizes_attribute_default_properties() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![
        ("role".to_string(), "string".to_string()),
        ("is_active".to_string(), "bool".to_string()),
        ("login_count".to_string(), "int".to_string()),
    ];

    let result = provider.provide(&user, &no_loader, None);

    let role = result.properties.iter().find(|p| p.name == "role");
    assert!(role.is_some(), "should produce role property");
    assert_eq!(role.unwrap().type_hint.as_deref(), Some("string"));

    let is_active = result.properties.iter().find(|p| p.name == "is_active");
    assert!(is_active.is_some(), "should produce is_active property");
    assert_eq!(is_active.unwrap().type_hint.as_deref(), Some("bool"));

    let login_count = result.properties.iter().find(|p| p.name == "login_count");
    assert!(login_count.is_some(), "should produce login_count property");
    assert_eq!(login_count.unwrap().type_hint.as_deref(), Some("int"));
}

#[test]
fn attribute_defaults_are_public_and_not_static() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![("role".to_string(), "string".to_string())];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result.properties.iter().find(|p| p.name == "role").unwrap();
    assert_eq!(prop.visibility, Visibility::Public);
    assert!(!prop.is_static);
    assert!(prop.deprecation_message.is_none());
}

#[test]
fn casts_take_priority_over_attribute_defaults() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    // Both $casts and $attributes define is_active
    user.laravel_mut().casts_definitions = vec![("is_active".to_string(), "boolean".to_string())];
    user.laravel_mut().attributes_definitions = vec![("is_active".to_string(), "int".to_string())];

    let result = provider.provide(&user, &no_loader, None);

    // Should only have one is_active property (from casts)
    let matching: Vec<_> = result
        .properties
        .iter()
        .filter(|p| p.name == "is_active")
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "should have exactly one is_active property"
    );
    assert_eq!(
        matching[0].type_hint.as_deref(),
        Some("bool"),
        "casts type should win over attributes type"
    );
}

#[test]
fn attribute_defaults_coexist_with_casts_for_different_columns() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("is_admin".to_string(), "boolean".to_string())];
    user.laravel_mut().attributes_definitions = vec![("role".to_string(), "string".to_string())];

    let result = provider.provide(&user, &no_loader, None);

    assert!(
        result.properties.iter().any(|p| p.name == "is_admin"),
        "cast property should be present"
    );
    assert!(
        result.properties.iter().any(|p| p.name == "role"),
        "attribute default property should be present"
    );
}

#[test]
fn attribute_defaults_coexist_with_relationships_and_scopes() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![("role".to_string(), "string".to_string())];
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);

    assert!(
        result.properties.iter().any(|p| p.name == "role"),
        "attribute default property"
    );
    assert!(
        result.properties.iter().any(|p| p.name == "posts"),
        "relationship property"
    );
    assert!(
        result
            .methods
            .iter()
            .any(|m| m.name == "active" && !m.is_static),
        "scope instance method"
    );
}

#[test]
fn empty_attributes_produces_no_properties() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = Vec::new();

    let result = provider.provide(&user, &no_loader, None);
    assert!(result.properties.is_empty());
}

#[test]
fn attribute_default_float_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![("rating".to_string(), "float".to_string())];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result
        .properties
        .iter()
        .find(|p| p.name == "rating")
        .unwrap();
    assert_eq!(prop.type_hint.as_deref(), Some("float"));
}

#[test]
fn attribute_default_null_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![("bio".to_string(), "null".to_string())];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result.properties.iter().find(|p| p.name == "bio").unwrap();
    assert_eq!(prop.type_hint.as_deref(), Some("null"));
}

#[test]
fn attribute_default_array_type() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![("settings".to_string(), "array".to_string())];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result
        .properties
        .iter()
        .find(|p| p.name == "settings")
        .unwrap();
    assert_eq!(prop.type_hint.as_deref(), Some("array"));
}

// ── Column name property synthesis tests ($fillable/$guarded/$hidden) ──

#[test]
fn synthesizes_column_name_properties_as_mixed() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().column_names = vec![
        "name".to_string(),
        "email".to_string(),
        "password".to_string(),
    ];

    let result = provider.provide(&user, &no_loader, None);

    let name = result.properties.iter().find(|p| p.name == "name");
    assert!(name.is_some(), "should produce name property");
    assert_eq!(name.unwrap().type_hint.as_deref(), Some("mixed"));

    let email = result.properties.iter().find(|p| p.name == "email");
    assert!(email.is_some(), "should produce email property");
    assert_eq!(email.unwrap().type_hint.as_deref(), Some("mixed"));

    let password = result.properties.iter().find(|p| p.name == "password");
    assert!(password.is_some(), "should produce password property");
    assert_eq!(password.unwrap().type_hint.as_deref(), Some("mixed"));
}

#[test]
fn column_name_properties_are_public_and_not_static() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().column_names = vec!["name".to_string()];

    let result = provider.provide(&user, &no_loader, None);
    let prop = result.properties.iter().find(|p| p.name == "name").unwrap();
    assert_eq!(prop.visibility, Visibility::Public);
    assert!(!prop.is_static);
    assert!(prop.deprecation_message.is_none());
}

#[test]
fn casts_take_priority_over_column_names() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("is_admin".to_string(), "boolean".to_string())];
    user.laravel_mut().column_names = vec!["is_admin".to_string(), "name".to_string()];

    let result = provider.provide(&user, &no_loader, None);

    let matching: Vec<_> = result
        .properties
        .iter()
        .filter(|p| p.name == "is_admin")
        .collect();
    assert_eq!(matching.len(), 1, "should have exactly one is_admin");
    assert_eq!(
        matching[0].type_hint.as_deref(),
        Some("bool"),
        "casts type should win over column name mixed"
    );

    let name = result.properties.iter().find(|p| p.name == "name");
    assert!(name.is_some(), "column-only name should still appear");
    assert_eq!(name.unwrap().type_hint.as_deref(), Some("mixed"));
}

#[test]
fn attributes_take_priority_over_column_names() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().attributes_definitions = vec![("role".to_string(), "string".to_string())];
    user.laravel_mut().column_names = vec!["role".to_string(), "email".to_string()];

    let result = provider.provide(&user, &no_loader, None);

    let matching: Vec<_> = result
        .properties
        .iter()
        .filter(|p| p.name == "role")
        .collect();
    assert_eq!(matching.len(), 1, "should have exactly one role");
    assert_eq!(
        matching[0].type_hint.as_deref(),
        Some("string"),
        "attributes type should win over column name mixed"
    );

    let email = result.properties.iter().find(|p| p.name == "email");
    assert!(email.is_some(), "column-only email should still appear");
    assert_eq!(email.unwrap().type_hint.as_deref(), Some("mixed"));
}

#[test]
fn all_three_sources_coexist() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().casts_definitions = vec![("is_admin".to_string(), "boolean".to_string())];
    user.laravel_mut().attributes_definitions = vec![("role".to_string(), "string".to_string())];
    user.laravel_mut().column_names = vec![
        "is_admin".to_string(),
        "role".to_string(),
        "email".to_string(),
    ];

    let result = provider.provide(&user, &no_loader, None);

    let is_admin = result
        .properties
        .iter()
        .find(|p| p.name == "is_admin")
        .unwrap();
    assert_eq!(is_admin.type_hint.as_deref(), Some("bool"), "from casts");

    let role = result.properties.iter().find(|p| p.name == "role").unwrap();
    assert_eq!(role.type_hint.as_deref(), Some("string"), "from attributes");

    let email = result
        .properties
        .iter()
        .find(|p| p.name == "email")
        .unwrap();
    assert_eq!(
        email.type_hint.as_deref(),
        Some("mixed"),
        "from column_names"
    );
}

#[test]
fn column_names_coexist_with_relationships_and_scopes() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().column_names = vec!["email".to_string()];
    user.methods
        .push(make_method("posts", Some("HasMany<Post, $this>")));
    user.methods.push(make_method_with_params(
        "scopeActive",
        Some("void"),
        vec![make_param("$query", Some("Builder"), true)],
    ));

    let result = provider.provide(&user, &no_loader, None);

    assert!(
        result.properties.iter().any(|p| p.name == "email"),
        "column name property"
    );
    assert!(
        result.properties.iter().any(|p| p.name == "posts"),
        "relationship property"
    );
    assert!(
        result
            .methods
            .iter()
            .any(|m| m.name == "active" && !m.is_static),
        "scope instance method"
    );
}

#[test]
fn empty_column_names_produces_no_extra_properties() {
    let provider = LaravelModelProvider;
    let mut user = make_class(ELOQUENT_MODEL_FQN);
    user.name = "User".to_string();
    user.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    user.laravel_mut().column_names = Vec::new();

    let result = provider.provide(&user, &no_loader, None);
    assert!(result.properties.is_empty());
}

// ─── build_scope_methods_for_builder ─────────────────────────────

#[test]
fn builder_scope_returns_empty_when_model_not_found() {
    let methods = build_scope_methods_for_builder("App\\Models\\Missing", &no_loader);
    assert!(methods.is_empty());
}

#[test]
fn builder_scope_returns_empty_for_non_model() {
    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\Plain" {
            Some(Arc::new(make_class("App\\Models\\Plain")))
        } else {
            None
        }
    };
    let methods = build_scope_methods_for_builder("App\\Models\\Plain", &loader);
    assert!(methods.is_empty());
}

#[test]
fn builder_scope_extracts_scope_methods_as_instance() {
    let mut model = make_class("App\\Models\\User");
    model.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    model.methods.push(make_method("scopeActive", Some("void")));
    model
        .methods
        .push(make_method("scopeVerified", Some("void")));
    model.methods.push(make_method("getName", Some("string")));

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\User" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_MODEL_FQN {
            Some(Arc::new(make_class(ELOQUENT_MODEL_FQN)))
        } else {
            None
        }
    };

    let methods = build_scope_methods_for_builder("App\\Models\\User", &loader);
    let names: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();

    assert!(
        names.contains(&"active"),
        "should contain active, got: {names:?}"
    );
    assert!(
        names.contains(&"verified"),
        "should contain verified, got: {names:?}"
    );
    assert!(
        !names.contains(&"getName"),
        "should not contain non-scope getName, got: {names:?}"
    );
    // All should be instance methods
    assert!(methods.iter().all(|m| !m.is_static));
}

#[test]
fn builder_scope_substitutes_static_in_return_type() {
    let mut model = make_class("App\\Models\\Brand");
    model.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    // Default scope return type contains `static`
    model
        .methods
        .push(make_method("scopePopular", Some("void")));

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\Brand" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_MODEL_FQN {
            Some(Arc::new(make_class(ELOQUENT_MODEL_FQN)))
        } else {
            None
        }
    };

    let methods = build_scope_methods_for_builder("App\\Models\\Brand", &loader);
    assert_eq!(methods.len(), 1);
    let popular = &methods[0];
    assert_eq!(popular.name, "popular");
    // The default return type `\...\Builder<static>` should have
    // `static` substituted with the concrete model name.
    let ret = popular.return_type.as_deref().unwrap();
    assert!(
        ret.contains("App\\Models\\Brand"),
        "return type should contain model name, got: {ret}"
    );
    assert!(
        !ret.contains("static"),
        "return type should not contain 'static', got: {ret}"
    );
}

#[test]
fn builder_scope_strips_query_parameter() {
    let mut model = make_class("App\\Models\\Task");
    model.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    model.methods.push(make_method_with_params(
        "scopeOfType",
        Some("void"),
        vec![
            make_param("$query", Some("Builder"), true),
            make_param("$type", Some("string"), true),
        ],
    ));

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\Task" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_MODEL_FQN {
            Some(Arc::new(make_class(ELOQUENT_MODEL_FQN)))
        } else {
            None
        }
    };

    let methods = build_scope_methods_for_builder("App\\Models\\Task", &loader);
    assert_eq!(methods.len(), 1);
    let of_type = &methods[0];
    assert_eq!(of_type.name, "ofType");
    // $query should be stripped, only $type remains
    assert_eq!(of_type.parameters.len(), 1);
    assert_eq!(of_type.parameters[0].name, "$type");
}

#[test]
fn builder_scope_with_custom_return_type() {
    let mut model = make_class("App\\Models\\Post");
    model.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    model.methods.push(make_method(
        "scopeDraft",
        Some("\\Illuminate\\Database\\Eloquent\\Builder<static>"),
    ));

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\Post" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_MODEL_FQN {
            Some(Arc::new(make_class(ELOQUENT_MODEL_FQN)))
        } else {
            None
        }
    };

    let methods = build_scope_methods_for_builder("App\\Models\\Post", &loader);
    assert_eq!(methods.len(), 1);
    let draft = &methods[0];
    assert_eq!(draft.name, "draft");
    let ret = draft.return_type.as_deref().unwrap();
    assert_eq!(
        ret,
        "\\Illuminate\\Database\\Eloquent\\Builder<App\\Models\\Post>"
    );
}

#[test]
fn builder_scope_preserves_deprecated() {
    let mut model = make_class("App\\Models\\Item");
    model.parent_class = Some(ELOQUENT_MODEL_FQN.to_string());
    let mut scope = make_method("scopeOld", Some("void"));
    scope.deprecation_message = Some("Use scopeNew() instead".into());
    model.methods.push(scope);

    let loader = |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "App\\Models\\Item" {
            Some(Arc::new(model.clone()))
        } else if name == ELOQUENT_MODEL_FQN {
            Some(Arc::new(make_class(ELOQUENT_MODEL_FQN)))
        } else {
            None
        }
    };

    let methods = build_scope_methods_for_builder("App\\Models\\Item", &loader);
    assert_eq!(methods.len(), 1);
    assert!(methods[0].deprecation_message.is_some());
}
