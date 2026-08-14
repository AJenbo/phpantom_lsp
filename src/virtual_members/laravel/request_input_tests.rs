use super::*;

// ── input_accessor ──────────────────────────────────────────────────

#[test]
fn the_input_accessors_are_classified_by_their_source() {
    assert_eq!(input_accessor("header"), Some(InputAccessor::Header));
    assert_eq!(input_accessor("query"), Some(InputAccessor::Bag));
    assert_eq!(input_accessor("post"), Some(InputAccessor::Bag));
    assert_eq!(input_accessor("cookie"), Some(InputAccessor::Bag));
    assert_eq!(input_accessor("input"), Some(InputAccessor::Input));
    assert_eq!(input_accessor("file"), Some(InputAccessor::File));
}

#[test]
fn a_method_that_reads_no_input_is_not_an_accessor() {
    for name in ["all", "validated", "user", "hasFile", "string"] {
        assert_eq!(input_accessor(name), None, "{name} is not an input source");
    }
}

/// PHP method names are case-insensitive, so a call written `Header()`
/// reaches the same method.
#[test]
fn accessor_names_are_matched_case_insensitively() {
    assert_eq!(input_accessor("HEADER"), Some(InputAccessor::Header));
}

// ── whole_source_type ───────────────────────────────────────────────

#[test]
fn the_header_bag_holds_every_value_a_header_was_sent_with() {
    assert_eq!(
        whole_source_type(InputAccessor::Header).to_string(),
        "array<string, list<string|null>>"
    );
}

#[test]
fn the_file_bag_holds_one_upload_or_a_list_per_field() {
    assert_eq!(
        whole_source_type(InputAccessor::File).to_string(),
        "array<string, Illuminate\\Http\\UploadedFile|list<Illuminate\\Http\\UploadedFile>>"
    );
}

#[test]
fn an_input_bag_holds_whatever_the_request_carried() {
    assert_eq!(
        whole_source_type(InputAccessor::Bag).to_string(),
        "array<string, mixed>"
    );
}

// ── mentions_upload ─────────────────────────────────────────────────

#[test]
fn a_rules_type_naming_an_upload_is_recognised_through_a_list() {
    assert!(mentions_upload(&PhpType::parse(
        "list<\\Illuminate\\Http\\UploadedFile>"
    )));
    assert!(mentions_upload(&PhpType::parse(
        "Illuminate\\Http\\UploadedFile"
    )));
}

/// A key the rules describe as something else says nothing about what
/// `file()` returns for it.
#[test]
fn a_rules_type_naming_no_upload_is_not_one() {
    for ty in ["string", "list<string>", "int"] {
        assert!(!mentions_upload(&PhpType::parse(ty)), "{ty} is not a file");
    }
}

// ── is_null_literal ─────────────────────────────────────────────────

#[test]
fn a_null_key_is_the_keyless_form_written_out() {
    assert!(is_null_literal("null"));
    assert!(is_null_literal("  NULL "));
    assert!(!is_null_literal("$key"));
    assert!(!is_null_literal("'null'"));
}
