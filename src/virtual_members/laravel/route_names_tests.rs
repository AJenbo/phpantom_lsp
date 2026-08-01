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
fn applies_chain_uri_prefix_without_a_group() {
    let content =
        "<?php\nRoute::prefix('{tenant}')->get('/users/{user}', 'show')->name('users.show');\n";
    assert_eq!(uri_of(content, "users.show"), "{tenant}/users/{user}");
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
fn derives_resource_route_uris() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class);\n";
    assert_eq!(uri_of(content, "photos.index"), "photos");
    assert_eq!(uri_of(content, "photos.create"), "photos/create");
    assert_eq!(uri_of(content, "photos.store"), "photos");
    assert_eq!(uri_of(content, "photos.show"), "photos/{photo}");
    assert_eq!(uri_of(content, "photos.edit"), "photos/{photo}/edit");
    assert_eq!(uri_of(content, "photos.update"), "photos/{photo}");
    assert_eq!(uri_of(content, "photos.destroy"), "photos/{photo}");
}

#[test]
fn api_resource_omits_create_and_edit() {
    let content = "<?php\nRoute::apiResource('photos', PhotoController::class);\n";
    let names: Vec<String> = routes_of(content).into_iter().map(|r| r.name).collect();
    assert!(!names.contains(&"photos.create".to_string()), "{names:?}");
    assert!(!names.contains(&"photos.edit".to_string()), "{names:?}");
    assert_eq!(uri_of(content, "photos.show"), "photos/{photo}");
}

#[test]
fn nested_resource_singularizes_each_parent_segment() {
    let content = "<?php\nRoute::resource('photos.comments', CommentController::class);\n";
    assert_eq!(
        uri_of(content, "photos.comments.index"),
        "photos/{photo}/comments"
    );
    assert_eq!(
        uri_of(content, "photos.comments.show"),
        "photos/{photo}/comments/{comment}"
    );
    assert_eq!(
        uri_of(content, "photos.comments.edit"),
        "photos/{photo}/comments/{comment}/edit"
    );
}

#[test]
fn shallow_nested_resource_drops_parent_segments() {
    // The routes that identify the child by its own id lose the parent
    // segments from their name as well as their URI.
    let content =
        "<?php\nRoute::resource('photos.comments', CommentController::class)->shallow();\n";
    assert_eq!(
        uri_of(content, "photos.comments.create"),
        "photos/{photo}/comments/create"
    );
    assert_eq!(uri_of(content, "comments.show"), "comments/{comment}");
    assert_eq!(uri_of(content, "comments.edit"), "comments/{comment}/edit");
}

#[test]
fn parameters_override_the_derived_wildcard() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)\n    ->parameters(['photos' => 'grid']);\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{grid}");
}

#[test]
fn hyphenated_resource_wildcard_uses_underscores() {
    let content = "<?php\nRoute::resource('blog-posts', PostController::class);\n";
    assert_eq!(uri_of(content, "blog-posts.show"), "blog-posts/{blog_post}");
}

#[test]
fn resource_uri_inherits_group_and_chain_prefixes() {
    let content = "<?php\nRoute::prefix('admin')->name('admin.')->group(function () {\n    Route::prefix('v2')->resource('photos', PhotoController::class);\n});\n";
    assert_eq!(
        uri_of(content, "admin.photos.show"),
        "admin/v2/photos/{photo}"
    );
}

#[test]
fn slash_in_resource_name_becomes_a_uri_prefix() {
    // Laravel registers `photos/comments` as the resource `comments` under
    // the URI prefix `photos`, so the names are not prefixed with `photos.`.
    let content = "<?php\nRoute::resource('photos/comments', CommentController::class);\n";
    assert_eq!(
        uri_of(content, "comments.show"),
        "photos/comments/{comment}"
    );
}

#[test]
fn only_and_except_still_filter_the_generated_routes() {
    let only =
        "<?php\nRoute::resource('photos', PhotoController::class)->only(['index', 'show']);\n";
    let names: Vec<String> = routes_of(only).into_iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["photos.index", "photos.show"]);
    assert_eq!(uri_of(only, "photos.show"), "photos/{photo}");

    // The filter is found even behind another chain link.
    let except = "<?php\nRoute::resource('photos', PhotoController::class)\n    ->middleware('auth')->except(['create', 'edit', 'update', 'destroy']);\n";
    let names: Vec<String> = routes_of(except).into_iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["photos.index", "photos.store", "photos.show"]);
}

#[test]
fn individual_string_args_filter_the_generated_routes() {
    // `->only()` also accepts its suffixes as separate arguments.
    let content =
        "<?php\nRoute::resource('photos', PhotoController::class)->only('index', 'show');\n";
    let names: Vec<String> = routes_of(content).into_iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["photos.index", "photos.show"]);
}

#[test]
fn nested_parameters_override_a_parent_wildcard() {
    let content = "<?php\nRoute::resource('photos.comments', CommentController::class)\n    ->parameters(['photos' => 'grid']);\n";
    assert_eq!(
        uri_of(content, "photos.comments.show"),
        "photos/{grid}/comments/{comment}"
    );
}

#[test]
fn single_parameter_call_overrides_the_wildcard() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)\n    ->parameter('photos', 'grid');\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{grid}");
}

#[test]
fn array_form_group_prefixes_apply_to_a_resource() {
    let content = "<?php\nRoute::group(['prefix' => 'admin', 'as' => 'admin.'], function () {\n    Route::resource('photos', PhotoController::class);\n});\n";
    assert_eq!(uri_of(content, "admin.photos.show"), "admin/photos/{photo}");
}

#[test]
fn api_resource_member_routes_share_one_uri() {
    let content = "<?php\nRoute::apiResource('photos', PhotoController::class);\n";
    assert_eq!(uri_of(content, "photos.index"), "photos");
    assert_eq!(uri_of(content, "photos.store"), "photos");
    assert_eq!(uri_of(content, "photos.update"), "photos/{photo}");
    assert_eq!(uri_of(content, "photos.destroy"), "photos/{photo}");
}

#[test]
fn unrecoverable_resource_name_generates_no_routes() {
    // A variable or empty name yields nothing rather than a bogus route.
    assert!(routes_of("<?php\nRoute::resource($name, PhotoController::class);\n").is_empty());
    assert!(routes_of("<?php\nRoute::resource('', PhotoController::class);\n").is_empty());
    assert!(routes_of("<?php\nRoute::resource('.', PhotoController::class);\n").is_empty());
}

#[test]
fn a_resource_chain_does_not_declare_a_route_name() {
    // `->name()` on a resource registration is Laravel's per-method name
    // override (`name($method, $name)`), not a route name of its own.
    let content =
        "<?php\nRoute::resource('photos', PhotoController::class)->name('index', 'listing');\n";
    let names: Vec<String> = routes_of(content).into_iter().map(|r| r.name).collect();
    assert!(
        !names.contains(&"index".to_string()),
        "the first argument is a method name, not a route name, got {names:?}"
    );
}

#[test]
fn resource_registered_on_a_router_variable_is_collected() {
    // `Route::group([], function ($router) { … })` hands the router to the
    // closure, and older code registers resources on it directly.
    let content = "<?php\nRoute::group([], function ($router) {\n    $router->resource('photos', PhotoController::class);\n});\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{photo}");
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
