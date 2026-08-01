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

/// The enclosing group's prefix reaches the resource, but a `->prefix()` on
/// the registration's own chain does not: `ResourceRegistrar` builds its
/// action from `as`/`uses`/`middleware`/`where`/`missing` and never copies
/// `prefix` across, so Laravel silently drops it.
#[test]
fn resource_uri_inherits_the_group_prefix_but_not_the_chain_prefix() {
    let content = "<?php\nRoute::prefix('admin')->name('admin.')->group(function () {\n    Route::prefix('v2')->resource('photos', PhotoController::class);\n});\n";
    assert_eq!(uri_of(content, "admin.photos.show"), "admin/photos/{photo}");
}

/// `->as()` and `->name()` ahead of the registration prefix every generated
/// route name, and the last one on the chain wins.
#[test]
fn a_chain_as_prefix_reaches_the_generated_route_names() {
    let content = "<?php\nRoute::as('admin')->resource('photos', PhotoController::class);\n";
    assert_eq!(uri_of(content, "admin.photos.show"), "photos/{photo}");

    let replaced = "<?php\nRoute::as('a')->as('b')->resource('photos', PhotoController::class);\n";
    assert_eq!(uri_of(replaced, "b.photos.show"), "photos/{photo}");
}

/// The registrar appends its own separator, so the trailing dot users write
/// out of habit produces a doubled one rather than being absorbed.
#[test]
fn a_chain_name_prefix_keeps_a_trailing_dot_the_user_wrote() {
    let content = "<?php\nRoute::name('admin.')->resource('photos', PhotoController::class);\n";
    assert_eq!(uri_of(content, "admin..photos.show"), "photos/{photo}");
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
fn chains_that_register_no_resource_contribute_nothing() {
    // A method chain on something that is not a registration must not be
    // mistaken for one, however deep the receiver goes.
    assert!(routes_of("<?php\n$router->middleware('auth')->boot();\n").is_empty());
}

#[test]
fn an_unrecognized_modifier_leaves_the_registration_intact() {
    // A dynamic method name cannot be matched against the known modifiers, so
    // it is skipped rather than discarding the registration.
    let content = "<?php\nRoute::resource('photos', C::class)->{$modifier}();\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{photo}");
}

#[test]
fn legacy_array_parameters_override_the_wildcard() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)\n    ->parameters(array('photos' => 'grid'));\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{grid}");
}

#[test]
fn a_non_array_parameters_argument_leaves_the_wildcard_derived() {
    // Laravel's `->parameters('singular')` string form asks for exactly the
    // singularization that is already the default here.
    let singular =
        "<?php\nRoute::resource('photos', PhotoController::class)->parameters('singular');\n";
    assert_eq!(uri_of(singular, "photos.show"), "photos/{photo}");

    let dynamic = "<?php\nRoute::resource('photos', PhotoController::class)->parameters($map);\n";
    assert_eq!(uri_of(dynamic, "photos.show"), "photos/{photo}");
}

#[test]
fn computed_parameters_entries_leave_the_wildcard_derived() {
    // An entry whose key or value is not a literal contributes nothing rather
    // than a wrong wildcard, and an element without a key is not an override.
    let dynamic_value =
        "<?php\nRoute::resource('photos', C::class)->parameters(['photos' => $name]);\n";
    assert_eq!(uri_of(dynamic_value, "photos.show"), "photos/{photo}");

    let dynamic_key = "<?php\nRoute::resource('photos', C::class)->parameters([$key => 'grid']);\n";
    assert_eq!(uri_of(dynamic_key, "photos.show"), "photos/{photo}");

    let keyless = "<?php\nRoute::resource('photos', C::class)->parameters(['grid']);\n";
    assert_eq!(uri_of(keyless, "photos.show"), "photos/{photo}");
}

#[test]
fn single_parameter_call_overrides_the_wildcard() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)\n    ->parameter('photos', 'grid');\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{grid}");

    // Both arguments are required for an override; a partial call is ignored.
    let partial = "<?php\nRoute::resource('photos', C::class)->parameter('photos');\n";
    assert_eq!(uri_of(partial, "photos.show"), "photos/{photo}");
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

/// `->name()` on a resource registration is Laravel's per-method name
/// override (`name($method, $name)`), not a route name of its own.  The
/// override is the *whole* name, so it replaces `photos.index` rather than
/// being appended to it.
#[test]
fn a_resource_chain_name_overrides_one_methods_route_name() {
    let content =
        "<?php\nRoute::resource('photos', PhotoController::class)->name('index', 'listing');\n";
    let names: Vec<String> = routes_of(content).into_iter().map(|r| r.name).collect();
    assert!(
        names.contains(&"listing".to_string()),
        "the override should name the index route, got {names:?}"
    );
    assert!(
        !names.contains(&"photos.index".to_string()),
        "the override should replace the derived name, got {names:?}"
    );
    assert!(
        !names.contains(&"index".to_string()),
        "the first argument is a method name, not a route name, got {names:?}"
    );
    // The other six keep their derived names.
    assert!(names.contains(&"photos.show".to_string()), "{names:?}");
}

/// `->names('images')` renames the resource every route is derived from,
/// while a per-method entry bypasses the `->as()` prefix entirely.
#[test]
fn names_rewrites_the_resource_the_routes_are_derived_from() {
    let content =
        "<?php\nRoute::as('admin')->resource('photos', PhotoController::class)->names('images');\n";
    assert_eq!(uri_of(content, "admin.images.show"), "photos/{photo}");

    let per_method = "<?php\nRoute::as('admin')->resource('photos', PhotoController::class)->name('index', 'x');\n";
    let names: Vec<String> = routes_of(per_method).into_iter().map(|r| r.name).collect();
    assert!(names.contains(&"x".to_string()), "{names:?}");
}

/// `getResourceMethods()` intersects with `only` and *then* subtracts
/// `except`; neither cancels the other out.
#[test]
fn only_and_except_are_both_applied() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)->only(['index', 'create'])->except(['create']);\n";
    let names: Vec<String> = routes_of(content).into_iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["photos.index".to_string()]);
}

/// An empty `->only([])` restricts to nothing, which is not the same as
/// never having called it.
#[test]
fn an_empty_only_registers_no_routes() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)->only([]);\n";
    assert!(routes_of(content).is_empty());
}

/// `apiResource()` is an implicit `only` of the five API methods, so an
/// explicit `->only()` replaces it and can bring `create` back.
#[test]
fn an_explicit_only_replaces_the_api_resource_restriction() {
    let content =
        "<?php\nRoute::apiResource('photos', PhotoController::class)->only(['create']);\n";
    assert_eq!(uri_of(content, "photos.create"), "photos/create");

    // `->except()` narrows the API set instead of replacing it.
    let except = "<?php\nRoute::apiResource('photos', PhotoController::class)->except(['show']);\n";
    let names: Vec<String> = routes_of(except).into_iter().map(|r| r.name).collect();
    assert!(!names.contains(&"photos.create".to_string()), "{names:?}");
    assert!(!names.contains(&"photos.show".to_string()), "{names:?}");
    assert!(names.contains(&"photos.index".to_string()), "{names:?}");
}

/// `->shallow(false)` turns shallow routing back off.
#[test]
fn shallow_reads_its_argument() {
    let off =
        "<?php\nRoute::resource('photos.comments', CommentController::class)->shallow(false);\n";
    assert_eq!(
        uri_of(off, "photos.comments.show"),
        "photos/{photo}/comments/{comment}"
    );

    let on = "<?php\nRoute::resource('photos.comments', CommentController::class)->shallow();\n";
    assert_eq!(uri_of(on, "comments.show"), "comments/{comment}");
}

/// `->parameters()` replaces the whole map and `->parameter()` appends to
/// it, so whichever came last on the chain is the one that applies.
#[test]
fn the_last_parameter_override_for_a_segment_wins() {
    let content = "<?php\nRoute::resource('photos', PhotoController::class)->parameters(['photos' => 'grid'])->parameter('photos', 'other');\n";
    assert_eq!(uri_of(content, "photos.show"), "photos/{other}");
}

/// Laravel deletes the last segment's wildcard from the nested URI without
/// anchoring the deletion, so segments that singularize alike collapse.
#[test]
fn a_repeated_wildcard_collapses_the_nested_uri() {
    let content = "<?php\nRoute::resource('company.companies', CompanyController::class);\n";
    assert_eq!(
        uri_of(content, "company.companies.show"),
        "company/companies/{company}"
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

/// `RouteRegistrar::group()` returns the registrar, so a `->group()` and a
/// `->resource()` can share one chain.  The resource must not swallow the
/// group's body, or its routes vanish from completion and every `route()`
/// call naming one is reported as unknown.
#[test]
fn a_group_sharing_the_resource_chain_still_registers_its_routes() {
    let content = "<?php\nRoute::prefix('admin')->group(function () {\n    Route::get('/dashboard', 'index')->name('dashboard');\n})->resource('photos', PhotoController::class);\n";
    let names: Vec<String> = routes_of(content).into_iter().map(|r| r.name).collect();
    assert!(names.contains(&"dashboard".to_string()), "{names:?}");
    assert!(names.contains(&"photos.show".to_string()), "{names:?}");
    assert_eq!(uri_of(content, "dashboard"), "admin/dashboard");
}
