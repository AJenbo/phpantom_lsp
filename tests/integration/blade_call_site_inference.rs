//! Call-site variable inference for Blade templates: variables passed
//! at `view()` call sites (array literals, `compact()`, `->with()`)
//! are injected into the template's scope when it declares no `@var`
//! of its own.

#[cfg(test)]
mod tests {
    use crate::common::create_psr4_workspace;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::*;

    const COMPOSER: &str = r#"{"autoload": {"psr-4": {"App\\": "app/"}}}"#;

    const ITEM_CLASS: &str =
        "<?php\nnamespace App;\nclass Item { public string $name; public int $price; }\n";

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

    async fn hover_type(
        backend: &phpantom_lsp::Backend,
        uri: &Url,
        line: u32,
        character: u32,
    ) -> String {
        let result = backend
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position { line, character },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .unwrap();
        match result {
            Some(Hover {
                contents: HoverContents::Markup(m),
                ..
            }) => m.value,
            other => panic!("expected markup hover, got {:?}", other),
        }
    }

    /// A `view('shop', ['item' => $item])` call site types `$item`
    /// inside the template.
    #[tokio::test]
    async fn array_literal_call_site_types_template_variable() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                ("app/Item.php", ITEM_CLASS),
                (
                    "app/Controller.php",
                    "<?php\nnamespace App;\nclass Controller {\n    public function show(): mixed {\n        $item = new Item();\n        return view('shop', ['item' => $item]);\n    }\n}\n",
                ),
                ("resources/views/shop.blade.php", "{{ $item->name }}\n"),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let controller_uri = Url::from_file_path(root.join("app/Controller.php")).unwrap();
        let blade_uri = Url::from_file_path(root.join("resources/views/shop.blade.php")).unwrap();

        open(
            &backend,
            &controller_uri,
            "php",
            &std::fs::read_to_string(root.join("app/Controller.php")).unwrap(),
        )
        .await;
        open(
            &backend,
            &blade_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/shop.blade.php")).unwrap(),
        )
        .await;

        // Completion after `$item->` must offer Item's members.
        let result = backend
            .completion(CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: blade_uri.clone(),
                    },
                    position: Position {
                        line: 0,
                        character: 10,
                    },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
                context: Some(CompletionContext {
                    trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                    trigger_character: Some(">".to_string()),
                }),
            })
            .await
            .unwrap();

        let items = match result {
            Some(CompletionResponse::Array(items)) => items,
            Some(CompletionResponse::List(list)) => list.items,
            None => panic!("no completions for injected $item->"),
        };
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"name") && labels.contains(&"price"),
            "expected Item members from call-site inference, got: {:?}",
            labels
        );
    }

    /// `->with('key', $value)` chained onto `view()` and `compact()`
    /// both contribute variables.
    #[tokio::test]
    async fn with_chain_and_compact_call_sites_type_template_variables() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                ("app/Item.php", ITEM_CLASS),
                (
                    "app/Controller.php",
                    "<?php\nnamespace App;\nclass Controller {\n    public function show(): mixed {\n        $item = new Item();\n        $count = 3;\n        return view('shop')->with('item', $item)->with(compact('count'));\n    }\n}\n",
                ),
                (
                    "resources/views/shop.blade.php",
                    "{{ $item->name }}\n{{ $count }}\n",
                ),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let controller_uri = Url::from_file_path(root.join("app/Controller.php")).unwrap();
        let blade_uri = Url::from_file_path(root.join("resources/views/shop.blade.php")).unwrap();

        open(
            &backend,
            &controller_uri,
            "php",
            &std::fs::read_to_string(root.join("app/Controller.php")).unwrap(),
        )
        .await;
        open(
            &backend,
            &blade_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/shop.blade.php")).unwrap(),
        )
        .await;

        let item_hover = hover_type(&backend, &blade_uri, 0, 4).await;
        assert!(
            item_hover.contains("Item"),
            "hover on $item should show Item, got: {}",
            item_hover
        );

        // The call site passes the literal `3`, so the inferred type is
        // the literal int itself.
        let count_hover = hover_type(&backend, &blade_uri, 1, 4).await;
        assert!(
            count_hover.contains("$count = 3") || count_hover.contains("int"),
            "hover on $count should show the inferred int, got: {}",
            count_hover
        );
    }

    /// A template with its own `@var` declaration keeps it: inference
    /// is skipped entirely (declared sources win).
    #[tokio::test]
    async fn declared_var_shadows_call_site_inference() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                ("app/Item.php", ITEM_CLASS),
                (
                    "app/Controller.php",
                    "<?php\nnamespace App;\nclass Controller {\n    public function show(): mixed {\n        return view('shop', ['item' => 42]);\n    }\n}\n",
                ),
                (
                    "resources/views/shop.blade.php",
                    "@php\n/** @var \\App\\Item $item */\n@endphp\n{{ $item->name }}\n",
                ),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let controller_uri = Url::from_file_path(root.join("app/Controller.php")).unwrap();
        let blade_uri = Url::from_file_path(root.join("resources/views/shop.blade.php")).unwrap();

        open(
            &backend,
            &controller_uri,
            "php",
            &std::fs::read_to_string(root.join("app/Controller.php")).unwrap(),
        )
        .await;
        open(
            &backend,
            &blade_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/shop.blade.php")).unwrap(),
        )
        .await;

        let hover = hover_type(&backend, &blade_uri, 3, 4).await;
        assert!(
            hover.contains("Item"),
            "declared @var must win over the int call site, got: {}",
            hover
        );
    }

    /// Multiple call sites union their types per variable.
    #[tokio::test]
    async fn multiple_call_sites_union_types() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                ("app/Item.php", ITEM_CLASS),
                (
                    "app/AController.php",
                    "<?php\nnamespace App;\nclass AController {\n    public function show(): mixed {\n        return view('shop', ['subject' => new Item()]);\n    }\n}\n",
                ),
                (
                    "app/BController.php",
                    "<?php\nnamespace App;\nclass BController {\n    public function show(): mixed {\n        return view('shop', ['subject' => 'fallback']);\n    }\n}\n",
                ),
                ("resources/views/shop.blade.php", "{{ $subject }}\n"),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        for controller in ["app/AController.php", "app/BController.php"] {
            let uri = Url::from_file_path(root.join(controller)).unwrap();
            open(
                &backend,
                &uri,
                "php",
                &std::fs::read_to_string(root.join(controller)).unwrap(),
            )
            .await;
        }
        let blade_uri = Url::from_file_path(root.join("resources/views/shop.blade.php")).unwrap();
        open(
            &backend,
            &blade_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/shop.blade.php")).unwrap(),
        )
        .await;

        // One call site passes an Item, the other the literal
        // 'fallback' string; the union carries both.
        let hover = hover_type(&backend, &blade_uri, 0, 4).await;
        assert!(
            hover.contains("Item") && (hover.contains("string") || hover.contains("'fallback'")),
            "hover should union both call sites' types, got: {}",
            hover
        );
    }

    /// Injected variables must not shift diagnostics: an undefined
    /// variable in the template is still reported on the right line,
    /// and injected variables produce no undefined-variable diagnostic.
    #[tokio::test]
    async fn injected_vars_do_not_break_diagnostics() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                ("app/Item.php", ITEM_CLASS),
                (
                    "app/Controller.php",
                    "<?php\nnamespace App;\nclass Controller {\n    public function show(): mixed {\n        return view('shop', ['item' => new Item()]);\n    }\n}\n",
                ),
                (
                    "resources/views/shop.blade.php",
                    "{{ $item->name }}\n{{ $undefined_thing }}\n",
                ),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let controller_uri = Url::from_file_path(root.join("app/Controller.php")).unwrap();
        let blade_uri = Url::from_file_path(root.join("resources/views/shop.blade.php")).unwrap();

        open(
            &backend,
            &controller_uri,
            "php",
            &std::fs::read_to_string(root.join("app/Controller.php")).unwrap(),
        )
        .await;
        open(
            &backend,
            &blade_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/shop.blade.php")).unwrap(),
        )
        .await;

        // Diagnose the virtual PHP the way the live pipeline does, with
        // ranges translated back to Blade coordinates.
        let virtual_php = backend
            .blade_virtual_php(blade_uri.as_str())
            .expect("blade virtual content");
        let mut diags = Vec::new();
        backend.collect_undefined_variable_diagnostics(
            blade_uri.as_str(),
            &virtual_php,
            &mut diags,
        );
        let undefined: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Undefined variable"))
            .collect();
        assert!(
            !undefined.iter().any(|d| d.message.contains("$item")),
            "injected $item must not be flagged undefined: {:?}",
            undefined
        );
        assert!(
            undefined
                .iter()
                .any(|d| d.message.contains("$undefined_thing") && d.range.start.line == 1),
            "$undefined_thing must be flagged on Blade line 1: {:?}",
            undefined
        );
    }
}
