//! Find References (`textDocument/references`).
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
//! them on demand (via the fqn_uri_index, PSR-4, and workspace scan).
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
//!
//! The per-symbol-kind finders live in sibling submodules
//! ([`dispatch`], [`variables`], [`classes`], [`members`],
//! [`functions`]); this module retains the shared symbol-map snapshot
//! helpers, the workspace-indexing pipeline, and the free helpers those
//! finders share.

mod classes;
mod dispatch;
mod functions;
mod members;
mod variables;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tower_lsp::lsp_types::{Location, Position, Range, Url};

use crate::Backend;
use crate::reference_index::ReferenceIndexKey;
use crate::symbol_map::SymbolMap;
use crate::util::strip_fqn_prefix;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceSearchMode {
    References,
    Rename,
}

impl ReferenceSearchMode {
    fn include_declaring_interfaces(self) -> bool {
        matches!(self, ReferenceSearchMode::References)
    }
}

impl Backend {
    /// Snapshot all symbol maps for user (non-vendor, non-stub) files.
    ///
    /// Ensures the workspace is indexed first, then returns a cloned
    /// snapshot of every symbol map whose URI does not fall under the
    /// vendor directory or the internal stub scheme.  All four cross-file
    /// reference scanners use this to restrict results to user code.
    pub(crate) fn user_file_symbol_maps(&self) -> Vec<(String, Arc<SymbolMap>)> {
        self.ensure_workspace_indexed_for_request();
        self.user_file_symbol_maps_matching(None)
    }

    pub(crate) fn user_file_symbol_maps_for_reference_keys(
        &self,
        keys: &[ReferenceIndexKey],
    ) -> Vec<(String, Arc<SymbolMap>)> {
        self.ensure_workspace_indexed_for_request();
        let candidate_uris = self.reference_candidate_uris_for_keys(keys);
        self.user_file_symbol_maps_matching(candidate_uris.as_ref())
    }

    fn user_file_symbol_maps_matching(
        &self,
        candidate_uris: Option<&HashSet<Arc<str>>>,
    ) -> Vec<(String, Arc<SymbolMap>)> {
        let vendor_prefixes = self.workspace.vendor_uri_prefixes.lock().clone();

        let maps = self.symbol_maps.read();
        maps.iter()
            .filter(|(uri, _)| {
                candidate_uris.is_none_or(|uris| uris.contains(uri.as_str()))
                    && !uri.starts_with("phpantom-stub://")
                    && !uri.starts_with("phpantom-stub-fn://")
                    && !vendor_prefixes.iter().any(|p| uri.starts_with(p.as_str()))
            })
            .map(|(uri, map)| (uri.clone(), Arc::clone(map)))
            .collect()
    }

    pub(super) fn reference_file_content(&self, uri: &str) -> Option<String> {
        if self.is_blade_file(uri)
            && let Some(content) = self.blade_virtual_content.read().get(uri)
        {
            return Some(content.clone());
        }
        self.get_file_content(uri)
    }

    pub(super) fn reference_file_content_arc(&self, uri: &str) -> Option<Arc<String>> {
        if self.is_blade_file(uri)
            && let Some(content) = self.blade_virtual_content.read().get(uri)
        {
            return Some(Arc::new(content.clone()));
        }
        self.get_file_content_arc(uri)
    }

    /// Enter the per-file scan window (80..100) of the current
    /// request's progress bar and register `total` files to scan.
    /// No-op when no progress sink is attached.
    pub(crate) fn begin_request_scan_window(&self, total: usize, label: &str) {
        if let Some(state) = self.request_progress.as_deref() {
            state.set_scope(80, 100, label);
            state.add_total(total as u64);
        }
    }

    /// Record one scanned file in the current request's progress bar.
    pub(crate) fn request_scan_file_done(&self) {
        if let Some(state) = self.request_progress.as_deref() {
            state.add_done(1);
        }
    }
}

/// Normalise a class FQN: strip leading `\` if present.
pub(super) fn normalize_fqn(fqn: &str) -> String {
    strip_fqn_prefix(fqn).to_string()
}

pub(super) fn static_call_root(
    expr: &crate::type_engine::subject_expr::SubjectExpr,
) -> Option<(&str, &str)> {
    match expr {
        crate::type_engine::subject_expr::SubjectExpr::CallExpr { callee, .. } => {
            static_call_root(callee)
        }
        crate::type_engine::subject_expr::SubjectExpr::MethodCall { base, .. } => {
            static_call_root(base)
        }
        crate::type_engine::subject_expr::SubjectExpr::StaticMethodCall { class, method } => {
            Some((class.as_str(), method.as_str()))
        }
        _ => None,
    }
}

pub(super) fn unresolved_member_subject_matches_scope(
    subject_text: &str,
    scope: &HashSet<String>,
) -> bool {
    let Some(subject_name) = unresolved_member_subject_name(subject_text) else {
        return false;
    };
    let subject_key = normalized_member_subject_key(&subject_name);
    if subject_key.is_empty() {
        return false;
    }

    scope.iter().any(|fqn| {
        member_scope_name_keys(crate::util::short_name(fqn))
            .into_iter()
            .any(|key| key == subject_key)
    })
}

fn unresolved_member_subject_name(subject_text: &str) -> Option<String> {
    match crate::type_engine::subject_expr::SubjectExpr::parse(subject_text) {
        crate::type_engine::subject_expr::SubjectExpr::Variable(name) => {
            Some(name.trim_start_matches('$').to_string())
        }
        crate::type_engine::subject_expr::SubjectExpr::PropertyChain { property, .. } => {
            Some(property)
        }
        _ => None,
    }
}

fn member_scope_name_keys(short_name: &str) -> Vec<String> {
    let mut names = vec![short_name.to_string()];
    for suffix in ["Repository", "Gateway"] {
        if let Some(stem) = short_name.strip_suffix(suffix) {
            names.push(format!("{stem}{suffix}"));
            if suffix == "Repository" {
                names.push(format!("{stem}Repo"));
            }
        }
    }

    names
        .into_iter()
        .map(|name| normalized_member_subject_key(&name))
        .filter(|name| !name.is_empty())
        .collect()
}

fn normalized_member_subject_key(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn is_laravel_builder_static_entrypoint(method_name: &str) -> bool {
    matches!(
        method_name.to_ascii_lowercase().as_str(),
        "query"
            | "newquery"
            | "where"
            | "wherein"
            | "wherenull"
            | "wherenotnull"
            | "orderby"
            | "select"
            | "with"
            | "without"
            | "latest"
            | "oldest"
    )
}

/// Whether a member name is the PHP constructor (`__construct`).
///
/// PHP method names are case-insensitive, so `__CONSTRUCT` matches too.
pub(super) fn is_constructor_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("__construct")
}

/// Check whether a resolved class name matches the target FQN.
///
/// Two names match if their fully-qualified forms are equal, or if both
/// are unqualified and their short names match.
pub(super) fn class_names_match(resolved: &str, target: &str, target_short: &str) -> bool {
    if resolved == target {
        return true;
    }
    // When neither name is qualified, compare short names.
    if !resolved.contains('\\') && !target.contains('\\') {
        return resolved == target_short;
    }
    // When the resolved name is unqualified but the target is
    // namespace-qualified, the resolved name might be a short-name
    // reference to the target class (e.g. `Request` referencing
    // `Illuminate\Http\Request` via a `use` import that was not
    // tracked in the resolved-names map).  Accept the match only
    // when the short names agree.
    //
    // The reverse (resolved is qualified, target is unqualified) is
    // NOT accepted: `App\Helper` is a different class from a global
    // `Helper`, so matching by short name alone would produce false
    // positives.
    if !resolved.contains('\\') && target.contains('\\') {
        return resolved == target_short;
    }
    false
}

pub(super) fn class_candidate_keys(target: &str, target_short: &str) -> Vec<ReferenceIndexKey> {
    symbol_candidate_names(target, target_short)
        .into_iter()
        .map(ReferenceIndexKey::Class)
        .collect()
}

pub(super) fn function_candidate_keys(target: &str, target_short: &str) -> Vec<ReferenceIndexKey> {
    symbol_candidate_names(target, target_short)
        .into_iter()
        .map(ReferenceIndexKey::Function)
        .collect()
}

fn symbol_candidate_names(target: &str, target_short: &str) -> Vec<String> {
    let mut keys = vec![
        strip_fqn_prefix(target).to_string(),
        strip_fqn_prefix(target_short).to_string(),
    ];
    keys.sort();
    keys.dedup();
    keys
}

pub(super) fn member_candidate_keys(
    target_member: &str,
    target_is_static: bool,
    hierarchy: Option<&HashSet<String>>,
) -> Vec<ReferenceIndexKey> {
    let mut keys = vec![ReferenceIndexKey::Member {
        name: target_member.to_string(),
        is_static: target_is_static,
    }];
    if hierarchy.is_some() {
        keys.push(ReferenceIndexKey::Member {
            name: target_member.to_string(),
            is_static: !target_is_static,
        });
    }
    keys
}

/// Recursively collect all `.php` files under a workspace root,
/// respecting `.gitignore` rules (including nested and global
/// gitignore files).
///
/// Used by Find References which walks the entire workspace root.
/// Unlike `classmap_scanner`'s PSR-4 walkers, this uses the `ignore`
/// crate's [`ignore::WalkBuilder`] so that generated/cached directories
/// listed in `.gitignore` (e.g. `storage/framework/views/`,
/// `var/cache/`, `node_modules/`) are automatically skipped.
///
/// All known vendor directories are always skipped regardless of
/// `.gitignore` content, since some projects commit their vendor
/// directory.  `vendor_dir_paths` contains absolute paths of all
/// known vendor directories (one per subproject in monorepo mode).
///
/// Hidden files and directories are skipped by default (handled by
/// the `ignore` crate).
pub(crate) fn collect_php_files_gitignore(
    root: &Path,
    vendor_dir_paths: &[PathBuf],
) -> Vec<PathBuf> {
    use ignore::WalkBuilder;

    let mut result = Vec::new();
    let vendor_paths_owned: Vec<PathBuf> = vendor_dir_paths.to_vec();

    let walker = WalkBuilder::new(root)
        // Respect .gitignore, .git/info/exclude, global gitignore
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // Skip hidden files/dirs (.git, .idea, etc.)
        .hidden(true)
        // Read parent .gitignore files
        .parents(true)
        // Also respect .ignore files (ripgrep convention)
        .ignore(true)
        // Always skip vendor directories, even if not gitignored
        .filter_entry(move |entry| {
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let path = entry.path();
                if vendor_paths_owned.iter().any(|vp| vp == path) {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "php") {
            result.push(path.to_path_buf());
        }
    }

    result
}

/// Push a location only if it is not already present (deduplication).
pub(crate) fn push_unique_location(
    locations: &mut Vec<Location>,
    uri: &Url,
    start: Position,
    end: Position,
) {
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
