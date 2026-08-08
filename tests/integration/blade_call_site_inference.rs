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

    /// A bound attribute on an `<x-…>` component tag types the variable
    /// inside the component's own template, even though the component
    /// declares no `@props` for it.
    #[tokio::test]
    async fn bound_component_attribute_types_the_component_variable() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                ("app/Item.php", ITEM_CLASS),
                (
                    "resources/views/page.blade.php",
                    "<x-brand.boxes :hairAnalysis=\"$model\" />\n",
                ),
                (
                    "resources/views/components/brand/boxes.blade.php",
                    "{{ $hairAnalysis->name }}\n",
                ),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let page_uri = Url::from_file_path(root.join("resources/views/page.blade.php")).unwrap();
        let component_uri =
            Url::from_file_path(root.join("resources/views/components/brand/boxes.blade.php"))
                .unwrap();

        // The caller must declare `$model`'s type for inference to pick
        // it up; a `@php` block is the simplest way to do that in a plain
        // view.
        let page_source = "@php\n/** @var \\App\\Item $model */\n@endphp\n<x-brand.boxes :hairAnalysis=\"$model\" />\n";
        open(&backend, &page_uri, "blade", page_source).await;
        open(
            &backend,
            &component_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/components/brand/boxes.blade.php"))
                .unwrap(),
        )
        .await;

        let hover = hover_type(&backend, &component_uri, 0, 4).await;
        assert!(
            hover.contains("Item"),
            "$hairAnalysis should be typed Item from the tag's bound attribute, got: {}",
            hover
        );
    }

    /// A plain string attribute on an `<x-…>` tag still declares the
    /// variable (as `string`), so the component body does not report it
    /// as undefined.
    #[tokio::test]
    async fn literal_component_attribute_is_not_reported_undefined() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                (
                    "resources/views/page.blade.php",
                    "<x-alert type=\"danger\" />\n",
                ),
                (
                    "resources/views/components/alert.blade.php",
                    "{{ $type }}\n",
                ),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let page_uri = Url::from_file_path(root.join("resources/views/page.blade.php")).unwrap();
        let component_uri =
            Url::from_file_path(root.join("resources/views/components/alert.blade.php")).unwrap();

        open(
            &backend,
            &page_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/page.blade.php")).unwrap(),
        )
        .await;
        open(
            &backend,
            &component_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/components/alert.blade.php"))
                .unwrap(),
        )
        .await;

        let virtual_php = backend
            .blade_virtual_php(component_uri.as_str())
            .expect("blade virtual content");
        let mut diags = Vec::new();
        backend.collect_undefined_variable_diagnostics(
            component_uri.as_str(),
            &virtual_php,
            &mut diags,
        );
        let undefined: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Undefined variable"))
            .collect();
        assert!(
            undefined.is_empty(),
            "$type should be declared from the tag's literal attribute: {:?}",
            undefined
        );
    }

    /// `@props` wins over the call-site-inferred type for the same name,
    /// per the declaration priority chain.
    #[tokio::test]
    async fn props_default_wins_over_component_tag_inference() {
        let (backend, _dir) = create_psr4_workspace(
            COMPOSER,
            &[
                (
                    "resources/views/page.blade.php",
                    "<x-alert type=\"danger\" />\n",
                ),
                (
                    "resources/views/components/alert.blade.php",
                    "@props(['type' => 1])\n{{ $type }}\n",
                ),
            ],
        );

        let root = backend.workspace_root().read().clone().unwrap();
        let page_uri = Url::from_file_path(root.join("resources/views/page.blade.php")).unwrap();
        let component_uri =
            Url::from_file_path(root.join("resources/views/components/alert.blade.php")).unwrap();

        open(
            &backend,
            &page_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/page.blade.php")).unwrap(),
        )
        .await;
        open(
            &backend,
            &component_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/components/alert.blade.php"))
                .unwrap(),
        )
        .await;

        // `@props` types `$type` from its default `1`; the page's literal
        // `"danger"` string attribute must not override it.
        let hover = hover_type(&backend, &component_uri, 1, 4).await;
        assert!(
            hover.contains('1') && !hover.contains("danger"),
            "@props default should win over the tag's literal attribute, got: {}",
            hover
        );
    }

    /// Open a workspace where `shop.blade.php` receives an `App\Item` from a
    /// `view()` call but never names the class itself, so `App\Item` appears
    /// in the template's virtual PHP only through the injected prologue.
    async fn workspace_naming_item_only_in_the_prologue()
    -> (phpantom_lsp::Backend, tempfile::TempDir, Url, Url) {
        let (backend, dir) = create_psr4_workspace(
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
        let item_uri = Url::from_file_path(root.join("app/Item.php")).unwrap();
        let blade_uri = Url::from_file_path(root.join("resources/views/shop.blade.php")).unwrap();

        for rel in ["app/Item.php", "app/Controller.php"] {
            let uri = Url::from_file_path(root.join(rel)).unwrap();
            open(
                &backend,
                &uri,
                "php",
                &std::fs::read_to_string(root.join(rel)).unwrap(),
            )
            .await;
        }
        open(
            &backend,
            &blade_uri,
            "blade",
            &std::fs::read_to_string(root.join("resources/views/shop.blade.php")).unwrap(),
        )
        .await;

        (backend, dir, item_uri, blade_uri)
    }

    /// Every text edit a workspace edit applies to `uri`, from either the
    /// `changes` map or the `document_changes` operation list.
    fn edits_for(edit: &WorkspaceEdit, uri: &Url) -> Vec<Range> {
        let mut ranges = Vec::new();

        if let Some(changes) = &edit.changes
            && let Some(edits) = changes.get(uri)
        {
            ranges.extend(edits.iter().map(|e| e.range));
        }

        let doc_edits: Vec<&TextDocumentEdit> = match &edit.document_changes {
            Some(DocumentChanges::Edits(edits)) => edits.iter().collect(),
            Some(DocumentChanges::Operations(ops)) => ops
                .iter()
                .filter_map(|op| match op {
                    DocumentChangeOperation::Edit(e) => Some(e),
                    DocumentChangeOperation::Op(_) => None,
                })
                .collect(),
            None => Vec::new(),
        };
        for doc_edit in doc_edits {
            if doc_edit.text_document.uri != *uri {
                continue;
            }
            ranges.extend(doc_edit.edits.iter().map(|e| match e {
                OneOf::Left(e) => e.range,
                OneOf::Right(e) => e.text_edit.range,
            }));
        }

        ranges
    }

    /// A template that names a class only through its injected prologue is
    /// not a reference site: the prologue is not template text, so there is
    /// no Blade position to report.
    #[tokio::test]
    async fn references_skip_a_class_named_only_by_the_prologue() {
        let (backend, _dir, item_uri, blade_uri) =
            workspace_naming_item_only_in_the_prologue().await;

        let locations = backend
            .references(ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: item_uri.clone(),
                    },
                    position: Position {
                        line: 2,
                        character: 7,
                    },
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
                context: ReferenceContext {
                    include_declaration: true,
                },
            })
            .await
            .unwrap()
            .unwrap_or_default();

        let in_template: Vec<_> = locations.iter().filter(|l| l.uri == blade_uri).collect();
        assert!(
            in_template.is_empty(),
            "the template never names App\\Item; the prologue is not a reference site: {:?}",
            in_template
        );
    }

    /// Renaming a class a template only receives (never names) must leave the
    /// template alone.  Clamping the prologue match to `0:0` used to produce
    /// an empty-range edit that prepended the new name to the template.
    #[tokio::test]
    async fn rename_does_not_write_into_a_template_that_only_names_the_class_in_its_prologue() {
        let (backend, _dir, item_uri, blade_uri) =
            workspace_naming_item_only_in_the_prologue().await;

        let edit = backend
            .rename(RenameParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: item_uri.clone(),
                    },
                    position: Position {
                        line: 2,
                        character: 7,
                    },
                },
                new_name: "Product".to_string(),
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await
            .unwrap();

        let Some(edit) = edit else {
            return;
        };

        let template_edits = edits_for(&edit, &blade_uri);
        assert!(
            template_edits.is_empty(),
            "renaming App\\Item must not edit a template that only receives it: {:?}",
            template_edits
        );
    }
}
