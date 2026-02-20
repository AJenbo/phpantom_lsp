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

use crate::Backend;
use crate::composer;
use crate::types::{ClassInfo, FunctionInfo};
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

        // ── Phase 0: Try the class_index for a direct FQN → URI lookup ──
        // This handles classes that don't follow PSR-4 conventions, such as
        // classes defined in Composer autoload_files.php entries.  Using the
        // FQN avoids false positives from short-name collisions.
        if name.contains('\\')
            && let Ok(idx) = self.class_index.lock()
            && let Some(uri) = idx.get(name)
            && let Ok(map) = self.ast_map.lock()
            && let Some(classes) = map.get(uri)
            && let Some(cls) = classes.iter().find(|c| c.name == last_segment)
        {
            return Some(cls.clone());
        }

        // ── Phase 1: Search all already-parsed files in the ast_map ──
        // When the requested name is namespace-qualified (e.g. "Demo\\PDO"),
        // only match classes in files whose namespace matches the expected
        // prefix.  This prevents "Demo\\PDO" from matching the global "PDO"
        // stub that was cached under a different URI.
        if let Ok(map) = self.ast_map.lock() {
            let nmap = self.namespace_map.lock().ok();
            for (uri, classes) in map.iter() {
                if let Some(cls) = classes.iter().find(|c| c.name == last_segment) {
                    // Verify namespace matches when a specific namespace is
                    // expected.
                    if let Some(exp_ns) = expected_ns {
                        let file_ns = nmap
                            .as_ref()
                            .and_then(|nm| nm.get(uri))
                            .and_then(|opt| opt.as_deref());
                        if file_ns != Some(exp_ns) {
                            continue;
                        }
                    }
                    return Some(cls.clone());
                }
            }
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

            if let Ok(content) = std::fs::read_to_string(&file_path) {
                let mut classes = self.parse_php(&content);

                let file_use_map = self.parse_use_statements(&content);
                let file_namespace = self.parse_namespace(&content);
                Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

                let result = classes.iter().find(|c| c.name == last_segment).cloned();

                let uri = format!("file://{}", file_path.display());
                if let Ok(mut map) = self.ast_map.lock() {
                    map.insert(uri.clone(), classes);
                }
                if let Ok(mut map) = self.use_map.lock() {
                    map.insert(uri.clone(), file_use_map);
                }
                if let Ok(mut map) = self.namespace_map.lock() {
                    map.insert(uri, file_namespace);
                }

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
            && let Ok(content) = std::fs::read_to_string(&file_path)
        {
            let mut classes = self.parse_php(&content);

            let file_use_map = self.parse_use_statements(&content);
            let file_namespace = self.parse_namespace(&content);
            Self::resolve_parent_class_names(&mut classes, &file_use_map, &file_namespace);

            let result = classes.iter().find(|c| c.name == last_segment).cloned();

            let uri = format!("file://{}", file_path.display());
            if let Ok(mut map) = self.ast_map.lock() {
                map.insert(uri.clone(), classes);
            }
            if let Ok(mut map) = self.use_map.lock() {
                map.insert(uri.clone(), file_use_map);
            }
            if let Ok(mut map) = self.namespace_map.lock() {
                map.insert(uri, file_namespace);
            }

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
            let mut classes = self.parse_php(stub_content);

            // Stubs are in the root namespace — use an empty use_map / namespace.
            let empty_use_map = std::collections::HashMap::new();
            let no_namespace: Option<String> = None;
            Self::resolve_parent_class_names(&mut classes, &empty_use_map, &no_namespace);

            let result = classes.iter().find(|c| c.name == last_segment).cloned();

            let uri = format!("phpantom-stub://{}", last_segment);
            if let Ok(mut map) = self.ast_map.lock() {
                map.insert(uri, classes);
            }

            return result;
        }

        None
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
                    let empty_use_map = std::collections::HashMap::new();
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
}
