/// Utility functions for the PHPantomLSP server.
///
/// This module contains helper methods for position/offset conversion,
/// class lookup by offset, logging, cross-file class resolution, and
/// shared text-processing helpers used by multiple modules.
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::types::{ClassInfo, FunctionInfo};

/// Skip backwards past a balanced parenthesised group `(…)` in a char slice.
///
/// `pos` must point one past the closing `)`.  Returns the index of the
/// opening `(`, or `None` if parens are unbalanced.
///
/// This is a standalone function (not on `impl Backend`) so it can be
/// shared by both the completion target extractor and the definition
/// resolver without creating duplicate method definitions.
pub(crate) fn skip_balanced_parens_back(chars: &[char], pos: usize) -> Option<usize> {
    if pos == 0 || chars[pos - 1] != ')' {
        return None;
    }
    let mut depth: u32 = 0;
    let mut j = pos;
    while j > 0 {
        j -= 1;
        match chars[j] {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// Check if the `new` keyword (followed by whitespace) appears immediately
/// before the identifier starting at position `ident_start`.
///
/// Returns the class name (possibly with namespace) if `new` is found.
pub(crate) fn check_new_keyword_before(
    chars: &[char],
    ident_start: usize,
    class_name: &str,
) -> Option<String> {
    let mut j = ident_start;
    // Skip whitespace between `new` and the class name.
    while j > 0 && chars[j - 1] == ' ' {
        j -= 1;
    }
    // Check for the `new` keyword.
    if j >= 3 && chars[j - 3] == 'n' && chars[j - 2] == 'e' && chars[j - 1] == 'w' {
        // Verify word boundary before `new` (start of line, whitespace, `(`, etc.).
        let before_ok = j == 3 || {
            let prev = chars[j - 4];
            !prev.is_alphanumeric() && prev != '_'
        };
        if before_ok {
            // Strip leading `\` from FQN if present.
            let name = class_name.strip_prefix('\\').unwrap_or(class_name);
            return Some(name.to_string());
        }
    }
    None
}

/// Try to extract a class name from a parenthesized `new` expression:
/// `(new ClassName(...))`.
///
/// `open` is the position of the outer `(`, `close` is one past the
/// outer `)`.  The function looks inside for the pattern
/// `new ClassName(...)`.
pub(crate) fn extract_new_expression_inside_parens(
    chars: &[char],
    open: usize,
    close: usize,
) -> Option<String> {
    // Content is chars[open+1 .. close-1].
    let inner_start = open + 1;
    let inner_end = close - 1;
    if inner_start >= inner_end {
        return None;
    }

    // Skip whitespace inside the opening `(`.
    let mut k = inner_start;
    while k < inner_end && chars[k] == ' ' {
        k += 1;
    }

    // Check for `new` keyword.
    if k + 3 >= inner_end {
        return None;
    }
    if chars[k] != 'n' || chars[k + 1] != 'e' || chars[k + 2] != 'w' {
        return None;
    }
    k += 3;

    // Must be followed by whitespace.
    if k >= inner_end || chars[k] != ' ' {
        return None;
    }
    while k < inner_end && chars[k] == ' ' {
        k += 1;
    }

    // Read the class name (may include `\` for namespaces).
    let name_start = k;
    while k < inner_end && (chars[k].is_alphanumeric() || chars[k] == '_' || chars[k] == '\\') {
        k += 1;
    }
    if k == name_start {
        return None;
    }
    let class_name: String = chars[name_start..k].iter().collect();
    let name = class_name.strip_prefix('\\').unwrap_or(&class_name);
    Some(name.to_string())
}

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

        // ── Phase 0: Try the class_index for a direct FQN → URI lookup ──
        // This handles classes that don't follow PSR-4 conventions, such as
        // classes defined in Composer autoload_files.php entries.  Using the
        // FQN avoids false positives from short-name collisions.
        if name.contains('\\')
            && let Ok(idx) = self.class_index.lock()
            && let Some(uri) = idx.get(name)
            && let Ok(map) = self.ast_map.lock()
            && let Some(classes) = map.get(uri)
            && let Some(cls) = classes.iter().find(|c| c.name == last_segment)
        {
            return Some(cls.clone());
        }

        // ── Phase 1: Search all already-parsed files in the ast_map ──
        if let Ok(map) = self.ast_map.lock() {
            for classes in map.values() {
                if let Some(cls) = classes.iter().find(|c| c.name == last_segment) {
                    return Some(cls.clone());
                }
            }
        }

        // ── Phase 1.5: Try Composer classmap ──
        // The classmap (from `vendor/composer/autoload_classmap.php`) maps
        // FQNs directly to file paths.  This is more targeted than PSR-4
        // (a single hash lookup) and covers classes that don't follow PSR-4
        // conventions.  When the user runs `composer install -o`, *all*
        // classes end up in the classmap, giving complete coverage.
        if let Ok(cmap) = self.classmap.lock()
            && let Some(file_path) = cmap.get(name)
        {
            let file_path = file_path.clone();
            drop(cmap); // release lock before doing I/O

            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let mut classes = self.parse_php(&content);

                let file_use_map = self.parse_use_statements(&content);
                let file_namespace = self.parse_namespace(&content);
                Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

                let result = classes.iter().find(|c| c.name == last_segment).cloned();

                let uri = format!("file://{}", file_path.display());
                if let Ok(mut map) = self.ast_map.lock() {
                    map.insert(uri.clone(), classes);
                }
                if let Ok(mut map) = self.use_map.lock() {
                    map.insert(uri.clone(), file_use_map);
                }
                if let Ok(mut map) = self.namespace_map.lock() {
                    map.insert(uri, file_namespace);
                }

                if result.is_some() {
                    return result;
                }
            }
        }

        // ── Phase 2: Try PSR-4 resolution ──
        if let Some(workspace_root) = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            && let Ok(mappings) = self.psr4_mappings.lock()
            && let Some(file_path) = composer::resolve_class_path(&mappings, &workspace_root, name)
            && let Ok(content) = std::fs::read_to_string(&file_path)
        {
            let mut classes = self.parse_php(&content);

            let file_use_map = self.parse_use_statements(&content);
            let file_namespace = self.parse_namespace(&content);
            Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

            let result = classes.iter().find(|c| c.name == last_segment).cloned();

            let uri = format!("file://{}", file_path.display());
            if let Ok(mut map) = self.ast_map.lock() {
                map.insert(uri.clone(), classes);
            }
            if let Ok(mut map) = self.use_map.lock() {
                map.insert(uri.clone(), file_use_map);
            }
            if let Ok(mut map) = self.namespace_map.lock() {
                map.insert(uri, file_namespace);
            }

            if result.is_some() {
                return result;
            }
        }

        // ── Phase 3: Try embedded PHP stubs ──
        // Stubs are bundled in the binary for built-in classes/interfaces
        // (e.g. UnitEnum, BackedEnum).  Parse on first access and cache in
        // the ast_map under a `phpantom-stub://` URI so subsequent lookups
        // hit Phase 1 and skip parsing entirely.
        if let Some(&stub_content) = self.stub_index.get(last_segment) {
            let mut classes = self.parse_php(stub_content);

            // Stubs are in the root namespace — use an empty use_map / namespace.
            let empty_use_map = std::collections::HashMap::new();
            let no_namespace: Option<String> = None;
            Self::resolve_parent_class_names(&mut classes, &empty_use_map, &no_namespace);

            let result = classes.iter().find(|c| c.name == last_segment).cloned();

            let uri = format!("phpantom-stub://{}", last_segment);
            if let Ok(mut map) = self.ast_map.lock() {
                map.insert(uri, classes);
            }

            return result;
        }

        None
    }

    /// Try to find a standalone function by name, checking user-defined
    /// functions first, then falling back to embedded PHP stubs.
    ///
    /// The lookup order is:
    ///   1. `global_functions` — functions from Composer autoload files and
    ///      opened/changed files.
    ///   2. `stub_function_index` — built-in PHP functions embedded from
    ///      phpstorm-stubs.  Parsed lazily on first access and cached in
    ///      `global_functions` under a `phpantom-stub-fn://` URI so
    ///      subsequent lookups hit step 1.
    ///
    /// `candidates` is a list of names to try (e.g. the bare name, the
    /// FQN via use-map, the namespace-qualified name).  The first match
    /// wins.
    pub fn find_or_load_function(&self, candidates: &[&str]) -> Option<FunctionInfo> {
        // ── Phase 1: Check global_functions (user code + already-cached stubs) ──
        if let Ok(fmap) = self.global_functions.lock() {
            for &name in candidates {
                if let Some((_, info)) = fmap.get(name) {
                    return Some(info.clone());
                }
            }
        }

        // ── Phase 2: Try embedded PHP stubs ──
        // The stub_function_index maps function names (including namespaced
        // ones like "Brotli\\compress") to the raw PHP source of the file
        // that defines them.  We parse the entire file, cache all discovered
        // functions in global_functions, and return the one we need.
        for &name in candidates {
            // Normalise: strip leading `\`
            let lookup = name.strip_prefix('\\').unwrap_or(name);

            if let Some(&stub_content) = self.stub_function_index.get(lookup) {
                let functions = self.parse_functions(stub_content);

                if functions.is_empty() {
                    continue;
                }

                let stub_uri = format!("phpantom-stub-fn://{}", lookup);
                let mut result: Option<FunctionInfo> = None;

                if let Ok(mut fmap) = self.global_functions.lock() {
                    for func in &functions {
                        let fqn = if let Some(ref ns) = func.namespace {
                            format!("{}\\{}", ns, &func.name)
                        } else {
                            func.name.clone()
                        };

                        // Check if this is the function we're looking for.
                        if result.is_none() && (fqn == lookup || func.name == lookup) {
                            result = Some(func.clone());
                        }

                        // Cache both the FQN and short name so future
                        // lookups hit Phase 1.
                        fmap.entry(fqn.clone())
                            .or_insert_with(|| (stub_uri.clone(), func.clone()));
                        if func.namespace.is_some() {
                            fmap.entry(func.name.clone())
                                .or_insert_with(|| (stub_uri.clone(), func.clone()));
                        }
                    }
                }

                // Also cache any classes defined in the same stub file so
                // that class lookups for types referenced by the function
                // (e.g. return types) can find them later.
                let mut classes = self.parse_php(stub_content);
                if !classes.is_empty() {
                    let empty_use_map = std::collections::HashMap::new();
                    let stub_namespace = self.parse_namespace(stub_content);
                    Self::resolve_parent_class_names(&mut classes, &empty_use_map, &stub_namespace);
                    let class_uri = format!("phpantom-stub-fn://{}", lookup);
                    if let Ok(mut map) = self.ast_map.lock() {
                        map.insert(class_uri, classes);
                    }
                }

                if result.is_some() {
                    return result;
                }
            }
        }

        None
    }
}
