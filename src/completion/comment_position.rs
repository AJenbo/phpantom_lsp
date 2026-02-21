//! Comment and docblock position detection.
//!
//! This module provides utilities to determine whether a given cursor
//! position falls inside a comment or docblock.  These are used early in
//! the completion pipeline to decide whether to suppress normal
//! completions (inside `//` / `/* */`) or to switch to PHPDoc tag
//! completion (inside `/** */`).
//!
//! The functions here are pure — they take `(content, Position)` and
//! return a result without any shared state.

use tower_lsp::lsp_types::Position;

// Re-export the canonical position-to-byte-offset helper so that existing
// `use super::comment_position::position_to_byte_offset` imports continue
// to work without modification.
pub(crate) use crate::util::position_to_byte_offset;

/// Returns `true` if the given position is inside a `/** … */` docblock.
///
/// Scans backwards from the cursor to find the nearest `/**` that has not
/// been closed by a matching `*/` before the cursor position.
pub fn is_inside_docblock(content: &str, position: Position) -> bool {
    // Convert position to byte offset for easier scanning
    let byte_offset = position_to_byte_offset(content, position);

    let before_cursor = &content[..byte_offset.min(content.len())];

    // Find the last `/**` before the cursor
    let last_open = before_cursor.rfind("/**");
    if last_open.is_none() {
        return false;
    }
    let open_pos = last_open.unwrap();

    // Check if there is a `*/` between the `/**` and the cursor
    // (which would mean the docblock is closed)
    let after_open = &before_cursor[open_pos + 3..];
    !after_open.contains("*/")
}

/// Returns `true` if the given position is inside a `//` line comment or
/// a `/* … */` block comment that is **not** a `/** … */` docblock.
///
/// Uses a forward state-machine scan from the start of the file to
/// correctly handle comments inside string literals (which are ignored).
pub fn is_inside_non_doc_comment(content: &str, position: Position) -> bool {
    let target = position_to_byte_offset(content, position);
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Scanner states
    #[derive(PartialEq)]
    enum State {
        Code,
        SingleString,
        DoubleString,
        LineComment,
        BlockComment,
        Docblock,
        Heredoc,
    }

    let mut state = State::Code;
    // For heredoc/nowdoc we track the closing label
    let mut heredoc_label: Vec<u8> = Vec::new();

    while i < len {
        if i >= target {
            return state == State::LineComment || state == State::BlockComment;
        }

        match state {
            State::Code => {
                if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
                    state = State::LineComment;
                    i += 2;
                } else if bytes[i] == b'/'
                    && i + 2 < len
                    && bytes[i + 1] == b'*'
                    && bytes[i + 2] == b'*'
                    && (i + 3 >= len || bytes[i + 3] != b'*')
                {
                    // `/**` but not `/***` — that's a docblock
                    state = State::Docblock;
                    i += 3;
                } else if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
                    state = State::BlockComment;
                    i += 2;
                } else if bytes[i] == b'\'' {
                    state = State::SingleString;
                    i += 1;
                } else if bytes[i] == b'"' {
                    state = State::DoubleString;
                    i += 1;
                } else if bytes[i] == b'<'
                    && i + 2 < len
                    && bytes[i + 1] == b'<'
                    && bytes[i + 2] == b'<'
                {
                    // Possible heredoc / nowdoc
                    i += 3;
                    // Skip optional whitespace
                    while i < len && bytes[i] == b' ' {
                        i += 1;
                    }
                    let is_nowdoc = i < len && bytes[i] == b'\'';
                    if is_nowdoc {
                        i += 1; // skip opening quote
                    }
                    heredoc_label.clear();
                    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        heredoc_label.push(bytes[i]);
                        i += 1;
                    }
                    if !heredoc_label.is_empty() {
                        if is_nowdoc && i < len && bytes[i] == b'\'' {
                            i += 1; // skip closing quote
                        }
                        state = State::Heredoc;
                    }
                } else {
                    i += 1;
                }
            }
            State::LineComment => {
                if bytes[i] == b'\n' {
                    state = State::Code;
                }
                i += 1;
            }
            State::BlockComment => {
                if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    state = State::Code;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            State::Docblock => {
                if bytes[i] == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    state = State::Code;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            State::SingleString => {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2; // skip escaped char
                } else if bytes[i] == b'\'' {
                    state = State::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            State::DoubleString => {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2; // skip escaped char
                } else if bytes[i] == b'"' {
                    state = State::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            State::Heredoc => {
                // Look for the closing label at the start of a line
                if bytes[i] == b'\n' {
                    i += 1;
                    // Skip optional whitespace before the label
                    let line_start = i;
                    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                        i += 1;
                    }
                    if i + heredoc_label.len() <= len
                        && &bytes[i..i + heredoc_label.len()] == heredoc_label.as_slice()
                    {
                        let after_label = i + heredoc_label.len();
                        if after_label >= len
                            || bytes[after_label] == b';'
                            || bytes[after_label] == b'\n'
                            || bytes[after_label] == b'\r'
                        {
                            i = after_label;
                            state = State::Code;
                            continue;
                        }
                    }
                    // Not the closing label — rewind to just after the newline
                    // to avoid skipping content, but we already advanced past
                    // whitespace which is fine (it's part of the heredoc body).
                    let _ = line_start; // consumed above
                } else {
                    i += 1;
                }
            }
        }
    }

    // Cursor is at or past end of file
    state == State::LineComment || state == State::BlockComment
}
