use std::path::Path;

use super::super::provider_resources::extract_provider_resources;
use super::*;

const PROVIDER: &str = "/ws/app/Providers/ViewServiceProvider.php";

fn scan(content: &str) -> super::super::provider_resources::ProviderResources {
    extract_provider_resources(
        content,
        Path::new(PROVIDER),
        Path::new("/ws"),
        super::super::const_eval::ClassContext::default(),
        Default::default(),
    )
}

/// The name of every variable a scan recorded as shared, with the source text
/// of the expression its type comes from.
fn shared(content: &str) -> Vec<(String, String)> {
    scan(content)
        .shared_view_vars
        .into_iter()
        .map(|var| (var.name, expression_at(content, var.offset)))
        .collect()
}

/// The source text starting at a recorded offset, up to the end of the
/// statement, so a test can assert *which* expression the type will be
/// resolved from.
fn expression_at(content: &str, offset: u32) -> String {
    let rest = &content[offset as usize..];
    let end = rest.find([',', ';', ')']).unwrap_or(rest.len());
    rest[..end].to_string()
}

#[test]
fn records_a_shared_variable_from_the_facade() {
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::share('siteName', config('app.name'));\n\
            }\n\
        }\n";
    assert_eq!(
        shared(content),
        vec![("siteName".to_string(), "config('app.name'".to_string())]
    );
}

#[test]
fn records_shared_variables_from_the_container_view_factory() {
    // Every way a provider reaches the factory without the facade.
    for receiver in [
        "$this->app['view']",
        "$this->app->make('view')",
        "app('view')",
        "view()",
    ] {
        let content = format!(
            "<?php\n\
            class ViewServiceProvider {{\n\
                public function boot(): void {{\n\
                    {receiver}->share('menu', $this->menu());\n\
                }}\n\
            }}\n"
        );
        assert_eq!(
            shared(&content)
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["menu"],
            "receiver: {receiver}"
        );
    }
}

#[test]
fn records_every_entry_of_an_array_share() {
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::share(['siteName' => 'Acme', 'year' => $this->year()]);\n\
            }\n\
        }\n";
    assert_eq!(
        shared(content),
        vec![
            ("siteName".to_string(), "'Acme'".to_string()),
            ("year".to_string(), "$this->year(".to_string()),
        ]
    );
}

/// A key that cannot be written as a variable names nothing a template can
/// read, so declaring it would put an unusable name in scope.
#[test]
fn a_key_that_is_not_a_variable_name_is_skipped() {
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::share('site.name', 'Acme');\n\
                View::share($runtime, 'Acme');\n\
            }\n\
        }\n";
    assert!(shared(content).is_empty());
}

/// A `share()` call on something that is not the view factory shares nothing
/// with any template.
#[test]
fn an_unrelated_share_call_is_ignored() {
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                Cache::share('siteName', 'Acme');\n\
                $this->app['config']->share('siteName', 'Acme');\n\
            }\n\
        }\n";
    assert!(shared(content).is_empty());
}

#[test]
fn records_an_inline_closure_composer() {
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::composer('profile', function ($view) {\n\
                    $view->with('user', Auth::user())->with('count', 3);\n\
                });\n\
            }\n\
        }\n";
    let composers = scan(content).view_composers;
    assert_eq!(composers.len(), 1);
    assert_eq!(composers[0].views, vec!["profile".to_string()]);
    assert_eq!(composers[0].class, None);
    // Both links of the chain are read; the outer call is reached first.
    let mut names: Vec<&str> = composers[0]
        .inline
        .iter()
        .map(|var| var.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, vec!["count", "user"]);
}

#[test]
fn records_the_views_a_composer_registration_lists() {
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::composer(['profile', 'dashboard.*'], ProfileComposer::class);\n\
            }\n\
        }\n";
    let composers = scan(content).view_composers;
    assert_eq!(composers.len(), 1);
    assert_eq!(
        composers[0].views,
        vec!["profile".to_string(), "dashboard.*".to_string()]
    );
    assert_eq!(composers[0].class.as_deref(), Some("ProfileComposer"));
}

#[test]
fn a_class_composer_name_resolves_through_the_files_imports() {
    let content = "<?php\n\
        namespace App\\Providers;\n\
        use App\\View\\Composers\\ProfileComposer;\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::composer('profile', ProfileComposer::class);\n\
            }\n\
        }\n";
    assert_eq!(
        scan(content).view_composers[0].class.as_deref(),
        Some("App\\View\\Composers\\ProfileComposer")
    );
}

#[test]
fn records_a_whole_composers_table() {
    // `composers()` reads the other way round: the handler is the key.
    let content = "<?php\n\
        class ViewServiceProvider {\n\
            public function boot(): void {\n\
                View::composers([\n\
                    ProfileComposer::class => 'profile',\n\
                    'App\\\\View\\\\Composers\\\\MenuComposer' => ['layouts.app', 'partials.*'],\n\
                ]);\n\
            }\n\
        }\n";
    let composers = scan(content).view_composers;
    assert_eq!(composers.len(), 2);
    assert_eq!(composers[0].class.as_deref(), Some("ProfileComposer"));
    assert_eq!(composers[0].views, vec!["profile".to_string()]);
    assert_eq!(
        composers[1].class.as_deref(),
        Some("App\\View\\Composers\\MenuComposer")
    );
    assert_eq!(
        composers[1].views,
        vec!["layouts.app".to_string(), "partials.*".to_string()]
    );
}

/// A composer written against a differently-named parameter is still the
/// view, so its `with()` calls still count.
#[test]
fn a_composer_body_is_read_through_its_own_parameter_name() {
    let content = "<?php\n\
        namespace App\\View\\Composers;\n\
        use Illuminate\\View\\View;\n\
        class ProfileComposer {\n\
            public function compose(View $v): void {\n\
                $v->with('user', $this->users->current());\n\
            }\n\
        }\n";
    let vars = composer_class_vars(
        content,
        Path::new("/ws/app/View/Composers/ProfileComposer.php"),
    );
    assert_eq!(
        vars.iter().map(|var| var.name.as_str()).collect::<Vec<_>>(),
        vec!["user"]
    );
    assert_eq!(
        expression_at(content, vars[0].offset),
        "$this->users->current("
    );
}

/// A `with()` call on anything but the view parameter puts nothing in the
/// template's scope.
#[test]
fn a_with_call_on_another_object_is_ignored() {
    let content = "<?php\n\
        class ProfileComposer {\n\
            public function compose($view): void {\n\
                $this->query->with('relations')->get();\n\
            }\n\
            public function other($view): void {\n\
                $view->with('stray', 1);\n\
            }\n\
        }\n";
    assert!(
        composer_class_vars(content, Path::new("/ws/ProfileComposer.php")).is_empty(),
        "only the composer's own entry point declares view data"
    );
}

#[test]
fn an_invokable_composer_declares_its_data_too() {
    let content = "<?php\n\
        class MenuComposer {\n\
            public function __invoke($view): void {\n\
                $view->with('menu', $this->menu);\n\
            }\n\
        }\n";
    assert_eq!(
        composer_class_vars(content, Path::new("/ws/MenuComposer.php"))
            .iter()
            .map(|var| var.name.as_str())
            .collect::<Vec<_>>(),
        vec!["menu"]
    );
}
