/// Goto-definition resolution.
///
/// Given a cursor position in a PHP file this module:
///   1. Extracts the symbol (class / interface / trait / enum name) under the cursor.
///   2. Resolves it to a fully-qualified name using the file's `use` map and namespace.
///   3. Locates the file on disk via PSR-4 mappings.
///   4. Finds the exact line of the symbol's declaration inside that file.
///   5. Returns an LSP `Location` the editor can jump to.
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;

impl Backend {
    /// Handle a "go to definition" request.
    ///
    /// Returns `Some(Location)` when the symbol under the cursor can be
    /// resolved to a file and a position inside that file, or `None` when
    /// resolution fails at any step.
    pub(crate) fn resolve_definition(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<Location> {
        // 1. Extract the symbol name under the cursor.
        let word = Self::extract_word_at_position(content, position)?;

        if word.is_empty() {
            return None;
        }

        // 2. Gather context from the current file (use map + namespace).
        let file_use_map = self
            .use_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())
            .flatten();

        // 3. Resolve to a fully-qualified name.
        let fqn = Self::resolve_to_fqn(&word, &file_use_map, &file_namespace);

        // Build a list of FQN candidates to try.  The resolved name is tried
        // first, but when the original word already contains `\` (e.g. from a
        // `use` statement where the name is already fully-qualified) we also
        // try the raw word so we don't fail just because namespace-prefixing
        // produced a wrong result.
        let mut candidates = vec![fqn];
        if word.contains('\\') && !candidates.contains(&word) {
            candidates.push(word.clone());
        }

        // 4. Try to find the class in the current file first (same-file jump).
        for fqn in &candidates {
            if let Some(location) = self.find_definition_in_ast_map(fqn, content, uri) {
                return Some(location);
            }
        }

        // 5. Resolve file path via PSR-4.
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())?;

        let mappings = self.psr4_mappings.lock().ok()?;

        for fqn in &candidates {
            if let Some(file_path) = composer::resolve_class_path(&mappings, &workspace_root, fqn) {
                // 6. Read the target file and find the definition line.
                if let Some(target_content) = std::fs::read_to_string(&file_path).ok() {
                    let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);
                    if let Some(target_position) =
                        Self::find_definition_position(&target_content, short_name)
                    {
                        if let Some(target_uri) = Url::from_file_path(&file_path).ok() {
                            return Some(Location {
                                uri: target_uri,
                                range: Range {
                                    start: target_position,
                                    end: target_position,
                                },
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// Extract the symbol name (class / interface / trait / enum / namespace)
    /// at the given cursor position.
    ///
    /// The word is defined as a contiguous run of alphanumeric characters,
    /// underscores, and backslashes (to capture fully-qualified names).
    pub fn extract_word_at_position(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return None;
        }

        let line = lines[line_idx];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Nothing to do on an empty line or if cursor is at position 0
        // with no word character.
        if chars.is_empty() {
            return None;
        }

        // If the cursor is right after a word (col points at a non-word char
        // or end-of-line), we still want to resolve the word to its left.
        // But if the cursor is in the middle of a word, expand in both
        // directions.

        let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '\\';

        // Find the start of the word: walk left from cursor.
        let mut start = col;

        // If cursor is between two chars and the right one is a word char,
        // start there.  Otherwise start from the char to the left.
        if start < chars.len() && is_word_char(chars[start]) {
            // cursor is on a word char — expand left
        } else if start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        } else {
            return None;
        }

        // Walk left to find start of word
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }

        // Walk right to find end of word
        let mut end = col;
        if end < chars.len() && is_word_char(chars[end]) {
            // cursor is on a word char — also expand right
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        } else {
            // Cursor was past the word — expand right from start
            end = start;
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        }

        if start == end {
            return None;
        }

        let word: String = chars[start..end].iter().collect();

        // Strip a leading `\` (PHP fully-qualified prefix).
        let word = word.strip_prefix('\\').unwrap_or(&word).to_string();

        // Strip trailing `\` if any (partial namespace).
        let word = word.strip_suffix('\\').unwrap_or(&word).to_string();

        if word.is_empty() {
            return None;
        }

        Some(word)
    }

    /// Resolve a short or partially-qualified name to a fully-qualified name
    /// using the file's `use` map and namespace context.
    ///
    /// Rules:
    ///   - If the name contains `\` it is already (partially) qualified.
    ///     Check if the first segment is in the use_map; if so, expand it.
    ///     Otherwise prefix with the current namespace.
    ///   - If the name is unqualified (no `\`):
    ///     1. Check the use_map for a direct mapping.
    ///     2. Prefix with the current namespace.
    ///     3. Fall back to the bare name (global namespace).
    pub fn resolve_to_fqn(
        name: &str,
        use_map: &std::collections::HashMap<String, String>,
        namespace: &Option<String>,
    ) -> String {
        // Already fully-qualified (leading `\` was stripped earlier).
        // If name contains `\`, check if the first segment is aliased.
        if name.contains('\\') {
            let first_segment = name.split('\\').next().unwrap_or(name);
            if let Some(fqn_prefix) = use_map.get(first_segment) {
                // Replace the first segment with the FQN prefix.
                let rest = &name[first_segment.len()..];
                return format!("{}{}", fqn_prefix, rest);
            }
            // Not in use map — might already be fully-qualified, or
            // needs current namespace prepended.
            if let Some(ns) = namespace {
                return format!("{}\\{}", ns, name);
            }
            return name.to_string();
        }

        // Unqualified name — try use_map first.
        if let Some(fqn) = use_map.get(name) {
            return fqn.clone();
        }

        // Try current namespace.
        if let Some(ns) = namespace {
            return format!("{}\\{}", ns, name);
        }

        // Fall back to global / bare name.
        name.to_string()
    }

    /// Try to find the definition of a class in the current file by checking
    /// the ast_map.
    fn find_definition_in_ast_map(&self, fqn: &str, content: &str, uri: &str) -> Option<Location> {
        let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())?;

        let _class_info = classes.iter().find(|c| c.name == short_name)?;

        // Convert start_offset to a position.  start_offset is the opening
        // brace — scan backwards to find the class/interface keyword line.
        let position = Self::find_definition_position(content, short_name)?;

        // Build a file URI from the current URI string.
        let parsed_uri = Url::parse(uri).ok()?;

        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }

    /// Find the position (line, character) of a class / interface / trait / enum
    /// declaration inside the given file content.
    ///
    /// Searches for patterns like:
    ///   `class ClassName`
    ///   `interface ClassName`
    ///   `trait ClassName`
    ///   `enum ClassName`
    ///   `abstract class ClassName`
    ///   `final class ClassName`
    ///   `readonly class ClassName`
    ///
    /// Returns the position of the keyword (`class`, `interface`, etc.) on
    /// the matching line.
    pub fn find_definition_position(content: &str, class_name: &str) -> Option<Position> {
        let keywords = ["class", "interface", "trait", "enum"];

        for (line_idx, line) in content.lines().enumerate() {
            for keyword in &keywords {
                // Search for `keyword ClassName` making sure ClassName is
                // followed by a word boundary (whitespace, `{`, `:`, end of
                // line) so we don't match partial names.
                let pattern = format!("{} {}", keyword, class_name);
                if let Some(col) = line.find(&pattern) {
                    // Verify word boundary before the keyword: either start
                    // of line or preceded by whitespace / non-alphanumeric.
                    let before_ok = col == 0 || {
                        let prev = line.as_bytes().get(col - 1).copied().unwrap_or(b' ');
                        !(prev as char).is_alphanumeric() && prev != b'_'
                    };

                    // Verify word boundary after the class name.
                    let after_pos = col + pattern.len();
                    let after_ok = after_pos >= line.len() || {
                        let next = line.as_bytes().get(after_pos).copied().unwrap_or(b' ');
                        !(next as char).is_alphanumeric() && next != b'_'
                    };

                    if before_ok && after_ok {
                        return Some(Position {
                            line: line_idx as u32,
                            character: col as u32,
                        });
                    }
                }
            }
        }

        None
    }
}
