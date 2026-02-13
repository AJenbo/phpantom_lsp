use phpantom_lsp::composer::{normalise_path, parse_composer_json, resolve_class_path};
use std::fs;
use std::path::Path;

/// Helper: create a temporary workspace with a composer.json and
/// optional PHP class files.
struct TestWorkspace {
    dir: tempfile::TempDir,
}

impl TestWorkspace {
    fn new(composer_json: &str) -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(dir.path().join("composer.json"), composer_json)
            .expect("failed to write composer.json");
        TestWorkspace { dir }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Create a PHP file at the given relative path with minimal content.
    fn create_php_file(&self, relative_path: &str, content: &str) {
        let full_path = self.dir.path().join(relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("failed to create dirs");
        }
        fs::write(&full_path, content).expect("failed to write PHP file");
    }
}

#[test]
fn test_parse_basic_psr4() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].prefix, "Klarna\\");
    assert_eq!(mappings[0].base_path, "src/Klarna/");
}

#[test]
fn test_parse_autoload_dev() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            },
            "autoload-dev": {
                "psr-4": {
                    "Klarna\\Rest\\Tests\\": "tests/"
                }
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());
    assert_eq!(mappings.len(), 2);

    // Longest prefix first
    assert_eq!(mappings[0].prefix, "Klarna\\Rest\\Tests\\");
    assert_eq!(mappings[0].base_path, "tests/");
    assert_eq!(mappings[1].prefix, "Klarna\\");
    assert_eq!(mappings[1].base_path, "src/Klarna/");
}

#[test]
fn test_parse_array_paths() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": ["src/", "lib/"]
                }
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());
    assert_eq!(mappings.len(), 2);
    assert_eq!(mappings[0].prefix, "App\\");
    assert_eq!(mappings[0].base_path, "src/");
    assert_eq!(mappings[1].prefix, "App\\");
    assert_eq!(mappings[1].base_path, "lib/");
}

#[test]
fn test_parse_no_composer_json() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let mappings = parse_composer_json(dir.path());
    assert!(mappings.is_empty());
}

#[test]
fn test_parse_invalid_json() {
    let ws = TestWorkspace::new("not valid json {{{");
    let mappings = parse_composer_json(ws.root());
    assert!(mappings.is_empty());
}

#[test]
fn test_parse_no_psr4_section() {
    let ws = TestWorkspace::new(
        r#"{
            "name": "vendor/project",
            "autoload": {
                "classmap": ["src/"]
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());
    assert!(mappings.is_empty());
}

#[test]
fn test_resolve_simple_class() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
    );
    ws.create_php_file(
        "src/Klarna/Customer.php",
        "<?php\nnamespace Klarna;\nclass Customer {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "Klarna\\Customer");

    assert!(result.is_some());
    let path = result.unwrap();
    assert!(path.ends_with("src/Klarna/Customer.php"));
}

#[test]
fn test_resolve_nested_namespace() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
    );
    ws.create_php_file(
        "src/Klarna/Rest/Order.php",
        "<?php\nnamespace Klarna\\Rest;\nclass Order {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "Klarna\\Rest\\Order");

    assert!(result.is_some());
    let path = result.unwrap();
    assert!(path.ends_with("src/Klarna/Rest/Order.php"));
}

#[test]
fn test_resolve_strips_leading_backslash() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
    );
    ws.create_php_file(
        "src/Klarna/Customer.php",
        "<?php\nnamespace Klarna;\nclass Customer {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "\\Klarna\\Customer");

    assert!(result.is_some());
}

#[test]
fn test_resolve_nonexistent_file_returns_none() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "Klarna\\DoesNotExist");

    assert!(result.is_none());
}

#[test]
fn test_resolve_no_matching_prefix() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "Acme\\Foo");

    assert!(result.is_none());
}

#[test]
fn test_resolve_longest_prefix_wins() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/",
                    "Klarna\\Rest\\Tests\\": "tests/"
                }
            }
        }"#,
    );
    ws.create_php_file(
        "tests/OrderTest.php",
        "<?php\nnamespace Klarna\\Rest\\Tests;\nclass OrderTest {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "Klarna\\Rest\\Tests\\OrderTest");

    assert!(result.is_some());
    let path = result.unwrap();
    assert!(path.ends_with("tests/OrderTest.php"));
}

#[test]
fn test_resolve_builtin_types_return_none() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "": "src/"
                }
            }
        }"#,
    );

    let mappings = parse_composer_json(ws.root());

    for builtin in &[
        "self", "static", "parent", "string", "int", "float", "bool", "array", "object", "mixed",
        "void", "never", "null", "true", "false", "callable", "iterable",
    ] {
        assert!(
            resolve_class_path(&mappings, ws.root(), builtin).is_none(),
            "builtin type '{}' should not resolve",
            builtin
        );
    }
}

#[test]
fn test_resolve_array_paths_first_match() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": ["src/", "lib/"]
                }
            }
        }"#,
    );
    // File exists only in lib/
    ws.create_php_file(
        "lib/Service.php",
        "<?php\nnamespace App;\nclass Service {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "App\\Service");

    assert!(result.is_some());
    let path = result.unwrap();
    assert!(path.ends_with("lib/Service.php"));
}

#[test]
fn test_normalise_path_adds_trailing_slash() {
    assert_eq!(normalise_path("src"), "src/");
    assert_eq!(normalise_path("src/"), "src/");
    assert_eq!(normalise_path(""), "");
}

#[test]
fn test_normalise_path_converts_backslashes() {
    assert_eq!(normalise_path("src\\Klarna\\"), "src/Klarna/");
}

#[test]
fn test_prefix_without_trailing_backslash() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "App": "src/"
                }
            }
        }"#,
    );
    ws.create_php_file(
        "src/Service.php",
        "<?php\nnamespace App;\nclass Service {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    // The prefix gets normalised to "App\"
    assert_eq!(mappings[0].prefix, "App\\");

    let result = resolve_class_path(&mappings, ws.root(), "App\\Service");
    assert!(result.is_some());
}
