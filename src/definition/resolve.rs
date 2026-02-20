/// Goto-definition resolution — core entry points.
///
/// Given a cursor position in a PHP file this module:
///   1. Extracts the symbol (class / interface / trait / enum name) under the cursor.
///   2. Resolves it to a fully-qualified name using the file's `use` map and namespace.
///   3. Locates the file on disk via PSR-4 mappings.
///   4. Finds the exact line of the symbol's declaration inside that file.
///   5. Returns an LSP `Location` the editor can jump to.
///
/// Member-access resolution (methods, properties, constants via `->`, `?->`,
/// `::`) is handled by the sibling [`super::member`] module.
///
/// Variable definition resolution (`$var` → most recent assignment /
/// declaration) is handled by the sibling [`super::variable`] module.
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::util::short_name;

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

        // ── Variable go-to-definition ──
        // When the cursor is on a `$variable`, jump to its most recent
        // assignment or declaration (parameter, foreach, catch) above the
        // cursor position.
        //
        // When we are already *at* the definition (resolve returns None),
        // fall through to type-hint resolution so the user can jump from
        // e.g. `HtmlString|string $content` to the `HtmlString` class.
        if Self::cursor_is_on_variable(content, position, &word) {
            let var_name = format!("${}", word);
            if let Some(location) =
                Self::resolve_variable_definition(content, uri, position, &var_name)
            {
                return Some(location);
            }

            // We are at the definition site — try to resolve the type hint.
            if let Some(location) =
                self.resolve_type_hint_at_variable(uri, content, position, &var_name)
            {
                return Some(location);
            }

            return None;
        }

        // ── Member access resolution (::, ->, ?->) ──
        // If the cursor is on a member name (right side of an operator),
        // resolve the owning class and jump to the member declaration.
        //
        // When a member-access operator IS detected but resolution fails
        // (e.g. the owning class couldn't be determined because a helper
        // function like `collect()` isn't indexed), we must return early
        // so that the member name (e.g. `map`) is NOT misinterpreted as
        // a standalone function / class / constant.  Without this guard,
        // `collect($x)->map(` would fall through and resolve `map` to a
        // global `map()` helper function — or even crash while trying.
        let is_member_access = Self::is_member_access_context(content, position);
        if let Some(location) = self.resolve_member_definition(uri, content, position, &word) {
            return Some(location);
        }
        if is_member_access {
            // The cursor is on the RHS of `->`, `?->`, or `::` but we
            // couldn't resolve the owning class.  Don't fall through to
            // standalone symbol resolution — there is no standalone
            // symbol named `map`, `getName`, etc.
            return None;
        }

        // ── Handle `self`, `static`, `parent` keywords ──
        // When the cursor is on one of these keywords (e.g. `new self()`,
        // `new static()`, `new parent()`), resolve to the enclosing class
        // definition (or the parent class for `parent`).
        if (word == "self" || word == "static" || word == "parent")
            && let Some(location) = self.resolve_self_static_parent(uri, content, position, &word)
        {
            return Some(location);
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

        // 5. Resolve file path via PSR-4 (only when workspace root is available).
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(workspace_root) = workspace_root
            && let Ok(mappings) = self.psr4_mappings.lock()
        {
            for fqn in &candidates {
                if let Some(file_path) =
                    composer::resolve_class_path(&mappings, &workspace_root, fqn)
                {
                    // 6. Read the target file and find the definition line.
                    if let Ok(target_content) = std::fs::read_to_string(&file_path) {
                        let sn = short_name(fqn);
                        if let Some(target_position) =
                            Self::find_definition_position(&target_content, sn)
                            && let Ok(target_uri) = Url::from_file_path(&file_path)
                        {
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

        // 7. Try global function lookup as a last resort.
        //    Build candidates: the word itself, the FQN-resolved version, and
        //    (if inside a namespace) the namespace-qualified version.
        let mut func_candidates = candidates.clone();
        if !func_candidates.contains(&word) {
            func_candidates.push(word.clone());
        }

        if let Some(location) = self.resolve_function_definition(&func_candidates) {
            return Some(location);
        }

        // 8. Try standalone constant lookup (define() constants).
        if let Some(location) = self.resolve_constant_definition(&func_candidates) {
            return Some(location);
        }

        None
    }

    // ─── Constant Definition Resolution ─────────────────────────────────────

    /// Resolve a standalone constant to its `define('NAME', …)` call site.
    ///
    /// Checks `global_defines` (user-defined constants discovered from parsed
    /// files) for a matching constant name, reads the source file, and returns
    /// a `Location` pointing at the `define(` call.  Built-in constants from
    /// `stub_constant_index` are not navigable (they have no real file).
    fn resolve_constant_definition(&self, candidates: &[String]) -> Option<Location> {
        // Look up the constant in global_defines.
        let file_uri = {
            let dmap = self.global_defines.lock().ok()?;
            let mut result = None;
            for candidate in candidates {
                if let Some(uri) = dmap.get(candidate.as_str()) {
                    result = Some((candidate.clone(), uri.clone()));
                    break;
                }
            }
            result
        };

        let (const_name, file_uri) = file_uri?;

        // Read the file content (try open files first, then disk).
        let file_content = self
            .open_files
            .lock()
            .ok()
            .and_then(|files| files.get(&file_uri).cloned())
            .or_else(|| {
                let path = Url::parse(&file_uri).ok()?.to_file_path().ok()?;
                std::fs::read_to_string(path).ok()
            })?;

        let position = Self::find_define_position(&file_content, &const_name)?;
        let parsed_uri = Url::parse(&file_uri).ok()?;

        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }

    /// Find the position of a `define('NAME'` or `define("NAME"` call in
    /// file content.
    ///
    /// Searches each line for a `define(` keyword followed (possibly with
    /// whitespace) by a string literal containing the constant name.
    /// Returns the position of the `define` keyword on the matching line.
    fn find_define_position(content: &str, constant_name: &str) -> Option<Position> {
        // Patterns: `'NAME'` and `"NAME"` — we search for these after
        // the `define(` token, allowing optional whitespace.
        let single_q = format!("'{}'", constant_name);
        let double_q = format!("\"{}\"", constant_name);

        for (line_idx, line) in content.lines().enumerate() {
            // Find `define(` anywhere on the line.
            let Some(def_pos) = line.find("define(") else {
                continue;
            };

            // Extract the text after `define(` and trim leading whitespace
            // to allow `define( 'NAME'` with spaces.
            let after_paren = line[def_pos + 7..].trim_start();
            if after_paren.starts_with(&single_q) || after_paren.starts_with(&double_q) {
                return Some(Position {
                    line: line_idx as u32,
                    character: def_pos as u32,
                });
            }
        }

        None
    }

    // ─── Function Definition Resolution ─────────────────────────────────────

    /// Try to resolve a standalone function name to its definition.
    ///
    /// Searches the `global_functions` map (populated from autoload files,
    /// opened/changed files, and cached stub functions) for any of the
    /// given candidate names.  If not found there, falls back to the
    /// embedded PHP stubs via `find_or_load_function` — which parses the
    /// stub lazily and caches it in `global_functions` for future lookups.
    ///
    /// When found, reads the source file and locates the `function name(`
    /// declaration line.  Stub functions (with `phpantom-stub-fn://` URIs)
    /// are not navigable so they are skipped for go-to-definition but
    /// still loaded into the cache for return-type resolution.
    fn resolve_function_definition(&self, candidates: &[String]) -> Option<Location> {
        // ── Phase 1: Check global_functions (user code + cached stubs) ──
        let found = {
            let fmap = self.global_functions.lock().ok()?;
            let mut result = None;
            for candidate in candidates {
                if let Some((uri, info)) = fmap.get(candidate.as_str()) {
                    result = Some((uri.clone(), info.clone()));
                    break;
                }
            }
            result
        };

        // ── Phase 2: Try embedded PHP stubs as fallback ──
        let (file_uri, func_info) = if let Some(pair) = found {
            pair
        } else {
            // Build &str candidates for find_or_load_function.
            let str_candidates: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
            let loaded = self.find_or_load_function(&str_candidates)?;

            // After find_or_load_function, the function is cached in
            // global_functions.  Look it up to get the URI.
            let fmap = self.global_functions.lock().ok()?;
            let mut result = None;
            for candidate in candidates {
                if let Some((uri, info)) = fmap.get(candidate.as_str()) {
                    result = Some((uri.clone(), info.clone()));
                    break;
                }
            }
            result.unwrap_or_else(|| {
                // Fallback: use a synthetic URI with the loaded info.
                (format!("phpantom-stub-fn://{}", loaded.name), loaded)
            })
        };

        // Stub functions don't have real file locations — skip
        // go-to-definition for them (they're still useful for return-type
        // resolution via the function_loader).
        if file_uri.starts_with("phpantom-stub-fn://") {
            return None;
        }

        // Read the file content (try open files first, then disk).
        let file_content = self
            .open_files
            .lock()
            .ok()
            .and_then(|files| files.get(&file_uri).cloned())
            .or_else(|| {
                let path = Url::parse(&file_uri).ok()?.to_file_path().ok()?;
                std::fs::read_to_string(path).ok()
            })?;

        let position = Self::find_function_position(&file_content, &func_info.name)?;
        let parsed_uri = Url::parse(&file_uri).ok()?;

        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }

    /// Find the position of a standalone `function name(` declaration in
    /// file content.
    ///
    /// This is distinct from `find_member_position` (which searches inside
    /// a class body) — here we look for top-level or namespace-level
    /// function declarations.
    fn find_function_position(content: &str, function_name: &str) -> Option<Position> {
        let pattern = format!("function {}", function_name);

        let is_word_boundary = |c: u8| {
            let ch = c as char;
            !ch.is_alphanumeric() && ch != '_'
        };

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(col) = line.find(&pattern) {
                // Verify word boundary before `function` keyword.
                let before_ok = col == 0 || is_word_boundary(line.as_bytes()[col - 1]);

                // Verify word boundary after the function name.
                let after_pos = col + pattern.len();
                let after_ok =
                    after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);

                if before_ok && after_ok {
                    return Some(Position {
                        line: line_idx as u32,
                        character: col as u32,
                    });
                }
            }
        }

        None
    }

    // ─── Word Extraction & FQN Resolution ───────────────────────────────────

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
    pub(super) fn find_definition_in_ast_map(
        &self,
        fqn: &str,
        content: &str,
        uri: &str,
    ) -> Option<Location> {
        let short_name = short_name(fqn);

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())?;

        let _class_info = classes.iter().find(|c| {
            if c.name != short_name {
                return false;
            }
            // Build the FQN of this class in the current file and compare
            // against the requested FQN to avoid false matches when two
            // namespaces contain classes with the same short name.
            let file_namespace = self
                .namespace_map
                .lock()
                .ok()
                .and_then(|map| map.get(uri).cloned())
                .flatten();
            let class_fqn = match &file_namespace {
                Some(ns) => format!("{}\\{}", ns, c.name),
                None => c.name.clone(),
            };
            class_fqn == fqn
        })?;

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
    /// Resolve `self`, `static`, or `parent` keywords to a class definition.
    ///
    /// - `self` / `static` → jump to the enclosing class declaration.
    /// - `parent` → jump to the parent class declaration (from `extends`).
    fn resolve_self_static_parent(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        keyword: &str,
    ) -> Option<Location> {
        let cursor_offset = Self::position_to_offset(content, position)?;

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let current_class = Self::find_class_at_offset(&classes, cursor_offset)?;

        if keyword == "self" || keyword == "static" {
            // Jump to the enclosing class definition in the current file.
            let target_position = Self::find_definition_position(content, &current_class.name)?;
            let parsed_uri = Url::parse(uri).ok()?;
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: target_position,
                    end: target_position,
                },
            });
        }

        // keyword == "parent"
        let parent_name = current_class.parent_class.as_ref()?;

        // Try to find the parent class in the current file first.
        if let Some(pos) = Self::find_definition_position(content, parent_name) {
            let parsed_uri = Url::parse(uri).ok()?;
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: pos,
                    end: pos,
                },
            });
        }

        // Resolve the parent class name to a FQN using use-map / namespace.
        let file_use_map = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        let fqn = Self::resolve_to_fqn(parent_name, &file_use_map, &file_namespace);

        // Try class_index / ast_map lookup via find_class_file_content.
        let sn = short_name(&fqn);
        if let Some((class_uri, class_content)) = self.find_class_file_content(sn, uri, content)
            && let Some(pos) = Self::find_definition_position(&class_content, sn)
            && let Ok(parsed_uri) = Url::parse(&class_uri)
        {
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: pos,
                    end: pos,
                },
            });
        }

        // Try PSR-4 resolution as a last resort.
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(workspace_root) = workspace_root
            && let Ok(mappings) = self.psr4_mappings.lock()
        {
            let candidates = [fqn.as_str(), parent_name.as_str()];
            for candidate in &candidates {
                if let Some(file_path) =
                    composer::resolve_class_path(&mappings, &workspace_root, candidate)
                    && let Ok(target_content) = std::fs::read_to_string(&file_path)
                {
                    let name = short_name(candidate);
                    if let Some(target_position) =
                        Self::find_definition_position(&target_content, name)
                        && let Ok(target_uri) = Url::from_file_path(&file_path)
                    {
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

        None
    }

    pub fn find_definition_position(content: &str, class_name: &str) -> Option<Position> {
        let keywords = ["class", "interface", "trait", "enum"];

        // Track whether we are inside a `/* … */` block comment.
        let mut in_block_comment = false;

        for (line_idx, line) in content.lines().enumerate() {
            // ── Block-comment tracking ──────────────────────────────────
            // Walk through the line handling `/*` and `*/` toggles so we
            // know whether the keyword match is inside a comment.
            let mut effective_line = String::new();
            let line_bytes = line.as_bytes();
            let mut i = 0;
            while i < line_bytes.len() {
                if in_block_comment {
                    // Look for closing `*/`.
                    if i + 1 < line_bytes.len()
                        && line_bytes[i] == b'*'
                        && line_bytes[i + 1] == b'/'
                    {
                        in_block_comment = false;
                        // Replace the `*/` with spaces to preserve column offsets.
                        effective_line.push(' ');
                        effective_line.push(' ');
                        i += 2;
                    } else {
                        effective_line.push(' ');
                        i += 1;
                    }
                } else if i + 1 < line_bytes.len()
                    && line_bytes[i] == b'/'
                    && line_bytes[i + 1] == b'*'
                {
                    // Opening `/*` — rest of line (until `*/`) is a comment.
                    in_block_comment = true;
                    effective_line.push(' ');
                    effective_line.push(' ');
                    i += 2;
                } else if i + 1 < line_bytes.len()
                    && line_bytes[i] == b'/'
                    && line_bytes[i + 1] == b'/'
                {
                    // Line comment `//` — blank out the rest of the line.
                    while i < line_bytes.len() {
                        effective_line.push(' ');
                        i += 1;
                    }
                } else if line_bytes[i] == b'#' {
                    // Line comment `#` — blank out the rest of the line.
                    while i < line_bytes.len() {
                        effective_line.push(' ');
                        i += 1;
                    }
                } else {
                    effective_line.push(line_bytes[i] as char);
                    i += 1;
                }
            }

            for keyword in &keywords {
                // Search for `keyword ClassName` making sure ClassName is
                // followed by a word boundary (whitespace, `{`, `:`, end of
                // line) so we don't match partial names.
                let pattern = format!("{} {}", keyword, class_name);
                if let Some(col) = effective_line.find(&pattern) {
                    // Verify word boundary before the keyword: either start
                    // of line or preceded by whitespace / non-alphanumeric.
                    let before_ok = col == 0 || {
                        let prev = effective_line
                            .as_bytes()
                            .get(col - 1)
                            .copied()
                            .unwrap_or(b' ');
                        !(prev as char).is_alphanumeric() && prev != b'_'
                    };

                    // Verify word boundary after the class name.
                    let after_pos = col + pattern.len();
                    let after_ok = after_pos >= effective_line.len() || {
                        let next = effective_line
                            .as_bytes()
                            .get(after_pos)
                            .copied()
                            .unwrap_or(b' ');
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
