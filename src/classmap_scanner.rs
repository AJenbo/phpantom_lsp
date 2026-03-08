//! Fast byte-level PHP class/interface/trait/enum scanner for classmap
//! generation.
//!
//! This module provides a single-pass state machine that extracts
//! fully-qualified class names (`namespace\ClassName`) from PHP source
//! without a full AST parse.  It is used by the self-generated classmap
//! fallback (Sprint 2) to build a `HashMap<String, PathBuf>` when
//! Composer's `autoload_classmap.php` is missing or incomplete.
//!
//! The implementation is modelled after Composer's `PhpFileParser` /
//! `PhpFileCleaner` pipeline and Libretto's `FastScanner`.  It handles:
//!
//! - `class`, `interface`, `trait`, and `enum` declarations
//! - `namespace` declarations (including braced and semicolon forms)
//! - Single-quoted and double-quoted strings (with escape handling)
//! - Heredoc and nowdoc literals
//! - Line comments (`//`, `#`) and block comments (`/* ... */`)
//! - PHP attributes (`#[...]`) — not confused with `#` comments
//! - Property/nullsafe access like `$node->class` (not treated as a
//!   class declaration)
//! - `SomeClass::class` constant access (not treated as a declaration)
//!
//! # Performance
//!
//! The scanner uses `memchr` for SIMD-accelerated keyword pre-screening.
//! Files that contain none of the keywords `class`, `interface`, `trait`,
//! or `enum` are rejected in a single fast pass without entering the
//! state machine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use memchr::memmem;

// ─── Public API ─────────────────────────────────────────────────────────────

/// Scan a single PHP file and return the fully-qualified class names it
/// defines.
///
/// Returns an empty `Vec` when the file cannot be read, is empty, or
/// contains no class-like declarations.
pub fn scan_file(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read(path) else {
        return Vec::new();
    };
    if content.is_empty() {
        return Vec::new();
    }
    find_classes(&content)
}

/// Build a classmap by scanning all `.php` files under the given
/// directories.
///
/// Each directory is walked recursively.  Hidden directories (starting
/// with `.`) and common non-PHP directories (`node_modules`, `target`)
/// are skipped.  The `vendor_dir_name` directory is also skipped at
/// every level to avoid scanning vendor code when walking user source
/// directories.
///
/// Returns a `HashMap<String, PathBuf>` mapping fully-qualified class
/// names to the absolute file path where they are defined.  When a
/// class name appears in multiple files, the first occurrence wins.
pub fn scan_directories(dirs: &[PathBuf], vendor_dir_name: &str) -> HashMap<String, PathBuf> {
    let mut classmap = HashMap::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        scan_directory_recursive(dir, vendor_dir_name, &mut classmap);
    }
    classmap
}

/// Build a classmap by scanning all `.php` files under the given
/// directories, applying PSR-4 compliance filtering.
///
/// For each `(namespace_prefix, base_path)` pair the scanner walks
/// `base_path` recursively and only includes classes whose FQN matches
/// the PSR-4 mapping: the namespace prefix plus the relative file path
/// must equal the class name.
///
/// Entries from `classmap_dirs` are scanned without PSR-4 filtering
/// (equivalent to Composer's `autoload.classmap` entries).
pub fn scan_psr4_directories(
    psr4: &[(String, PathBuf)],
    classmap_dirs: &[PathBuf],
    vendor_dir_name: &str,
) -> HashMap<String, PathBuf> {
    let mut classmap = HashMap::new();

    // PSR-4 directories with namespace filtering
    for (prefix, base_path) in psr4 {
        if !base_path.is_dir() {
            continue;
        }
        scan_psr4_directory_recursive(base_path, base_path, prefix, vendor_dir_name, &mut classmap);
    }

    // Plain classmap directories (no namespace filtering)
    for dir in classmap_dirs {
        if !dir.is_dir() {
            continue;
        }
        scan_directory_recursive(dir, vendor_dir_name, &mut classmap);
    }

    classmap
}

/// Build a classmap from `installed.json` vendor package metadata.
///
/// Reads `<vendor_path>/composer/installed.json` and scans each
/// package's autoload directories.  Supports PSR-4 and classmap
/// entries.
pub fn scan_vendor_packages(workspace_root: &Path, vendor_dir: &str) -> HashMap<String, PathBuf> {
    let vendor_path = workspace_root.join(vendor_dir);
    let installed_path = vendor_path.join("composer").join("installed.json");

    let Ok(content) = std::fs::read_to_string(&installed_path) else {
        return HashMap::new();
    };

    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return HashMap::new();
    };

    // installed.json has two formats:
    //   Composer 1: top-level array of packages
    //   Composer 2: { "packages": [...] }
    let packages = if let Some(arr) = json.as_array() {
        arr.clone()
    } else if let Some(pkgs) = json.get("packages").and_then(|p| p.as_array()) {
        pkgs.clone()
    } else {
        return HashMap::new();
    };

    let mut classmap = HashMap::new();
    let vendor_dir_name = vendor_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("vendor");

    // The directory containing installed.json — install-path values
    // are relative to this directory.
    let composer_dir = vendor_path.join("composer");

    for package in &packages {
        // Locate the package on disk.  Composer 2's installed.json
        // includes an `install-path` field that is relative to the
        // `vendor/composer/` directory.  This is the authoritative
        // location and handles path repositories, custom installers,
        // and any other layout that doesn't follow the default
        // `vendor/<name>/` convention.  Fall back to `vendor/<name>`
        // only when `install-path` is absent (Composer 1 format).
        let pkg_path =
            if let Some(install_path) = package.get("install-path").and_then(|p| p.as_str()) {
                composer_dir.join(install_path)
            } else if let Some(pkg_name) = package.get("name").and_then(|n| n.as_str()) {
                vendor_path.join(pkg_name)
            } else {
                continue;
            };

        let pkg_path = match pkg_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // Directory doesn't exist (package not installed yet).
                if !pkg_path.is_dir() {
                    continue;
                }
                pkg_path
            }
        };

        if !pkg_path.is_dir() {
            continue;
        }

        // Extract autoload section
        let Some(autoload) = package.get("autoload") else {
            continue;
        };

        // PSR-4 entries
        if let Some(psr4) = autoload.get("psr-4").and_then(|p| p.as_object()) {
            for (prefix, paths) in psr4 {
                let prefix = normalise_prefix(prefix);
                for dir_str in value_to_strings(paths) {
                    let dir = pkg_path.join(&dir_str);
                    if dir.is_dir() {
                        scan_psr4_directory_recursive(
                            &dir,
                            &dir,
                            &prefix,
                            vendor_dir_name,
                            &mut classmap,
                        );
                    }
                }
            }
        }

        // Classmap entries
        if let Some(cm) = autoload.get("classmap").and_then(|c| c.as_array()) {
            for entry in cm {
                if let Some(dir_str) = entry.as_str() {
                    let dir = pkg_path.join(dir_str);
                    if dir.is_dir() {
                        scan_directory_recursive(&dir, vendor_dir_name, &mut classmap);
                    } else if dir.is_file() && dir.extension().is_some_and(|ext| ext == "php") {
                        for fqcn in scan_file(&dir) {
                            classmap.entry(fqcn).or_insert_with(|| dir.clone());
                        }
                    }
                }
            }
        }
    }

    classmap
}

/// Scan all `.php` files under the workspace root (excluding hidden
/// directories and the vendor directory).
///
/// This is the non-Composer fallback: when no `composer.json` exists,
/// we walk everything to provide basic cross-file resolution.
pub fn scan_workspace_fallback(
    workspace_root: &Path,
    vendor_dir_name: &str,
) -> HashMap<String, PathBuf> {
    scan_directories(&[workspace_root.to_path_buf()], vendor_dir_name)
}

// ─── Core scanner ───────────────────────────────────────────────────────────

/// Single-pass byte-level scanner that extracts fully-qualified class
/// names from PHP source bytes.
///
/// Skips comments, strings, heredocs, and nowdocs inline without
/// allocating a separate "cleaned" buffer.
pub fn find_classes(content: &[u8]) -> Vec<String> {
    // Quick rejection — use SIMD to check if any class-like keywords exist
    if !has_class_keyword(content) {
        return Vec::new();
    }

    let mut classes = Vec::with_capacity(4);
    let mut namespace = String::new();
    let len = content.len();
    let mut i = 0;

    // State flags
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut in_single_string = false;
    let mut in_double_string = false;
    let mut in_heredoc = false;
    let mut heredoc_id: &[u8] = &[];

    while i < len {
        // ── Skip: line comment ──────────────────────────────────────
        if in_line_comment {
            if content[i] == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        // ── Skip: block comment ─────────────────────────────────────
        if in_block_comment {
            if content[i] == b'*' && i + 1 < len && content[i + 1] == b'/' {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        // ── Skip: single-quoted string ──────────────────────────────
        if in_single_string {
            if content[i] == b'\\' && i + 1 < len {
                i += 2;
            } else if content[i] == b'\'' {
                in_single_string = false;
                i += 1;
            } else {
                i += 1;
            }
            continue;
        }

        // ── Skip: double-quoted string ──────────────────────────────
        if in_double_string {
            if content[i] == b'\\' && i + 1 < len {
                i += 2;
            } else if content[i] == b'"' {
                in_double_string = false;
                i += 1;
            } else {
                i += 1;
            }
            continue;
        }

        // ── Skip: heredoc / nowdoc ──────────────────────────────────
        if in_heredoc {
            let line_start = i;
            // Skip leading whitespace (PHP 7.3+ flexible heredoc)
            while i < len && (content[i] == b' ' || content[i] == b'\t') {
                i += 1;
            }
            if i + heredoc_id.len() <= len && &content[i..i + heredoc_id.len()] == heredoc_id {
                let after = i + heredoc_id.len();
                if after >= len
                    || content[after] == b';'
                    || content[after] == b'\n'
                    || content[after] == b'\r'
                    || content[after] == b','
                    || content[after] == b')'
                {
                    in_heredoc = false;
                    i = after;
                    continue;
                }
            }
            // Skip to next line
            i = line_start;
            while i < len && content[i] != b'\n' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            continue;
        }

        // ── Main code parsing ───────────────────────────────────────
        let b = content[i];

        // Comments: // and /* */
        if b == b'/' && i + 1 < len {
            if content[i + 1] == b'/' {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if content[i + 1] == b'*' {
                in_block_comment = true;
                i += 2;
                continue;
            }
        }

        // Hash comments (but not PHP attributes #[...])
        if b == b'#' {
            if i + 1 < len && content[i + 1] == b'[' {
                // PHP attribute — skip past it (it's not a comment)
                i += 1;
                continue;
            }
            in_line_comment = true;
            i += 1;
            continue;
        }

        // Strings
        if b == b'\'' {
            in_single_string = true;
            i += 1;
            continue;
        }
        if b == b'"' {
            in_double_string = true;
            i += 1;
            continue;
        }

        // Heredoc / nowdoc: <<<
        if b == b'<' && i + 2 < len && content[i + 1] == b'<' && content[i + 2] == b'<' {
            i += 3;
            // Skip whitespace
            while i < len && content[i] == b' ' {
                i += 1;
            }
            // Skip optional quote (nowdoc uses single quotes)
            if i < len && (content[i] == b'\'' || content[i] == b'"') {
                i += 1;
            }
            let id_start = i;
            while i < len && (content[i].is_ascii_alphanumeric() || content[i] == b'_') {
                i += 1;
            }
            if i > id_start {
                heredoc_id = &content[id_start..i];
                in_heredoc = true;
                // Skip closing quote
                if i < len && (content[i] == b'\'' || content[i] == b'"') {
                    i += 1;
                }
                // Skip to newline
                while i < len && content[i] != b'\n' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            continue;
        }

        // ── Keyword detection ───────────────────────────────────────
        // Only match at valid keyword boundaries to avoid matching
        // property accesses like `$node->class`.
        if is_keyword_boundary(content, i) {
            // namespace
            if b == b'n'
                && i + 9 <= len
                && &content[i..i + 9] == b"namespace"
                && (i + 9 >= len
                    || content[i + 9].is_ascii_whitespace()
                    || content[i + 9] == b';'
                    || content[i + 9] == b'{')
            {
                i += 9;
                while i < len && content[i].is_ascii_whitespace() {
                    i += 1;
                }

                // Check for braced namespace (e.g. `namespace Foo { ... }`)
                // vs. semicolon form. Either way, read the name.
                let ns_start = i;
                while i < len {
                    let c = content[i];
                    if c.is_ascii_alphanumeric()
                        || c == b'_'
                        || c == b'\\'
                        || c.is_ascii_whitespace()
                    {
                        i += 1;
                    } else {
                        break;
                    }
                }
                namespace = content[ns_start..i]
                    .iter()
                    .filter(|&&c| !c.is_ascii_whitespace())
                    .map(|&c| c as char)
                    .collect();
                if !namespace.is_empty() && !namespace.ends_with('\\') {
                    namespace.push('\\');
                }
                continue;
            }

            // class
            if b == b'c'
                && i + 5 <= len
                && &content[i..i + 5] == b"class"
                && (i + 5 >= len || content[i + 5].is_ascii_whitespace())
            {
                i += 5;
                if let Some(name) = read_name(content, &mut i) {
                    classes.push(format!("{namespace}{name}"));
                }
                continue;
            }

            // interface
            if b == b'i'
                && i + 9 <= len
                && &content[i..i + 9] == b"interface"
                && (i + 9 >= len || content[i + 9].is_ascii_whitespace())
            {
                i += 9;
                if let Some(name) = read_name(content, &mut i) {
                    classes.push(format!("{namespace}{name}"));
                }
                continue;
            }

            // trait
            if b == b't'
                && i + 5 <= len
                && &content[i..i + 5] == b"trait"
                && (i + 5 >= len || content[i + 5].is_ascii_whitespace())
            {
                i += 5;
                if let Some(name) = read_name(content, &mut i) {
                    classes.push(format!("{namespace}{name}"));
                }
                continue;
            }

            // enum
            if b == b'e'
                && i + 4 <= len
                && &content[i..i + 4] == b"enum"
                && (i + 4 >= len || content[i + 4].is_ascii_whitespace())
            {
                i += 4;
                if let Some(name) = read_name(content, &mut i) {
                    classes.push(format!("{namespace}{name}"));
                }
                continue;
            }
        }

        i += 1;
    }

    classes
}

// ─── Internal helpers ───────────────────────────────────────────────────────

/// SIMD-accelerated pre-screening: check whether the content contains
/// any of the class-like keywords.
#[inline]
fn has_class_keyword(content: &[u8]) -> bool {
    memmem::find(content, b"class").is_some()
        || memmem::find(content, b"interface").is_some()
        || memmem::find(content, b"trait").is_some()
        || memmem::find(content, b"enum").is_some()
}

/// Check if a character is a valid boundary (not part of an identifier).
#[inline]
fn is_boundary_char(c: u8) -> bool {
    !c.is_ascii_alphanumeric() && c != b'_' && c != b':' && c != b'$'
}

/// Check whether a keyword can start at this offset.
///
/// Rejects property accesses like `$node->class` and
/// `$node?->class` to avoid false positives.
#[inline]
fn is_keyword_boundary(content: &[u8], i: usize) -> bool {
    if i == 0 {
        return true;
    }

    let prev = content[i - 1];
    if !is_boundary_char(prev) {
        return false;
    }

    // Reject object/nullsafe property access: ->class, ?->class
    if prev == b'>' && i >= 2 {
        let prev2 = content[i - 2];
        if prev2 == b'-' || prev2 == b'?' {
            return false;
        }
    }

    true
}

/// Read a class/interface/trait/enum name after the keyword.
///
/// Skips whitespace, then reads an identifier.  Returns `None` for
/// keywords like `extends`/`implements` that can follow `class` in
/// anonymous class expressions (`new class extends Foo {}`).
#[inline]
fn read_name<'a>(content: &'a [u8], i: &mut usize) -> Option<&'a str> {
    let len = content.len();

    // Skip whitespace
    while *i < len && content[*i].is_ascii_whitespace() {
        *i += 1;
    }

    let start = *i;

    // Read identifier characters
    while *i < len {
        let c = content[*i];
        if c.is_ascii_alphanumeric() || c == b'_' {
            *i += 1;
        } else {
            break;
        }
    }

    if *i == start {
        return None;
    }

    let name = &content[start..*i];

    // Skip keywords that appear in anonymous class expressions
    if name == b"extends" || name == b"implements" {
        return None;
    }

    std::str::from_utf8(name).ok()
}

/// Normalise a PSR-4 prefix: ensure it ends with `\`.
fn normalise_prefix(prefix: &str) -> String {
    if prefix.is_empty() {
        String::new()
    } else if prefix.ends_with('\\') {
        prefix.to_string()
    } else {
        format!("{prefix}\\")
    }
}

/// Extract string values from a JSON value that is either a single
/// string or an array of strings.
fn value_to_strings(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether a directory name should be skipped during recursive walks.
fn should_skip_dir(name: &str, vendor_dir_name: &str) -> bool {
    name.starts_with('.') || name == vendor_dir_name || name == "node_modules" || name == "target"
}

/// Recursively scan a directory for `.php` files and add discovered
/// class names to the classmap.
fn scan_directory_recursive(
    dir: &Path,
    vendor_dir_name: &str,
    classmap: &mut HashMap<String, PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && should_skip_dir(name, vendor_dir_name)
            {
                continue;
            }
            scan_directory_recursive(&path, vendor_dir_name, classmap);
        } else if path.extension().is_some_and(|ext| ext == "php") {
            for fqcn in scan_file(&path) {
                classmap.entry(fqcn).or_insert_with(|| path.clone());
            }
        }
    }
}

/// Recursively scan a PSR-4 directory, only including classes whose
/// FQN matches the PSR-4 mapping.
fn scan_psr4_directory_recursive(
    dir: &Path,
    base_path: &Path,
    namespace_prefix: &str,
    vendor_dir_name: &str,
    classmap: &mut HashMap<String, PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && should_skip_dir(name, vendor_dir_name)
            {
                continue;
            }
            scan_psr4_directory_recursive(
                &path,
                base_path,
                namespace_prefix,
                vendor_dir_name,
                classmap,
            );
        } else if path.extension().is_some_and(|ext| ext == "php") {
            // Compute expected FQN from the file path relative to the
            // PSR-4 base directory.
            let relative = match path.strip_prefix(base_path) {
                Ok(rel) => rel,
                Err(_) => continue,
            };
            let relative_str = relative.to_string_lossy();
            // Strip the `.php` extension
            let stem = match relative_str.strip_suffix(".php") {
                Some(s) => s,
                None => continue,
            };
            // Convert path separators to namespace separators
            let expected_fqn = format!("{}{}", namespace_prefix, stem.replace('/', "\\"));

            let classes = scan_file(&path);
            for fqcn in classes {
                if fqcn == expected_fqn {
                    classmap.entry(fqcn).or_insert_with(|| path.clone());
                }
            }
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── find_classes unit tests ──────────────────────────────────────

    #[test]
    fn simple_class() {
        let content = b"<?php\nclass Foo {}";
        assert_eq!(find_classes(content), vec!["Foo"]);
    }

    #[test]
    fn namespaced_class() {
        let content = b"<?php\nnamespace App\\Models;\nclass User {}";
        assert_eq!(find_classes(content), vec!["App\\Models\\User"]);
    }

    #[test]
    fn multiple_declarations() {
        let content = br"<?php
namespace App;

class Foo {}
interface Bar {}
trait Baz {}
enum Status {}
";
        assert_eq!(
            find_classes(content),
            vec!["App\\Foo", "App\\Bar", "App\\Baz", "App\\Status"]
        );
    }

    #[test]
    fn class_in_comment_ignored() {
        let content = br"<?php
// class Fake {}
/* class AlsoFake {} */
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn class_in_string_ignored() {
        let content = br#"<?php
$x = "class Fake {}";
$y = 'class AlsoFake {}';
class Real {}
"#;
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn no_classes() {
        let content = b"<?php\necho 'hello';";
        assert!(find_classes(content).is_empty());
    }

    #[test]
    fn enum_with_type() {
        let content = b"<?php\nenum Status: int { case Active = 1; }";
        assert_eq!(find_classes(content), vec!["Status"]);
    }

    #[test]
    fn class_constant_not_treated_as_declaration() {
        let content = b"<?php\n$x = SomeClass::class;\nclass Real {}";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn php_attribute() {
        let content = br"<?php
#[Attribute]
class MyAttribute {}
";
        assert_eq!(find_classes(content), vec!["MyAttribute"]);
    }

    #[test]
    fn heredoc() {
        let content = br"<?php
$x = <<<EOT
class Fake {}
EOT;
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn nowdoc() {
        let content = br"<?php
$x = <<<'EOT'
class Fake {}
EOT;
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn property_access_class_ignored() {
        let content = br"<?php
namespace Foo;
if ($node->class instanceof Name) {
}
";
        assert!(find_classes(content).is_empty());
    }

    #[test]
    fn nullsafe_property_access_class_ignored() {
        let content = br"<?php
namespace Foo;
if ($node?->class instanceof Name) {
}
";
        assert!(find_classes(content).is_empty());
    }

    #[test]
    fn real_class_not_affected_by_property_access() {
        let content = br"<?php
namespace Foo;
class Real {}
if ($node->class instanceof Name) {
}
";
        assert_eq!(find_classes(content), vec!["Foo\\Real"]);
    }

    #[test]
    fn anonymous_class_ignored() {
        let content = br"<?php
$x = new class extends Foo {};
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn anonymous_class_implements_ignored() {
        let content = br"<?php
$x = new class implements Bar {};
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn hash_comment_not_confused_with_attribute() {
        let content = br"<?php
# This is a comment with class keyword
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn multiple_namespaces() {
        let content = br"<?php
namespace First;
class A {}
namespace Second;
class B {}
";
        assert_eq!(find_classes(content), vec!["First\\A", "Second\\B"]);
    }

    #[test]
    fn global_namespace_after_named() {
        // namespace; with no name resets to global
        let content = br"<?php
namespace Foo;
class A {}
namespace;
class B {}
";
        // When `namespace;` is encountered with no name, the namespace
        // becomes empty (global).
        assert_eq!(find_classes(content), vec!["Foo\\A", "B"]);
    }

    #[test]
    fn escaped_string_does_not_leak() {
        let content = br#"<?php
$x = "escaped \" class Fake {}";
class Real {}
"#;
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn escaped_single_quote_string_does_not_leak() {
        let content = br"<?php
$x = 'escaped \' class Fake {}';
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn block_comment_with_star() {
        let content = br"<?php
/**
 * class Fake {}
 */
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    #[test]
    fn empty_content() {
        assert!(find_classes(b"").is_empty());
    }

    #[test]
    fn no_keyword_quick_rejection() {
        let content = b"<?php\necho 'hello world';";
        assert!(find_classes(content).is_empty());
    }

    #[test]
    fn flexible_heredoc_php73() {
        // PHP 7.3+ allows the closing identifier to be indented
        let content = br"<?php
$x = <<<EOT
    class Fake {}
    EOT;
class Real {}
";
        assert_eq!(find_classes(content), vec!["Real"]);
    }

    // ── scan_directories integration tests ──────────────────────────

    #[test]
    fn scan_directories_finds_classes() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("User.php"),
            "<?php\nnamespace App\\Models;\nclass User {}",
        )
        .unwrap();
        std::fs::write(
            src.join("Order.php"),
            "<?php\nnamespace App\\Models;\nclass Order {}",
        )
        .unwrap();

        let classmap = scan_directories(&[src], "vendor");
        assert_eq!(classmap.len(), 2);
        assert!(classmap.contains_key("App\\Models\\User"));
        assert!(classmap.contains_key("App\\Models\\Order"));
    }

    #[test]
    fn scan_directories_skips_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".hidden");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("Secret.php"), "<?php\nclass Secret {}").unwrap();

        let classmap = scan_directories(&[dir.path().to_path_buf()], "vendor");
        assert!(!classmap.contains_key("Secret"));
    }

    #[test]
    fn scan_directories_skips_vendor() {
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        std::fs::create_dir_all(&vendor).unwrap();
        std::fs::write(vendor.join("Lib.php"), "<?php\nclass Lib {}").unwrap();

        let classmap = scan_directories(&[dir.path().to_path_buf()], "vendor");
        assert!(!classmap.contains_key("Lib"));
    }

    #[test]
    fn psr4_filtering() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let models = src.join("Models");
        std::fs::create_dir_all(&models).unwrap();

        // Compliant: App\Models\User in src/Models/User.php
        std::fs::write(
            models.join("User.php"),
            "<?php\nnamespace App\\Models;\nclass User {}",
        )
        .unwrap();

        // Non-compliant: class name doesn't match file path
        std::fs::write(
            models.join("Misplaced.php"),
            "<?php\nnamespace App\\Wrong;\nclass Misplaced {}",
        )
        .unwrap();

        let classmap = scan_psr4_directories(&[("App\\".to_string(), src)], &[], "vendor");
        assert!(classmap.contains_key("App\\Models\\User"));
        assert!(!classmap.contains_key("App\\Wrong\\Misplaced"));
    }

    #[test]
    fn scan_vendor_packages_installed_json_v2() {
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        let composer_dir = vendor.join("composer");
        std::fs::create_dir_all(&composer_dir).unwrap();

        // Create a fake package
        let pkg_src = vendor.join("acme").join("logger").join("src");
        std::fs::create_dir_all(&pkg_src).unwrap();
        std::fs::write(
            pkg_src.join("Logger.php"),
            "<?php\nnamespace Acme\\Logger;\nclass Logger {}",
        )
        .unwrap();

        // Composer 2 format installed.json with install-path
        let installed = serde_json::json!({
            "packages": [
                {
                    "name": "acme/logger",
                    "install-path": "../acme/logger",
                    "autoload": {
                        "psr-4": {
                            "Acme\\Logger\\": "src/"
                        }
                    }
                }
            ]
        });
        std::fs::write(
            composer_dir.join("installed.json"),
            serde_json::to_string(&installed).unwrap(),
        )
        .unwrap();

        let classmap = scan_vendor_packages(dir.path(), "vendor");
        assert!(
            classmap.contains_key("Acme\\Logger\\Logger"),
            "classmap keys: {:?}",
            classmap.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn scan_vendor_packages_install_path_non_standard_location() {
        // Packages installed via path repositories or custom installers
        // may not live under vendor/<name>/.  The install-path field
        // (relative to vendor/composer/) is the authoritative location.
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        let composer_dir = vendor.join("composer");
        std::fs::create_dir_all(&composer_dir).unwrap();

        // Package lives in a non-standard location outside the vendor dir
        let custom_location = dir.path().join("packages").join("my-lib").join("src");
        std::fs::create_dir_all(&custom_location).unwrap();
        std::fs::write(
            custom_location.join("Widget.php"),
            "<?php\nnamespace My\\Lib;\nclass Widget {}",
        )
        .unwrap();

        // install-path is relative to vendor/composer/
        let installed = serde_json::json!({
            "packages": [
                {
                    "name": "my/lib",
                    "install-path": "../../packages/my-lib",
                    "autoload": {
                        "psr-4": {
                            "My\\Lib\\": "src/"
                        }
                    }
                }
            ]
        });
        std::fs::write(
            composer_dir.join("installed.json"),
            serde_json::to_string(&installed).unwrap(),
        )
        .unwrap();

        let classmap = scan_vendor_packages(dir.path(), "vendor");
        assert!(
            classmap.contains_key("My\\Lib\\Widget"),
            "install-path should resolve non-standard locations; keys: {:?}",
            classmap.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn scan_vendor_packages_falls_back_to_name_without_install_path() {
        // Composer 1 format: no install-path field, falls back to
        // vendor/<name>/.
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        let composer_dir = vendor.join("composer");
        std::fs::create_dir_all(&composer_dir).unwrap();

        let pkg_src = vendor.join("old").join("pkg").join("src");
        std::fs::create_dir_all(&pkg_src).unwrap();
        std::fs::write(
            pkg_src.join("Legacy.php"),
            "<?php\nnamespace Old\\Pkg;\nclass Legacy {}",
        )
        .unwrap();

        // No install-path — Composer 1 style
        let installed = serde_json::json!([
            {
                "name": "old/pkg",
                "autoload": {
                    "psr-4": {
                        "Old\\Pkg\\": "src/"
                    }
                }
            }
        ]);
        std::fs::write(
            composer_dir.join("installed.json"),
            serde_json::to_string(&installed).unwrap(),
        )
        .unwrap();

        let classmap = scan_vendor_packages(dir.path(), "vendor");
        assert!(
            classmap.contains_key("Old\\Pkg\\Legacy"),
            "should fall back to vendor/<name> when install-path is absent; keys: {:?}",
            classmap.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn scan_vendor_packages_classmap_entry() {
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        let composer_dir = vendor.join("composer");
        std::fs::create_dir_all(&composer_dir).unwrap();

        // Create a fake package with classmap autoloading
        let pkg_lib = vendor.join("acme").join("utils").join("lib");
        std::fs::create_dir_all(&pkg_lib).unwrap();
        std::fs::write(pkg_lib.join("Helper.php"), "<?php\nclass Helper {}").unwrap();

        let installed = serde_json::json!({
            "packages": [
                {
                    "name": "acme/utils",
                    "install-path": "../acme/utils",
                    "autoload": {
                        "classmap": ["lib/"]
                    }
                }
            ]
        });
        std::fs::write(
            composer_dir.join("installed.json"),
            serde_json::to_string(&installed).unwrap(),
        )
        .unwrap();

        let classmap = scan_vendor_packages(dir.path(), "vendor");
        assert!(classmap.contains_key("Helper"));
    }

    #[test]
    fn scan_workspace_fallback_finds_all() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("lib");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Foo.php"), "<?php\nclass Foo {}").unwrap();
        std::fs::write(dir.path().join("Bar.php"), "<?php\nclass Bar {}").unwrap();

        let classmap = scan_workspace_fallback(dir.path(), "vendor");
        assert!(classmap.contains_key("Foo"));
        assert!(classmap.contains_key("Bar"));
    }
}
