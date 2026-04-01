//! Object shape property and Eloquent array entry position lookup.
//!
//! These helpers locate property keys inside `object{…}` shape annotations
//! and string literal entries inside Eloquent array properties (`$casts`,
//! `$fillable`, etc.) so that go-to-definition can jump to the right
//! position in the source file.

use tower_lsp::lsp_types::Position;

use crate::Backend;

impl Backend {
    /// Find the position of a property key inside an `object{…}` shape
    /// annotation within docblock comments.
    ///
    /// Scans `content` for docblock lines containing `object{` (or
    /// `?object{`, `\object{`) and, within matching braces, looks for
    /// `key_name:` or `key_name?:`.  Returns the `Position` of the
    /// first character of the key name.
    ///
    /// When `near_offset` is provided, the match closest to that byte
    /// offset (in either direction) is returned.  This handles both
    /// inline `@var` annotations above the cursor and `@return`
    /// docblocks on methods defined below the usage site.
    pub(in crate::definition) fn find_object_shape_property_position(
        content: &str,
        key_name: &str,
        near_offset: Option<usize>,
    ) -> Option<Position> {
        // We need to find `key_name:` or `key_name?:` inside an
        // `object{…}` block that appears inside a docblock comment.
        //
        // Strategy: scan every line.  Track whether we are inside a
        // `/** … */` comment.  When we see `object{` (case-insensitive
        // base word) at brace depth 0, enter shape-scanning mode and
        // look for the key.

        let mut matches: Vec<(usize, u32, u32)> = Vec::new(); // (byte_offset, line, col)
        let mut byte_offset: usize = 0;
        let mut in_docblock = false;

        for (line_idx, line) in content.lines().enumerate() {
            let line_len = line.len() + 1; // +1 for newline

            // Track docblock boundaries.
            if line.contains("/**") {
                in_docblock = true;
            }

            if in_docblock {
                // Search for object shape property keys in this line.
                // Look for `object{` patterns (possibly preceded by `?` or `\`).
                if let Some(pos) = Self::find_shape_key_in_line(line, key_name) {
                    let abs_offset = byte_offset + pos;
                    matches.push((abs_offset, line_idx as u32, pos as u32));
                }
            }

            if line.contains("*/") {
                in_docblock = false;
            }

            byte_offset += line_len;
        }

        // Pick the match closest to the cursor.  When no near_offset
        // is given, return the last match (highest line number).
        let best = match near_offset {
            Some(cursor) => matches
                .into_iter()
                .min_by_key(|(off, _, _)| cursor.abs_diff(*off)),
            None => matches.into_iter().last(),
        };

        best.map(|(_, line, col)| Position {
            line,
            character: col,
        })
    }

    /// Search a single line for a property key inside an `object{…}`
    /// shape.  Returns the byte offset of the key within the line, or
    /// `None`.
    fn find_shape_key_in_line(line: &str, key_name: &str) -> Option<usize> {
        let bytes = line.as_bytes();

        // Find every `object{` (case-insensitive) in the line.
        let lower = line.to_ascii_lowercase();
        let mut search_from = 0usize;

        while let Some(obj_pos) = lower[search_from..].find("object{") {
            let abs_obj = search_from + obj_pos;
            let brace_start = abs_obj + "object".len(); // index of `{`

            // Walk from the `{` respecting nesting to find keys.
            let mut depth = 0i32;
            let mut i = brace_start;
            while i < bytes.len() {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ if depth == 1 => {
                        // At depth 1 we are inside the outermost `object{…}`.
                        // Check if the key starts here.
                        if let Some(col) = Self::match_shape_key_at(line, i, key_name) {
                            return Some(col);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }

            search_from = abs_obj + 1;
        }

        None
    }

    /// Check whether `key_name` (possibly quoted) starts at position
    /// `pos` within `line`.  Returns the column of the first character
    /// of the key (inside quotes if quoted).
    fn match_shape_key_at(line: &str, pos: usize, key_name: &str) -> Option<usize> {
        let rest = &line[pos..];
        let rest_trimmed = rest.trim_start();
        let leading_ws = rest.len() - rest_trimmed.len();
        let col_base = pos + leading_ws;

        // Bare key: `name:` or `name?:`
        if let Some(after) = rest_trimmed.strip_prefix(key_name)
            && (after.starts_with(':') || after.starts_with("?:"))
        {
            return Some(col_base);
        }

        // Single-quoted key: `'name':` or `'name'?:`
        if let Some(inner) = rest_trimmed.strip_prefix('\'')
            && let Some(after_key) = inner.strip_prefix(key_name)
            && (after_key.starts_with("':") || after_key.starts_with("'?:"))
        {
            // Point inside the quote at the first letter.
            return Some(col_base + 1);
        }

        // Double-quoted key: `"name":` or `"name"?:`
        if let Some(inner) = rest_trimmed.strip_prefix('"')
            && let Some(after_key) = inner.strip_prefix(key_name)
            && (after_key.starts_with("\":") || after_key.starts_with("\"?:"))
        {
            return Some(col_base + 1);
        }

        None
    }

    /// Find a string literal entry inside an Eloquent array property.
    ///
    /// Searches for `'member_name'` or `"member_name"` inside `$casts`,
    /// `$attributes`, `$fillable`, `$guarded`, `$hidden`, `$visible`,
    /// and `$appends` property declarations within the given class range.
    /// Returns the position of the string literal so go-to-definition
    /// can jump to it.
    pub(in crate::definition) fn find_eloquent_array_entry(
        content: &str,
        member_name: &str,
        class_range: Option<(usize, usize)>,
    ) -> Option<Position> {
        let single_pattern = format!("'{member_name}'");
        let double_pattern = format!("\"{member_name}\"");
        let targets = [
            "$casts",
            "$attributes",
            "$fillable",
            "$guarded",
            "$hidden",
            "$visible",
            "$appends",
        ];

        // Track whether we're inside one of the target property arrays.
        let mut in_target_property = false;
        let mut byte_offset: usize = 0;

        for (line_idx, line) in content.lines().enumerate() {
            let line_len = line.len() + 1;
            let in_range = match class_range {
                Some((start, end)) => byte_offset >= start && byte_offset < end,
                None => true,
            };
            if in_range {
                let trimmed = line.trim();
                // Detect property declarations for target arrays.
                if targets.iter().any(|t| trimmed.contains(t)) {
                    in_target_property = true;
                }
                // Also detect the casts() method body.
                if trimmed.contains("function casts(") {
                    in_target_property = true;
                }

                if in_target_property {
                    // Look for the member name as a string key.
                    if let Some(col) = line.find(&single_pattern) {
                        // Position cursor inside the quotes on the first
                        // letter of the column name.
                        return Some(Position {
                            line: line_idx as u32,
                            character: (col + 1) as u32,
                        });
                    }
                    if let Some(col) = line.find(&double_pattern) {
                        return Some(Position {
                            line: line_idx as u32,
                            character: (col + 1) as u32,
                        });
                    }

                    // A line ending with `];` or just `];` closes the array.
                    if trimmed == "];" || trimmed.ends_with("];") {
                        in_target_property = false;
                    }
                }
            }
            byte_offset += line_len;
        }
        None
    }
}
