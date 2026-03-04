use super::*;
use crate::test_fixtures::{make_class, make_method, make_property};
use crate::types::Visibility;

// ── VirtualMembers tests ────────────────────────────────────────────

#[test]
fn virtual_members_is_empty() {
    let vm = VirtualMembers {
        methods: Vec::new(),
        properties: Vec::new(),
        constants: Vec::new(),
    };
    assert!(vm.is_empty());
}

#[test]
fn virtual_members_not_empty_with_method() {
    let vm = VirtualMembers {
        methods: vec![make_method("foo", Some("string"))],
        properties: Vec::new(),
        constants: Vec::new(),
    };
    assert!(!vm.is_empty());
}

#[test]
fn virtual_members_not_empty_with_property() {
    let vm = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("bar", Some("int"))],
        constants: Vec::new(),
    };
    assert!(!vm.is_empty());
}

#[test]
fn virtual_members_not_empty_with_constant() {
    let vm = VirtualMembers {
        methods: Vec::new(),
        properties: Vec::new(),
        constants: vec![ConstantInfo {
            name: "FOO".to_string(),
            name_offset: 0,
            type_hint: None,
            visibility: Visibility::Public,
            deprecation_message: None,
            description: None,
            is_enum_case: false,
            enum_value: None,
            value: None,
            is_virtual: true,
        }],
    };
    assert!(!vm.is_empty());
}

// ── merge_virtual_members tests ─────────────────────────────────────

#[test]
fn merge_adds_new_methods() {
    let mut class = make_class("Foo");
    class.methods.push(make_method("existing", Some("string")));

    let virtual_members = VirtualMembers {
        methods: vec![make_method("new_method", Some("int"))],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.methods.len(), 2);
    assert!(class.methods.iter().any(|m| m.name == "existing"));
    assert!(class.methods.iter().any(|m| m.name == "new_method"));
}

#[test]
fn merge_adds_new_properties() {
    let mut class = make_class("Foo");
    class
        .properties
        .push(make_property("existing", Some("string")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("new_prop", Some("int"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 2);
    assert!(class.properties.iter().any(|p| p.name == "existing"));
    assert!(class.properties.iter().any(|p| p.name == "new_prop"));
}

#[test]
fn merge_does_not_overwrite_existing_method() {
    let mut class = make_class("Foo");
    class.methods.push(make_method("doStuff", Some("string")));

    let virtual_members = VirtualMembers {
        methods: vec![make_method("doStuff", Some("int"))],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.methods.len(), 1);
    assert_eq!(
        class.methods[0].return_type.as_deref(),
        Some("string"),
        "existing method should not be overwritten"
    );
}

#[test]
fn merge_allows_same_name_methods_with_different_staticness() {
    let mut class = make_class("Foo");
    // Existing instance method
    class.methods.push(make_method("active", Some("string")));

    // Virtual: one instance (should be blocked) and one static (should be added)
    let mut static_method = make_method("active", Some("Builder"));
    static_method.is_static = true;

    let virtual_members = VirtualMembers {
        methods: vec![make_method("active", Some("int")), static_method],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.methods.len(), 2, "instance + static should coexist");
    let instance = class
        .methods
        .iter()
        .find(|m| m.name == "active" && !m.is_static)
        .unwrap();
    assert_eq!(
        instance.return_type.as_deref(),
        Some("string"),
        "existing instance method should not be overwritten"
    );
    let static_m = class
        .methods
        .iter()
        .find(|m| m.name == "active" && m.is_static)
        .unwrap();
    assert_eq!(
        static_m.return_type.as_deref(),
        Some("Builder"),
        "static variant should be added alongside instance"
    );
}

#[test]
fn merge_replaces_scope_attribute_method_with_virtual() {
    let mut class = make_class("Foo");
    let mut original = make_method("active", Some("void"));
    original.has_scope_attribute = true;
    original.visibility = Visibility::Protected;
    class.methods.push(original);

    let mut virtual_scope = make_method("active", Some("Builder<static>"));
    virtual_scope.visibility = Visibility::Public;

    let virtual_members = VirtualMembers {
        methods: vec![virtual_scope],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.methods.len(), 1);
    assert_eq!(
        class.methods[0].return_type.as_deref(),
        Some("Builder<static>"),
        "#[Scope] original should be replaced by virtual scope method"
    );
    assert_eq!(
        class.methods[0].visibility,
        Visibility::Public,
        "replacement should be public"
    );
}

#[test]
fn merge_does_not_replace_non_scope_attribute_method() {
    let mut class = make_class("Foo");
    let mut original = make_method("active", Some("string"));
    original.has_scope_attribute = false;
    class.methods.push(original);

    let virtual_members = VirtualMembers {
        methods: vec![make_method("active", Some("int"))],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.methods.len(), 1);
    assert_eq!(
        class.methods[0].return_type.as_deref(),
        Some("string"),
        "non-#[Scope] method should not be replaced"
    );
}

#[test]
fn merge_replaces_scope_attribute_and_adds_static_variant() {
    let mut class = make_class("Foo");
    let mut original = make_method("active", Some("void"));
    original.has_scope_attribute = true;
    original.visibility = Visibility::Protected;
    class.methods.push(original);

    let mut virtual_instance = make_method("active", Some("Builder<static>"));
    virtual_instance.visibility = Visibility::Public;

    let mut virtual_static = make_method("active", Some("Builder<static>"));
    virtual_static.is_static = true;
    virtual_static.visibility = Visibility::Public;

    let virtual_members = VirtualMembers {
        methods: vec![virtual_instance, virtual_static],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(
        class.methods.len(),
        2,
        "replaced instance + new static should coexist"
    );
    let instance = class
        .methods
        .iter()
        .find(|m| m.name == "active" && !m.is_static)
        .unwrap();
    assert_eq!(
        instance.return_type.as_deref(),
        Some("Builder<static>"),
        "instance should be the virtual replacement"
    );
    assert_eq!(instance.visibility, Visibility::Public);
    let static_m = class
        .methods
        .iter()
        .find(|m| m.name == "active" && m.is_static)
        .unwrap();
    assert_eq!(
        static_m.return_type.as_deref(),
        Some("Builder<static>"),
        "static variant should be added"
    );
}

#[test]
fn merge_blocks_same_name_same_staticness() {
    let mut class = make_class("Foo");
    let mut existing = make_method("active", Some("string"));
    existing.is_static = true;
    class.methods.push(existing);

    let mut virtual_static = make_method("active", Some("int"));
    virtual_static.is_static = true;

    let virtual_members = VirtualMembers {
        methods: vec![virtual_static],
        properties: Vec::new(),
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.methods.len(), 1);
    assert_eq!(
        class.methods[0].return_type.as_deref(),
        Some("string"),
        "existing static method should not be overwritten by virtual static"
    );
}

#[test]
fn merge_does_not_overwrite_existing_property() {
    let mut class = make_class("Foo");
    class
        .properties
        .push(make_property("value", Some("string")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("value", Some("int"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("string"),
        "existing property should not be overwritten"
    );
}

#[test]
fn merge_handles_empty_virtual_members() {
    let mut class = make_class("Foo");
    class.methods.push(make_method("foo", Some("void")));
    class.properties.push(make_property("bar", Some("int")));

    merge_virtual_members(
        &mut class,
        VirtualMembers {
            methods: Vec::new(),
            properties: Vec::new(),
            constants: Vec::new(),
        },
    );

    assert_eq!(class.methods.len(), 1);
    assert_eq!(class.properties.len(), 1);
}

// ── apply_virtual_members / provider priority tests ─────────────────

/// A test provider that always applies and contributes fixed members.
struct TestProvider {
    methods: Vec<MethodInfo>,
    properties: Vec<PropertyInfo>,
}

impl VirtualMemberProvider for TestProvider {
    fn applies_to(
        &self,
        _class: &ClassInfo,
        _class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> bool {
        true
    }

    fn provide(
        &self,
        _class: &ClassInfo,
        _class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> VirtualMembers {
        VirtualMembers {
            methods: self.methods.clone(),
            properties: self.properties.clone(),
            constants: Vec::new(),
        }
    }
}

/// A test provider that never applies.
struct NeverProvider;

impl VirtualMemberProvider for NeverProvider {
    fn applies_to(
        &self,
        _class: &ClassInfo,
        _class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> bool {
        false
    }

    fn provide(
        &self,
        _class: &ClassInfo,
        _class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> VirtualMembers {
        panic!("provide should not be called when applies_to returns false")
    }
}

#[test]
fn apply_providers_in_priority_order() {
    let mut class = make_class("Foo");

    // Higher priority provider contributes "doStuff" returning "string"
    let high_priority = Box::new(TestProvider {
        methods: vec![make_method("doStuff", Some("string"))],
        properties: Vec::new(),
    }) as Box<dyn VirtualMemberProvider>;

    // Lower priority provider contributes "doStuff" returning "int"
    // (should be shadowed) and "other" returning "bool" (should be added)
    let low_priority = Box::new(TestProvider {
        methods: vec![
            make_method("doStuff", Some("int")),
            make_method("other", Some("bool")),
        ],
        properties: Vec::new(),
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![high_priority, low_priority];
    let class_loader = |_: &str| -> Option<ClassInfo> { None };

    apply_virtual_members(&mut class, &class_loader, &providers);

    assert_eq!(class.methods.len(), 2);

    let do_stuff = class.methods.iter().find(|m| m.name == "doStuff").unwrap();
    assert_eq!(
        do_stuff.return_type.as_deref(),
        Some("string"),
        "higher-priority provider should win"
    );

    let other = class.methods.iter().find(|m| m.name == "other").unwrap();
    assert_eq!(other.return_type.as_deref(), Some("bool"));
}

#[test]
fn apply_providers_skips_non_applicable() {
    let mut class = make_class("Foo");

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![Box::new(NeverProvider)];
    let class_loader = |_: &str| -> Option<ClassInfo> { None };

    apply_virtual_members(&mut class, &class_loader, &providers);

    assert!(class.methods.is_empty());
    assert!(class.properties.is_empty());
}

#[test]
fn apply_providers_real_members_beat_virtual() {
    let mut class = make_class("Foo");
    class
        .methods
        .push(make_method("realMethod", Some("string")));

    let provider = Box::new(TestProvider {
        methods: vec![make_method("realMethod", Some("int"))],
        properties: Vec::new(),
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![provider];
    let class_loader = |_: &str| -> Option<ClassInfo> { None };

    apply_virtual_members(&mut class, &class_loader, &providers);

    assert_eq!(class.methods.len(), 1);
    assert_eq!(
        class.methods[0].return_type.as_deref(),
        Some("string"),
        "real declared method should not be overwritten by virtual"
    );
}

#[test]
fn apply_providers_property_priority() {
    let mut class = make_class("Foo");

    let high_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("name", Some("string"))],
    }) as Box<dyn VirtualMemberProvider>;

    let low_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![
            make_property("name", Some("mixed")),
            make_property("email", Some("string")),
        ],
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![high_priority, low_priority];
    let class_loader = |_: &str| -> Option<ClassInfo> { None };

    apply_virtual_members(&mut class, &class_loader, &providers);

    assert_eq!(class.properties.len(), 2);

    let name = class.properties.iter().find(|p| p.name == "name").unwrap();
    assert_eq!(
        name.type_hint.as_deref(),
        Some("string"),
        "higher-priority provider property should win"
    );

    let email = class.properties.iter().find(|p| p.name == "email").unwrap();
    assert_eq!(email.type_hint.as_deref(), Some("string"));
}

#[test]
fn default_providers_has_laravel_and_phpdoc() {
    let providers = default_providers();
    assert_eq!(
        providers.len(),
        3,
        "should have LaravelModelProvider, LaravelFactoryProvider, and PHPDocProvider registered"
    );
}

// ── resolve_class_fully tests ───────────────────────────────────────

#[test]
fn resolve_class_fully_returns_same_as_base_when_no_providers() {
    // With no providers registered, resolve_class_fully should produce
    // the same result as resolve_class_with_inheritance.
    let mut class = make_class("Child");
    class.methods.push(make_method("childMethod", Some("void")));
    class.parent_class = Some("Parent".to_string());

    let mut parent = make_class("Parent");
    parent
        .methods
        .push(make_method("parentMethod", Some("string")));

    let class_loader = move |name: &str| -> Option<ClassInfo> {
        if name == "Parent" {
            Some(parent.clone())
        } else {
            None
        }
    };

    let base = crate::inheritance::resolve_class_with_inheritance(&class, &class_loader);
    let full = crate::virtual_members::resolve_class_fully(&class, &class_loader);

    assert_eq!(base.methods.len(), full.methods.len());
    assert_eq!(base.properties.len(), full.properties.len());
    for base_method in &base.methods {
        assert!(
            full.methods.iter().any(|m| m.name == base_method.name),
            "full resolution should contain all base methods"
        );
    }
}
