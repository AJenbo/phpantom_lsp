//! Integration tests for the "Sort use statements" code action.
//!
//! These exercise the full pipeline: request code actions at a cursor
//! position on a `use` line, extract the immediate edit (this action
//! isn't deferred), and verify the resulting source.

use crate::common::{
    apply_edits, create_test_backend, extract_edits, find_action, get_code_actions_on_line,
};
use tower_lsp::lsp_types::*;

#[test]
fn offers_sort_action_when_block_is_unsorted() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = "<?php\nuse Zebra\\Foo;\nuse Aardvark\\Bar;\n\nclass Test {}\n";
    backend.update_ast(uri, content);

    let actions = get_code_actions_on_line(&backend, uri, content, 1);
    let action = find_action(&actions, "Sort use statements").expect("should offer sort action");

    assert_eq!(
        action.kind,
        Some(CodeActionKind::new("source.organizeImports"))
    );

    let edits = extract_edits(action);
    let result = apply_edits(content, &edits);
    assert_eq!(
        result,
        "<?php\nuse Aardvark\\Bar;\nuse Zebra\\Foo;\n\nclass Test {}\n"
    );
}

#[test]
fn no_action_when_already_sorted() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = "<?php\nuse Aardvark\\Bar;\nuse Zebra\\Foo;\n\nclass Test {}\n";
    backend.update_ast(uri, content);

    let actions = get_code_actions_on_line(&backend, uri, content, 1);
    assert!(
        find_action(&actions, "Sort use statements").is_none(),
        "should not offer sort action when the block is already sorted"
    );
}

#[test]
fn no_action_when_cursor_not_on_use_line() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = "<?php\nuse Zebra\\Foo;\nuse Aardvark\\Bar;\n\nclass Test {}\n";
    backend.update_ast(uri, content);

    let actions = get_code_actions_on_line(&backend, uri, content, 4);
    assert!(
        find_action(&actions, "Sort use statements").is_none(),
        "should not offer sort action when cursor is away from the use block"
    );
}

#[test]
fn no_action_for_single_import() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = "<?php\nuse Foo\\Bar;\n\nclass Test {}\n";
    backend.update_ast(uri, content);

    let actions = get_code_actions_on_line(&backend, uri, content, 1);
    assert!(find_action(&actions, "Sort use statements").is_none());
}
