use phpantom_lsp::composer::{
    normalise_path, parse_composer_json, parse_vendor_autoload_psr4, resolve_class_path,
};
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
fn test_vendor_autoload_basic() {
    let ws = TestWorkspace::new(r#"{"name": "test/project"}"#);

    // Create the vendor autoload file
    ws.create_php_file(
        "vendor/composer/autoload_psr4.php",
        r#"<?php

// autoload_psr4.php @generated by Composer

$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'voku\\' => array($vendorDir . '/voku/portable-ascii/src/voku'),
    'Webmozart\\Assert\\' => array($vendorDir . '/webmozart/assert/src'),
);
"#,
    );

    let mappings = parse_vendor_autoload_psr4(ws.root(), "vendor");
    assert_eq!(mappings.len(), 2);

    assert_eq!(mappings[0].prefix, "voku\\");
    assert_eq!(
        mappings[0].base_path,
        "vendor/voku/portable-ascii/src/voku/"
    );

    assert_eq!(mappings[1].prefix, "Webmozart\\Assert\\");
    assert_eq!(mappings[1].base_path, "vendor/webmozart/assert/src/");
}

#[test]
fn test_vendor_autoload_multiple_paths_per_prefix() {
    let ws = TestWorkspace::new(r#"{"name": "test/project"}"#);

    ws.create_php_file(
        "vendor/composer/autoload_psr4.php",
        r#"<?php

$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'phpDocumentor\\Reflection\\' => array($vendorDir . '/phpdocumentor/reflection-docblock/src', $vendorDir . '/phpdocumentor/type-resolver/src', $vendorDir . '/phpdocumentor/reflection-common/src'),
);
"#,
    );

    let mappings = parse_vendor_autoload_psr4(ws.root(), "vendor");
    assert_eq!(mappings.len(), 3);

    assert!(
        mappings
            .iter()
            .all(|m| m.prefix == "phpDocumentor\\Reflection\\")
    );
    assert_eq!(
        mappings[0].base_path,
        "vendor/phpdocumentor/reflection-docblock/src/"
    );
    assert_eq!(
        mappings[1].base_path,
        "vendor/phpdocumentor/type-resolver/src/"
    );
    assert_eq!(
        mappings[2].base_path,
        "vendor/phpdocumentor/reflection-common/src/"
    );
}

#[test]
fn test_vendor_autoload_basedir_entries() {
    let ws = TestWorkspace::new(r#"{"name": "test/project"}"#);

    ws.create_php_file(
        "vendor/composer/autoload_psr4.php",
        r#"<?php

$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'App\\' => array($baseDir . '/src'),
    'App\\Tests\\' => array($baseDir . '/tests'),
);
"#,
    );

    let mappings = parse_vendor_autoload_psr4(ws.root(), "vendor");
    assert_eq!(mappings.len(), 2);

    assert_eq!(mappings[0].prefix, "App\\");
    assert_eq!(mappings[0].base_path, "src/");

    assert_eq!(mappings[1].prefix, "App\\Tests\\");
    assert_eq!(mappings[1].base_path, "tests/");
}

#[test]
fn test_vendor_autoload_missing_file_returns_empty() {
    let ws = TestWorkspace::new(r#"{"name": "test/project"}"#);
    // No vendor directory at all — should not panic
    let mappings = parse_vendor_autoload_psr4(ws.root(), "vendor");
    assert!(mappings.is_empty());
}

#[test]
fn test_vendor_autoload_custom_vendor_dir() {
    let ws = TestWorkspace::new(
        r#"{
            "config": {
                "vendor-dir": "php-packages"
            }
        }"#,
    );

    ws.create_php_file(
        "php-packages/composer/autoload_psr4.php",
        r#"<?php

$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'Monolog\\' => array($vendorDir . '/monolog/monolog/src/Monolog'),
);
"#,
    );

    // parse_composer_json should pick up the custom vendor-dir and find the vendor autoload
    let mappings = parse_composer_json(ws.root());
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].prefix, "Monolog\\");
    assert_eq!(
        mappings[0].base_path,
        "php-packages/monolog/monolog/src/Monolog/"
    );
}

#[test]
fn test_vendor_autoload_integrated_with_composer_json() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
    );

    ws.create_php_file(
        "vendor/composer/autoload_psr4.php",
        r#"<?php

$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'App\\' => array($baseDir . '/src'),
    'Monolog\\' => array($vendorDir . '/monolog/monolog/src/Monolog'),
);
"#,
    );

    let mappings = parse_composer_json(ws.root());

    // Should have App\ from composer.json, plus App\ and Monolog\ from vendor autoload
    assert!(mappings.len() >= 2);

    // Monolog should be present (from vendor autoload)
    let monolog = mappings.iter().find(|m| m.prefix == "Monolog\\");
    assert!(monolog.is_some());
    assert_eq!(
        monolog.unwrap().base_path,
        "vendor/monolog/monolog/src/Monolog/"
    );

    // App\ from composer.json should be present
    let app_entries: Vec<_> = mappings.iter().filter(|m| m.prefix == "App\\").collect();
    assert!(app_entries.len() >= 1);
    assert!(app_entries.iter().any(|m| m.base_path == "src/"));
}

#[test]
fn test_vendor_autoload_resolve_vendor_class() {
    let ws = TestWorkspace::new(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
    );

    ws.create_php_file(
        "vendor/composer/autoload_psr4.php",
        r#"<?php

$vendorDir = dirname(__DIR__);
$baseDir = dirname($vendorDir);

return array(
    'Monolog\\' => array($vendorDir . '/monolog/monolog/src/Monolog'),
);
"#,
    );

    // Create the actual vendor PHP file so resolve_class_path can find it
    ws.create_php_file(
        "vendor/monolog/monolog/src/Monolog/Logger.php",
        "<?php\nnamespace Monolog;\nclass Logger {}\n",
    );

    let mappings = parse_composer_json(ws.root());
    let result = resolve_class_path(&mappings, ws.root(), "Monolog\\Logger");

    assert!(result.is_some());
    let path = result.unwrap();
    assert!(path.ends_with("vendor/monolog/monolog/src/Monolog/Logger.php"));
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
