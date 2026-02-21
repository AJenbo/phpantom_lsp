/// Cross-file class and function resolution.
///
/// This module contains the heavyweight name-resolution logic that is
/// shared by the completion handler, definition resolver, and
/// named-argument resolution.  It was extracted from `util.rs` so that
/// module can focus on simple helper functions.
///
/// # Resolution pipeline
///
/// ## Class resolution ([`Backend::find_or_load_class`])
///
///   0. **Class index** — direct FQN → URI lookup (covers non-PSR-4 classes)
///   1. **ast_map scan** — search all already-parsed files by short name,
///      with namespace verification when a qualified name is requested
///      1.5. **Composer classmap** — `vendor/composer/autoload_classmap.php`
///      direct FQN → file lookup (covers optimised autoloaders)
///   2. **PSR-4 resolution** — convert namespace to file path and parse
///   3. **Embedded stubs** — built-in PHP classes/interfaces bundled in
///      the binary (e.g. `UnitEnum`, `BackedEnum`, `Iterator`)
///
/// ## Function resolution ([`Backend::find_or_load_function`])
///
///   1. **global_functions** — user code + already-cached stubs
///   2. **Embedded stubs** — built-in PHP functions from phpstorm-stubs
///
/// ## Name resolution ([`Backend::resolve_class_name`], [`Backend::resolve_function_name`])
///
///   These methods take a raw name as it appears in source code and resolve
///   it to a concrete `ClassInfo` or `FunctionInfo` using the file's `use`
///   statement mappings and namespace context.  They handle:
///
///   - Fully-qualified names (`\PDO`, `\Couchbase\Cluster`)
///   - Unqualified names resolved via the import table or current namespace
///   - Qualified names with alias expansion and namespace prefixing
use std::collections::HashMap;

use std::path::Path;

use crate::Backend;
use crate::composer;
use crate::types::{ClassInfo, FileContext, FunctionInfo};
use crate::util::short_name;

impl Backend {
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
        let last_segment = short_name(name);

        // Extract the expected namespace prefix (if any).
        // For "Demo\\PDO" → expected_ns = Some("Demo")
        // For "PDO"       → expected_ns = None (global scope)
        let expected_ns: Option<&str> = if name.contains('\\') {
            Some(&name[..name.len() - last_segment.len() - 1])
        } else {
            None
        };

        // ── Phase 1: Search all already-parsed files in the ast_map ──
        // Checks short name + namespace to avoid false positives (e.g.
        // "Demo\\PDO" won't match the global "PDO" stub).
        if let Some(cls) = self.find_class_in_ast_map(class_name) {
            return Some(cls);
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

            if let Some(classes) = self.parse_and_cache_file(&file_path) {
                let result = classes.iter().find(|c| c.name == last_segment).cloned();
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
            && let Some(classes) = self.parse_and_cache_file(&file_path)
        {
            let result = classes.iter().find(|c| c.name == last_segment).cloned();
            if result.is_some() {
                return result;
            }
        }

        // ── Phase 3: Try embedded PHP stubs ──
        // Stubs are bundled in the binary for built-in classes/interfaces
        // (e.g. UnitEnum, BackedEnum).  Parse on first access and cache in
        // the ast_map under a `phpantom-stub://` URI so subsequent lookups
        // hit Phase 1 and skip parsing entirely.
        //
        // Stubs live in the global namespace, so skip this phase when the
        // caller is looking for a class in a specific namespace (e.g.
        // "Demo\\PDO" should NOT match the global PDO stub).
        if expected_ns.is_none()
            && let Some(&stub_content) = self.stub_index.get(last_segment)
        {
            let stub_uri = format!("phpantom-stub://{}", last_segment);
            if let Some(classes) = self.parse_and_cache_content(stub_content, &stub_uri) {
                let result = classes.iter().find(|c| c.name == last_segment).cloned();
                if result.is_some() {
                    return result;
                }
            }
        }

        None
    }

    /// Parse a PHP file from disk, cache the results, and return the
    /// extracted classes.
    ///
    /// Convenience wrapper around [`parse_and_cache_content`] that reads
    /// the file and derives a `file://` URI from the path.  Used by
    /// [`find_or_load_class`] (Phases 1.5 and 2) and by the
    /// go-to-implementation scanner.
    pub(crate) fn parse_and_cache_file(&self, file_path: &Path) -> Option<Vec<ClassInfo>> {
        let content = std::fs::read_to_string(file_path).ok()?;
        let uri = format!("file://{}", file_path.display());
        self.parse_and_cache_content(&content, &uri)
    }

    /// Parse PHP source text, cache the results in
    /// `ast_map`/`use_map`/`namespace_map`, and return the extracted
    /// classes.
    ///
    /// This is the single canonical implementation of the "parse → cache"
    /// pipeline.  All code paths that need to parse PHP content and store
    /// the results (file-based resolution, stub resolution, implementation
    /// scanning) funnel through here so the caching logic stays consistent.
    pub(crate) fn parse_and_cache_content(
        &self,
        content: &str,
        uri: &str,
    ) -> Option<Vec<ClassInfo>> {
        let mut classes = self.parse_php(content);
        let file_use_map = self.parse_use_statements(content);
        let file_namespace = self.parse_namespace(content);
        Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri.to_owned(), classes.clone());
        }
        if let Ok(mut map) = self.use_map.lock() {
            map.insert(uri.to_owned(), file_use_map);
        }
        if let Ok(mut map) = self.namespace_map.lock() {
            map.insert(uri.to_owned(), file_namespace);
        }

        Some(classes)
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
                    let empty_use_map = HashMap::new();
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

    // ─── Shared Name Resolution ─────────────────────────────────────────────

    /// Resolve a class name using use-map, namespace, local classes, and
    /// cross-file / PSR-4 / stubs.
    ///
    /// This is the single canonical implementation of the "class_loader"
    /// logic used by the completion handler, definition resolver, and
    /// named-argument resolution.  It handles:
    ///
    ///   - Fully-qualified names (`\PDO`, `\Couchbase\Cluster`)
    ///   - Unqualified names resolved via the import table (`use` statements),
    ///     local class list, current namespace, or global scope
    ///   - Qualified names with alias expansion and namespace prefixing
    pub(crate) fn resolve_class_name(
        &self,
        name: &str,
        local_classes: &[ClassInfo],
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Option<ClassInfo> {
        // ── Fully qualified name (leading `\`) ──────────────
        if let Some(stripped) = name.strip_prefix('\\') {
            return self.find_or_load_class(stripped);
        }

        // ── Unqualified name (no `\` at all) ────────────────
        if !name.contains('\\') {
            // Check the import table first (`use` statements).
            if let Some(fqn) = file_use_map.get(name) {
                return self.find_or_load_class(fqn);
            }
            // Check local classes (same-file shortcut).
            let lookup = short_name(name);
            if let Some(cls) = local_classes.iter().find(|c| c.name == lookup) {
                return Some(cls.clone());
            }
            // In a namespace, prepend the current namespace.
            // Class names do NOT fall back to global scope —
            // unlike functions/constants.  See:
            // https://www.php.net/manual/en/language.namespaces.fallback.php
            if let Some(ns) = file_namespace {
                let ns_qualified = format!("{}\\{}", ns, name);
                return self.find_or_load_class(&ns_qualified);
            }
            // No namespace — we're in global scope already.
            return self.find_or_load_class(name);
        }

        // ── Qualified name (contains `\`, no leading `\`) ───
        // Check if the first segment is a use-map alias
        // (e.g. `OA\Endpoint` where `use Swagger\OpenAPI as OA;`
        // maps `OA` → `Swagger\OpenAPI`).  Expand to FQN.
        let first_segment = name.split('\\').next().unwrap_or(name);
        if let Some(fqn_prefix) = file_use_map.get(first_segment) {
            let rest = &name[first_segment.len()..];
            let expanded = format!("{}{}", fqn_prefix, rest);
            if let Some(cls) = self.find_or_load_class(&expanded) {
                return Some(cls);
            }
        }
        // Prepend current namespace (if any).
        if let Some(ns) = file_namespace {
            let ns_qualified = format!("{}\\{}", ns, name);
            if let Some(cls) = self.find_or_load_class(&ns_qualified) {
                return Some(cls);
            }
        }
        // Fall back to the name as-is.  Qualified names that
        // reach here are typically already-resolved FQNs from
        // the parser (parent classes, traits, mixins) that
        // were resolved by `resolve_parent_class_names` before
        // being stored.
        self.find_or_load_class(name)
    }

    /// Resolve a function name using use-map and namespace context.
    ///
    /// Builds a list of candidate names (exact name, use-map resolved,
    /// namespace-qualified) and tries each via `find_or_load_function`.
    ///
    /// This is the single canonical implementation of the "function_loader"
    /// logic used by both the completion handler and definition resolver.
    pub(crate) fn resolve_function_name(
        &self,
        name: &str,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Option<FunctionInfo> {
        // Build candidate names to try: exact name, use-map
        // resolved name, and namespace-qualified name.
        let mut candidates: Vec<&str> = vec![name];

        let use_resolved: Option<String> = file_use_map.get(name).cloned();
        if let Some(ref fqn) = use_resolved {
            candidates.push(fqn.as_str());
        }

        let ns_qualified: Option<String> = file_namespace
            .as_ref()
            .map(|ns| format!("{}\\{}", ns, name));
        if let Some(ref nq) = ns_qualified {
            candidates.push(nq.as_str());
        }

        // Unified lookup: checks global_functions first, then
        // falls back to embedded PHP stubs (parsed lazily and
        // cached for future lookups).
        self.find_or_load_function(&candidates)
    }

    // ─── Loader Closure Factories ───────────────────────────────────────

    /// Return a class-loader closure bound to a [`FileContext`].
    ///
    /// This is the convenience wrapper for the common case where the
    /// caller already has a `FileContext`.  For situations that need a
    /// different class list (e.g. patched/effective classes after error
    /// recovery), use [`class_loader_with`](Self::class_loader_with).
    pub(crate) fn class_loader<'a>(
        &'a self,
        ctx: &'a FileContext,
    ) -> impl Fn(&str) -> Option<ClassInfo> + 'a {
        self.class_loader_with(&ctx.classes, &ctx.use_map, &ctx.namespace)
    }

    /// Return a class-loader closure from individual file-context
    /// components.
    ///
    /// Useful when the class list differs from what is stored in a
    /// `FileContext` (e.g. after re-parsing patched content for error
    /// recovery).
    pub(crate) fn class_loader_with<'a>(
        &'a self,
        classes: &'a [ClassInfo],
        use_map: &'a HashMap<String, String>,
        namespace: &'a Option<String>,
    ) -> impl Fn(&str) -> Option<ClassInfo> + 'a {
        move |name: &str| self.resolve_class_name(name, classes, use_map, namespace)
    }

    /// Return a function-loader closure bound to a [`FileContext`].
    ///
    /// This is the convenience wrapper for the common case where the
    /// caller already has a `FileContext`.  For situations that need
    /// explicit use-map / namespace values, use
    /// [`function_loader_with`](Self::function_loader_with).
    pub(crate) fn function_loader<'a>(
        &'a self,
        ctx: &'a FileContext,
    ) -> impl Fn(&str) -> Option<FunctionInfo> + 'a {
        self.function_loader_with(&ctx.use_map, &ctx.namespace)
    }

    /// Return a function-loader closure from individual file-context
    /// components.
    ///
    /// Useful when the caller does not have a full `FileContext` or
    /// needs to use a different use-map / namespace.
    pub(crate) fn function_loader_with<'a>(
        &'a self,
        use_map: &'a HashMap<String, String>,
        namespace: &'a Option<String>,
    ) -> impl Fn(&str) -> Option<FunctionInfo> + 'a {
        move |name: &str| self.resolve_function_name(name, use_map, namespace)
    }
}
