mod common;

use common::create_test_backend;
use phpantom_lsp::Backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Helper: open a file and request completion at the given line/character.
async fn complete_at(
    backend: &Backend,
    uri: &Url,
    text: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    match backend.completion(completion_params).await.unwrap() {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        _ => vec![],
    }
}

// ─── extract_partial_variable_name unit tests ───────────────────────────────

#[test]
fn test_extract_partial_variable_name_simple() {
    let content = "<?php\n$user\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 5,
        },
    );
    assert_eq!(result, Some("$user".to_string()));
}

#[test]
fn test_extract_partial_variable_name_partial() {
    let content = "<?php\n$us\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 3,
        },
    );
    assert_eq!(result, Some("$us".to_string()));
}

#[test]
fn test_extract_partial_variable_name_bare_dollar() {
    let content = "<?php\n$\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 1,
        },
    );
    assert_eq!(
        result,
        Some("$".to_string()),
        "Bare '$' should return Some(\"$\") to trigger showing all variables"
    );
}

#[test]
fn test_extract_partial_variable_name_underscore_prefix() {
    let content = "<?php\n$_SE\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 4,
        },
    );
    assert_eq!(result, Some("$_SE".to_string()));
}

#[test]
fn test_extract_partial_variable_name_not_a_variable() {
    let content = "<?php\nfoo\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 3,
        },
    );
    assert!(
        result.is_none(),
        "Non-variable identifiers should return None"
    );
}

#[test]
fn test_extract_partial_variable_name_class_name() {
    let content = "<?php\nMyClass\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 7,
        },
    );
    assert!(result.is_none(), "Class names (no $) should return None");
}

#[test]
fn test_extract_partial_variable_name_variable_variable_skipped() {
    let content = "<?php\n$$var\n";
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 5,
        },
    );
    assert!(
        result.is_none(),
        "Variable variables ($$var) should return None"
    );
}

#[test]
fn test_extract_partial_variable_name_after_arrow_returns_none() {
    // After `->`, member completion handles this, not variable name completion.
    // The `->$` pattern doesn't actually occur in PHP (->prop not ->$prop),
    // but just make sure our guard works.
    let content = "<?php\n$obj->$prop\n";
    // Position at end of `$prop` — the `$prop` portion starts at col 6
    // extract walks back: p,r,o,p,$ — finds $ at col 6
    // then checks chars[4]='>' chars[5]='$' — not `->` at [i-2][i-1]
    // Actually the guard checks chars[i-2] and chars[i-1] where i is the position of `$`
    // i=6, chars[4]='-', chars[5]='>' → that IS `->` at positions i-2, i-1
    // Wait, let me re-check. The `$` is at index 6. i-1=5 is '>', i-2=4 is '-'. Yes, that's `->`.
    let result = Backend::extract_partial_variable_name(
        content,
        Position {
            line: 1,
            character: 11,
        },
    );
    assert!(
        result.is_none(),
        "Variable after '->' should return None (member access context)"
    );
}

// ─── Variable name completion integration tests ─────────────────────────────

/// Typing `$us` should suggest `$user` when `$user` is defined in the file.
#[tokio::test]
async fn test_completion_variable_name_basic() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_basic.php").unwrap();
    let text = concat!("<?php\n", "$user = new stdClass();\n", "$us\n",);

    // Cursor at end of `$us` on line 2
    let items = complete_at(&backend, &uri, text, 2, 3).await;

    let var_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .collect();

    let labels: Vec<&str> = var_items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"$user"),
        "Should suggest $user when typing $us. Got: {:?}",
        labels
    );
}

/// Typing `$` alone should show all variables in the file.
#[tokio::test]
async fn test_completion_bare_dollar_shows_all_variables() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_dollar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$name = 'Alice';\n",
        "$age = 30;\n",
        "$email = 'alice@example.com';\n",
        "$\n",
    );

    // Cursor right after `$` on line 4
    let items = complete_at(&backend, &uri, text, 4, 1).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$name"),
        "Should suggest $name. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$age"),
        "Should suggest $age. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$email"),
        "Should suggest $email. Got: {:?}",
        var_labels
    );
}

/// Variables should be deduplicated — even if `$user` appears multiple times.
#[tokio::test]
async fn test_completion_variable_names_deduplicated() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_dedup.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$user = getUser();\n",
        "$user->name;\n",
        "echo $user;\n",
        "$us\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 3).await;

    let user_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE) && i.label == "$user")
        .collect();

    assert_eq!(
        user_items.len(),
        1,
        "Should have exactly one $user completion (deduplicated). Got: {}",
        user_items.len()
    );
}

/// PHP superglobals should appear in variable completion.
#[tokio::test]
async fn test_completion_superglobals() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_super.php").unwrap();
    let text = concat!("<?php\n", "$_GE\n",);

    let items = complete_at(&backend, &uri, text, 1, 4).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$_GET"),
        "Should suggest $_GET superglobal. Got: {:?}",
        var_labels
    );
}

/// All PHP superglobals should be available when typing `$_`.
#[tokio::test]
async fn test_completion_all_superglobals() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_all_super.php").unwrap();
    let text = concat!("<?php\n", "$_\n",);

    let items = complete_at(&backend, &uri, text, 1, 2).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    let expected_superglobals = [
        "$_GET",
        "$_POST",
        "$_REQUEST",
        "$_SESSION",
        "$_COOKIE",
        "$_SERVER",
        "$_FILES",
        "$_ENV",
    ];

    for sg in &expected_superglobals {
        assert!(
            var_labels.contains(sg),
            "Should suggest superglobal {}. Got: {:?}",
            sg,
            var_labels
        );
    }
}

/// Superglobals should have detail "PHP superglobal" and be marked deprecated
/// (grayed out in the UI).
#[tokio::test]
async fn test_completion_superglobal_detail() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_sg_detail.php").unwrap();
    let text = concat!("<?php\n", "$_POST\n",);

    let items = complete_at(&backend, &uri, text, 1, 6).await;

    let post = items.iter().find(|i| i.label == "$_POST");
    assert!(post.is_some(), "Should find $_POST in completions");
    let post = post.unwrap();
    assert_eq!(
        post.detail.as_deref(),
        Some("PHP superglobal"),
        "Superglobals should have 'PHP superglobal' as detail"
    );
    assert_eq!(
        post.deprecated,
        Some(true),
        "Superglobals should be marked deprecated (grayed out)"
    );
}

/// User-defined variables should have detail "variable".
#[tokio::test]
async fn test_completion_variable_detail() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_detail.php").unwrap();
    let text = concat!("<?php\n", "$myVariable = 42;\n", "$myV\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let my_var = items.iter().find(|i| i.label == "$myVariable");
    assert!(my_var.is_some(), "Should find $myVariable in completions");
    assert_eq!(
        my_var.unwrap().detail.as_deref(),
        Some("variable"),
        "User variables should have 'variable' as detail"
    );
}

/// Variable completions should use CompletionItemKind::VARIABLE.
#[tokio::test]
async fn test_completion_variable_kind() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_kind.php").unwrap();
    let text = concat!("<?php\n", "$count = 42;\n", "$cou\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let count_item = items.iter().find(|i| i.label == "$count");
    assert!(count_item.is_some(), "Should find $count in completions");
    assert_eq!(
        count_item.unwrap().kind,
        Some(CompletionItemKind::VARIABLE),
        "Variable completions should use VARIABLE kind"
    );
}

/// Superglobals should sort after user-defined variables.
#[tokio::test]
async fn test_completion_superglobals_sort_after_variables() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_sg_sort.php").unwrap();
    let text = concat!("<?php\n", "$_GET['name'];\n", "$_myVar = 1;\n", "$_\n",);

    // Cursor at `$_` on line 3 — matches both $_myVar and superglobals
    let items = complete_at(&backend, &uri, text, 3, 2).await;

    let var_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .collect();

    let my_var = var_items.iter().find(|i| i.label == "$_myVar");
    let get_sg = var_items.iter().find(|i| i.label == "$_GET");

    assert!(
        my_var.is_some(),
        "Should find $_myVar. Got: {:?}",
        var_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
    assert!(
        get_sg.is_some(),
        "Should find $_GET. Got: {:?}",
        var_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );

    let my_var = my_var.unwrap();
    let get_sg = get_sg.unwrap();

    // User-defined variables should NOT be deprecated
    assert_ne!(
        my_var.deprecated,
        Some(true),
        "User-defined variables should not be marked deprecated"
    );

    // Superglobals should be deprecated (grayed out)
    assert_eq!(
        get_sg.deprecated,
        Some(true),
        "Superglobals should be marked deprecated (grayed out)"
    );

    // sort_text of user variable should come before superglobal
    assert!(
        my_var.sort_text.as_deref().unwrap() < get_sg.sort_text.as_deref().unwrap(),
        "User variables (sort_text={:?}) should sort before superglobals (sort_text={:?})",
        my_var.sort_text,
        get_sg.sort_text
    );
}

/// `insert_text` should include the `$` prefix.
#[tokio::test]
async fn test_completion_variable_insert_text_includes_dollar() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_insert.php").unwrap();
    let text = concat!("<?php\n", "$result = compute();\n", "$res\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let result_item = items.iter().find(|i| i.label == "$result");
    assert!(result_item.is_some(), "Should find $result in completions");
    assert_eq!(
        result_item.unwrap().insert_text.as_deref(),
        Some("$result"),
        "insert_text should include the $ prefix"
    );
}

/// Variables from function parameters should be suggested.
#[tokio::test]
async fn test_completion_variable_from_function_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_params.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function greet(string $firstName, string $lastName): string {\n",
        "    return $fir\n",
        "}\n",
    );

    // Cursor at end of `$fir` on line 2
    let items = complete_at(&backend, &uri, text, 2, 15).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$firstName"),
        "Should suggest $firstName from function params. Got: {:?}",
        var_labels
    );
}

/// Variables from method parameters should be suggested.
#[tokio::test]
async fn test_completion_variable_from_method_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_method_params.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class UserService {\n",
        "    public function findUser(int $userId, string $role): void {\n",
        "        $user\n",
        "    }\n",
        "}\n",
    );

    // Cursor at end of `$user` on line 3
    let items = complete_at(&backend, &uri, text, 3, 13).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$userId"),
        "Should suggest $userId from method params. Got: {:?}",
        var_labels
    );
}

/// Variables defined AFTER the cursor should NOT be suggested.
/// PHP variables don't exist until assigned, so suggesting a variable
/// defined hundreds of lines later is incorrect and confusing.
#[tokio::test]
async fn test_completion_variable_from_later_in_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_later.php").unwrap();
    let text = concat!("<?php\n", "$ear\n", "$earlyVar = 1;\n", "$laterVar = 2;\n",);

    // Cursor at end of `$ear` on line 1 — both $earlyVar and $laterVar
    // are defined AFTER the cursor, so neither should be suggested.
    let items = complete_at(&backend, &uri, text, 1, 4).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$earlyVar"),
        "$earlyVar is defined after the cursor and should NOT be suggested. Got: {:?}",
        var_labels
    );
    assert!(
        !var_labels.contains(&"$laterVar"),
        "$laterVar is defined after the cursor and should NOT be suggested. Got: {:?}",
        var_labels
    );
}

/// Variables defined BEFORE the cursor should still be suggested.
#[tokio::test]
async fn test_completion_variable_defined_before_cursor() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_before.php").unwrap();
    let text = concat!("<?php\n", "$earlyVar = 1;\n", "$laterVar = 2;\n", "$ear\n",);

    // Cursor at end of `$ear` on line 3 — both variables are defined
    // BEFORE the cursor, so both should be suggested.
    let items = complete_at(&backend, &uri, text, 3, 4).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$earlyVar"),
        "Should suggest $earlyVar (defined before cursor). Got: {:?}",
        var_labels
    );
}

/// A variable defined far below the cursor (e.g. line 535 vs line 15)
/// should NOT appear in completions.
#[tokio::test]
async fn test_completion_variable_far_below_cursor_not_suggested() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_far_below.php").unwrap();

    // Build a file where the cursor is near the top and a matching
    // variable is defined much further down.
    let mut text = String::from("<?php\n$amb\n");
    // Add many blank lines to simulate distance
    for _ in 0..100 {
        text.push_str("// filler line\n");
    }
    text.push_str("$ambiguous = new stdClass();\n");

    // Cursor at end of `$amb` on line 1
    let items = complete_at(&backend, &uri, &text, 1, 4).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$ambiguous"),
        "$ambiguous is defined far below the cursor and should NOT be suggested. Got: {:?}",
        var_labels
    );
}

/// The variable currently being typed should NOT appear in its own completions.
#[tokio::test]
async fn test_completion_excludes_variable_at_cursor() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_exclude.php").unwrap();
    let text = concat!("<?php\n", "$uniqueTestVar\n",);

    // Cursor at end of `$uniqueTestVar` on line 1 — only occurrence
    let items = complete_at(&backend, &uri, text, 1, 14).await;

    let self_items: Vec<_> = items
        .iter()
        .filter(|i| i.label == "$uniqueTestVar")
        .collect();

    assert!(
        self_items.is_empty(),
        "Should NOT suggest the variable being typed at the cursor. Got: {:?}",
        self_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Variable completion should NOT trigger after `->` (member access).
#[tokio::test]
async fn test_completion_variable_not_after_arrow() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_no_arrow.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo { public string $name; }\n",
        "$foo = new Foo();\n",
        "$foo->na\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;

    // After `->`, we should NOT get standalone variable name completions
    // (member completion handles this context).
    let standalone_var_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::VARIABLE) && i.detail.as_deref() == Some("variable")
        })
        .collect();

    assert!(
        standalone_var_items.is_empty(),
        "Standalone variable names should not appear after '->'. Got: {:?}",
        standalone_var_items
            .iter()
            .map(|i| &i.label)
            .collect::<Vec<_>>()
    );
}

/// Multiple variables with similar prefixes should all be suggested.
#[tokio::test]
async fn test_completion_multiple_matching_variables() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_multi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$userData = [];\n",
        "$userName = 'Alice';\n",
        "$userEmail = 'alice@test.com';\n",
        "$userAge = 30;\n",
        "$user\n",
    );

    // Cursor at end of `$user` on line 5
    let items = complete_at(&backend, &uri, text, 5, 5).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$userData"),
        "Should suggest $userData. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$userName"),
        "Should suggest $userName. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$userEmail"),
        "Should suggest $userEmail. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$userAge"),
        "Should suggest $userAge. Got: {:?}",
        var_labels
    );
}

/// `$this` should be suggested inside a class method even when it
/// doesn't appear elsewhere in the file (it's a built-in variable).
#[tokio::test]
async fn test_completion_this_inside_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_this.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class MyClass {\n",
        "    public function doSomething(): void {\n",
        "        $th\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 11).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$this"),
        "Should suggest $this inside a class method (built-in). Got: {:?}",
        var_labels
    );
}

/// Variables in foreach loops should be suggested.
#[tokio::test]
async fn test_completion_variable_from_foreach() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_foreach.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$items = [1, 2, 3];\n",
        "foreach ($items as $key => $value) {\n",
        "    echo $val\n",
        "}\n",
    );

    // Prefix is `$val` — should match `$value`
    let items = complete_at(&backend, &uri, text, 3, 13).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$value"),
        "Should suggest $value from foreach. Got: {:?}",
        var_labels
    );
}

/// Foreach loop key variable should be suggested with a matching prefix.
#[tokio::test]
async fn test_completion_variable_from_foreach_key() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_foreach_key.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$items = [1, 2, 3];\n",
        "foreach ($items as $key => $value) {\n",
        "    echo $ke\n",
        "}\n",
    );

    // Prefix is `$ke` — should match `$key`
    let items = complete_at(&backend, &uri, text, 3, 12).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$key"),
        "Should suggest $key from foreach. Got: {:?}",
        var_labels
    );
}

/// Variables from catch blocks should be suggested.
#[tokio::test]
async fn test_completion_variable_from_catch() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_catch.php").unwrap();
    let text = concat!(
        "<?php\n",
        "try {\n",
        "    riskyOperation();\n",
        "} catch (Exception $exception) {\n",
        "    echo $exc\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 13).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$exception"),
        "Should suggest $exception from catch block. Got: {:?}",
        var_labels
    );
}

/// `$GLOBALS` should be suggested when typing `$GL`.
#[tokio::test]
async fn test_completion_globals_superglobal() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_globals.php").unwrap();
    let text = concat!("<?php\n", "$GL\n",);

    let items = complete_at(&backend, &uri, text, 1, 3).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$GLOBALS"),
        "Should suggest $GLOBALS superglobal. Got: {:?}",
        var_labels
    );
}

/// `$argc` and `$argv` should be suggested.
#[tokio::test]
async fn test_completion_argc_argv() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_cli.php").unwrap();
    let text = concat!("<?php\n", "$arg\n",);

    let items = complete_at(&backend, &uri, text, 1, 4).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$argc"),
        "Should suggest $argc. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$argv"),
        "Should suggest $argv. Got: {:?}",
        var_labels
    );
}

/// Variable completion should work inside an if block.
#[tokio::test]
async fn test_completion_variable_inside_if_block() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_if.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$config = loadConfig();\n",
        "$connection = null;\n",
        "if ($config) {\n",
        "    $con\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 8).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$config"),
        "Should suggest $config. Got: {:?}",
        var_labels
    );
    assert!(
        var_labels.contains(&"$connection"),
        "Should suggest $connection. Got: {:?}",
        var_labels
    );
}

/// Non-variable identifiers (class names, functions) should NOT trigger
/// variable completion.
#[tokio::test]
async fn test_completion_no_variable_for_classname() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_no_class.php").unwrap();
    let text = concat!("<?php\n", "MyClass\n",);

    let items = complete_at(&backend, &uri, text, 1, 7).await;

    // This should trigger class/function/constant completion, NOT variable
    let var_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::VARIABLE) && i.detail.as_deref() == Some("variable")
        })
        .collect();

    assert!(
        var_items.is_empty(),
        "Class name identifiers should not produce variable completions. Got: {:?}",
        var_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Variable completion should work with variables containing underscores.
#[tokio::test]
async fn test_completion_variable_with_underscores() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_underscore.php").unwrap();
    let text = concat!("<?php\n", "$my_long_variable_name = 'hello';\n", "$my_lo\n",);

    let items = complete_at(&backend, &uri, text, 2, 6).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$my_long_variable_name"),
        "Should suggest $my_long_variable_name. Got: {:?}",
        var_labels
    );
}

/// Variable completion should be case-insensitive for matching.
#[tokio::test]
async fn test_completion_variable_case_insensitive() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_case.php").unwrap();
    let text = concat!("<?php\n", "$MyVariable = 42;\n", "$myv\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$MyVariable"),
        "Should suggest $MyVariable (case-insensitive match). Got: {:?}",
        var_labels
    );
}

/// Variable completion should work in a closure body.
#[tokio::test]
async fn test_completion_variable_in_closure() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_closure.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$outerVar = 'hello';\n",
        "$callback = function() use ($outerVar) {\n",
        "    echo $outer\n",
        "};\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 16).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        var_labels.contains(&"$outerVar"),
        "Should suggest $outerVar in closure. Got: {:?}",
        var_labels
    );
}

/// When no variables match the prefix, no variable completions should appear.
#[tokio::test]
async fn test_completion_no_matching_variables() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_no_match.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$apple = 1;\n",
        "$banana = 2;\n",
        "$zzz_unique_prefix_xyz\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 22).await;

    // The only variable matching `$zzz_unique_prefix_xyz` is the one at
    // the cursor itself, which should be excluded. So no matches.
    let var_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE) && i.label.starts_with("$zzz"))
        .collect();

    assert!(
        var_items.is_empty(),
        "Should not suggest variables with no match. Got: {:?}",
        var_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// `$this` used inside a class method should NOT leak to top-level scope.
/// Scope-aware collection ensures variables stay within their scope.
#[tokio::test]
async fn test_completion_this_not_visible_at_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_this_scope.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function bar(): void {\n",
        "        $this->doSomething();\n",
        "    }\n",
        "}\n",
        "$th\n",
    );

    // Cursor at top-level `$th` on line 6
    let items = complete_at(&backend, &uri, text, 6, 3).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$this"),
        "$this should NOT appear in top-level scope. Got: {:?}",
        var_labels
    );
}

/// `$this` should NOT appear inside a static method.
#[tokio::test]
async fn test_completion_this_not_in_static_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_this_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public static function create(): void {\n",
        "        $th\n",
        "    }\n",
        "}\n",
    );

    // Cursor inside static method at `$th` on line 3
    let items = complete_at(&backend, &uri, text, 3, 11).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$this"),
        "$this should NOT appear inside a static method. Got: {:?}",
        var_labels
    );
}

/// Variables defined in one method should NOT leak into another method.
#[tokio::test]
async fn test_completion_variables_scoped_to_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_method_scope.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function first(): void {\n",
        "        $onlyInFirst = 1;\n",
        "    }\n",
        "    public function second(): void {\n",
        "        $on\n",
        "    }\n",
        "}\n",
    );

    // Cursor inside second() at `$on` on line 6
    let items = complete_at(&backend, &uri, text, 6, 11).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$onlyInFirst"),
        "$onlyInFirst should NOT appear in second(). Got: {:?}",
        var_labels
    );
}

/// Method parameters should NOT appear outside of their method.
#[tokio::test]
async fn test_completion_params_scoped_to_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_param_scope.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function doWork(string $taskName, int $priority): void {\n",
        "        echo $taskName;\n",
        "    }\n",
        "}\n",
        "$ta\n",
    );

    // Cursor at top-level `$ta` on line 6
    let items = complete_at(&backend, &uri, text, 6, 3).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$taskName"),
        "$taskName should NOT appear outside its method. Got: {:?}",
        var_labels
    );
    assert!(
        !var_labels.contains(&"$priority"),
        "$priority should NOT appear outside its method. Got: {:?}",
        var_labels
    );
}

/// Properties like `$createdAt` in class declarations should NOT
/// appear as variable completions.
#[tokio::test]
async fn test_completion_properties_not_listed_as_variables() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_no_props.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Post {\n",
        "    public string $title;\n",
        "    protected ?string $createdAt = null;\n",
        "    private int $views = 0;\n",
        "    public function render(): void {\n",
        "        $cr\n",
        "    }\n",
        "}\n",
    );

    // Cursor inside render() at `$cr` on line 6
    let items = complete_at(&backend, &uri, text, 6, 11).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$createdAt"),
        "Properties should NOT appear as variable completions. Got: {:?}",
        var_labels
    );
    assert!(
        !var_labels.contains(&"$title"),
        "Properties should NOT appear as variable completions. Got: {:?}",
        var_labels
    );
    assert!(
        !var_labels.contains(&"$views"),
        "Properties should NOT appear as variable completions. Got: {:?}",
        var_labels
    );
}

/// Variables from a distant line should NOT appear at the top of a file
/// when they are in a completely different scope (function).
#[tokio::test]
async fn test_completion_variables_from_function_not_at_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_far_scope.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$a\n",
        "function farAway(): void {\n",
        "    $aDistantVariable = 42;\n",
        "}\n",
    );

    // Cursor at top-level `$a` on line 1
    let items = complete_at(&backend, &uri, text, 1, 2).await;

    let var_labels: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE))
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !var_labels.contains(&"$aDistantVariable"),
        "Variables inside a function should NOT appear at top level. Got: {:?}",
        var_labels
    );
}

/// When the same variable is used as both defined and referenced,
/// it should appear only once.
#[tokio::test]
async fn test_completion_variable_used_in_different_contexts() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_contexts.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function process(array $data): void {\n",
        "    $result = transform($data);\n",
        "    if ($result !== null) {\n",
        "        save($result);\n",
        "    }\n",
        "    $res\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 8).await;

    let result_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE) && i.label == "$result")
        .collect();

    assert_eq!(
        result_items.len(),
        1,
        "$result should appear exactly once despite multiple uses. Got: {}",
        result_items.len()
    );
}

/// Superglobals should not be duplicated even if they also appear in file content.
#[tokio::test]
async fn test_completion_superglobal_not_duplicated() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_sg_dedup.php").unwrap();
    let text = concat!("<?php\n", "$name = $_GET['name'];\n", "$_G\n",);

    let items = complete_at(&backend, &uri, text, 2, 3).await;

    let get_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::VARIABLE) && i.label == "$_GET")
        .collect();

    assert_eq!(
        get_items.len(),
        1,
        "$_GET should appear exactly once even though it's both in the file and in superglobals. Got: {}",
        get_items.len()
    );
}
