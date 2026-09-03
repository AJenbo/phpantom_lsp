use super::super::const_eval::ClassContext;
use super::super::provider_resources::extract_provider_resources;
use super::*;

fn folio_mounts(content: &str) -> Vec<FolioMount> {
    extract_provider_resources(
        content,
        Path::new("/ws/app/Providers/AppServiceProvider.php"),
        Path::new("/ws"),
        ClassContext::default(),
        Default::default(),
    )
    .folio_mounts
}

#[test]
fn records_a_bare_path_mount_with_no_modifiers() {
    let content = "<?php\n\
        class AppServiceProvider {\n\
            public function boot(): void {\n\
                Folio::path(resource_path('views/pages'));\n\
            }\n\
        }\n";
    assert_eq!(
        folio_mounts(content),
        vec![FolioMount {
            directory: PathBuf::from("/ws/resources/views/pages"),
            uri_prefix: String::new(),
            name_prefix: String::new(),
        }]
    );
}

#[test]
fn records_uri_and_name_chained_in_either_order() {
    let content = "<?php\n\
        class AppServiceProvider {\n\
            public function boot(): void {\n\
                Folio::path(resource_path('views/pages/admin'))\n\
                    ->middleware(['auth'])\n\
                    ->uri('admin')\n\
                    ->name('admin.');\n\
                Folio::path(resource_path('views/pages/guest'))\n\
                    ->name('guest.')\n\
                    ->uri('guest');\n\
            }\n\
        }\n";
    let mounts = folio_mounts(content);
    assert_eq!(
        mounts,
        vec![
            FolioMount {
                directory: PathBuf::from("/ws/resources/views/pages/admin"),
                uri_prefix: "admin".to_string(),
                name_prefix: "admin.".to_string(),
            },
            FolioMount {
                directory: PathBuf::from("/ws/resources/views/pages/guest"),
                uri_prefix: "guest".to_string(),
                name_prefix: "guest.".to_string(),
            },
        ]
    );
}

#[test]
fn a_single_mount_chain_is_not_recorded_once_per_link() {
    // The whole point of matching at the statement level rather than per
    // expression node: `Folio::path(...)`, `->uri(...)`, and `->name(...)`
    // are three distinct nodes the generic expression walker would visit
    // separately if this used it directly.
    let content = "<?php\n\
        class AppServiceProvider {\n\
            public function boot(): void {\n\
                Folio::path(resource_path('views/pages'))->uri('a')->name('a.');\n\
            }\n\
        }\n";
    assert_eq!(folio_mounts(content).len(), 1);
}

#[test]
fn folio_route_registers_a_mount_with_no_prefixes() {
    let content = "<?php\n\
        class AppServiceProvider {\n\
            public function boot(): void {\n\
                Folio::route(resource_path('views/pages'), middleware: ['web']);\n\
            }\n\
        }\n";
    assert_eq!(
        folio_mounts(content),
        vec![FolioMount {
            directory: PathBuf::from("/ws/resources/views/pages"),
            uri_prefix: String::new(),
            name_prefix: String::new(),
        }]
    );
}

#[test]
fn with_routing_pages_argument_registers_a_mount() {
    let content = "<?php\n\
        return Application::configure(basePath: dirname(__DIR__))\n\
            ->withRouting(\n\
                web: __DIR__.'/../routes/web.php',\n\
                pages: __DIR__.'/../resources/views/pages',\n\
                commands: __DIR__.'/../routes/console.php',\n\
            )->create();\n";
    let mounts = extract_provider_resources(
        content,
        Path::new("/ws/bootstrap/app.php"),
        Path::new("/ws"),
        ClassContext::default(),
        Default::default(),
    )
    .folio_mounts;
    assert_eq!(
        mounts,
        vec![FolioMount {
            directory: Path::new("/ws/bootstrap").join("../resources/views/pages"),
            uri_prefix: String::new(),
            name_prefix: String::new(),
        }]
    );
}

#[test]
fn with_routing_without_a_pages_argument_registers_nothing() {
    let content = "<?php\n\
        return Application::configure(basePath: dirname(__DIR__))\n\
            ->withRouting(web: __DIR__.'/../routes/web.php')\n\
            ->create();\n";
    assert!(folio_mounts(content).is_empty());
}

#[test]
fn derives_a_plain_uri_from_a_page_path() {
    assert_eq!(
        derive_folio_uri(Path::new("products.blade.php")),
        "products"
    );
}

#[test]
fn derives_the_root_uri_from_index() {
    assert_eq!(derive_folio_uri(Path::new("index.blade.php")), "");
}

#[test]
fn drops_only_a_trailing_index_segment() {
    assert_eq!(
        derive_folio_uri(Path::new("users/index.blade.php")),
        "users"
    );
}

#[test]
fn rewrites_a_plain_parameter_segment() {
    assert_eq!(
        derive_folio_uri(Path::new("users/[id].blade.php")),
        "users/{id}"
    );
}

#[test]
fn rewrites_a_catch_all_segment() {
    assert_eq!(
        derive_folio_uri(Path::new("docs/[...slug].blade.php")),
        "docs/{slug}"
    );
}

#[test]
fn rewrites_an_implicit_model_binding_segment() {
    assert_eq!(
        derive_folio_uri(Path::new("users/[User].blade.php")),
        "users/{user}"
    );
}

#[test]
fn rewrites_a_custom_key_model_binding_segment_to_the_model_name() {
    assert_eq!(
        derive_folio_uri(Path::new("posts/[Post:slug].blade.php")),
        "posts/{post}"
    );
}

#[test]
fn composes_the_mount_root_as_a_single_slash() {
    assert_eq!(compose_folio_uri("", ""), "/");
    assert_eq!(compose_folio_uri("admin", ""), "admin");
}

#[test]
fn extracts_a_pages_own_name_call() {
    let content = "<?php\n\
        use function Laravel\\Folio\\name;\n\
        \n\
        name('explore');\n\
        ?>\n\
        <div>Explore</div>\n";
    let (name, _offset) = extract_page_route_name(content).expect("a name was declared");
    assert_eq!(name, "explore");
}

#[test]
fn extracts_a_name_call_from_a_grouped_import() {
    let content = "<?php\n\
        use function Laravel\\Folio\\{name, middleware};\n\
        \n\
        name('products.index');\n\
        middleware(['auth']);\n";
    let (name, _offset) = extract_page_route_name(content).expect("a name was declared");
    assert_eq!(name, "products.index");
}

#[test]
fn extracts_a_fully_qualified_name_call_with_no_import() {
    let content = "<?php\n\
        \\Laravel\\Folio\\name('explore');\n";
    let (name, _offset) = extract_page_route_name(content).expect("a name was declared");
    assert_eq!(name, "explore");
}

#[test]
fn an_unimported_name_call_is_not_mistaken_for_folios() {
    // A page that calls some other `name()` function (and never imports
    // Folio's) must not be read as declaring a route name.
    let content = "<?php\n\
        use function App\\Support\\name;\n\
        \n\
        name('not-a-route');\n";
    assert!(extract_page_route_name(content).is_none());
}

#[test]
fn a_route_group_name_chain_is_not_mistaken_for_folios() {
    let content = "<?php\n\
        use function Laravel\\Folio\\name;\n\
        \n\
        Route::name('admin.')->group(function () {});\n";
    assert!(extract_page_route_name(content).is_none());
}

#[test]
fn a_page_without_a_name_call_declares_nothing() {
    let content = "<?php\n?>\n<div>No name here</div>\n";
    assert!(extract_page_route_name(content).is_none());
}
