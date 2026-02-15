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
/// Run `composer update jetbrains/phpstorm-stubs` in the project root
/// (with `stubs/` as the vendor directory) and rebuild.  The `build.rs`
/// script re-reads the map file and re-embeds everything automatically.

use std::collections::HashMap;

// Pull in the generated static arrays.
include!(concat!(env!("OUT_DIR"), "/stub_map_generated.rs"));

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