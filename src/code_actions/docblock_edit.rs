//! Shared docblock-location helpers for code actions.
//!
//! Several PHPStan quickfixes need to locate the `/** … */` block that sits
//! directly above a function, method, or property signature so they can
//! rewrite or remove tags inside it.  This module owns the single
//! implementation they share.

/// Information about a docblock found above a given line.
pub(crate) struct DocblockAbove {
    /// Byte offset of the start of the docblock (first char of the `/**`
    /// line, including leading whitespace).
    pub(crate) start: usize,
    /// Byte offset just past the end of the docblock (past the `*/` line,
    /// including its trailing newline).
    pub(crate) end: usize,
    /// The raw docblock text including indentation.
    pub(crate) text: String,
}

/// Find the docblock immediately above the given line.
///
/// The diagnostic line is the function/method/property signature.  The
/// docblock (if any) sits directly above it, possibly separated by blank
/// lines or attribute (`#[…]`) lines.
pub(crate) fn find_docblock_above_line(content: &str, line: usize) -> Option<DocblockAbove> {
    let lines: Vec<&str> = content.lines().collect();
    if line == 0 || line > lines.len() {
        return None;
    }

    // Walk backward from the line before the diagnostic to find `*/`.
    let mut doc_end_line = None;
    for i in (0..line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.ends_with("*/") {
            doc_end_line = Some(i);
            break;
        }
        // Attributes (#[...]) can appear between docblock and declaration.
        if trimmed.starts_with("#[") {
            continue;
        }
        // Anything else means no docblock.
        break;
    }

    let end_line = doc_end_line?;

    // Walk backward from end_line to find `/**`.
    let mut doc_start_line = None;
    for i in (0..=end_line).rev() {
        let trimmed = lines[i].trim();
        if trimmed.contains("/**") {
            doc_start_line = Some(i);
            break;
        }
        // Should be a `*`-prefixed line or end-of-docblock.
        if !trimmed.starts_with('*') && !trimmed.ends_with("*/") {
            break;
        }
    }

    let start_line = doc_start_line?;

    // Convert line numbers to byte offsets.
    let mut byte_offset = 0;
    let mut start_byte = 0;
    let mut end_byte = 0;
    for (i, line_text) in lines.iter().enumerate() {
        if i == start_line {
            start_byte = byte_offset;
        }
        byte_offset += line_text.len() + 1; // +1 for newline
        if i == end_line {
            end_byte = byte_offset; // include trailing newline
        }
    }

    let text = content
        .get(start_byte..end_byte.min(content.len()))
        .unwrap_or("")
        .to_string();

    Some(DocblockAbove {
        start: start_byte,
        end: end_byte.min(content.len()),
        text,
    })
}
