/// Composer autoload support.
///
/// This module handles parsing `composer.json` to extract PSR-4 autoload
/// mappings, and resolving fully-qualified PHP class names to file paths
/// on disk using those mappings.
///
/// # PSR-4 Resolution
///
/// Given a mapping like `"Klarna\\" => "src/Klarna/"`, a class name like
/// `Klarna\Customer` is resolved by:
///   1. Stripping the matching prefix (`Klarna\`) from the class name
///   2. Converting remaining namespace separators to directory separators
///   3. Appending `.php`
///   4. Prepending the mapped base directory
///
/// Result: `<workspace>/src/Klarna/Customer.php`
use std::path::{Path, PathBuf};

/// A single PSR-4 namespace-to-directory mapping.
#[derive(Debug, Clone)]
pub struct Psr4Mapping {
    /// The namespace prefix, always ending with `\` (e.g. `"Klarna\"`).
    pub prefix: String,
    /// The base directory path relative to the workspace root (e.g. `"src/Klarna/"`).
    pub base_path: String,
}

/// Parse a `composer.json` file at the given workspace root and extract all
/// PSR-4 autoload mappings from both `autoload` and `autoload-dev` sections.
///
/// Returns an empty `Vec` if the file doesn't exist, can't be read, or
/// contains no PSR-4 mappings.
pub fn parse_composer_json(workspace_root: &Path) -> Vec<Psr4Mapping> {
    let composer_path = workspace_root.join("composer.json");
    let content = match std::fs::read_to_string(&composer_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut mappings = Vec::new();

    // Extract from both "autoload" and "autoload-dev" sections
    for section_key in &["autoload", "autoload-dev"] {
        if let Some(section) = json.get(section_key)
            && let Some(psr4) = section.get("psr-4")
            && let Some(psr4_obj) = psr4.as_object()
        {
            for (prefix, paths) in psr4_obj {
                extract_psr4_entries(prefix, paths, &mut mappings);
            }
        }
    }

    // Sort by prefix length descending so longest-prefix-first matching works
    mappings.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));

    mappings
}

/// Extract PSR-4 entries from a single prefix → path(s) pair.
///
/// The value can be either a string (`"src/"`) or an array of strings
/// (`["src/", "lib/"]`).
fn extract_psr4_entries(prefix: &str, paths: &serde_json::Value, mappings: &mut Vec<Psr4Mapping>) {
    // Normalise the prefix: ensure it ends with `\`
    let normalised_prefix = if prefix.ends_with('\\') {
        prefix.to_string()
    } else if prefix.is_empty() {
        // Empty prefix means "fallback" / root namespace
        String::new()
    } else {
        format!("{}\\", prefix)
    };

    match paths {
        serde_json::Value::String(path) => {
            mappings.push(Psr4Mapping {
                prefix: normalised_prefix.clone(),
                base_path: normalise_path(path),
            });
        }
        serde_json::Value::Array(arr) => {
            for entry in arr {
                if let Some(path) = entry.as_str() {
                    mappings.push(Psr4Mapping {
                        prefix: normalised_prefix.clone(),
                        base_path: normalise_path(path),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Normalise a directory path: ensure it uses forward slashes and ends with `/`.
fn normalise_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    if p.ends_with('/') || p.is_empty() {
        p
    } else {
        format!("{}/", p)
    }
}

/// Resolve a fully-qualified PHP class name to a file path using PSR-4 mappings.
///
/// The `class_name` should be the namespace-qualified name (e.g.
/// `"Klarna\\Customer"` or `"Klarna\\Rest\\Order"`). A leading `\` is
/// stripped if present (PHP fully-qualified syntax).
///
/// Returns the first path that exists on disk, or `None` if no mapping
/// matches or the resolved file doesn't exist.
pub fn resolve_class_path(
    mappings: &[Psr4Mapping],
    workspace_root: &Path,
    class_name: &str,
) -> Option<PathBuf> {
    // Strip leading `\` (PHP fully-qualified name syntax)
    let name = class_name.strip_prefix('\\').unwrap_or(class_name);

    // Skip built-in type keywords that are never real classes
    if is_builtin_type(name) {
        return None;
    }

    // Try each mapping (already sorted longest-prefix-first)
    for mapping in mappings {
        let relative = if mapping.prefix.is_empty() {
            // Empty prefix matches everything (root namespace fallback)
            Some(name)
        } else {
            name.strip_prefix(&mapping.prefix).map(|rest| rest)
        };

        if let Some(relative_class) = relative {
            // Convert namespace separators to directory separators
            let relative_path = relative_class.replace('\\', "/");
            let file_path = workspace_root
                .join(&mapping.base_path)
                .join(format!("{}.php", relative_path));

            if file_path.is_file() {
                return Some(file_path);
            }
        }
    }

    None
}

/// Check if a name is a PHP built-in type (not a class).
fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "self"
            | "static"
            | "parent"
            | "string"
            | "int"
            | "float"
            | "bool"
            | "array"
            | "object"
            | "mixed"
            | "void"
            | "never"
            | "null"
            | "true"
            | "false"
            | "callable"
            | "iterable"
    )
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
            "self", "static", "parent", "string", "int", "float", "bool", "array", "object",
            "mixed", "void", "never", "null", "true", "false", "callable", "iterable",
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
}
