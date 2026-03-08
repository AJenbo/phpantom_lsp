//! Find References (`textDocument/references`) support.
//!
//! When the user invokes "Find All References" on a symbol, the LSP
//! collects every occurrence of that symbol across the project.
//!
//! **Same-file references** are answered from the precomputed
//! [`SymbolMap`] — we iterate all spans and collect those that match
//! the symbol under the cursor.
//!
//! **Cross-file references** iterate every `SymbolMap` stored in
//! `self.symbol_maps` (one per opened / parsed file).  For files that
//! are in the workspace but have not been opened yet, we lazily parse
//! them on demand (via the classmap, PSR-4, and workspace scan).
//!
//! **Variable references** (including `$this`) are strictly scoped to
//! the enclosing function / method / closure body within the current
//! file.

use std::collections::HashSet;

use tower_lsp::lsp_types::{Location, Position, Range, Url};

use crate::Backend;
use crate::symbol_map::{SymbolKind, SymbolMap};
use crate::util::{collect_php_files_gitignore, offset_to_position, position_to_offset};

impl Backend {
    /// Entry point for `textDocument/references`.
    ///
    /// Returns all locations where the symbol under the cursor is
    /// referenced.  When `include_declaration` is true the declaration
    /// site itself is included in the results.
    pub(crate) fn find_references(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let offset = position_to_offset(content, position);

        // Consult the precomputed symbol map for the current file.
        let symbol = self.lookup_symbol_map(uri, offset).or_else(|| {
            if offset > 0 {
                self.lookup_symbol_map(uri, offset - 1)
            } else {
                None
            }
        });

        // When the cursor is on a symbol span, dispatch by kind.
        if let Some(ref sym) = symbol {
            let locations = self.dispatch_symbol_references(
                &sym.kind,
                uri,
                content,
                sym.start,
                sym.end,
                include_declaration,
            );
            return if locations.is_empty() {
                None
            } else {
                Some(locations)
            };
        }

        None
    }

    /// Dispatch a symbol-map hit to the appropriate reference finder.
    fn dispatch_symbol_references(
        &self,
        kind: &SymbolKind,
        uri: &str,
        content: &str,
        span_start: u32,
        span_end: u32,
        include_declaration: bool,
    ) -> Vec<Location> {
        match kind {
            SymbolKind::Variable { name } => {
                // Property declarations use Variable spans (so GTD can
                // jump to the type hint), but Find References should
                // search for member accesses, not local variable uses.
                if let Some(crate::symbol_map::VarDefKind::Property) =
                    self.lookup_var_def_kind_at(uri, name, span_start)
                {
                    // Properties are never static in the Variable span
                    // context ($this->prop).  Static properties use
                    // MemberAccess spans at their usage sites with
                    // is_static=true, but the declaration-site Variable
                    // span doesn't encode static-ness.  Check the
                    // ast_map to determine the correct flag.
                    let is_static = self
                        .get_classes_for_uri(uri)
                        .iter()
                        .flat_map(|classes| classes.iter())
                        .flat_map(|c| c.properties.iter())
                        .any(|p| {
                            let p_name = p.name.strip_prefix('$').unwrap_or(&p.name);
                            p_name == name && p.is_static
                        });
                    return self.find_member_references(name, is_static, include_declaration);
                }
                self.find_variable_references(uri, content, name, span_start, include_declaration)
            }
            SymbolKind::ClassReference { name, is_fqn } => {
                let ctx = self.file_context(uri);
                let fqn = if *is_fqn {
                    name.clone()
                } else {
                    Self::resolve_to_fqn(name, &ctx.use_map, &ctx.namespace)
                };
                self.find_class_references(&fqn, include_declaration)
            }
            SymbolKind::ClassDeclaration { name } => {
                let ctx = self.file_context(uri);
                let fqn = if let Some(ref ns) = ctx.namespace {
                    format!("{}\\{}", ns, name)
                } else {
                    name.clone()
                };
                self.find_class_references(&fqn, include_declaration)
            }
            SymbolKind::MemberAccess {
                member_name,
                is_static,
                ..
            } => self.find_member_references(member_name, *is_static, include_declaration),
            SymbolKind::FunctionCall { name } => {
                let ctx = self.file_context(uri);
                let fqn = Self::resolve_to_fqn(name, &ctx.use_map, &ctx.namespace);
                self.find_function_references(&fqn, name, include_declaration)
            }
            SymbolKind::ConstantReference { name } => {
                self.find_constant_references(name, include_declaration)
            }
            SymbolKind::MemberDeclaration { name, is_static } => {
                self.find_member_references(name, *is_static, include_declaration)
            }
            SymbolKind::SelfStaticParent { keyword } => {
                // `$this` is recorded as SelfStaticParent { keyword: "static" }.
                // Treat it as a file-local variable, not a cross-file class search.
                //
                // We detect `$this` by checking the actual span source text
                // (not the cursor offset, which may land in the middle of
                // the token).
                if keyword == "static"
                    && content
                        .get(span_start as usize..span_end as usize)
                        .is_some_and(|s| s == "$this")
                {
                    return self.find_this_references(
                        uri,
                        content,
                        span_start,
                        include_declaration,
                    );
                }

                // For real self/static/parent keywords, resolve to the class FQN.
                let ctx = self.file_context(uri);
                let current_class = crate::util::find_class_at_offset(&ctx.classes, span_start);
                let fqn = match keyword.as_str() {
                    "parent" => current_class
                        .and_then(|cc| cc.parent_class.as_ref())
                        .cloned(),
                    _ => current_class.map(|cc| {
                        if let Some(ref ns) = ctx.namespace {
                            format!("{}\\{}", ns, &cc.name)
                        } else {
                            cc.name.clone()
                        }
                    }),
                };
                if let Some(fqn) = fqn {
                    self.find_class_references(&fqn, include_declaration)
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Find all references to a variable within its enclosing scope.
    ///
    /// Variables are file-local and scope-local — a `$user` in method A
    /// must not match `$user` in method B.
    fn find_variable_references(
        &self,
        uri: &str,
        content: &str,
        var_name: &str,
        cursor_offset: u32,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        let maps = self.symbol_maps.read();
        let Some(symbol_map) = maps.get(uri) else {
            return locations;
        };

        let scope_start = symbol_map.find_enclosing_scope(cursor_offset);

        let parsed_uri = match Url::parse(uri) {
            Ok(u) => u,
            Err(_) => return locations,
        };

        for span in &symbol_map.spans {
            if let SymbolKind::Variable { name } = &span.kind {
                if name != var_name {
                    continue;
                }
                // Check that this variable is in the same scope.
                let span_scope = symbol_map.find_enclosing_scope(span.start);
                if span_scope != scope_start {
                    continue;
                }
                // Optionally skip declaration sites.
                if !include_declaration && symbol_map.var_def_kind_at(name, span.start).is_some() {
                    continue;
                }
                let start = offset_to_position(content, span.start as usize);
                let end = offset_to_position(content, span.end as usize);
                locations.push(Location {
                    uri: parsed_uri.clone(),
                    range: Range { start, end },
                });
            }
        }

        // Also include var_def sites if include_declaration is set,
        // since some definition tokens (parameters, foreach bindings)
        // may not have a corresponding Variable span in the spans vec
        // with the exact same offset.
        if include_declaration {
            let mut seen_offsets: HashSet<u32> = locations
                .iter()
                .map(|loc| position_to_offset(content, loc.range.start))
                .collect();

            for def in &symbol_map.var_defs {
                if def.name == var_name
                    && def.scope_start == scope_start
                    && seen_offsets.insert(def.offset)
                {
                    let start = offset_to_position(content, def.offset as usize);
                    // The token is `$` + name.
                    let end_offset = def.offset as usize + 1 + def.name.len();
                    let end = offset_to_position(content, end_offset);
                    locations.push(Location {
                        uri: parsed_uri.clone(),
                        range: Range { start, end },
                    });
                }
            }
        }

        // Sort by position for stable output.
        locations.sort_by(|a, b| {
            a.range
                .start
                .line
                .cmp(&b.range.start.line)
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        locations
    }

    /// Find all references to `$this` within the enclosing class body
    /// in the current file.
    ///
    /// `$this` is semantically a variable scoped to the enclosing class,
    /// not a cross-file class reference.  We match every
    /// `SelfStaticParent { keyword: "static" }` span whose source text
    /// is `$this`, as well as `MemberAccess` spans whose `subject_text`
    /// is `"$this"` (for the `$this` part of `$this->method()`), within
    /// the same class body.
    fn find_this_references(
        &self,
        uri: &str,
        content: &str,
        cursor_offset: u32,
        include_declaration: bool,
    ) -> Vec<Location> {
        let _ = include_declaration; // $this has no "declaration site"
        let mut locations = Vec::new();

        let maps = self.symbol_maps.read();
        let Some(symbol_map) = maps.get(uri) else {
            return locations;
        };

        // Determine the class body the cursor is in.
        let ctx_classes = self.ast_map.read().get(uri).cloned().unwrap_or_default();
        let current_class = crate::util::find_class_at_offset(&ctx_classes, cursor_offset);
        let (class_start, class_end) = match current_class {
            Some(cc) => (cc.start_offset, cc.end_offset),
            None => return locations,
        };

        let parsed_uri = match Url::parse(uri) {
            Ok(u) => u,
            Err(_) => return locations,
        };

        for span in &symbol_map.spans {
            // Only consider spans within the same class body.
            if span.start < class_start || span.start > class_end {
                continue;
            }

            let is_this = match &span.kind {
                SymbolKind::SelfStaticParent { keyword } if keyword == "static" => {
                    // Check the actual source text to distinguish
                    // `$this` from the `static` keyword.
                    content
                        .get(span.start as usize..span.end as usize)
                        .is_some_and(|s| s == "$this")
                }
                _ => false,
            };

            if is_this {
                let start = offset_to_position(content, span.start as usize);
                let end = offset_to_position(content, span.end as usize);
                locations.push(Location {
                    uri: parsed_uri.clone(),
                    range: Range { start, end },
                });
            }
        }

        locations.sort_by(|a, b| {
            a.range
                .start
                .line
                .cmp(&b.range.start.line)
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        locations
    }

    /// Snapshot all symbol maps for user (non-vendor, non-stub) files.
    ///
    /// Ensures the workspace is indexed first, then returns a cloned
    /// snapshot of every symbol map whose URI does not fall under the
    /// vendor directory or the internal stub scheme.  All four cross-file
    /// reference scanners use this to restrict results to user code.
    fn user_file_symbol_maps(&self) -> Vec<(String, SymbolMap)> {
        self.ensure_workspace_indexed();

        let vendor_prefix = self.vendor_uri_prefix.lock().clone();

        let maps = self.symbol_maps.read();
        maps.iter()
            .filter(|(uri, _)| {
                !uri.starts_with("phpantom-stub://")
                    && !uri.starts_with("phpantom-stub-fn://")
                    && (vendor_prefix.is_empty() || !uri.starts_with(vendor_prefix.as_str()))
            })
            .map(|(uri, map)| (uri.clone(), map.clone()))
            .collect()
    }

    /// Find all references to a class/interface/trait/enum across all files.
    ///
    /// Matches `ClassReference` spans whose resolved FQN equals `target_fqn`,
    /// and optionally `ClassDeclaration` spans at the declaration site.
    fn find_class_references(&self, target_fqn: &str, include_declaration: bool) -> Vec<Location> {
        let mut locations = Vec::new();

        // Normalise: strip leading backslash if present.
        let target = target_fqn.strip_prefix('\\').unwrap_or(target_fqn);
        let target_short = crate::util::short_name(target);

        // Snapshot user-file symbol maps (excludes vendor and stubs).
        let snapshot = self.user_file_symbol_maps();

        for (file_uri, symbol_map) in &snapshot {
            // Get the file's use_map and namespace for FQN resolution.
            let file_use_map = self
                .use_map
                .read()
                .get(file_uri)
                .cloned()
                .unwrap_or_default();
            let file_namespace = self.namespace_map.read().get(file_uri).cloned().flatten();

            let parsed_uri = match Url::parse(file_uri) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let content = match self.get_file_content(file_uri) {
                Some(c) => c,
                None => continue,
            };

            for span in &symbol_map.spans {
                match &span.kind {
                    SymbolKind::ClassReference { name, is_fqn } => {
                        let resolved = if *is_fqn {
                            name.clone()
                        } else {
                            Self::resolve_to_fqn(name, &file_use_map, &file_namespace)
                        };
                        let resolved_normalized = resolved.strip_prefix('\\').unwrap_or(&resolved);
                        if !class_names_match(resolved_normalized, target, target_short) {
                            continue;
                        }
                        let start = offset_to_position(&content, span.start as usize);
                        let end = offset_to_position(&content, span.end as usize);
                        locations.push(Location {
                            uri: parsed_uri.clone(),
                            range: Range { start, end },
                        });
                    }
                    SymbolKind::ClassDeclaration { name } if include_declaration => {
                        let fqn = if let Some(ref ns) = file_namespace {
                            format!("{}\\{}", ns, name)
                        } else {
                            name.clone()
                        };
                        if !class_names_match(&fqn, target, target_short) {
                            continue;
                        }
                        let start = offset_to_position(&content, span.start as usize);
                        let end = offset_to_position(&content, span.end as usize);
                        locations.push(Location {
                            uri: parsed_uri.clone(),
                            range: Range { start, end },
                        });
                    }
                    SymbolKind::SelfStaticParent { keyword } => {
                        // self/static/parent resolve to the current class —
                        // include them if they resolve to the target FQN.
                        //
                        // Skip `$this` — it is handled as a variable, not a
                        // class reference.
                        if keyword == "static"
                            && content
                                .get(span.start as usize..span.end as usize)
                                .is_some_and(|s| s == "$this")
                        {
                            continue;
                        }
                        if let Some(fqn) = self.resolve_keyword_to_fqn(
                            keyword,
                            file_uri,
                            &file_namespace,
                            span.start,
                        ) {
                            let fqn_normalized = fqn.strip_prefix('\\').unwrap_or(&fqn);
                            if class_names_match(fqn_normalized, target, target_short) {
                                let start = offset_to_position(&content, span.start as usize);
                                let end = offset_to_position(&content, span.end as usize);
                                locations.push(Location {
                                    uri: parsed_uri.clone(),
                                    range: Range { start, end },
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Sort: by URI, then by position.
        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        locations
    }

    /// Find all references to a member (method or property) across all files.
    ///
    /// For v1, matching is by member name and static-ness only (no subject
    /// type resolution).  This may produce false positives when unrelated
    /// classes have members with the same name, but it is simple and fast.
    fn find_member_references(
        &self,
        target_member: &str,
        target_is_static: bool,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        let snapshot = self.user_file_symbol_maps();

        for (file_uri, symbol_map) in &snapshot {
            let parsed_uri = match Url::parse(file_uri) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let content = match self.get_file_content(file_uri) {
                Some(c) => c,
                None => continue,
            };

            for span in &symbol_map.spans {
                let matches = match &span.kind {
                    SymbolKind::MemberAccess {
                        member_name,
                        is_static,
                        ..
                    } => member_name == target_member && *is_static == target_is_static,
                    SymbolKind::MemberDeclaration { name, is_static } if include_declaration => {
                        name == target_member && *is_static == target_is_static
                    }
                    _ => false,
                };
                if matches {
                    let start = offset_to_position(&content, span.start as usize);
                    let end = offset_to_position(&content, span.end as usize);
                    locations.push(Location {
                        uri: parsed_uri.clone(),
                        range: Range { start, end },
                    });
                }
            }

            // Property declarations use Variable spans (not
            // MemberDeclaration) because GTD relies on the Variable
            // kind to jump to the type hint.  Scan the ast_map to
            // pick up property declaration sites.
            if include_declaration && let Some(classes) = self.get_classes_for_uri(file_uri) {
                for class in &classes {
                    for prop in &class.properties {
                        let prop_name = prop.name.strip_prefix('$').unwrap_or(&prop.name);
                        let target_name = target_member.strip_prefix('$').unwrap_or(target_member);
                        if prop_name == target_name
                            && prop.is_static == target_is_static
                            && prop.name_offset != 0
                        {
                            let offset = prop.name_offset;
                            let start = offset_to_position(&content, offset as usize);
                            let end =
                                offset_to_position(&content, offset as usize + prop.name.len());
                            push_unique_location(&mut locations, &parsed_uri, start, end);
                        }
                    }
                }
            }
        }

        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        locations
    }

    /// Find all references to a standalone function across all files.
    fn find_function_references(
        &self,
        target_fqn: &str,
        target_short: &str,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        let target = target_fqn.strip_prefix('\\').unwrap_or(target_fqn);

        let snapshot = self.user_file_symbol_maps();

        for (file_uri, symbol_map) in &snapshot {
            let file_use_map = self
                .use_map
                .read()
                .get(file_uri)
                .cloned()
                .unwrap_or_default();
            let file_namespace = self.namespace_map.read().get(file_uri).cloned().flatten();

            let parsed_uri = match Url::parse(file_uri) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let content = match self.get_file_content(file_uri) {
                Some(c) => c,
                None => continue,
            };

            for span in &symbol_map.spans {
                if let SymbolKind::FunctionCall { name } = &span.kind {
                    // Resolve the function call name to its FQN in the
                    // context of the file where it appears.
                    let resolved = Self::resolve_to_fqn(name, &file_use_map, &file_namespace);
                    let resolved_normalized = resolved.strip_prefix('\\').unwrap_or(&resolved);

                    // Match by FQN or by short name (for global functions
                    // that may be called without a namespace prefix).
                    if resolved_normalized == target
                        || (name == target_short && !target.contains('\\'))
                    {
                        let start = offset_to_position(&content, span.start as usize);
                        let end = offset_to_position(&content, span.end as usize);
                        locations.push(Location {
                            uri: parsed_uri.clone(),
                            range: Range { start, end },
                        });
                    }
                }
            }

            // Include the function declaration site if requested.
            if include_declaration {
                let fmap = self.global_functions.read();
                if let Some((func_uri, func_info)) = fmap.get(target)
                    && func_uri == file_uri
                    && func_info.name_offset != 0
                {
                    let offset = func_info.name_offset;
                    let start = offset_to_position(&content, offset as usize);
                    let end = offset_to_position(&content, offset as usize + func_info.name.len());
                    push_unique_location(&mut locations, &parsed_uri, start, end);
                }
            }
        }

        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        locations
    }

    /// Find all references to a constant across all files.
    fn find_constant_references(
        &self,
        target_name: &str,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        let snapshot = self.user_file_symbol_maps();

        for (file_uri, symbol_map) in &snapshot {
            let parsed_uri = match Url::parse(file_uri) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let content = match self.get_file_content(file_uri) {
                Some(c) => c,
                None => continue,
            };

            for span in &symbol_map.spans {
                if let SymbolKind::ConstantReference { name } = &span.kind
                    && name == target_name
                {
                    let start = offset_to_position(&content, span.start as usize);
                    let end = offset_to_position(&content, span.end as usize);
                    locations.push(Location {
                        uri: parsed_uri.clone(),
                        range: Range { start, end },
                    });
                }
            }

            // Include define() declaration sites if requested.
            if include_declaration {
                let dmap = self.global_defines.read();
                if let Some(info) = dmap.get(target_name)
                    && info.file_uri == *file_uri
                {
                    let start = offset_to_position(&content, info.name_offset as usize);
                    let end =
                        offset_to_position(&content, info.name_offset as usize + target_name.len());
                    push_unique_location(&mut locations, &parsed_uri, start, end);
                }
            }
        }

        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });

        locations
    }

    /// Resolve a `self`/`static`/`parent` keyword to the FQN of the class
    /// it refers to in the given file and offset context.
    fn resolve_keyword_to_fqn(
        &self,
        keyword: &str,
        uri: &str,
        namespace: &Option<String>,
        offset: u32,
    ) -> Option<String> {
        let classes = self.ast_map.read().get(uri).cloned().unwrap_or_default();

        let current_class = crate::util::find_class_at_offset(&classes, offset)?;

        match keyword {
            "parent" => current_class.parent_class.clone(),
            _ => {
                // self / static → current class FQN
                Some(if let Some(ns) = namespace {
                    format!("{}\\{}", ns, &current_class.name)
                } else {
                    current_class.name.clone()
                })
            }
        }
    }

    /// Ensure all workspace PHP files have been parsed and have symbol maps.
    ///
    /// This lazily parses files that are in the workspace directory but
    /// have not been opened or indexed yet.  It also covers files known
    /// via the classmap and class_index.  The vendor directory (read from
    /// `composer.json` `config.vendor-dir`, defaulting to `vendor`) is
    /// skipped during the filesystem walk.
    fn ensure_workspace_indexed(&self) {
        // Snapshot ast_map keys before scanning so we can evict
        // transiently-loaded entries afterwards (see §13 in bugs.md).
        let pre_scan_uris: HashSet<String> = self.ast_map.read().keys().cloned().collect();

        // Collect URIs that already have symbol maps.
        let existing_uris: HashSet<String> = self.symbol_maps.read().keys().cloned().collect();

        // Build the vendor URI prefix so we can skip vendor files in
        // Phase 1 (class_index may contain vendor URIs from prior
        // resolution, but we only need symbol maps for user files).
        let vendor_prefix = self.vendor_uri_prefix.lock().clone();

        // ── Phase 1: class_index files (user only) ─────────────────────
        // These are files we already know about from update_ast calls,
        // ensuring their symbol maps are populated.  Vendor files are
        // skipped — find references only reports user code.
        let index_uris: Vec<String> = self.class_index.read().values().cloned().collect();

        for uri in &index_uris {
            if existing_uris.contains(uri) {
                continue;
            }
            if !vendor_prefix.is_empty() && uri.starts_with(vendor_prefix.as_str()) {
                continue;
            }
            if uri.starts_with("phpantom-stub://") || uri.starts_with("phpantom-stub-fn://") {
                continue;
            }
            if let Some(content) = self.get_file_content(uri) {
                self.update_ast(uri, &content);
            }
        }

        // ── Phase 2: workspace directory scan ───────────────────────────
        // Recursively discover PHP files in the workspace root that are
        // not yet indexed.  This catches files that are not in the
        // classmap, class_index, or already opened.  The vendor directory
        // is skipped — find references only reports user code.  The walk
        // respects .gitignore so that generated/cached directories (e.g.
        // storage/framework/views/, var/cache/, node_modules/) are
        // automatically excluded.
        let workspace_root = self.workspace_root.read().clone();

        if let Some(root) = workspace_root {
            let vendor_dir_name = self.vendor_dir_name.lock().clone();

            // Re-read existing URIs after phase 1 may have added more.
            let existing_uris: HashSet<String> = self.symbol_maps.read().keys().cloned().collect();

            let php_files = collect_php_files_gitignore(&root, &vendor_dir_name);
            for path in &php_files {
                let uri = format!("file://{}", path.display());
                if existing_uris.contains(&uri) {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(path) {
                    self.update_ast(&uri, &content);
                }
            }
        }

        // ── Evict transient ast_map entries ─────────────────────────────
        // The scan above may have added files to ast_map, use_map, and
        // namespace_map that are not open in the editor.  Symbol maps are
        // preserved (they are the whole point of this scan), but the other
        // maps can be evicted to save memory.
        self.evict_transient_ast_entries(&pre_scan_uris);
    }
}

/// Check whether a resolved class name matches the target FQN.
///
/// Two names match if their fully-qualified forms are equal, or if both
/// are unqualified and their short names match.
fn class_names_match(resolved: &str, target: &str, target_short: &str) -> bool {
    if resolved == target {
        return true;
    }
    // When neither name is qualified, compare short names.
    if !resolved.contains('\\') && !target.contains('\\') {
        return resolved == target_short;
    }
    // Compare short names as a fallback — the resolved name's short
    // segment must match.  This handles cases where one side has a
    // namespace and the other doesn't (common for global classes).
    let resolved_short = crate::util::short_name(resolved);
    resolved_short == target_short && (resolved == target || !target.contains('\\'))
}

/// Push a location only if it is not already present (deduplication).
fn push_unique_location(locations: &mut Vec<Location>, uri: &Url, start: Position, end: Position) {
    let already_present = locations.iter().any(|l| {
        l.uri == *uri
            && l.range.start.line == start.line
            && l.range.start.character == start.character
    });
    if !already_present {
        locations.push(Location {
            uri: uri.clone(),
            range: Range { start, end },
        });
    }
}

#[cfg(test)]
mod tests;
