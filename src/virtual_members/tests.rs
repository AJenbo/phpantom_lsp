use super::*;
use crate::test_fixtures::{make_class, make_method, make_property};
use crate::types::Visibility;
use std::sync::Arc;

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
            deprecated_replacement: None,
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
fn merge_replaces_mixed_property_with_specific_type() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("vat", Some("mixed")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("vat", Some("Decimal"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("Decimal"),
        "mixed-typed property should be replaced by a more specific type"
    );
}

#[test]
fn merge_replaces_untyped_property_with_specific_type() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("vat", None));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("vat", Some("Decimal"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("Decimal"),
        "untyped property should be replaced by a more specific type"
    );
}

#[test]
fn merge_does_not_replace_mixed_with_mixed() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("col", Some("mixed")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("col", Some("mixed"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(class.properties[0].type_hint.as_deref(), Some("mixed"));
}

#[test]
fn merge_does_not_replace_specific_type_with_mixed() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("col", Some("string")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("col", Some("mixed"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("string"),
        "specific type should not be overwritten by mixed"
    );
}

#[test]
fn merge_generic_type_beats_bare_type() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("tags", Some("array")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array<string>"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("array<string>"),
        "generic type should replace bare type of the same base"
    );
}

#[test]
fn merge_bare_type_beats_mixed() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("tags", Some("mixed")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("array"),
        "bare type should replace mixed"
    );
}

#[test]
fn merge_bare_type_does_not_replace_generic() {
    let mut class = make_class("Foo");
    class
        .properties
        .push(make_property("tags", Some("array<int>")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("array<int>"),
        "bare type should not replace a more specific generic type"
    );
}

#[test]
fn merge_same_specificity_preserves_first_writer() {
    let mut class = make_class("Foo");
    class
        .properties
        .push(make_property("tags", Some("array<int>")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array<string>"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("array<int>"),
        "equal specificity should preserve the first writer, not merge or replace"
    );
}

#[test]
fn merge_mixed_does_not_replace_bare_type() {
    let mut class = make_class("Foo");
    class.properties.push(make_property("tags", Some("array")));

    let virtual_members = VirtualMembers {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("mixed"))],
        constants: Vec::new(),
    };

    merge_virtual_members(&mut class, virtual_members);

    assert_eq!(class.properties.len(), 1);
    assert_eq!(
        class.properties[0].type_hint.as_deref(),
        Some("array"),
        "mixed should not replace a bare type"
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
        _class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> bool {
        true
    }

    fn provide(
        &self,
        _class: &ClassInfo,
        _class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        _cache: Option<&ResolvedClassCache>,
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
        _class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> bool {
        false
    }

    fn provide(
        &self,
        _class: &ClassInfo,
        _class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        _cache: Option<&ResolvedClassCache>,
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
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

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
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

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
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

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
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

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

/// When a higher-priority provider contributes a `mixed` property and
/// a lower-priority provider has a specific type, the specific type
/// should replace the `mixed` placeholder.
#[test]
fn apply_providers_low_priority_overrides_mixed_from_high_priority() {
    let mut class = make_class("Foo");

    // Simulates the Laravel model provider adding a column with type `mixed`
    // (e.g. from $fillable or an unresolvable cast).
    let high_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("vat", Some("mixed"))],
    }) as Box<dyn VirtualMemberProvider>;

    // Simulates the PHPDoc provider contributing `@property Decimal $vat`.
    let low_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("vat", Some("Decimal"))],
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![high_priority, low_priority];
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

    assert_eq!(class.properties.len(), 1);

    let vat = class.properties.iter().find(|p| p.name == "vat").unwrap();
    assert_eq!(
        vat.type_hint.as_deref(),
        Some("Decimal"),
        "PHPDoc @property type should replace mixed from Laravel provider"
    );
}

/// When a higher-priority provider contributes a specific type,
/// a lower-priority provider must not replace it — even with another
/// specific type.
#[test]
fn apply_providers_low_priority_cannot_override_specific_from_high_priority() {
    let mut class = make_class("Foo");

    let high_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("is_admin", Some("bool"))],
    }) as Box<dyn VirtualMemberProvider>;

    let low_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("is_admin", Some("int"))],
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![high_priority, low_priority];
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

    assert_eq!(class.properties.len(), 1);

    let prop = class
        .properties
        .iter()
        .find(|p| p.name == "is_admin")
        .unwrap();
    assert_eq!(
        prop.type_hint.as_deref(),
        Some("bool"),
        "higher-priority specific type should not be replaced"
    );
}

/// When a higher-priority provider contributes a bare type (`array`) and
/// a lower-priority provider has a generic type (`array<string>`), the
/// more specific generic type should win.
#[test]
fn apply_providers_generic_from_low_priority_beats_bare_from_high_priority() {
    let mut class = make_class("Foo");

    // Simulates the Laravel model provider adding a cast column with
    // bare type `array` (from `$casts = ['tags' => 'array']`).
    let high_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array"))],
    }) as Box<dyn VirtualMemberProvider>;

    // Simulates the PHPDoc provider contributing
    // `@property array<string> $tags`.
    let low_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array<string>"))],
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![high_priority, low_priority];
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

    assert_eq!(class.properties.len(), 1);

    let tags = class.properties.iter().find(|p| p.name == "tags").unwrap();
    assert_eq!(
        tags.type_hint.as_deref(),
        Some("array<string>"),
        "generic type from PHPDoc should replace bare type from Laravel provider"
    );
}

/// When both providers contribute the same specificity level, the
/// first writer (higher priority) wins.  We do NOT try to merge
/// `array<int>` and `array<string>` into `array<int|string>`.
#[test]
fn apply_providers_same_specificity_preserves_high_priority() {
    let mut class = make_class("Foo");

    let high_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array<int>"))],
    }) as Box<dyn VirtualMemberProvider>;

    let low_priority = Box::new(TestProvider {
        methods: Vec::new(),
        properties: vec![make_property("tags", Some("array<string>"))],
    }) as Box<dyn VirtualMemberProvider>;

    let providers: Vec<Box<dyn VirtualMemberProvider>> = vec![high_priority, low_priority];
    let class_loader = |_: &str| -> Option<Arc<ClassInfo>> { None };

    apply_virtual_members(&mut class, &class_loader, &providers, None);

    assert_eq!(class.properties.len(), 1);

    let tags = class.properties.iter().find(|p| p.name == "tags").unwrap();
    assert_eq!(
        tags.type_hint.as_deref(),
        Some("array<int>"),
        "equal specificity should preserve higher-priority provider, not merge types"
    );
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

    let class_loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        if name == "Parent" {
            Some(Arc::new(parent.clone()))
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

// ── evict_fqn / depends_on_any tests ────────────────────────────────

/// Helper: create a plain `HashMap` cache for eviction tests.
/// `evict_fqn` operates on the inner `HashMap`, not the
/// `Arc<Mutex<…>>` wrapper used at runtime.
fn make_cache() -> HashMap<ResolvedClassCacheKey, Arc<ClassInfo>> {
    HashMap::new()
}

#[test]
fn evict_removes_direct_match() {
    let mut cache = make_cache();
    let cls = make_class("App\\Models\\User");
    cache.insert(("App\\Models\\User".to_string(), Vec::new()), Arc::new(cls));

    evict_fqn(&mut cache, "App\\Models\\User");
    assert!(cache.is_empty(), "direct match should be evicted");
}

#[test]
fn evict_transitively_removes_child_class() {
    let mut cache = make_cache();

    let parent = make_class("Model");
    cache.insert(
        ("App\\Models\\Model".to_string(), Vec::new()),
        Arc::new(parent),
    );

    let mut child = make_class("User");
    child.parent_class = Some("App\\Models\\Model".to_string());
    cache.insert(
        ("App\\Models\\User".to_string(), Vec::new()),
        Arc::new(child),
    );

    evict_fqn(&mut cache, "App\\Models\\Model");
    assert!(cache.is_empty(), "both parent and child should be evicted");
}

#[test]
fn evict_transitively_removes_model_referencing_cast_class() {
    let mut cache = make_cache();

    let cast_class = make_class("DecimalCast");
    cache.insert(
        ("App\\Casts\\DecimalCast".to_string(), Vec::new()),
        Arc::new(cast_class),
    );

    let mut model = make_class("Setting");
    model.laravel_mut().casts_definitions =
        vec![("vat".to_string(), "App\\Casts\\DecimalCast".to_string())];
    cache.insert(
        ("App\\Models\\Setting".to_string(), Vec::new()),
        Arc::new(model),
    );

    // Evict the cast class — the model should be transitively evicted.
    evict_fqn(&mut cache, "App\\Casts\\DecimalCast");
    assert!(
        cache.is_empty(),
        "model referencing cast class via $casts should be transitively evicted"
    );
}

#[test]
fn evict_cast_class_with_colon_argument_transitively_removes_model() {
    let mut cache = make_cache();

    let cast_class = make_class("DecimalCast");
    cache.insert(
        ("App\\Casts\\DecimalCast".to_string(), Vec::new()),
        Arc::new(cast_class),
    );

    let mut model = make_class("Setting");
    // Cast type has a `:argument` suffix like `DecimalCast:8:2`.
    model.laravel_mut().casts_definitions =
        vec![("vat".to_string(), "App\\Casts\\DecimalCast:8:2".to_string())];
    cache.insert(
        ("App\\Models\\Setting".to_string(), Vec::new()),
        Arc::new(model),
    );

    evict_fqn(&mut cache, "App\\Casts\\DecimalCast");
    assert!(
        cache.is_empty(),
        "cast type with :argument suffix should still trigger transitive eviction"
    );
}

#[test]
fn evict_cast_class_matched_by_short_name() {
    let mut cache = make_cache();

    let cast_class = make_class("DecimalCast");
    cache.insert(
        ("App\\Casts\\DecimalCast".to_string(), Vec::new()),
        Arc::new(cast_class),
    );

    let mut model = make_class("Setting");
    // The model references the cast class by short name (same-file scenario).
    model.laravel_mut().casts_definitions = vec![("vat".to_string(), "DecimalCast".to_string())];
    cache.insert(
        ("App\\Models\\Setting".to_string(), Vec::new()),
        Arc::new(model),
    );

    evict_fqn(&mut cache, "App\\Casts\\DecimalCast");
    assert!(
        cache.is_empty(),
        "short-name cast reference should match FQN eviction"
    );
}

#[test]
fn evict_cast_class_canonical() {
    let mut cache = make_cache();

    let cast_class = make_class("DecimalCast");
    cache.insert(
        ("App\\Casts\\DecimalCast".to_string(), Vec::new()),
        Arc::new(cast_class),
    );

    let mut model = make_class("Setting");
    // Cast values are canonical (no leading `\`) after ingestion normalization.
    model.laravel_mut().casts_definitions =
        vec![("vat".to_string(), "App\\Casts\\DecimalCast".to_string())];
    cache.insert(
        ("App\\Models\\Setting".to_string(), Vec::new()),
        Arc::new(model),
    );

    evict_fqn(&mut cache, "App\\Casts\\DecimalCast");
    assert!(
        cache.is_empty(),
        "canonical cast type should trigger transitive eviction"
    );
}

#[test]
fn evict_builtin_cast_does_not_affect_model() {
    let mut cache = make_cache();

    let mut model = make_class("Setting");
    model.laravel_mut().casts_definitions = vec![
        ("is_active".to_string(), "boolean".to_string()),
        ("created_at".to_string(), "datetime".to_string()),
    ];
    cache.insert(
        ("App\\Models\\Setting".to_string(), Vec::new()),
        Arc::new(model),
    );

    // Evicting a random class should not affect the model since
    // its casts only reference built-in types.
    evict_fqn(&mut cache, "App\\Something\\Unrelated");
    assert_eq!(
        cache.len(),
        1,
        "model with only built-in casts should not be evicted"
    );
}

#[test]
fn evict_cast_class_chains_through_model_to_child() {
    let mut cache = make_cache();

    let cast_class = make_class("DecimalCast");
    cache.insert(
        ("App\\Casts\\DecimalCast".to_string(), Vec::new()),
        Arc::new(cast_class),
    );

    let mut model = make_class("Setting");
    model.laravel_mut().casts_definitions =
        vec![("vat".to_string(), "App\\Casts\\DecimalCast".to_string())];
    cache.insert(
        ("App\\Models\\Setting".to_string(), Vec::new()),
        Arc::new(model),
    );

    let mut child = make_class("AdvancedSetting");
    child.parent_class = Some("App\\Models\\Setting".to_string());
    cache.insert(
        ("App\\Models\\AdvancedSetting".to_string(), Vec::new()),
        Arc::new(child),
    );

    // Evicting the cast class should evict the model (via casts_definitions),
    // and the child (via parent_class → model).
    evict_fqn(&mut cache, "App\\Casts\\DecimalCast");
    assert!(
        cache.is_empty(),
        "cast eviction should chain: cast → model (via casts) → child (via parent)"
    );
}
