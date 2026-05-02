pub mod directives;
pub mod preprocessor;
pub mod source_map;

/// Check whether a URI refers to a Blade template file.
pub fn is_blade_file(uri: &str) -> bool {
    uri.ends_with(".blade.php")
}
