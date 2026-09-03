//! Namespace rename edits.
//!
//! Handles `textDocument/rename` on a namespace segment: rewriting
//! `namespace` declarations, `use` statements, and inline FQN references
//! across every workspace file, and emitting `RenameFile` operations for
//! the PSR-4 directory move.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::symbol_map::SymbolKind;
use crate::text_position::{line_start_byte_offset, offset_to_position, ranges_overlap};
use crate::util::strip_fqn_prefix;

use super::RenameOutcome;

impl Backend {
    /// Plan a namespace-prefix move without requiring an LSP cursor position.
    pub(crate) fn plan_namespace_move(&self, old_prefix: &str, new_prefix: &str) -> RenameOutcome {
        self.build_namespace_prefix_rename_edit(
            strip_fqn_prefix(old_prefix),
            strip_fqn_prefix(new_prefix),
        )
    }

    /// Build a `WorkspaceEdit` for renaming a namespace segment.
    ///
    /// `full_ns` is the full namespace at the declaration site (e.g.
    /// `"App\\Bar\\Service"`).  `segment_idx` is the 0-based index of
    /// the segment being renamed.  `new_segment` is the replacement
    /// text for that segment.
    ///
    /// The method scans every file known to the server to find:
    /// - `namespace` declarations that start with the old prefix
    /// - `use` statements that reference the old prefix
    /// - Inline FQN references (in code and docblocks)
    ///
    /// It also emits `RenameFile` operations when a PSR-4 mapping
    /// exists so that the directory structure stays consistent.
    pub(super) fn build_namespace_rename_edit(
        &self,
        full_ns: &str,
        segment_idx: usize,
        new_segment: &str,
    ) -> RenameOutcome {
        let segments: Vec<&str> = full_ns.split('\\').collect();
        if segment_idx >= segments.len() {
            return Ok(None);
        }

        // Build the old prefix up to and including the renamed segment.
        // For example, if `full_ns` is `App\Bar\Service` and we rename
        // segment 1 (`Bar`), `old_prefix` is `App\Bar`.
        let old_prefix: String = segments[..=segment_idx].join("\\");
        let mut new_segments = segments.clone();
        new_segments[segment_idx] = new_segment;
        let new_prefix: String = new_segments[..=segment_idx].join("\\");

        self.build_namespace_prefix_rename_edit(&old_prefix, &new_prefix)
    }

    pub(super) fn build_namespace_prefix_rename_edit(
        &self,
        old_prefix: &str,
        new_prefix: &str,
    ) -> RenameOutcome {
        // Renaming a namespace onto one that already exists is a merge,
        // and a merge only works where the two sides declare no name
        // twice.  Where they do, the rename is refused outright: moving
        // the rest anyway would rewrite every reference to the clashing
        // name so it points at the class that was already there.
        if let Some(conflict) = self.namespace_merge_conflict(old_prefix, new_prefix) {
            return Err(conflict);
        }

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

        // Scan all known files. The live per-file maps only contain files
        // that have been fully parsed through update_ast, but workspace
        // scanning can index many more class files via uri_classes_index.
        // Include those URIs too so namespace rename updates references in
        // unopened workspace files.
        let all_uris: Vec<String> = {
            let nmap = self.file_namespaces.read();
            let umap = self.file_imports.read();
            let smap = self.symbol_maps.read();
            let cmap = self.symbols.uri_classes_index.read();
            let ofiles = self.open_files.read();
            let workspace_root = self.workspace.workspace_root.read().clone();
            let vendor_dir_paths = self.workspace.vendor_dir_paths.lock().clone();
            let mut uris: std::collections::HashSet<String> = std::collections::HashSet::new();
            for uri in nmap.keys() {
                uris.insert(uri.clone());
            }
            for uri in umap.keys() {
                uris.insert(uri.clone());
            }
            for uri in smap.keys() {
                uris.insert(uri.clone());
            }
            for uri in cmap.keys() {
                uris.insert(uri.clone());
            }
            for uri in ofiles.keys() {
                uris.insert(uri.clone());
            }

            if let Some(root) = workspace_root {
                for path in crate::references::collect_php_files_gitignore(
                    &root,
                    &vendor_dir_paths,
                    &self.index_filters(),
                ) {
                    if let Ok(uri) = Url::from_file_path(&path) {
                        uris.insert(uri.to_string());
                    }
                }
            }

            uris.into_iter().collect()
        };

        let vendor_prefixes = self.workspace.vendor_uri_prefixes.lock().clone();

        for file_uri in &all_uris {
            if vendor_prefixes
                .iter()
                .any(|p| file_uri.starts_with(p.as_str()))
            {
                continue;
            }

            let content = match self.get_file_content(file_uri) {
                Some(c) => c,
                None => continue,
            };

            let parsed_uri = match Url::parse(file_uri) {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!(
                        "rename: dropping edits for file with unparseable URI {file_uri:?}: {e}"
                    );
                    continue;
                }
            };

            let mut file_edits: Vec<TextEdit> = Vec::new();

            // 1. Update `namespace` declarations.
            //    Find lines like `namespace App\Bar\Service;` or
            //    `namespace App\Bar\Service {` where the namespace
            //    starts with `old_prefix`.
            self.collect_namespace_decl_edits(&content, old_prefix, new_prefix, &mut file_edits);

            // 2. Update `use` statements.
            self.collect_use_statement_edits(&content, old_prefix, new_prefix, &mut file_edits);

            // 3. Update inline FQN references from the symbol map.
            //    Steps 1 and 2 scan `content` directly, so only this one
            //    can be poisoned by a map that predates the file; when it
            //    is, the whole rename is dropped.  Emitting the other two
            //    files' edits would leave the workspace half-renamed.
            if !self.collect_fqn_reference_edits(
                file_uri,
                &content,
                old_prefix,
                new_prefix,
                &mut file_edits,
            ) {
                return Ok(None);
            }

            if !file_edits.is_empty() {
                // Sort edits by start position descending so they don't
                // interfere with each other when applied.
                file_edits.sort_by(|a, b| {
                    b.range
                        .start
                        .line
                        .cmp(&a.range.start.line)
                        .then(b.range.start.character.cmp(&a.range.start.character))
                });
                // Deduplicate overlapping edits (keep first = largest line).
                file_edits.dedup_by(|a, b| ranges_overlap(&a.range, &b.range));
                changes.entry(parsed_uri).or_default().extend(file_edits);
            }
        }

        if changes.is_empty() {
            return Ok(None);
        }

        // PSR-4 directory rename: if a mapping exists, emit RenameFile
        // operations to move the directory.
        if let Some(ops) = self.build_namespace_psr4_rename_ops(old_prefix, new_prefix)
            && !ops.is_empty()
            && self.supports_file_rename.load(Ordering::Acquire)
        {
            let mut doc_ops: Vec<DocumentChangeOperation> = Vec::new();

            // Add directory/file rename operations first.
            for (old_uri, new_uri) in &ops {
                doc_ops.push(DocumentChangeOperation::Op(ResourceOp::Rename(
                    RenameFile {
                        old_uri: old_uri.clone(),
                        new_uri: new_uri.clone(),
                        options: None,
                        annotation_id: None,
                    },
                )));
            }

            // Convert text edits to document changes. Rewrite URIs
            // that fall inside a renamed directory.
            for (uri, edits) in changes {
                // A directory operation carries every file beneath it,
                // so its edits have to follow.  The remainder has to
                // start at a path separator or `src/Internal` would also
                // claim `src/InternalOther/Thing.php`.  A per-file
                // operation matches outright and leaves the remainder
                // empty; a file no operation names keeps its own URI,
                // which is what leaves a skipped file edited in place.
                let target_uri = ops
                    .iter()
                    .find_map(|(old_u, new_u)| {
                        let rest = uri.as_str().strip_prefix(old_u.as_str())?;
                        if !rest.is_empty() && !rest.starts_with('/') {
                            return None;
                        }
                        Url::parse(&format!("{}{}", new_u.as_str(), rest)).ok()
                    })
                    .unwrap_or(uri);

                let text_doc_edit = TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: target_uri,
                        version: None,
                    },
                    edits: edits.into_iter().map(OneOf::Left).collect(),
                };
                doc_ops.push(DocumentChangeOperation::Edit(text_doc_edit));
            }

            return Ok(Some(WorkspaceEdit {
                changes: None,
                document_changes: Some(DocumentChanges::Operations(doc_ops)),
                change_annotations: None,
            }));
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    /// Collect text edits for `namespace` declaration lines where the
    /// namespace starts with `old_prefix`.
    fn collect_namespace_decl_edits(
        &self,
        content: &str,
        old_prefix: &str,
        new_prefix: &str,
        edits: &mut Vec<TextEdit>,
    ) {
        let old_prefix_lower = old_prefix.to_lowercase();
        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("namespace ") else {
                continue;
            };
            let rest = rest.trim();
            let ns_name = rest.trim_end_matches(';').trim_end_matches('{').trim();

            if ns_name.is_empty() {
                continue;
            }

            let ns_lower = ns_name.to_lowercase();
            // The namespace must equal old_prefix or start with old_prefix + `\`.
            if ns_lower != old_prefix_lower
                && !ns_lower.starts_with(&format!("{}\\", old_prefix_lower))
            {
                continue;
            }

            let new_ns = if ns_name.len() == old_prefix.len() {
                new_prefix.to_string()
            } else {
                format!("{}{}", new_prefix, &ns_name[old_prefix.len()..])
            };

            let line_start_byte = line_start_byte_offset(content, line_idx);
            let ns_offset_in_line = line.find(ns_name).unwrap_or(0);
            let ns_start = line_start_byte + ns_offset_in_line;
            let ns_end = ns_start + ns_name.len();

            edits.push(TextEdit {
                range: Range {
                    start: offset_to_position(content, ns_start),
                    end: offset_to_position(content, ns_end),
                },
                new_text: new_ns,
            });
        }
    }

    /// Collect text edits for `use` statement lines that reference the
    /// old namespace prefix.
    fn collect_use_statement_edits(
        &self,
        content: &str,
        old_prefix: &str,
        new_prefix: &str,
        edits: &mut Vec<TextEdit>,
    ) {
        let old_prefix_lower = old_prefix.to_lowercase();
        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("use ") else {
                continue;
            };
            let rest = rest.trim();
            // Handle `use function` and `use const` prefixes.
            let rest = rest
                .strip_prefix("function ")
                .or_else(|| rest.strip_prefix("const "))
                .unwrap_or(rest)
                .trim();

            let rest = rest.strip_suffix(';').unwrap_or(rest).trim();

            // Handle group use: `use App\Old\{Foo, Bar};`
            if let Some(brace_pos) = rest.find('{') {
                let group_prefix = rest[..brace_pos].trim_end_matches('\\').trim();
                let group_lower = group_prefix.to_lowercase();

                if group_lower == old_prefix_lower
                    || group_lower.starts_with(&format!("{}\\", old_prefix_lower))
                {
                    let new_group_prefix = if group_prefix.len() == old_prefix.len() {
                        new_prefix.to_string()
                    } else {
                        format!("{}{}", new_prefix, &group_prefix[old_prefix.len()..])
                    };

                    let line_start_byte = line_start_byte_offset(content, line_idx);
                    let prefix_offset_in_line = line.find(group_prefix).unwrap_or(0);
                    let prefix_start = line_start_byte + prefix_offset_in_line;
                    let prefix_end = prefix_start + group_prefix.len();

                    edits.push(TextEdit {
                        range: Range {
                            start: offset_to_position(content, prefix_start),
                            end: offset_to_position(content, prefix_end),
                        },
                        new_text: new_group_prefix,
                    });
                }
                continue;
            }

            // Simple use: `use App\Old\Foo;` or `use App\Old\Foo as Bar;`
            let (fqn_part, _alias_part) = if let Some(as_pos) = rest.find(" as ") {
                (rest[..as_pos].trim(), Some(&rest[as_pos + 4..]))
            } else {
                (rest, None)
            };

            let fqn_lower = fqn_part.to_lowercase();
            if fqn_lower == old_prefix_lower
                || fqn_lower.starts_with(&format!("{}\\", old_prefix_lower))
            {
                let new_fqn = if fqn_part.len() == old_prefix.len() {
                    new_prefix.to_string()
                } else {
                    format!("{}{}", new_prefix, &fqn_part[old_prefix.len()..])
                };

                let line_start_byte = line_start_byte_offset(content, line_idx);
                let fqn_offset_in_line = line.find(fqn_part).unwrap_or(0);
                let fqn_start = line_start_byte + fqn_offset_in_line;
                let fqn_end = fqn_start + fqn_part.len();

                edits.push(TextEdit {
                    range: Range {
                        start: offset_to_position(content, fqn_start),
                        end: offset_to_position(content, fqn_end),
                    },
                    new_text: new_fqn,
                });
            }
        }
    }

    /// Collect text edits for inline FQN references (e.g. `\App\Old\Foo`
    /// in type hints or docblocks) that contain the old prefix.
    ///
    /// Returns `false` when the file's symbol map cannot be trusted
    /// against its current text, in which case no edits are collected and
    /// the caller must abandon the rename.
    fn collect_fqn_reference_edits(
        &self,
        file_uri: &str,
        content: &str,
        old_prefix: &str,
        new_prefix: &str,
        edits: &mut Vec<TextEdit>,
    ) -> bool {
        let symbol_map = match self.symbol_maps.read().get(file_uri) {
            Some(sm) => sm.clone(),
            // No map means no offsets to mistrust: this file's namespace
            // and use-statement edits come from scanning `content`.
            None => return true,
        };

        if !symbol_map.matches_source(content) {
            return false;
        }

        let old_prefix_lower = old_prefix.to_lowercase();

        for span in &symbol_map.spans {
            let name = match &span.kind {
                SymbolKind::ClassReference {
                    name, is_fqn: true, ..
                } => name,
                _ => continue,
            };

            // Only process references that contain a backslash (FQN-style).
            let name_normalized = strip_fqn_prefix(name);
            let name_lower = name_normalized.to_lowercase();

            if name_lower != old_prefix_lower
                && !name_lower.starts_with(&format!("{}\\", old_prefix_lower))
            {
                continue;
            }

            // Check source text to see if this is an inline FQN reference
            // (contains `\` in source).  Use-statement references are
            // handled separately by collect_use_statement_edits.
            let source = content
                .get(span.start as usize..span.end as usize)
                .unwrap_or("");

            // A same-length edit keeps `matches_source` happy while still
            // moving the token, so re-read the span and require it to
            // spell the name the map recorded.
            if !source_spells_reference(source, name) {
                return false;
            }

            // A group-use member (`use App\Old\{Foo, Bar};`) records the
            // composed FQN (`App\Old\Foo`) on a span that covers only the
            // trailing segment (`Foo`). `collect_use_statement_edits`
            // already rewrites the group's shared prefix, so emitting a
            // second edit here would duplicate that rewrite onto the
            // trailing segment, turning `App\New\{Foo, Bar}` into
            // `App\New\{App\New\Foo, Bar}`. Skip it: only a span that
            // spells the reference in full needs its own edit.
            if !source_spells_reference_fully(source, name) {
                continue;
            }

            let new_name = if name_normalized.len() == old_prefix.len() {
                if name.starts_with('\\') {
                    format!("\\{}", new_prefix)
                } else {
                    new_prefix.to_string()
                }
            } else {
                let suffix = &name_normalized[old_prefix.len()..];
                if name.starts_with('\\') {
                    format!("\\{}{}", new_prefix, suffix)
                } else {
                    format!("{}{}", new_prefix, suffix)
                }
            };

            // Only emit an edit if the text actually changes.
            if source == new_name {
                continue;
            }

            edits.push(TextEdit {
                range: Range {
                    start: offset_to_position(content, span.start as usize),
                    end: offset_to_position(content, span.end as usize),
                },
                new_text: new_name,
            });
        }

        true
    }

    /// Why a namespace cannot be renamed onto `new_prefix`, or `None`
    /// when every name it carries lands somewhere free.
    ///
    /// Renaming `App\Internal` to an `App\Support` that already exists
    /// is a merge, and the merge is only well-defined while the two
    /// namespaces share no name.  Where they do, there is no answer the
    /// rename can pick: moving the rest and leaving the clash behind
    /// still rewrites every `App\Internal\Helper` reference to
    /// `App\Support\Helper`, which is a different class.
    fn namespace_merge_conflict(&self, old_prefix: &str, new_prefix: &str) -> Option<String> {
        if old_prefix.eq_ignore_ascii_case(new_prefix) {
            return None;
        }

        let mut clashes: Vec<String> = {
            let index = self.symbols.fqn_uri_index.read();
            index
                .iter()
                .filter_map(|(fqn, uri)| {
                    // The tail of a name the rename carries over, e.g.
                    // `Nested\Deep` of `App\Internal\Nested\Deep`.
                    let tail = fqn
                        .get(..old_prefix.len())
                        .filter(|head| head.eq_ignore_ascii_case(old_prefix))
                        .and_then(|_| fqn.get(old_prefix.len()..))
                        .and_then(|rest| rest.strip_prefix('\\'))?;
                    let (declared, at_uri) =
                        index.get_key_value(&format!("{}\\{}", new_prefix, tail))?;
                    // A name the move itself produces is not a clash.
                    (at_uri != uri).then(|| declared.to_string())
                })
                .collect()
        };
        clashes.sort();
        clashes.dedup();

        if clashes.is_empty() {
            return None;
        }

        let listed: Vec<&str> = clashes.iter().take(5).map(String::as_str).collect();
        let extra = clashes.len().saturating_sub(listed.len());
        let suffix = if extra > 0 {
            format!(" (and {} more)", extra)
        } else {
            String::new()
        };

        Some(format!(
            "Cannot rename `{}` to `{}`: {} already declares {}{}. \
             Rename or move the clashing {} first, then retry.",
            old_prefix,
            new_prefix,
            new_prefix,
            listed.join(", "),
            suffix,
            if clashes.len() == 1 {
                "class"
            } else {
                "classes"
            },
        ))
    }

    /// Determine PSR-4 file/directory rename operations for a namespace
    /// rename.
    ///
    /// Returns pairs of `(old_uri, new_uri)`, or `None` if no PSR-4
    /// mapping applies.  Where the destination directory does not exist
    /// the whole directory is moved in one operation; where it does, the
    /// move is a merge and each file is moved individually so the
    /// contents already there survive.
    fn build_namespace_psr4_rename_ops(
        &self,
        old_prefix: &str,
        new_prefix: &str,
    ) -> Option<Vec<(Url, Url)>> {
        let psr4 = self.workspace.psr4_mappings.read();
        let workspace_root = self.workspace.workspace_root.read().clone()?;

        // The destination directory has to come from whichever mapping
        // covers the *new* namespace, which need not be the one covering
        // the old one: a namespace can move between mappings, or out of
        // the autoload map entirely.  Deriving it from the old mapping
        // instead builds a path around a prefix the new name never had.
        // No mapping covers the destination means no file can be placed
        // there, so nothing moves and the declarations are rewritten in
        // place.
        let new_dir = composer::psr4_directory_for_namespace(&psr4, &workspace_root, new_prefix)?;

        let mut ops: Vec<(Url, Url)> = Vec::new();

        for mapping in psr4.iter() {
            let Some(relative_ns) = composer::namespace_below_prefix(old_prefix, &mapping.prefix)
            else {
                continue;
            };

            let base_dir = workspace_root.join(&mapping.base_path);
            let old_dir = if relative_ns.is_empty() {
                base_dir
            } else {
                base_dir.join(relative_ns.replace('\\', std::path::MAIN_SEPARATOR_STR))
            };

            if old_dir == new_dir {
                continue;
            }

            if !old_dir.is_dir() {
                continue;
            }

            if new_dir.exists() {
                collect_merge_move_ops(&old_dir, &old_dir, &new_dir, &mut ops);
            } else {
                let old_url = Url::from_file_path(&old_dir).ok()?;
                let new_url = Url::from_file_path(&new_dir).ok()?;
                ops.push((old_url, new_url));
            }
        }

        if ops.is_empty() { None } else { Some(ops) }
    }
}

/// Collect one move operation per file under `dir`, rebasing each onto
/// `new_root` at the same relative path.
///
/// This is the merge case: `new_root` already exists, so moving the
/// source directory on top of it would clobber or fail depending on the
/// editor.  A destination that is already occupied is left out entirely
/// — the file stays where it is rather than overwriting what is there.
fn collect_merge_move_ops(dir: &Path, old_root: &Path, new_root: &Path, ops: &mut Vec<(Url, Url)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_merge_move_ops(&path, old_root, new_root, ops);
            continue;
        }

        let Ok(relative) = path.strip_prefix(old_root) else {
            continue;
        };
        let destination = new_root.join(relative);
        if destination.exists() {
            continue;
        }

        if let (Ok(old_url), Ok(new_url)) = (
            Url::from_file_path(&path),
            Url::from_file_path(&destination),
        ) {
            ops.push((old_url, new_url));
        }
    }
}

/// Whether `source` is text that can spell a reference recorded as `name`.
///
/// Normally the span covers the whole name, but a group `use`
/// (`use App\Old\{Foo, Bar};`) records the composed FQN `App\Old\Foo` on a
/// span that covers only the trailing `Foo`, so a segment-aligned tail
/// counts too.  Anything else means the offsets no longer describe the
/// buffer.
fn source_spells_reference(source: &str, name: &str) -> bool {
    source_spells_reference_fully(source, name) || source_spells_reference_as_tail(source, name)
}

/// Whether `source` spells the entirety of `name` (case-insensitively).
fn source_spells_reference_fully(source: &str, name: &str) -> bool {
    strip_fqn_prefix(source).eq_ignore_ascii_case(strip_fqn_prefix(name))
}

/// Whether `source` spells only a segment-aligned tail of `name`, as a
/// group-use member's span does (see [`source_spells_reference`]).
fn source_spells_reference_as_tail(source: &str, name: &str) -> bool {
    let source = strip_fqn_prefix(source);
    let name = strip_fqn_prefix(name);
    name.len()
        .checked_sub(source.len())
        .and_then(|split| name.split_at_checked(split))
        .is_some_and(|(prefix, tail)| prefix.ends_with('\\') && tail.eq_ignore_ascii_case(source))
}

// ─── Namespace segment helpers ──────────────────────────────────────────────

/// Given a namespace name (e.g. `"App\\Bar\\Service"`) and its starting
/// byte offset in the source, find which segment the cursor (byte
/// offset) falls on.
///
/// Returns `(segment_text, segment_start_offset, segment_end_offset)`.
pub(super) fn find_namespace_segment_at_offset(
    ns_name: &str,
    ns_start: u32,
    cursor: u32,
) -> Option<(&str, u32, u32)> {
    let mut offset = ns_start;
    for segment in ns_name.split('\\') {
        let seg_end = offset + segment.len() as u32;
        if cursor >= offset && cursor < seg_end {
            return Some((segment, offset, seg_end));
        }
        // Skip past the segment and the `\` separator.
        offset = seg_end + 1;
    }
    // If cursor is exactly at the end of the last segment, return that.
    let last_seg = ns_name.rsplit('\\').next()?;
    let last_start = ns_start + ns_name.len() as u32 - last_seg.len() as u32;
    let last_end = ns_start + ns_name.len() as u32;
    if cursor == last_end {
        return Some((last_seg, last_start, last_end));
    }
    None
}
