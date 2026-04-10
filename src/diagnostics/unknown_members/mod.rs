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
use crate::completion::resolver::{ResolutionCtx, SubjectOutcome, resolve_subject_outcome};
use crate::symbol_map::SymbolKind;
use crate::types::{AccessKind, ClassInfo, ClassLikeKind};
use crate::virtual_members::resolve_class_fully_cached;

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

/// Result of checking whether a member exists on resolved classes.
///
/// Returned by [`Backend::check_member_on_resolved_classes`] to tell
/// the caller whether a diagnostic was emitted and whether the chain
/// should be considered broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberCheckResult {
    /// No issue — member exists or access is fully suppressed (e.g. `__get`).
    Ok,
    /// Diagnostic emitted; the chain is broken because the type
    /// cannot be recovered.
    Break,
    /// Diagnostic emitted; a magic method (`__call` / `__callStatic`)
    /// can recover the return type so the chain continues resolving.
    MagicFallback,
}

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
    /// The byte offset of the member-access span for variable subjects
    /// (excluding `$this`), or `0` for non-variable subjects.  This
    /// ensures that accesses inside expression-level narrowing contexts
    /// (ternary branches, inline `&&` chains) get independent cache
    /// entries even when they share the same block-level
    /// `narrowing_offset`.  Without this, a prior access outside the
    /// ternary caches the un-narrowed type and the ternary-narrowed
    /// access reuses that stale result.
    access_offset: u32,
}

/// Per-pass cache mapping subject keys to their resolution outcomes.
type SubjectCache = HashMap<SubjectCacheKey, SubjectOutcome>;

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
        // (resolve_variable_types, resolve_variable_type, etc.)
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

        // ── Chain error propagation ─────────────────────────────────────
        // When a member access is flagged as broken, subsequent links
        // in the same fluent chain are suppressed because their failure
        // is a direct consequence of the first break.  We record
        // "broken chain prefixes" and skip any span whose subject_text
        // starts with one of them.
        let mut broken_chain_prefixes: Vec<String> = Vec::new();

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
            // after a reassignment get separate cache entries.
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

            // Use the innermost narrowing block (if/elseif/else body)
            // as a cache discriminator so that accesses in different
            // instanceof-narrowing contexts get independent entries.
            // Accesses in the same block share a cache entry because
            // they receive identical narrowing.
            //
            // This applies to regular variables ($var) AND property
            // chains on $this ($this->prop), because instanceof
            // checks and assert() calls can narrow property types
            // just like local variables.  Bare $this is excluded
            // because its type never changes within a method.
            let needs_narrowing_discriminator = subject_text.starts_with('$')
                && (subject_text.starts_with("$this->") || !subject_text.starts_with("$this"));
            let narrowing_offset = if needs_narrowing_discriminator {
                symbol_map.find_narrowing_block(span.start)
            } else {
                0
            };

            // Also check whether an `assert($var instanceof …)`
            // precedes this access.  Assert-instanceof does not
            // create a block scope, so without this discriminator
            // accesses before and after the assert would share the
            // same (stale) cache entry.
            let assert_offset = if needs_narrowing_discriminator {
                symbol_map.find_preceding_assert_offset(span.start)
            } else {
                0
            };

            // Use the access span offset as a cache discriminator so
            // that expression-level narrowing (ternary branches,
            // inline && chains) produces independent entries.
            // Block-level narrowing (if/else) is already covered by
            // `narrowing_offset`, but ternary expressions are
            // standalone statements — they don't create a narrowing
            // block.  Using the access offset ensures each member
            // access is resolved at its own cursor position, which is
            // what the unified resolution pipeline expects.
            //
            // This only applies to local variables ($var), NOT to
            // $this->prop chains.  Property accesses on $this are
            // already discriminated by narrowing_offset and
            // assert_offset (which cover if/else and assert()
            // narrowing).  Adding per-access offsets for $this->prop
            // would bust the cache on every distinct source location,
            // turning O(1) cached lookups into O(N) full resolutions
            // and causing pathological slowdowns on large files.
            let needs_access_offset =
                subject_text.starts_with('$') && !subject_text.starts_with("$this");
            let access_offset = if needs_access_offset { span.start } else { 0 };

            let cache_key = SubjectCacheKey {
                subject_text: subject_text.clone(),
                access_kind,
                scope: scope_key_for(current_class, fn_scope_start),
                var_def_offset,
                narrowing_offset,
                assert_offset,
                access_offset,
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
                    resolve_subject_outcome(subject_text, access_kind, &rctx)
                })
                .clone();

            // ── Chain error propagation: suppress downstream links ──────
            // If the subject of this access is downstream of an
            // already-flagged broken chain, skip it entirely.  The
            // original broken prefix propagates to all further links
            // naturally (it is a prefix of every subsequent subject).
            if is_downstream_of_broken_chain(subject_text, &broken_chain_prefixes) {
                continue;
            }

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
                    broken_chain_prefixes.push(broken_chain_prefix(
                        subject_text,
                        member_name,
                        is_static,
                        is_method_call,
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
                    broken_chain_prefixes.push(broken_chain_prefix(
                        subject_text,
                        member_name,
                        is_static,
                        is_method_call,
                    ));
                }

                SubjectOutcome::Untyped => {
                    // When the opt-in `unresolved-member-access` diagnostic
                    // is enabled, report every member access where the
                    // subject type could not be resolved — regardless of
                    // whether the subject is a bare variable, a chain, an
                    // array access, or a function call result.
                    if self.config().diagnostics.unresolved_member_access_enabled() {
                        let range = match offset_range_to_lsp_range(
                            content,
                            span.start as usize,
                            span.end as usize,
                        ) {
                            Some(r) => r,
                            None => continue,
                        };
                        let subject_display = subject_text.trim();
                        let kind_label = if is_method_call { "method" } else { "property" };
                        let message = format!(
                            "Cannot verify {} '{}' — type of '{}' could not be resolved",
                            kind_label, member_name, subject_display,
                        );
                        out.push(make_diagnostic(
                            range,
                            DiagnosticSeverity::HINT,
                            UNRESOLVED_MEMBER_ACCESS_CODE,
                            message,
                        ));
                        broken_chain_prefixes.push(broken_chain_prefix(
                            subject_text,
                            member_name,
                            is_static,
                            is_method_call,
                        ));
                    }
                }

                SubjectOutcome::Resolved(ref base_classes) => {
                    let result = self.check_member_on_resolved_classes(
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
                    // Only break the chain when the member is truly
                    // missing (no magic method fallback).  When
                    // `__call`/`__callStatic` exists, the diagnostic
                    // is emitted but the chain continues because the
                    // magic method's return type recovers the type.
                    if result == MemberCheckResult::Break {
                        broken_chain_prefixes.push(broken_chain_prefix(
                            subject_text,
                            member_name,
                            is_static,
                            is_method_call,
                        ));
                    }
                }
            }
        }
    }

    /// Check whether a member exists on the resolved classes and emit
    /// a diagnostic if it does not.
    ///
    /// Returns `true` if a diagnostic was emitted (the member was not
    /// found), `false` otherwise.
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
    ) -> MemberCheckResult {
        // ── Quick check on pre-resolved base classes ────────────────
        // `resolve_target_classes` already returns fully-resolved
        // classes in many code paths (e.g. `type_hint_to_classes_typed`
        // calls `resolve_class_fully` and injects model-specific
        // scope methods onto Eloquent Builders).  Check the member
        // on these classes FIRST, before re-resolving through the
        // cache.  The cache is keyed by bare FQN and may hold a
        // stale entry that lacks context-specific virtual members
        // (e.g. Builder scope methods that depend on the concrete
        // model type).  Checking here avoids false positives when
        // the cache and the resolver disagree.

        // ── Suppress property access on __get classes ───────────────
        // `__get` handles arbitrary property names.  Unlike __call,
        // we suppress the diagnostic entirely because there is no
        // meaningful return-type recovery to perform.
        if !is_method_call
            && base_classes
                .iter()
                .any(|c| has_magic_method_for_access(c, is_static, false))
        {
            return MemberCheckResult::Ok;
        }
        if base_classes.iter().any(|c| c.name == "stdClass") {
            return MemberCheckResult::Ok;
        }
        if base_classes.iter().any(|c| {
            member_exists(c, member_name, is_static, is_method_call)
                || (is_docblock_ref && member_exists_relaxed(c, member_name, is_method_call))
        }) {
            return MemberCheckResult::Ok;
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

        // ── Suppress property access on __get classes (resolved) ────
        if !is_method_call
            && resolved_classes
                .iter()
                .any(|c| has_magic_method_for_access(c, is_static, false))
        {
            return MemberCheckResult::Ok;
        }

        // ── Skip stdClass (universal object container) ──────────────
        if resolved_classes.iter().any(|c| c.name == "stdClass") {
            return MemberCheckResult::Ok;
        }

        // ── Check whether the member exists on ANY branch ───────────
        if resolved_classes.iter().any(|c| {
            member_exists(c, member_name, is_static, is_method_call)
                || (is_docblock_ref && member_exists_relaxed(c, member_name, is_method_call))
        }) {
            return MemberCheckResult::Ok;
        }

        // ── Check for __call / __callStatic on ANY branch ───────────
        // When any branch has a magic call handler, the method IS
        // dispatched at runtime (no fatal error), but it is still
        // "unknown" in the sense that it has no explicit declaration.
        // We emit the diagnostic so the user knows, but we return
        // `MagicFallback` so the chain is NOT broken — the return
        // type of `__call`/`__callStatic` recovers the chain type.
        let has_magic_call = is_method_call
            && (base_classes
                .iter()
                .any(|c| has_magic_method_for_access(c, is_static, true))
                || resolved_classes
                    .iter()
                    .any(|c| has_magic_method_for_access(c, is_static, true)));

        // ── Member is unresolved on ALL branches — emit diagnostic ──
        let range = match offset_range_to_lsp_range(content, start as usize, end as usize) {
            Some(r) => r,
            None => return MemberCheckResult::Ok,
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

        if has_magic_call {
            MemberCheckResult::MagicFallback
        } else {
            MemberCheckResult::Break
        }
    }
}

// ─── Chain error propagation ────────────────────────────────────────────────

/// Build the "broken chain prefix" for a flagged member access.
///
/// When a member access is flagged as broken (unknown member, scalar access,
/// etc.), downstream links in the same chain should be suppressed because
/// their failure is a consequence of the first break.
///
/// The prefix is constructed so that downstream `subject_text` values
/// (produced by `expr_to_subject_text`) will start with this prefix.
///
/// For method calls the prefix ends with `(` — this prevents ambiguity
/// with similarly-named methods (e.g. `callHome(` vs `callHomeLate(`).
/// For property accesses the prefix is the bare expression; callers use
/// [`is_downstream_of_broken_chain`] which checks for a chain-operator
/// boundary after the prefix to avoid false matches with longer property
/// names (e.g. `value` vs `value_extra`).
fn broken_chain_prefix(
    subject_text: &str,
    member_name: &str,
    is_static: bool,
    is_method_call: bool,
) -> String {
    let normalized = subject_text.replace("?->", "->");
    let operator = if is_static { "::" } else { "->" };
    if is_method_call {
        // Trailing `(` ensures "callHome(" does not match "callHomeLate(".
        format!("{}{}{}{}", normalized, operator, member_name, "(")
    } else {
        format!("{}{}{}", normalized, operator, member_name)
    }
}

/// Check whether `subject_text` is downstream of any previously flagged
/// broken chain expression.
///
/// Normalises null-safe operators (`?->` → `->`) so that chains mixing
/// `->` and `?->` are handled correctly.
fn is_downstream_of_broken_chain(subject_text: &str, broken_prefixes: &[String]) -> bool {
    if broken_prefixes.is_empty() {
        return false;
    }
    let normalized = subject_text.replace("?->", "->");
    broken_prefixes.iter().any(|prefix| {
        if prefix.ends_with('(') {
            // Method-call prefix: `starts_with` is sufficient because
            // the trailing `(` prevents name-prefix ambiguity.
            normalized.starts_with(prefix.as_str())
        } else {
            // Property prefix: the subject must equal the prefix or
            // the prefix must be followed by a chain operator to avoid
            // matching longer property names (e.g. `value` matching
            // `value_extra`).
            if normalized == *prefix {
                return true;
            }
            if !normalized.starts_with(prefix.as_str()) {
                return false;
            }
            let rest = &normalized[prefix.len()..];
            rest.starts_with("->") || rest.starts_with("::") || rest.starts_with('[')
        }
    })
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

fn display_class_name(class: &ClassInfo) -> String {
    if class.name.starts_with("__anonymous@") {
        return "anonymous class".to_string();
    }

    // Show the FQN when available for clarity.
    class.fqn()
}

#[cfg(test)]
mod tests;
