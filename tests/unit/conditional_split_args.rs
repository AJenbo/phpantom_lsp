//! Unit tests for split_text_args in conditional.rs, focusing on quoted strings and escapes.

use phpantom_lsp::type_engine::types::conditional::split_text_args;

#[test]
fn test_split_text_args_basic() {
    let args = split_text_args("a, b, c");
    assert_eq!(args, vec!["a", " b", " c"]);
}

#[test]
fn test_split_text_args_with_parentheses() {
    let args = split_text_args("foo(bar, baz), qux");
    assert_eq!(args, vec!["foo(bar, baz)", " qux"]);
}

#[test]
fn test_split_text_args_single_quoted_string_with_comma() {
    let args = split_text_args("'1,234,567.89', true");
    assert_eq!(args, vec!["'1,234,567.89'", " true"]);
}

#[test]
fn test_split_text_args_double_quoted_string_with_comma() {
    let args = split_text_args("\"hello, world\", 42");
    assert_eq!(args, vec!["\"hello, world\"", " 42"]);
}

#[test]
fn test_split_text_args_nested_quotes_and_escapes() {
    let args = split_text_args(r#""a,\"b,c\",d", 'x,\'y,z\'', foo)"#);
    assert_eq!(args, vec![r#""a,\"b,c\",d""#, " 'x,\\'y,z\\''", " foo)"]);
}

#[test]
fn test_split_text_args_mixed_quotes_and_brackets() {
    let args = split_text_args(r#"array('a,b', ["x,y"]), "foo,bar""#);
    assert_eq!(args, vec![r#"array('a,b', ["x,y"])"#, r#" "foo,bar""#]);
}

#[test]
fn test_split_text_args_escaped_quotes() {
    let args = split_text_args(r#"'foo\,bar', "baz\"qux", plain"#);
    assert_eq!(args, vec![r#"'foo\,bar'"#, r#" "baz\"qux""#, " plain"]);
}

#[test]
fn test_split_text_args_empty_and_whitespace() {
    let args = split_text_args("   ");
    assert_eq!(args, Vec::<&str>::new());

    let args = split_text_args("");
    assert_eq!(args, Vec::<&str>::new());
}
