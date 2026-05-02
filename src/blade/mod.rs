pub mod directives;
pub mod preprocessor;
pub mod source_map;

use std::path::{Path, PathBuf};

/// Number of lines the Blade preprocessor injects as a prologue
/// (<?php header, $errors declaration, $__env declaration, etc.).
pub const PROLOGUE_LINES: u32 = 5;

/// Check whether a URI refers to a Blade template file.
pub fn is_blade_file(uri: &str) -> bool {
    uri.ends_with(".blade.php")
}

/// Discover Laravel Blade view directories from `config/view.php`.
///
/// Parses the `'paths'` array in the config file to extract directory
/// paths.  Falls back to `resources/views` if the config file is
/// missing or unparseable.  Returns only directories that exist.
pub fn discover_view_paths(workspace_root: &Path) -> Vec<PathBuf> {
    let config_path = workspace_root.join("config/view.php");
    let paths = if config_path.is_file() {
        parse_view_config_paths(&config_path, workspace_root)
    } else {
        Vec::new()
    };

    if paths.is_empty() {
        // Fallback: use the conventional Laravel view directory.
        let default = workspace_root.join("resources/views");
        if default.is_dir() {
            return vec![default];
        }
        return Vec::new();
    }

    paths
}

/// Parse `config/view.php` to extract the `'paths'` array entries.
///
/// Looks for string literals inside `'paths' => [...]` and resolves
/// `base_path('...')` calls relative to the workspace root.
fn parse_view_config_paths(config_path: &Path, workspace_root: &Path) -> Vec<PathBuf> {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Find the 'paths' => [...] section.
    let paths_idx = match content.find("'paths'") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let after = &content[paths_idx..];

    // Find the opening bracket.
    let bracket_start = match after.find('[') {
        Some(i) => i,
        None => return Vec::new(),
    };
    let bracket_end = match after[bracket_start..].find(']') {
        Some(i) => bracket_start + i,
        None => return Vec::new(),
    };
    let array_content = &after[bracket_start + 1..bracket_end];

    let mut result = Vec::new();

    // Match `base_path('...')` or `realpath(base_path('...'))`.
    for segment in array_content.split(',') {
        let trimmed = segment.trim();
        if let Some(path) = extract_base_path_arg(trimmed) {
            let resolved = workspace_root.join(path);
            if resolved.is_dir() {
                result.push(resolved);
            }
        } else if let Some(path) = extract_string_literal(trimmed) {
            // Absolute or relative path literal.
            let resolved = if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                workspace_root.join(path)
            };
            if resolved.is_dir() {
                result.push(resolved);
            }
        }
    }

    result
}

/// Extract the string argument from `base_path('...')` or
/// `realpath(base_path('...'))`.
fn extract_base_path_arg(s: &str) -> Option<&str> {
    // Strip optional `realpath(` wrapper.
    let inner = if let Some(rest) = s.strip_prefix("realpath(") {
        rest.strip_suffix(')')?.trim()
    } else {
        s
    };

    let rest = inner.strip_prefix("base_path(")?.strip_suffix(')')?.trim();
    extract_string_literal(rest)
}

/// Extract content from a single- or double-quoted PHP string literal.
fn extract_string_literal(s: &str) -> Option<&str> {
    let s = s.trim();
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        Some(&s[1..s.len() - 1])
    } else {
        None
    }
}
