use super::*;
use crate::test_fixtures::{make_class, no_loader};

/// Parse a receiver expression the way the resolver hands it to
/// [`chain_count`]: the text to the left of the final `->create()`.
fn count_of(receiver: &str) -> FactoryCount {
    chain_count(&SubjectExpr::parse(receiver))
}

// ── is_count_conditional_method ─────────────────────────────────────

#[test]
fn count_conditional_methods_are_create_and_make() {
    assert!(is_count_conditional_method("create"));
    assert!(is_count_conditional_method("createQuietly"));
    assert!(is_count_conditional_method("make"));
}

#[test]
fn one_and_many_methods_are_not_count_conditional() {
    for name in [
        "createOne",
        "createOneQuietly",
        "createMany",
        "createManyQuietly",
        "makeOne",
        "makeMany",
        "state",
        "count",
    ] {
        assert!(
            !is_count_conditional_method(name),
            "{name} should not be count-conditional"
        );
    }
}

// ── chain_count: single-model chains ────────────────────────────────

#[test]
fn bare_factory_call_builds_one() {
    assert_eq!(count_of("User::factory()"), FactoryCount::One);
}

#[test]
fn factory_with_state_array_builds_one() {
    assert_eq!(
        count_of("User::factory(['name' => 'Ada'])"),
        FactoryCount::One
    );
}

#[test]
fn factory_with_closure_builds_one() {
    assert_eq!(
        count_of("User::factory(fn () => ['name' => 'Ada'])"),
        FactoryCount::One
    );
}

#[test]
fn factory_with_null_builds_one() {
    assert_eq!(count_of("User::factory(null)"), FactoryCount::One);
}

#[test]
fn factory_with_a_boolean_builds_one() {
    // `is_numeric(true)` is false, so this is state rather than a count.
    assert_eq!(count_of("User::factory(true)"), FactoryCount::One);
}

#[test]
fn factory_new_builds_one() {
    assert_eq!(count_of("UserFactory::new()"), FactoryCount::One);
}

#[test]
fn relationship_calls_do_not_set_a_count() {
    // `hasPosts(3)` takes a count for the *relationship*, not the factory.
    assert_eq!(count_of("User::factory()->hasPosts(3)"), FactoryCount::One);
    assert_eq!(
        count_of("User::factory()->forAuthor()->trashed()"),
        FactoryCount::One
    );
}

#[test]
fn count_null_clears_a_count() {
    assert_eq!(count_of("User::factory()->count(null)"), FactoryCount::One);
    assert_eq!(count_of("User::factory(3)->count(null)"), FactoryCount::One);
    assert_eq!(count_of("User::factory()->count(NULL)"), FactoryCount::One);
}

#[test]
fn count_without_arguments_builds_one() {
    assert_eq!(count_of("User::factory()->count()"), FactoryCount::One);
}

// ── chain_count: chains that settle nothing ─────────────────────────

/// A receiver that is not a chain carries no visible count state.  The
/// caller leaves the declared return type alone rather than picking a
/// branch, so a factory held in a variable *with* a count set does not
/// become a single model.
#[test]
fn a_non_call_receiver_is_unknown() {
    assert_eq!(count_of("$factory"), FactoryCount::Unknown);
    assert_eq!(count_of("$this->factory"), FactoryCount::Unknown);
    assert_eq!(count_of("$factory->state([])"), FactoryCount::Unknown);
}

/// `new UserFactory(3)` sets a count through the constructor, and the
/// subject parser does not keep the arguments to tell it from
/// `new UserFactory()`.
#[test]
fn a_new_expression_head_is_unknown() {
    assert_eq!(count_of("new UserFactory()"), FactoryCount::Unknown);
    assert_eq!(
        count_of("new UserFactory()->state([])"),
        FactoryCount::Unknown
    );
}

/// A static call other than `factory()`/`times()`/`new()` hands back a
/// factory built somewhere else, so its count is not visible here.
#[test]
fn an_unrelated_static_head_is_unknown() {
    assert_eq!(
        count_of("UserFactory::forTesting()->state([])"),
        FactoryCount::Unknown
    );
}

/// A variable argument to `factory()` could be the integer Laravel reads
/// as a count or the array it reads as state, and guessing wrong would
/// swap a model for a collection.
#[test]
fn factory_with_a_non_literal_argument_is_unknown() {
    assert_eq!(count_of("User::factory($count)"), FactoryCount::Unknown);
    assert_eq!(
        count_of("User::factory(self::COUNT)"),
        FactoryCount::Unknown
    );
    assert_eq!(
        count_of("User::factory(count($rows))"),
        FactoryCount::Unknown
    );
}

// ── chain_count: collection chains ──────────────────────────────────

#[test]
fn count_call_builds_many() {
    assert_eq!(count_of("User::factory()->count(3)"), FactoryCount::Many);
}

#[test]
fn count_with_variable_builds_many() {
    // `count(?int $count)` only takes an integer or null, so a variable
    // argument that is not literally `null` sets a count.
    assert_eq!(count_of("User::factory()->count($n)"), FactoryCount::Many);
}

#[test]
fn count_zero_builds_many() {
    // `count(0)` yields an empty collection, not a single model.
    assert_eq!(count_of("User::factory()->count(0)"), FactoryCount::Many);
}

#[test]
fn instance_times_builds_many() {
    assert_eq!(count_of("User::factory()->times(3)"), FactoryCount::Many);
}

#[test]
fn static_times_builds_many() {
    assert_eq!(count_of("UserFactory::times(3)"), FactoryCount::Many);
}

#[test]
fn integer_factory_argument_builds_many() {
    assert_eq!(count_of("User::factory(3)"), FactoryCount::Many);
}

#[test]
fn numeric_string_factory_argument_builds_many() {
    // Laravel gates on `is_numeric()`, which accepts numeric strings.
    assert_eq!(count_of("User::factory('3')"), FactoryCount::Many);
}

#[test]
fn count_survives_later_non_count_calls() {
    assert_eq!(
        count_of("User::factory()->count(3)->hasPosts(2)->trashed()"),
        FactoryCount::Many
    );
}

#[test]
fn last_count_call_wins() {
    assert_eq!(
        count_of("User::factory()->count(null)->count(2)"),
        FactoryCount::Many
    );
    assert_eq!(
        count_of("User::factory()->count(2)->count(null)"),
        FactoryCount::One
    );
}

/// A count-setting call is read where it stands, without needing to walk
/// back to the head of the chain.
#[test]
fn count_on_a_new_factory_instance_builds_many() {
    assert_eq!(count_of("new UserFactory()->count(3)"), FactoryCount::Many);
    assert_eq!(count_of("$factory->count(3)"), FactoryCount::Many);
}

/// The chain head can be something other than a static call — the legacy
/// global `factory()` helper, or an invoked callable.  Neither says
/// anything about the count, whatever it was handed.
#[test]
fn a_non_static_chain_head_is_unknown() {
    assert_eq!(count_of("factory(3)"), FactoryCount::Unknown);
    assert_eq!(count_of("$makeFactory()"), FactoryCount::Unknown);
    assert_eq!(count_of("$makeFactory()->state([])"), FactoryCount::Unknown);
}

/// The walk descends into a strictly smaller sub-expression each step, so
/// a chain far longer than anything real still finds the count it set.
#[test]
fn a_very_long_chain_still_finds_the_count() {
    let counted = |links: usize| {
        let mut expr = String::from("User::factory()->count(3)");
        for _ in 0..links {
            expr.push_str("->state([])");
        }
        count_of(&expr)
    };

    assert_eq!(counted(10), FactoryCount::Many);
    assert_eq!(counted(200), FactoryCount::Many);
}

// ── factory_model_type ──────────────────────────────────────────────

#[test]
fn model_type_prefers_extends_generic() {
    let mut factory = make_class("UserFactory");
    factory.file_namespace = Some(atom("Database\\Factories"));
    factory.extends_generics = vec![(
        atom("Illuminate\\Database\\Eloquent\\Factories\\Factory"),
        vec![PhpType::named(atom("App\\Domain\\Person"))],
    )];

    assert_eq!(
        factory_model_type(&factory, &no_loader).map(|t| t.to_string()),
        Some("App\\Domain\\Person".to_string()),
        "the @extends annotation names the model outright"
    );
}

#[test]
fn model_type_falls_back_to_the_naming_convention() {
    let mut factory = make_class("UserFactory");
    factory.file_namespace = Some(atom("Database\\Factories"));

    let model = Arc::new(make_class("User"));
    let loader = move |name: &str| -> Option<Arc<ClassInfo>> {
        (name == "App\\Models\\User").then(|| Arc::clone(&model))
    };

    assert_eq!(
        factory_model_type(&factory, &loader).map(|t| t.to_string()),
        Some("App\\Models\\User".to_string())
    );
}

#[test]
fn model_type_is_none_when_the_conventional_model_is_missing() {
    let mut factory = make_class("WidgetFactory");
    factory.file_namespace = Some(atom("Database\\Factories"));

    assert_eq!(factory_model_type(&factory, &no_loader), None);
}

#[test]
fn model_type_ignores_generics_for_other_parents() {
    let mut factory = make_class("UserFactory");
    factory.file_namespace = Some(atom("Database\\Factories"));
    factory.extends_generics = vec![(
        atom("Illuminate\\Support\\Collection"),
        vec![PhpType::named(atom("App\\Models\\Other"))],
    )];

    assert_eq!(
        factory_model_type(&factory, &no_loader),
        None,
        "only an @extends Factory<…> annotation names the model"
    );
}

// ── Count state carried by the value ────────────────────────────────

/// A resolved factory value with a count state attached.
fn factory_value(class: &str, count: FactoryCount) -> ResolvedType {
    let mut value = ResolvedType::from_arc(Arc::new(make_class(class)));
    value.factory_count = count;
    value
}

#[test]
fn a_lone_factory_value_speaks_for_itself() {
    assert_eq!(
        carried_count(&[factory_value("UserFactory", FactoryCount::Many)]),
        FactoryCount::Many
    );
}

#[test]
fn factory_values_that_disagree_carry_nothing() {
    assert_eq!(
        carried_count(&[
            factory_value("UserFactory", FactoryCount::One),
            factory_value("UserFactory", FactoryCount::Many),
        ]),
        FactoryCount::Unknown
    );
}

/// The `null` half of a `?UserFactory` is not a factory of unknown
/// count, so it does not overrule the factory beside it.
#[test]
fn a_nullable_factory_keeps_its_count() {
    assert_eq!(
        carried_count(&[
            factory_value("UserFactory", FactoryCount::One),
            ResolvedType::from_type_string(PhpType::null()),
        ]),
        FactoryCount::One
    );
}

#[test]
fn a_fluent_call_hands_its_state_to_the_same_class() {
    let receiver = vec![factory_value("UserFactory", FactoryCount::Many)];
    let mut results = vec![
        ResolvedType::from_arc(Arc::new(make_class("UserFactory"))),
        ResolvedType::from_arc(Arc::new(make_class("User"))),
    ];

    carry_factory_count(&mut results, &receiver, FactoryCount::Many);

    assert_eq!(results[0].factory_count, FactoryCount::Many);
    assert_eq!(
        results[1].factory_count,
        FactoryCount::Unknown,
        "the model a factory builds is not itself a factory"
    );
}

// ── factory($count) settled by the argument's type ──────────────────

#[test]
fn a_numeric_argument_type_sets_a_count() {
    for ty in ["int", "float", "positive-int"] {
        assert_eq!(
            numeric_argument_count(&PhpType::parse(ty)),
            FactoryCount::Many,
            "{ty} is what is_numeric() accepts"
        );
    }
}

#[test]
fn a_state_argument_type_sets_no_count() {
    for ty in ["array<string, mixed>", "callable", "Closure", "null"] {
        assert_eq!(
            numeric_argument_count(&PhpType::parse(ty)),
            FactoryCount::One,
            "{ty} is state, not a count"
        );
    }
}

#[test]
fn an_argument_type_that_could_be_either_settles_nothing() {
    for ty in ["mixed", "string", "int|array<mixed>"] {
        assert_eq!(
            numeric_argument_count(&PhpType::parse(ty)),
            FactoryCount::Unknown,
            "{ty} does not settle what is_numeric() would say"
        );
    }
}
