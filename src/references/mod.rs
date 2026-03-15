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
//!
//! **Member references** (methods, properties, constants) are filtered
//! by the class hierarchy of the target member.  When the user triggers
//! "Find References" on `MyClass::save()`, only accesses where the
//! subject resolves to a class in the same inheritance tree are returned.
//! Accesses on unrelated classes that happen to have a member with the
//! same name are excluded.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use tower_lsp::lsp_types::{Location, Position, Range, Url};

use crate::Backend;
use crate::symbol_map::{SymbolKind, SymbolMap};
use crate::types::{ClassInfo, MAX_INHERITANCE_DEPTH};
use crate::util::{
    collect_php_files_gitignore, find_class_at_offset, offset_to_position, position_to_offset,
};

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

                    // Resolve the enclosing class to scope the search.
                    let hierarchy = self.resolve_member_declaration_hierarchy(uri, span_start);
                    return self.find_member_references(
                        name,
                        is_static,
                        include_declaration,
                        hierarchy.as_ref(),
                    );
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
                subject_text,
                member_name,
                is_static,
                ..
            } => {
                // Resolve the subject to determine the class hierarchy
                // so we only return references on related classes.
                let hierarchy =
                    self.resolve_member_access_hierarchy(uri, subject_text, *is_static, span_start);
                self.find_member_references(
                    member_name,
                    *is_static,
                    include_declaration,
                    hierarchy.as_ref(),
                )
            }
            SymbolKind::FunctionCall { name, .. } => {
                let ctx = self.file_context(uri);
                let fqn = Self::resolve_to_fqn(name, &ctx.use_map, &ctx.namespace);
                self.find_function_references(&fqn, name, include_declaration)
            }
            SymbolKind::ConstantReference { name } => {
                self.find_constant_references(name, include_declaration)
            }
            SymbolKind::MemberDeclaration { name, is_static } => {
                // Resolve the enclosing class to scope the search.
                let hierarchy = self.resolve_member_declaration_hierarchy(uri, span_start);
                self.find_member_references(
                    name,
                    *is_static,
                    include_declaration,
                    hierarchy.as_ref(),
                )
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
        let symbol_map = match maps.get(uri) {
            Some(m) => m,
            None => return locations,
        };

        // Determine the enclosing scope for the variable.
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

    /// Find all references to `$this` within the enclosing class body.
    ///
    /// `$this` is scoped to the enclosing class — it must not match
    /// `$this` in a different class or top-level function.  Unlike
    /// regular variables, `$this` is **not** scoped to the enclosing
    /// method: `$this` in method A and `$this` in method B inside the
    /// same class both refer to the same object, so they should all
    /// appear in the results.
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
        let symbol_map = match maps.get(uri) {
            Some(m) => m,
            None => return locations,
        };

        // Determine the class body the cursor is in.
        let ctx_classes: Vec<Arc<ClassInfo>> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();
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
    fn user_file_symbol_maps(&self) -> Vec<(String, Arc<SymbolMap>)> {
        self.ensure_workspace_indexed();

        let vendor_prefixes = self.vendor_uri_prefixes.lock().clone();

        let maps = self.symbol_maps.read();
        maps.iter()
            .filter(|(uri, _)| {
                !uri.starts_with("phpantom-stub://")
                    && !uri.starts_with("phpantom-stub-fn://")
                    && !vendor_prefixes.iter().any(|p| uri.starts_with(p.as_str()))
            })
            .map(|(uri, map)| (uri.clone(), Arc::clone(map)))
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

            let content = match self.get_file_content_arc(file_uri) {
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
                        // Input boundary: resolve_to_fqn may return a leading `\`.
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
                        ) && class_names_match(&fqn, target, target_short)
                        {
                            let start = offset_to_position(&content, span.start as usize);
                            let end = offset_to_position(&content, span.end as usize);
                            locations.push(Location {
                                uri: parsed_uri.clone(),
                                range: Range { start, end },
                            });
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

    /// Find all references to a member (method, property, or constant)
    /// across all files.
    ///
    /// When `hierarchy` is `Some`, only references where the subject
    /// resolves to a class in the given set of FQNs are returned.  When
    /// the subject cannot be resolved (e.g. a complex expression or an
    /// untyped variable), the reference is conservatively included.
    ///
    /// When `hierarchy` is `None`, all references with a matching member
    /// name and static-ness are returned (the v1 behaviour, kept as a
    /// fallback when the target class cannot be determined).
    fn find_member_references(
        &self,
        target_member: &str,
        target_is_static: bool,
        include_declaration: bool,
        hierarchy: Option<&HashSet<String>>,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        let snapshot = self.user_file_symbol_maps();

        for (file_uri, symbol_map) in &snapshot {
            let parsed_uri = match Url::parse(file_uri) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let content = match self.get_file_content_arc(file_uri) {
                Some(c) => c,
                None => continue,
            };

            // Lazily resolved file context — only computed when we need
            // to check a candidate's subject against the hierarchy.
            let file_ctx_cell: std::cell::OnceCell<crate::types::FileContext> =
                std::cell::OnceCell::new();

            for span in &symbol_map.spans {
                match &span.kind {
                    SymbolKind::MemberAccess {
                        subject_text,
                        member_name,
                        is_static,
                        ..
                    } if member_name == target_member && *is_static == target_is_static => {
                        // Check if the subject belongs to the target hierarchy.
                        if let Some(hier) = hierarchy {
                            let ctx = file_ctx_cell.get_or_init(|| self.file_context(file_uri));
                            let subject_fqns = self.resolve_subject_to_fqns(
                                subject_text,
                                *is_static,
                                ctx,
                                span.start,
                                &content,
                            );
                            if !subject_fqns.is_empty()
                                && !subject_fqns.iter().any(|fqn| hier.contains(fqn))
                            {
                                // Subject resolved but none of the resolved
                                // classes are in the target hierarchy — skip.
                                continue;
                            }
                            // If subject_fqns is empty, we couldn't resolve
                            // the subject — include conservatively.
                        }

                        let start = offset_to_position(&content, span.start as usize);
                        let end = offset_to_position(&content, span.end as usize);
                        locations.push(Location {
                            uri: parsed_uri.clone(),
                            range: Range { start, end },
                        });
                    }
                    SymbolKind::MemberDeclaration { name, is_static }
                        if include_declaration
                            && name == target_member
                            && *is_static == target_is_static =>
                    {
                        // Check if the enclosing class is in the hierarchy.
                        if let Some(hier) = hierarchy {
                            let ctx = file_ctx_cell.get_or_init(|| self.file_context(file_uri));
                            if let Some(enclosing) = find_class_at_offset(&ctx.classes, span.start)
                            {
                                let fqn = enclosing.fqn();
                                if !hier.contains(&fqn) {
                                    continue;
                                }
                            }
                        }

                        let start = offset_to_position(&content, span.start as usize);
                        let end = offset_to_position(&content, span.end as usize);
                        locations.push(Location {
                            uri: parsed_uri.clone(),
                            range: Range { start, end },
                        });
                    }
                    _ => {}
                }
            }

            // Property declarations use Variable spans (not
            // MemberDeclaration) because GTD relies on the Variable
            // kind to jump to the type hint.  Scan the ast_map to
            // pick up property declaration sites.
            if include_declaration && let Some(classes) = self.get_classes_for_uri(file_uri) {
                for class in &classes {
                    // Filter by hierarchy when available.
                    if let Some(hier) = hierarchy {
                        let class_fqn = class.fqn();
                        if !hier.contains(&class_fqn) {
                            continue;
                        }
                    }

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

    /// Find all references to a function across all files.
    fn find_function_references(
        &self,
        target_fqn: &str,
        target_short: &str,
        include_declaration: bool,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        // Input boundary: callers may pass FQNs with a leading `\`.
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

            let content = match self.get_file_content_arc(file_uri) {
                Some(c) => c,
                None => continue,
            };

            for span in &symbol_map.spans {
                if let SymbolKind::FunctionCall {
                    name,
                    is_definition,
                } = &span.kind
                {
                    if *is_definition && !include_declaration {
                        continue;
                    }
                    let resolved = Self::resolve_to_fqn(name, &file_use_map, &file_namespace);
                    // Input boundary: resolve_to_fqn may return a leading `\`.
                    let resolved_normalized = resolved.strip_prefix('\\').unwrap_or(&resolved);
                    if resolved_normalized != target
                        && crate::util::short_name(resolved_normalized)
                            != crate::util::short_name(target)
                    {
                        // Also try matching by short name when the
                        // namespaces don't line up (common for global
                        // functions referenced from within a namespace).
                        if name != target_short {
                            continue;
                        }
                    }
                    let start = offset_to_position(&content, span.start as usize);
                    let end = offset_to_position(&content, span.end as usize);
                    locations.push(Location {
                        uri: parsed_uri.clone(),
                        range: Range { start, end },
                    });
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

            let content = match self.get_file_content_arc(file_uri) {
                Some(c) => c,
                None => continue,
            };

            for span in &symbol_map.spans {
                if let SymbolKind::ConstantReference { name } = &span.kind {
                    if name != target_name {
                        continue;
                    }
                    let start = offset_to_position(&content, span.start as usize);
                    let end = offset_to_position(&content, span.end as usize);
                    locations.push(Location {
                        uri: parsed_uri.clone(),
                        range: Range { start, end },
                    });
                }
                // Include MemberDeclaration for constant declarations
                // when they match (class constants use MemberDeclaration).
                if include_declaration
                    && let SymbolKind::MemberDeclaration { name, is_static } = &span.kind
                    && name == target_name
                    && *is_static
                {
                    let start = offset_to_position(&content, span.start as usize);
                    let end = offset_to_position(&content, span.end as usize);
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

    fn resolve_keyword_to_fqn(
        &self,
        keyword: &str,
        uri: &str,
        namespace: &Option<String>,
        offset: u32,
    ) -> Option<String> {
        let classes: Vec<Arc<ClassInfo>> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();

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

    // ─── Class hierarchy resolution for member references ───────────────────

    /// Resolve the class hierarchy for a `MemberAccess` subject.
    ///
    /// Returns `Some(set_of_fqns)` when the subject can be resolved to at
    /// least one class, or `None` when resolution fails entirely.
    fn resolve_member_access_hierarchy(
        &self,
        uri: &str,
        subject_text: &str,
        is_static: bool,
        span_start: u32,
    ) -> Option<HashSet<String>> {
        let ctx = self.file_context(uri);
        let content = self.get_file_content(uri)?;
        let fqns =
            self.resolve_subject_to_fqns(subject_text, is_static, &ctx, span_start, &content);
        if fqns.is_empty() {
            return None;
        }
        Some(self.collect_hierarchy_for_fqns(&fqns))
    }

    /// Resolve the class hierarchy for a `MemberDeclaration` at a given offset.
    ///
    /// Finds the enclosing class and builds the hierarchy set from it.
    fn resolve_member_declaration_hierarchy(
        &self,
        uri: &str,
        offset: u32,
    ) -> Option<HashSet<String>> {
        let classes: Vec<Arc<ClassInfo>> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();
        let current_class = find_class_at_offset(&classes, offset)?;
        let fqn = current_class.fqn();
        Some(self.collect_hierarchy_for_fqns(&[fqn]))
    }

    /// Resolve a member access subject to zero or more class FQNs.
    ///
    /// This is a lightweight resolution path used during reference scanning.
    /// It handles the common cases (`self`, `static`, `$this`, `parent`,
    /// bare class names for static access, and typed `$variable` parameters)
    /// without the full weight of the completion resolver.
    fn resolve_subject_to_fqns(
        &self,
        subject_text: &str,
        is_static: bool,
        ctx: &crate::types::FileContext,
        access_offset: u32,
        content: &str,
    ) -> Vec<String> {
        let trimmed = subject_text.trim();

        match trimmed {
            "$this" | "self" | "static" => {
                if let Some(cls) =
                    self.find_enclosing_class_fqn(&ctx.classes, &ctx.namespace, access_offset)
                {
                    return vec![cls];
                }
                Vec::new()
            }
            "parent" => {
                if let Some(cc) = find_class_at_offset(&ctx.classes, access_offset)
                    && let Some(ref parent) = cc.parent_class
                {
                    let fqn = Self::resolve_to_fqn(parent, &ctx.use_map, &ctx.namespace);
                    return vec![normalize_fqn(&fqn)];
                }
                Vec::new()
            }
            _ if is_static && !trimmed.starts_with('$') => {
                // Bare class name for static access: `ClassName::method()`.
                let fqn = Self::resolve_to_fqn(trimmed, &ctx.use_map, &ctx.namespace);
                vec![normalize_fqn(&fqn)]
            }
            _ if trimmed.starts_with('$') => {
                // Variable — try variable type resolution.
                self.resolve_variable_to_fqns(trimmed, ctx, access_offset, content)
            }
            _ => Vec::new(),
        }
    }

    /// Try to resolve a variable to its class FQN(s) using the type
    /// resolution engine.
    fn resolve_variable_to_fqns(
        &self,
        var_name: &str,
        ctx: &crate::types::FileContext,
        cursor_offset: u32,
        content: &str,
    ) -> Vec<String> {
        let enclosing_class = find_class_at_offset(&ctx.classes, cursor_offset)
            .cloned()
            .unwrap_or_default();

        let class_loader = self.class_loader(ctx);
        let function_loader = self.function_loader(ctx);

        let resolved = crate::completion::variable::resolution::resolve_variable_types(
            var_name,
            &enclosing_class,
            &ctx.classes,
            content,
            cursor_offset,
            &class_loader,
            Some(&function_loader),
        );

        resolved
            .into_iter()
            .map(|ci| normalize_fqn(&ci.fqn()))
            .collect()
    }

    /// Find the FQN of the class enclosing a given byte offset.
    fn find_enclosing_class_fqn(
        &self,
        classes: &[Arc<ClassInfo>],
        namespace: &Option<String>,
        offset: u32,
    ) -> Option<String> {
        let cc = find_class_at_offset(classes, offset)?;
        let fqn = match namespace {
            Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, cc.name),
            _ => cc.name.clone(),
        };
        Some(normalize_fqn(&fqn))
    }

    /// Collect the full class hierarchy (ancestors and descendants) for
    /// a set of starting FQNs.
    ///
    /// The result includes:
    /// - The starting FQNs themselves
    /// - All ancestor FQNs (parent chain, interfaces, traits)
    /// - All descendant FQNs (classes that extend/implement any class in
    ///   the hierarchy)
    fn collect_hierarchy_for_fqns(&self, seed_fqns: &[String]) -> HashSet<String> {
        let mut hierarchy = HashSet::new();
        let class_loader = |name: &str| -> Option<Arc<ClassInfo>> { self.find_or_load_class(name) };

        // Insert the seeds.
        for fqn in seed_fqns {
            hierarchy.insert(fqn.clone());
        }

        // Walk up: collect all ancestors for each seed.
        for fqn in seed_fqns {
            self.collect_ancestors(fqn, &class_loader, &mut hierarchy);
        }

        // Walk down: collect all descendants from ast_map and class_index.
        // We iterate until no new FQNs are added (transitive closure).
        let mut changed = true;
        let mut depth = 0u32;
        while changed && depth < MAX_INHERITANCE_DEPTH {
            changed = false;
            depth += 1;

            // Snapshot the current hierarchy to check against.
            let current: Vec<String> = hierarchy.iter().cloned().collect();

            // Scan all known classes for ones that extend/implement/use
            // anything in the current hierarchy.
            let all_classes: Vec<ClassInfo> = {
                let map = self.ast_map.read();
                map.values()
                    .flat_map(|classes| classes.iter().map(|c| ClassInfo::clone(c)))
                    .collect()
            };

            for cls in &all_classes {
                let cls_fqn = normalize_fqn(&cls.fqn());
                if hierarchy.contains(&cls_fqn) {
                    continue;
                }

                if self.class_is_descendant_of(cls, &current, &class_loader) {
                    hierarchy.insert(cls_fqn);
                    changed = true;
                }
            }

            // Also check class_index entries not yet in ast_map.
            let index_entries: Vec<String> = {
                let idx = self.class_index.read();
                idx.keys().cloned().collect()
            };

            for fqn in &index_entries {
                let normalized = normalize_fqn(fqn);
                if hierarchy.contains(&normalized) {
                    continue;
                }

                if let Some(cls) = class_loader(fqn)
                    && self.class_is_descendant_of(&cls, &current, &class_loader)
                {
                    hierarchy.insert(normalized);
                    changed = true;
                }
            }
        }

        hierarchy
    }

    /// Walk up the inheritance chain and collect all ancestor FQNs.
    fn collect_ancestors(
        &self,
        fqn: &str,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        hierarchy: &mut HashSet<String>,
    ) {
        let cls = match class_loader(fqn) {
            Some(c) => c,
            None => return,
        };

        // Parent class chain.
        if let Some(ref parent) = cls.parent_class {
            let parent_fqn = normalize_fqn(parent);
            if hierarchy.insert(parent_fqn.clone()) {
                self.collect_ancestors(&parent_fqn, class_loader, hierarchy);
            }
        }

        // Interfaces.
        for iface in &cls.interfaces {
            let iface_fqn = normalize_fqn(iface);
            if hierarchy.insert(iface_fqn.clone()) {
                self.collect_ancestors(&iface_fqn, class_loader, hierarchy);
            }
        }

        // Used traits.
        for trait_name in &cls.used_traits {
            let trait_fqn = normalize_fqn(trait_name);
            if hierarchy.insert(trait_fqn.clone()) {
                self.collect_ancestors(&trait_fqn, class_loader, hierarchy);
            }
        }

        // Mixins.
        for mixin in &cls.mixins {
            let mixin_fqn = normalize_fqn(mixin);
            if hierarchy.insert(mixin_fqn.clone()) {
                self.collect_ancestors(&mixin_fqn, class_loader, hierarchy);
            }
        }
    }

    /// Check whether a class directly extends, implements, or uses
    /// anything in the given set of FQNs.
    fn class_is_descendant_of(
        &self,
        cls: &ClassInfo,
        targets: &[String],
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> bool {
        // Direct parent.
        if let Some(ref parent) = cls.parent_class {
            let parent_fqn = normalize_fqn(parent);
            if targets.contains(&parent_fqn) {
                return true;
            }
            // Transitive: walk the parent chain.
            if self.ancestor_in_set(&parent_fqn, targets, class_loader, 0) {
                return true;
            }
        }

        // Direct interfaces.
        for iface in &cls.interfaces {
            let iface_fqn = normalize_fqn(iface);
            if targets.contains(&iface_fqn) {
                return true;
            }
            if self.ancestor_in_set(&iface_fqn, targets, class_loader, 0) {
                return true;
            }
        }

        // Used traits.
        for trait_name in &cls.used_traits {
            let trait_fqn = normalize_fqn(trait_name);
            if targets.contains(&trait_fqn) {
                return true;
            }
        }

        // Mixins.
        for mixin in &cls.mixins {
            let mixin_fqn = normalize_fqn(mixin);
            if targets.contains(&mixin_fqn) {
                return true;
            }
        }

        false
    }

    /// Recursively check whether any ancestor of `fqn` is in the target set.
    fn ancestor_in_set(
        &self,
        fqn: &str,
        targets: &[String],
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        depth: u32,
    ) -> bool {
        if depth >= MAX_INHERITANCE_DEPTH {
            return false;
        }

        let cls = match class_loader(fqn) {
            Some(c) => c,
            None => return false,
        };

        if let Some(ref parent) = cls.parent_class {
            let parent_fqn = normalize_fqn(parent);
            if targets.contains(&parent_fqn) {
                return true;
            }
            if self.ancestor_in_set(&parent_fqn, targets, class_loader, depth + 1) {
                return true;
            }
        }

        for iface in &cls.interfaces {
            let iface_fqn = normalize_fqn(iface);
            if targets.contains(&iface_fqn) {
                return true;
            }
            if self.ancestor_in_set(&iface_fqn, targets, class_loader, depth + 1) {
                return true;
            }
        }

        false
    }

    /// Ensure all workspace PHP files have been parsed and have symbol maps.
    ///
    /// This lazily parses files that are in the workspace directory but
    /// have not been opened or indexed yet.  It also covers files known
    /// via the classmap and class_index.  The vendor directory (read from
    /// `composer.json` `config.vendor-dir`, defaulting to `vendor`) is
    /// skipped during the filesystem walk.
    fn ensure_workspace_indexed(&self) {
        // Collect URIs that already have symbol maps.
        let existing_uris: HashSet<String> = self.symbol_maps.read().keys().cloned().collect();

        // Build the vendor URI prefixes so we can skip vendor files in
        // Phase 1 (class_index may contain vendor URIs from prior
        // resolution, but we only need symbol maps for user files).
        let vendor_prefixes = self.vendor_uri_prefixes.lock().clone();

        // ── Phase 1: class_index files (user only) ─────────────────────
        // These are files we already know about from update_ast calls,
        // ensuring their symbol maps are populated.  Vendor files are
        // skipped — find references only reports user code.
        //
        // File content is read and parsed in parallel using
        // `std::thread::scope`.  Each thread reads one file from disk
        // and calls `update_ast` which acquires write locks briefly to
        // store the results.  The expensive parsing step runs without
        // any locks held.
        let index_uris: Vec<String> = self.class_index.read().values().cloned().collect();

        let phase1_uris: Vec<&String> = index_uris
            .iter()
            .filter(|uri| {
                !existing_uris.contains(*uri)
                    && !vendor_prefixes.iter().any(|p| uri.starts_with(p.as_str()))
                    && !uri.starts_with("phpantom-stub://")
                    && !uri.starts_with("phpantom-stub-fn://")
            })
            .collect();

        self.parse_files_parallel(
            phase1_uris
                .iter()
                .map(|uri| (uri.as_str(), None::<&str>))
                .collect(),
        );

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
            let vendor_dir_paths = self.vendor_dir_paths.lock().clone();

            // Re-read existing URIs after phase 1 may have added more.
            let existing_uris: HashSet<String> = self.symbol_maps.read().keys().cloned().collect();

            let php_files = collect_php_files_gitignore(&root, &vendor_dir_paths);

            let phase2_work: Vec<(String, PathBuf)> = php_files
                .into_iter()
                .filter_map(|path| {
                    let uri = format!("file://{}", path.display());
                    if existing_uris.contains(&uri) {
                        None
                    } else {
                        Some((uri, path))
                    }
                })
                .collect();

            self.parse_paths_parallel(&phase2_work);
        }
    }

    /// Parse a batch of files in parallel using OS threads.
    ///
    /// Each entry is `(uri, optional_content)`.  When `content` is `None`,
    /// the file is loaded via [`get_file_content`].  The expensive parsing
    /// step runs without any locks held; only the brief map insertions at
    /// the end of [`update_ast`] acquire write locks.
    ///
    /// Uses [`std::thread::scope`] for structured concurrency so that all
    /// spawned threads are guaranteed to finish before this method returns.
    /// The thread count is capped at the number of available CPU cores.
    fn parse_files_parallel(&self, files: Vec<(&str, Option<&str>)>) {
        if files.is_empty() {
            return;
        }

        // For very small batches, avoid thread overhead.
        if files.len() <= 2 {
            for (uri, content) in &files {
                if let Some(c) = content {
                    self.update_ast(uri, c);
                } else if let Some(c) = self.get_file_content(uri) {
                    self.update_ast(uri, &c);
                }
            }
            return;
        }

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(files.len());

        let chunks: Vec<Vec<(&str, Option<&str>)>> = {
            let chunk_size = files.len().div_ceil(n_threads);
            files.chunks(chunk_size).map(|c| c.to_vec()).collect()
        };

        // Use a 16 MB stack per thread.  The default 8 MB can overflow
        // when parsing deeply-nested PHP files (e.g. WordPress
        // admin-bar.php) because `extract_symbol_map` recurses through
        // the full AST via `extract_from_expression` /
        // `extract_from_statement`.  Stack overflows are fatal
        // (abort, not panic) so `catch_unwind` cannot save us.
        const PARSE_STACK_SIZE: usize = 16 * 1024 * 1024;

        std::thread::scope(|s| {
            for chunk in &chunks {
                std::thread::Builder::new()
                    .stack_size(PARSE_STACK_SIZE)
                    .spawn_scoped(s, move || {
                        for (uri, content) in chunk {
                            if let Some(c) = content {
                                self.update_ast(uri, c);
                            } else if let Some(c) = self.get_file_content(uri) {
                                self.update_ast(uri, &c);
                            }
                        }
                    })
                    .expect("failed to spawn parse thread");
            }
        });
    }

    /// Parse a batch of files from disk paths in parallel.
    ///
    /// Each entry is `(uri, path)`.  The file is read from disk and
    /// parsed in a worker thread.  Uses [`std::thread::scope`] for
    /// structured concurrency.
    fn parse_paths_parallel(&self, files: &[(String, PathBuf)]) {
        if files.is_empty() {
            return;
        }

        // For very small batches, avoid thread overhead.
        if files.len() <= 2 {
            for (uri, path) in files {
                if let Ok(content) = std::fs::read_to_string(path) {
                    self.update_ast(uri, &content);
                }
            }
            return;
        }

        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(files.len());

        let chunks: Vec<&[(String, PathBuf)]> = {
            let chunk_size = files.len().div_ceil(n_threads);
            files.chunks(chunk_size).collect()
        };

        const PARSE_STACK_SIZE: usize = 16 * 1024 * 1024;

        std::thread::scope(|s| {
            for chunk in &chunks {
                std::thread::Builder::new()
                    .stack_size(PARSE_STACK_SIZE)
                    .spawn_scoped(s, move || {
                        for (uri, path) in *chunk {
                            if let Ok(content) = std::fs::read_to_string(path) {
                                self.update_ast(uri, &content);
                            }
                        }
                    })
                    .expect("failed to spawn parse thread");
            }
        });
    }
}

/// Normalise a class FQN: strip leading `\` if present.
fn normalize_fqn(fqn: &str) -> String {
    fqn.strip_prefix('\\').unwrap_or(fqn).to_string()
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
