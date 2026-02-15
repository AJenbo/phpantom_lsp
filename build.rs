//! Build script for PHPantomLSP.
//!
//! Parses `stubs/jetbrains/phpstorm-stubs/PhpStormStubsMap.php` and generates
//! a Rust source file (`stub_map_generated.rs`) that:
//!
//!   1. Embeds every referenced PHP stub file via `include_str!`.
//!   2. Provides static arrays mapping class names and function names to
//!      indices into the embedded file array.
//!
//! The generated file is consumed by `src/stubs.rs` at compile time.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;

/// Relative path from the crate root to the stubs map file.
const MAP_FILE: &str = "stubs/jetbrains/phpstorm-stubs/PhpStormStubsMap.php";

/// Relative path from the crate root to the stubs directory (the base for
/// relative paths found in the map file).
const STUBS_DIR: &str = "stubs/jetbrains/phpstorm-stubs";

fn main() {
    // Tell Cargo to re-run this script when dependencies change.
    // We watch composer.lock (rather than PhpStormStubsMap.php directly)
    // because that's the file that changes when `composer update` pulls
    // a new version of phpstorm-stubs — more natural for PHP developers.
    println!("cargo:rerun-if-changed=composer.lock");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let map_path = Path::new(&manifest_dir).join(MAP_FILE);

    let map_content = match fs::read_to_string(&map_path) {
        Ok(c) => c,
        Err(e) => {
            // If stubs aren't installed yet, generate an empty map so the
            // build still succeeds (just without built-in stubs).
            eprintln!(
                "cargo:warning=Could not read PhpStormStubsMap.php ({}); generating empty stub index",
                e
            );
            write_empty_generated_file();
            return;
        }
    };

    // ── Parse the three sections ────────────────────────────────────────

    let class_map = parse_section(&map_content, "CLASSES");
    let function_map = parse_section(&map_content, "FUNCTIONS");
    let constant_map = parse_section(&map_content, "CONSTANTS");

    // ── Collect unique file paths ───────────────────────────────────────

    let mut unique_files = BTreeSet::new();
    for path in class_map.values() {
        unique_files.insert(path.as_str());
    }
    for path in function_map.values() {
        unique_files.insert(path.as_str());
    }
    for path in constant_map.values() {
        unique_files.insert(path.as_str());
    }

    // Only keep files that actually exist on disk.
    let stubs_base = Path::new(&manifest_dir).join(STUBS_DIR);
    let existing_files: Vec<&str> = unique_files
        .iter()
        .copied()
        .filter(|rel| stubs_base.join(rel).is_file())
        .collect();

    // Build a path → index mapping.
    let file_index: BTreeMap<&str, usize> = existing_files
        .iter()
        .enumerate()
        .map(|(i, &p)| (p, i))
        .collect();

    // ── Generate Rust source ────────────────────────────────────────────

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("stub_map_generated.rs");

    let mut out = String::with_capacity(512 * 1024);

    // 1. The embedded file array.
    out.push_str("/// Embedded PHP stub file contents.\n");
    out.push_str("///\n");
    out.push_str("/// Each entry corresponds to one PHP file from phpstorm-stubs,\n");
    out.push_str("/// embedded at compile time via `include_str!`.\n");
    out.push_str(&format!(
        "pub(crate) static STUB_FILES: [&str; {}] = [\n",
        existing_files.len()
    ));
    for rel_path in &existing_files {
        // Build the include_str! path relative to the generated file's
        // location ($OUT_DIR).  We use an absolute path rooted at CARGO_MANIFEST_DIR
        // to avoid fragile relative path arithmetic.
        let abs = stubs_base.join(rel_path);
        let abs_str = abs.to_string_lossy().replace('\\', "/");
        out.push_str(&format!("    include_str!(\"{}\")", abs_str));
        out.push_str(",\n");
    }
    out.push_str("];\n\n");

    // 2. Class name → file index mapping.
    let class_entries: Vec<(&str, usize)> = class_map
        .iter()
        .filter_map(|(name, path)| {
            file_index
                .get(path.as_str())
                .map(|&idx| (name.as_str(), idx))
        })
        .collect();

    out.push_str("/// Maps PHP class/interface/trait short names to an index into\n");
    out.push_str("/// [`STUB_FILES`].\n");
    out.push_str(&format!(
        "pub(crate) static STUB_CLASS_MAP: [(&str, usize); {}] = [\n",
        class_entries.len()
    ));
    for (name, idx) in &class_entries {
        out.push_str(&format!("    (\"{}\", {}),\n", escape(name), idx));
    }
    out.push_str("];\n\n");

    // 3. Function name → file index mapping.
    let function_entries: Vec<(&str, usize)> = function_map
        .iter()
        .filter_map(|(name, path)| {
            file_index
                .get(path.as_str())
                .map(|&idx| (name.as_str(), idx))
        })
        .collect();

    out.push_str("/// Maps PHP function names (including namespaced ones) to an index\n");
    out.push_str("/// into [`STUB_FILES`].\n");
    out.push_str(&format!(
        "pub(crate) static STUB_FUNCTION_MAP: [(&str, usize); {}] = [\n",
        function_entries.len()
    ));
    for (name, idx) in &function_entries {
        out.push_str(&format!("    (\"{}\", {}),\n", escape(name), idx));
    }
    out.push_str("];\n\n");

    // 4. Constant name → file index mapping.
    let constant_entries: Vec<(&str, usize)> = constant_map
        .iter()
        .filter_map(|(name, path)| {
            file_index
                .get(path.as_str())
                .map(|&idx| (name.as_str(), idx))
        })
        .collect();

    out.push_str("/// Maps PHP constant names (including namespaced ones) to an index\n");
    out.push_str("/// into [`STUB_FILES`].\n");
    out.push_str(&format!(
        "pub(crate) static STUB_CONSTANT_MAP: [(&str, usize); {}] = [\n",
        constant_entries.len()
    ));
    for (name, idx) in &constant_entries {
        out.push_str(&format!("    (\"{}\", {}),\n", escape(name), idx));
    }
    out.push_str("];\n");

    fs::write(&dest_path, &out).expect("Failed to write generated stub map");
}

/// Parse one of the `const CLASSES = array(...)`, `const FUNCTIONS = array(...)`,
/// or `const CONSTANTS = array(...)` sections from the PhpStormStubsMap.php file.
///
/// Returns a `BTreeMap<String, String>` of `symbol_name → relative_file_path`.
fn parse_section(content: &str, section_name: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();

    // Find the start: `const SECTION = array (`
    let marker = format!("const {} = array (", section_name);
    let start = match content.find(&marker) {
        Some(pos) => pos + marker.len(),
        None => return map,
    };

    // Walk line by line until we hit `);`
    for line in content[start..].lines() {
        let trimmed = line.trim();
        if trimmed == ");" {
            break;
        }

        // Lines look like:  'ClassName' => 'relative/path.php',
        if let Some(entry) = parse_map_entry(trimmed) {
            map.insert(entry.0, entry.1);
        }
    }

    map
}

/// Parse a single `'key' => 'value',` line.
fn parse_map_entry(line: &str) -> Option<(String, String)> {
    // Strip leading whitespace and trailing comma.
    let trimmed = line.trim().trim_end_matches(',');

    // Split on ` => `.
    let (lhs, rhs) = trimmed.split_once(" => ")?;

    // Strip surrounding single quotes.
    let key = lhs.trim().strip_prefix('\'')?.strip_suffix('\'')?;
    let value = rhs.trim().strip_prefix('\'')?.strip_suffix('\'')?;

    Some((key.to_string(), value.to_string()))
}

/// Escape a string for embedding in a Rust string literal.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Write an empty generated file when stubs are not available.
fn write_empty_generated_file() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("stub_map_generated.rs");
    let content = concat!(
        "pub(crate) static STUB_FILES: [&str; 0] = [];\n",
        "pub(crate) static STUB_CLASS_MAP: [(&str, usize); 0] = [];\n",
        "pub(crate) static STUB_FUNCTION_MAP: [(&str, usize); 0] = [];\n",
        "pub(crate) static STUB_CONSTANT_MAP: [(&str, usize); 0] = [];\n",
    );
    fs::write(&dest_path, content).expect("Failed to write empty generated stub map");
}
