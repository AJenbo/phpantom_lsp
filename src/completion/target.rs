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
use crate::subject_extraction::{extract_arrow_subject, extract_double_colon_subject};
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

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

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
