use super::*;

/// Place the cursor immediately after `needle` and detect the context there.
fn detect_at(content: &str, needle: &str) -> Option<RequestFieldContext> {
    let offset = content.find(needle).unwrap() + needle.len();
    detect_at_offset(content, offset)
}

/// Detect the context at a byte offset, resolving the lexical position the
/// completion handler would otherwise pass in.
fn detect_at_offset(content: &str, offset: usize) -> Option<RequestFieldContext> {
    let code = crate::completion::source::code_context::code_context_at(content, offset)?;
    detect_request_field_context(content, offset, &code)
}

#[test]
fn detects_input_call() {
    let content = "<?php\n$request->input('na');\n";
    let ctx = detect_at(content, "input('na").expect("should detect input()");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "na");
    assert_eq!(ctx.full_value(content), Some("na"));
}

#[test]
fn detects_nullsafe_call() {
    let content = "<?php\n$request?->string('');\n";
    let ctx = detect_at(content, "string('").expect("should detect ?->string()");
    assert_eq!(ctx.receiver, "$request");
}

#[test]
fn detects_this_receiver() {
    let content = "<?php\n$x = $this->validated('em');\n";
    let ctx = detect_at(content, "validated('em").expect("should detect $this->validated()");
    assert_eq!(ctx.receiver, "$this");
    assert_eq!(ctx.prefix, "em");
}

#[test]
fn detects_array_access() {
    let content = "<?php\n$title = $request['ti'];\n";
    let ctx = detect_at(content, "$request['ti").expect("should detect array access");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "ti");
}

#[test]
fn detects_key_inside_an_array_argument() {
    let content = "<?php\n$request->only(['name', 'ag']);\n";
    let ctx = detect_at(content, "'ag").expect("should detect only([…]) key");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "ag");
}

#[test]
fn looks_through_safe_to_the_request() {
    let content = "<?php\n$data = $request->safe()->only(['na']);\n";
    let ctx = detect_at(content, "'na").expect("should detect safe()->only() key");
    assert_eq!(ctx.receiver, "$request");
}

#[test]
fn rejects_later_arguments_of_single_key_accessors() {
    let content = "<?php\n$request->input('name', 'defau');\n";
    assert!(detect_at(content, "'defau").is_none());
}

#[test]
fn accepts_later_arguments_of_variadic_accessors() {
    let content = "<?php\n$request->hasAny('name', 'em');\n";
    let ctx = detect_at(content, "'em").expect("hasAny() takes any number of field names");
    assert_eq!(ctx.receiver, "$request");
}

#[test]
fn rejects_unrelated_methods() {
    let content = "<?php\n$request->merge('na');\n";
    assert!(detect_at(content, "'na").is_none());
}

#[test]
fn rejects_static_calls() {
    let content = "<?php\nRequest::input('na');\n";
    assert!(detect_at(content, "'na").is_none());
}

#[test]
fn full_value_stops_at_the_closing_quote() {
    let content = "<?php\n$request->input('address.city');\n";
    let offset = content.find("address").unwrap() + 3;
    let ctx = detect_at_offset(content, offset).unwrap();
    assert_eq!(ctx.prefix, "add");
    assert_eq!(ctx.full_value(content), Some("address.city"));
}

/// A call broken over lines opens the field's literal on a line of its own.
#[test]
fn detects_a_call_argument_on_a_later_line() {
    let content = "<?php\n$request->input(\n    'na');\n";
    let ctx = detect_at(content, "'na").expect("should detect the wrapped input()");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "na");
}

/// An apostrophe in a comment on the key's own line is not an opening quote.
#[test]
fn detects_a_key_past_an_apostrophe_in_a_comment() {
    let content = "<?php\n$request->only([/* don't */ 'ag']);\n";
    let ctx = detect_at(content, "'ag").expect("should detect only([…]) key");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "ag");
}

/// A comment between the `[` and the key does not hide the array access either.
#[test]
fn detects_array_access_past_a_comment() {
    let content = "<?php\n$title = $request[/* the key */ 'ti'];\n";
    let ctx = detect_at(content, "'ti").expect("should detect array access");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "ti");
}

/// A comment between the receiver and the arrow does not hide it either.
#[test]
fn detects_receiver_past_a_comment_before_the_arrow() {
    let content = "<?php\n$request /* the request */ ->input('na');\n";
    let ctx = detect_at(content, "'na").expect("should detect input()");
    assert_eq!(ctx.receiver, "$request");
    assert_eq!(ctx.prefix, "na");
}

/// A comment between the `safe()` hop and the arrow that follows it does not
/// hide the request it narrows either.
#[test]
fn looks_through_safe_to_the_request_past_a_comment() {
    let content = "<?php\n$data = $request->safe() /* validated */ ->only(['na']);\n";
    let ctx = detect_at(content, "'na").expect("should detect safe()->only() key");
    assert_eq!(ctx.receiver, "$request");
}

#[test]
fn full_value_is_none_for_an_unterminated_string() {
    let content = "<?php\n$request->input('na\n";
    let ctx = detect_at(content, "'na").unwrap();
    assert_eq!(ctx.full_value(content), None);
}
