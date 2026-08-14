use super::{
    PREG_OFFSET_CAPTURE, PREG_SET_ORDER, PREG_UNMATCHED_AS_NULL, capture_groups, matches_type,
    opaque_matches_type, split_delimiters,
};

/// The group list as `name?:optional` pairs, for compact assertions.
fn groups(pattern: &str) -> Option<Vec<(Option<String>, bool)>> {
    Some(
        capture_groups(pattern)?
            .into_iter()
            .map(|g| (g.name, g.optional))
            .collect(),
    )
}

fn shape(pattern: &str) -> String {
    matches_type(pattern, 0, false)
        .map(|t| t.to_string())
        .unwrap_or_else(|| "<none>".to_string())
}

fn shape_all(pattern: &str, flags: i64) -> String {
    matches_type(pattern, flags, true)
        .map(|t| t.to_string())
        .unwrap_or_else(|| "<none>".to_string())
}

#[test]
fn splits_symmetric_delimiters() {
    assert_eq!(split_delimiters("/foo/i"), Some(("foo", "i")));
    assert_eq!(split_delimiters("#foo#"), Some(("foo", "")));
    assert_eq!(split_delimiters("~a/b~u"), Some(("a/b", "u")));
    // Leading whitespace and spaced-out modifiers are both allowed.
    assert_eq!(split_delimiters("  /foo/ i u"), Some(("foo", " i u")));
    // The body ends at the first *unescaped* delimiter.
    assert_eq!(split_delimiters(r"_foo(.)\_i_i"), Some((r"foo(.)\_i", "i")));
}

#[test]
fn splits_bracket_delimiters() {
    assert_eq!(split_delimiters("(foo)i"), Some(("foo", "i")));
    assert_eq!(split_delimiters("(a(b)c)"), Some(("a(b)c", "")));
    assert_eq!(split_delimiters("{a}"), Some(("a", "")));
    assert_eq!(split_delimiters("<a>"), Some(("a", "")));
}

#[test]
fn rejects_malformed_patterns() {
    assert_eq!(split_delimiters(""), None);
    assert_eq!(split_delimiters("/unterminated"), None);
    // An alphanumeric or backslash delimiter is not allowed.
    assert_eq!(split_delimiters("afooa"), None);
    assert_eq!(split_delimiters(r"\foo\"), None);
}

#[test]
fn counts_plain_groups() {
    assert_eq!(groups("/(a)(b)/"), Some(vec![(None, false), (None, false)]));
    // Non-capturing, lookahead, lookbehind and atomic groups do not count.
    assert_eq!(groups("/(?:a)(?=b)(?<=c)(?>d)/"), Some(vec![]));
    assert_eq!(groups("/Price: (?:£|€)(\\d+)/"), Some(vec![(None, false)]));
}

#[test]
fn ignores_parens_that_are_not_groups() {
    assert_eq!(groups(r"/\(a\)/"), Some(vec![]));
    assert_eq!(groups("/[(]/"), Some(vec![]));
    assert_eq!(groups(r"/[\](]/"), Some(vec![]));
    assert_eq!(groups("/[])(]/"), Some(vec![]));
    assert_eq!(groups("/[[:alpha:](]/"), Some(vec![]));
    assert_eq!(groups("/(?#a comment with a ( in it)/"), Some(vec![]));
}

#[test]
fn reads_named_groups() {
    let expected = Some(vec![(Some("num".to_string()), false)]);
    assert_eq!(groups("/(?<num>\\d+)/"), expected);
    assert_eq!(groups("/(?P<num>\\d+)/"), expected);
    assert_eq!(groups("/(?'num'\\d+)/"), expected);
}

#[test]
fn zero_allowing_quantifiers_make_a_group_optional() {
    assert_eq!(groups("/(a)?/"), Some(vec![(None, true)]));
    assert_eq!(groups("/(a)*/"), Some(vec![(None, true)]));
    assert_eq!(groups("/(a){0,3}/"), Some(vec![(None, true)]));
    assert_eq!(groups("/(a){,3}/"), Some(vec![(None, true)]));
    assert_eq!(groups("/(a)*?/"), Some(vec![(None, true)]));
    assert_eq!(groups("/(a)?+/"), Some(vec![(None, true)]));
    // A quantifier that demands at least one repetition does not.
    assert_eq!(groups("/(a)+/"), Some(vec![(None, false)]));
    assert_eq!(groups("/(a){2,3}/"), Some(vec![(None, false)]));
    assert_eq!(groups("/(a){2}/"), Some(vec![(None, false)]));
    // A `{` that does not spell a quantifier is a literal.
    assert_eq!(groups("/(a){x}/"), Some(vec![(None, false)]));
}

#[test]
fn optionality_propagates_from_enclosing_groups() {
    assert_eq!(
        groups("/(?:(a)(b))?/"),
        Some(vec![(None, true), (None, true)])
    );
    assert_eq!(
        groups("/((a))?/"),
        Some(vec![(None, true), (None, true)]),
        "the outer group's quantifier reaches the nested one"
    );
    // A negative lookaround's groups cannot have participated in a match.
    assert_eq!(groups("/x(?!(a))/"), Some(vec![(None, true)]));
    assert_eq!(groups("/(?<!(a))x/"), Some(vec![(None, true)]));
}

#[test]
fn alternation_makes_the_branches_optional() {
    assert_eq!(groups("/(a)|(b)/"), Some(vec![(None, true), (None, true)]));
    assert_eq!(
        groups("/(?:(a)|(b))/"),
        Some(vec![(None, true), (None, true)])
    );
    assert_eq!(
        groups("/((a)|(b))/"),
        Some(vec![(None, false), (None, true), (None, true)]),
        "the group holding the alternation still matches"
    );
    assert_eq!(
        groups("/Price: (£|€)/"),
        Some(vec![(None, false)]),
        "an alternation of literals says nothing about the group's presence"
    );
}

#[test]
fn refuses_constructs_it_cannot_model() {
    // Branch reset restarts the group counter.
    assert_eq!(groups("/(?|(a)|(b))/"), None);
    // Conditional groups, recursion and subroutine calls.
    assert_eq!(groups("/(a)(?(1)b|c)/"), None);
    assert_eq!(groups("/(a(?R))/"), None);
    assert_eq!(groups("/(a)(?1)/"), None);
    assert_eq!(groups("/(a)(?&name)/"), None);
    // Control verbs, one of which adds a `MARK` key.
    assert_eq!(groups("/(a)(*MARK:x)/"), None);
    // `\Q…\E` makes a `(` literal.
    assert_eq!(groups(r"/\Q(a)\E/"), None);
    // Extended mode changes what counts as whitespace and a comment.
    assert_eq!(groups("/(a) # comment/x"), None);
    assert_eq!(groups("/(?x)(a) # comment/"), None);
    // Unbalanced parens.
    assert_eq!(groups("/(a/"), None);
    assert_eq!(groups("/a)/"), None);
    // An unknown modifier means the pattern is not one PHP would accept.
    assert_eq!(groups("/(a)/z"), None);
}

#[test]
fn inline_modifiers_are_transparent() {
    assert_eq!(groups("/(?i)(a)/"), Some(vec![(None, false)]));
    assert_eq!(groups("/(?i:(a))/"), Some(vec![(None, false)]));
    assert_eq!(groups("/(?i-m:(a))?/"), Some(vec![(None, true)]));
}

#[test]
fn no_auto_capture_modifier_leaves_only_named_groups() {
    assert_eq!(
        groups("/(a)(?<name>b)/n"),
        Some(vec![(Some("name".to_string()), false)])
    );
}

/// The `J` modifier lets two groups share a name, and only one of them can
/// hold the key.
#[test]
fn a_repeated_group_name_abandons_the_analysis() {
    assert_eq!(groups("/(?<n>a)|(?<n>b)/J"), None);
}

#[test]
fn shapes_the_whole_match_and_every_group() {
    assert_eq!(shape("/Price: /i"), "array{0: string}");
    assert_eq!(shape("/(a)(b)/"), "array{0: string, 1: string, 2: string}");
    assert_eq!(
        shape("/\\w-(?P<num>\\d+)-(\\w)/"),
        "array{0: string, num: string, 1: string, 2: string}"
    );
}

#[test]
fn only_trailing_optional_groups_get_optional_keys() {
    // PHP reports an unmatched group followed by a matched one as `''`, and
    // drops only the trailing ones.
    assert_eq!(
        shape("/(a)(b)*(c)(d)*/"),
        "array{0: string, 1: string, 2: string, 3: string, 4?: string}"
    );
    assert_eq!(
        shape("/(a)(b)*(c)(?<name>d)*/"),
        "array{0: string, 1: string, 2: string, 3: string, name?: string, 4?: string}"
    );
}

#[test]
fn unmatched_as_null_reports_every_group() {
    assert_eq!(
        matches_type("/(a)(b)?/", PREG_UNMATCHED_AS_NULL, false)
            .unwrap()
            .to_string(),
        "array{0: string, 1: string, 2: ?string}"
    );
}

#[test]
fn offset_capture_pairs_each_entry_with_its_offset() {
    assert_eq!(
        matches_type("/(a)/", PREG_OFFSET_CAPTURE, false)
            .unwrap()
            .to_string(),
        "array{0: array{string, int<-1, max>}, 1: array{string, int<-1, max>}}"
    );
}

#[test]
fn match_all_collects_each_group_in_pattern_order() {
    assert_eq!(
        shape_all("/(a)(b)?/", 0),
        "array{0: list<string>, 1: list<string>, 2: list<string>}"
    );
}

#[test]
fn match_all_in_set_order_is_a_list_of_match_shapes() {
    assert_eq!(
        shape_all("/(a)(b)?/", PREG_SET_ORDER),
        "list<array{0: string, 1: string, 2?: string}>"
    );
}

#[test]
fn unmodelled_flags_abandon_the_analysis() {
    assert_eq!(matches_type("/(a)/", 4096, false), None);
    assert_eq!(opaque_matches_type(4096, false), None);
}

#[test]
fn an_unanalysable_pattern_still_types_the_entries() {
    assert_eq!(
        opaque_matches_type(0, false).unwrap().to_string(),
        "array<array-key, string>"
    );
    assert_eq!(
        opaque_matches_type(0, true).unwrap().to_string(),
        "array<array-key, list<string>>"
    );
    assert_eq!(
        opaque_matches_type(PREG_SET_ORDER, true)
            .unwrap()
            .to_string(),
        "list<array<array-key, string>>"
    );
}
