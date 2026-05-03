use tower_lsp::lsp_types::{Location, Position, Url};

use crate::Backend;

/// Resolve `view('name')` or `View::make('name')` to the corresponding blade templates.
///
/// Converts dot-notation to a file path under `resources/views/`:
/// `'components.button'` → `resources/views/components/button.blade.php`
pub(crate) fn resolve_view_definitions(backend: &Backend, name: &str) -> Vec<Location> {
    let rel = name.replace('.', "/");
    let target_suffixes = [
        format!("/resources/views/{}.blade.php", rel),
        format!("/resources/views/{}.php", rel),
    ];

    let mut results = Vec::new();
    let snapshot = backend.user_file_symbol_maps();

    for (file_uri, _) in snapshot {
        if target_suffixes.iter().any(|s| file_uri.ends_with(s))
            && let Ok(uri) = Url::parse(&file_uri)
        {
            results.push(crate::definition::point_location(uri, Position::new(0, 0)));
        }
    }
    results
}
