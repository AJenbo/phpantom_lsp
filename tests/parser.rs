mod common;

use common::create_test_backend;
use phpantom_lsp::Visibility;

// ─── PHP Parsing / AST Extraction Tests ─────────────────────────────────────

#[tokio::test]
async fn test_parse_php_extracts_class_and_methods() {
    let backend = create_test_backend();
    let php = "<?php\nclass User {\n    function login() {}\n    function logout() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods.len(), 2);
    assert_eq!(classes[0].methods[0].name, "login");
    assert_eq!(classes[0].methods[1].name, "logout");
}

#[tokio::test]
async fn test_parse_php_ignores_standalone_functions() {
    let backend = create_test_backend();
    let php = "<?php\nfunction standalone() {}\nclass Service {\n    function handle() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(
        classes.len(),
        1,
        "Only class declarations should be extracted"
    );
    assert_eq!(classes[0].name, "Service");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "handle");
}

#[tokio::test]
async fn test_parse_php_no_classes_returns_empty() {
    let backend = create_test_backend();
    let php = "<?php\nfunction foo() {}\n$x = 1;\n";

    let classes = backend.parse_php(php);
    assert!(classes.is_empty(), "No classes should be found");
}

#[tokio::test]
async fn test_parse_php_extracts_properties() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public int $age;\n",
        "    private $secret;\n",
        "    function login() {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(
        classes[0].properties.len(),
        3,
        "Should extract 3 properties"
    );

    let prop_names: Vec<&str> = classes[0]
        .properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(prop_names.contains(&"name"), "Should contain 'name'");
    assert!(prop_names.contains(&"age"), "Should contain 'age'");
    assert!(prop_names.contains(&"secret"), "Should contain 'secret'");

    // Verify type hints
    let name_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "name")
        .unwrap();
    assert_eq!(
        name_prop.type_hint.as_deref(),
        Some("string"),
        "name property should have string type hint"
    );

    let age_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "age")
        .unwrap();
    assert_eq!(
        age_prop.type_hint.as_deref(),
        Some("int"),
        "age property should have int type hint"
    );

    let secret_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "secret")
        .unwrap();
    assert_eq!(
        secret_prop.type_hint, None,
        "secret property should have no type hint"
    );
}

#[tokio::test]
async fn test_parse_php_extracts_static_properties() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Counter {\n",
        "    public static int $count = 0;\n",
        "    public string $label;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].properties.len(), 2);

    let count_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "count")
        .expect("Should have count property");
    assert!(count_prop.is_static, "count should be static");

    let label_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "label")
        .expect("Should have label property");
    assert!(!label_prop.is_static, "label should not be static");
}

#[tokio::test]
async fn test_parse_php_extracts_method_return_type() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Greeter {\n",
        "    function greet(string $name): string {}\n",
        "    function doStuff() {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].methods.len(), 2);

    let greet = &classes[0].methods[0];
    assert_eq!(greet.name, "greet");
    assert_eq!(
        greet.return_type.as_deref(),
        Some("string"),
        "greet should have return type 'string'"
    );
    assert_eq!(greet.parameters.len(), 1);
    assert_eq!(greet.parameters[0].name, "$name");
    assert!(greet.parameters[0].is_required);
    assert_eq!(greet.parameters[0].type_hint.as_deref(), Some("string"));

    let do_stuff = &classes[0].methods[1];
    assert_eq!(do_stuff.name, "doStuff");
    assert_eq!(
        do_stuff.return_type, None,
        "doStuff should have no return type"
    );
}

#[tokio::test]
async fn test_parse_php_method_parameter_info() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Service {\n",
        "    function process(string $input, int $count, ?string $label = null, ...$extras): bool {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let method = &classes[0].methods[0];
    assert_eq!(method.name, "process");
    assert_eq!(method.parameters.len(), 4);

    let input = &method.parameters[0];
    assert_eq!(input.name, "$input");
    assert!(input.is_required);
    assert_eq!(input.type_hint.as_deref(), Some("string"));
    assert!(!input.is_variadic);

    let count = &method.parameters[1];
    assert_eq!(count.name, "$count");
    assert!(count.is_required);
    assert_eq!(count.type_hint.as_deref(), Some("int"));

    let label = &method.parameters[2];
    assert_eq!(label.name, "$label");
    assert!(
        !label.is_required,
        "$label has a default value, should not be required"
    );
    assert_eq!(label.type_hint.as_deref(), Some("?string"));

    let extras = &method.parameters[3];
    assert_eq!(extras.name, "$extras");
    assert!(
        !extras.is_required,
        "variadic params should not be required"
    );
    assert!(extras.is_variadic);
}

#[tokio::test]
async fn test_parse_php_property_with_default_value() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Settings {\n",
        "    public bool $debug = false;\n",
        "    public string $title = 'default';\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].properties.len(), 2);

    let prop_names: Vec<&str> = classes[0]
        .properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(prop_names.contains(&"debug"));
    assert!(prop_names.contains(&"title"));
}

#[tokio::test]
async fn test_parse_php_class_inside_implicit_namespace() {
    let backend = create_test_backend();
    let php = "<?php\nnamespace Demo;\n\nclass User {\n    function login() {}\n    function logout() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(
        classes.len(),
        1,
        "Should find class inside implicit namespace"
    );
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods.len(), 2);
    assert_eq!(classes[0].methods[0].name, "login");
    assert_eq!(classes[0].methods[1].name, "logout");
}

#[tokio::test]
async fn test_parse_php_class_inside_brace_delimited_namespace() {
    let backend = create_test_backend();
    let php =
        "<?php\nnamespace Demo {\n    class Service {\n        function handle() {}\n    }\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(
        classes.len(),
        1,
        "Should find class inside brace-delimited namespace"
    );
    assert_eq!(classes[0].name, "Service");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "handle");
}

#[tokio::test]
async fn test_parse_php_multiple_classes_in_brace_delimited_namespaces() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace Foo {\n",
        "    class A {\n",
        "        function doA() {}\n",
        "    }\n",
        "}\n",
        "namespace Bar {\n",
        "    class B {\n",
        "        function doB() {}\n",
        "    }\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 2, "Should find classes in both namespaces");
    assert_eq!(classes[0].name, "A");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "doA");
    assert_eq!(classes[1].name, "B");
    assert_eq!(classes[1].methods.len(), 1);
    assert_eq!(classes[1].methods[0].name, "doB");
}

#[tokio::test]
async fn test_parse_php_static_method() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public static function create(string $type): self {}\n",
        "    public function build(): void {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].methods.len(), 2);

    let create = &classes[0].methods[0];
    assert_eq!(create.name, "create");
    assert!(create.is_static, "create should be static");
    assert_eq!(create.parameters.len(), 1);
    assert_eq!(create.parameters[0].name, "$type");

    let build = &classes[0].methods[1];
    assert_eq!(build.name, "build");
    assert!(!build.is_static, "build should not be static");
}

#[tokio::test]
async fn test_parse_php_extracts_constants() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Config {\n",
        "    const VERSION = '1.0';\n",
        "    const int MAX_RETRIES = 3;\n",
        "    public string $name;\n",
        "    public function getName(): string {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].constants.len(), 2);

    let version = &classes[0].constants[0];
    assert_eq!(version.name, "VERSION");
    assert!(version.type_hint.is_none(), "VERSION has no type hint");

    let max_retries = &classes[0].constants[1];
    assert_eq!(max_retries.name, "MAX_RETRIES");
    assert_eq!(
        max_retries.type_hint.as_deref(),
        Some("int"),
        "MAX_RETRIES should have int type hint"
    );
}

#[tokio::test]
async fn test_parse_php_extracts_multiple_constants_in_one_declaration() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Status {\n",
        "    const ACTIVE = 1, INACTIVE = 0;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].constants.len(), 2);
    assert_eq!(classes[0].constants[0].name, "ACTIVE");
    assert_eq!(classes[0].constants[1].name, "INACTIVE");
}

#[tokio::test]
async fn test_parse_php_extracts_parent_class() {
    let backend = create_test_backend();
    let classes = backend.parse_php(concat!(
        "<?php\n",
        "class Animal {\n",
        "    public function breathe(): void {}\n",
        "}\n",
        "class Dog extends Animal {\n",
        "    public function bark(): void {}\n",
        "}\n",
    ));

    assert_eq!(classes.len(), 2);
    assert_eq!(classes[0].name, "Animal");
    assert!(classes[0].parent_class.is_none());
    assert_eq!(classes[1].name, "Dog");
    assert_eq!(classes[1].parent_class.as_deref(), Some("Animal"));
}

#[tokio::test]
async fn test_parse_php_extracts_visibility() {
    let backend = create_test_backend();
    let classes = backend.parse_php(concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function pubMethod(): void {}\n",
        "    protected function protMethod(): void {}\n",
        "    private function privMethod(): void {}\n",
        "    function defaultMethod(): void {}\n",
        "    public string $pubProp;\n",
        "    protected string $protProp;\n",
        "    private string $privProp;\n",
        "    public const PUB_CONST = 1;\n",
        "    protected const PROT_CONST = 2;\n",
        "    private const PRIV_CONST = 3;\n",
        "    const DEFAULT_CONST = 4;\n",
        "}\n",
    ));

    assert_eq!(classes.len(), 1);
    let cls = &classes[0];

    // Methods
    let pub_m = cls.methods.iter().find(|m| m.name == "pubMethod").unwrap();
    assert_eq!(pub_m.visibility, Visibility::Public);
    let prot_m = cls.methods.iter().find(|m| m.name == "protMethod").unwrap();
    assert_eq!(prot_m.visibility, Visibility::Protected);
    let priv_m = cls.methods.iter().find(|m| m.name == "privMethod").unwrap();
    assert_eq!(priv_m.visibility, Visibility::Private);
    let def_m = cls
        .methods
        .iter()
        .find(|m| m.name == "defaultMethod")
        .unwrap();
    assert_eq!(
        def_m.visibility,
        Visibility::Public,
        "No modifier defaults to public"
    );

    // Properties
    let pub_p = cls.properties.iter().find(|p| p.name == "pubProp").unwrap();
    assert_eq!(pub_p.visibility, Visibility::Public);
    let prot_p = cls
        .properties
        .iter()
        .find(|p| p.name == "protProp")
        .unwrap();
    assert_eq!(prot_p.visibility, Visibility::Protected);
    let priv_p = cls
        .properties
        .iter()
        .find(|p| p.name == "privProp")
        .unwrap();
    assert_eq!(priv_p.visibility, Visibility::Private);

    // Constants
    let pub_c = cls
        .constants
        .iter()
        .find(|c| c.name == "PUB_CONST")
        .unwrap();
    assert_eq!(pub_c.visibility, Visibility::Public);
    let prot_c = cls
        .constants
        .iter()
        .find(|c| c.name == "PROT_CONST")
        .unwrap();
    assert_eq!(prot_c.visibility, Visibility::Protected);
    let priv_c = cls
        .constants
        .iter()
        .find(|c| c.name == "PRIV_CONST")
        .unwrap();
    assert_eq!(priv_c.visibility, Visibility::Private);
    let def_c = cls
        .constants
        .iter()
        .find(|c| c.name == "DEFAULT_CONST")
        .unwrap();
    assert_eq!(
        def_c.visibility,
        Visibility::Public,
        "No modifier defaults to public"
    );
}

// ─── Interface Parsing Tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_extracts_interface_methods() {
    let backend = create_test_backend();
    let php = r#"<?php
interface Loggable {
    public function log(string $message): void;
    public function getLogLevel(): int;
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Loggable");
    assert_eq!(classes[0].methods.len(), 2);
    assert_eq!(classes[0].methods[0].name, "log");
    assert_eq!(classes[0].methods[0].return_type.as_deref(), Some("void"));
    assert_eq!(classes[0].methods[1].name, "getLogLevel");
    assert_eq!(classes[0].methods[1].return_type.as_deref(), Some("int"));
}

#[tokio::test]
async fn test_parse_php_extracts_interface_constants() {
    let backend = create_test_backend();
    let php = r#"<?php
interface HasStatus {
    const STATUS_ACTIVE = 1;
    const STATUS_INACTIVE = 0;
    public function getStatus(): int;
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "HasStatus");
    assert_eq!(classes[0].constants.len(), 2);
    assert_eq!(classes[0].constants[0].name, "STATUS_ACTIVE");
    assert_eq!(classes[0].constants[1].name, "STATUS_INACTIVE");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "getStatus");
}

#[tokio::test]
async fn test_parse_php_interface_extends() {
    let backend = create_test_backend();
    let php = r#"<?php
interface Readable {
    public function read(): string;
}
interface Writable extends Readable {
    public function write(string $data): void;
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 2);

    let readable = classes.iter().find(|c| c.name == "Readable").unwrap();
    assert!(readable.parent_class.is_none());
    assert_eq!(readable.methods.len(), 1);

    let writable = classes.iter().find(|c| c.name == "Writable").unwrap();
    assert_eq!(writable.parent_class.as_deref(), Some("Readable"));
    assert_eq!(writable.methods.len(), 1);
    assert_eq!(writable.methods[0].name, "write");
}

#[tokio::test]
async fn test_parse_php_interface_inside_namespace() {
    let backend = create_test_backend();
    let php = r#"<?php
namespace App\Contracts;

interface Repository {
    public function find(int $id): mixed;
    public function save(object $entity): void;
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Repository");
    assert_eq!(classes[0].methods.len(), 2);
    assert_eq!(classes[0].methods[0].name, "find");
    assert_eq!(classes[0].methods[1].name, "save");
}

#[tokio::test]
async fn test_parse_php_class_and_interface_together() {
    let backend = create_test_backend();
    let php = r#"<?php
interface Cacheable {
    public function getCacheKey(): string;
    const TTL = 3600;
}

class UserRepository implements Cacheable {
    public function getCacheKey(): string { return 'users'; }
    public function findAll(): array { return []; }
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 2);

    let iface = classes.iter().find(|c| c.name == "Cacheable").unwrap();
    assert_eq!(iface.methods.len(), 1);
    assert_eq!(iface.constants.len(), 1);
    assert_eq!(iface.constants[0].name, "TTL");

    let class = classes.iter().find(|c| c.name == "UserRepository").unwrap();
    assert_eq!(class.methods.len(), 2);
}

#[tokio::test]
async fn test_parse_php_interface_static_method() {
    let backend = create_test_backend();
    let php = r#"<?php
interface Factory {
    public static function create(): static;
    public function build(): object;
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Factory");
    assert_eq!(classes[0].methods.len(), 2);

    let create = classes[0].methods.iter().find(|m| m.name == "create").unwrap();
    assert!(create.is_static);
    assert_eq!(create.return_type.as_deref(), Some("static"));

    let build = classes[0].methods.iter().find(|m| m.name == "build").unwrap();
    assert!(!build.is_static);
}
