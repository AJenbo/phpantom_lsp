/// Go-to-implementation support (`textDocument/implementation`).
///
/// When the cursor is on an interface name, abstract class name, or a method
/// call where the owning type is an interface or abstract class, this module
/// finds all concrete implementations and returns their locations.
///
/// # Resolution strategy
///
/// 1. **Determine the target symbol** — reuse the same word-extraction and
///    member-access detection logic from go-to-definition.
/// 2. **Identify the target type** — resolve the symbol to a `ClassInfo` and
///    check whether it is an interface or abstract class.
/// 3. **Scan for implementors** — walk all classes known to the server
///    (`ast_map`, `class_index`, `classmap`, PSR-4 directories) and collect
///    those whose `interfaces` list or `parent_class` matches the target type.
/// 4. **Return locations** — for class-level requests, return the class
///    declaration position; for method-level requests, return the method
///    position in each implementing class.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::*;

use super::member::MemberKind;
use crate::Backend;
use crate::types::{ClassInfo, ClassLikeKind, FunctionInfo};

/// Recursively collect all `.php` files under a directory.
///
/// Walks the directory tree rooted at `dir` and returns the paths of all
/// files whose extension is `php`.  Silently skips directories and files
/// that cannot be read (e.g. permission errors, broken symlinks).
fn collect_php_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(collect_php_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "php") {
                result.push(path);
            }
        }
    }
    result
}

impl Backend {
    /// Entry point for `textDocument/implementation`.
    ///
    /// Returns a list of locations where the symbol under the cursor is
    /// concretely implemented.  Returns `None` if the cursor is not on a
    /// resolvable interface/abstract symbol.
    pub(crate) fn resolve_implementation(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<Vec<Location>> {
        // ── 1. Extract the word under the cursor ────────────────────────
        let word = Self::extract_word_at_position(content, position)?;
        if word.is_empty() {
            return None;
        }

        // ── 2. Gather file context ──────────────────────────────────────
        let file_use_map: HashMap<String, String> = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace: Option<String> = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let cursor_offset = Self::position_to_offset(content, position)?;
        let current_class = Self::find_class_at_offset(&classes, cursor_offset).cloned();

        let class_loader = |name: &str| -> Option<ClassInfo> {
            self.resolve_class_name(name, &classes, &file_use_map, &file_namespace)
        };

        let function_loader = |name: &str| -> Option<FunctionInfo> {
            self.resolve_function_name(name, &file_use_map, &file_namespace)
        };

        // ── 3. Check for member access context (->method, ::method) ─────
        let is_member_access = Self::is_member_access_context(content, position);
        if is_member_access {
            return self.resolve_member_implementations(
                uri,
                content,
                position,
                &word,
                &classes,
                current_class.as_ref(),
                &class_loader,
                &function_loader,
                &file_use_map,
                &file_namespace,
            );
        }

        // ── 4. Not a member access — the cursor is on a class/interface name ─
        // Resolve the word to a fully-qualified class name.
        let fqn = Self::resolve_to_fqn(&word, &file_use_map, &file_namespace);
        let target = class_loader(&fqn).or_else(|| class_loader(&word))?;

        // Only interfaces and abstract classes are meaningful targets.
        if target.kind != ClassLikeKind::Interface && !target.is_abstract {
            return None;
        }

        let target_short = target.name.clone();
        let target_fqn = self
            .class_fqn_for_short(&target_short)
            .unwrap_or(target_short.clone());

        let implementors = self.find_implementors(&target_short, &target_fqn, &class_loader);

        if implementors.is_empty() {
            return None;
        }

        let mut locations = Vec::new();
        for imp in &implementors {
            if let Some(loc) = self.locate_class_declaration(imp, uri, content) {
                locations.push(loc);
            }
        }

        if locations.is_empty() {
            None
        } else {
            Some(locations)
        }
    }

    /// Resolve implementations of a method call on an interface/abstract class.
    #[allow(clippy::too_many_arguments)]
    fn resolve_member_implementations(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        member_name: &str,
        classes: &[ClassInfo],
        current_class: Option<&ClassInfo>,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: &dyn Fn(&str) -> Option<FunctionInfo>,
        _file_use_map: &HashMap<String, String>,
        _file_namespace: &Option<String>,
    ) -> Option<Vec<Location>> {
        // Extract the subject (left side of -> or ::).
        let (subject, access_kind) = Self::extract_member_access_context(content, position)?;

        let cursor_offset = Self::position_to_offset(content, position)?;

        // Resolve the subject to candidate classes.
        let candidates = Self::resolve_target_classes(
            &subject,
            access_kind,
            current_class,
            classes,
            content,
            cursor_offset,
            class_loader,
            Some(function_loader),
        );

        if candidates.is_empty() {
            return None;
        }

        // Check if ANY candidate is an interface or abstract class with this
        // method.  If so, find all implementors that have the method.
        let mut all_locations = Vec::new();

        for candidate in &candidates {
            if candidate.kind != ClassLikeKind::Interface && !candidate.is_abstract {
                continue;
            }

            // Verify the method exists on this interface/abstract class
            // (directly or inherited).
            let merged = Self::resolve_class_with_inheritance(candidate, class_loader);
            let has_method = merged.methods.iter().any(|m| m.name == member_name);
            let has_property = merged.properties.iter().any(|p| p.name == member_name);

            if !has_method && !has_property {
                continue;
            }

            let member_kind = if has_method {
                MemberKind::Method
            } else {
                MemberKind::Property
            };

            let target_short = candidate.name.clone();
            let target_fqn = self
                .class_fqn_for_short(&target_short)
                .unwrap_or(target_short.clone());

            let implementors = self.find_implementors(&target_short, &target_fqn, class_loader);

            for imp in &implementors {
                // Check that the implementor actually has this member.
                let imp_merged = Self::resolve_class_with_inheritance(imp, class_loader);
                let imp_has = match member_kind {
                    MemberKind::Method => imp_merged.methods.iter().any(|m| m.name == member_name),
                    MemberKind::Property => {
                        imp_merged.properties.iter().any(|p| p.name == member_name)
                    }
                    MemberKind::Constant => {
                        imp_merged.constants.iter().any(|c| c.name == member_name)
                    }
                };

                if !imp_has {
                    continue;
                }

                // Find the member position in the implementor's file.
                // We want the member defined directly on this class (not
                // inherited), so check the un-merged class first.
                let owns_member = match member_kind {
                    MemberKind::Method => imp.methods.iter().any(|m| m.name == member_name),
                    MemberKind::Property => imp.properties.iter().any(|p| p.name == member_name),
                    MemberKind::Constant => imp.constants.iter().any(|c| c.name == member_name),
                };

                if !owns_member {
                    // The member is inherited — the implementor doesn't
                    // override it, so there's no definition to jump to
                    // in this class.
                    continue;
                }

                if let Some((class_uri, class_content)) =
                    self.find_class_file_content(&imp.name, uri, content)
                    && let Some(member_pos) = Self::find_member_position_in_class(
                        &class_content,
                        member_name,
                        member_kind,
                        imp,
                    )
                    && let Ok(parsed_uri) = Url::parse(&class_uri)
                {
                    let loc = Location {
                        uri: parsed_uri,
                        range: Range {
                            start: member_pos,
                            end: member_pos,
                        },
                    };
                    if !all_locations.contains(&loc) {
                        all_locations.push(loc);
                    }
                }
            }
        }

        // If no interface/abstract candidate was found, try treating the
        // request as a regular "find all overrides" — useful for concrete
        // base-class methods too.
        if all_locations.is_empty() {
            return None;
        }

        Some(all_locations)
    }

    /// Find all classes that implement a given interface or extend a given
    /// abstract class.
    ///
    /// Scans:
    /// 1. All classes already in `ast_map` (open files + autoload-discovered)
    /// 2. All classes loadable via `class_index`
    /// 3. Classmap files not yet loaded — string pre-filter then parse
    /// 4. Embedded PHP stubs — string pre-filter then lazy parse
    /// 5. PSR-4 directories — walk for `.php` files not covered by the
    ///    classmap, string pre-filter then parse
    ///
    /// Returns the list of concrete `ClassInfo` values (non-interface,
    /// non-abstract).
    fn find_implementors(
        &self,
        target_short: &str,
        target_fqn: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        let mut result: Vec<ClassInfo> = Vec::new();
        let mut seen_names = HashSet::new();

        // ── Phase 1: scan ast_map ───────────────────────────────────────
        // Collect all candidate classes first, then drop the lock before
        // calling class_loader (which may re-lock ast_map).
        let ast_candidates: Vec<ClassInfo> = if let Ok(map) = self.ast_map.lock() {
            map.values()
                .flat_map(|classes| classes.iter().cloned())
                .collect()
        } else {
            Vec::new()
        };

        for cls in &ast_candidates {
            if self.class_implements_or_extends(cls, target_short, target_fqn, class_loader)
                && seen_names.insert(cls.name.clone())
            {
                result.push(cls.clone());
            }
        }

        // ── Phase 2: scan class_index for classes not yet in ast_map ────
        let index_entries: Vec<(String, String)> = self
            .class_index
            .lock()
            .ok()
            .map(|idx| {
                idx.iter()
                    .map(|(fqn, uri)| (fqn.clone(), uri.clone()))
                    .collect()
            })
            .unwrap_or_default();

        for (fqn, _uri) in &index_entries {
            let short = fqn.rsplit('\\').next().unwrap_or(fqn);
            if seen_names.contains(short) {
                continue;
            }
            if let Some(cls) = class_loader(fqn)
                && self.class_implements_or_extends(&cls, target_short, target_fqn, class_loader)
                && seen_names.insert(cls.name.clone())
            {
                result.push(cls);
            }
        }

        // ── Phase 3: scan classmap files with string pre-filter ─────────
        // Collect unique file paths from the classmap (one file may define
        // multiple classes, so we de-duplicate by path and scan each file
        // at most once).  Files already present in ast_map were covered by
        // Phase 1 and can be skipped.
        let classmap_paths: HashSet<PathBuf> = self
            .classmap
            .lock()
            .ok()
            .map(|cm| cm.values().cloned().collect())
            .unwrap_or_default();

        let loaded_uris: HashSet<String> = self
            .ast_map
            .lock()
            .ok()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        for path in &classmap_paths {
            let uri = format!("file://{}", path.display());
            if loaded_uris.contains(&uri) {
                continue;
            }

            // Cheap pre-filter: read the raw file and skip it if the
            // source doesn't mention the target name at all.
            let raw = match std::fs::read_to_string(path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if !raw.contains(target_short) {
                continue;
            }

            // Parse the file, cache it, and check every class it defines.
            if let Some(classes) = self.parse_and_cache_file(path) {
                for cls in &classes {
                    if seen_names.contains(&cls.name) {
                        continue;
                    }
                    if self.class_implements_or_extends(cls, target_short, target_fqn, class_loader)
                    {
                        seen_names.insert(cls.name.clone());
                        result.push(cls.clone());
                    }
                }
            }
        }

        // ── Phase 4: scan embedded stubs with string pre-filter ─────────
        // Stubs are static strings baked into the binary.  A cheap text
        // search for the target name narrows candidates before we parse.
        // Parsing is lazy and cached in ast_map, so subsequent lookups
        // hit Phase 1.
        for (&stub_name, &stub_source) in &self.stub_index {
            if seen_names.contains(stub_name) {
                continue;
            }
            // Cheap pre-filter: skip stubs whose source doesn't mention
            // the target name at all.
            if !stub_source.contains(target_short) {
                continue;
            }
            if let Some(cls) = class_loader(stub_name)
                && self.class_implements_or_extends(&cls, target_short, target_fqn, class_loader)
                && seen_names.insert(cls.name.clone())
            {
                result.push(cls);
            }
        }

        // ── Phase 5: scan PSR-4 directories for files not in classmap ───
        // The user may have created classes that are not yet in the
        // classmap (e.g. they haven't run `composer dump-autoload -o`).
        // Walk every PSR-4 root directory, skip files already covered by
        // the classmap or already loaded, then apply the same string
        // pre-filter → parse → check pipeline.
        if let Some(workspace_root) = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
        {
            let psr4_dirs: Vec<PathBuf> = self
                .psr4_mappings
                .lock()
                .ok()
                .map(|mappings| {
                    mappings
                        .iter()
                        .map(|m| workspace_root.join(&m.base_path))
                        .filter(|p| p.is_dir())
                        .collect()
                })
                .unwrap_or_default();

            // Refresh loaded URIs — Phase 3 may have added entries.
            let loaded_uris_p5: HashSet<String> = self
                .ast_map
                .lock()
                .ok()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();

            for dir in &psr4_dirs {
                for php_file in collect_php_files(dir) {
                    // Skip files already covered by the classmap (Phase 3).
                    if classmap_paths.contains(&php_file) {
                        continue;
                    }

                    let uri = format!("file://{}", php_file.display());
                    if loaded_uris_p5.contains(&uri) {
                        continue;
                    }

                    let raw = match std::fs::read_to_string(&php_file) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };
                    if !raw.contains(target_short) {
                        continue;
                    }

                    if let Some(classes) = self.parse_and_cache_file(&php_file) {
                        for cls in &classes {
                            if seen_names.contains(&cls.name) {
                                continue;
                            }
                            if self.class_implements_or_extends(
                                cls,
                                target_short,
                                target_fqn,
                                class_loader,
                            ) {
                                seen_names.insert(cls.name.clone());
                                result.push(cls.clone());
                            }
                        }
                    }
                }
            }
        }

        result
    }

    /// Check whether `cls` implements the target interface or extends the
    /// target abstract class (directly or transitively through its parent
    /// chain).
    fn class_implements_or_extends(
        &self,
        cls: &ClassInfo,
        target_short: &str,
        target_fqn: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> bool {
        // Skip the target class itself.
        if cls.name == target_short {
            return false;
        }

        // Skip interfaces and abstract classes — we want concrete implementations.
        if cls.kind == ClassLikeKind::Interface || cls.is_abstract {
            return false;
        }

        // Direct `implements` match.
        for iface in &cls.interfaces {
            let iface_short = iface.rsplit('\\').next().unwrap_or(iface);
            if iface_short == target_short || iface == target_fqn {
                return true;
            }
        }

        // Direct `extends` match (for abstract class implementations).
        if let Some(ref parent) = cls.parent_class {
            let parent_short = parent.rsplit('\\').next().unwrap_or(parent);
            if parent_short == target_short || parent == target_fqn {
                return true;
            }
        }

        // ── Transitive check: walk the parent chain ─────────────────────
        // A class might extend another class that implements the target
        // interface.  Walk up to a bounded depth to find it.
        const MAX_DEPTH: usize = 10;
        let mut current = cls.parent_class.clone();
        let mut depth = 0;

        while let Some(ref parent_name) = current {
            if depth >= MAX_DEPTH {
                break;
            }
            depth += 1;

            if let Some(parent_cls) = class_loader(parent_name) {
                // Check if the parent implements the target interface.
                for iface in &parent_cls.interfaces {
                    let iface_short = iface.rsplit('\\').next().unwrap_or(iface);
                    if iface_short == target_short || iface == target_fqn {
                        return true;
                    }
                }

                // Check if the parent IS the target (for abstract class chains).
                let pshort = parent_cls.name.as_str();
                if pshort == target_short {
                    return true;
                }

                current = parent_cls.parent_class.clone();
            } else {
                break;
            }
        }

        false
    }

    /// Find a member position scoped to a specific class body.
    ///
    /// When multiple classes in the same file define a method with the same
    /// name, [`find_member_position`](Self::find_member_position) would
    /// always return the first match.  This variant restricts the search
    /// to lines that fall within the class's `start_offset..end_offset`
    /// byte range so that each implementing class resolves to its own
    /// definition.
    fn find_member_position_in_class(
        content: &str,
        member_name: &str,
        kind: MemberKind,
        cls: &ClassInfo,
    ) -> Option<Position> {
        // Convert byte offsets to line numbers.
        let start_line = content
            .get(..cls.start_offset as usize)
            .map(|s| s.matches('\n').count())
            .unwrap_or(0);
        let end_line = content
            .get(..cls.end_offset as usize)
            .map(|s| s.matches('\n').count())
            .unwrap_or(usize::MAX);

        // Build a sub-content containing only the class body lines and
        // delegate to the existing searcher, adjusting the result line.
        let class_lines: Vec<&str> = content
            .lines()
            .skip(start_line)
            .take(end_line - start_line + 1)
            .collect();
        let class_body = class_lines.join("\n");

        Self::find_member_position(&class_body, member_name, kind).map(|pos| Position {
            line: pos.line + start_line as u32,
            character: pos.character,
        })
    }

    /// Get the FQN for a class given its short name, by looking it up in
    /// the `class_index`.
    fn class_fqn_for_short(&self, short_name: &str) -> Option<String> {
        let idx = self.class_index.lock().ok()?;
        // Look for an entry whose short name matches.
        for fqn in idx.keys() {
            let short = fqn.rsplit('\\').next().unwrap_or(fqn);
            if short == short_name {
                return Some(fqn.clone());
            }
        }
        None
    }

    /// Parse a PHP file, cache the results in `ast_map`/`use_map`/`namespace_map`,
    /// and return the extracted classes.
    ///
    /// This follows the same caching pattern as [`find_or_load_class`] Phases
    /// 1.5 and 2 so that subsequent calls to the class loader can find the
    /// parsed classes via the ast_map without re-reading the file.
    fn parse_and_cache_file(&self, file_path: &Path) -> Option<Vec<ClassInfo>> {
        let content = std::fs::read_to_string(file_path).ok()?;
        let mut classes = self.parse_php(&content);
        let file_use_map = self.parse_use_statements(&content);
        let file_namespace = self.parse_namespace(&content);
        Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

        let uri = format!("file://{}", file_path.display());
        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri.clone(), classes.clone());
        }
        if let Ok(mut map) = self.use_map.lock() {
            map.insert(uri.clone(), file_use_map);
        }
        if let Ok(mut map) = self.namespace_map.lock() {
            map.insert(uri, file_namespace);
        }

        Some(classes)
    }

    /// Find the location of a class declaration for an implementor.
    fn locate_class_declaration(
        &self,
        cls: &ClassInfo,
        current_uri: &str,
        current_content: &str,
    ) -> Option<Location> {
        let (class_uri, class_content) =
            self.find_class_file_content(&cls.name, current_uri, current_content)?;

        let position = Self::find_definition_position(&class_content, &cls.name)?;
        let parsed_uri = Url::parse(&class_uri).ok()?;

        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }
}
