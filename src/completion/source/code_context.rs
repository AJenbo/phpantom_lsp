//! Where an offset sits in PHP source, resolved by a single forward scan.
//!
//! Completion needs two things about the text leading up to a cursor: the
//! last byte of real code before it (so the key in `route('name', [ // note
//! (here)\n '|'` is still seen to follow a `[`), and the chain of brackets
//! left open at it (so the `[` of an argument array and the `(` of the call
//! it belongs to can be told apart from any other nesting).
//!
//! Both come out of one forward pass.  A backwards walk cannot answer
//! either: read from the right, a `//`, a `#`, or a quote is ambiguous (it
//! may open a comment, sit inside a string, or close one), so a stray
//! bracket or apostrophe inside a comment unbalances the walk.  Scanning
//! forward the lexical state is unambiguous, and tracking the open brackets
//! on the way gives the enclosing constructs directly.

use memchr::{memchr, memchr2, memmem};

/// The lexical position of an offset in PHP source.
pub(crate) struct CodeContext<'a> {
    /// The source text up to and including the last byte of code before the
    /// offset, with trailing whitespace and comments cut off.  A
    /// `ends_with` / `strip_suffix` check on it therefore sees code.
    pub(crate) code_before: &'a str,
    /// The brackets still open at the offset, outermost first, each as
    /// `(offset, byte)` where the byte is `(`, `[`, or `{`.
    pub(crate) open_brackets: Vec<(usize, u8)>,
}

impl CodeContext<'_> {
    /// The last byte of code before the offset, or `None` when nothing but
    /// whitespace and comments precedes it.
    pub(crate) fn last_code_byte(&self) -> Option<u8> {
        self.code_before.as_bytes().last().copied()
    }

    /// The offset of the innermost open bracket and of the one enclosing it,
    /// when they are `expected_inner` nested directly in `expected_outer`.
    ///
    /// `route('users.show', ['|'` asked for `[` in `(` yields the offsets of
    /// that `[` and that `(`; a key of a nested array (`['a' => ['|'`) has an
    /// `[` around it instead and yields `None`.
    pub(crate) fn nested_pair(
        &self,
        expected_inner: u8,
        expected_outer: u8,
    ) -> Option<(usize, usize)> {
        match self.open_brackets[..] {
            [.., (outer, ob), (inner, ib)] if ib == expected_inner && ob == expected_outer => {
                Some((inner, outer))
            }
            _ => None,
        }
    }
}

/// What the scan is in the middle of.  Nowdocs behave like heredocs here:
/// both are opaque bodies closed by their label.
enum State {
    /// Inline HTML, outside any `<?php` block.
    Html,
    Code,
    SingleString,
    DoubleString,
    /// `// …` or `# …`.
    LineComment,
    /// `/* … */`, docblocks included.
    BlockComment,
    Heredoc,
}

/// Resolve the lexical position of `offset` in `content`.
///
/// Returns `None` when `offset` is not in PHP code — inside a string
/// literal, a comment, a heredoc body, or inline HTML — because the bracket
/// nesting recorded for such a position says nothing about the expression
/// the cursor is in.
///
/// The scan starts in inline HTML when an opening tag appears before
/// `offset`, and in code otherwise, so a bare fragment with no `<?php` (as
/// tests and synthesised snippets use) is read as code throughout.
pub(crate) fn code_context_at(content: &str, offset: usize) -> Option<CodeContext<'_>> {
    let bytes = content.as_bytes();
    let end = offset;
    if end > bytes.len() {
        return None;
    }

    let mut state = if memmem::find(&bytes[..end], b"<?").is_some() {
        State::Html
    } else {
        State::Code
    };
    let mut open_brackets: Vec<(usize, u8)> = Vec::new();
    // One past the last byte of code seen so far.
    let mut code_end = 0usize;
    let mut heredoc_label: &[u8] = &[];
    let mut i = 0usize;

    while i < end {
        match state {
            State::Html => match memmem::find(&bytes[i..end], b"<?") {
                Some(pos) => {
                    i += pos + 2;
                    if bytes[i..end].starts_with(b"php") {
                        i += 3;
                    } else if bytes[i..end].starts_with(b"=") {
                        i += 1;
                    }
                    state = State::Code;
                }
                None => i = end,
            },
            State::LineComment => {
                // The comment ends at the newline, or earlier at a `?>`,
                // which closes the PHP block even inside a comment.
                let newline = memchr(b'\n', &bytes[i..end]);
                let line_end = newline.map_or(end, |pos| i + pos + 1);
                match memmem::find(&bytes[i..line_end], b"?>") {
                    Some(pos) => {
                        i += pos + 2;
                        state = State::Html;
                    }
                    None => {
                        i = line_end;
                        if newline.is_some() {
                            state = State::Code;
                        }
                    }
                }
            }
            State::BlockComment => match memmem::find(&bytes[i..end], b"*/") {
                Some(pos) => {
                    i += pos + 2;
                    state = State::Code;
                }
                None => i = end,
            },
            State::SingleString | State::DoubleString => {
                let quote = if matches!(state, State::SingleString) {
                    b'\''
                } else {
                    b'"'
                };
                match memchr2(quote, b'\\', &bytes[i..end]) {
                    Some(pos) if bytes[i + pos] == b'\\' => {
                        // The byte after the backslash is escaped and cannot
                        // close the literal.  Only ASCII escapes exist, so
                        // stepping over a multi-byte character is never
                        // needed (and would land mid-character).
                        i += pos + 1;
                        if i < end && bytes[i].is_ascii() {
                            i += 1;
                        }
                    }
                    Some(pos) => {
                        i += pos + 1;
                        code_end = i;
                        state = State::Code;
                    }
                    None => i = end,
                }
            }
            State::Heredoc => {
                // The body runs to a line whose first non-blank text is the
                // label, followed by anything that cannot continue an
                // identifier.
                let Some(newline) = memchr(b'\n', &bytes[i..end]) else {
                    i = end;
                    continue;
                };
                let mut j = i + newline + 1;
                while j < end && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                if bytes[j..end].starts_with(heredoc_label) {
                    let after = j + heredoc_label.len();
                    if after >= end
                        || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_')
                    {
                        code_end = after;
                        i = after;
                        state = State::Code;
                        continue;
                    }
                }
                i = j;
            }
            State::Code => {
                let byte = bytes[i];
                match byte {
                    b'/' if bytes[i + 1..end].starts_with(b"/") => {
                        state = State::LineComment;
                        i += 2;
                        continue;
                    }
                    b'/' if bytes[i + 1..end].starts_with(b"*") => {
                        state = State::BlockComment;
                        i += 2;
                        continue;
                    }
                    // `#[` opens an attribute; a bare `#` opens a comment.
                    b'#' if !bytes[i + 1..end].starts_with(b"[") => {
                        state = State::LineComment;
                        i += 1;
                        continue;
                    }
                    b'?' if bytes[i + 1..end].starts_with(b">") => {
                        state = State::Html;
                        i += 2;
                        continue;
                    }
                    b'\'' => {
                        state = State::SingleString;
                        i += 1;
                        continue;
                    }
                    b'"' => {
                        state = State::DoubleString;
                        i += 1;
                        continue;
                    }
                    b'<' if bytes[i..end].starts_with(b"<<<") => {
                        if let Some((label, body)) = heredoc_opener(bytes, i, end) {
                            heredoc_label = label;
                            state = State::Heredoc;
                            i = body;
                            continue;
                        }
                    }
                    b'(' | b'[' | b'{' => open_brackets.push((i, byte)),
                    b')' | b']' | b'}' => {
                        let opener = match byte {
                            b')' => b'(',
                            b']' => b'[',
                            _ => b'{',
                        };
                        // A closer that does not match the innermost opener
                        // belongs to an unbalanced edit; dropping it keeps
                        // the brackets that do pair up intact.
                        if open_brackets.last().is_some_and(|(_, b)| *b == opener) {
                            open_brackets.pop();
                        }
                    }
                    _ => {}
                }
                if !byte.is_ascii_whitespace() {
                    code_end = i + 1;
                }
                i += 1;
            }
        }
    }

    if !matches!(state, State::Code) {
        return None;
    }
    Some(CodeContext {
        code_before: content.get(..code_end)?,
        open_brackets,
    })
}

/// Read a `<<<LABEL` heredoc/nowdoc opener starting at `start`.
///
/// Returns the label and the offset the body scan should resume from (the
/// rest of the opener's line, which the [`State::Heredoc`] arm skips to find
/// its newline).  Returns `None` when no label follows, which means the
/// `<<<` is not an opener.
fn heredoc_opener(bytes: &[u8], start: usize, end: usize) -> Option<(&[u8], usize)> {
    let mut i = start + 3;
    while i < end && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let quoted = i < end && (bytes[i] == b'\'' || bytes[i] == b'"');
    if quoted {
        i += 1;
    }
    let label_start = i;
    while i < end && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    if i == label_start {
        return None;
    }
    let label = &bytes[label_start..i];
    if quoted && i < end && (bytes[i] == b'\'' || bytes[i] == b'"') {
        i += 1;
    }
    Some((label, i))
}

#[cfg(test)]
#[path = "code_context_tests.rs"]
mod tests;
