//! Build script for PHPantom.
//!
//! Parses `stubs/jetbrains/phpstorm-stubs/PhpStormStubsMap.php` and generates
//! a Rust source file (`stub_map_generated.rs`) that:
//!
//!   1. Embeds every referenced PHP stub file via `include_str!`.
//!   2. Provides static arrays mapping class names and function names to
//!      indices into the embedded file array.
//!
//! The generated file is consumed by `src/stubs.rs` at compile time.
//!
//! ## Automatic stub fetching
//!
//! If the stubs directory doesn't exist, the build script will automatically
//! fetch the latest release of phpstorm-stubs from GitHub. This allows
//! `cargo install` to work without requiring `composer install` first.
//!
//! ## Re-run strategy
//!
//! The `stubs/` directory is gitignored, so Cargo's default "re-run when
//! any package file changes" behaviour does not notice when stubs are
//! downloaded.  Explicit `rerun-if-changed` on paths inside `stubs/` also
//! fails when the directory doesn't exist yet.
//!
//! Instead we watch the project root directory (`.`).  Its mtime changes
//! whenever a direct child like `stubs/` is created or removed.  We also
//! watch `build.rs` and `composer.lock` for targeted rebuilds.
//!
//! To avoid unnecessary recompilation of the main crate we compare the
//! newly generated content against the existing output file and only write
//! when something actually changed.  This way `rustc` sees a stable mtime
//! on `stub_map_generated.rs` and skips recompilation when the stubs
//! haven't changed.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use serde::Deserialize;
use tar::Archive;

const GITHUB_REPO: &str = "JetBrains/phpstorm-stubs";

/// Relative path from the crate root to the stubs map file.
const MAP_FILE: &str = "stubs/jetbrains/phpstorm-stubs/PhpStormStubsMap.php";

/// Relative path from the crate root to the stubs directory (the base for
/// relative paths found in the map file).
const STUBS_DIR: &str = "stubs/jetbrains/phpstorm-stubs";

/// GitHub API response for latest release.
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    tarball_url: String,
}

fn main() {
    // Watch the project root directory so that creating/removing `stubs/`
    // (which is gitignored) is detected via the directory mtime change.
    // Without this, Cargo's default "any package file" check ignores
    // gitignored paths, and explicit watches on non-existent paths don't
    // reliably trigger when they first appear.
    println!("cargo:rerun-if-changed=.");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=composer.lock");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let stubs_path = Path::new(&manifest_dir).join(STUBS_DIR);
    let map_path = Path::new(&manifest_dir).join(MAP_FILE);

    if !map_path.exists() {
        eprintln!("cargo:warning=Stubs not found, fetching from GitHub...");
        if let Err(e) = fetch_stubs(&manifest_dir) {
            eprintln!("cargo:warning=Failed to fetch stubs from GitHub: {}", e);
            eprintln!(
                "cargo:warning=Building without stubs. Run `composer install` for full stub support."
            );
            write_empty_stubs();
            return;
        }
    }

    let map_content = match fs::read_to_string(&map_path) {
        Ok(c) => c,
        Err(e) => {
            // If stubs aren't installed yet, generate an empty map so the
            // build still succeeds (just without built-in stubs).
            eprintln!(
                "cargo:warning=Could not read PhpStormStubsMap.php ({}); generating empty stub index",
                e
            );
            write_empty_stubs();
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
    let existing_files: Vec<&str> = unique_files
        .iter()
        .copied()
        .filter(|rel| stubs_path.join(rel).is_file())
        .collect();

    // Build a path → index mapping.
    let file_index: BTreeMap<&str, usize> = existing_files
        .iter()
        .enumerate()
        .map(|(i, &p)| (p, i))
        .collect();

    // ── Generate Rust source ────────────────────────────────────────────

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
        let abs = stubs_path.join(rel_path);
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

    write_if_changed(&out);
}

fn fetch_stubs(manifest_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let response = ureq::get(&api_url)
        .set("User-Agent", "phpantom-lsp-build")
        .set("Accept", "application/vnd.github.v3+json")
        .call()?;

    let release: GitHubRelease = response.into_json()?;
    eprintln!(
        "cargo:warning=Downloading phpstorm-stubs {}",
        release.tag_name
    );

    let tarball_response = ureq::get(&release.tarball_url)
        .set("User-Agent", "phpantom-lsp-build")
        .call()?;

    let mut tarball_bytes = Vec::new();
    tarball_response
        .into_reader()
        .read_to_end(&mut tarball_bytes)?;

    let decoder = GzDecoder::new(&tarball_bytes[..]);
    let mut archive = Archive::new(decoder);

    let target_dir = Path::new(manifest_dir).join("stubs/jetbrains/phpstorm-stubs");
    fs::create_dir_all(&target_dir)?;

    // GitHub tarballs have a top-level directory like "JetBrains-phpstorm-stubs-abc1234/"
    // We need to strip that prefix when extracting.
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let components: Vec<_> = path.components().collect();
        if components.len() <= 1 {
            continue;
        }

        let relative_path: std::path::PathBuf = components[1..].iter().collect();
        let dest_path = target_dir.join(&relative_path);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if entry.header().entry_type().is_file() {
            let mut file = fs::File::create(&dest_path)?;
            std::io::copy(&mut entry, &mut file)?;
        }
    }

    eprintln!(
        "cargo:warning=Successfully downloaded phpstorm-stubs {}",
        release.tag_name
    );
    Ok(())
}

/// Write an empty stub map when stubs aren't available.
fn write_empty_stubs() {
    let content = concat!(
        "pub(crate) static STUB_FILES: [&str; 0] = [];\n",
        "pub(crate) static STUB_CLASS_MAP: [(&str, usize); 0] = [];\n",
        "pub(crate) static STUB_FUNCTION_MAP: [(&str, usize); 0] = [];\n",
        "pub(crate) static STUB_CONSTANT_MAP: [(&str, usize); 0] = [];\n",
    );
    write_if_changed(content);
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

    // Unescape PHP single-quoted string escapes:
    //   `\\` → `\`   and   `\'` → `'`
    // This is needed because the PhpStormStubsMap.php file uses PHP
    // single-quoted strings where namespace separators are written as
    // `\\` (e.g. `'Couchbase\\GetUserOptions'` → `Couchbase\GetUserOptions`).
    let key = php_unescape_single_quoted(key);
    let value = php_unescape_single_quoted(value);

    Some((key, value))
}

/// Unescape a PHP single-quoted string value.
///
/// PHP single-quoted strings only recognise two escape sequences:
///   - `\\` → `\`
///   - `\'` → `'`
fn php_unescape_single_quoted(s: &str) -> String {
    s.replace("\\\\", "\x00")
        .replace("\\'", "'")
        .replace('\x00', "\\")
}

/// Escape a string for embedding in a Rust string literal.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Write the generated file only if its content has actually changed.
///
/// This avoids bumping the mtime on `stub_map_generated.rs` when nothing
/// changed, which in turn prevents `rustc` from unnecessarily recompiling
/// the main crate.
fn write_if_changed(content: &str) {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("stub_map_generated.rs");

    if let Ok(existing) = fs::read_to_string(&dest_path)
        && existing == content
    {
        return;
    }

    fs::write(&dest_path, content).expect("Failed to write generated stub map");
}
