/// Embedded PHP stub support (powered by JetBrains phpstorm-stubs).
///
/// This module provides access to PHP standard library stubs (interfaces,
/// classes, and functions) that are embedded directly into the binary at
/// compile time.  The stubs come from the
/// [phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) package.
///
/// ## How it works
///
/// 1. A `build.rs` script parses `PhpStormStubsMap.php` (the index file
///    shipped with phpstorm-stubs) and generates `stub_map_generated.rs`
///    containing:
///    - `STUB_FILES`: an array of every PHP stub file, embedded via
///      `include_str!`.
///    - `STUB_CLASS_MAP`: a `(class_name, file_index)` array mapping
///      class/interface/trait names to indices into `STUB_FILES`.
///    - `STUB_FUNCTION_MAP`: the same for standalone functions.
///
/// 2. At `Backend` construction time, [`build_stub_class_index`] and
///    [`build_stub_function_index`] convert the static arrays into
///    `HashMap`s for O(1) lookup.
///
/// 3. `find_or_load_class` (in `util.rs`) consults the class index as a
///    final fallback (Phase 3) after the `ast_map` and PSR-4 resolution.
///    The stub PHP source is parsed lazily on first access and cached in
///    the `ast_map` under a `phpantom-stub://` URI so subsequent lookups
///    are free.
///
/// ## Updating stubs
///
/// Delete the `stubs/` directory and rebuild. The `build.rs` script will
/// automatically fetch the latest release from GitHub, re-read the map
/// file and re-embed everything.
use std::collections::HashMap;

// Pull in the generated static arrays.
include!(concat!(env!("OUT_DIR"), "/stub_map_generated.rs"));

/// The phpstorm-stubs version that was embedded at build time.
///
/// Set by `build.rs` via `cargo:rustc-env`.  Contains the GitHub release
/// tag (e.g. `"v2025.3"`), `"unknown"` when stubs were present but the
/// version file was missing, or `"none"` when stubs could not be fetched.
pub const STUBS_VERSION: &str = env!("PHPANTOM_STUBS_VERSION");

/// Build a lookup table mapping class/interface/trait short names to their
/// embedded PHP source code.
///
/// Called once during `Backend` construction.  The returned map is stored
/// on the backend and consulted by `find_or_load_class` as a final
/// fallback after the `ast_map` and PSR-4 resolution.
pub fn build_stub_class_index() -> HashMap<&'static str, &'static str> {
    STUB_CLASS_MAP
        .iter()
        .map(|&(name, idx)| (name, STUB_FILES[idx]))
        .collect()
}

/// Build a lookup table mapping function names to their embedded PHP
/// source code.
///
/// This covers both unqualified names (e.g. `"array_map"`) and
/// namespace-qualified names (e.g. `"Brotli\\compress"`).
///
/// Called once during `Backend` construction.  The returned map can be
/// consulted when resolving standalone function calls to provide return
/// type information from stubs.
pub fn build_stub_function_index() -> HashMap<&'static str, &'static str> {
    STUB_FUNCTION_MAP
        .iter()
        .map(|&(name, idx)| (name, STUB_FILES[idx]))
        .collect()
}

/// Quick byte-level check whether a stub function has been `@removed`
/// at or before the given PHP version.
///
/// This scans the raw PHP source for the function's docblock without a
/// full AST parse, so it is cheap enough to call during completion
/// filtering.  Only the docblock immediately preceding
/// `function <short_name>` is examined.
///
/// Returns `true` when the function's docblock contains `@removed X.Y`
/// and `php_version >= X.Y`.
pub fn is_stub_function_removed(
    source: &str,
    func_name: &str,
    php_version: crate::types::PhpVersion,
) -> bool {
    // Fast path: if the entire file doesn't contain `@removed`, no
    // function in it can be version-gated.  This skips ~92% of stub
    // files without any allocations or string searches.
    if !source.contains("@removed") {
        return false;
    }

    // Use the short (unqualified) name for the search pattern.
    let short = func_name.rsplit('\\').next().unwrap_or(func_name);

    let needle = format!("function {short}(");
    let Some(func_pos) = source.find(&needle).or_else(|| {
        // Some stubs have a space or newline before `(`.
        let needle2 = format!("function {short} ");
        source.find(&needle2)
    }) else {
        return false;
    };

    is_preceding_docblock_removed(source, func_pos, php_version)
}

/// Quick byte-level check whether a stub class/interface/trait has been
/// `@removed` at or before the given PHP version.
///
/// Same approach as [`is_stub_function_removed`] but searches for
/// `class <name>`, `interface <name>`, or `trait <name>`.
pub fn is_stub_class_removed(
    source: &str,
    class_name: &str,
    php_version: crate::types::PhpVersion,
) -> bool {
    // Fast path: skip files that don't mention `@removed` at all.
    if !source.contains("@removed") {
        return false;
    }

    // Use the short (unqualified) name for the search pattern.
    let short = class_name.rsplit('\\').next().unwrap_or(class_name);

    // Try `class Name`, `interface Name`, `trait Name`.
    let candidates = [
        format!("class {short}"),
        format!("interface {short}"),
        format!("trait {short}"),
    ];

    let decl_pos = candidates
        .iter()
        .filter_map(|needle| {
            source.find(needle.as_str()).and_then(|pos| {
                // Verify the character after the name is a boundary
                // (space, newline, `{`, or end-of-string) to avoid
                // matching `class FooBar` when looking for `class Foo`.
                let after = pos + needle.len();
                if after >= source.len() {
                    return Some(pos);
                }
                let ch = source.as_bytes()[after];
                if ch == b' ' || ch == b'\n' || ch == b'\r' || ch == b'{' || ch == b'\t' {
                    Some(pos)
                } else {
                    None
                }
            })
        })
        .next();

    let Some(pos) = decl_pos else {
        return false;
    };

    is_preceding_docblock_removed(source, pos, php_version)
}

/// Shared helper: check if the docblock immediately preceding the
/// declaration at `decl_pos` contains `@removed X.Y` where
/// `php_version >= X.Y`.
fn is_preceding_docblock_removed(
    source: &str,
    decl_pos: usize,
    php_version: crate::types::PhpVersion,
) -> bool {
    let before = &source[..decl_pos];
    let Some(doc_end) = before.rfind("*/") else {
        return false;
    };

    // Make sure there is no intervening declaration between the
    // docblock end and our target — otherwise the docblock belongs
    // to a different element.
    let between = &source[doc_end + 2..decl_pos];
    if between.contains("function ") || between.contains("class ") || between.contains("interface ")
    {
        return false;
    }

    let Some(doc_start) = source[..doc_end].rfind("/**") else {
        return false;
    };

    let docblock = &source[doc_start..doc_end + 2];

    // Simple line-by-line scan for `@removed X.Y` instead of a full
    // docblock parse.  This is called for every stub entry during
    // `set_php_version`, so avoiding the mago-docblock parser here
    // saves significant startup time.
    for line in docblock.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        let rest = if let Some(r) = trimmed.strip_prefix("@removed") {
            r
        } else {
            continue;
        };
        // The version string follows the tag, separated by whitespace.
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }
        if let Some(ver) = crate::types::PhpVersion::from_composer_constraint(rest)
            && php_version >= ver
        {
            return true;
        }
    }

    false
}

/// Build a lookup table mapping constant names to their embedded PHP
/// source code.
///
/// This covers both unqualified names (e.g. `"PHP_EOL"`) and
/// namespace-qualified names (e.g. `"CURL\\CURLOPT_URL"`).
///
/// Called once during `Backend` construction.  The returned map can be
/// consulted when resolving standalone constant references to provide
/// type and value information from stubs.
pub fn build_stub_constant_index() -> HashMap<&'static str, &'static str> {
    STUB_CONSTANT_MAP
        .iter()
        .map(|&(name, idx)| (name, STUB_FILES[idx]))
        .collect()
}
