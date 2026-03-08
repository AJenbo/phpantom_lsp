//! File loading and member position lookup helpers.
//!
//! These functions locate the source file for a class and find the exact
//! position of a member declaration within it.  They are used by
//! `resolve_member_definition_with` after the declaring class has been
//! identified by the `declaring` module.

use tower_lsp::lsp_types::Position;

use crate::Backend;
use crate::types::ClassInfo;
use crate::util::short_name;

use super::MemberKind;

impl Backend {
    /// Reload the raw (unmerged) `ClassInfo` for a candidate.
    ///
    /// Candidates returned by `resolve_target_classes` may be
    /// fully-resolved classes with virtual/mixin members baked into
    /// their `methods` list (this happens when `type_hint_to_classes`
    /// calls `resolve_class_fully` to apply generic substitutions).
    /// `find_declaring_class` needs the raw class so it can trace
    /// member declarations through the real inheritance and mixin
    /// chain instead of short-circuiting on a merged method.
    ///
    /// Returns `Some(raw)` when a reload succeeds, or `None` when the
    /// class cannot be reloaded (e.g. synthetic/anonymous classes).
    pub(in crate::definition) fn reload_raw_class(
        candidate: &ClassInfo,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        let fqn = match &candidate.file_namespace {
            Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, candidate.name),
            _ => candidate.name.clone(),
        };
        crate::util::find_class_by_name(all_classes, &fqn)
            .cloned()
            .or_else(|| class_loader(&fqn))
    }

    /// Find the file URI and content for the file that contains a given class.
    ///
    /// `class_name` can be a short name (e.g. `"Kernel"`) or a
    /// fully-qualified name (e.g. `"Illuminate\\Foundation\\Console\\Kernel"`).
    /// When a namespace prefix is present the file's namespace (from
    /// `namespace_map`) must match for the class to be returned.  This
    /// prevents short-name collisions when a child class and its parent
    /// share the same simple name but live in different namespaces.
    ///
    /// Searches the `ast_map` (which includes files loaded via PSR-4 by
    /// `find_or_load_class`) and returns `(uri, content)`.
    pub(crate) fn find_class_file_content(
        &self,
        class_name: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<(String, String)> {
        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let last_segment = short_name(normalized);
        let expected_ns: Option<&str> = if normalized.contains('\\') {
            Some(&normalized[..normalized.len() - last_segment.len() - 1])
        } else {
            None
        };

        // Search the ast_map for the file containing this class.
        let uri = {
            let map = self.ast_map.read();
            let nmap = self.namespace_map.read();

            // Check whether a class with the right short name and
            // namespace lives in this file.  Uses the per-class
            // `file_namespace` field first (correct for multi-namespace
            // files like example.php), falling back to the file-level
            // `namespace_map` for single-namespace files.
            let class_in_file = |file_uri: &str, classes: &[ClassInfo]| -> bool {
                match expected_ns {
                    None => classes.iter().any(|c| c.name == last_segment),
                    Some(exp) => {
                        // Prefer per-class file_namespace (handles
                        // multi-namespace files correctly).
                        let found_via_class_ns = classes.iter().any(|c| {
                            c.name == last_segment && c.file_namespace.as_deref() == Some(exp)
                        });
                        if found_via_class_ns {
                            return true;
                        }
                        // Fall back to file-level namespace_map for
                        // classes that don't have file_namespace set
                        // (e.g. single-namespace files, stubs).
                        let file_ns = nmap.get(file_uri).and_then(|opt| opt.as_deref());
                        file_ns == Some(exp) && classes.iter().any(|c| c.name == last_segment)
                    }
                }
            };

            // Check the current file first (common case: $this->method).
            if let Some(classes) = map.get(current_uri) {
                if class_in_file(current_uri, classes) {
                    Some(current_uri.to_string())
                } else {
                    // Search other files.
                    map.iter()
                        .find(|(u, classes)| class_in_file(u, classes))
                        .map(|(u, _)| u.clone())
                }
            } else {
                map.iter()
                    .find(|(u, classes)| class_in_file(u, classes))
                    .map(|(u, _)| u.clone())
            }
        }?;

        // Get the file content.
        let file_content = if uri == current_uri {
            current_content.to_string()
        } else if uri.starts_with("phpantom-stub://") {
            // Embedded stubs are stored under synthetic URIs and have no
            // on-disk file.  Retrieve the raw stub source from the
            // stub_index instead.
            self.stub_index.get(last_segment).map(|s| s.to_string())?
        } else {
            self.get_file_content(&uri)?
        };

        Some((uri, file_content))
    }

    /// Find the position of a member declaration (method, property, or constant)
    /// inside a PHP file.
    ///
    /// Find the position of a member declaration in source content.
    ///
    /// When `name_offset` is `Some(off)` with `off > 0`, the position is
    /// computed directly from the stored byte offset (fast path).
    ///
    /// When the offset is unavailable (virtual `@method` / `@property`
    /// members), falls back to scanning the file's docblock comments for
    /// the tag that declares the member.
    pub(crate) fn find_member_position(
        content: &str,
        member_name: &str,
        kind: MemberKind,
        name_offset: Option<u32>,
    ) -> Option<Position> {
        // ── Fast path: use stored AST offset ────────────────────────────
        if let Some(off) = name_offset
            && off > 0
            && (off as usize) <= content.len()
        {
            let mut pos = crate::util::offset_to_position(content, off as usize);
            // For properties, place the cursor on the first letter
            // after `$` so that a second go-to-definition triggers
            // type-hint resolution (matches the text-search behavior).
            if kind == MemberKind::Property {
                pos.character += 1;
            }
            return Some(pos);
        }

        let is_word_boundary = |c: u8| {
            let ch = c as char;
            !ch.is_alphanumeric() && ch != '_'
        };

        // Fallback: for properties, check if this is a magic property
        // declared via a `@property` tag in the class docblock.
        // Lines look like: ` * @property Type $propertyName`
        // NOTE: docblock tags precede the class body, so they fall
        // outside `[start_offset, end_offset)`.  Don't scope these
        // fallback searches by class_range.
        if kind == MemberKind::Property {
            let var_pattern = format!("${}", member_name);
            for (line_idx, line) in content.lines().enumerate() {
                if let Some(col) = line.find(&var_pattern) {
                    let after_pos = col + var_pattern.len();
                    let after_ok =
                        after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                    if !after_ok {
                        continue;
                    }

                    let trimmed = line.trim().trim_start_matches('*').trim();
                    if trimmed.starts_with("@property-read")
                        || trimmed.starts_with("@property-write")
                        || trimmed.starts_with("@property")
                    {
                        return Some(Position {
                            line: line_idx as u32,
                            character: (col + 1) as u32,
                        });
                    }
                }
            }
        }

        // Fallback: for methods, check if this is a magic method
        // declared via a `@method` tag in the class docblock.
        // Lines look like: ` * @method ReturnType methodName(params...)`
        // NOTE: same as above — docblock tags are outside the class body
        // range, so don't scope by class_range.
        if kind == MemberKind::Method {
            // The method name is followed by `(` in a @method tag.
            let method_pattern = member_name;
            for (line_idx, line) in content.lines().enumerate() {
                // Search for ALL occurrences of the pattern within the line,
                // not just the first one.  This is important when the method
                // name collides with a type keyword (e.g. `string`) that also
                // appears as the return type on the same line.
                let mut search_start = 0;
                while let Some(offset) = line[search_start..].find(method_pattern) {
                    let col = search_start + offset;
                    search_start = col + method_pattern.len();

                    // Verify the character after the name is `(` (method call syntax).
                    let after_pos = col + method_pattern.len();
                    if after_pos >= line.len() {
                        continue;
                    }
                    let after_char = line.as_bytes()[after_pos];
                    if after_char != b'(' {
                        continue;
                    }

                    // Verify the character before is a word boundary (whitespace)
                    // to avoid matching partial names.
                    if col > 0 && !is_word_boundary(line.as_bytes()[col - 1]) {
                        continue;
                    }

                    let trimmed = line.trim().trim_start_matches('*').trim();
                    if trimmed.starts_with("@method") {
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
