/// Class name, constant, and function completions.
///
/// This module handles building completion items for bare identifiers
/// (class names, global constants, and standalone functions) when no
/// member-access operator (`->` or `::`) is present.
///
/// Also provides a Throwable-filtered variant for catch clause fallback
/// and `throw new` completion, which only suggests exception classes
/// from already-parsed sources and includes everything else (classmap,
/// stubs) unfiltered.
use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;
use crate::util::short_name;

use super::builder::{build_callable_snippet, build_use_edit, find_use_insert_position};

/// Heuristic check for class names that are unlikely to be instantiable.
///
/// Returns `true` when the short name matches common naming conventions
/// for abstract classes and interfaces:
///
/// - **Abstract:** case-insensitive `"abstract"` as prefix or suffix
///   (e.g. `AbstractController`, `HandlerAbstract`)
/// - **Interface:** case-insensitive `"interface"` as suffix
///   (e.g. `LoggerInterface`)
/// - **I-prefix:** `I` followed by an uppercase letter
///   (e.g. `ILogger`, `IRepository` — C#-style interface naming)
/// - **Base-prefix:** `Base` followed by an uppercase letter
///   (e.g. `BaseController`, `BaseModel`)
fn likely_non_instantiable(short_name: &str) -> bool {
    let lower = short_name.to_ascii_lowercase();
    if lower.ends_with("abstract") || lower.starts_with("abstract") || lower.ends_with("interface")
    {
        return true;
    }
    // I[A-Z] prefix — C#-style interface naming (ILogger, IRepository).
    // The length check + ascii uppercase guard avoids matching `Image`, `Item`, etc.
    if short_name.starts_with('I') && short_name.len() >= 2 {
        let second = short_name.as_bytes()[1];
        if second.is_ascii_uppercase() {
            return true;
        }
    }
    // Base[A-Z] prefix (BaseController, BaseModel).
    // Requires uppercase after "Base" to avoid matching e.g. `Baseline`.
    if short_name.starts_with("Base") && short_name.len() >= 5 {
        let fifth = short_name.as_bytes()[4];
        if fifth.is_ascii_uppercase() {
            return true;
        }
    }
    false
}

impl Backend {
    /// Extract the partial identifier (class name fragment) that the user
    /// is currently typing at the given cursor position.
    ///
    /// Walks backward from the cursor through alphanumeric characters,
    /// underscores, and backslashes (namespace separators).  Returns
    /// `None` if the resulting text starts with `$` (variable context)
    /// or is empty.
    pub fn extract_partial_class_name(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards through identifier characters (including `\`)
        let mut i = col;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        if i == col {
            // Nothing typed — no partial identifier
            return None;
        }

        // If preceded by `$`, this is a variable, not a class name
        if i > 0 && chars[i - 1] == '$' {
            return None;
        }

        // If preceded by `->` or `::`, member completion handles this
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            return None;
        }
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            return None;
        }

        let partial: String = chars[i..col].iter().collect();
        if partial.is_empty() {
            return None;
        }

        Some(partial)
    }

    /// Detect whether the cursor is in a `throw new ClassName` context.
    ///
    /// Returns `true` when the text immediately before the partial
    /// identifier (at the cursor) is `throw new` (with optional
    /// whitespace).  This tells the handler to restrict completion to
    /// Throwable descendants only and skip constants / functions.
    pub fn is_throw_new_context(content: &str, position: Position) -> bool {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return false;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backward past the partial identifier (same logic as
        // extract_partial_class_name) to find where it starts.
        let mut i = col;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        // Now skip whitespace before the identifier
        let mut j = i;
        while j > 0 && chars[j - 1] == ' ' {
            j -= 1;
        }

        // Check for `new` keyword
        if j >= 3
            && chars[j - 3] == 'n'
            && chars[j - 2] == 'e'
            && chars[j - 1] == 'w'
            && (j < 4 || !chars[j - 4].is_alphanumeric())
        {
            // Skip whitespace before `new`
            let mut k = j - 3;
            while k > 0 && chars[k - 1] == ' ' {
                k -= 1;
            }

            // Check for `throw` keyword
            if k >= 5
                && chars[k - 5] == 't'
                && chars[k - 4] == 'h'
                && chars[k - 3] == 'r'
                && chars[k - 2] == 'o'
                && chars[k - 1] == 'w'
                && (k < 6 || !chars[k - 6].is_alphanumeric())
            {
                return true;
            }
        }

        false
    }

    /// Return `true` when the cursor sits in a `new ClassName` context —
    /// i.e. the partial identifier is immediately preceded by the `new`
    /// keyword (with optional whitespace).  This is used to enrich class
    /// name completions with constructor parameter snippets.
    pub fn is_new_context(content: &str, position: Position) -> bool {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return false;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backward past the partial identifier.
        let mut i = col;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        // Skip whitespace before the identifier.
        let mut j = i;
        while j > 0 && chars[j - 1] == ' ' {
            j -= 1;
        }

        // Check for the `new` keyword, ensuring it isn't part of a
        // longer identifier (e.g. `renew`).
        j >= 3
            && chars[j - 3] == 'n'
            && chars[j - 2] == 'e'
            && chars[j - 1] == 'w'
            && (j < 4 || !chars[j - 4].is_alphanumeric())
    }

    /// Build `(insert_text, insert_text_format)` for a class in `new` context.
    ///
    /// When `ctor_params` is `Some`, those constructor parameters are used
    /// to build a snippet with tab-stops for each required argument.
    /// When `None`, a plain `Name()$0` snippet is returned so the user
    /// still gets parentheses inserted automatically.
    fn build_new_insert(
        short_name: &str,
        ctor_params: Option<&[ParameterInfo]>,
    ) -> (String, Option<InsertTextFormat>) {
        let snippet = if let Some(p) = ctor_params {
            build_callable_snippet(short_name, p)
        } else {
            // No constructor info available — insert empty parens.
            format!("{short_name}()$0")
        };

        (snippet, Some(InsertTextFormat::SNIPPET))
    }

    /// Build completion items for class names from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Classes imported via `use` statements in the current file
    ///   2. Classes in the same namespace (from the ast_map)
    ///   3. Classes from the class_index (discovered during parsing)
    ///   4. Classes from the Composer classmap (`autoload_classmap.php`)
    ///   5. Built-in PHP classes from embedded stubs
    ///
    /// Each item uses the short class name as `label` and the
    /// fully-qualified name as `detail`.  Items are deduplicated by FQN.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching classes exceeds [`MAX_CLASS_COMPLETIONS`], the result is
    /// truncated and `is_incomplete` is `true`, signalling the client to
    /// re-request as the user types more characters.
    const MAX_CLASS_COMPLETIONS: usize = 100;

    pub(crate) fn build_class_name_completions(
        &self,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        prefix: &str,
        content: &str,
        is_new: bool,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen_fqns: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // Pre-compute the insertion position for `use` statements.
        // Only items from sources 3–5 (not already imported, not same
        // namespace) will carry an `additional_text_edits` entry.
        let use_insert_pos = find_use_insert_position(content);

        // ── 1. Use-imported classes (highest priority) ──────────────
        for (short_name, fqn) in file_use_map {
            if !short_name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen_fqns.insert(fqn.clone()) {
                continue;
            }
            // After `new`, skip interfaces, traits, enums and abstract
            // classes that we already know about.
            if is_new && !self.is_instantiable_or_unloaded(fqn) {
                continue;
            }
            let (insert_text, insert_text_format) = if is_new {
                Self::build_new_insert(short_name, None)
            } else {
                (short_name.clone(), None)
            };
            items.push(CompletionItem {
                label: short_name.clone(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(fqn.clone()),
                insert_text: Some(insert_text),
                insert_text_format,
                filter_text: Some(short_name.clone()),
                sort_text: Some(format!("0_{}", short_name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        // ── 2. Same-namespace classes (from ast_map) ────────────────
        if let Some(ns) = file_namespace
            && let Ok(nmap) = self.namespace_map.lock()
        {
            // Find all URIs that share the same namespace
            let same_ns_uris: Vec<String> = nmap
                .iter()
                .filter_map(|(uri, opt_ns)| {
                    if opt_ns.as_deref() == Some(ns.as_str()) {
                        Some(uri.clone())
                    } else {
                        None
                    }
                })
                .collect();
            drop(nmap);

            if let Ok(amap) = self.ast_map.lock() {
                for uri in &same_ns_uris {
                    if let Some(classes) = amap.get(uri) {
                        for cls in classes {
                            if !cls.name.to_lowercase().contains(&prefix_lower) {
                                continue;
                            }
                            // After `new`, only concrete classes are valid.
                            if is_new && (cls.kind != ClassLikeKind::Class || cls.is_abstract) {
                                continue;
                            }
                            let fqn = format!("{}\\{}", ns, cls.name);
                            if !seen_fqns.insert(fqn.clone()) {
                                continue;
                            }
                            let (insert_text, insert_text_format) = if is_new {
                                // We already have the ClassInfo — check
                                // for __construct directly.
                                let ctor_params: Option<Vec<ParameterInfo>> = cls
                                    .methods
                                    .iter()
                                    .find(|m| m.name.eq_ignore_ascii_case("__construct"))
                                    .map(|m| m.parameters.clone());
                                Self::build_new_insert(&cls.name, ctor_params.as_deref())
                            } else {
                                (cls.name.clone(), None)
                            };
                            items.push(CompletionItem {
                                label: cls.name.clone(),
                                kind: Some(CompletionItemKind::CLASS),
                                detail: Some(fqn),
                                insert_text: Some(insert_text),
                                insert_text_format,
                                filter_text: Some(cls.name.clone()),
                                sort_text: Some(format!("1_{}", cls.name.to_lowercase())),
                                deprecated: if cls.is_deprecated { Some(true) } else { None },
                                ..CompletionItem::default()
                            });
                        }
                    }
                }
            }
        }

        // ── 3. class_index (discovered / interacted-with classes) ───
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                let short_name = short_name(fqn);
                if !short_name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                // After `new`, skip non-concrete classes that are loaded.
                if is_new && !self.is_instantiable_or_unloaded(fqn) {
                    continue;
                }
                let (insert_text, insert_text_format) = if is_new {
                    // class_index is a FQN → URI map; the class may or
                    // may not be fully loaded — just insert empty parens.
                    (format!("{short_name}()$0"), Some(InsertTextFormat::SNIPPET))
                } else {
                    (short_name.to_string(), None)
                };
                // In `new` context, demote names that look like abstract
                // classes or interfaces so concrete-looking names appear first.
                let sort_prefix = if is_new && likely_non_instantiable(short_name) {
                    "7"
                } else {
                    "2"
                };
                items.push(CompletionItem {
                    label: short_name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text),
                    insert_text_format,
                    filter_text: Some(short_name.to_string()),
                    sort_text: Some(format!("{}_{}", sort_prefix, short_name.to_lowercase())),
                    additional_text_edits: build_use_edit(fqn, use_insert_pos, file_namespace),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 4. Composer classmap (all autoloaded classes) ───────────
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                let short_name = short_name(fqn);
                if !short_name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                let (insert_text, insert_text_format) = if is_new {
                    Self::build_new_insert(short_name, None)
                } else {
                    (short_name.to_string(), None)
                };
                // In `new` context, demote names that look like abstract
                // classes or interfaces so concrete-looking names appear first.
                let sort_prefix = if is_new && likely_non_instantiable(short_name) {
                    "8"
                } else {
                    "3"
                };
                items.push(CompletionItem {
                    label: short_name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text),
                    insert_text_format,
                    filter_text: Some(short_name.to_string()),
                    sort_text: Some(format!("{}_{}", sort_prefix, short_name.to_lowercase())),
                    additional_text_edits: build_use_edit(fqn, use_insert_pos, file_namespace),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 5. Built-in PHP classes from stubs (lowest priority) ────
        for &name in self.stub_index.keys() {
            let short_name = short_name(name);
            if !short_name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen_fqns.insert(name.to_string()) {
                continue;
            }
            let (insert_text, insert_text_format) = if is_new {
                // Stub classes are not parsed yet — just insert empty
                // parens without attempting a lookup.
                (format!("{short_name}()$0"), Some(InsertTextFormat::SNIPPET))
            } else {
                (short_name.to_string(), None)
            };
            // In `new` context, demote names that look like abstract
            // classes or interfaces so concrete-looking names appear first.
            let sort_prefix = if is_new && likely_non_instantiable(short_name) {
                "9"
            } else {
                "4"
            };
            items.push(CompletionItem {
                label: short_name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(name.to_string()),
                insert_text: Some(insert_text),
                insert_text_format,
                filter_text: Some(short_name.to_string()),
                sort_text: Some(format!("{}_{}", sort_prefix, short_name.to_lowercase())),
                additional_text_edits: build_use_edit(name, use_insert_pos, file_namespace),
                ..CompletionItem::default()
            });
        }

        // Cap the result set so the client isn't overwhelmed.
        // Sort by sort_text first so that higher-priority items
        // (use-imports, same-namespace, user project classes) survive
        // the truncation ahead of lower-priority SPL stubs.
        let is_incomplete = items.len() > Self::MAX_CLASS_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CLASS_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Catch clause / throw-new fallback completion ───────────────

    /// Check whether a class is a confirmed `\Throwable` descendant using
    /// only already-loaded data from the `ast_map`.
    ///
    /// Returns `true` only when the full parent chain can be walked to
    /// one of the three Throwable root types (`Throwable`, `Exception`,
    /// `Error`).  Returns `false` if the chain is broken (parent not
    /// loaded) or terminates at a non-Throwable class.
    ///
    /// This is a strict check: the caller should only include the class
    /// when this returns `true`.
    ///
    /// This never triggers disk I/O; it only consults `ast_map`.
    fn is_throwable_descendant(&self, class_name: &str, depth: u32) -> bool {
        if depth > 20 {
            return false; // prevent infinite loops
        }

        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let short = short_name(normalized);

        // These three types form the root of PHP's exception hierarchy.
        if matches!(short, "Throwable" | "Exception" | "Error") {
            return true;
        }

        // Look up ClassInfo from ast_map (no disk I/O).
        let last_segment = short_name(normalized);
        let expected_ns: Option<&str> = if normalized.contains('\\') {
            Some(&normalized[..normalized.len() - last_segment.len() - 1])
        } else {
            None
        };

        let cls = {
            let Some(map) = self.ast_map.lock().ok() else {
                return false;
            };
            let nmap = self.namespace_map.lock().ok();
            let mut found: Option<ClassInfo> = None;
            for (uri, classes) in map.iter() {
                if let Some(c) = classes.iter().find(|c| c.name == last_segment) {
                    if let Some(exp_ns) = expected_ns {
                        let file_ns = nmap
                            .as_ref()
                            .and_then(|nm| nm.get(uri))
                            .and_then(|opt| opt.as_deref());
                        if file_ns != Some(exp_ns) {
                            continue;
                        }
                    }
                    found = Some(c.clone());
                    break;
                }
            }
            found
        };

        match cls {
            Some(ci) => match &ci.parent_class {
                Some(parent) => self.is_throwable_descendant(parent, depth + 1),
                None => false, // no parent, not a Throwable type
            },
            None => false, // class not loaded — can't confirm
        }
    }

    /// Check whether the class identified by `class_name` is instantiable
    /// or simply not loaded yet.
    ///
    /// Returns `true` when the class is **not** found in the `ast_map`
    /// (unloaded — we cannot tell, so we allow it) **or** when it is
    /// found and is a concrete, non-abstract `class`.
    ///
    /// Returns `false` only when the class **is** loaded and is an
    /// interface, trait, enum, or abstract class.  This never triggers
    /// disk I/O.
    fn is_instantiable_or_unloaded(&self, class_name: &str) -> bool {
        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let last_segment = short_name(normalized);
        let expected_ns: Option<&str> = if normalized.contains('\\') {
            Some(&normalized[..normalized.len() - last_segment.len() - 1])
        } else {
            None
        };

        let Some(map) = self.ast_map.lock().ok() else {
            return true;
        };
        let nmap = self.namespace_map.lock().ok();
        for (uri, classes) in map.iter() {
            if let Some(c) = classes.iter().find(|c| c.name == last_segment) {
                if let Some(exp_ns) = expected_ns {
                    let file_ns = nmap
                        .as_ref()
                        .and_then(|nm| nm.get(uri))
                        .and_then(|opt| opt.as_deref());
                    if file_ns != Some(exp_ns) {
                        continue;
                    }
                }
                return c.kind == ClassLikeKind::Class && !c.is_abstract;
            }
        }
        // Not found in ast_map — unloaded, so allow it.
        true
    }

    /// Check whether the class identified by `class_name` is a concrete,
    /// non-abstract `class` (i.e. `ClassLikeKind::Class` and not
    /// `is_abstract`) in the `ast_map`.
    ///
    /// Returns `false` for interfaces, traits, enums, abstract classes,
    /// and classes that are not currently loaded.  This never triggers
    /// disk I/O.
    fn is_concrete_class_in_ast_map(&self, class_name: &str) -> bool {
        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let last_segment = short_name(normalized);
        let expected_ns: Option<&str> = if normalized.contains('\\') {
            Some(&normalized[..normalized.len() - last_segment.len() - 1])
        } else {
            None
        };

        let Some(map) = self.ast_map.lock().ok() else {
            return false;
        };
        let nmap = self.namespace_map.lock().ok();
        for (uri, classes) in map.iter() {
            if let Some(c) = classes.iter().find(|c| c.name == last_segment) {
                if let Some(exp_ns) = expected_ns {
                    let file_ns = nmap
                        .as_ref()
                        .and_then(|nm| nm.get(uri))
                        .and_then(|opt| opt.as_deref());
                    if file_ns != Some(exp_ns) {
                        continue;
                    }
                }
                return c.kind == ClassLikeKind::Class && !c.is_abstract;
            }
        }
        false
    }

    /// Collect the FQN of every class that is currently loaded in the
    /// `ast_map`.  Used by `build_catch_class_name_completions` so that
    /// classmap / stub sources can skip classes we already evaluated.
    fn collect_loaded_fqns(&self) -> HashSet<String> {
        let mut loaded = HashSet::new();
        let Ok(amap) = self.ast_map.lock() else {
            return loaded;
        };
        let nmap = self.namespace_map.lock().ok();
        for (uri, classes) in amap.iter() {
            let file_ns = nmap
                .as_ref()
                .and_then(|nm| nm.get(uri))
                .and_then(|opt| opt.as_deref());
            for cls in classes {
                let fqn = if let Some(ns) = file_ns {
                    format!("{}\\{}", ns, cls.name)
                } else {
                    cls.name.clone()
                };
                loaded.insert(fqn);
            }
        }
        loaded
    }

    /// Build completion items for class names, filtered for Throwable
    /// descendants.  Used as the catch clause fallback when no specific
    /// `@throws` types were discovered in the try block, and for
    /// `throw new` completion.
    ///
    /// The logic follows this priority:
    ///
    /// 1. **Loaded concrete classes** (use-imports, same-namespace,
    ///    class_index): only classes (not interfaces/traits/enums) whose
    ///    parent chain is fully walkable to `\Throwable` / `\Exception`
    ///    / `\Error`.
    /// 2. **Classmap** entries (not yet parsed) whose short name ends
    ///    with `Exception` — filtered to exclude already-loaded FQNs.
    /// 3. **Stub** entries whose short name ends with `Exception` —
    ///    filtered to exclude already-loaded FQNs.
    /// 4. **Classmap** entries that do *not* end with `Exception`.
    /// 5. **Stub** entries that do *not* end with `Exception`.
    pub(crate) fn build_catch_class_name_completions(
        &self,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        prefix: &str,
        content: &str,
        is_new: bool,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen_fqns: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        let use_insert_pos = find_use_insert_position(content);

        // Build the set of every FQN currently in the ast_map so that
        // classmap / stub sources can exclude already-evaluated classes.
        let loaded_fqns = self.collect_loaded_fqns();

        // ── 1a. Use-imported classes (must be concrete + Throwable) ─
        for (short_name, fqn) in file_use_map {
            if !short_name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen_fqns.insert(fqn.clone()) {
                continue;
            }
            // Only concrete classes (not interfaces/traits/enums)
            if !self.is_concrete_class_in_ast_map(fqn) {
                continue;
            }
            // Strict check: only include if confirmed Throwable descendant
            if !self.is_throwable_descendant(fqn, 0) {
                continue;
            }
            let (insert_text, insert_text_format) = if is_new {
                Self::build_new_insert(short_name, None)
            } else {
                (short_name.clone(), None)
            };
            items.push(CompletionItem {
                label: short_name.clone(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(fqn.clone()),
                insert_text: Some(insert_text),
                insert_text_format,
                filter_text: Some(short_name.clone()),
                sort_text: Some(format!("0_{}", short_name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        // ── 1b. Same-namespace classes (must be concrete + Throwable)
        // Collect candidates while holding the lock, then drop the lock
        // before calling `is_throwable_descendant` (which re-locks
        // `ast_map` internally — Rust's Mutex is not re-entrant).
        if let Some(ns) = file_namespace
            && let Ok(nmap) = self.namespace_map.lock()
        {
            let same_ns_uris: Vec<String> = nmap
                .iter()
                .filter_map(|(uri, opt_ns)| {
                    if opt_ns.as_deref() == Some(ns.as_str()) {
                        Some(uri.clone())
                    } else {
                        None
                    }
                })
                .collect();
            drop(nmap);

            // Phase 1: collect candidate (name, fqn, is_deprecated)
            // tuples under the ast_map lock — only concrete classes.
            let mut candidates: Vec<(String, String, bool)> = Vec::new();
            if let Ok(amap) = self.ast_map.lock() {
                for uri in &same_ns_uris {
                    if let Some(classes) = amap.get(uri) {
                        for cls in classes {
                            if cls.kind != ClassLikeKind::Class || cls.is_abstract {
                                continue;
                            }
                            if !cls.name.to_lowercase().contains(&prefix_lower) {
                                continue;
                            }
                            let fqn = format!("{}\\{}", ns, cls.name);
                            if !seen_fqns.insert(fqn.clone()) {
                                continue;
                            }
                            candidates.push((cls.name.clone(), fqn, cls.is_deprecated));
                        }
                    }
                }
            }
            // Phase 2: filter by Throwable ancestry without holding locks.
            for (name, fqn, is_deprecated) in candidates {
                if !self.is_throwable_descendant(&fqn, 0) {
                    continue;
                }
                let (insert_text, insert_text_format) = if is_new {
                    // Same-namespace classes are collected without
                    // ClassInfo, so we cannot check __construct here.
                    Self::build_new_insert(&name, None)
                } else {
                    (name.clone(), None)
                };
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn),
                    insert_text: Some(insert_text),
                    insert_text_format,
                    filter_text: Some(name.clone()),
                    sort_text: Some(format!("1_{}", name.to_lowercase())),
                    deprecated: if is_deprecated { Some(true) } else { None },
                    ..CompletionItem::default()
                });
            }
        }

        // ── 1c. class_index (must be concrete + Throwable) ──────────
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                let short_name = short_name(fqn);
                if !short_name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                if !self.is_concrete_class_in_ast_map(fqn) {
                    continue;
                }
                if !self.is_throwable_descendant(fqn, 0) {
                    continue;
                }
                let (insert_text, insert_text_format) = if is_new {
                    (format!("{short_name}()$0"), Some(InsertTextFormat::SNIPPET))
                } else {
                    (short_name.to_string(), None)
                };
                items.push(CompletionItem {
                    label: short_name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text),
                    insert_text_format,
                    filter_text: Some(short_name.to_string()),
                    sort_text: Some(format!("2_{}", short_name.to_lowercase())),
                    additional_text_edits: build_use_edit(fqn, use_insert_pos, file_namespace),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Classmap — names ending with "Exception" ─────────────
        // ── 4. Classmap — names NOT ending with "Exception" ─────────
        // We collect both buckets in a single pass over the classmap and
        // assign different sort_text prefixes so "Exception" entries
        // appear first.
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                if loaded_fqns.contains(fqn) {
                    continue;
                }
                let short_name = short_name(fqn);
                if !short_name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                let prefix_num = if short_name.ends_with("Exception") {
                    "3"
                } else {
                    "5"
                };
                let (insert_text, insert_text_format) = if is_new {
                    (format!("{short_name}()$0"), Some(InsertTextFormat::SNIPPET))
                } else {
                    (short_name.to_string(), None)
                };
                items.push(CompletionItem {
                    label: short_name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text),
                    insert_text_format,
                    filter_text: Some(short_name.to_string()),
                    sort_text: Some(format!("{}_{}", prefix_num, short_name.to_lowercase())),
                    additional_text_edits: build_use_edit(fqn, use_insert_pos, file_namespace),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 3. Stubs — names ending with "Exception" ────────────────
        // ── 5. Stubs — names NOT ending with "Exception" ────────────
        for &name in self.stub_index.keys() {
            if loaded_fqns.contains(name) {
                continue;
            }
            let short_name = short_name(name);
            if !short_name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen_fqns.insert(name.to_string()) {
                continue;
            }
            let prefix_num = if short_name.ends_with("Exception") {
                "4"
            } else {
                "6"
            };
            let (insert_text, insert_text_format) = if is_new {
                (format!("{short_name}()$0"), Some(InsertTextFormat::SNIPPET))
            } else {
                (short_name.to_string(), None)
            };
            items.push(CompletionItem {
                label: short_name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(name.to_string()),
                insert_text: Some(insert_text),
                insert_text_format,
                filter_text: Some(short_name.to_string()),
                sort_text: Some(format!("{}_{}", prefix_num, short_name.to_lowercase())),
                additional_text_edits: build_use_edit(name, use_insert_pos, file_namespace),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_CLASS_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CLASS_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Constant name completion ───────────────────────────────────

    /// Build completion items for standalone constants (`define()` constants)
    /// from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Constants discovered from parsed files (`global_defines`)
    ///   2. Built-in PHP constants from embedded stubs (`stub_constant_index`)
    ///
    /// Each item uses the constant name as `label` and the source as `detail`.
    /// Items are deduplicated by name.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching constants exceeds [`MAX_CONSTANT_COMPLETIONS`], the result
    /// is truncated and `is_incomplete` is `true`.
    const MAX_CONSTANT_COMPLETIONS: usize = 100;

    pub(crate) fn build_constant_completions(&self, prefix: &str) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined constants (from parsed files) ───────────
        if let Ok(dmap) = self.global_defines.lock() {
            for (name, _uri) in dmap.iter() {
                if !name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen.insert(name.clone()) {
                    continue;
                }
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some("define constant".to_string()),
                    insert_text: Some(name.clone()),
                    filter_text: Some(name.clone()),
                    sort_text: Some(format!("5_{}", name.to_lowercase())),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Built-in PHP constants from stubs ────────────────────
        for &name in self.stub_constant_index.keys() {
            if !name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some("PHP constant".to_string()),
                insert_text: Some(name.to_string()),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("6_{}", name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_CONSTANT_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CONSTANT_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Function name completion ───────────────────────────────────

    /// Build a label showing the full function signature.
    ///
    /// Example: `array_map(callable|null $callback, array $array, array ...$arrays): array`
    pub(crate) fn build_function_label(func: &FunctionInfo) -> String {
        let params: Vec<String> = func
            .parameters
            .iter()
            .map(|p| {
                let mut parts = Vec::new();
                if let Some(ref th) = p.type_hint {
                    parts.push(th.clone());
                }
                if p.is_reference {
                    parts.push(format!("&{}", p.name));
                } else if p.is_variadic {
                    parts.push(format!("...{}", p.name));
                } else {
                    parts.push(p.name.clone());
                }
                let param_str = parts.join(" ");
                if !p.is_required && !p.is_variadic {
                    format!("{} = ...", param_str)
                } else {
                    param_str
                }
            })
            .collect();

        let ret = func
            .return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        format!("{}({}){}", func.name, params.join(", "), ret)
    }

    /// Build completion items for standalone functions from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Functions discovered from parsed files (`global_functions`)
    ///   2. Built-in PHP functions from embedded stubs (`stub_function_index`)
    ///
    /// For user-defined functions (source 1), the full signature is shown in
    /// the label because we already have a parsed `FunctionInfo`.  For stub
    /// functions (source 2), only the function name is shown to avoid the
    /// cost of parsing every matching stub at completion time.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching functions exceeds [`MAX_FUNCTION_COMPLETIONS`], the result
    /// is truncated and `is_incomplete` is `true`.
    const MAX_FUNCTION_COMPLETIONS: usize = 100;

    pub(crate) fn build_function_completions(&self, prefix: &str) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined functions (from parsed files) ───────────
        if let Ok(fmap) = self.global_functions.lock() {
            for (name, (_uri, info)) in fmap.iter() {
                if !name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                // Use the short name for deduplication — if a user-defined
                // function shadows a built-in, the user version wins.
                if !seen.insert(info.name.clone()) {
                    continue;
                }
                let label = Self::build_function_label(info);
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("function".to_string()),
                    insert_text: Some(build_callable_snippet(&info.name, &info.parameters)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    filter_text: Some(info.name.clone()),
                    sort_text: Some(format!("4_{}", info.name.to_lowercase())),
                    deprecated: if info.is_deprecated { Some(true) } else { None },
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Built-in PHP functions from stubs ────────────────────
        for &name in self.stub_function_index.keys() {
            if !name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("PHP function".to_string()),
                insert_text: Some(format!("{name}()$0")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("5_{}", name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_FUNCTION_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_FUNCTION_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}
