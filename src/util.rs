/// Utility functions for the PHPantomLSP server.
///
/// This module contains helper methods for position/offset conversion,
/// class lookup by offset, logging, and cross-file class resolution.
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::types::ClassInfo;

impl Backend {
    /// Convert an LSP Position (line, character) to a byte offset in content.
    pub(crate) fn position_to_offset(content: &str, position: Position) -> Option<u32> {
        let mut offset: u32 = 0;
        for (i, line) in content.lines().enumerate() {
            if i == position.line as usize {
                let char_offset = position.character as usize;
                // Convert character offset (UTF-16 code units in LSP) to byte offset.
                // For simplicity, treat characters as single-byte (ASCII).
                // This is sufficient for most PHP code.
                let byte_col = line
                    .char_indices()
                    .nth(char_offset)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
                return Some(offset + byte_col as u32);
            }
            // +1 for the newline character
            offset += line.len() as u32 + 1;
        }
        // If the position is past the last line, return end of content
        Some(content.len() as u32)
    }

    /// Find which class the cursor (byte offset) is inside.
    pub(crate) fn find_class_at_offset(classes: &[ClassInfo], offset: u32) -> Option<&ClassInfo> {
        classes
            .iter()
            .find(|c| offset >= c.start_offset && offset <= c.end_offset)
    }

    /// Public helper for tests: get the ast_map for a given URI.
    pub fn get_classes_for_uri(&self, uri: &str) -> Option<Vec<ClassInfo>> {
        if let Ok(map) = self.ast_map.lock() {
            map.get(uri).cloned()
        } else {
            None
        }
    }

    pub(crate) async fn log(&self, typ: MessageType, message: String) {
        if let Some(client) = &self.client {
            client.log_message(typ, message).await;
        }
    }

    /// Try to find a class by name across all cached files in the ast_map,
    /// and if not found, attempt PSR-4 resolution to load the class from disk.
    ///
    /// The `class_name` can be:
    ///   - A simple name like `"Customer"`
    ///   - A namespace-qualified name like `"Klarna\\Customer"`
    ///   - A fully-qualified name like `"\\Klarna\\Customer"` (leading `\` is stripped)
    ///
    /// Returns a cloned `ClassInfo` if found, or `None`.
    pub(crate) fn find_or_load_class(&self, class_name: &str) -> Option<ClassInfo> {
        // Normalise: strip leading `\`
        let name = class_name.strip_prefix('\\').unwrap_or(class_name);

        // The class name stored in ClassInfo is just the short name (e.g. "Customer"),
        // so we match against the last segment of the namespace-qualified name.
        let last_segment = name.rsplit('\\').next().unwrap_or(name);

        // ── Phase 1: Search all already-parsed files in the ast_map ──
        if let Ok(map) = self.ast_map.lock() {
            for classes in map.values() {
                if let Some(cls) = classes.iter().find(|c| c.name == last_segment) {
                    return Some(cls.clone());
                }
            }
        }

        // ── Phase 2: Try PSR-4 resolution ──
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())?;

        let mappings = self.psr4_mappings.lock().ok()?;

        let file_path = composer::resolve_class_path(&mappings, &workspace_root, name)?;

        // Read and parse the file
        let content = std::fs::read_to_string(&file_path).ok()?;
        let classes = self.parse_php(&content);

        // Find the target class in the parsed results
        let result = classes.iter().find(|c| c.name == last_segment).cloned();

        // Cache the parsed file in the ast_map so we don't re-parse next time.
        // Use a file:// URI as the key (consistent with LSP document URIs).
        let uri = format!("file://{}", file_path.display());
        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri, classes);
        }

        result
    }
}
