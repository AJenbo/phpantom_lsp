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
use std::sync::Arc;

use std::path::Path;

use crate::Backend;
use crate::composer;
use crate::types::{ClassInfo, FileContext, FunctionInfo, PhpVersion};
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
    /// Returns a shared `Arc<ClassInfo>` if found, or `None`.
    pub(crate) fn find_or_load_class(&self, class_name: &str) -> Option<Arc<ClassInfo>> {
        // The class name stored in ClassInfo is just the short name (e.g. "Customer"),
        // so we match against the last segment of the namespace-qualified name.
        let last_segment = short_name(class_name);

        // Extract the expected namespace prefix (if any).
        // For "Demo\\PDO" → expected_ns = Some("Demo")
        // For "PDO"       → expected_ns = None (global scope)
        let expected_ns: Option<&str> = if class_name.contains('\\') {
            Some(&class_name[..class_name.len() - last_segment.len() - 1])
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
        if let Some(file_path) = self.classmap.read().get(class_name).cloned()
            && let Some(classes) = self.parse_and_cache_file(&file_path)
            && let Some(cls) = classes.iter().find(|c| c.name == last_segment)
        {
            return Some(Arc::clone(cls));
        }

        // ── Phase 2: Try PSR-4 resolution ──
        // PSR-4 mappings come exclusively from composer.json (user code).
        // Vendor code is covered by the classmap (Phase 1.5).  If a
        // vendor class is missing from the classmap, it fails visibly
        // rather than being silently resolved, making stale classmaps
        // obvious (fix: run `composer dump-autoload`).
        if let Some(workspace_root) = self.workspace_root.read().clone() {
            let file_path = {
                let mappings = self.psr4_mappings.read();
                composer::resolve_class_path(&mappings, &workspace_root, class_name)
            };
            if let Some(file_path) = file_path
                && let Some(classes) = self.parse_and_cache_file(&file_path)
                && let Some(cls) = classes.iter().find(|c| c.name == last_segment)
            {
                return Some(Arc::clone(cls));
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
            let ver = Some(self.php_version());
            if let Some(classes) =
                self.parse_and_cache_content_versioned(stub_content, &stub_uri, ver)
                && let Some(cls) = classes.iter().find(|c| c.name == last_segment)
            {
                return Some(Arc::clone(cls));
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
    pub(crate) fn parse_and_cache_file(&self, file_path: &Path) -> Option<Vec<Arc<ClassInfo>>> {
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
    ) -> Option<Vec<Arc<ClassInfo>>> {
        self.parse_and_cache_content_versioned(content, uri, None)
    }

    /// Version-aware variant of [`parse_and_cache_content`].
    ///
    /// When `php_version` is `Some`, elements annotated with
    /// `#[PhpStormStubsElementAvailable]` whose version range excludes
    /// the target version are filtered out during extraction.  Used when
    /// parsing phpstorm-stubs so that only the correct variant of each
    /// function, method, or parameter is presented.
    ///
    /// # Consistency model
    ///
    /// The five maps (`ast_map`, `use_map`, `namespace_map`, `fqn_index`,
    /// `resolved_class_cache`) are written sequentially, not under a
    /// single lock.  A concurrent reader could briefly observe a state
    /// where some maps reflect the new parse while others still hold
    /// stale data for the same URI.  This is acceptable because:
    ///
    /// - All writes complete within microseconds.
    /// - Every consumer clones the data it needs from each map
    ///   independently and does not rely on cross-map atomicity.
    /// - An audit of all read sites (completion, diagnostics, hover,
    ///   definition, references, highlighting) confirmed that none
    ///   requires a consistent snapshot across multiple maps.
    ///
    /// If a future change adds a reader that checks two of these maps
    /// for consistency within the same request, the writes here must
    /// be batched under a single coordination mechanism.
    pub(crate) fn parse_and_cache_content_versioned(
        &self,
        content: &str,
        uri: &str,
        php_version: Option<PhpVersion>,
    ) -> Option<Vec<Arc<ClassInfo>>> {
        let mut classes = Self::parse_php_versioned(content, php_version);
        let file_use_map = self.parse_use_statements(content);
        let file_namespace = self.parse_namespace(content);
        Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

        // Set the per-class file_namespace so that classes loaded via
        // PSR-4 / classmap carry their namespace.  This mirrors the
        // same assignment done in `update_ast_inner` for files opened
        // through `did_open` / `did_change`.
        for cls in &mut classes {
            if cls.file_namespace.is_none() {
                cls.file_namespace = file_namespace.clone();
            }
        }

        // Wrap each ClassInfo in Arc before inserting into the maps.
        let arc_classes: Vec<Arc<ClassInfo>> = classes.into_iter().map(Arc::new).collect();

        self.ast_map
            .write()
            .insert(uri.to_owned(), arc_classes.clone());
        self.use_map.write().insert(uri.to_owned(), file_use_map);
        self.namespace_map
            .write()
            .insert(uri.to_owned(), file_namespace);

        // Populate the fqn_index so that `find_class_in_ast_map` can
        // resolve these classes via O(1) hash lookup.
        {
            let mut fqn_idx = self.fqn_index.write();
            for cls in &arc_classes {
                if cls.name.starts_with("__anonymous@") {
                    continue;
                }
                let fqn = match &cls.file_namespace {
                    Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, cls.name),
                    _ => cls.name.clone(),
                };
                fqn_idx.insert(fqn, Arc::clone(cls));
            }
        }

        // Selectively invalidate the resolved-class cache for the
        // classes defined in this file.  Loading a new file from disk
        // (classmap, PSR-4, stubs) should not nuke cached resolutions
        // for unrelated classes.  Only the FQNs we just parsed need
        // to be evicted — their old (if any) cache entries were built
        // without the members we just loaded.
        {
            let mut cache = self.resolved_class_cache.lock();
            for cls in &arc_classes {
                let fqn = match &cls.file_namespace {
                    Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, cls.name),
                    _ => cls.name.clone(),
                };
                crate::virtual_members::evict_fqn(&mut cache, &fqn);
            }
        }

        Some(arc_classes)
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
        {
            let fmap = self.global_functions.read();
            for &name in candidates {
                if let Some((_, info)) = fmap.get(name) {
                    return Some(info.clone());
                }
            }
        }

        // ── Phase 1.5: Check autoload_function_index (byte-level scan) ──
        // The lightweight `find_symbols` byte-level scan discovers
        // function names at startup without a full AST parse, for both
        // non-Composer projects (workspace scan) and Composer projects
        // (autoload_files.php scan).  When a candidate matches here, we
        // lazily call `update_ast` on the file to get a complete
        // `FunctionInfo` and cache it in global_functions so subsequent
        // lookups hit Phase 1.
        //
        // Note: the lazy parse is a full AST parse (`update_ast`), which
        // is the same cost as opening the file.  This is acceptable
        // because it only happens once per function, on first access.
        {
            let idx = self.autoload_function_index.read();
            for &name in candidates {
                if let Some(path) = idx.get(name) {
                    let path = path.clone();
                    drop(idx); // release read lock before parsing

                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let uri = format!("file://{}", path.display());
                        self.update_ast(&uri, &content);

                        // Re-check global_functions after parsing.
                        let fmap = self.global_functions.read();
                        for &retry_name in candidates {
                            if let Some((_, info)) = fmap.get(retry_name) {
                                return Some(info.clone());
                            }
                        }
                    }
                    break; // Only try one file per lookup
                }
            }
        }

        // ── Phase 1.75: Last-resort lazy parse of known autoload files ──
        // The byte-level scanner misses functions wrapped in
        // `if (! function_exists(...))` guards (brace depth > 0).
        // These are common in Laravel helpers and similar packages.
        // As a safety net, lazily parse each known autoload file via
        // `update_ast` until the function is found.  Each file is
        // parsed at most once: subsequent lookups hit Phase 1
        // (`global_functions`).
        {
            let paths = self.autoload_file_paths.read().clone();
            for path in &paths {
                // Skip files that have already been fully parsed (their
                // functions are already in global_functions via Phase 1).
                let uri = format!("file://{}", path.display());
                if self.ast_map.read().contains_key(&uri) {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(path) {
                    self.update_ast(&uri, &content);

                    let fmap = self.global_functions.read();
                    for &name in candidates {
                        if let Some((_, info)) = fmap.get(name) {
                            return Some(info.clone());
                        }
                    }
                }
            }
        }

        // ── Phase 2: Try embedded PHP stubs ──
        // The stub_function_index maps function names (including namespaced
        // ones like "Brotli\\compress") to the raw PHP source of the file
        // that defines them.  We parse the entire file, cache all discovered
        // functions in global_functions, and return the one we need.
        for &name in candidates {
            if let Some(&stub_content) = self.stub_function_index.get(name) {
                let ver = Some(self.php_version());
                let functions = self.parse_functions_versioned(stub_content, ver);

                if functions.is_empty() {
                    continue;
                }

                let stub_uri = format!("phpantom-stub-fn://{}", name);
                let mut result: Option<FunctionInfo> = None;

                {
                    let mut fmap = self.global_functions.write();
                    for func in &functions {
                        let fqn = if let Some(ref ns) = func.namespace {
                            format!("{}\\{}", ns, &func.name)
                        } else {
                            func.name.clone()
                        };

                        // Check if this is the function we're looking for.
                        if result.is_none() && (fqn == name || func.name == name) {
                            result = Some(func.clone());
                        }

                        // Cache the FQN so future lookups hit Phase 1.
                        // No short-name fallback: `resolve_function_name`
                        // already builds namespace-qualified candidates.
                        fmap.entry(fqn)
                            .or_insert_with(|| (stub_uri.clone(), func.clone()));
                    }
                }

                // Also cache any classes defined in the same stub file so
                // that class lookups for types referenced by the function
                // (e.g. return types) can find them later.
                let mut classes = Self::parse_php_versioned(stub_content, ver);
                if !classes.is_empty() {
                    let empty_use_map = HashMap::new();
                    let stub_namespace = self.parse_namespace(stub_content);
                    Self::resolve_parent_class_names(&mut classes, &empty_use_map, &stub_namespace);
                    let class_uri = format!("phpantom-stub-fn://{}", name);
                    let arc_classes: Vec<Arc<ClassInfo>> =
                        classes.into_iter().map(Arc::new).collect();
                    self.ast_map.write().insert(class_uri, arc_classes);
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
        local_classes: &[Arc<ClassInfo>],
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Option<Arc<ClassInfo>> {
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
                return Some(Arc::clone(cls));
            }
            // In a namespace, try the namespace-qualified form first.
            // Per PHP semantics, class names do NOT fall back to global
            // scope (unlike functions/constants).  However, names that
            // arrive here may be already-resolved FQNs from ClassInfo
            // fields (e.g. `parent_class = "Exception"`) that happen to
            // be single-segment global names.  For those, the namespace-
            // qualified attempt will fail, so we fall back to a direct
            // lookup.  To preserve PHP semantics for user-typed code,
            // the namespace-qualified form is tried first and wins when
            // a same-named class exists in the current namespace.
            if let Some(ns) = file_namespace {
                let ns_qualified = format!("{}\\{}", ns, name);
                if let Some(cls) = self.find_or_load_class(&ns_qualified) {
                    return Some(cls);
                }
            }
            // Global scope: either no namespace context, or the
            // namespace-qualified lookup above did not find a match.
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
    ) -> impl Fn(&str) -> Option<Arc<ClassInfo>> + 'a {
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
        classes: &'a [Arc<ClassInfo>],
        use_map: &'a HashMap<String, String>,
        namespace: &'a Option<String>,
    ) -> impl Fn(&str) -> Option<Arc<ClassInfo>> + 'a {
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
