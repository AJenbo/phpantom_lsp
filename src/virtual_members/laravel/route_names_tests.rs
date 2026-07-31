use super::*;

/// Collect the routes of a single route file, as `enumerate_all_routes` does
/// per file (without the workspace walk).
fn routes_of(content: &str) -> Vec<RouteEntry> {
    let mut out = Vec::new();
    collect_all_names_from_file(content, None, &mut out);
    out
}

fn uri_of(content: &str, name: &str) -> String {
    routes_of(content)
        .into_iter()
        .find(|route| route.name == name)
        .unwrap_or_else(|| panic!("route {name} not collected"))
        .uri
}

#[test]
fn records_uri_of_simple_registration() {
    let content = "<?php\nRoute::get('/users/{user}', 'show')->name('users.show');\n";
    assert_eq!(uri_of(content, "users.show"), "users/{user}");
}

#[test]
fn root_uri_is_reported_as_slash() {
    let content = "<?php\nRoute::get('/', fn() => 'home')->name('home');\n";
    assert_eq!(uri_of(content, "home"), "/");
}

#[test]
fn records_uri_through_intermediate_chain_links() {
    let content = "<?php\nRoute::middleware('auth')->get('/orders/{order}', 'show')\n    ->whereNumber('order')->name('orders.show');\n";
    assert_eq!(uri_of(content, "orders.show"), "orders/{order}");
}

#[test]
fn records_uri_of_match_registration() {
    let content =
        "<?php\nRoute::match(['get', 'post'], '/search/{term?}', 'run')->name('search');\n";
    assert_eq!(uri_of(content, "search"), "search/{term?}");
}

#[test]
fn applies_fluent_group_uri_prefix() {
    let content = "<?php\nRoute::prefix('admin')->name('admin.')->group(function () {\n    Route::get('users/{user}', 'show')->name('users.show');\n});\n";
    assert_eq!(uri_of(content, "admin.users.show"), "admin/users/{user}");
}

#[test]
fn applies_nested_group_uri_prefixes() {
    let content = "<?php\nRoute::prefix('api')->group(function () {\n    Route::prefix('v1/{tenant}')->group(function () {\n        Route::get('/teams/{team}', 'show')->name('teams.show');\n    });\n});\n";
    assert_eq!(
        uri_of(content, "teams.show"),
        "api/v1/{tenant}/teams/{team}"
    );
}

#[test]
fn applies_array_group_uri_prefix() {
    let content = "<?php\nRoute::group(['prefix' => 'admin', 'as' => 'admin.'], function () {\n    Route::patch('/posts/{post}', 'update')->name('posts.update');\n});\n";
    assert_eq!(uri_of(content, "admin.posts.update"), "admin/posts/{post}");
}

#[test]
fn unrecoverable_uri_is_left_empty() {
    // A variable URI cannot be read from the source text.
    let content = "<?php\nRoute::get($uri, 'show')->name('dynamic');\n";
    assert_eq!(uri_of(content, "dynamic"), "");
}

#[test]
fn resource_routes_have_no_uri() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class);\n";
    let routes = routes_of(content);
    assert!(
        routes.iter().any(|route| route.name == "photos.show"),
        "resource names are still collected, got {routes:?}"
    );
    assert!(
        routes.iter().all(|route| route.uri.is_empty()),
        "resource URIs are not recoverable from the registration, got {routes:?}"
    );
}

#[test]
fn extracts_parameter_names() {
    assert_eq!(
        route_uri_parameters("users/{user}/posts/{post}"),
        vec!["user", "post"]
    );
}

#[test]
fn strips_optional_marker_and_binding_field() {
    assert_eq!(
        route_uri_parameters("posts/{post:slug}/comments/{comment?}"),
        vec!["post", "comment"]
    );
}

#[test]
fn parameterless_uri_yields_no_parameters() {
    assert!(route_uri_parameters("/").is_empty());
    assert!(route_uri_parameters("admin/users").is_empty());
    // Unterminated braces must not loop or panic.
    assert!(route_uri_parameters("users/{user").is_empty());
}
