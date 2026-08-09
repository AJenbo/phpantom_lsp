//! The render sites a view name can be written at beyond `view()` and
//! `View::make()`: `Response::view()`, the `…First` directives' candidate
//! lists, a mailable's `Content` and its `$this->view()`, and the view
//! factory's own methods.
//!
//! Each is checked twice: that the name is indexed at all (go-to-definition
//! reaches the template), and that the call site is paired with the data it
//! passes (the contract checks report against it).

#[cfg(test)]
mod tests {
    use crate::common::create_psr4_workspace;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::*;

    const COMPOSER: &str = r#"{
        "require": { "laravel/framework": "^11.0" },
        "autoload": { "psr-4": {
            "App\\": "app/",
            "Illuminate\\": "stubs/Illuminate/"
        } }
    }"#;

    /// Enough of `Illuminate\Mail\Mailable` for the subclass check, with a
    /// public property of its own so that the ones a mailable declares can
    /// be told from the ones the base does.
    const MAILABLE_STUB: &str = "<?php\nnamespace Illuminate\\Mail;\n\
        class Mailable {\n\
        \x20   public array $callbacks = [];\n\
        \x20   public function view(string $view, array $data = []): static { return $this; }\n\
        \x20   public function markdown(string $view, array $data = []): static { return $this; }\n\
        \x20   public function with(array|string $key, mixed $value = null): static { return $this; }\n\
        }\n";

    const CONTENT_STUB: &str = "<?php\nnamespace Illuminate\\Mail\\Mailables;\n\
        class Content {\n\
        \x20   public function __construct(\n\
        \x20       public ?string $view = null,\n\
        \x20       public ?string $html = null,\n\
        \x20       public ?string $text = null,\n\
        \x20       public ?string $markdown = null,\n\
        \x20       public array $with = [],\n\
        \x20   ) {}\n\
        }\n";

    /// A template declaring `$title` (string).
    const TITLED: &str = "@php\n\
        /**\n\
         * @bladestan-signature\n\
         * @var string $title\n\
         */\n\
        @endphp\n\
        <h1>{{ $title }}</h1>\n";

    fn workspace(files: &[(&str, &str)]) -> (phpantom_lsp::Backend, tempfile::TempDir) {
        let mut all = vec![
            ("stubs/Illuminate/Mail/Mailable.php", MAILABLE_STUB),
            ("stubs/Illuminate/Mail/Mailables/Content.php", CONTENT_STUB),
        ];
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

    /// The file name go-to-definition at `line`/`character` lands in.
    async fn definition_file(
        backend: &phpantom_lsp::Backend,
        dir: &tempfile::TempDir,
        relative: &str,
        language_id: &str,
        line: u32,
        character: u32,
    ) -> Option<String> {
        backend.initialized(InitializedParams {}).await;
        let path = dir.path().join(relative);
        let text = std::fs::read_to_string(&path).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        open(backend, &uri, language_id, &text).await;

        let response = backend
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: Position { line, character },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await
            .unwrap()?;
        let target = match &response {
            GotoDefinitionResponse::Scalar(location) => location.uri.clone(),
            GotoDefinitionResponse::Array(locations) => locations.first()?.uri.clone(),
            GotoDefinitionResponse::Link(links) => links.first()?.target_uri.clone(),
        };
        Some(target.to_string())
    }

    /// Collect the call-site diagnostics for one file, as `(code, message)`
    /// pairs in report order.
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

        let effective = backend.blade_virtual_php(uri.as_str()).unwrap_or(text);
        let uri = uri.to_string();
        let mut diags = Vec::new();
        backend.collect_slow_diagnostics(&uri, &effective, &mut diags);
        diags
            .into_iter()
            .filter_map(|d| match &d.code {
                Some(NumberOrString::String(code)) if code.ends_with("_view_variable") => {
                    Some((code.clone(), d.message))
                }
                _ => None,
            })
            .collect()
    }

    // ── Response::view() ────────────────────────────────────────────────

    #[tokio::test]
    async fn response_view_names_a_template() {
        let controller = "<?php\nnamespace App;\nuse Illuminate\\Support\\Facades\\Response;\n\
            class PageController {\n\
            \x20   public function about(): mixed {\n\
            \x20       return Response::view('pages.about', ['title' => 'About']);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/pages/about.blade.php", TITLED),
            ("app/PageController.php", controller),
        ]);

        let target = definition_file(&backend, &dir, "app/PageController.php", "php", 5, 35).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/pages/about.blade.php")),
            "Response::view() should reach the template, got {target:?}"
        );
    }

    #[tokio::test]
    async fn response_view_is_judged_against_the_contract() {
        let controller = "<?php\nnamespace App;\nuse Illuminate\\Support\\Facades\\Response;\n\
            class PageController {\n\
            \x20   public function about(): mixed {\n\
            \x20       return Response::view('pages.about', []);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/pages/about.blade.php", TITLED),
            ("app/PageController.php", controller),
        ]);

        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(diags[0].1.contains("$title"), "got {:?}", diags[0].1);
    }

    // ── @includeFirst / @componentFirst / @extendsFirst ─────────────────

    #[tokio::test]
    async fn include_first_names_every_candidate() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/custom/header.blade.php",
                "<header></header>\n",
            ),
            (
                "resources/views/partials/header.blade.php",
                "<header></header>\n",
            ),
            (
                "resources/views/page.blade.php",
                "@includeFirst(['custom.header', 'partials.header'])\n",
            ),
        ]);

        // The cursor sits on the *second* candidate, which nothing would
        // reach if only the first entry of the array were indexed.
        let target = definition_file(
            &backend,
            &dir,
            "resources/views/page.blade.php",
            "blade",
            0,
            35,
        )
        .await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/partials/header.blade.php")),
            "the second candidate should reach its template, got {target:?}"
        );
    }

    #[tokio::test]
    async fn include_first_judges_every_candidate() {
        let (backend, dir) = workspace(&[
            ("resources/views/custom/header.blade.php", TITLED),
            (
                "resources/views/partials/header.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var string $heading\n */\n@endphp\n<h1>{{ $heading }}</h1>\n",
            ),
            (
                "resources/views/page.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var string $page\n */\n@endphp\n\
                 @includeFirst(['custom.header', 'partials.header'], ['title' => 'Hi'])\n",
            ),
        ]);

        let diags =
            call_site_diagnostics(&backend, &dir, "resources/views/page.blade.php", "blade").await;
        assert_eq!(
            diags.len(),
            2,
            "the second candidate is short of $heading and has no use for $title: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|(code, message)| code == "missing_view_variable"
                    && message.contains("$heading")),
            "got {diags:?}"
        );
        assert!(
            diags.iter().any(
                |(code, message)| code == "unknown_view_variable" && message.contains("$title")
            ),
            "got {diags:?}"
        );
    }

    /// Only one candidate of a `…First` list is rendered, and which one
    /// depends on what is on disk, so a candidate that names nothing is why
    /// the directive takes a list at all rather than a typo.
    #[tokio::test]
    async fn an_include_first_candidate_that_names_nothing_is_not_reported() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/partials/header.blade.php",
                "<header></header>\n",
            ),
            (
                "resources/views/page.blade.php",
                "@includeFirst(['custom.header', 'partials.header'])\n@include('gone.away')\n",
            ),
        ]);

        backend.initialized(InitializedParams {}).await;
        let path = dir.path().join("resources/views/page.blade.php");
        let text = std::fs::read_to_string(&path).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        open(&backend, &uri, "blade", &text).await;

        let effective = backend.blade_virtual_php(uri.as_str()).unwrap_or(text);
        let mut diags = Vec::new();
        backend.collect_slow_diagnostics(uri.as_str(), &effective, &mut diags);
        let invalid: Vec<&String> = diags
            .iter()
            .filter(
                |d| matches!(&d.code, Some(NumberOrString::String(code)) if code == "invalid_laravel_view"),
            )
            .map(|d| &d.message)
            .collect();
        assert_eq!(
            invalid.len(),
            1,
            "only the plain @include names a view that has to exist: {invalid:?}"
        );
        assert!(invalid[0].contains("gone.away"), "got {invalid:?}");
    }

    /// A candidate that names nothing is never rendered and so reads
    /// nothing: it must not stand the unknown check down for the template
    /// that holds the directive, the way an unreadable name does.
    #[tokio::test]
    async fn an_absent_candidate_does_not_stand_the_unknown_check_down() {
        let (backend, dir) = workspace(&[
            (
                "resources/views/partials/header.blade.php",
                "<header></header>\n",
            ),
            (
                "resources/views/page.blade.php",
                "@php\n/**\n * @bladestan-signature\n * @var string $title\n */\n@endphp\n\
                 <h1>{{ $title }}</h1>\n\
                 @includeFirst(['custom.header', 'partials.header'])\n",
            ),
            (
                "app/PageController.php",
                "<?php\nnamespace App;\nclass PageController {\n\
                 \x20   public function show(): mixed {\n\
                 \x20       return view('page', ['title' => 'Hi', 'titel' => 'Hi']);\n\
                 \x20   }\n\
                 }\n",
            ),
        ]);

        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "unknown_view_variable");
        assert!(diags[0].1.contains("$titel"), "got {:?}", diags[0].1);
    }

    // ── Mailables ───────────────────────────────────────────────────────

    /// A mailable whose `content()` returns `$content`.
    fn mailable(properties: &str, content: &str) -> String {
        format!(
            "<?php\nnamespace App;\nuse Illuminate\\Mail\\Mailable;\n\
             use Illuminate\\Mail\\Mailables\\Content;\n\
             class OrderShipped extends Mailable {{\n\
             {properties}\
             \x20   public function content(): Content {{\n\
             \x20       return {content};\n\
             \x20   }}\n\
             }}\n"
        )
    }

    #[tokio::test]
    async fn a_content_view_argument_names_a_template() {
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            (
                "app/OrderShipped.php",
                &mailable(
                    "",
                    "new Content(view: 'emails.shipped', with: ['title' => 'Hi'])",
                ),
            ),
        ]);

        let target = definition_file(&backend, &dir, "app/OrderShipped.php", "php", 6, 40).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/emails/shipped.blade.php")),
            "the Content view: argument should reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/OrderShipped.php", "php").await;
        assert!(
            diags.is_empty(),
            "expected a clean call site, got {diags:?}"
        );
    }

    /// The constructor takes its view name first and its data fifth, so a
    /// `Content` written positionally down to the view name names the same
    /// template a `view:` argument does, and a `with:` after it is still
    /// the data paired with it.
    #[tokio::test]
    async fn a_positional_content_view_argument_names_a_template() {
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            (
                "app/OrderShipped.php",
                &mailable("", "new Content('emails.shipped', with: ['titel' => 'Hi'])"),
            ),
        ]);

        let target = definition_file(&backend, &dir, "app/OrderShipped.php", "php", 6, 30).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/emails/shipped.blade.php")),
            "a positional Content view argument should reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/OrderShipped.php", "php").await;
        assert_eq!(diags.len(), 2, "got {diags:?}");
        assert!(
            diags.iter().any(
                |(code, message)| code == "missing_view_variable" && message.contains("$title")
            ),
            "got {diags:?}"
        );
        assert!(
            diags.iter().any(
                |(code, message)| code == "unknown_view_variable" && message.contains("$titel")
            ),
            "the typo in with: should be reported, got {diags:?}"
        );
    }

    #[tokio::test]
    async fn a_content_pairs_its_named_data_argument() {
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            (
                "app/OrderShipped.php",
                &mailable(
                    "",
                    "new Content(view: 'emails.shipped', with: ['titel' => 'Hi'])",
                ),
            ),
        ]);

        let diags = call_site_diagnostics(&backend, &dir, "app/OrderShipped.php", "php").await;
        assert_eq!(diags.len(), 2, "got {diags:?}");
        assert!(
            diags.iter().any(
                |(code, message)| code == "missing_view_variable" && message.contains("$title")
            ),
            "got {diags:?}"
        );
        assert!(
            diags.iter().any(
                |(code, message)| code == "unknown_view_variable" && message.contains("$titel")
            ),
            "the typo in with: should be reported, got {diags:?}"
        );
    }

    #[tokio::test]
    async fn a_markdown_content_argument_names_a_template() {
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            (
                "app/OrderShipped.php",
                &mailable(
                    "",
                    "new Content(markdown: 'emails.shipped', with: ['title' => 'Hi'])",
                ),
            ),
        ]);

        let target = definition_file(&backend, &dir, "app/OrderShipped.php", "php", 6, 44).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/emails/shipped.blade.php")),
            "the Content markdown: argument should reach the template, got {target:?}"
        );
    }

    /// A mailable hands its view every public property it declares, so a
    /// `Content` that passes nothing is not short of them.
    #[tokio::test]
    async fn a_mailables_own_properties_are_not_missing() {
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            (
                "app/OrderShipped.php",
                &mailable(
                    "    public string $title = 'Hi';\n",
                    "new Content(view: 'emails.shipped')",
                ),
            ),
        ]);

        let diags = call_site_diagnostics(&backend, &dir, "app/OrderShipped.php", "php").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }

    #[tokio::test]
    async fn a_mailable_view_method_names_a_template() {
        let build = "<?php\nnamespace App;\nuse Illuminate\\Mail\\Mailable;\n\
            class OrderShipped extends Mailable {\n\
            \x20   public function build(): static {\n\
            \x20       return $this->view('emails.shipped')->with(['title' => 'Hi']);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            ("app/OrderShipped.php", build),
        ]);

        let target = definition_file(&backend, &dir, "app/OrderShipped.php", "php", 5, 32).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/emails/shipped.blade.php")),
            "$this->view() in a mailable should reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/OrderShipped.php", "php").await;
        assert!(
            diags.is_empty(),
            "the chained ->with() completes the site, got {diags:?}"
        );
    }

    /// `markdown()` names a template the same way `view()` does, and takes
    /// its data in the same place.
    #[tokio::test]
    async fn a_mailable_markdown_method_names_a_template() {
        let build = "<?php\nnamespace App;\nuse Illuminate\\Mail\\Mailable;\n\
            class OrderShipped extends Mailable {\n\
            \x20   public function build(): static {\n\
            \x20       return $this->markdown('emails.shipped', ['titel' => 'Hi']);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            ("app/OrderShipped.php", build),
        ]);

        let target = definition_file(&backend, &dir, "app/OrderShipped.php", "php", 5, 36).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/emails/shipped.blade.php")),
            "$this->markdown() in a mailable should reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/OrderShipped.php", "php").await;
        assert_eq!(diags.len(), 2, "got {diags:?}");
        assert!(
            diags.iter().any(
                |(code, message)| code == "missing_view_variable" && message.contains("$title")
            ),
            "got {diags:?}"
        );
        assert!(
            diags.iter().any(
                |(code, message)| code == "unknown_view_variable" && message.contains("$titel")
            ),
            "got {diags:?}"
        );
    }

    /// The same method name on a class that is not a mailable is an
    /// ordinary method call, not a render.
    #[tokio::test]
    async fn a_view_method_elsewhere_is_not_a_render() {
        let renderer = "<?php\nnamespace App;\nclass Renderer {\n\
            \x20   public function view(string $name): string { return $name; }\n\
            \x20   public function run(): string {\n\
            \x20       return $this->view('emails.shipped');\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/emails/shipped.blade.php", TITLED),
            ("app/Renderer.php", renderer),
        ]);

        let target = definition_file(&backend, &dir, "app/Renderer.php", "php", 5, 32).await;
        assert!(
            target
                .as_deref()
                .is_none_or(|uri| !uri.ends_with(".blade.php")),
            "a plain class's view() should name no template, got {target:?}"
        );
    }

    // ── The view factory ────────────────────────────────────────────────

    #[tokio::test]
    async fn the_factories_first_names_every_candidate() {
        let controller = "<?php\nnamespace App;\n\
            class PageController {\n\
            \x20   public function show(): mixed {\n\
            \x20       return view()->first(['custom.header', 'partials.header'], []);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            (
                "resources/views/custom/header.blade.php",
                "<header></header>\n",
            ),
            ("resources/views/partials/header.blade.php", TITLED),
            ("app/PageController.php", controller),
        ]);

        let target = definition_file(&backend, &dir, "app/PageController.php", "php", 4, 52).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/partials/header.blade.php")),
            "view()->first() should reach every candidate, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(diags[0].1.contains("$title"), "got {:?}", diags[0].1);
    }

    #[tokio::test]
    async fn the_facades_render_when_names_its_second_argument() {
        let controller = "<?php\nnamespace App;\nuse Illuminate\\Support\\Facades\\View;\n\
            class PageController {\n\
            \x20   public function show(bool $ok): mixed {\n\
            \x20       return View::renderWhen($ok, 'pages.about', ['title' => 'About']);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/pages/about.blade.php", TITLED),
            ("app/PageController.php", controller),
        ]);

        let target = definition_file(&backend, &dir, "app/PageController.php", "php", 5, 43).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/pages/about.blade.php")),
            "View::renderWhen() should reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert!(
            diags.is_empty(),
            "expected a clean call site, got {diags:?}"
        );
    }

    /// `renderUnless()` is `renderWhen()` with the condition inverted, so
    /// it names its template second and takes its data third as well.
    #[tokio::test]
    async fn the_facades_render_unless_names_its_second_argument() {
        let controller = "<?php\nnamespace App;\nuse Illuminate\\Support\\Facades\\View;\n\
            class PageController {\n\
            \x20   public function show(bool $ok): mixed {\n\
            \x20       return View::renderUnless($ok, 'pages.about', []);\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/pages/about.blade.php", TITLED),
            ("app/PageController.php", controller),
        ]);

        let target = definition_file(&backend, &dir, "app/PageController.php", "php", 5, 45).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/pages/about.blade.php")),
            "View::renderUnless() should reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert_eq!(diags.len(), 1, "got {diags:?}");
        assert_eq!(diags[0].0, "missing_view_variable");
        assert!(diags[0].1.contains("$title"), "got {:?}", diags[0].1);
    }

    /// `View::exists()` names a template without rendering it, so it hands
    /// it nothing and is not a call site to judge.
    #[tokio::test]
    async fn view_exists_names_a_template_without_rendering_it() {
        let controller = "<?php\nnamespace App;\nuse Illuminate\\Support\\Facades\\View;\n\
            class PageController {\n\
            \x20   public function show(): bool {\n\
            \x20       return View::exists('pages.about');\n\
            \x20   }\n\
            }\n";
        let (backend, dir) = workspace(&[
            ("resources/views/pages/about.blade.php", TITLED),
            ("app/PageController.php", controller),
        ]);

        let target = definition_file(&backend, &dir, "app/PageController.php", "php", 5, 34).await;
        assert!(
            target
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/resources/views/pages/about.blade.php")),
            "View::exists() should still reach the template, got {target:?}"
        );

        let diags = call_site_diagnostics(&backend, &dir, "app/PageController.php", "php").await;
        assert!(diags.is_empty(), "expected no report, got {diags:?}");
    }
}
