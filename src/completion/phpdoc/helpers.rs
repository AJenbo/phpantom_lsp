//! Shared helpers for PHPDoc context detection and generation.
//!
//! These functions are used by both `context.rs` and
//! `generation/parse_decl.rs` to parse PHP declarations (parameter
//! lists, keyword positions, balanced parentheses).

/// Find the position of a whole-word keyword in a declaration string.
///
/// Returns the byte offset of the first occurrence of `keyword` that is
/// not part of a larger identifier (i.e. the characters immediately
/// before and after the match are not ASCII-alphanumeric).
pub(super) fn find_keyword_pos(decl: &str, keyword: &str) -> Option<usize> {
    let lower = decl.to_lowercase();
    let mut start = 0;
    while let Some(pos) = lower[start..].find(keyword) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0 || !decl.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_pos = abs_pos + keyword.len();
        let after_ok =
            after_pos >= decl.len() || !decl.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return Some(abs_pos);
        }
        start = abs_pos + keyword.len();
    }
    None
}

/// Find the position of the matching `)` for the first `(`, handling nesting.
///
/// The input `s` is expected to start *after* the opening `(`.  Returns
/// the byte offset of the closing `)` within `s`.
pub(super) fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Split a parameter string on commas, respecting nested `()`, `<>`,
/// `[]`, and `{}` delimiters.
///
/// Returns borrowed slices into the input string.
pub(super) fn split_params(params_str: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = params_str.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'<' | b'[' | b'{' => depth += 1,
            b')' | b'>' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                result.push(&params_str[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&params_str[start..]);
    result
}
