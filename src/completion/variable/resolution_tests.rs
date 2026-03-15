use std::sync::Arc;

use super::enrich_builder_type_in_scope;
use crate::test_fixtures::make_class;

use crate::types::ClassInfo;

fn make_model(name: &str) -> ClassInfo {
    let mut class = make_class(name);
    class.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
    class
}

fn model_loader(name: &str) -> Option<Arc<ClassInfo>> {
    if name == "Illuminate\\Database\\Eloquent\\Model" {
        Some(Arc::new(make_class(
            "Illuminate\\Database\\Eloquent\\Model",
        )))
    } else if name == "App\\Models\\User" {
        Some(Arc::new(make_model("App\\Models\\User")))
    } else {
        None
    }
}

#[test]
fn enrich_scope_method_with_builder_type() {
    let model = make_model("App\\Models\\User");
    let result =
        enrich_builder_type_in_scope("Builder", "scopeActive", false, &model, &model_loader);
    assert_eq!(result, Some("Builder<App\\Models\\User>".to_string()));
}

#[test]
fn enrich_scope_method_with_fqn_builder() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope(
        "Illuminate\\Database\\Eloquent\\Builder",
        "scopeActive",
        false,
        &model,
        &model_loader,
    );
    assert_eq!(
        result,
        Some("Illuminate\\Database\\Eloquent\\Builder<App\\Models\\User>".to_string())
    );
}

#[test]
fn enrich_skips_non_scope_method() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope("Builder", "getName", false, &model, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_skips_bare_scope_name() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope("Builder", "scope", false, &model, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_skips_non_model_class() {
    let plain = make_class("App\\Services\\SomeService");
    let result =
        enrich_builder_type_in_scope("Builder", "scopeActive", false, &plain, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_skips_non_builder_type() {
    let model = make_model("App\\Models\\User");
    let result =
        enrich_builder_type_in_scope("Collection", "scopeActive", false, &model, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_skips_builder_with_existing_generics() {
    let model = make_model("App\\Models\\User");
    let result =
        enrich_builder_type_in_scope("Builder<User>", "scopeActive", false, &model, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_scope_multi_word_method_name() {
    let model = make_model("App\\Models\\User");
    let result =
        enrich_builder_type_in_scope("Builder", "scopeByAuthor", false, &model, &model_loader);
    assert_eq!(result, Some("Builder<App\\Models\\User>".to_string()));
}

#[test]
fn enrich_scope_with_fqn_builder() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope(
        "Illuminate\\Database\\Eloquent\\Builder",
        "scopeActive",
        false,
        &model,
        &model_loader,
    );
    assert_eq!(
        result,
        Some("Illuminate\\Database\\Eloquent\\Builder<App\\Models\\User>".to_string())
    );
}

// ── #[Scope] attribute tests ────────────────────────────────────────

#[test]
fn enrich_scope_attribute_method_with_builder_type() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope("Builder", "active", true, &model, &model_loader);
    assert_eq!(result, Some("Builder<App\\Models\\User>".to_string()));
}

#[test]
fn enrich_scope_attribute_with_fqn_builder() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope(
        "Illuminate\\Database\\Eloquent\\Builder",
        "active",
        true,
        &model,
        &model_loader,
    );
    assert_eq!(
        result,
        Some("Illuminate\\Database\\Eloquent\\Builder<App\\Models\\User>".to_string())
    );
}

#[test]
fn enrich_scope_attribute_skips_non_model_class() {
    let plain = make_class("App\\Services\\SomeService");
    let result = enrich_builder_type_in_scope("Builder", "active", true, &plain, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_scope_attribute_skips_non_builder_type() {
    let model = make_model("App\\Models\\User");
    let result = enrich_builder_type_in_scope("Collection", "active", true, &model, &model_loader);
    assert_eq!(result, None);
}

#[test]
fn enrich_no_scope_attribute_and_no_convention_skips() {
    let model = make_model("App\\Models\\User");
    // Not a scopeX name and no attribute → should skip.
    let result = enrich_builder_type_in_scope("Builder", "active", false, &model, &model_loader);
    assert_eq!(result, None);
}

// ── Variable resolution: static chain assignment ────────────────────

/// `$result = Foo::create()->process(); $result->` should resolve
/// through the static call chain when `resolve_variable_types` is
/// called directly.
#[test]
fn resolve_var_from_static_method_chain_assignment() {
    use crate::types::MethodInfo;

    let content = r#"<?php
class Processor {
    public function getOutput(): string { return ''; }
}

class Builder {
    public function process(): Processor { return new Processor(); }
}

class Factory {
    public static function create(): Builder { return new Builder(); }
}

function test() {
    $result = Factory::create()->process();
    $result->
}
"#;
    // Classes that exist in this file
    let processor = {
        let mut c = make_class("Processor");
        c.methods.push(MethodInfo {
            is_static: false,
            ..MethodInfo::virtual_method("getOutput", Some("string"))
        });
        c
    };
    let builder = {
        let mut c = make_class("Builder");
        c.methods.push(MethodInfo {
            is_static: false,
            ..MethodInfo::virtual_method("process", Some("Processor"))
        });
        c
    };
    let factory = {
        let mut c = make_class("Factory");
        c.methods.push(MethodInfo {
            is_static: true,
            ..MethodInfo::virtual_method("create", Some("Builder"))
        });
        c
    };

    let all_classes: Vec<Arc<ClassInfo>> = vec![
        Arc::new(processor.clone()),
        Arc::new(builder.clone()),
        Arc::new(factory.clone()),
    ];
    let class_loader = |name: &str| -> Option<Arc<ClassInfo>> {
        match name {
            "Processor" => Some(Arc::new(processor.clone())),
            "Builder" => Some(Arc::new(builder.clone())),
            "Factory" => Some(Arc::new(factory.clone())),
            _ => None,
        }
    };

    // cursor_offset: find the position of `$result->` on the last
    // meaningful line.  We need an offset inside `function test()`.
    let cursor_offset = content.find("$result->").unwrap() as u32 + 9; // after `->`

    let results = super::resolve_variable_types(
        "$result",
        &ClassInfo::default(),
        &all_classes,
        content,
        cursor_offset,
        &class_loader,
        None,
    );

    let names: Vec<&str> = results.iter().map(|c| c.name.as_str()).collect();
    assert!(
        names.contains(&"Processor"),
        "$result should resolve to Processor via Factory::create()->process(), got: {:?}",
        names
    );
}

/// Cross-file scenario: `$user = User::factory()->create(); $user->`
/// where `factory()` comes from a trait with `@return TFactory` and
/// `create()` comes from the Factory base class with `@return TModel`.
///
/// This mirrors the Laravel `HasFactory` + `Factory` pattern that the
/// integration test `test_factory_variable_assignment_then_create`
/// exercises through the full LSP handler.
#[test]
fn resolve_var_from_cross_file_factory_chain() {
    use crate::types::MethodInfo;

    // The PHP source that the variable resolver will parse.
    // Classes are NOT defined here — they come from class_loader.
    let content = r#"<?php
use App\Models\User;
function test() {
    $user = User::factory()->create();
    $user->
}
"#;

    // ── Build the class graph ───────────────────────────────────

    // HasFactory trait: `public static function factory(): TFactory`
    // After trait merging with convention-based subs, User gets
    // `factory()` with return type `Database\Factories\UserFactory`.
    let has_factory_trait = {
        let mut c = make_class("HasFactory");
        c.file_namespace = Some("Illuminate\\Database\\Eloquent\\Factories".to_string());
        c.template_params = vec!["TFactory".to_string()];
        c.methods.push(MethodInfo {
            is_static: true,
            ..MethodInfo::virtual_method("factory", Some("TFactory"))
        });
        c
    };

    // Factory base class: `public function create(): TModel`
    let factory_base = {
        let mut c = make_class("Factory");
        c.file_namespace = Some("Illuminate\\Database\\Eloquent\\Factories".to_string());
        c.template_params = vec!["TModel".to_string()];
        c.methods
            .push(MethodInfo::virtual_method("create", Some("TModel")));
        c.methods
            .push(MethodInfo::virtual_method("make", Some("TModel")));
        c
    };

    // UserFactory extends Factory — convention says TModel = User.
    let user_factory = {
        let mut c = make_class("UserFactory");
        c.file_namespace = Some("Database\\Factories".to_string());
        c.parent_class = Some("Illuminate\\Database\\Eloquent\\Factories\\Factory".to_string());
        // The virtual member provider would synthesize create()/make()
        // returning User, but for this unit test we add them directly
        // with the substituted return type.
        c.methods.push(MethodInfo::virtual_method(
            "create",
            Some("App\\Models\\User"),
        ));
        c.methods.push(MethodInfo::virtual_method(
            "make",
            Some("App\\Models\\User"),
        ));
        c
    };

    // Model base class
    let model_base = make_class("Model");

    // User extends Model, uses HasFactory.
    // After trait merging, factory() returns UserFactory.
    let user = {
        let mut c = make_class("User");
        c.file_namespace = Some("App\\Models".to_string());
        c.parent_class = Some("Illuminate\\Database\\Eloquent\\Model".to_string());
        c.used_traits = vec!["Illuminate\\Database\\Eloquent\\Factories\\HasFactory".to_string()];
        // Simulate the result of trait merging with convention-based
        // TFactory substitution: factory() returns UserFactory FQN.
        c.methods.push(MethodInfo {
            is_static: true,
            ..MethodInfo::virtual_method("factory", Some("Database\\Factories\\UserFactory"))
        });
        c.methods
            .push(MethodInfo::virtual_method("greet", Some("string")));
        c
    };

    let all_classes: Vec<Arc<ClassInfo>> = vec![];

    let user_c = user.clone();
    let user_factory_c = user_factory.clone();
    let factory_base_c = factory_base.clone();
    let model_base_c = model_base.clone();
    let has_factory_c = has_factory_trait.clone();
    let class_loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        match name {
            "User" | "App\\Models\\User" => Some(Arc::new(user_c.clone())),
            "UserFactory" | "Database\\Factories\\UserFactory" => {
                Some(Arc::new(user_factory_c.clone()))
            }
            "Factory" | "Illuminate\\Database\\Eloquent\\Factories\\Factory" => {
                Some(Arc::new(factory_base_c.clone()))
            }
            "Model" | "Illuminate\\Database\\Eloquent\\Model" => {
                Some(Arc::new(model_base_c.clone()))
            }
            "HasFactory" | "Illuminate\\Database\\Eloquent\\Factories\\HasFactory" => {
                Some(Arc::new(has_factory_c.clone()))
            }
            _ => None,
        }
    };

    let cursor_offset = content.find("$user->").unwrap() as u32 + 7;

    let results = super::resolve_variable_types(
        "$user",
        &ClassInfo::default(),
        &all_classes,
        content,
        cursor_offset,
        &class_loader,
        None,
    );

    let names: Vec<&str> = results.iter().map(|c| c.name.as_str()).collect();
    assert!(
        names.contains(&"User"),
        "$user should resolve to User via User::factory()->create(), got: {:?}",
        names
    );
}
