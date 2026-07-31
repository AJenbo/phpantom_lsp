use super::*;
use crate::virtual_members::laravel::validation_rules::rules_from_array_text;

/// Build the shape for a rules array written as PHP source, and render it.
fn shape_of(array_text: &str) -> String {
    let rules = rules_from_array_text(array_text).expect("rules should parse");
    rules_to_shape(&rules)
        .map(|ty| ty.to_string())
        .unwrap_or_else(|| "array".to_string())
}

#[test]
fn required_string_is_a_required_key() {
    assert_eq!(
        shape_of("['name' => 'required|string|max:255']"),
        "array{name: string}"
    );
}

#[test]
fn nullable_adds_null_and_leaves_the_key_optional() {
    // Validating `['age' => 'nullable|integer']` against `[]` yields `[]`,
    // not `['age' => null]`, so the key is optional as well as nullable.
    assert_eq!(
        shape_of("['age' => 'nullable|integer']"),
        "array{age?: ?int}"
    );
}

#[test]
fn present_guarantees_the_key_without_requiring_a_value() {
    assert_eq!(
        shape_of("['note' => 'present|string']"),
        "array{note: string}"
    );
}

#[test]
fn a_conditional_requirement_still_allows_the_key_to_be_missing() {
    assert_eq!(
        shape_of("['tax_id' => 'required_if:kind,business|string']"),
        "array{tax_id?: string}"
    );
}

#[test]
fn a_field_that_is_neither_required_nor_nullable_is_optional() {
    assert_eq!(shape_of("['active' => 'boolean']"), "array{active?: bool}");
}

#[test]
fn sometimes_makes_even_a_required_field_optional() {
    assert_eq!(
        shape_of("['nickname' => 'sometimes|required|string']"),
        "array{nickname?: string}"
    );
}

#[test]
fn array_form_rules_are_read_like_pipe_form() {
    assert_eq!(
        shape_of("['age' => ['required', 'integer']]"),
        "array{age: int}"
    );
}

#[test]
fn rule_parameters_do_not_confuse_the_type() {
    assert_eq!(
        shape_of("['published_at' => 'required|date_format:Y-m-d']"),
        "array{published_at: string}"
    );
}

#[test]
fn a_rule_object_keeps_the_key_but_loses_the_type() {
    // `new Enum(Role::class)` still validated something; we cannot say what,
    // so the key survives as `mixed` rather than dropping out of the shape.
    assert_eq!(
        shape_of("['role' => [new Enum(Role::class)]]"),
        "array{role?: mixed}"
    );
}

#[test]
fn a_required_rule_object_is_still_a_required_key() {
    assert_eq!(
        shape_of("['role' => ['required', new Enum(Role::class)]]"),
        "array{role: mixed}"
    );
}

#[test]
fn unknown_constraint_rules_leave_the_type_open() {
    assert_eq!(
        shape_of("['token' => 'required|confirmed']"),
        "array{token: mixed}"
    );
}

#[test]
fn file_rules_resolve_to_uploaded_file() {
    assert_eq!(
        shape_of("['avatar' => 'required|image|max:2048']"),
        "array{avatar: \\Illuminate\\Http\\UploadedFile}"
    );
}

#[test]
fn numeric_admits_both_number_types() {
    assert_eq!(
        shape_of("['price' => 'required|numeric']"),
        "array{price: int|float}"
    );
}

// ─── Nesting ────────────────────────────────────────────────────────────────

#[test]
fn wildcard_children_build_a_list_of_shapes() {
    assert_eq!(
        shape_of(
            "[
                'items' => 'required|array',
                'items.*.id' => 'required|integer',
                'items.*.note' => 'nullable|string',
            ]"
        ),
        "array{items: list<array{id: int, note?: ?string}>}"
    );
}

#[test]
fn a_bare_wildcard_lists_its_scalar() {
    assert_eq!(
        shape_of(
            "[
                'tags' => 'required|array',
                'tags.*' => 'string',
            ]"
        ),
        "array{tags: list<string>}"
    );
}

#[test]
fn a_nullable_parent_keeps_its_null_alongside_the_child_shape() {
    // The child rules describe what a present `items` holds; `nullable` on
    // `items` itself still says the value may be null.
    assert_eq!(
        shape_of(
            "[
                'items' => 'required|nullable|array',
                'items.*.id' => 'required|integer',
            ]"
        ),
        "array{items: ?list<array{id: int}>}"
    );
}

#[test]
fn dotted_keys_build_a_nested_shape() {
    assert_eq!(
        shape_of(
            "[
                'owner' => 'required|array',
                'owner.email' => 'required|email',
            ]"
        ),
        "array{owner: array{email: string}}"
    );
}

#[test]
fn an_undeclared_parent_is_required_when_a_child_is() {
    // No `'owner' => …` entry, but `owner.email` is required, so the parent
    // key cannot be absent.
    assert_eq!(
        shape_of("['owner.email' => 'required|email']"),
        "array{owner: array{email: string}}"
    );
}

#[test]
fn an_undeclared_parent_is_optional_when_every_child_is() {
    assert_eq!(
        shape_of("['owner.email' => 'email']"),
        "array{owner?: array{email?: string}}"
    );
}

// ─── Bailing out ────────────────────────────────────────────────────────────

#[test]
fn a_computed_key_abandons_the_shape() {
    // The key set is incomplete, so a shape would report real input as an
    // unknown key.
    let rules = rules_from_array_text("['name' => 'required', $dynamic => 'required']").unwrap();
    assert!(!rules.keys_complete);
    assert!(rules_to_shape(&rules).is_none());
}

#[test]
fn a_spread_abandons_the_shape() {
    let rules = rules_from_array_text("[...$base, 'name' => 'required']").unwrap();
    assert!(rules_to_shape(&rules).is_none());
}

#[test]
fn an_empty_rules_array_has_no_shape() {
    assert!(rules_from_array_text("[]").is_none());
}

// ─── Member lookup and narrowing ────────────────────────────────────────────

fn rules(array_text: &str) -> RulesArray {
    rules_from_array_text(array_text).expect("rules should parse")
}

#[test]
fn member_type_reads_a_single_key() {
    let rules = rules("['name' => 'required|string', 'age' => 'nullable|integer']");
    assert_eq!(
        rules_member_type(&rules, "age").map(|t| t.to_string()),
        Some("?int".to_string())
    );
}

#[test]
fn member_type_walks_dot_notation() {
    let rules = rules("['owner.email' => 'required|email']");
    assert_eq!(
        rules_member_type(&rules, "owner.email").map(|t| t.to_string()),
        Some("string".to_string())
    );
}

#[test]
fn member_type_of_an_unknown_key_is_none() {
    let rules = rules("['name' => 'required|string']");
    assert!(rules_member_type(&rules, "nope").is_none());
}

#[test]
fn only_keeps_the_listed_keys() {
    let shape = rules_to_shape(&rules(
        "['name' => 'required|string', 'age' => 'required|integer', 'city' => 'required|string']",
    ))
    .unwrap();
    let narrowed = narrow_shape(&shape, &["name".to_string(), "city".to_string()], true).unwrap();
    assert_eq!(narrowed.to_string(), "array{name: string, city: string}");
}

#[test]
fn except_drops_the_listed_keys() {
    let shape = rules_to_shape(&rules(
        "['name' => 'required|string', 'age' => 'required|integer']",
    ))
    .unwrap();
    let narrowed = narrow_shape(&shape, &["age".to_string()], false).unwrap();
    assert_eq!(narrowed.to_string(), "array{name: string}");
}

#[test]
fn nullable_numeric_renders_as_a_nullable_union() {
    // Documented in `examples/laravel/app/Demo.php::validatedArrayShape`.
    assert_eq!(
        shape_of("['dough_temp' => 'nullable|numeric']"),
        "array{dough_temp?: ?int|float}"
    );
}

#[test]
fn the_bakery_demo_rules_produce_the_documented_shape() {
    // Mirrors App\Http\Requests\StoreBakeryRequest::rules() so the comments
    // in the Laravel demo cannot drift from what the engine infers.
    assert_eq!(
        shape_of(
            "[
                'name' => 'required|string|max:255',
                'apricot' => 'boolean',
                'dough_temp' => 'nullable|numeric',
                'notes' => 'array',
                'notes.*.body' => 'required|string',
                'owner.email' => 'required|email',
            ]"
        ),
        "array{name: string, apricot?: bool, dough_temp?: ?int|float, \
notes?: list<array{body: string}>, owner: array{email: string}}"
    );
}
