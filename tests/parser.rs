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

    let create = classes[0]
        .methods
        .iter()
        .find(|m| m.name == "create")
        .unwrap();
    assert!(create.is_static);
    assert_eq!(create.return_type.as_deref(), Some("static"));

    let build = classes[0]
        .methods
        .iter()
        .find(|m| m.name == "build")
        .unwrap();
    assert!(!build.is_static);
}

// ─── Promoted Property Tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_promoted_properties_basic() {
    let backend = create_test_backend();
    let php = r#"<?php
class Service {
    public function __construct(
        private IShoppingCart $cart,
        protected Logger $logger,
    ) {}
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let cls = &classes[0];
    assert_eq!(
        cls.properties.len(),
        2,
        "Should extract 2 promoted properties"
    );

    let cart = cls.properties.iter().find(|p| p.name == "cart").unwrap();
    assert_eq!(cart.type_hint.as_deref(), Some("IShoppingCart"));
    assert_eq!(cart.visibility, Visibility::Private);
    assert!(!cart.is_static);

    let logger = cls.properties.iter().find(|p| p.name == "logger").unwrap();
    assert_eq!(logger.type_hint.as_deref(), Some("Logger"));
    assert_eq!(logger.visibility, Visibility::Protected);
    assert!(!logger.is_static);
}

#[tokio::test]
async fn test_parse_php_promoted_properties_mixed_with_regular() {
    let backend = create_test_backend();
    let php = r#"<?php
class ShoppingCartService {
    private IShoppingCart $regular;

    public function __construct(
        private IShoppingCart $promoted,
    ) {}
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let cls = &classes[0];
    assert_eq!(
        cls.properties.len(),
        2,
        "Should have regular + promoted property"
    );

    let regular = cls.properties.iter().find(|p| p.name == "regular").unwrap();
    assert_eq!(regular.type_hint.as_deref(), Some("IShoppingCart"));
    assert_eq!(regular.visibility, Visibility::Private);

    let promoted = cls
        .properties
        .iter()
        .find(|p| p.name == "promoted")
        .unwrap();
    assert_eq!(promoted.type_hint.as_deref(), Some("IShoppingCart"));
    assert_eq!(promoted.visibility, Visibility::Private);
}

#[tokio::test]
async fn test_parse_php_promoted_property_public_visibility() {
    let backend = create_test_backend();
    let php = r#"<?php
class Config {
    public function __construct(
        public string $name,
        public int $value,
    ) {}
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let cls = &classes[0];
    assert_eq!(cls.properties.len(), 2);

    for prop in &cls.properties {
        assert_eq!(prop.visibility, Visibility::Public);
    }

    let name = cls.properties.iter().find(|p| p.name == "name").unwrap();
    assert_eq!(name.type_hint.as_deref(), Some("string"));

    let value = cls.properties.iter().find(|p| p.name == "value").unwrap();
    assert_eq!(value.type_hint.as_deref(), Some("int"));
}

#[tokio::test]
async fn test_parse_php_non_promoted_constructor_params_ignored() {
    let backend = create_test_backend();
    let php = r#"<?php
class Service {
    public function __construct(
        private string $promoted,
        string $regularParam,
    ) {}
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let cls = &classes[0];
    assert_eq!(
        cls.properties.len(),
        1,
        "Only promoted params (with visibility) should become properties"
    );
    assert_eq!(cls.properties[0].name, "promoted");
}

#[tokio::test]
async fn test_parse_php_promoted_property_readonly() {
    let backend = create_test_backend();
    let php = r#"<?php
class User {
    public function __construct(
        public readonly string $name,
        private readonly int $id,
    ) {}
}
"#;

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let cls = &classes[0];
    assert_eq!(
        cls.properties.len(),
        2,
        "readonly promoted params are still promoted"
    );

    let name = cls.properties.iter().find(|p| p.name == "name").unwrap();
    assert_eq!(name.visibility, Visibility::Public);
    assert_eq!(name.type_hint.as_deref(), Some("string"));

    let id = cls.properties.iter().find(|p| p.name == "id").unwrap();
    assert_eq!(id.visibility, Visibility::Private);
    assert_eq!(id.type_hint.as_deref(), Some("int"));
}

// ─── Standalone Function Parsing Tests ──────────────────────────────────────

#[tokio::test]
async fn test_parse_functions_standalone() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "function hello(): void {}\n",
        "function add(int $a, int $b): int { return $a + $b; }\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(functions.len(), 2, "Should extract 2 standalone functions");

    let hello = functions.iter().find(|f| f.name == "hello").unwrap();
    assert!(hello.parameters.is_empty());
    assert_eq!(hello.return_type.as_deref(), Some("void"));
    assert!(hello.namespace.is_none());

    let add = functions.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add.parameters.len(), 2);
    assert_eq!(add.parameters[0].name, "$a");
    assert_eq!(add.parameters[0].type_hint.as_deref(), Some("int"));
    assert_eq!(add.parameters[1].name, "$b");
    assert_eq!(add.return_type.as_deref(), Some("int"));
    assert!(add.namespace.is_none());
}

#[tokio::test]
async fn test_parse_functions_inside_namespace() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace Amp;\n",
        "\n",
        "function delay(float $seconds): void {}\n",
        "function async(callable $callback): void {}\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(functions.len(), 2, "Should extract 2 namespaced functions");

    let delay = functions.iter().find(|f| f.name == "delay").unwrap();
    assert_eq!(delay.namespace.as_deref(), Some("Amp"));
    assert_eq!(delay.parameters.len(), 1);
    assert_eq!(delay.parameters[0].name, "$seconds");
    assert_eq!(delay.parameters[0].type_hint.as_deref(), Some("float"));
    assert_eq!(delay.return_type.as_deref(), Some("void"));

    let async_fn = functions.iter().find(|f| f.name == "async").unwrap();
    assert_eq!(async_fn.namespace.as_deref(), Some("Amp"));
}

#[tokio::test]
async fn test_parse_functions_ignores_class_methods() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "function standalone(): void {}\n",
        "class Service {\n",
        "    public function handle(): void {}\n",
        "}\n",
        "function another(): string { return ''; }\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(
        functions.len(),
        2,
        "Should only extract standalone functions, not class methods"
    );
    assert!(functions.iter().any(|f| f.name == "standalone"));
    assert!(functions.iter().any(|f| f.name == "another"));
    assert!(
        !functions.iter().any(|f| f.name == "handle"),
        "Class methods should not appear"
    );
}

#[tokio::test]
async fn test_parse_functions_no_return_type() {
    let backend = create_test_backend();
    let php = "<?php\nfunction legacy($x, $y) { return $x + $y; }\n";

    let functions = backend.parse_functions(php);
    assert_eq!(functions.len(), 1);

    let f = &functions[0];
    assert_eq!(f.name, "legacy");
    assert!(f.return_type.is_none(), "No return type hint");
    assert_eq!(f.parameters.len(), 2);
    assert!(f.parameters[0].type_hint.is_none());
}

#[tokio::test]
async fn test_parse_functions_nullable_and_union_types() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "function maybe(?string $val): ?int { return null; }\n",
        "function either(string|int $val): string|false { return ''; }\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(functions.len(), 2);

    let maybe = functions.iter().find(|f| f.name == "maybe").unwrap();
    assert_eq!(maybe.parameters[0].type_hint.as_deref(), Some("?string"));
    assert_eq!(maybe.return_type.as_deref(), Some("?int"));

    let either = functions.iter().find(|f| f.name == "either").unwrap();
    assert_eq!(
        either.parameters[0].type_hint.as_deref(),
        Some("string|int")
    );
    assert_eq!(either.return_type.as_deref(), Some("string|false"));
}

#[tokio::test]
async fn test_parse_functions_variadic_and_reference() {
    let backend = create_test_backend();
    let php = "<?php\nfunction gather(string ...$items): array { return $items; }\nfunction swap(int &$a, int &$b): void {}\n";

    let functions = backend.parse_functions(php);
    assert_eq!(functions.len(), 2);

    let gather = functions.iter().find(|f| f.name == "gather").unwrap();
    assert_eq!(gather.parameters.len(), 1);
    assert!(gather.parameters[0].is_variadic);
    assert!(!gather.parameters[0].is_reference);
    assert!(!gather.parameters[0].is_required);

    let swap = functions.iter().find(|f| f.name == "swap").unwrap();
    assert_eq!(swap.parameters.len(), 2);
    assert!(swap.parameters[0].is_reference);
    assert!(!swap.parameters[0].is_variadic);
}

#[tokio::test]
async fn test_parse_functions_brace_delimited_namespace() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace Foo\\Bar {\n",
        "    function helper(): void {}\n",
        "}\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name, "helper");
    assert_eq!(functions[0].namespace.as_deref(), Some("Foo\\Bar"));
}

#[tokio::test]
async fn test_parse_functions_empty_file() {
    let backend = create_test_backend();
    let php = "<?php\n// nothing here\n";

    let functions = backend.parse_functions(php);
    assert!(functions.is_empty(), "No functions in an empty file");
}

// ─── Functions inside if-guards ─────────────────────────────────────────────

#[tokio::test]
async fn test_parse_functions_inside_function_exists_guard() {
    let backend = create_test_backend();
    // This is the exact pattern used by Laravel helpers.php and many other
    // PHP libraries: functions wrapped in `if (! function_exists(...))`.
    let php = concat!(
        "<?php\n",
        "\n",
        "if (! function_exists('session')) {\n",
        "    /**\n",
        "     * Get / set the specified session value.\n",
        "     */\n",
        "    function session($key = null, $default = null)\n",
        "    {\n",
        "        if (is_null($key)) {\n",
        "            return app('session');\n",
        "        }\n",
        "        return app('session')->get($key, $default);\n",
        "    }\n",
        "}\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(
        functions.len(),
        1,
        "Should find function inside if-guard block"
    );
    assert_eq!(functions[0].name, "session");
    assert_eq!(functions[0].parameters.len(), 2);
    assert!(
        functions[0].return_type.is_none(),
        "session() has no return type hint"
    );
}

#[tokio::test]
async fn test_parse_functions_multiple_function_exists_guards() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "\n",
        "if (! function_exists('app')) {\n",
        "    function app(?string $abstract = null, array $parameters = []): mixed\n",
        "    {\n",
        "        return Container::getInstance();\n",
        "    }\n",
        "}\n",
        "\n",
        "if (! function_exists('session')) {\n",
        "    function session($key = null, $default = null)\n",
        "    {\n",
        "        return app('session');\n",
        "    }\n",
        "}\n",
        "\n",
        "if (! function_exists('config')) {\n",
        "    function config($key = null, $default = null): mixed\n",
        "    {\n",
        "        return null;\n",
        "    }\n",
        "}\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(
        functions.len(),
        3,
        "Should find all 3 functions inside separate if-guards"
    );

    let names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"app"), "Should find app()");
    assert!(names.contains(&"session"), "Should find session()");
    assert!(names.contains(&"config"), "Should find config()");

    let app = functions.iter().find(|f| f.name == "app").unwrap();
    assert_eq!(app.return_type.as_deref(), Some("mixed"));

    let config = functions.iter().find(|f| f.name == "config").unwrap();
    assert_eq!(config.return_type.as_deref(), Some("mixed"));
}

#[tokio::test]
async fn test_parse_functions_inside_namespace_with_function_exists_guard() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace Illuminate\\Support;\n",
        "\n",
        "if (! function_exists('Illuminate\\Support\\enum_value')) {\n",
        "    function enum_value($value): mixed\n",
        "    {\n",
        "        return $value;\n",
        "    }\n",
        "}\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(
        functions.len(),
        1,
        "Should find function inside if-guard within namespace"
    );
    assert_eq!(functions[0].name, "enum_value");
    assert_eq!(
        functions[0].namespace.as_deref(),
        Some("Illuminate\\Support"),
        "Should preserve namespace context"
    );
    assert_eq!(functions[0].return_type.as_deref(), Some("mixed"));
}

#[tokio::test]
async fn test_parse_functions_mixed_guarded_and_unguarded() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "\n",
        "function always_defined(): void {}\n",
        "\n",
        "if (! function_exists('maybe_defined')) {\n",
        "    function maybe_defined(): string { return ''; }\n",
        "}\n",
        "\n",
        "function also_always(): int { return 0; }\n",
    );

    let functions = backend.parse_functions(php);
    assert_eq!(
        functions.len(),
        3,
        "Should find both guarded and unguarded functions"
    );

    let names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"always_defined"));
    assert!(names.contains(&"maybe_defined"));
    assert!(names.contains(&"also_always"));
}

// ─── Enum Parsing ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_extracts_backed_enum_cases_as_constants() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum CustomerAvailabilityStatus: int\n",
        "{\n",
        "    case CUSTOMER_NOT_IN_AUDIENCE = -1;\n",
        "    case AVAILABLE_TO_CUSTOMER = 0;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1, "Should parse the enum as a class-like");
    assert_eq!(classes[0].name, "CustomerAvailabilityStatus");
    assert_eq!(
        classes[0].constants.len(),
        2,
        "Enum cases should be extracted as constants"
    );

    let case0 = &classes[0].constants[0];
    assert_eq!(case0.name, "CUSTOMER_NOT_IN_AUDIENCE");
    assert_eq!(
        case0.visibility,
        phpantom_lsp::types::Visibility::Public,
        "Enum cases are always public"
    );

    let case1 = &classes[0].constants[1];
    assert_eq!(case1.name, "AVAILABLE_TO_CUSTOMER");
}

#[tokio::test]
async fn test_parse_php_extracts_unit_enum_cases() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum Color\n",
        "{\n",
        "    case Red;\n",
        "    case Green;\n",
        "    case Blue;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Color");
    assert_eq!(
        classes[0].constants.len(),
        3,
        "Unit enum cases should be extracted as constants"
    );

    let names: Vec<&str> = classes[0]
        .constants
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(names, vec!["Red", "Green", "Blue"]);
}

#[tokio::test]
async fn test_parse_php_extracts_enum_with_methods() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum Suit: string\n",
        "{\n",
        "    case Hearts = 'H';\n",
        "    case Diamonds = 'D';\n",
        "    case Clubs = 'C';\n",
        "    case Spades = 'S';\n",
        "\n",
        "    public function color(): string\n",
        "    {\n",
        "        return match($this) {\n",
        "            Suit::Hearts, Suit::Diamonds => 'red',\n",
        "            Suit::Clubs, Suit::Spades => 'black',\n",
        "        };\n",
        "    }\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Suit");
    assert_eq!(classes[0].constants.len(), 4, "Should have 4 enum cases");
    assert_eq!(classes[0].methods.len(), 1, "Should have 1 method");
    assert_eq!(classes[0].methods[0].name, "color");
    assert_eq!(classes[0].methods[0].return_type.as_deref(), Some("string"));
}

#[tokio::test]
async fn test_parse_php_extracts_enum_with_constants_and_cases() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum Status: int\n",
        "{\n",
        "    const DEFAULT_STATUS = 0;\n",
        "    case Active = 1;\n",
        "    case Inactive = 2;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Status");
    // Both the `const` and the `case` entries should appear as constants.
    assert_eq!(
        classes[0].constants.len(),
        3,
        "Should have 1 real constant + 2 enum cases"
    );

    let names: Vec<&str> = classes[0]
        .constants
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(names.contains(&"DEFAULT_STATUS"));
    assert!(names.contains(&"Active"));
    assert!(names.contains(&"Inactive"));
}

#[tokio::test]
async fn test_parse_php_extracts_enum_inside_namespace() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace App\\Enums;\n",
        "\n",
        "enum Direction\n",
        "{\n",
        "    case Up;\n",
        "    case Down;\n",
        "    case Left;\n",
        "    case Right;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Direction");
    assert_eq!(classes[0].constants.len(), 4);
}

// ─── Implicit UnitEnum / BackedEnum interface tests ─────────────────────────

/// A unit enum (no backing type) should have `UnitEnum` in its `used_traits`.
#[tokio::test]
async fn test_parse_php_unit_enum_has_implicit_unit_enum_interface() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum Color\n",
        "{\n",
        "    case Red;\n",
        "    case Green;\n",
        "    case Blue;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Color");
    // parse_php returns raw names before resolution; the leading backslash
    // marks the name as fully-qualified so resolve_name won't prepend a
    // namespace later.
    assert!(
        classes[0].used_traits.iter().any(|t| t == "\\UnitEnum"),
        "Unit enum should implicitly implement \\UnitEnum, got used_traits: {:?}",
        classes[0].used_traits
    );
    assert!(
        !classes[0].used_traits.iter().any(|t| t == "\\BackedEnum"),
        "Unit enum should NOT implement \\BackedEnum, got used_traits: {:?}",
        classes[0].used_traits
    );
}

/// A backed enum (`: int`) should have `BackedEnum` in its `used_traits`.
#[tokio::test]
async fn test_parse_php_backed_int_enum_has_implicit_backed_enum_interface() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum Priority: int\n",
        "{\n",
        "    case Low = 0;\n",
        "    case High = 1;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Priority");
    assert!(
        classes[0].used_traits.iter().any(|t| t == "\\BackedEnum"),
        "Backed int enum should implicitly implement \\BackedEnum, got used_traits: {:?}",
        classes[0].used_traits
    );
    assert!(
        !classes[0].used_traits.iter().any(|t| t == "\\UnitEnum"),
        "Backed enum should NOT implement \\UnitEnum, got used_traits: {:?}",
        classes[0].used_traits
    );
}

/// A backed enum (`: string`) should have `BackedEnum` in its `used_traits`.
#[tokio::test]
async fn test_parse_php_backed_string_enum_has_implicit_backed_enum_interface() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "enum Suit: string\n",
        "{\n",
        "    case Hearts = 'H';\n",
        "    case Spades = 'S';\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Suit");
    assert!(
        classes[0].used_traits.iter().any(|t| t == "\\BackedEnum"),
        "Backed string enum should implicitly implement \\BackedEnum, got used_traits: {:?}",
        classes[0].used_traits
    );
}

/// An enum inside a namespace should still have UnitEnum/BackedEnum resolved
/// to the root namespace (not prefixed with the current namespace).
#[tokio::test]
async fn test_parse_php_namespaced_enum_implicit_interface_is_not_namespace_prefixed() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace App\\Enums;\n",
        "\n",
        "enum Mode\n",
        "{\n",
        "    case Automatic;\n",
        "    case Manual;\n",
        "}\n",
    );

    let mut classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Mode");
    // parse_php returns the raw `\UnitEnum` (leading backslash marks it as
    // fully-qualified).  After resolve_parent_class_names the backslash is
    // stripped to `UnitEnum`, which is the root-namespace FQN.
    assert!(
        classes[0].used_traits.iter().any(|t| t == "\\UnitEnum"),
        "Namespaced unit enum should have \\UnitEnum before resolution, got: {:?}",
        classes[0].used_traits
    );

    // Simulate the resolution step that update_ast performs.
    let use_map = std::collections::HashMap::new();
    let namespace = Some("App\\Enums".to_string());
    phpantom_lsp::Backend::resolve_parent_class_names(&mut classes, &use_map, &namespace);

    // After resolution the leading backslash is stripped and the name stays
    // in the root namespace — NOT prefixed with `App\Enums\`.
    assert!(
        classes[0].used_traits.iter().any(|t| t == "UnitEnum"),
        "After resolution, should be UnitEnum (root namespace), got: {:?}",
        classes[0].used_traits
    );
    assert!(
        !classes[0]
            .used_traits
            .iter()
            .any(|t| t == "App\\Enums\\UnitEnum"),
        "Should NOT be namespace-prefixed as App\\Enums\\UnitEnum, got: {:?}",
        classes[0].used_traits
    );
}

/// An enum that explicitly uses a trait should have both the trait and the
/// implicit interface in `used_traits`.
#[tokio::test]
async fn test_parse_php_enum_with_trait_also_has_implicit_interface() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "trait HasDescription {\n",
        "    public function describe(): string { return 'desc'; }\n",
        "}\n",
        "\n",
        "enum Status: int\n",
        "{\n",
        "    use HasDescription;\n",
        "\n",
        "    case Active = 1;\n",
        "    case Inactive = 0;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    let enum_info = classes.iter().find(|c| c.name == "Status").unwrap();
    assert!(
        enum_info.used_traits.iter().any(|t| t == "HasDescription"),
        "Should include the explicit trait, got: {:?}",
        enum_info.used_traits
    );
    assert!(
        enum_info.used_traits.iter().any(|t| t == "\\BackedEnum"),
        "Should include implicit \\BackedEnum, got: {:?}",
        enum_info.used_traits
    );
}
