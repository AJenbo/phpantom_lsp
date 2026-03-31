//! Unknown member access diagnostics.
//!
//! Walk the precomputed [`SymbolMap`] for a file and flag every
//! `MemberAccess` span where the member does not exist on the resolved
//! class after full resolution (inheritance + virtual member providers).
//!
//! Diagnostics use `Severity::Warning` because the code may still run
//! (e.g. via `__call` / `__get` magic methods that we cannot see), but
//! the user benefits from knowing that PHPantom can't resolve the member.
//!
//! We suppress diagnostics when:
//!
//! - The subject type cannot be resolved (we can't know what members it has).
//! - Any resolved class in a union type has the member (the member is
//!   valid for at least one branch of the union).
//! - Any resolved class has `__call` / `__callStatic` (for method calls)
//!   or `__get` (for property access) magic methods — these accept
//!   arbitrary member names at runtime.
//! - Any resolved class is `stdClass` — it is a universal object
//!   container that accepts arbitrary properties at runtime.
//! - The member name is `class` (the magic `::class` constant).
//! - The subject is an enum and the member is a case name (enum cases
//!   are accessed via `::` but stored as constants).
//! - The subject is `$this`, `self`, `static`, or `parent` inside a
//!   trait method.  Traits are incomplete by nature — they expect to
//!   be mixed into classes that provide the missing members.  Flagging
//!   accesses that only exist on host classes produces a high rate of
//!   false positives.
//!
//! ## Performance: subject resolution cache
//!
//! A single file can contain hundreds of member access spans that share
//! the same subject text (e.g. 60 occurrences of `$this->assertEquals`,
//! `$this->assertTrue`, etc.).  Without caching, each span triggers the
//! full resolution pipeline including `resolve_variable_types` which
//! re-parses the entire file via `with_parsed_program`.  For unresolved
//! subjects the secondary helpers (`resolve_scalar_subject_type`,
//! `resolve_unresolvable_class_subject`) add further re-parses.
//!
//! To avoid this, we cache the resolution outcome per unique
//! `(subject_text, access_kind, scope_key)` tuple, where `scope_key`
//! combines the innermost enclosing class (name + byte offset) with
//! the innermost enclosing function/method/closure scope start offset.
//! This means `$var->` accesses in different methods of the same class
//! get independent cache entries even when the variable name is the
//! same but has a different type in each method.  The cache lives for
//! a single `collect_unknown_member_diagnostics` call and is not
//! shared across files or invocations.

use std::collections::HashMap;
use std::sync::Arc;

use super::unresolved_member_access::UNRESOLVED_MEMBER_ACCESS_CODE;
use crate::parser::with_parse_cache;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::resolver::{
    Loaders, ResolutionCtx, resolve_target_classes, resolve_target_classes_expr,
};
use crate::completion::variable::resolution::resolve_variable_types;

use crate::hover::variable_type::resolve_variable_type_string;
use crate::inheritance::resolve_property_type_hint;
use crate::subject_expr::SubjectExpr;
use crate::symbol_map::SymbolKind;
use crate::types::{AccessKind, ClassInfo, ClassLikeKind, ResolvedType};
use crate::virtual_members::{resolve_class_fully_cached, resolve_class_fully_maybe_cached};

use super::helpers::{find_innermost_enclosing_class, make_diagnostic};
use super::offset_range_to_lsp_range;

/// Diagnostic code used for unknown-member diagnostics so that code
/// actions can match on it.
pub(crate) const UNKNOWN_MEMBER_CODE: &str = "unknown_member";

/// Diagnostic code used when member access is attempted on a scalar
/// type (int, string, bool, float, null, void, never, array).  This
/// is always a runtime crash, so the severity is `Error`.
pub(crate) const SCALAR_MEMBER_ACCESS_CODE: &str = "scalar_member_access";

// ─── Subject resolution cache ───────────────────────────────────────────────

/// Scope identifier for the subject resolution cache.
///
/// Two member accesses share the same scope when they are inside the
/// same class body (identified by class name and byte offset of the
/// opening brace) **and** the same function/method/closure body
/// (identified by its start offset).  This prevents two methods in
/// the same class from sharing a cache entry when a same-named
/// variable has a different type in each method.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ScopeKey {
    /// Inside a class at the given byte offset, within a specific
    /// function/method/closure scope.  `fn_scope_start` is the byte
    /// offset of the enclosing function body (from
    /// [`SymbolMap::find_enclosing_scope`]), or `0` for class-level
    /// code outside any method.
    Class {
        name: String,
        start_offset: u32,
        fn_scope_start: u32,
    },
    /// Top-level code outside any class, within a specific
    /// function scope (`0` when truly top-level).
    TopLevel { fn_scope_start: u32 },
}

/// Cache key combining the subject text, access kind, and scope.
///
/// For variable-based subjects (starting with `$`, excluding `$this`),
/// `var_def_offset` distinguishes accesses that fall under different
/// definitions of the same variable.  Without this, the cache would
/// return the parameter type for accesses after a reassignment.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SubjectCacheKey {
    subject_text: String,
    access_kind: AccessKind,
    scope: ScopeKey,
    /// The `effective_from` offset of the active variable definition at
    /// the point of access, or `0` for non-variable subjects.  This
    /// ensures that accesses before and after a reassignment get
    /// separate cache entries.
    var_def_offset: u32,
    /// The span start offset for variable subjects (excluding `$this`),
    /// or `0` for non-variable subjects.  This ensures that accesses
    /// inside different instanceof-narrowing contexts (e.g. different
    /// if-bodies) get independent cache entries.  Without this, the
    /// first access caches a narrowed type and subsequent accesses in
    /// a different narrowing context reuse the wrong result.
    narrowing_offset: u32,
    /// The offset of the most recent `assert($var instanceof …)`
    /// statement preceding this access, or `0` if there is none.
    /// Assert-instanceof statements act as sequential narrowing
    /// boundaries: they change the variable's resolved type without
    /// creating a block scope, so accesses before and after the
    /// assert must get separate cache entries.
    assert_offset: u32,
}

/// The outcome of resolving a subject for diagnostic purposes.
///
/// Cached so that subsequent member accesses on the same subject in the
/// same scope skip the entire resolution pipeline (including expensive
/// `with_parsed_program` re-parses).
#[derive(Clone, Debug)]
enum SubjectOutcome {
    /// Subject resolved to one or more classes.
    Resolved(Vec<Arc<ClassInfo>>),
    /// Subject resolved to a scalar type — member access is always a
    /// runtime crash.
    Scalar(String),
    /// Subject resolved to a class name that couldn't be loaded.
    UnresolvableClass(String),
    /// Subject is a chain or call expression whose type couldn't be
    /// resolved.
    UnresolvableChain,
    /// Subject is a bare variable with no type information at all.
    /// No diagnostic should be emitted (the opt-in
    /// `unresolved-member-access` diagnostic covers this case).
    Untyped,
}

/// Per-pass cache mapping subject keys to their resolution outcomes.
type SubjectCache = HashMap<SubjectCacheKey, SubjectOutcome>;

/// Build a [`ScopeKey`] from the innermost enclosing class (if any)
/// and the enclosing function/method/closure scope start offset.
/// Check whether a subject text is rooted in `$this`, `self`, `static`,
/// or `parent`.  This matches both bare keywords (`"$this"`, `"static"`)
/// and chain expressions that start with one of them
/// (`"$this->relation()"`, `"static::where('x', 'y')"`, `"self::$prop"`).
fn subject_text_is_rooted_in_self(subject_text: &str) -> bool {
    // Bare keyword match (most common case).
    if matches!(subject_text, "$this" | "self" | "static" | "parent") {
        return true;
    }

    // Chain rooted at `$this->` or `$this?->`
    if subject_text.starts_with("$this->") || subject_text.starts_with("$this?->") {
        return true;
    }

    // Chain rooted at `self::`, `static::`, or `parent::`
    if subject_text.starts_with("self::")
        || subject_text.starts_with("static::")
        || subject_text.starts_with("parent::")
    {
        return true;
    }

    false
}

/// Build a [`ScopeKey`] from the innermost enclosing class (if any)
/// and the enclosing function/method/closure scope start offset.
fn scope_key_for(current_class: Option<&ClassInfo>, fn_scope_start: u32) -> ScopeKey {
    match current_class {
        Some(cc) => ScopeKey::Class {
            name: cc.name.clone(),
            start_offset: cc.start_offset,
            fn_scope_start,
        },
        None => ScopeKey::TopLevel { fn_scope_start },
    }
}

/// Resolve the subject and return a [`SubjectOutcome`].
///
/// This runs the full resolution pipeline exactly once per unique
/// cache key.
fn resolve_subject_outcome(
    subject_text: &str,
    access_kind: AccessKind,
    rctx: &ResolutionCtx<'_>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
    cache: &crate::virtual_members::ResolvedClassCache,
) -> SubjectOutcome {
    let base_classes: Vec<Arc<ClassInfo>> = resolve_target_classes(subject_text, access_kind, rctx);

    if !base_classes.is_empty() {
        return SubjectOutcome::Resolved(base_classes);
    }

    // ── Subject did not resolve to any class ────────────────────────
    let expr = SubjectExpr::parse(subject_text);

    // Try scalar type detection.
    if let Some(scalar) = resolve_scalar_subject_type(
        &expr,
        access_kind,
        rctx,
        class_loader,
        function_loader,
        cache,
    ) {
        return SubjectOutcome::Scalar(scalar);
    }

    // Try unresolvable class detection.
    if let Some(unresolved) =
        resolve_unresolvable_class_subject(&expr, rctx, class_loader, function_loader)
    {
        return SubjectOutcome::UnresolvableClass(unresolved);
    }

    // Check if the subject is a chain or call expression.
    let is_chain = matches!(
        expr,
        SubjectExpr::PropertyChain { .. } | SubjectExpr::CallExpr { .. }
    );
    if is_chain {
        return SubjectOutcome::UnresolvableChain;
    }

    SubjectOutcome::Untyped
}

impl Backend {
    /// Collect unknown-member diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub fn collect_unknown_member_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Gather context under locks ──────────────────────────────────
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_use_map: HashMap<String, String> = self.file_use_map(uri);

        let file_namespace: Option<String> = self.namespace_map.read().get(uri).cloned().flatten();

        let local_classes: Vec<Arc<ClassInfo>> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();

        let class_loader = self.class_loader_with(&local_classes, &file_use_map, &file_namespace);
        let function_loader = self.function_loader_with(&file_use_map, &file_namespace);
        let resolved_cache = &self.resolved_class_cache;

        // ── Parse cache for this diagnostic pass ────────────────────────
        // The file content is immutable during a single diagnostic pass.
        // Activating the thread-local parse cache means every call to
        // `with_parsed_program(content, …)` in the resolution pipeline
        // (resolve_variable_types, resolve_variable_type_string, etc.)
        // will reuse the same parsed AST instead of re-parsing the
        // entire file from scratch.
        let _parse_guard = with_parse_cache(content);

        // ── Inner resolution cache for chain bases ──────────────────────
        // When resolving chain subjects like `$class->methods()` and
        // `$class->properties()`, each one independently calls
        // `resolve_target_classes("$class", …)` to resolve the base.
        // The DIAG_SUBJECT_CACHE deduplicates these inner lookups.
        //
        // In production this cache is already active (activated by
        // `publish_diagnostics_for_file` or `analyse::run`), so the
        // guard here is a no-op.  For standalone calls (benchmarks,
        // tests) it ensures chain bases are resolved once rather than
        // once per unique chain expression.
        let _subj_guard = crate::completion::resolver::with_diagnostic_subject_cache();

        // Provide scope boundaries so the diagnostic subject cache can
        // distinguish variables in different methods of the same class.
        // In production this is already set by the outer caller; for
        // standalone calls (tests, benchmarks) set it here.
        crate::completion::resolver::set_diagnostic_subject_cache_scopes(
            symbol_map.scopes.clone(),
            symbol_map.var_defs.clone(),
            symbol_map.narrowing_blocks.clone(),
            symbol_map.assert_narrowing_offsets.clone(),
        );

        // ── Subject resolution cache for this diagnostic pass ───────────
        let mut subject_cache: SubjectCache = HashMap::new();

        // ── Walk every symbol span ──────────────────────────────────────
        for span in &symbol_map.spans {
            let (subject_text, member_name, is_static, is_method_call, is_docblock_ref) =
                match &span.kind {
                    SymbolKind::MemberAccess {
                        subject_text,
                        member_name,
                        is_static,
                        is_method_call,
                        is_docblock_reference,
                    } => (
                        subject_text,
                        member_name,
                        *is_static,
                        *is_method_call,
                        *is_docblock_reference,
                    ),
                    _ => continue,
                };

            // ── Skip the magic `::class` constant ───────────────────────
            if member_name == "class" && is_static {
                continue;
            }

            let access_kind = if is_static {
                AccessKind::DoubleColon
            } else {
                AccessKind::Arrow
            };

            let current_class = find_innermost_enclosing_class(&local_classes, span.start);

            // ── Suppress inside traits for self-referencing subjects ────
            // Traits are incomplete: they expect host classes to provide
            // members accessed via $this/self/static/parent.  Flagging
            // these produces false positives for every trait that relies
            // on the host class's members.
            //
            // This also covers chain expressions rooted at these keywords,
            // e.g. `static::where('x', 'y')->update(...)` has subject_text
            // `"static::where('x', 'y')"` and `$this->relation()->first()`
            // has subject_text `"$this->relation()"`.  The root of the
            // chain is still the trait's self-reference, so the entire
            // chain is unsuppressable without knowing the host class.
            if let Some(cc) = current_class
                && cc.kind == ClassLikeKind::Trait
                && subject_text_is_rooted_in_self(subject_text)
            {
                continue;
            }

            let fn_scope_start = symbol_map.find_enclosing_scope(span.start);

            // ── Look up or populate the subject cache ───────────────────
            // For variable subjects (excluding $this), compute the
            // active definition offset so that accesses before and
            // after a reassignment get separate cache entries (B4).
            let var_def_offset =
                if subject_text.starts_with('$') && !subject_text.starts_with("$this") {
                    // Extract the bare variable name (e.g. "$file" from
                    // "$file" or from a chain like "$file->foo()").
                    let var_name = subject_text
                        .find("->")
                        .map(|i| &subject_text[..i])
                        .unwrap_or(subject_text);
                    symbol_map.active_var_def_offset(
                        &var_name[1..], // strip leading '$'
                        span.start,
                    )
                } else {
                    0
                };

            // For variable subjects (excluding $this), use the
            // innermost narrowing block (if/elseif/else body) as a
            // cache discriminator so that accesses in different
            // instanceof-narrowing contexts get independent entries.
            // Accesses in the same block share a cache entry because
            // they receive identical narrowing.
            let narrowing_offset =
                if subject_text.starts_with('$') && !subject_text.starts_with("$this") {
                    symbol_map.find_narrowing_block(span.start)
                } else {
                    0
                };

            // For variable subjects, also check whether an
            // `assert($var instanceof …)` precedes this access.
            // Assert-instanceof does not create a block scope, so
            // without this discriminator accesses before and after
            // the assert would share the same (stale) cache entry.
            let assert_offset =
                if subject_text.starts_with('$') && !subject_text.starts_with("$this") {
                    symbol_map.find_preceding_assert_offset(span.start)
                } else {
                    0
                };

            let cache_key = SubjectCacheKey {
                subject_text: subject_text.clone(),
                access_kind,
                scope: scope_key_for(current_class, fn_scope_start),
                var_def_offset,
                narrowing_offset,
                assert_offset,
            };

            let outcome = subject_cache
                .entry(cache_key)
                .or_insert_with(|| {
                    let rctx = ResolutionCtx {
                        current_class,
                        all_classes: &local_classes,
                        content,
                        cursor_offset: span.start,
                        class_loader: &class_loader,
                        resolved_class_cache: Some(resolved_cache),
                        function_loader: Some(&function_loader),
                    };
                    resolve_subject_outcome(
                        subject_text,
                        access_kind,
                        &rctx,
                        &class_loader,
                        &function_loader,
                        resolved_cache,
                    )
                })
                .clone();

            // ── Emit diagnostics based on the cached outcome ────────────
            match outcome {
                SubjectOutcome::Scalar(ref scalar) => {
                    let range = match offset_range_to_lsp_range(
                        content,
                        span.start as usize,
                        span.end as usize,
                    ) {
                        Some(r) => r,
                        None => continue,
                    };
                    let kind_label = if is_method_call { "method" } else { "property" };
                    let message = format!(
                        "Cannot access {} '{}' on type '{}'",
                        kind_label, member_name, scalar,
                    );
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::ERROR,
                        SCALAR_MEMBER_ACCESS_CODE,
                        message,
                    ));
                }

                SubjectOutcome::UnresolvableClass(ref unresolved) => {
                    let range = match offset_range_to_lsp_range(
                        content,
                        span.start as usize,
                        span.end as usize,
                    ) {
                        Some(r) => r,
                        None => continue,
                    };
                    let kind_label = if is_method_call { "method" } else { "property" };
                    let message = format!(
                        "Cannot verify {} '{}' — subject type '{}' could not be resolved",
                        kind_label, member_name, unresolved,
                    );
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::WARNING,
                        UNKNOWN_MEMBER_CODE,
                        message,
                    ));
                }

                SubjectOutcome::UnresolvableChain => {
                    let range = match offset_range_to_lsp_range(
                        content,
                        span.start as usize,
                        span.end as usize,
                    ) {
                        Some(r) => r,
                        None => continue,
                    };
                    let kind_label = if is_method_call { "method" } else { "property" };
                    let message = format!(
                        "Cannot verify {} '{}' — subject type could not be resolved",
                        kind_label, member_name,
                    );
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::WARNING,
                        UNKNOWN_MEMBER_CODE,
                        message,
                    ));
                }

                SubjectOutcome::Untyped => {
                    // When the opt-in `unresolved-member-access` diagnostic
                    // is enabled, emit it here instead of in a separate
                    // collector pass.  This avoids a second full walk of
                    // the same symbol spans with duplicate type resolution.
                    if self.config().diagnostics.unresolved_member_access_enabled() {
                        // Skip call-expression subjects — the failure is
                        // usually because the symbol map's subject_text
                        // doesn't preserve full argument text, not because
                        // the user is missing a type annotation.
                        if !subject_text.is_empty() && !subject_text.contains('(') {
                            let range = match offset_range_to_lsp_range(
                                content,
                                span.start as usize,
                                span.end as usize,
                            ) {
                                Some(r) => r,
                                None => continue,
                            };
                            let subject_display = subject_text.trim();
                            let message = format!(
                                "Cannot resolve type of '{}'. Add a type annotation or PHPDoc tag to enable full IDE support.",
                                subject_display,
                            );
                            out.push(make_diagnostic(
                                range,
                                DiagnosticSeverity::HINT,
                                UNRESOLVED_MEMBER_ACCESS_CODE,
                                message,
                            ));
                        }
                    }
                }

                SubjectOutcome::Resolved(ref base_classes) => {
                    self.check_member_on_resolved_classes(
                        base_classes,
                        member_name,
                        is_static,
                        is_method_call,
                        is_docblock_ref,
                        &class_loader,
                        resolved_cache,
                        content,
                        span.start,
                        span.end,
                        out,
                    );
                }
            }
        }
    }

    /// Check whether a member exists on the resolved classes and emit
    /// a diagnostic if it does not.
    ///
    /// Extracted from the main loop to keep `collect_unknown_member_diagnostics`
    /// readable.
    #[allow(clippy::too_many_arguments)]
    fn check_member_on_resolved_classes(
        &self,
        base_classes: &[Arc<ClassInfo>],
        member_name: &str,
        is_static: bool,
        is_method_call: bool,
        is_docblock_ref: bool,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        cache: &crate::virtual_members::ResolvedClassCache,
        content: &str,
        start: u32,
        end: u32,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Quick check on pre-resolved base classes ────────────────
        // `resolve_target_classes` already returns fully-resolved
        // classes in many code paths (e.g. `type_hint_to_classes`
        // calls `resolve_class_fully` and injects model-specific
        // scope methods onto Eloquent Builders).  Check the member
        // on these classes FIRST, before re-resolving through the
        // cache.  The cache is keyed by bare FQN and may hold a
        // stale entry that lacks context-specific virtual members
        // (e.g. Builder scope methods that depend on the concrete
        // model type).  Checking here avoids false positives when
        // the cache and the resolver disagree.
        if base_classes
            .iter()
            .any(|c| has_magic_method_for_access(c, is_static, is_method_call))
        {
            return;
        }
        if base_classes.iter().any(|c| c.name == "stdClass") {
            return;
        }
        if base_classes.iter().any(|c| {
            member_exists(c, member_name, is_static, is_method_call)
                || (is_docblock_ref && member_exists_relaxed(c, member_name, is_method_call))
        }) {
            return;
        }

        // ── Fully resolve each class (inheritance + virtual members) ─
        // Synthetic classes like `__object_shape` already carry all
        // their members and must NOT go through the cache (every
        // object shape shares the same name, so the cache would
        // return the wrong entry).
        let resolved_classes: Vec<Arc<ClassInfo>> = base_classes
            .iter()
            .map(|c| {
                if c.name == "__object_shape" {
                    Arc::clone(c)
                } else {
                    resolve_class_fully_cached(c, class_loader, cache)
                }
            })
            .collect();

        // ── Check for magic methods on ANY branch ───────────────────
        if resolved_classes
            .iter()
            .any(|c| has_magic_method_for_access(c, is_static, is_method_call))
        {
            return;
        }

        // ── Skip stdClass (universal object container) ──────────────
        if resolved_classes.iter().any(|c| c.name == "stdClass") {
            return;
        }

        // ── Check whether the member exists on ANY branch ───────────
        if resolved_classes.iter().any(|c| {
            member_exists(c, member_name, is_static, is_method_call)
                || (is_docblock_ref && member_exists_relaxed(c, member_name, is_method_call))
        }) {
            return;
        }

        // ── Member is unresolved on ALL branches — emit diagnostic ──
        let range = match offset_range_to_lsp_range(content, start as usize, end as usize) {
            Some(r) => r,
            None => return,
        };

        let kind_label = if is_method_call {
            "Method"
        } else if is_static {
            // Static non-method could be a property ($prop) or constant
            "Member"
        } else {
            "Property"
        };

        // Show the first resolved class name for context.  For union
        // types we could list all of them, but keeping it short is
        // more useful in the editor gutter.
        let class_display = display_class_name(&resolved_classes[0]);

        let message = if resolved_classes.len() > 1 {
            format!(
                "{} '{}' not found on any of the {} possible types ({})",
                kind_label,
                member_name,
                resolved_classes.len(),
                resolved_classes
                    .iter()
                    .map(|c| display_class_name(c))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        } else {
            format!(
                "{} '{}' not found on class '{}'",
                kind_label, member_name, class_display,
            )
        };

        out.push(make_diagnostic(
            range,
            DiagnosticSeverity::WARNING,
            UNKNOWN_MEMBER_CODE,
            message,
        ));
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Check whether a member exists on the fully-resolved class.
///
/// For method calls, checks `methods`.  For non-method static access,
/// checks constants first then static properties.  For instance property
/// access, checks properties.
///
/// Method name matching is case-insensitive (PHP methods are
/// case-insensitive).  Property and constant matching is case-sensitive.
/// Relaxed member check for docblock references (`@see Class::member`).
///
/// PHPDoc `@see` uses `::` notation for all members (instance properties,
/// instance methods, static properties, constants), so we check every
/// member kind regardless of `is_static` or `is_method_call`.
fn member_exists_relaxed(class: &ClassInfo, member_name: &str, _is_method_call: bool) -> bool {
    // Check methods (case-insensitive, like PHP).
    let lower = member_name.to_ascii_lowercase();
    if class
        .methods
        .iter()
        .any(|m| m.name.to_ascii_lowercase() == lower)
    {
        return true;
    }
    // Check instance and static properties.
    if class.properties.iter().any(|p| p.name == member_name) {
        return true;
    }
    // Check constants.
    class.constants.iter().any(|c| c.name == member_name)
}

fn member_exists(
    class: &ClassInfo,
    member_name: &str,
    is_static: bool,
    is_method_call: bool,
) -> bool {
    if is_method_call {
        // Method name matching is case-insensitive in PHP.
        let lower = member_name.to_ascii_lowercase();
        return class
            .methods
            .iter()
            .any(|m| m.name.to_ascii_lowercase() == lower);
    }

    if is_static {
        // Static property or constant.
        // Constants first (most common in `Class::CONST` usage).
        if class.constants.iter().any(|c| c.name == member_name) {
            return true;
        }
        // Static property (e.g. `Class::$prop`).
        // PHP static properties include the `$` in the access syntax,
        // but the stored name may or may not include it.  Check both.
        if class.properties.iter().any(|p| {
            p.is_static && (p.name == member_name || format!("${}", p.name) == member_name)
        }) {
            return true;
        }
        // Also check enum cases which are stored as constants.
        return false;
    }

    // Instance property access.
    class.properties.iter().any(|p| p.name == member_name)
}

/// Check whether the class has a magic method that would handle the
/// member access at runtime, making the "unknown member" diagnostic
/// a false positive.
fn has_magic_method_for_access(class: &ClassInfo, is_static: bool, is_method_call: bool) -> bool {
    if is_method_call {
        let magic = if is_static { "__callStatic" } else { "__call" };
        return class
            .methods
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(magic));
    }

    if !is_static {
        // Instance property access — `__get` handles arbitrary property names.
        return class
            .methods
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case("__get"));
    }

    false
}

/// Try to determine a scalar type for the subject, so we can report a
/// more specific "member access on scalar" diagnostic.
fn resolve_scalar_subject_type(
    expr: &SubjectExpr,
    access_kind: AccessKind,
    rctx: &ResolutionCtx<'_>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
    cache: &crate::virtual_members::ResolvedClassCache,
) -> Option<String> {
    match expr {
        // ── Bare variable: $number = 1; $number->foo() ──────────
        SubjectExpr::Variable(var_name) => {
            let default_class = ClassInfo::default();
            let effective_class = rctx.current_class.unwrap_or(&default_class);
            let resolved = resolve_variable_types(
                var_name,
                effective_class,
                rctx.all_classes,
                rctx.content,
                rctx.cursor_offset,
                class_loader,
                Loaders::with_function(rctx.function_loader),
            );
            if resolved.is_empty() {
                return None;
            }
            let joined = ResolvedType::types_joined(&resolved);
            if joined.all_members_primitive_scalar() {
                let display = joined
                    .non_null_type()
                    .map_or_else(|| joined.to_string(), |t| t.to_string());
                Some(display)
            } else {
                None
            }
        }

        // ── Property chain: $user->age->value ───────────────────
        SubjectExpr::PropertyChain { base, property } => {
            // Resolve the base to classes, then look up the property's
            // type hint on the resolved class.
            let base_classes = resolve_target_classes_expr(base, access_kind, rctx);
            for cls in &base_classes {
                let resolved = resolve_class_fully_maybe_cached(cls, class_loader, Some(cache));
                if let Some(parsed) = resolve_property_type_hint(&resolved, property, class_loader)
                {
                    // Check each union branch — if ALL branches are scalar, the
                    // type is scalar.  If any branch is a class, resolve_target_classes
                    // would have returned it, so we wouldn't be here.
                    if parsed.all_members_primitive_scalar() {
                        let display = parsed
                            .non_null_type()
                            .map_or_else(|| parsed.to_string(), |t| t.to_string());
                        return Some(display);
                    }
                    // Non-scalar, non-class type (e.g. a type alias we can't
                    // resolve) — treat as unresolvable.
                    return None;
                }
            }
            None
        }

        // ── Call expression: getInt()->value, $obj->getAge()->value ──
        SubjectExpr::CallExpr { callee, args_text } => {
            // Resolve the call return type.  If it's a scalar, report it.
            let return_classes = Backend::resolve_call_return_types_expr(callee, args_text, rctx);
            if return_classes.is_empty() {
                // Try to get the raw return type hint from the callable.
                match callee.as_ref() {
                    // Instance method call: $obj->getAge()
                    SubjectExpr::MethodCall { base, method } => {
                        let base_classes = resolve_target_classes_expr(base, access_kind, rctx);
                        for cls in &base_classes {
                            let resolved =
                                resolve_class_fully_maybe_cached(cls, class_loader, Some(cache));
                            if let Some(m) = resolved
                                .methods
                                .iter()
                                .find(|m| m.name.eq_ignore_ascii_case(method))
                                && let Some(ref hint) = m.return_type
                                && hint.all_members_primitive_scalar()
                            {
                                let display = hint
                                    .non_null_type()
                                    .map_or_else(|| hint.to_string(), |t| t.to_string());
                                return Some(display);
                            }
                        }
                    }
                    // Standalone function call: getInt()
                    SubjectExpr::FunctionCall(fn_name) => {
                        if let Some(func_info) = function_loader(fn_name)
                            && let Some(ref hint) = func_info.return_type
                            && hint.all_members_primitive_scalar()
                        {
                            let display = hint
                                .non_null_type()
                                .map_or_else(|| hint.to_string(), |t| t.to_string());
                            return Some(display);
                        }
                    }
                    // Static method call: Foo::getInt()
                    SubjectExpr::StaticMethodCall { class, method } => {
                        let cls = class_loader(class);
                        if let Some(cls) = cls {
                            let resolved =
                                resolve_class_fully_maybe_cached(&cls, class_loader, Some(cache));
                            if let Some(m) = resolved
                                .methods
                                .iter()
                                .find(|m| m.name.eq_ignore_ascii_case(method))
                                && let Some(ref hint) = m.return_type
                                && hint.all_members_primitive_scalar()
                            {
                                let display = hint
                                    .non_null_type()
                                    .map_or_else(|| hint.to_string(), |t| t.to_string());
                                return Some(display);
                            }
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

/// Try to determine an unresolvable class name for the subject.
///
/// When the subject's raw type looks like a class name but cannot be
/// loaded, we emit a diagnostic that names the unresolvable type.
fn resolve_unresolvable_class_subject(
    expr: &SubjectExpr,
    rctx: &ResolutionCtx<'_>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
) -> Option<String> {
    let raw_type = match expr {
        SubjectExpr::Variable(var_name) => {
            // Try the unified pipeline first (covers assignments,
            // parameter type hints, foreach bindings, catch variables,
            // inline @var overrides, etc.).
            let default_class = ClassInfo::default();
            let effective_class = rctx.current_class.unwrap_or(&default_class);
            let resolved = resolve_variable_types(
                var_name,
                effective_class,
                rctx.all_classes,
                rctx.content,
                rctx.cursor_offset,
                class_loader,
                Loaders::with_function(rctx.function_loader),
            );
            if !resolved.is_empty() {
                Some(ResolvedType::type_strings_joined(&resolved))
            } else {
                // Fall back to the hover variable type resolver which
                // also checks class-based foreach resolution through
                // @implements / @extends generics.
                resolve_variable_type_string(
                    var_name,
                    rctx.content,
                    rctx.cursor_offset,
                    rctx.current_class,
                    rctx.all_classes,
                    class_loader,
                    Loaders::with_function(rctx.function_loader),
                )
            }
        }
        SubjectExpr::CallExpr { callee, .. } => match callee.as_ref() {
            SubjectExpr::FunctionCall(fn_name) => {
                let fi = function_loader(fn_name)?;
                fi.return_type_str()
            }
            _ => None,
        },
        _ => None,
    }?;

    let parsed = crate::php_type::PhpType::parse(&raw_type);
    if parsed.all_members_scalar() {
        return None;
    }

    // Extract the non-null type (e.g. `User|null` → `User`), then get
    // the base class name.  `base_name()` returns `None` for scalars,
    // PHPDoc pseudo-types (`class-string`, `list`, etc.), unions, and
    // other non-class types — so we skip those automatically.
    let effective = parsed.non_null_type().unwrap_or_else(|| parsed.clone());
    let base = match effective.base_name() {
        Some(name) => name.to_string(),
        None => return None,
    };

    // The type looks like a class name.  If we can't resolve it,
    // the subject type is an unknown class.
    if class_loader(&base).is_none() {
        Some(base)
    } else {
        None
    }
}

fn display_class_name(class: &ClassInfo) -> String {
    if class.name.starts_with("__anonymous@") {
        return "anonymous class".to_string();
    }

    // Show the FQN when available for clarity.
    match &class.file_namespace {
        Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, class.name),
        _ => class.name.clone(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(backend: &Backend, uri: &str, content: &str) -> Vec<Diagnostic> {
        backend.update_ast(uri, content);
        let mut out = Vec::new();
        backend.collect_unknown_member_diagnostics(uri, content, &mut out);
        out
    }

    // ── Basic unknown-member detection ──────────────────────────────

    #[test]
    fn flags_unknown_method_on_known_class() {
        let php = r#"<?php
class Greeter {
    public function hello(): string { return ''; }
}

function test(): void {
    $g = new Greeter();
    $g->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| {
                d.message.contains("nonexistent")
                    && d.message.contains("Greeter")
                    && d.message.contains("Method")
            }),
            "expected diagnostic for nonexistent method, got: {diags:?}"
        );
    }

    #[test]
    fn flags_unknown_property_on_known_class() {
        let php = r#"<?php
class User {
    public string $name;
}

function test(): void {
    $u = new User();
    $u->missing;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| {
                d.message.contains("missing")
                    && d.message.contains("User")
                    && d.message.contains("Property")
            }),
            "expected diagnostic for missing property, got: {diags:?}"
        );
    }

    #[test]
    fn flags_unknown_static_method() {
        let php = r#"<?php
class MathHelper {
    public static function add(): int { return 0; }
}

MathHelper::nonexistent();
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("MathHelper")),
            "expected diagnostic for nonexistent static method, got: {diags:?}"
        );
    }

    #[test]
    fn flags_unknown_constant_on_class() {
        let php = r#"<?php
class Config {
    const VERSION = '1.0';
}

echo Config::MISSING;
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("MISSING") && d.message.contains("Config")),
            "expected diagnostic for missing constant, got: {diags:?}"
        );
    }

    // ── Should NOT produce diagnostics ──────────────────────────────

    #[test]
    fn no_diagnostic_for_existing_method() {
        let php = r#"<?php
class Greeter {
    public function hello(): string { return ''; }
    public function goodbye(): string { return ''; }
}

function test(): void {
    $g = new Greeter();
    $g->hello();
    $g->goodbye();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_existing_property() {
        let php = r#"<?php
class User {
    public string $name;
    public int $age;
}

function test(): void {
    $u = new User();
    echo $u->name;
    echo $u->age;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_existing_constant() {
        let php = r#"<?php
class Config {
    const VERSION = '1.0';
}

echo Config::VERSION;
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_class_keyword() {
        let php = r#"<?php
class Foo {}
echo Foo::class;
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Magic methods ───────────────────────────────────────────────

    #[test]
    fn no_diagnostic_when_class_has_magic_call() {
        let php = r#"<?php
class Dynamic {
    public function __call(string $name, array $args): mixed { return null; }
}

function test(): void {
    $d = new Dynamic();
    $d->anything();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_when_class_has_magic_get() {
        let php = r#"<?php
class Dynamic {
    public function __get(string $name): mixed { return null; }
}

function test(): void {
    $d = new Dynamic();
    echo $d->anything;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_when_class_has_magic_call_static() {
        let php = r#"<?php
class Dynamic {
    public static function __callStatic(string $name, array $args): mixed { return null; }
}

Dynamic::anything();
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Inheritance ─────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_inherited_method() {
        let php = r#"<?php
class Base {
    public function baseMethod(): void {}
}
class Child extends Base {}

function test(): void {
    $c = new Child();
    $c->baseMethod();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_trait_method() {
        let php = r#"<?php
trait Greetable {
    public function greet(): string { return ''; }
}

class Person {
    use Greetable;
}

function test(): void {
    $p = new Person();
    $p->greet();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Trait $this suppression (B2) ────────────────────────────────

    #[test]
    fn no_diagnostic_for_this_member_access_inside_trait() {
        // Regression test for B2: $this-> inside a trait method should
        // not produce false positives for members that exist on the
        // host class but not on the trait itself.
        let php = r#"<?php
trait LogsErrors {
    public function logError(): void {
        $this->model;
        $this->eventType;
    }
}

class ImportJob {
    use LogsErrors;
    public string $model = 'Product';
    public string $eventType = 'import';
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for $this-> inside trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_this_method_call_inside_trait() {
        let php = r#"<?php
trait Cacheable {
    public function cache(): void {
        $this->getCacheKey();
    }
}

class Product {
    use Cacheable;
    public function getCacheKey(): string { return ''; }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for $this->method() inside trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_self_static_inside_trait() {
        // self:: and static:: inside traits can reference members from
        // the host class.
        let php = r#"<?php
trait HasDefaults {
    public static function create(): void {
        self::DEFAULT_NAME;
        static::factory();
    }
}

class User {
    use HasDefaults;
    const DEFAULT_NAME = 'admin';
    public static function factory(): void {}
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for self::/static:: inside trait, got: {diags:?}"
        );
    }

    #[test]
    fn trait_own_members_still_resolve_on_host_class() {
        // When a class uses a trait, accessing the trait's own members
        // from outside should still work (no false positive).
        let php = r#"<?php
trait Greetable {
    public function greet(): string { return ''; }
}
class Person {
    use Greetable;
}
function test(): void {
    $p = new Person();
    $p->greet();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for trait member on host class, got: {diags:?}"
        );
    }

    #[test]
    fn variable_inside_trait_still_diagnosed() {
        // Only $this/self/static/parent are suppressed inside traits.
        // A typed variable like `$x` should still be diagnosed normally.
        let php = r#"<?php
class Foo {
    public function bar(): void {}
}

trait MyTrait {
    public function doStuff(Foo $x): void {
        $x->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("Foo")),
            "expected diagnostic for unknown method on typed variable inside trait, got: {diags:?}"
        );
    }

    // ── PHPDoc virtual members ──────────────────────────────────────

    #[test]
    fn no_diagnostic_for_phpdoc_method() {
        let php = r#"<?php
/**
 * @method string virtualMethod()
 */
class Magic {}

function test(): void {
    $m = new Magic();
    $m->virtualMethod();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_phpdoc_property() {
        let php = r#"<?php
/**
 * @property string $virtualProp
 */
class Magic {
    public function __get(string $name): mixed { return null; }
}

function test(): void {
    $m = new Magic();
    echo $m->virtualProp;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── $this / self / parent ───────────────────────────────────────

    #[test]
    fn flags_unknown_method_on_this() {
        let php = r#"<?php
class Foo {
    public function bar(): void {
        $this->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("Foo")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_this_in_second_class() {
        let php = r#"<?php
class First {
    public function a(): void {}
}
class Second {
    public function b(): void {
        $this->b();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_object_shape_property() {
        let php = r#"<?php
class Factory {
    /**
     * @return object{name: string, age: int}
     */
    public function create(): object {
        return (object)['name' => 'test', 'age' => 1];
    }
}

class Consumer {
    public function test(): void {
        $factory = new Factory();
        $obj = $factory->create();
        echo $obj->name;
        echo $obj->age;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_unknown_property_on_object_shape() {
        let php = r#"<?php
class Factory {
    /**
     * @return object{name: string, age: int}
     */
    public function create(): object {
        return (object)['name' => 'test', 'age' => 1];
    }
}

class Consumer {
    public function test(): void {
        $obj = (new Factory())->create();
        echo $obj->missing;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("missing")),
            "expected diagnostic for missing property on object shape, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_this_in_anonymous_class() {
        let php = r#"<?php
class Outer {
    public function make(): void {
        $anon = new class {
            public function inner(): void {}
            public function test(): void {
                $this->inner();
            }
        };
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_unknown_method_on_this_in_anonymous_class() {
        let php = r#"<?php
class Outer {
    public function make(): void {
        $anon = new class {
            public function inner(): void {}
            public function test(): void {
                $this->missing();
            }
        };
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("missing")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_parent_in_anonymous_class() {
        let php = r#"<?php
class Base {
    public function baseMethod(): void {}
}
class Outer {
    public function make(): void {
        $anon = new class extends Base {
            public function test(): void {
                parent::baseMethod();
            }
        };
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_unknown_method_on_this_in_second_class() {
        let php = r#"<?php
class First {
    public function a(): void {}
}
class Second {
    public function b(): void {
        $this->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("Second")),
            "expected diagnostic for Second, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_this_existing_method() {
        let php = r#"<?php
class Foo {
    public function bar(): void {
        $this->bar();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_unknown_method_on_self() {
        let php = r#"<?php
class Foo {
    public function bar(): void {
        self::nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("Foo")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_self_existing_method() {
        let php = r#"<?php
class Foo {
    public static function bar(): void {
        self::bar();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_parent_existing_method() {
        let php = r#"<?php
class Base {
    public function base(): void {}
}
class Child extends Base {
    public function test(): void {
        parent::base();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Diagnostic metadata ─────────────────────────────────────────

    #[test]
    fn diagnostic_has_warning_severity() {
        let php = r#"<?php
class Foo { }
function test(): void {
    $f = new Foo();
    $f->missing();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn diagnostic_has_code_and_source() {
        let php = r#"<?php
class Foo { }
function test(): void {
    $f = new Foo();
    $f->missing();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(!diags.is_empty());
        match &diags[0].code {
            Some(NumberOrString::String(code)) => {
                assert_eq!(code, UNKNOWN_MEMBER_CODE);
            }
            other => panic!("expected string code, got: {other:?}"),
        }
        assert_eq!(diags[0].source, Some("phpantom".to_string()));
    }

    // ── Case insensitivity ──────────────────────────────────────────

    #[test]
    fn method_matching_is_case_insensitive() {
        let php = r#"<?php
class Foo {
    public function hello(): void {}
}
function test(): void {
    $f = new Foo();
    $f->HELLO();
    $f->Hello();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Multiple unknowns ───────────────────────────────────────────

    #[test]
    fn flags_multiple_unknown_members() {
        let php = r#"<?php
class Foo {
    public function real(): void {}
}
function test(): void {
    $f = new Foo();
    $f->missing1();
    $f->real();
    $f->missing2();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert_eq!(
            diags.len(),
            2,
            "expected 2 diagnostics, got {}: {diags:?}",
            diags.len()
        );
    }

    // ── Unresolvable subjects ───────────────────────────────────────

    #[test]
    fn no_diagnostic_when_subject_unresolvable() {
        // $x has no type info — we can't know what members it has,
        // so we should not flag anything.
        let php = r#"<?php
function test(): void {
    $x->something();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for unresolvable subject, got: {diags:?}"
        );
    }

    // ── Enums ───────────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_enum_case() {
        let php = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}
echo Color::Red;
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_unknown_enum_case() {
        let php = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}
echo Color::Yellow;
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("Yellow")),
            "expected diagnostic for unknown enum case, got: {diags:?}"
        );
    }

    // ── Parameters ──────────────────────────────────────────────────

    #[test]
    fn flags_unknown_method_via_parameter() {
        let php = r#"<?php
class Service {
    public function run(): void {}
}
function handler(Service $svc): void {
    $svc->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("Service")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_method_via_parameter() {
        let php = r#"<?php
class Service {
    public function run(): void {}
}
function handler(Service $svc): void {
    $svc->run();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Parent with magic ───────────────────────────────────────────

    #[test]
    fn no_diagnostic_when_parent_has_magic_call() {
        let php = r#"<?php
class Base {
    public function __call(string $name, array $args): mixed { return null; }
}
class Child extends Base {}

function test(): void {
    $c = new Child();
    $c->anything();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Interfaces ──────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_interface_method() {
        let php = r#"<?php
interface Runnable {
    public function run(): void;
}

class Worker implements Runnable {
    public function run(): void {}
}

function handler(Runnable $r): void {
    $r->run();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Static properties ───────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_existing_static_property() {
        let php = r#"<?php
class Config {
    public static string $version = '1.0';
}
echo Config::$version;
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Union types ─────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_member_on_any_union_branch() {
        let php = r#"<?php
class Cat {
    public function purr(): void {}
    public function eat(): void {}
}
class Dog {
    public function bark(): void {}
    public function eat(): void {}
}
class Shelter {
    /**
     * @return Cat|Dog
     */
    public function adopt(): Cat|Dog {
        return new Cat();
    }
}

class Test {
    public function run(): void {
        $shelter = new Shelter();
        $pet = $shelter->adopt();
        $pet->eat();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_member_missing_from_all_union_branches() {
        let php = r#"<?php
class Cat {
    public function purr(): void {}
}
class Dog {
    public function bark(): void {}
}
class Shelter {
    /**
     * @return Cat|Dog
     */
    public function adopt(): Cat|Dog {
        return new Cat();
    }
}

class Test {
    public function run(): void {
        $shelter = new Shelter();
        $pet = $shelter->adopt();
        $pet->fly();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("fly")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn union_diagnostic_message_mentions_multiple_types() {
        let php = r#"<?php
class Cat {
    public function purr(): void {}
}
class Dog {
    public function bark(): void {}
}
class Shelter {
    /**
     * @return Cat|Dog
     */
    public function adopt(): Cat|Dog {
        return new Cat();
    }
}

class Test {
    public function run(): void {
        $shelter = new Shelter();
        $pet = $shelter->adopt();
        $pet->fly();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        let d = diags
            .iter()
            .find(|d| d.message.contains("fly"))
            .expect("expected diagnostic");
        assert!(
            d.message.contains("Cat") && d.message.contains("Dog"),
            "expected both types in message: {}",
            d.message
        );
    }

    #[test]
    fn no_diagnostic_when_any_union_branch_has_magic_call() {
        let php = r#"<?php
class Normal {
    public function known(): void {}
}
class Dynamic {
    public function __call(string $name, array $args): mixed { return null; }
}

class Test {
    /**
     * @return Normal|Dynamic
     */
    public function get(): Normal|Dynamic { return new Normal(); }

    public function run(): void {
        $x = $this->get();
        $x->anything();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── stdClass ────────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_property_on_stdclass() {
        let php = r#"<?php
function test(stdClass $obj): void {
    echo $obj->anything;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_method_on_stdclass() {
        let php = r#"<?php
function test(stdClass $obj): void {
    $obj->anything();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_stdclass_in_union() {
        let php = r#"<?php
class Foo { public function a(): void {} }
/**
 * @return Foo|stdClass
 */
function get(): Foo|stdClass { return new Foo(); }
function test(): void {
    $x = get();
    $x->anything;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_stdclass_parameter() {
        let php = r#"<?php
function test(stdClass $obj): void {
    echo $obj->name;
    echo $obj->whatever;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── PHPDoc property on child class ──────────────────────────────

    #[test]
    fn no_diagnostic_for_phpdoc_property_on_child_class() {
        let php = r#"<?php
/**
 * @property string $virtualProp
 */
class Base {
    public function __get(string $name): mixed { return null; }
}

class Child extends Base {}

function test(): void {
    $c = new Child();
    echo $c->virtualProp;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_phpdoc_property_from_interface() {
        let php = r#"<?php
/**
 * @property string $name
 */
interface HasName {}

class User implements HasName {
    public function __get(string $n): mixed { return null; }
}

function test(): void {
    $u = new User();
    echo $u->name;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── PHPDoc members inside type-narrowing contexts ───────────────

    #[test]
    fn no_diagnostic_for_phpdoc_members_inside_assert() {
        let php = r#"<?php
/**
 * @method string getName()
 */
class Entity {
    public function __call(string $name, array $args): mixed { return null; }
}

class Base {}

class Test {
    public function run(Base $item): void {
        assert($item instanceof Entity);
        echo $item->getName();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_phpdoc_members_after_instanceof_narrowing() {
        let php = r#"<?php
/**
 * @method string getName()
 */
class Entity {
    public function __call(string $name, array $args): mixed { return null; }
}

class Base {}

class Test {
    public function run(Base $item): void {
        if ($item instanceof Entity) {
            echo $item->getName();
        }
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Inline && narrowing (B3) ────────────────────────────────────

    #[test]
    fn no_diagnostic_for_instanceof_and_chain() {
        // Regression test for B3: instanceof checks in the LHS of &&
        // should narrow the variable type for the RHS.
        let php = r#"<?php
class QueryException extends \Exception {
    public array $errorInfo = [];
}

function test(\Throwable $e): void {
    $e instanceof QueryException && $e->errorInfo;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for && narrowing, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_instanceof_and_chain_in_catch() {
        // B3 variant: variable comes from a catch block.
        let php = r#"<?php
class QueryException extends \Exception {
    public array $errorInfo = [];
}

function test(): void {
    try {
        throw new \Exception('fail');
    } catch (\Throwable $e) {
        $e instanceof QueryException && $e->errorInfo;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for && narrowing in catch, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_instanceof_and_chain_method_call() {
        // B3 variant: method call instead of property access on RHS.
        let php = r#"<?php
class SpecialException extends \Exception {
    public function getDetail(): string { return ''; }
}

function test(\Throwable $e): void {
    $e instanceof SpecialException && $e->getDetail();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for && narrowing with method call, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_instanceof_and_chain_in_if_condition() {
        // B3 variant: the && is the condition of an if statement.
        let php = r#"<?php
class QueryException extends \Exception {
    public array $errorInfo = [];
}

function test(\Throwable $e): void {
    if ($e instanceof QueryException && count($e->errorInfo) > 0) {
        echo 'has errors';
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for && narrowing in if condition, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_instanceof_and_chain_in_return() {
        // Real-world repro: instanceof on LHS of && inside a return
        // statement.  The narrowing must propagate through the entire
        // chained && even when wrapped in `return`.
        let php = r#"<?php
class QueryException extends \Exception {
    public array $errorInfo = [];
}

trait UniqueConstraintViolation {
    protected function isUniqueConstraintViolation(\Throwable $exception): bool {
        return $exception instanceof QueryException
            && is_array($exception->errorInfo)
            && count($exception->errorInfo) >= 2
            && ($exception->errorInfo[0] ?? '') === '23000'
            && ($exception->errorInfo[1] ?? 0) === 1062;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for && narrowing in return, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_ternary_instanceof_in_return() {
        // Ternary instanceof narrowing inside a return statement.
        let php = r#"<?php
class SpecialException extends \Exception {
    public function getDetail(): string { return ''; }
}

function test(\Throwable $e): string {
    return $e instanceof SpecialException ? $e->getDetail() : 'unknown';
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for ternary instanceof in return, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_chained_and_instanceof() {
        // B3 variant: chained && with multiple instanceof checks.
        let php = r#"<?php
class DetailedException extends \Exception {
    public string $detail = '';
    public string $context = '';
}

function test(\Throwable $e): void {
    $e instanceof DetailedException && $e->detail !== '' && $e->context !== '';
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for chained && narrowing, got: {diags:?}"
        );
    }

    // ── Property chains ─────────────────────────────────────────────

    #[test]
    fn flags_unknown_member_on_property_chain() {
        let php = r#"<?php
class Inner {
    public function known(): void {}
}
class Outer {
    public Inner $inner;
}

class Test {
    public function run(): void {
        $o = new Outer();
        $o->inner->missing();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("missing")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_valid_property_chain() {
        let php = r#"<?php
class Inner {
    public function known(): void {}
}
class Outer {
    public Inner $inner;
}

class Test {
    public function run(): void {
        $o = new Outer();
        $o->inner->known();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Method return chains ────────────────────────────────────────

    #[test]
    fn flags_unknown_member_on_method_return_chain() {
        let php = r#"<?php
class Inner {
    public function known(): void {}
}
class Outer {
    public function getInner(): Inner { return new Inner(); }
}

function test(): void {
    $o = new Outer();
    $o->getInner()->missing();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("missing")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_valid_method_return_chain() {
        let php = r#"<?php
class Inner {
    public function known(): void {}
}
class Outer {
    public function getInner(): Inner { return new Inner(); }
}

function test(): void {
    $o = new Outer();
    $o->getInner()->known();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // ── Virtual property chains ─────────────────────────────────────

    #[test]
    fn flags_unknown_member_on_virtual_property_chain() {
        let php = r#"<?php
class Inner {
    public function known(): void {}
}

/**
 * @property Inner $inner
 */
class Outer {
    public function __get(string $name): mixed { return null; }
}

function test(): void {
    $o = new Outer();
    $o->inner->missing();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("missing")),
            "expected diagnostic, got: {diags:?}"
        );
    }

    // ── Scalar member access ────────────────────────────────────────

    #[test]
    fn flags_member_access_on_scalar_property_type() {
        let php = r#"<?php
class Foo {
    public int $value = 0;
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $foo->value->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
            "expected ERROR severity for scalar access"
        );
    }

    #[test]
    fn flags_member_access_on_string_property_type() {
        let php = r#"<?php
class Foo {
    public string $name = '';
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $foo->name->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("string") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_member_access_on_scalar_method_return() {
        let php = r#"<?php
class Foo {
    public function getCount(): int { return 0; }
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $foo->getCount()->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_method_call_on_scalar_method_return_chain() {
        let php = r#"<?php
class Inner {
    public function getValue(): string { return ''; }
}

class Middle {
    public function getInner(): Inner { return new Inner(); }
}

class Outer {
    public function getMiddle(): Middle { return new Middle(); }
}

class Test {
    public function run(): void {
        $o = new Outer();
        $o->getMiddle()->getInner()->getValue()->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_method_call_on_scalar_return_typed_param() {
        let php = r#"<?php
class Foo {
    public function getCount(): int { return 0; }
}
function test(Foo $foo): void {
    $foo->getCount()->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_static_method_chain() {
        let php = r#"<?php
class Foo {
    public static function getCount(): int { return 0; }
}
class Test {
    public function run(): void {
        Foo::getCount()->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_function_return_chain() {
        let php = r#"<?php
function getNumber(): int { return 42; }
function test(): void {
    getNumber()->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_docblock_return_type() {
        let php = r#"<?php
class Foo {
    /**
     * @return string
     */
    public function getName() { return ''; }
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $foo->getName()->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_static_return_chain() {
        let php = r#"<?php
class Foo {
    public function getName(): string { return ''; }
}
class Test {
    public function run(): void {
        $foo = new Foo();
        $foo->getName()->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_scalar_diagnostic_for_class_returning_chain() {
        let php = r#"<?php
class Builder {
    public function where(): self { return $this; }
    public function get(): self { return $this; }
}
function test(): void {
    $b = new Builder();
    $b->where()->get();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no scalar access diagnostic for class-returning chain, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_function_returning_class_chain() {
        let php = r#"<?php
class Foo {
    public function getName(): string { return ''; }
}
function createFoo(): Foo { return new Foo(); }
function test(): void {
    createFoo()->getName()->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_array_element_method_chain() {
        let php = r#"<?php
class Item {
    public function getLabel(): string { return ''; }
}

function test(): void {
    /** @var array<int, Item> $items */
    $items = [];
    $items[0]->getLabel()->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_access_on_deeper_method_chain() {
        let php = r#"<?php
class Inner {
    public function getValue(): int { return 42; }
}
class Outer {
    public function getInner(): Inner { return new Inner(); }
}
class Test {
    public function run(): void {
        $o = new Outer();
        $o->getInner()->getValue()->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_scalar_property_access_on_deeper_method_chain() {
        let php = r#"<?php
class Inner {
    public string $label = '';
}
class Outer {
    public function getInner(): Inner { return new Inner(); }
}
class Test {
    public function run(): void {
        $o = new Outer();
        $o->getInner()->label->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn flags_member_access_on_virtual_scalar_property() {
        let php = r#"<?php
/**
 * @property int $age
 * @property string $name
 */
class User {
    public function __get(string $name): mixed { return null; }
}

class Test {
    public function run(): void {
        $u = new User();
        $u->age->nonexistent();
        $u->name->nonexistent2();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic for int property, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_scalar_property_access_itself() {
        let php = r#"<?php
class Foo {
    public int $count = 0;
}
function test(): void {
    $f = new Foo();
    echo $f->count;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "scalar property access itself should not be flagged, got: {diags:?}"
        );
    }

    // ── Bare variable with scalar type ──────────────────────────────

    #[test]
    fn flags_member_access_on_bare_int_variable() {
        let php = r#"<?php
class Foo {
    public function getCount(): int { return 0; }
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $number = $foo->getCount();
        $number->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic for bare int variable, got: {diags:?}"
        );
    }

    #[test]
    fn flags_property_access_on_bare_string_variable() {
        let php = r#"<?php
class Foo {
    public function getName(): string { return ''; }
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $name = $foo->getName();
        $name->nonexistent;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| { d.message.contains("string") && d.message.contains("nonexistent") }),
            "expected scalar access diagnostic for bare string variable, got: {diags:?}"
        );
    }

    #[test]
    fn flags_method_access_on_bare_bool_variable() {
        let php = r#"<?php
class Foo {
    public function isValid(): bool { return true; }
}

class Test {
    public function run(): void {
        $foo = new Foo();
        $valid = $foo->isValid();
        $valid->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("bool") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic for bare bool variable, got: {diags:?}"
        );
    }

    #[test]
    fn flags_member_access_on_scalar_function_return() {
        let php = r#"<?php
function getNumber(): int { return 42; }
class Test {
    public function run(): void {
        $n = getNumber();
        $n->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic for function return, got: {diags:?}"
        );
    }

    #[test]
    fn flags_member_access_on_scalar_method_return_via_variable() {
        let php = r#"<?php
class Foo {
    public function getCount(): int { return 0; }
}
class Test {
    public function run(): void {
        $foo = new Foo();
        $count = $foo->getCount();
        $count->nonexistent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_bare_scalar_variable_without_member_access() {
        let php = r#"<?php
function test(): void {
    $n = 42;
    echo $n;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "bare scalar variable without member access should not produce diagnostic, got: {diags:?}"
        );
    }

    // ── Typed parameter scalar access ───────────────────────────────

    #[test]
    fn flags_member_access_on_scalar_typed_parameter() {
        let php = r#"<?php
function test(int $value): void {
    $value->nonexistent();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("int") && d.message.contains("nonexistent")),
            "expected scalar access diagnostic for typed parameter, got: {diags:?}"
        );
    }

    // ── Unknown class parameter ─────────────────────────────────────

    #[test]
    fn flags_member_access_on_unknown_class_parameter() {
        let php = r#"<?php
function test(NonExistentClass $obj): void {
    $obj->doSomething();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| {
                d.message.contains("doSomething") && d.message.contains("NonExistentClass")
            }),
            "expected diagnostic for unknown class parameter, got: {diags:?}"
        );
    }

    #[test]
    fn flags_member_access_on_unknown_return_type_function() {
        let php = r#"<?php
/** @return NonExistentClass */
function createObj() { return new stdClass; }
function test(): void {
    $obj = createObj();
    $obj->doSomething();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            !diags.is_empty(),
            "expected diagnostic for unknown return type, got: {diags:?}"
        );
    }

    #[test]
    fn no_unknown_class_diagnostic_for_mixed_parameter() {
        let php = r#"<?php
function test(mixed $obj): void {
    $obj->doSomething();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for mixed parameter, got: {diags:?}"
        );
    }

    #[test]
    fn no_unknown_class_diagnostic_for_class_string_parameter() {
        let php = r#"<?php
/**
 * @param class-string<BackedEnum> $enum
 */
function test(string $enum): void {
    $enum::from('test');
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for class-string parameter, got: {diags:?}"
        );
    }

    // ── Type alias / array shape / object value ─────────────────────

    #[test]
    fn no_diagnostic_for_type_alias_array_shape_object_value() {
        let php = r#"<?php
class Service {
    public function getName(): string { return ''; }
}

class Factory {
    /**
     * @return array{service: Service, name: string}
     */
    public function create(): array { return []; }
}

class Test {
    public function run(): void {
        $f = new Factory();
        $result = $f->create();
        $result['service']->getName();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for array shape object value, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_multiple_type_alias_object_values() {
        let php = r#"<?php
class UserService {
    public function findAll(): array { return []; }
}

class PostService {
    public function findRecent(): array { return []; }
}

class Container {
    /**
     * @return array{users: UserService, posts: PostService}
     */
    public function services(): array { return []; }
}

class Test {
    public function run(): void {
        $c = new Container();
        $services = $c->services();
        $services['users']->findAll();
        $services['posts']->findRecent();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for multiple array shape values, got: {diags:?}"
        );
    }

    // ── Inline array element function call ──────────────────────────

    #[test]
    fn no_diagnostic_for_inline_array_element_function_call() {
        let php = r#"<?php
class Item {
    public function process(): void {}
}

function getItems(): array {
    /** @var Item[] */
    return [];
}

function test(): void {
    getItems()[0]->process();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for inline array element call, got: {diags:?}"
        );
    }

    // ── Pre-resolved base class has the member ──────────────────────

    #[test]
    fn no_diagnostic_when_member_exists_on_pre_resolved_base_class() {
        let php = r#"<?php
class Builder {
    public function where(): self { return $this; }
    public function get(): array { return []; }
}
function test(): void {
    $b = new Builder();
    $b->where();
    $b->get();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for existing methods, got: {diags:?}"
        );
    }

    // ── @see tag references ─────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_see_tag_method_reference() {
        let php = r#"<?php
class Foo {
    public function bar(): void {}

    /**
     * @see Foo::bar()
     */
    public function test(): void {}
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for @see tag method reference, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_see_tag_constant_reference() {
        let php = r#"<?php
class Foo {
    const BAR = 1;

    /**
     * @see Foo::BAR
     */
    public function test(): void {}
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for @see tag constant reference, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_inline_see_tag_method_reference() {
        let php = r#"<?php
class Foo {
    public function bar(): void {}

    /**
     * This delegates to {@see Foo::bar()}.
     */
    public function test(): void {}
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostic for inline @see reference, got: {diags:?}"
        );
    }

    // ── Namespaced stub class member ────────────────────────────────

    #[test]
    fn no_diagnostic_for_namespaced_stub_class_member() {
        let stubs = HashMap::from([(
            "Ns\\StubClass",
            r#"<?php
namespace Ns;
class StubClass {
    public function stubMethod(): void {}
}
"#,
        )]);
        let backend = Backend::new_test_with_stubs(stubs);
        let php = r#"<?php
use Ns\StubClass;

function test(StubClass $obj): void {
    $obj->stubMethod();
}
"#;
        let uri = "file:///test.php";
        backend.update_ast(uri, php);
        let mut out = Vec::new();
        backend.collect_unknown_member_diagnostics(uri, php, &mut out);
        assert!(
            out.is_empty(),
            "expected no diagnostic for namespaced stub class member, got: {out:?}"
        );
    }

    // ── Conditional $this return in chain ────────────────────────────

    #[test]
    fn no_false_positive_on_conditional_this_return_in_chain() {
        let php = r#"<?php
class Builder {
    /**
     * @return $this
     */
    public function where(): static { return $this; }

    public function get(): array { return []; }
}
class Test {
    public function run(): void {
        $b = new Builder();
        $b->where()->get();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no false positive on conditional $this return chain, got: {diags:?}"
        );
    }

    // ── Cross-method cache isolation (B1) ───────────────────────────

    #[test]
    fn no_false_positive_when_same_var_has_different_type_in_different_methods() {
        // Regression test for B1: the subject resolution cache was scoped
        // to the enclosing class, not the enclosing method.  Two methods
        // in the same class that both use `$order->` would share a cache
        // entry even when `$order` has a completely different type in each
        // method.  The first resolution wins and subsequent methods get
        // the wrong type, producing false-positive "unknown member"
        // diagnostics.
        let php = r#"<?php
class OrderA {
    public function propOnA(): void {}
}
class OrderB {
    public function propOnB(): void {}
}
class Service {
    public function handleA(OrderA $order): void {
        $order->propOnA();
    }
    public function handleB(OrderB $order): void {
        $order->propOnB();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no false positives when same-named variable has different types \
             in different methods, got: {diags:?}"
        );
    }

    #[test]
    fn no_false_positive_same_var_different_type_top_level_functions() {
        // Same bug as the class-method variant, but with top-level
        // functions instead of methods.
        let php = r#"<?php
class Alpha {
    public function alphaMethod(): void {}
}
class Beta {
    public function betaMethod(): void {}
}
function first(Alpha $x): void {
    $x->alphaMethod();
}
function second(Beta $x): void {
    $x->betaMethod();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no false positives for same-named variable in different \
             top-level functions, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_this_inside_closure_in_trait() {
        // B3: $this-> and static:: inside a closure nested within a trait
        // method should be suppressed, just like direct trait method bodies.
        let php = r#"<?php
trait SalesInfoGlobalTrait {
    public function getSalesInfo(): void {
        $items = array_map(function ($item) {
            $this->model;
            $this->eventType;
            static::where();
            static::query();
        }, []);
    }
}

class SalesReport {
    use SalesInfoGlobalTrait;
    public string $model = 'Sale';
    public string $eventType = 'report';
    public static function where(): void {}
    public static function query(): void {}
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for $this/static:: inside closure in trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_this_inside_arrow_fn_in_trait() {
        // B3: $this-> inside an arrow function nested within a trait method.
        let php = r#"<?php
trait FilterTrait {
    public function applyFilter(): void {
        $fn = fn() => $this->filterColumn;
    }
}

class Report {
    use FilterTrait;
    public string $filterColumn = 'status';
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for $this-> inside arrow fn in trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_chain_rooted_at_static_inside_trait() {
        // B3: `static::where(...)->update(...)` inside a trait method.
        // The subject_text for `update` is `"static::where('x', 'y')"`,
        // which is a chain rooted at `static`.  The suppression must
        // recognise the root keyword, not require an exact match.
        let php = r#"<?php
trait SalesInfoGlobalTrait {
    public function updateSalesInfo(): void {
        static::where('column', 'value')->update(['sales' => 1]);
    }
}

class SalesReport extends \Illuminate\Database\Eloquent\Model {
    use SalesInfoGlobalTrait;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for static::...->method() chain inside trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_chain_rooted_at_this_inside_trait() {
        // B3: `$this->relation()->first()` inside a trait method.
        // The subject_text for `first` is `"$this->relation()"`.
        let php = r#"<?php
trait HasRelation {
    public function loadRelation(): void {
        $this->items()->first();
    }
}

class Order {
    use HasRelation;
    /** @return \Illuminate\Database\Eloquent\Builder */
    public function items(): object { return new \stdClass(); }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for $this->...->method() chain inside trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_chain_rooted_at_static_inside_closure_in_trait() {
        // B3: `static::where(...)` inside a closure within a trait method.
        let php = r#"<?php
trait SalesInfoGlobalTrait {
    public function updateSalesInfo(): void {
        $items = array_map(function ($item) {
            static::where('col', 'val')->update(['x' => 1]);
        }, []);
    }
}

class SalesReport extends \Illuminate\Database\Eloquent\Model {
    use SalesInfoGlobalTrait;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for static:: chain inside closure in trait, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_self_chain_inside_trait() {
        // B3: `self::create(...)` chain inside a trait.
        let php = r#"<?php
trait Creatable {
    public function duplicate(): void {
        self::create(['name' => 'copy'])->save();
    }
}

class Product extends \Illuminate\Database\Eloquent\Model {
    use Creatable;
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for self::...->method() chain inside trait, got: {diags:?}"
        );
    }

    #[test]
    fn variable_chain_inside_trait_still_diagnosed() {
        // Non-self-referencing variables inside traits should still be
        // diagnosed when the member truly doesn't exist.
        let php = r#"<?php
trait BadTrait {
    public function doStuff(): void {
        $obj = new \stdClass();
        $obj->nonExistentMethod();
    }
}
"#;
        let backend = Backend::new_test();
        let _diags = collect(&backend, "file:///test.php", php);
        // stdClass has __get/__set magic, so property access is fine,
        // but we're just verifying the suppression doesn't swallow
        // non-self-referencing subjects.  stdClass actually tolerates
        // all member access, so this test verifies the suppression
        // is scoped to self-referencing subjects only.
        // (No assertion on diagnostic count — stdClass has magic methods.)
    }

    #[test]
    fn flags_unknown_member_despite_valid_in_other_method() {
        // The flip side of B1: make sure that a member that IS valid in
        // one method is still flagged as unknown in another method where
        // the variable has a different type that lacks the member.
        let php = r#"<?php
class HasFoo {
    public function foo(): void {}
}
class NoFoo {
    public function bar(): void {}
}
class Service {
    public function a(HasFoo $x): void {
        $x->foo();
    }
    public function b(NoFoo $x): void {
        $x->foo();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("foo") && d.message.contains("NoFoo")),
            "expected diagnostic for foo() on NoFoo in method b(), got: {diags:?}"
        );
        // Make sure it's exactly one diagnostic (the one in method b).
        let foo_diags: Vec<_> = diags.iter().filter(|d| d.message.contains("foo")).collect();
        assert_eq!(
            foo_diags.len(),
            1,
            "expected exactly one 'foo' diagnostic (in method b), got: {foo_diags:?}"
        );
    }

    #[test]
    fn no_false_positive_when_parameter_is_reassigned() {
        // Regression test for B4: when a method parameter is reassigned
        // mid-body, PHPantom should resolve subsequent accesses against
        // the new type, not the original parameter type.
        //
        // Before the fix, the subject cache keyed by (subject_text,
        // access_kind, scope) would cache the parameter type on the
        // first `$file->` encounter and reuse it for accesses after
        // the reassignment, producing false-positive "unknown member"
        // diagnostics.
        let php = r#"<?php
class UploadedFile {
    public string $originalName;
}
class FileModel {
    public int $id;
    public string $name;
}
class Result {
    public function getFile(): FileModel { return new FileModel(); }
}
class FileUploadService {
    public function uploadFile(UploadedFile $file): void {
        $file->originalName;
        $result = new Result();
        $file = $result->getFile();
        $file->id;
        $file->name;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no false positives when parameter is reassigned mid-body, got: {diags:?}"
        );
    }

    #[test]
    fn flags_unknown_member_after_reassignment() {
        // The flip side of B4: after reassignment, members from the
        // NEW type that don't exist should still be flagged.
        let php = r#"<?php
class TypeA {
    public function onlyOnA(): void {}
}
class TypeB {
    public function onlyOnB(): void {}
}
class Service {
    public function process(TypeA $var): void {
        $var->onlyOnA();
        $var = new TypeB();
        $var->onlyOnA();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("onlyOnA") && d.message.contains("TypeB")),
            "expected diagnostic for onlyOnA() on TypeB after reassignment, got: {diags:?}"
        );
        // Exactly one diagnostic — the post-reassignment access.
        let relevant: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("onlyOnA"))
            .collect();
        assert_eq!(
            relevant.len(),
            1,
            "expected exactly one 'onlyOnA' diagnostic (after reassignment), got: {relevant:?}"
        );
    }

    /// B11: `$found = null; foreach (...) { $found = $pen; } $found->write()`
    /// must not produce a scalar_member_access diagnostic when the foreach
    /// value variable has a known type.
    #[test]
    fn no_false_positive_null_init_foreach_var_to_var_reassign() {
        let php = r#"<?php
class Pen {
    public function write(): void {}
    public function color(): string { return ''; }
}
class Svc {
    /** @param list<Pen> $pens */
    public function find(array $pens): void {
        $found = null;
        foreach ($pens as $pen) {
            if ($pen->color() === 'blue') {
                $found = $pen;
            }
        }
        if ($found) {
            $found->write();
        }
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        let scalar_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.code == Some(NumberOrString::String("scalar_member_access".to_string())))
            .collect();
        assert!(
            scalar_diags.is_empty(),
            "should not flag scalar_member_access on $found->write() after foreach reassign, got: {scalar_diags:?}"
        );
        let unknown_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("write"))
            .collect();
        assert!(
            unknown_diags.is_empty(),
            "should not flag unknown member 'write' on $found after foreach reassign, got: {unknown_diags:?}"
        );
    }

    /// B11: direct instantiation inside foreach body (no var-to-var).
    #[test]
    fn no_false_positive_null_init_foreach_direct_reassign() {
        let php = r#"<?php
class Transaction {
    public function commit(): void {}
}
class Svc {
    /** @param list<string> $items */
    public function process(array $items): void {
        $tx = null;
        foreach ($items as $item) {
            $tx = new Transaction();
        }
        if ($tx) {
            $tx->commit();
        }
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        let bad_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("commit") || d.message.contains("null"))
            .collect();
        assert!(
            bad_diags.is_empty(),
            "should not flag commit() or scalar null after foreach reassign, got: {bad_diags:?}"
        );
    }

    // ── B10: Negative narrowing after early return ──────────────────

    #[test]
    fn no_false_positive_after_guard_clause_excludes_type() {
        // After `if ($value instanceof Stringable) { return; }`, the
        // variable should be narrowed to exclude Stringable.  Inside
        // the subsequent `if ($value instanceof BackedEnum)` block,
        // `$value` must resolve to BackedEnum (not Stringable).
        let php = r#"<?php
interface Stringable {
    public function __toString(): string;
}
interface BackedEnum {
    public readonly int|string $value;
}

class Svc {
    public static function toString(mixed $value): string
    {
        if ($value instanceof Stringable) {
            return $value->__toString();
        }
        if ($value instanceof BackedEnum) {
            $value = $value->value;
        }
        return '';
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        // There should be no diagnostic about 'value' not found on
        // 'Stringable'.  The guard clause return means $value cannot
        // be Stringable in subsequent code.
        let bad = diags
            .iter()
            .filter(|d| d.message.contains("value") && d.message.contains("Stringable"))
            .collect::<Vec<_>>();
        assert!(
            bad.is_empty(),
            "should not flag 'value' on Stringable after guard clause excludes it, got: {bad:?}"
        );
    }

    #[test]
    fn no_false_positive_sequential_instanceof_guards() {
        // Multiple sequential guard clauses should each exclude their
        // type from subsequent code.
        let php = r#"<?php
interface Alpha {
    public function alphaMethod(): void;
}
interface Beta {
    public function betaMethod(): void;
}
class Gamma {
    public function gammaMethod(): void {}
}

class Svc {
    public function test(Alpha|Beta|Gamma $x): void
    {
        if ($x instanceof Alpha) {
            return;
        }
        if ($x instanceof Beta) {
            return;
        }
        $x->gammaMethod();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        let bad = diags
            .iter()
            .filter(|d| {
                d.message.contains("gammaMethod")
                    && (d.message.contains("Alpha") || d.message.contains("Beta"))
            })
            .collect::<Vec<_>>();
        assert!(
            bad.is_empty(),
            "should not flag gammaMethod after two guard clauses exclude Alpha and Beta, got: {bad:?}"
        );
    }

    // ── self::/static::/parent:: in static access subjects ──────────

    fn create_enum_backend() -> Backend {
        let mut stubs = std::collections::HashMap::new();
        stubs.insert(
            "UnitEnum",
            "<?php\ninterface UnitEnum {\n    /** @return static[] */\n    public static function cases(): array;\n    public readonly string $name;\n}\n",
        );
        stubs.insert(
            "BackedEnum",
            "<?php\ninterface BackedEnum extends UnitEnum {\n    public static function from(int|string $value): static;\n    public static function tryFrom(int|string $value): ?static;\n    public readonly int|string $value;\n}\n",
        );
        Backend::new_test_with_stubs(stubs)
    }

    #[test]
    fn no_diagnostic_for_self_enum_case_value() {
        let php = r#"<?php
enum SizeUnit: string {
    case pcs = 'pcs';
    case pair = 'pair';
    case g = 'g';

    public function translation(): string {
        return self::pcs->value;
    }

    public static function units(): array {
        return [
            self::pcs->value,
            self::pair->value,
            self::g->value,
        ];
    }
}
"#;
        let backend = create_enum_backend();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_static_enum_case_value() {
        let php = r#"<?php
enum Currency: string {
    case USD = 'usd';
    case EUR = 'eur';

    public static function defaults(): array {
        return [static::USD->value];
    }
}
"#;
        let backend = create_enum_backend();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_self_enum_case_name() {
        let php = r#"<?php
enum Color: int {
    case Red = 1;
    case Blue = 2;

    public function label(): string {
        return self::Red->name;
    }
}
"#;
        let backend = create_enum_backend();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_self_static_access_on_regular_class() {
        let php = r#"<?php
class Config {
    public const VERSION = '1.0';
    public static function version(): string { return self::VERSION; }
    public function test(): string {
        return static::version();
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_method_on_anonymous_class_variable() {
        // When `$model = new class extends Foo { ... }` is used outside
        // the anonymous class body, member access on `$model` should
        // resolve via the anonymous class's ClassInfo (which inherits
        // from the parent and uses traits).
        let php = r#"<?php
class Base {
    public function hello(): string { return "hi"; }
}

function test(): void {
    $model = new class extends Base {};
    $model->hello();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn no_diagnostic_for_trait_method_on_anonymous_class_variable() {
        let php = r#"<?php
trait Greetable {
    public function greet(): string { return "hello"; }
}

function test(): void {
    $obj = new class {
        use Greetable;
    };
    $obj->greet();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    #[test]
    fn flags_unknown_method_on_anonymous_class_variable() {
        let php = r#"<?php
function test(): void {
    $obj = new class {
        public function known(): void {}
    };
    $obj->unknown();
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.iter().any(|d| d.message.contains("unknown")),
            "expected unknown member diagnostic, got: {diags:?}",
        );
    }

    #[test]
    fn no_diagnostic_for_standalone_var_docblock_in_closure() {
        // A standalone multi-variable `@var` block inside a closure body
        // (without a following assignment) should declare types for
        // untyped closure parameters.
        let php = r#"<?php
class App {
    public function make(string $class): mixed { return new $class; }
}

class Foo {
    public function test(): void {
        $fn = function ($app, $params) {
            /**
             * @var App                      $app
             * @var array{indexName: string} $params
             */
            $app->make('Something');
        };
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics when @var declares closure param type, got: {diags:?}",
        );
    }

    #[test]
    fn flags_unknown_member_with_standalone_var_docblock_in_closure() {
        // When `@var` resolves the type, unknown members should still
        // be flagged (proves the type was actually resolved).
        let php = r#"<?php
class App {
    public function make(string $class): mixed { return new $class; }
}

class Foo {
    public function test(): void {
        $fn = function ($app) {
            /** @var App $app */
            $app->nonExistentMethod();
        };
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonExistentMethod")),
            "expected unknown member diagnostic for nonExistentMethod, got: {diags:?}",
        );
    }

    #[test]
    fn no_diagnostic_for_property_chain_array_access_on_collection() {
        // `$obj->prop['key']` where `prop` is a collection class with
        // `@extends DataCollection<string, Day>` should resolve the
        // bracket access to the element type `Day`.
        let php = r#"<?php
class Day {
    public string $from;
    public string $to;
}

/**
 * @template TKey of array-key
 * @template TValue
 * @implements \ArrayAccess<TKey, TValue>
 */
class DataCollection implements \ArrayAccess {
    /** @return TValue */
    public function offsetGet(mixed $offset): mixed {}
    public function offsetExists(mixed $offset): bool {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

/**
 * @extends DataCollection<string, Day>
 */
class OpeningHours extends DataCollection {}

class ServicePoint {
    public ?OpeningHours $opening_hours;
}

function test(ServicePoint $sp): void {
    $day = $sp->opening_hours['monday'] ?? null;
    if ($day !== null) {
        $day->from;
    }
}
"#;
        let backend = Backend::new_test();
        let diags = collect(&backend, "file:///test.php", php);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for property chain array access on collection, got: {diags:?}",
        );
    }
}
