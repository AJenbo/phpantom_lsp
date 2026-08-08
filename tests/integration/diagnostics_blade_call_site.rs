//! `view()` call sites are checked against the contract the template they
//! name declares: a declared variable that is not passed, a value whose
//! type the declaration does not accept, and a variable nothing in the
//! template reads are each reported.
//!
//! A template that declares no contract is not checked at all, and every
//! check stands down as soon as the data involved stops being readable.

#[cfg(test)]
mod tests {
    use crate::common::create_psr4_workspace;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::*;

    const COMPOSER: &str = r#"{
        "require": { "laravel/framework": "^11.0" },
        "autoload": { "psr-4": { "App\\": "app/" } }
    }"#;

    const USER_CLASS: &str =
        "<?php\nnamespace App\\Models;\nclass User { public string $email = ''; }\n";

    /// A template declaring `$title` (string) and `$user` (User).
    const PROFILE: &str = "@php\n\
        /**\n\
         * @bladestan-signature\n\
         * @var string $title\n\
         * @var \\App\\Models\\User $user\n\
         */\n\
        @endphp\n\
        <h1>{{ $title }}</h1>\n\
        <p>{{ $user->email }}</p>\n";

    fn workspace(files: &[(&str, &str)]) -> (phpantom_lsp::Backend, tempfile::TempDir) {
        let mut all = vec![("app/Models/User.php", USER_CLASS)];
        all.extend_from_slice(files);
        create_psr4_workspace(COMPOSER, &all)
    }

    async fn open(backend: &phpantom_lsp::Backend, uri: &Url, language_id: &str, text: &str) {
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: language_id.to_string(),
                    version: 1,
                    text: text.to_string(),
                },
            })
            .await;
    }

    /// Collect the call-site diagnostics for one file, as
    /// `(code, message)` pairs in report order.
    async fn call_site_diagnostics(
        backend: &phpantom_lsp::Backend,
        dir: &tempfile::TempDir,
        relative: &str,
        language_id: &str,
    ) -> Vec<(String, String)> {
        backend.initialized(InitializedParams {}).await;
        let path = dir.path().join(relative);
        let text = std::fs::read_to_string(&path).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        open(backend, &uri, language_id, &text).await;

        // A Blade file is analysed through its virtual PHP, which is what
        // the symbol map and the source map are both keyed to.
        let effective = backend.blade_virtual_php(uri.as_str()).unwrap_or(text);
        let uri = uri.to_string();
        let mut diags = Vec::new();
        backend.collect_slow_diagnostics(&uri, &effective, &mut diags);
        diags
            .into_iter()
            .filter_map(|d| match &d.code {
                Some(NumberOrString::String(code))
                    if code.ends_with("_view_variable")
                        || code == "type_mismatch_view_variable" =>
                {
                    Some((code.clone(), d.message))
                }
                _ => None,
            })
            .collect()
    }

    /// A controller that renders `profile` with `data` as its data argument.
    fn controller(data: &str) -> String {
        format!(
            "<?php\nnamespace App;\nuse App\\Models\\User;\nclass ProfileController {{\n    public function show(User $user): mixed {{\n        return view('profile'{data});\n    }}\n}}\n"
        )
    }

    #[tokio::test]
    async fn a_complete_call_site_reports_nothing() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                &controller(", ['title' => 'Profile', 'user' => $user]"),
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert!(
            diags.is_empty(),
            "expected a clean call site, got {diags:?}"
        );
    }

    #[tokio::test]
    async fn a_declared_variable_that_is_not_passed_is_reported() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                &controller(", ['title' => 'Profile']"),
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(
            diags[0].1.contains("$user") && diags[0].1.contains("profile"),
            "message should name the variable and the view, got {:?}",
            diags[0].1
        );
    }

    #[tokio::test]
    async fn a_call_site_with_no_data_at_all_is_reported() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            ("app/ProfileController.php", &controller("")),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert_eq!(
            diags.len(),
            2,
            "both declared variables are missing: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .all(|(code, _)| code == "missing_view_variable")
        );
    }

    #[tokio::test]
    async fn a_value_of_the_wrong_type_is_reported() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                &controller(", ['title' => 42, 'user' => $user]"),
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "type_mismatch_view_variable");
        assert!(
            diags[0].1.contains("$title") && diags[0].1.contains("string"),
            "message should name the variable and the declared type, got {:?}",
            diags[0].1
        );
    }

    #[tokio::test]
    async fn a_variable_the_template_never_reads_is_reported() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                &controller(", ['title' => 'Profile', 'user' => $user, 'usre' => $user]"),
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "unknown_view_variable");
        assert!(
            diags[0].1.contains("$usre"),
            "message should name the typo, got {:?}",
            diags[0].1
        );
    }

    /// A partial three levels down is still handed the whole data array,
    /// so a name only it reads is not unwanted.
    #[tokio::test]
    async fn a_variable_an_included_partial_reads_is_accepted() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/profile.blade.php",
                &format!("{PROFILE}@include('partials.badge')\n"),
            ),
            ("resources/views/partials/badge.blade.php", "{{ $badge }}\n"),
            (
                "app/ProfileController.php",
                &controller(", ['title' => 'Profile', 'user' => $user, 'badge' => 'new']"),
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    /// An include whose name is built at runtime hides a whole template,
    /// so nothing about the extra names can be concluded.
    #[tokio::test]
    async fn a_dynamic_include_stands_the_unknown_check_down() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/profile.blade.php",
                &format!("{PROFILE}@include($partial)\n"),
            ),
            (
                "app/ProfileController.php",
                &controller(", ['title' => 'Profile', 'user' => $user, 'whatever' => 1]"),
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    /// A layout renders from the child's data, so what it declares the
    /// caller has to supply.
    #[tokio::test]
    async fn a_layouts_declaration_is_part_of_the_childs_contract() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/layouts/app.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var string $siteName\n */\n@endphp\n<title>{{ $siteName }}</title>\n@yield('body')\n",
            ),
            (
                "resources/views/page.blade.php",
                "@extends('layouts.app')\n@php\n/**\n * @bladestan-signature\n * @var string $title\n */\n@endphp\n@section('body'){{ $title }}@endsection\n",
            ),
            (
                "app/PageController.php",
                "<?php\nnamespace App;\nclass PageController {\n    public function show(): mixed {\n        return view('page', ['title' => 'Hi']);\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(
            diags[0].1.contains("$siteName"),
            "message should name the layout's variable, got {:?}",
            diags[0].1
        );
    }

    /// A template that declares nothing has no contract to be judged
    /// against, which is what keeps the whole check opt-in.
    #[tokio::test]
    async fn a_template_without_a_signature_is_not_checked() {
        let (backend, dir) = workspace(&[
            ("resources/views/plain.blade.php", "<h1>{{ $title }}</h1>\n"),
            (
                "app/PlainController.php",
                "<?php\nnamespace App;\nclass PlainController {\n    public function show(): mixed {\n        return view('plain', ['anything' => 1]);\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/PlainController.php", "php").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    /// A data array built from a variable hides the names it carries, so
    /// neither a missing nor an unwanted one can be concluded.
    #[tokio::test]
    async fn an_unreadable_data_argument_stands_the_checks_down() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                "<?php\nnamespace App;\nclass ProfileController {\n    public function show(array $data): mixed {\n        return view('profile', $data);\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    /// `compact()` names the variables it passes just as an array literal
    /// does.
    #[tokio::test]
    async fn compact_is_read_as_the_data_it_passes() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                "<?php\nnamespace App;\nuse App\\Models\\User;\nclass ProfileController {\n    public function show(User $user): mixed {\n        $title = 'Profile';\n        return view('profile', compact('title', 'user'));\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert!(
            diags.is_empty(),
            "expected a clean call site, got {diags:?}"
        );
    }

    /// Data added over a chain is one call site's worth, not several.
    #[tokio::test]
    async fn a_with_chain_completes_the_call_site() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                "<?php\nnamespace App;\nuse App\\Models\\User;\nclass ProfileController {\n    public function show(User $user): mixed {\n        return view('profile')->with('title', 'Profile')->withUser($user);\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert!(
            diags.is_empty(),
            "expected a clean call site, got {diags:?}"
        );
    }

    #[tokio::test]
    async fn view_make_is_checked_like_the_helper() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                "<?php\nnamespace App;\nuse Illuminate\\Support\\Facades\\View;\nclass ProfileController {\n    public function show(): mixed {\n        return View::make('profile', ['title' => 'Profile']);\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
    }

    /// `Route::view()` names its template second, and its data third.
    #[tokio::test]
    async fn route_view_is_checked_like_the_helper() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "routes/web.php",
                "<?php\nuse Illuminate\\Support\\Facades\\Route;\nRoute::view('/profile', 'profile', ['title' => 'Profile']);\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "routes/web.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(diags[0].1.contains("$user"), "got {:?}", diags[0].1);
    }

    /// Laravel merges a component's public members into its view's data, so
    /// a `render()` that passes nothing still satisfies the contract.
    #[tokio::test]
    async fn a_components_own_members_satisfy_the_view_it_renders() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/View/Components/ProfileCard.php",
                "<?php\nnamespace App\\View\\Components;\nuse App\\Models\\User;\nuse Illuminate\\View\\Component;\nclass ProfileCard extends Component {\n    public string $title = '';\n    public ?User $user = null;\n    public function render(): mixed {\n        return view('profile');\n    }\n}\n",
            ),
        ]);
        let diags =
            call_site_diagnostics(&backend, &dir, "app/View/Components/ProfileCard.php", "php")
                .await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    /// A plain controller's properties never reach the view, so they excuse
    /// nothing.
    #[tokio::test]
    async fn a_plain_classs_properties_do_not_satisfy_the_view_it_renders() {
        let (backend, dir) = workspace(&[
            ("resources/views/profile.blade.php", PROFILE),
            (
                "app/ProfileController.php",
                "<?php\nnamespace App;\nuse App\\Models\\User;\nclass ProfileController {\n    public string $title = '';\n    public ?User $user = null;\n    public function show(): mixed {\n        return view('profile');\n    }\n}\n",
            ),
        ]);
        let diags = call_site_diagnostics(&backend, &dir, "app/ProfileController.php", "php").await;
        assert_eq!(diags.len(), 2, "got {diags:?}");
    }

    /// `$loop` is Blade's own object however a call site writes it down, so
    /// the type a template declares for it is not the caller's to satisfy.
    #[tokio::test]
    async fn blades_own_loop_variable_is_not_type_checked() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/partials/row.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var \\App\\Models\\LoopStub $loop\n */\n@endphp\n<td>{{ $loop->index }}</td>\n",
            ),
            (
                "app/Models/LoopStub.php",
                "<?php\nnamespace App\\Models;\nclass LoopStub { public int $index = 0; }\n",
            ),
            (
                "resources/views/table.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var array<int, string> $rows\n */\n@endphp\n@foreach ($rows as $row)\n@include('partials.row', ['loop' => $loop])\n@endforeach\n",
            ),
        ]);
        let diags =
            call_site_diagnostics(&backend, &dir, "resources/views/table.blade.php", "blade").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    /// `@include` inside a template that declares its own contract is
    /// judged against what that contract puts in scope.
    #[tokio::test]
    async fn an_include_is_checked_against_the_including_templates_scope() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/partials/card.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var \\App\\Models\\User $user\n * @var string $caption\n */\n@endphp\n<p>{{ $user->email }} {{ $caption }}</p>\n",
            ),
            (
                "resources/views/page.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var \\App\\Models\\User $user\n */\n@endphp\n@include('partials.card')\n",
            ),
        ]);
        let diags =
            call_site_diagnostics(&backend, &dir, "resources/views/page.blade.php", "blade").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(
            diags[0].1.contains("$caption"),
            "the includer already has $user in scope; only $caption is short, got {:?}",
            diags[0].1
        );
    }

    /// Without a contract of its own, an including template's inbound data
    /// is whatever its callers happened to pass, so nothing is missing.
    #[tokio::test]
    async fn an_include_from_an_undeclared_template_is_not_checked_for_missing_data() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/partials/card.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var string $caption\n */\n@endphp\n<p>{{ $caption }}</p>\n",
            ),
            (
                "resources/views/page.blade.php",
                "<h1>Page</h1>\n@include('partials.card')\n",
            ),
        ]);
        let diags =
            call_site_diagnostics(&backend, &dir, "resources/views/page.blade.php", "blade").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }
}
