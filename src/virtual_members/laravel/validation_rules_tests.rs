use super::*;

fn keys(rules: &[ValidationRule]) -> Vec<&str> {
    rules.iter().map(|r| r.key.as_str()).collect()
}

#[test]
fn parses_form_request_rules_method() {
    let content = "<?php
namespace App\\Http\\Requests;
class StoreUserRequest extends FormRequest {
    public function rules(): array {
        return [
            'name' => 'required|string|max:255',
            'age' => ['nullable', 'integer'],
        ];
    }
}
";
    let rules = rules_from_class_source(content, "StoreUserRequest");
    assert_eq!(keys(&rules), vec!["name", "age"]);
    assert_eq!(rules[0].rules, "required|string|max:255");
    assert_eq!(rules[1].rules, "nullable|integer");
}

#[test]
fn follows_array_merge_in_rules_method() {
    let content = "<?php
class UpdateUserRequest extends FormRequest {
    public function rules(): array {
        return array_merge(parent::rules(), [
            'email' => 'required|email',
        ]);
    }
}
";
    let rules = rules_from_class_source(content, "UpdateUserRequest");
    assert_eq!(keys(&rules), vec!["email"]);
}

#[test]
fn ignores_rules_method_of_another_class() {
    let content = "<?php
class OtherRequest extends FormRequest {
    public function rules(): array { return ['other' => 'required']; }
}
class StoreUserRequest extends FormRequest {
    public function rules(): array { return ['name' => 'required']; }
}
";
    assert_eq!(
        keys(&rules_from_class_source(content, "StoreUserRequest")),
        vec!["name"]
    );
}

#[test]
fn renders_non_literal_rule_entries_from_source() {
    let content = "<?php
class StoreUserRequest extends FormRequest {
    public function rules(): array {
        return [
            'role' => [
                'required',
                new Enum(Role::class),
            ],
        ];
    }
}
";
    let rules = rules_from_class_source(content, "StoreUserRequest");
    assert_eq!(rules[0].rules, "required|new Enum(Role::class)");
}

#[test]
fn key_offsets_point_inside_the_quotes() {
    let content = "<?php
class StoreUserRequest extends FormRequest {
    public function rules(): array { return ['name' => 'required']; }
}
";
    let rules = rules_from_class_source(content, "StoreUserRequest");
    assert_eq!(&content[rules[0].key_start..rules[0].key_start + 4], "name");
}

#[test]
fn finds_inline_validate_call() {
    let content = "<?php
class UserController {
    public function store(Request $request) {
        $request->validate([
            'title' => 'required|string',
            'body' => 'required',
        ]);
        $request->input('');
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    let rules = inline_validate_rules(content, cursor).expect("should find validate() rules");
    assert_eq!(keys(&rules), vec!["title", "body"]);
}

#[test]
fn finds_validates_requests_trait_form() {
    let content = "<?php
class UserController {
    public function store(Request $request) {
        $this->validate($request, ['title' => 'required']);
        $request->input('');
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    let rules = inline_validate_rules(content, cursor).expect("should find $this->validate rules");
    assert_eq!(keys(&rules), vec!["title"]);
}

#[test]
fn finds_validator_make_rules() {
    let content = "<?php
class UserController {
    public function store(Request $request) {
        $validator = Validator::make($request->all(), ['title' => 'required']);
        $request->input('');
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    let rules = inline_validate_rules(content, cursor).expect("should find Validator::make rules");
    assert_eq!(keys(&rules), vec!["title"]);
}

#[test]
fn validator_make_ignores_the_data_argument() {
    let content = "<?php
class UserController {
    public function store() {
        $validator = Validator::make(['title' => 'Hi'], ['headline' => 'required']);
        $request->input('');
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    let rules = inline_validate_rules(content, cursor).expect("should find rules");
    assert_eq!(keys(&rules), vec!["headline"]);
}

#[test]
fn ignores_validate_calls_after_the_cursor() {
    let content = "<?php
class UserController {
    public function store(Request $request) {
        $request->input('');
        $request->validate(['title' => 'required']);
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    assert!(inline_validate_rules(content, cursor).is_none());
}

#[test]
fn ignores_validate_calls_in_a_sibling_method() {
    let content = "<?php
class UserController {
    public function update(Request $request) {
        $request->validate(['title' => 'required']);
    }
    public function store(Request $request) {
        $request->input('');
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    assert!(inline_validate_rules(content, cursor).is_none());
}

#[test]
fn rules_reach_into_a_closure_nested_in_the_same_method() {
    let content = "<?php
class UserController {
    public function store(Request $request) {
        $request->validate(['title' => 'required']);
        DB::transaction(function () use ($request) {
            $request->input('');
        });
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    let rules = inline_validate_rules(content, cursor).expect("closure is still the same method");
    assert_eq!(keys(&rules), vec!["title"]);
}

#[test]
fn prefers_the_nearest_preceding_validate_call() {
    let content = "<?php
class UserController {
    public function store(Request $request) {
        $request->validate(['first' => 'required']);
        $request->validate(['second' => 'required']);
        $request->input('');
    }
}
";
    let cursor = content.find("input('").unwrap() + 7;
    let rules = inline_validate_rules(content, cursor).unwrap();
    assert_eq!(keys(&rules), vec!["second"]);
}

#[test]
fn non_literal_keys_are_skipped() {
    let content = "<?php
class StoreUserRequest extends FormRequest {
    public function rules(): array {
        return [
            'name' => 'required',
            $dynamic => 'required',
        ];
    }
}
";
    assert_eq!(
        keys(&rules_from_class_source(content, "StoreUserRequest")),
        vec!["name"]
    );
}

// ─── Field expansion ────────────────────────────────────────────────────────

fn rule(key: &str) -> ValidationRule {
    ValidationRule {
        key: key.to_string(),
        rules: "required".to_string(),
        key_start: 0,
    }
}

#[test]
fn wildcard_keys_collapse_to_their_root() {
    let rules = vec![rule("items"), rule("items.*.id")];
    let fields = rule_fields(&rules);
    let names: Vec<&String> = fields.iter().map(|f| &f.name).collect();
    assert_eq!(names, vec!["items"]);
}

#[test]
fn wildcard_root_is_offered_even_without_its_own_rule() {
    let rules = vec![rule("items.*.id")];
    let fields = rule_fields(&rules);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "items");
    assert!(fields[0].rules.is_empty());
}

#[test]
fn plain_dotted_keys_are_offered_whole_and_by_root() {
    let rules = vec![rule("address.city")];
    let fields = rule_fields(&rules);
    let names: Vec<&String> = fields.iter().map(|f| &f.name).collect();
    assert_eq!(names, vec!["address.city", "address"]);
    assert_eq!(fields[0].rules, "required");
    assert!(fields[1].rules.is_empty());
}
