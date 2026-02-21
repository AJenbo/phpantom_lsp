/// Completion target extraction.
///
/// This module contains the logic for detecting the access operator (`->` or
/// `::`) before the cursor and extracting the textual subject to its left
/// (e.g. `$this`, `self`, `$var`, `$this->prop`, `ClassName`).
///
/// The low-level subject extraction helpers (walking backwards through
/// characters to find variables, call expressions, `::` subjects, etc.)
/// live in the shared [`crate::subject_extraction`] module so they can be
/// reused by the definition resolver and future features (hover,
/// references).
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::subject_extraction::{
    collapse_continuation_lines, extract_arrow_subject, extract_double_colon_subject,
};
use crate::types::*;

impl Backend {
    /// Detect the access operator before the cursor position and extract
    /// both the `AccessKind` and the textual subject to its left.
    ///
    /// Returns `None` when no `->` or `::` is found (i.e. `AccessKind::Other`).
    pub fn extract_completion_target(
        content: &str,
        position: Position,
    ) -> Option<CompletionTarget> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        // Collapse multi-line method chains so that continuation lines
        // (starting with `->` or `?->`) are joined with preceding lines.
        let (line, col) = collapse_continuation_lines(
            &lines,
            position.line as usize,
            position.character as usize,
        );
        let chars: Vec<char> = line.chars().collect();
        let col = col.min(chars.len());

        // Walk backwards past any partial identifier the user may have typed
        let mut i = col;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        // Detect operator
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let subject = extract_arrow_subject(&chars, i - 2);
            Some(CompletionTarget {
                access_kind: AccessKind::Arrow,
                subject,
            })
        } else if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            let subject = extract_double_colon_subject(&chars, i - 2);
            Some(CompletionTarget {
                access_kind: AccessKind::DoubleColon,
                subject,
            })
        } else {
            None
        }
    }
}
