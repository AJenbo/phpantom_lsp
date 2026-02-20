/// Variable definition resolution.
///
/// This module handles go-to-definition for `$variable` references,
/// jumping from a variable usage to its most recent assignment or
/// declaration site.
///
/// Supported definition sites (searched bottom-up from cursor):
///   - **Assignment**: `$var = …` (but not `==` / `===`)
///   - **Parameter**: `Type $var` in a function/method signature
///   - **Foreach**: `as $var` / `=> $var`
///   - **Catch**: `catch (…Exception $var)`
///   - **Static / global**: `static $var` / `global $var`
///
/// When the cursor is already at the definition site (e.g. on a
/// parameter), the module falls through to type-hint resolution:
/// it extracts the type hint and jumps to the first class-like type
/// in it (e.g. `HtmlString` in `HtmlString|string $content`).
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::util::short_name;

impl Backend {
    // ──────────────────────────────────────────────────────────────────────
    // Variable go-to-definition helpers
    // ──────────────────────────────────────────────────────────────────────

    /// Returns `true` when the cursor is sitting on a `$variable` token.
    ///
    /// `extract_word_at_position` strips `$`, so we peek at the character
    /// immediately before the word to see if it is `$`.
    pub(super) fn cursor_is_on_variable(content: &str, position: Position, _word: &str) -> bool {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return false;
        }
        let line = lines[line_idx];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Find where `word` starts on this line (same logic as
        // extract_word_at_position: walk left from cursor).
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '\\';
        let mut start = col;
        if start < chars.len() && is_word_char(chars[start]) {
            // on a word char
        } else if start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        } else {
            return false;
        }
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }

        // The character just before the word must be `$`.
        if start == 0 {
            return false;
        }
        chars[start - 1] == '$'
    }

    /// Find the most recent assignment or declaration of `$var_name` before
    /// `position` and return its location.
    ///
    /// Recognised definition sites (searched bottom-up):
    ///   - Assignment:          `$var = …`  (but not `==` / `===`)
    ///   - Parameter:           `Type $var` in a function/method signature
    ///   - Foreach:             `as $var`  /  `=> $var`
    ///   - Catch:               `catch (…Exception $var)`
    ///   - Static / global:     `static $var` / `global $var`
    pub(super) fn resolve_variable_definition(
        content: &str,
        uri: &str,
        position: Position,
        var_name: &str,
    ) -> Option<Location> {
        let lines: Vec<&str> = content.lines().collect();
        let cursor_line = position.line as usize;

        // Scan backwards from the line *before* the cursor.  If the cursor
        // line itself contains a definition we skip it — the user is
        // presumably already looking at it.  (If the cursor is on the
        // definition, there is nothing earlier to jump to, so we fall
        // through and return None.)
        let search_end = cursor_line;

        for line_idx in (0..search_end).rev() {
            let line = lines[line_idx];

            // Quick reject: line must mention the variable at all.
            if !line.contains(var_name) {
                continue;
            }

            if let Some(col) = Self::line_defines_variable(line, var_name) {
                let target_uri = Url::parse(uri).ok()?;
                return Some(Location {
                    uri: target_uri,
                    range: Range {
                        start: Position {
                            line: line_idx as u32,
                            character: col as u32,
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: (col + var_name.len()) as u32,
                        },
                    },
                });
            }
        }

        None
    }

    /// Find a whole-word occurrence of `var_name` in `line`, skipping
    /// partial matches like `$item` inside `$items`.
    ///
    /// Returns the byte offset of the match, or `None` when no whole-word
    /// occurrence exists.
    fn find_whole_var(line: &str, var_name: &str) -> Option<usize> {
        let is_ident_char = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = 0;
        while let Some(pos) = line[start..].find(var_name) {
            let abs = start + pos;
            let after = abs + var_name.len();
            // Check that the character immediately after is NOT an
            // identifier character (prevents `$item` matching `$items`).
            let boundary_ok =
                after >= line.len() || !line[after..].starts_with(|c: char| is_ident_char(c));
            if boundary_ok {
                return Some(abs);
            }
            // Skip past this false match and keep searching.
            start = abs + 1;
        }
        None
    }

    /// Heuristically decide whether `line` *defines* (assigns / declares)
    /// `$var_name`.
    ///
    /// Returns `Some(column)` with the byte offset of the variable on the
    /// line when it is a definition site, or `None` otherwise.
    fn line_defines_variable(line: &str, var_name: &str) -> Option<usize> {
        // Find a whole-word occurrence of the variable in the line.
        let var_pos = Self::find_whole_var(line, var_name)?;
        let after_var = var_pos + var_name.len();
        let rest = &line[after_var..];

        // 1. Assignment: `$var =` but NOT `$var ==` / `$var ===`
        let rest_trimmed = rest.trim_start();
        if rest_trimmed.starts_with('=') && !rest_trimmed.starts_with("==") {
            return Some(var_pos);
        }

        // 2. Foreach value: `as $var` or `=> $var`
        //    Look at what precedes the variable.
        let before = line[..var_pos].trim_end();
        if before.ends_with("as") || before.ends_with("=>") {
            return Some(var_pos);
        }

        // 3. Function / method parameter: the variable appears after a
        //    type hint (or bare) inside `(…)`.  A simple heuristic: the
        //    line contains `function ` and `$var` appears after `(`.
        if (line.contains("function ")
            || line.contains("function(")
            || line.contains("fn ")
            || line.contains("fn("))
            && before.contains('(')
        {
            return Some(var_pos);
        }

        // 4. Catch variable: `catch (SomeException $var)`
        if before.contains("catch") && before.contains('(') {
            return Some(var_pos);
        }

        // 5. Static / global declarations: `static $var` / `global $var`
        if before.ends_with("static") || before.ends_with("global") {
            return Some(var_pos);
        }

        None
    }

    // ─── Type-Hint Resolution at Variable Definition ────────────────────

    /// When the cursor is on a variable that is already at its definition
    /// site (parameter, property, promoted property), extract the type hint
    /// and jump to the first class-like type in it.
    ///
    /// For example, given `public readonly HtmlString|string $content,`
    /// this returns the location of the `HtmlString` class definition.
    pub(super) fn resolve_type_hint_at_variable(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        var_name: &str,
    ) -> Option<Location> {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return None;
        }
        let line = lines[line_idx];

        // The variable must actually appear on this line.
        let var_pos = Self::find_whole_var(line, var_name)?;

        // Extract the text before `$var` — this contains modifiers and the
        // type hint.
        let before_raw = line[..var_pos].trim_end();

        // For function/method parameters the text includes the signature up
        // to and including `(`, e.g. `public function handle(Request`.
        // Strip everything up to the last `(` so we only look at the
        // parameter's type portion.
        let before = match before_raw.rfind('(') {
            Some(pos) => before_raw[pos + 1..].trim_start(),
            None => before_raw,
        };

        // Extract the type-hint portion: everything after the last PHP
        // modifier keyword or visibility.  We split on whitespace and take
        // the last token, which should be the full type expression
        // (e.g. `HtmlString|string`, `?Foo`, `Foo&Bar`).
        let type_hint = match before.rsplit_once(char::is_whitespace) {
            Some((_, t)) => t,
            None => before,
        };
        if type_hint.is_empty() {
            return None;
        }

        // Split on `|` (union) and `&` (intersection), strip leading `?`
        // (nullable shorthand), and find the first class-like type.
        let scalars = [
            "string", "int", "float", "bool", "array", "callable", "iterable", "object", "mixed",
            "void", "never", "null", "false", "true", "self", "static", "parent",
        ];

        let class_name = type_hint
            .split(['|', '&'])
            .map(|t| t.trim_start_matches('?'))
            .find(|t| !t.is_empty() && !scalars.contains(&t.to_lowercase().as_str()))?;

        // Resolve to FQN and jump, reusing the standard class resolution
        // path from resolve_definition.
        let ctx = self.file_context(uri);

        let fqn = Self::resolve_to_fqn(class_name, &ctx.use_map, &ctx.namespace);

        let mut candidates = vec![fqn];
        if class_name.contains('\\') && !candidates.contains(&class_name.to_string()) {
            candidates.push(class_name.to_string());
        }

        // Try same-file first.
        for fqn in &candidates {
            if let Some(location) = self.find_definition_in_ast_map(fqn, content, uri) {
                return Some(location);
            }
        }

        // Try PSR-4 resolution.
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
                    && let Ok(target_content) = std::fs::read_to_string(&file_path)
                {
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

        None
    }
}
