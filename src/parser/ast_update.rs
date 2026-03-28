/// AST update orchestration and name resolution.
///
/// This module contains the `update_ast` method that performs a full
/// parse of a PHP file and updates all the backend maps (ast_map,
/// use_map, namespace_map, global_functions, global_defines, class_index,
/// symbol_maps) in a single pass.  It also contains the name resolution
/// helpers (`resolve_parent_class_names`, `resolve_name`) used to convert
/// short class names to fully-qualified names.
use std::collections::HashMap;
use std::sync::Arc;

use crate::docblock::types::is_scalar;
use crate::symbol_map::extract_symbol_map;

use bumpalo::Bump;

use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

use crate::Backend;
use crate::types::ClassInfo;

use super::DocblockCtx;

impl Backend {
    /// Update the ast_map, use_map, and namespace_map for a given file URI
    /// by parsing its content.
    ///
    /// Returns `true` when at least one class signature in this file
    /// changed (or a class was added/removed), meaning other open files
    /// that reference those classes may have stale diagnostics.
    pub fn update_ast(&self, uri: &str, content: &str) -> bool {
        // The mago-syntax parser contains `unreachable!()` and `.expect()`
        // calls that can panic on malformed PHP (e.g. partially-written
        // heredocs/nowdocs, which are common while editing).  Wrap the
        // entire parse + extraction in `catch_unwind` so a parser panic
        // doesn't crash the LSP server and produce a zombie process.
        //
        // On panic the file is simply skipped — no maps are updated, and
        // the user gets stale (but not missing) completions until the
        // file is saved in a parseable state.
        let content_owned = content.to_string();
        let uri_owned = uri.to_string();

        let result = crate::util::catch_panic_unwind_safe("parse", uri, None, || {
            self.update_ast_inner(&uri_owned, &content_owned)
        });

        match result {
            Some(changed) => changed,
            None => {
                // Parser panicked — store a single "Parse failed" error
                // so the syntax-error diagnostic collector can report it.
                self.parse_errors.write().insert(
                    uri.to_string(),
                    vec![("Parse failed (internal error)".to_string(), 0, 0)],
                );
                false
            }
        }
    }

    /// Inner implementation of [`update_ast`] that performs the actual
    /// parsing and map updates.  Separated so that [`update_ast`] can
    /// wrap the call in [`std::panic::catch_unwind`].
    ///
    /// Returns `true` when at least one class signature changed.
    fn update_ast_inner(&self, uri: &str, content: &str) -> bool {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        // Run mago-names resolver while the arena is still alive.
        // This produces a `ResolvedNames` that maps every identifier's
        // byte offset to its fully-qualified name.  We immediately copy
        // the data into an owned `OwnedResolvedNames` so it survives
        // the arena drop.
        let name_resolver = mago_names::resolver::NameResolver::new(&arena);
        let mago_resolved = name_resolver.resolve(program);
        let owned_resolved = crate::names::OwnedResolvedNames::from_resolved(&mago_resolved);

        // Cache parse errors for the syntax-error diagnostic collector.
        // Extract (message, start_byte, end_byte) tuples from the
        // arena-allocated errors before the arena is dropped.
        {
            use mago_span::HasSpan;

            let errors: Vec<(String, u32, u32)> = program
                .errors
                .iter()
                .map(|e| {
                    let span = e.span();
                    (
                        super::error_format::format_parse_error(e),
                        span.start.offset,
                        span.end.offset,
                    )
                })
                .collect();
            self.parse_errors.write().insert(uri.to_string(), errors);
        }

        let doc_ctx = DocblockCtx {
            trivias: program.trivia.as_slice(),
            content,
            php_version: Some(self.php_version()),
            use_map: HashMap::new(),
            namespace: None,
        };

        // Extract all three in a single parse pass.
        //
        // `classes_with_ns` tracks each extracted class together with the
        // namespace block it was declared in.  This is critical for files
        // that contain multiple `namespace { }` blocks (e.g. example.php
        // places demo classes in `Demo` and Illuminate stubs in their own
        // namespace blocks).  The per-class namespace is used later when
        // building the `class_index` and when resolving parent/trait names.
        let mut classes_with_ns: Vec<(ClassInfo, Option<String>)> = Vec::new();
        let mut use_map = HashMap::new();
        let mut namespace: Option<String> = None;

        for statement in program.statements.iter() {
            match statement {
                Statement::Use(use_stmt) => {
                    Self::extract_use_items(&use_stmt.items, &mut use_map);
                }
                Statement::Namespace(ns) => {
                    // Determine the namespace for this block.
                    let block_ns: Option<String> = ns
                        .name
                        .as_ref()
                        .map(|ident| ident.value().to_string())
                        .filter(|n| !n.is_empty());

                    // The file-level namespace is the FIRST non-empty one.
                    if namespace.is_none() {
                        namespace = block_ns.clone();
                    }

                    // Collect classes from this namespace block, tagging
                    // each with the block's namespace.
                    let mut block_classes = Vec::new();
                    // Recurse into namespace body for classes and use statements
                    for inner in ns.statements().iter() {
                        match inner {
                            Statement::Use(use_stmt) => {
                                Self::extract_use_items(&use_stmt.items, &mut use_map);
                            }
                            Statement::Class(_)
                            | Statement::Interface(_)
                            | Statement::Trait(_)
                            | Statement::Enum(_) => {
                                Self::extract_classes_from_statements(
                                    std::iter::once(inner),
                                    &mut block_classes,
                                    Some(&doc_ctx),
                                );
                            }
                            Statement::Namespace(inner_ns) => {
                                // Nested namespaces (rare but valid)
                                Self::extract_use_statements_from_statements(
                                    inner_ns.statements().iter(),
                                    &mut use_map,
                                );
                                Self::extract_classes_from_statements(
                                    inner_ns.statements().iter(),
                                    &mut block_classes,
                                    Some(&doc_ctx),
                                );
                            }
                            _ => {
                                // Walk other statements (expression statements,
                                // control flow, etc.) for anonymous classes.
                                Self::find_anonymous_classes_in_statement(
                                    inner,
                                    &mut block_classes,
                                    Some(&doc_ctx),
                                );
                            }
                        }
                    }

                    // Tag each class with the namespace of this block.
                    for cls in block_classes {
                        classes_with_ns.push((cls, block_ns.clone()));
                    }
                }
                Statement::Class(_)
                | Statement::Interface(_)
                | Statement::Trait(_)
                | Statement::Enum(_) => {
                    let mut top_classes = Vec::new();
                    Self::extract_classes_from_statements(
                        std::iter::once(statement),
                        &mut top_classes,
                        Some(&doc_ctx),
                    );
                    for cls in top_classes {
                        classes_with_ns.push((cls, None));
                    }
                }
                _ => {
                    // Walk other top-level statements (expression statements,
                    // function declarations, control flow, etc.) for anonymous
                    // classes.
                    let mut anon_classes = Vec::new();
                    Self::find_anonymous_classes_in_statement(
                        statement,
                        &mut anon_classes,
                        Some(&doc_ctx),
                    );
                    for cls in anon_classes {
                        classes_with_ns.push((cls, None));
                    }
                }
            }
        }

        // Extract standalone functions (including those inside if-guards
        // like `if (! function_exists('...'))`) using the shared helper
        // which recurses into if/block statements.
        let mut functions = Vec::new();
        Self::extract_functions_from_statements(
            program.statements.iter(),
            &mut functions,
            &namespace,
            Some(&doc_ctx),
        );
        if !functions.is_empty() {
            // Resolve class-like names in function return types and
            // parameter type hints to FQNs so that cross-file consumers
            // can resolve them without the declaring file's use map.
            // This mirrors the resolution done for class method return
            // types and parameter hints in `resolve_parent_class_names`.
            for func in &mut functions {
                let skip_names: Vec<String> = func.template_params.clone();

                if let Some(ref ret) = func.return_type {
                    let resolved =
                        Self::resolve_type_string(ret, &use_map, &namespace, &skip_names);
                    if resolved != *ret {
                        func.return_type = Some(resolved);
                    }
                }
                if let Some(ref ret) = func.native_return_type {
                    let resolved =
                        Self::resolve_type_string(ret, &use_map, &namespace, &skip_names);
                    if resolved != *ret {
                        func.native_return_type = Some(resolved);
                    }
                }
                for param in &mut func.parameters {
                    if let Some(ref hint) = param.type_hint {
                        let resolved =
                            Self::resolve_type_string(hint, &use_map, &namespace, &skip_names);
                        if resolved != *hint {
                            param.type_hint = Some(resolved);
                        }
                    }
                }
                // Resolve exception class names in @throws tags.
                for throw in &mut func.throws {
                    let resolved =
                        Self::resolve_type_string(throw, &use_map, &namespace, &skip_names);
                    if resolved != *throw {
                        *throw = resolved;
                    }
                }
            }

            let mut fmap = self.global_functions.write();
            for func_info in functions {
                let fqn = if let Some(ref ns) = func_info.namespace {
                    format!("{}\\{}", ns, &func_info.name)
                } else {
                    func_info.name.clone()
                };

                // Skip polyfill functions when a native stub exists.
                // Libraries like Laravel wrap helpers such as
                // `str_contains` in `if (! function_exists('…'))` guards
                // and mark them `@deprecated`.  On the configured PHP
                // version the native function exists, so the guard is
                // never entered and the polyfill is dead code.  Letting
                // the stub win ensures the correct signature, return
                // type, and deprecation status are used everywhere
                // (hover, completion, diagnostics).
                if func_info.is_polyfill
                    && self.stub_function_index.read().contains_key(fqn.as_str())
                {
                    continue;
                }

                // Insert under the FQN only.  For namespaced functions
                // the FQN is `Namespace\name`; for global functions it
                // is just the bare name.  `resolve_function_name` already
                // builds namespace-qualified candidates, so a short-name
                // fallback entry is unnecessary and would cause collisions
                // when two namespaces define the same short name.
                fmap.insert(fqn, (uri.to_string(), func_info));
            }
        }

        // Extract define() constants from the already-parsed AST and
        // store them in the global_defines map so they appear in
        // completions.  This reuses the parse pass above rather than
        // doing a separate regex scan over the raw content.
        let mut define_entries = Vec::new();
        Self::extract_defines_from_statements(
            program.statements.iter(),
            &mut define_entries,
            content,
        );
        if !define_entries.is_empty() {
            let mut dmap = self.global_defines.write();
            for (name, offset, value) in define_entries {
                dmap.entry(name)
                    .or_insert_with(|| crate::types::DefineInfo {
                        file_uri: uri.to_string(),
                        name_offset: offset,
                        value,
                    });
            }
        }

        // Post-process: resolve parent_class short names to fully-qualified
        // names using the file's use_map and each class's own namespace so
        // that cross-file inheritance resolution can find parent classes via
        // PSR-4.
        //
        // For files with multiple namespace blocks, each class's names are
        // resolved against its own namespace rather than the file-level
        // default.  This is done by grouping classes by namespace and
        // calling resolve_parent_class_names once per group.
        {
            // Gather distinct namespaces used in this file.
            let mut ns_groups: HashMap<Option<String>, Vec<usize>> = HashMap::new();
            for (i, (_cls, ns)) in classes_with_ns.iter().enumerate() {
                ns_groups.entry(ns.clone()).or_default().push(i);
            }

            // When all classes share the same namespace, take the fast
            // path (single call, no extra allocation).
            if ns_groups.len() <= 1 {
                let mut classes: Vec<ClassInfo> =
                    classes_with_ns.iter().map(|(c, _)| c.clone()).collect();
                Self::resolve_parent_class_names(&mut classes, &use_map, &namespace);
                // Write back
                for (i, cls) in classes.into_iter().enumerate() {
                    classes_with_ns[i].0 = cls;
                }
            } else {
                // Multi-namespace file: resolve each group with its own
                // namespace context.
                for (group_ns, indices) in &ns_groups {
                    let mut group: Vec<ClassInfo> = indices
                        .iter()
                        .map(|&i| classes_with_ns[i].0.clone())
                        .collect();
                    Self::resolve_parent_class_names(&mut group, &use_map, group_ns);
                    for (j, &idx) in indices.iter().enumerate() {
                        classes_with_ns[idx].0 = group[j].clone();
                    }
                }
            }
        }

        // Separate the classes from their namespace tags for storage,
        // stamping each ClassInfo with its namespace so that
        // `find_class_in_ast_map` can distinguish classes with the same
        // short name in different namespace blocks.
        let classes: Vec<ClassInfo> = classes_with_ns
            .iter()
            .map(|(c, ns)| {
                let mut cls = c.clone();
                cls.file_namespace = ns.clone();
                cls
            })
            .collect();

        let uri_string = uri.to_string();

        // Collect old ClassInfo values (not just FQNs) before the ast_map
        // entry is overwritten.  These are compared against the new classes
        // using `signature_eq` to decide whether each FQN's cache entry
        // actually needs eviction (signature-level cache invalidation).
        let old_classes_snapshot: Vec<crate::types::ClassInfo> = self
            .ast_map
            .read()
            .get(&uri_string)
            .map(|v| {
                v.iter()
                    .map(|c| crate::types::ClassInfo::clone(c))
                    .collect()
            })
            .unwrap_or_default();
        let old_fqns: Vec<String> = old_classes_snapshot
            .iter()
            .filter(|c| !c.name.starts_with("__anonymous@"))
            .map(|c| match &c.file_namespace {
                Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, c.name),
                _ => c.name.clone(),
            })
            .collect();

        // Populate the class_index with FQN → URI mappings for every class
        // found in this file.  This enables reliable lookup of classes that
        // don't follow PSR-4 conventions (e.g. classes defined in Composer
        // autoload_files.php entries).
        //
        // Uses the per-class namespace (not the file-level namespace) so
        // that files with multiple namespace blocks produce correct FQNs.
        {
            let mut idx = self.class_index.write();
            let mut fqn_idx = self.fqn_index.write();
            // Remove stale entries from previous parses of this file.
            // When a file's namespace changes (e.g. while the user is
            // typing a namespace declaration), old FQNs linger under
            // the previous namespace and pollute completions.
            //
            // Use targeted removes via old_fqns instead of a full
            // retain() scan — O(old_classes) ≈ O(1) vs O(class_index).
            for old_fqn in &old_fqns {
                idx.remove(old_fqn);
                fqn_idx.remove(old_fqn);
            }

            for (i, (class, class_ns)) in classes_with_ns.iter().enumerate() {
                // Anonymous classes (named `__anonymous@<offset>`) are
                // internal bookkeeping — they should never appear in
                // cross-file lookups or completion results.
                if class.name.starts_with("__anonymous@") {
                    continue;
                }
                let fqn = if let Some(ns) = class_ns {
                    format!("{}\\{}", ns, &class.name)
                } else {
                    class.name.clone()
                };
                idx.insert(fqn.clone(), uri_string.clone());
                // The `classes` vec already has `file_namespace` set,
                // so use it for the fqn_index entry.
                fqn_idx.insert(fqn, Arc::new(classes[i].clone()));
            }
        }

        // Remove newly-discovered FQNs from the negative-result cache
        // so classes that just became available are not suppressed.
        {
            let nf_cache = self.class_not_found_cache.read();
            if !nf_cache.is_empty() {
                drop(nf_cache);
                let mut nf_cache = self.class_not_found_cache.write();
                for (class, class_ns) in &classes_with_ns {
                    if class.name.starts_with("__anonymous@") {
                        continue;
                    }
                    let fqn = match class_ns {
                        Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, class.name),
                        _ => class.name.clone(),
                    };
                    nf_cache.remove(&fqn);
                }
            }
        }

        // Build the precomputed symbol map while the AST is still alive.
        // This must happen before the `Program` (and its arena) are dropped.
        let symbol_map = std::sync::Arc::new(extract_symbol_map(program, content));

        self.ast_map.write().insert(
            uri_string.clone(),
            classes.into_iter().map(Arc::new).collect(),
        );
        self.symbol_maps
            .write()
            .insert(uri_string.clone(), symbol_map);
        self.use_map.write().insert(uri_string.clone(), use_map);
        self.resolved_names
            .write()
            .insert(uri_string.clone(), Arc::new(owned_resolved));
        self.namespace_map.write().insert(uri_string, namespace);

        // Selectively invalidate the resolved-class cache with
        // signature-level granularity.
        //
        // Instead of evicting every FQN defined in this file on every
        // keystroke, compare the old and new ClassInfo values using
        // `signature_eq`.  When the signature has not changed (the
        // overwhelmingly common case during normal editing inside a
        // method body), the cache entry is kept warm.
        //
        // FQNs that only appear in the old set (renamed/removed classes)
        // or only in the new set (newly added classes) are always evicted.
        // FQNs present in both sets are evicted only when their signature
        // differs.
        //
        // `evict_fqn` transitively evicts dependents (classes that
        // extend/use/implement/mixin the changed class) so that
        // cached child classes don't serve stale inherited members.
        //
        // **First-parse fast path**: when `old_fqns` is empty the file
        // has never been parsed by `update_ast` before.  There are no
        // stale cache entries to evict — any existing cache entries for
        // these FQNs were populated by legitimate resolution paths
        // (classmap / PSR-4 / stubs) reading the same on-disk content.
        // Skipping eviction here eliminates the O(N²) cost of calling
        // `evict_fqn` (which does a full cache scan + transitive
        // dependent cascade) for every class during bulk operations
        // like `analyse`.
        let mut any_signature_changed = false;

        if !old_fqns.is_empty() {
            let mut cache = self.resolved_class_cache.lock();
            // Collect new FQNs from the classes we just parsed.
            let new_fqns: Vec<String> = classes_with_ns
                .iter()
                .filter(|(c, _)| !c.name.starts_with("__anonymous@"))
                .map(|(c, ns)| match ns {
                    Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, c.name),
                    _ => c.name.clone(),
                })
                .collect();

            // Evict old FQNs that no longer exist (renames / removals),
            // or whose signature changed.
            for (i, fqn) in old_fqns.iter().enumerate() {
                let old_cls = &old_classes_snapshot[old_classes_snapshot
                    .iter()
                    .position(|c| {
                        !c.name.starts_with("__anonymous@") && {
                            let f = match &c.file_namespace {
                                Some(ns) if !ns.is_empty() => {
                                    format!("{}\\{}", ns, c.name)
                                }
                                _ => c.name.clone(),
                            };
                            f == *fqn
                        }
                    })
                    .unwrap_or(i)];

                // Find the matching new class by FQN.
                let new_cls = classes_with_ns.iter().find(|(c, ns)| {
                    !c.name.starts_with("__anonymous@") && {
                        let f = match ns {
                            Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, c.name),
                            _ => c.name.clone(),
                        };
                        f == *fqn
                    }
                });

                match new_cls {
                    Some((new, _)) if old_cls.signature_eq(new) => {
                        // Signature unchanged — keep the cache entry warm.
                    }
                    _ => {
                        // Signature changed or class was removed — evict.
                        crate::virtual_members::evict_fqn(&mut cache, fqn);
                        any_signature_changed = true;
                    }
                }
            }

            // Evict new FQNs that did not exist before (new classes).
            for fqn in &new_fqns {
                if !old_fqns.contains(fqn) {
                    crate::virtual_members::evict_fqn(&mut cache, fqn);
                    any_signature_changed = true;
                }
            }
        }

        any_signature_changed
    }

    /// Resolve `parent_class` short names in a list of `ClassInfo` to
    /// fully-qualified names using the file's `use_map` and `namespace`.
    ///
    /// Rules (matching PHP name resolution):
    ///   1. Already fully-qualified (`\Foo\Bar`) → strip leading `\`
    ///   2. Qualified (`Foo\Bar`) → if first segment is in use_map, expand it;
    ///      otherwise prepend current namespace
    ///   3. Unqualified (`Bar`) → check use_map; otherwise prepend namespace
    ///   4. No namespace and not in use_map → keep as-is
    pub fn resolve_parent_class_names(
        classes: &mut [ClassInfo],
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) {
        // Collect type alias names from ALL classes in the file up-front.
        // A type alias defined on one class can be referenced from methods
        // in a different class in the same file, so we must skip all of
        // them to avoid mangling alias names into FQN form.
        let all_alias_names: Vec<String> = classes
            .iter()
            .flat_map(|c| c.type_aliases.keys().cloned())
            .collect();

        for class in classes.iter_mut() {
            if let Some(ref parent) = class.parent_class {
                let resolved = Self::resolve_name(parent, use_map, namespace);
                class.parent_class = Some(resolved);
            }
            // Resolve trait names to fully-qualified names
            class.used_traits = class
                .used_traits
                .iter()
                .map(|t| Self::resolve_name(t, use_map, namespace))
                .collect();

            // Resolve interface names to fully-qualified names
            class.interfaces = class
                .interfaces
                .iter()
                .map(|i| Self::resolve_name(i, use_map, namespace))
                .collect();

            // Resolve trait names in `insteadof` precedence adaptations
            for prec in &mut class.trait_precedences {
                prec.trait_name = Self::resolve_name(&prec.trait_name, use_map, namespace);
                prec.insteadof = prec
                    .insteadof
                    .iter()
                    .map(|t| Self::resolve_name(t, use_map, namespace))
                    .collect();
            }

            // Resolve trait names in `as` alias adaptations
            for alias in &mut class.trait_aliases {
                if let Some(ref t) = alias.trait_name {
                    alias.trait_name = Some(Self::resolve_name(t, use_map, namespace));
                }
            }

            // Resolve mixin names to fully-qualified names
            class.mixins = class
                .mixins
                .iter()
                .map(|m| Self::resolve_name(m, use_map, namespace))
                .collect();

            // Resolve custom collection class name to FQN
            if let Some(coll) = class.laravel().and_then(|l| l.custom_collection.as_ref()) {
                let resolved = Self::resolve_name(coll, use_map, namespace);
                class.laravel_mut().custom_collection = Some(resolved);
            }

            // Resolve cast class names to FQN so that custom cast
            // classes like `DecimalCast` (imported via `use`) are
            // loadable cross-file when `cast_type_to_php_type` calls
            // the class loader.
            {
                let casts: Vec<(String, String)> = class
                    .laravel()
                    .map(|l| l.casts_definitions.clone())
                    .unwrap_or_default();
                if !casts.is_empty() {
                    let resolved: Vec<(String, String)> = casts
                        .into_iter()
                        .map(|(col, cast_type)| {
                            // Only resolve class-like cast types (not
                            // built-in strings like "boolean", "datetime",
                            // etc.).  A simple heuristic: if the value
                            // contains an uppercase letter and is not a
                            // known built-in, treat it as a class name.
                            //
                            // Skip names that already contain a `\` — they
                            // are already qualified (e.g. the string literal
                            // `'App\Casts\HtmlCast'`).  Passing them through
                            // `resolve_name` would prepend the file's
                            // namespace, producing a broken FQN like
                            // `App\Models\App\Casts\HtmlCast`.
                            let first_segment = cast_type.split(':').next().unwrap_or(&cast_type);
                            if first_segment.contains('\\') || first_segment.starts_with('\\') {
                                // Already qualified — strip leading `\` if present to produce canonical FQN.
                                let canonical = cast_type
                                    .strip_prefix('\\')
                                    .map_or(cast_type.clone(), |s| s.to_string());
                                (col, canonical)
                            } else if first_segment.chars().any(|c| c.is_ascii_uppercase()) {
                                let resolved_class =
                                    Self::resolve_name(first_segment, use_map, namespace);
                                if resolved_class != first_segment {
                                    // Re-attach any `:argument` suffix.
                                    let suffix = &cast_type[first_segment.len()..];
                                    (col, format!("{resolved_class}{suffix}"))
                                } else {
                                    (col, cast_type)
                                }
                            } else {
                                (col, cast_type)
                            }
                        })
                        .collect();
                    class.laravel_mut().casts_definitions = resolved;
                }
            }

            // Resolve type arguments in @extends, @implements, and @use
            // generics so that after generic substitution, return types
            // and property types are fully-qualified and can be resolved
            // across files via PSR-4.
            //
            // Template params of the current class must be skipped so
            // that forwarded params (e.g. `@use BuildsQueries<TModel>`
            // where TModel is a class-level template) remain as bare
            // names and match substitution map keys later.
            let tpl_params = &class.template_params;
            Self::resolve_generics_type_args(
                &mut class.extends_generics,
                use_map,
                namespace,
                tpl_params,
            );
            Self::resolve_generics_type_args(
                &mut class.implements_generics,
                use_map,
                namespace,
                tpl_params,
            );
            Self::resolve_generics_type_args(
                &mut class.use_generics,
                use_map,
                namespace,
                tpl_params,
            );
            Self::resolve_generics_type_args(
                &mut class.mixin_generics,
                use_map,
                namespace,
                tpl_params,
            );

            // Resolve class-like names in method return types and property
            // type hints so that cross-file resolution works correctly.
            // For example, if a method returns `Country` and the file has
            // `use Luxplus\Core\Enums\Country`, the return type becomes
            // the FQN `Luxplus\Core\Enums\Country`.
            //
            // Template params and type alias names are excluded to avoid
            // mangling generic types and locally-defined type aliases.
            // We collect alias names from ALL classes in the file because
            // a type alias defined on one class may be referenced from a
            // method in a different class in the same file.
            let template_params = &class.template_params;
            let skip_names: Vec<String> = template_params
                .iter()
                .cloned()
                .chain(all_alias_names.iter().cloned())
                .collect();

            // Also resolve class-like names inside type alias definitions
            // so that `@phpstan-type ActiveUser User` where `User` is
            // imported via `use App\Models\User` becomes `App\Models\User`.
            // Skip imported aliases (`from:ClassName:OriginalName`) — those
            // are internal references, not type strings.
            for def in class.type_aliases.values_mut() {
                if let Some(rest) = def.strip_prefix("from:")
                    && let Some((class_name, original)) = rest.split_once(':')
                {
                    // Imported alias — resolve the class name portion.
                    // Format: `from:ClassName:OriginalName`
                    let resolved_class = Self::resolve_name(class_name, use_map, namespace);
                    *def = format!("from:{}:{}", resolved_class, original);
                    continue;
                }
                let resolved = Self::resolve_type_string(def, use_map, namespace, &skip_names);
                if resolved != *def {
                    *def = resolved;
                }
            }

            for method in class.methods.make_mut() {
                // Build a per-method skip list that includes both class-level
                // and method-level template params so that names like `T` in
                // `@return Collection<T>` are not namespace-resolved.
                let method_skip: Vec<String> = if method.template_params.is_empty() {
                    skip_names.clone()
                } else {
                    skip_names
                        .iter()
                        .cloned()
                        .chain(method.template_params.iter().cloned())
                        .collect()
                };

                if let Some(ref ret) = method.return_type {
                    let resolved = Self::resolve_type_string(ret, use_map, namespace, &method_skip);
                    if resolved != *ret {
                        method.return_type = Some(resolved);
                    }
                }
                for param in &mut method.parameters {
                    if let Some(ref hint) = param.type_hint {
                        let resolved =
                            Self::resolve_type_string(hint, use_map, namespace, &method_skip);
                        if resolved != *hint {
                            param.type_hint = Some(resolved);
                        }
                    }
                }
            }
            for prop in class.properties.make_mut() {
                if let Some(ref hint) = prop.type_hint {
                    let resolved = Self::resolve_type_string(hint, use_map, namespace, &skip_names);
                    if resolved != *hint {
                        prop.type_hint = Some(resolved);
                    }
                }
            }

            // Resolve type names inside `@property` / `@property-read` /
            // `@property-write` and `@method` tags in the raw class
            // docblock.  These tags are parsed lazily by the
            // `PHPDocProvider`, but their type strings use short names
            // relative to the declaring file's imports.  Without
            // resolving them here, cross-file consumers whose own
            // use-map does not import the same names would fail to
            // resolve the types.
            if let Some(ref docblock) = class.class_docblock {
                let resolved_docblock =
                    Self::resolve_docblock_tag_types(docblock, use_map, namespace, &skip_names);
                if resolved_docblock != *docblock {
                    class.class_docblock = Some(resolved_docblock);
                }
            }
        }
    }

    /// Resolve type names in `@property`, `@property-read`, `@property-write`,
    /// and `@method` tags inside a raw class-level docblock.
    ///
    /// These tags are parsed lazily by the `PHPDocProvider`, but their type
    /// strings use short names relative to the declaring file's imports.
    /// This method rewrites those type portions to fully-qualified names
    /// so that cross-file consumers can resolve them without access to the
    /// declaring file's use-map.
    fn resolve_docblock_tag_types(
        docblock: &str,
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
        skip_names: &[String],
    ) -> String {
        let mut result = String::with_capacity(docblock.len());

        for line in docblock.split('\n') {
            if !result.is_empty() {
                result.push('\n');
            }

            let trimmed = line.trim().trim_start_matches('*').trim();

            // ── @property[-read|-write] Type $name ──────────────────
            let prop_rest = trimmed
                .strip_prefix("@property-read")
                .or_else(|| trimmed.strip_prefix("@property-write"))
                .or_else(|| trimmed.strip_prefix("@property"));

            if let Some(rest) = prop_rest {
                let rest_trimmed = rest.trim_start();
                // Must have content after the tag
                if !rest_trimmed.is_empty() && !rest_trimmed.starts_with('$') {
                    // Extract the type token (everything before `$name`).
                    // The type may contain generics like `Collection<int, Model>`
                    // so we use `split_type_token` for correct parsing.
                    let (type_token, _remainder) =
                        crate::docblock::types::split_type_token(rest_trimmed);
                    let resolved_type =
                        Self::resolve_type_string(type_token, use_map, namespace, skip_names);
                    if resolved_type != type_token
                        && let Some(type_start) = line.find(type_token)
                    {
                        let type_end = type_start + type_token.len();
                        result.push_str(&line[..type_start]);
                        result.push_str(&resolved_type);
                        result.push_str(&line[type_end..]);
                        continue;
                    }
                }
            }

            // ── @method [static] ReturnType methodName(…) ───────────
            if let Some(rest) = trimmed.strip_prefix("@method") {
                let rest_trimmed = rest.trim_start();
                if !rest_trimmed.is_empty() {
                    // Skip optional `static` keyword
                    let after_static = if let Some(after) = rest_trimmed.strip_prefix("static") {
                        if after.is_empty()
                            || after.starts_with(char::is_whitespace)
                            || after.starts_with('(')
                        {
                            after.trim_start()
                        } else {
                            rest_trimmed
                        }
                    } else {
                        rest_trimmed
                    };

                    // Find the opening paren — the return type is between
                    // the tag (after optional `static`) and the last
                    // whitespace-delimited token before `(`.
                    if let Some(paren_pos) = after_static.find('(') {
                        let before_paren = after_static[..paren_pos].trim();
                        // Split into optional return type + method name.
                        if let Some(last_space) = before_paren.rfind(|c: char| c.is_whitespace()) {
                            let ret_type = before_paren[..last_space].trim();
                            if !ret_type.is_empty() {
                                let resolved_ret = Self::resolve_type_string(
                                    ret_type, use_map, namespace, skip_names,
                                );
                                if resolved_ret != ret_type
                                    && let Some(type_start) = line.find(ret_type)
                                {
                                    let type_end = type_start + ret_type.len();
                                    result.push_str(&line[..type_start]);
                                    result.push_str(&resolved_ret);
                                    result.push_str(&line[type_end..]);
                                    continue;
                                }
                            }
                        }
                    }
                }
            }

            // No tag matched or no rewriting needed — keep line as-is.
            result.push_str(line);
        }

        result
    }

    /// Resolve type arguments in a generics list (e.g. `@extends`, `@implements`,
    /// `@use`) to fully-qualified names.
    ///
    /// Each entry is `(ClassName, [TypeArg1, TypeArg2, …])`.  The class name
    /// itself is resolved (e.g. `HasFactory` → `App\Concerns\HasFactory`),
    /// and each type argument that looks like a class name (i.e. not a scalar
    /// like `int`, `string`, etc.) is also resolved.
    ///
    /// `skip_names` contains template parameter names that must NOT be
    /// resolved.  Without this, a forwarded template param like `TModel`
    /// in `@use BuildsQueries<TModel>` would be namespace-qualified to
    /// e.g. `Illuminate\Database\Eloquent\TModel`, preventing it from
    /// matching substitution map keys during generic resolution.
    fn resolve_generics_type_args(
        generics: &mut [(String, Vec<String>)],
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
        skip_names: &[String],
    ) {
        for (class_name, type_args) in generics.iter_mut() {
            // Resolve the base class/trait/interface name
            *class_name = Self::resolve_name(class_name, use_map, namespace);

            // Resolve each type argument that is a class-like name
            for arg in type_args.iter_mut() {
                if !is_scalar(arg)
                    && *arg != "mixed"
                    && *arg != "object"
                    && *arg != "static"
                    && *arg != "self"
                    && *arg != "$this"
                    && !skip_names.contains(arg)
                {
                    *arg = Self::resolve_name(arg, use_map, namespace);
                }
            }
        }
    }

    /// Resolve class-like identifiers within a type string to their
    /// fully-qualified forms.
    ///
    /// Walks through the type string token-by-token, identifies class-like
    /// identifiers (words that are not scalars, keywords, or template
    /// params), and resolves each one via `resolve_name`.
    ///
    /// Handles complex type strings including unions (`A|B`), intersections
    /// (`A&B`), nullable (`?A`), generics (`Collection<int, User>`), and
    /// array shapes (`array{name: string, user: User}`).
    ///
    /// # Examples
    /// - `"Country"` → `"Luxplus\\Core\\Enums\\Country"` (via use map)
    /// - `"?Country"` → `"?Luxplus\\Core\\Enums\\Country"`
    /// - `"Country|null"` → `"Luxplus\\Core\\Enums\\Country|null"`
    /// - `"Collection<int, User>"` → `"App\\Collection<int, App\\User>"`
    /// - `"T"` (template param) → `"T"` (unchanged)
    fn resolve_type_string(
        type_str: &str,
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
        skip_names: &[String],
    ) -> String {
        // Keywords that should never be resolved as class names.
        // NOTE: native PHP types (int, string, array, etc.) are caught by
        // `is_scalar()` elsewhere — only pseudo-types and special keywords
        // that slip past that check need to be listed here.
        const TYPE_KEYWORDS: &[&str] = &[
            // ── Special keywords ────────────────────────────────────
            "self",
            "static",
            "parent",
            "$this",
            "mixed",
            "object",
            "void",
            "never",
            "null",
            "true",
            "false",
            "class",
            "callable",
            "scalar",
            "numeric",
            "number",
            "empty",
            // ── Integer refinements ─────────────────────────────────
            "positive-int",
            "negative-int",
            "non-negative-int",
            "non-positive-int",
            "non-zero-int",
            "int-mask",
            "int-mask-of",
            // ── String refinements ──────────────────────────────────
            "non-empty-string",
            "numeric-string",
            "non-falsy-string",
            "truthy-string",
            "literal-string",
            "callable-string",
            "lowercase-string",
            "uppercase-string",
            "non-empty-lowercase-string",
            "non-empty-uppercase-string",
            "non-empty-literal-string",
            // ── Class-string variants ───────────────────────────────
            "class-string",
            "interface-string",
            "trait-string",
            "enum-string",
            // ── Array / list refinements ────────────────────────────
            "list",
            "non-empty-list",
            "non-empty-array",
            "associative-array",
            "array-key",
            // ── Scalar / mixed variants ─────────────────────────────
            "empty-scalar",
            "non-empty-scalar",
            "non-empty-mixed",
            // ── Object / callable variants ──────────────────────────
            "callable-object",
            "callable-array",
            // ── Resource variants ───────────────────────────────────
            "closed-resource",
            "open-resource",
            // ── Never aliases ───────────────────────────────────────
            "no-return",
            "noreturn",
            "never-return",
            "never-returns",
            // ── Key / value projection ──────────────────────────────
            "key-of",
            "value-of",
        ];

        let mut result = String::with_capacity(type_str.len());
        let bytes = type_str.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        // Track brace depth so we can distinguish array shape keys
        // (identifiers before `:` inside `{…}`) from type names.
        let mut brace_depth: u32 = 0;
        // Whether we are in "key position" inside a shape (before the `:`).
        // Reset to true after each `,` or `{` at the current brace level.
        let mut in_shape_key = false;

        while i < len {
            let c = bytes[i] as char;

            // Start of an identifier (letter, underscore, or backslash for FQN)
            if c.is_ascii_alphabetic() || c == '_' || c == '\\' {
                let start = i;
                // Consume the full identifier including namespace separators
                while i < len
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'\\')
                {
                    i += 1;
                }
                let word = &type_str[start..i];

                // Inside `{…}` in key position, identifiers are array shape
                // keys (e.g. `name` in `array{name: string}`), not types.
                if brace_depth > 0 && in_shape_key {
                    result.push_str(word);
                    continue;
                }

                let lower = word.to_ascii_lowercase();
                if is_scalar(word)
                    || TYPE_KEYWORDS.contains(&lower.as_str())
                    || skip_names.iter().any(|s| s == word)
                {
                    // Leave as-is: scalar, keyword, template param,
                    // or type alias name.
                    result.push_str(word);
                } else {
                    // Route through resolve_name, which strips `\`
                    // from already-qualified names and resolves
                    // unqualified names via the use-map / namespace.
                    result.push_str(&Self::resolve_name(word, use_map, namespace));
                }
            } else if c == '$' {
                // Variable reference like `$this` — consume fully
                let start = i;
                i += 1;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                result.push_str(&type_str[start..i]);
            } else {
                // Track brace depth and key/value position for array shapes.
                match c {
                    '{' => {
                        brace_depth += 1;
                        in_shape_key = true;
                    }
                    '}' => {
                        brace_depth = brace_depth.saturating_sub(1);
                        in_shape_key = brace_depth > 0;
                    }
                    ':' if brace_depth > 0 => {
                        // Colon separates key from value type — switch
                        // to value position where identifiers ARE types.
                        in_shape_key = false;
                    }
                    ',' if brace_depth > 0 => {
                        // Comma separates entries — next identifier is a key.
                        in_shape_key = true;
                    }
                    _ => {}
                }
                result.push(c);
                i += 1;
            }
        }

        result
    }

    /// Resolve a class name to its fully-qualified form given a use_map and
    /// namespace context.
    ///
    /// The returned name is **always without a leading `\`**.  This is the
    /// canonical FQN representation used throughout the codebase.  For
    /// example, `\RuntimeException` is returned as `RuntimeException`, and
    /// `\App\Models\User` as `App\Models\User`.
    fn resolve_name(
        name: &str,
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) -> String {
        // 1. Already fully-qualified — strip the leading `\`.
        if let Some(stripped) = name.strip_prefix('\\') {
            return stripped.to_string();
        }

        // 2/3. Check if the (first segment of the) name is in the use_map
        if let Some(pos) = name.find('\\') {
            // Qualified name — check first segment
            let first = &name[..pos];
            let rest = &name[pos..]; // includes leading '\'
            if let Some(fqn) = use_map.get(first) {
                return format!("{}{}", fqn, rest);
            }
        } else {
            // Unqualified name — check directly
            if let Some(fqn) = use_map.get(name) {
                return fqn.clone();
            }
        }

        // 4. Prepend current namespace if available.
        if let Some(ns) = namespace {
            format!("{}\\{}", ns, name)
        } else {
            name.to_string()
        }
    }
}
