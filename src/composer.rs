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
pub fn normalise_path(path: &str) -> String {
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
            name.strip_prefix(&mapping.prefix)
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
