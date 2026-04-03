/// Type resolution for completion subjects.
///
/// This module contains the core entry points for resolving a completion
/// subject (e.g. `$this`, `self`, `static`, `$var`, `$this->prop`,
/// `ClassName`) to a concrete `ClassInfo` so that the correct completion
/// items can be offered.
///
/// The resolution logic is split across several sibling modules:
///
/// - [`super::call_resolution`]: Call expression and callable target
///   resolution (method calls, static calls, function calls, constructor
///   calls, signature help, named-argument completion).
/// - [`super::type_resolution`]: Type-hint string to `ClassInfo` mapping
///   (unions, intersections, generics, type aliases, object shapes).
/// - [`super::source_helpers`]: Source-text scanning helpers (closure return
///   types, first-class callable resolution, `new` expression parsing,
///   array access segment walking).
/// - [`super::variable_resolution`]: Variable type resolution via
///   assignment scanning and parameter type hints.
/// - [`super::type_narrowing`]: instanceof / assert / custom type guard
///   narrowing.
/// - [`super::closure_resolution`]: Closure and arrow-function parameter
///   resolution.
/// - [`crate::inheritance`]: Class inheritance merging (traits, mixins,
///   parent chain).
/// - [`super::conditional_resolution`]: PHPStan conditional return type
///   resolution at call sites.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::Backend;
use crate::docblock;
use crate::inheritance::resolve_property_type_hint;
use crate::php_type::PhpType;
use crate::subject_expr::SubjectExpr;
use crate::types::*;
use crate::util::find_class_by_name;
use crate::virtual_members::resolve_class_fully_maybe_cached;

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

/// Type alias for the optional constant-value-loader closure passed
/// through the resolution chain.  Given a constant name, returns
/// `Some(Some(value))` when the constant exists with a known value,
/// `Some(None)` when it exists but the value is unknown, and `None`
/// when the constant was not found.
pub(crate) type ConstantLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<Option<String>>>;

/// Bundles optional cross-file loader callbacks so they can be threaded
/// through the resolution chain as a single argument instead of one
/// parameter per loader.
#[derive(Clone, Copy, Default)]
pub(crate) struct Loaders<'a> {
    /// Cross-file function resolution callback (optional).
    pub function_loader: FunctionLoaderFn<'a>,
    /// Cross-file constant value resolution callback (optional).
    ///
    /// Given a global constant name (e.g. `"PHP_EOL"`), returns the
    /// constant's value string so that the type can be inferred from
    /// the literal value.
    pub constant_loader: ConstantLoaderFn<'a>,
}

impl<'a> Loaders<'a> {
    /// Create a `Loaders` with only a function loader.
    pub fn with_function(fl: FunctionLoaderFn<'a>) -> Self {
        Self {
            function_loader: fl,
            constant_loader: None,
        }
    }
}

/// Bundles the context needed by [`resolve_target_classes`] and
/// the functions it delegates to.
///
/// Introduced to replace the 8-parameter signature of
/// `resolve_target_classes` with a cleaner `(subject, access_kind, ctx)`
/// triple.  Also used directly by `resolve_call_return_types_expr` and
/// `resolve_arg_text_to_type` (formerly `CallResolutionCtx`).
pub(crate) struct ResolutionCtx<'a> {
    /// The class the cursor is inside, if any.
    pub current_class: Option<&'a ClassInfo>,
    /// All classes known in the current file.
    pub all_classes: &'a [Arc<ClassInfo>],
    /// The full source text of the current file.
    pub content: &'a str,
    /// Byte offset of the cursor in `content`.
    pub cursor_offset: u32,
    /// Cross-file class resolution callback.
    pub class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    /// Shared cache of fully-resolved classes, keyed by FQN.
    ///
    /// When `Some`, [`resolve_class_fully_cached`](crate::virtual_members::resolve_class_fully_cached)
    /// is used instead of the uncached variant, eliminating redundant
    /// full-resolution work within a single request cycle.  `None` in
    /// contexts where no `Backend` (and therefore no cache) is available
    /// (e.g. standalone free-function callers, some test helpers).
    pub resolved_class_cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
    /// Cross-file function resolution callback (optional).
    pub function_loader: FunctionLoaderFn<'a>,
}

/// Bundles the common parameters threaded through variable-type resolution.
///
/// Introducing this struct avoids passing 7–10 individual arguments to
/// every helper in the resolution chain, which keeps clippy happy and
/// makes call-sites much easier to read.
pub(super) struct VarResolutionCtx<'a> {
    pub var_name: &'a str,
    pub current_class: &'a ClassInfo,
    pub all_classes: &'a [Arc<ClassInfo>],
    pub content: &'a str,
    pub cursor_offset: u32,
    pub class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    /// Cross-file loader callbacks (function loader, constant loader).
    pub loaders: Loaders<'a>,
    /// Shared cache of fully-resolved classes, keyed by FQN.
    ///
    /// See [`ResolutionCtx::resolved_class_cache`] for details.
    pub resolved_class_cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
    /// The `@return` type annotation of the enclosing function/method,
    /// if known.  Used inside generator bodies to reverse-infer variable
    /// types from `Generator<TKey, TValue, TSend, TReturn>`.
    pub enclosing_return_type: Option<String>,
    /// When `true`, if/else/elseif walking only considers the branch
    /// that contains the cursor instead of unioning all branches.
    /// This produces the single type visible at the cursor position,
    /// which is what hover needs (e.g. only `Lamp` inside an if-branch,
    /// not `Lamp|Faucet`).  Completion leaves this `false` so that all
    /// possible types are offered.
    pub branch_aware: bool,
}

impl<'a> VarResolutionCtx<'a> {
    /// Create a [`ResolutionCtx`] from this variable resolution context.
    ///
    /// The non-optional `current_class` is wrapped in `Some(…)`.
    pub(crate) fn as_resolution_ctx(&self) -> ResolutionCtx<'a> {
        ResolutionCtx {
            current_class: Some(self.current_class),
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset: self.cursor_offset,
            class_loader: self.class_loader,
            function_loader: self.loaders.function_loader,
            resolved_class_cache: self.resolved_class_cache,
        }
    }

    /// Convenience accessor for the function loader.
    pub fn function_loader(&self) -> FunctionLoaderFn<'a> {
        self.loaders.function_loader
    }

    /// Convenience accessor for the constant loader.
    pub fn constant_loader(&self) -> ConstantLoaderFn<'a> {
        self.loaders.constant_loader
    }

    /// Clone this context with a different `enclosing_return_type`.
    ///
    /// All other fields are copied by reference.  This is useful when
    /// descending into a nested function/method body whose `@return`
    /// annotation differs from the outer scope.
    pub(super) fn with_enclosing_return_type(
        &self,
        enclosing_return_type: Option<String>,
    ) -> VarResolutionCtx<'a> {
        VarResolutionCtx {
            var_name: self.var_name,
            current_class: self.current_class,
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset: self.cursor_offset,
            class_loader: self.class_loader,
            loaders: self.loaders,
            resolved_class_cache: self.resolved_class_cache,
            enclosing_return_type,
            branch_aware: self.branch_aware,
        }
    }

    /// Clone this context with a different `cursor_offset`.
    ///
    /// All other fields (including `enclosing_return_type`) are preserved.
    /// This is useful when resolving a right-hand-side expression at a
    /// position earlier than the original cursor to avoid infinite
    /// recursion on self-referential assignments.
    pub(super) fn with_cursor_offset(&self, cursor_offset: u32) -> VarResolutionCtx<'a> {
        VarResolutionCtx {
            var_name: self.var_name,
            current_class: self.current_class,
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset,
            class_loader: self.class_loader,
            loaders: self.loaders,
            resolved_class_cache: self.resolved_class_cache,
            enclosing_return_type: self.enclosing_return_type.clone(),
            branch_aware: self.branch_aware,
        }
    }
}

/// Thread-local cache for `resolve_target_classes` results.
/// Active during a diagnostic pass so that multiple collectors
/// (unknown_member, argument_count) share results instead of
/// re-resolving the same subjects independently.
///
/// The cache key is `(subject_text, access_kind, scope_start,
/// var_def_offset)` where `scope_start` is the byte offset of the
/// innermost enclosing function/method/closure body and
/// `var_def_offset` is the `effective_from` of the active variable
/// definition (or `0` for non-variable subjects).  This ensures that
/// two methods in the same class that both use `$order->` get
/// independent cache entries, and that accesses before vs. after a
/// variable reassignment within the same method also get independent
/// entries.
///
/// Scope boundaries and variable definitions are stored alongside the
/// cache and set by [`set_diagnostic_subject_cache_scopes`].
type DiagSubjectCache = HashMap<(String, AccessKind, u32, u32, u32, u32), Vec<Arc<ClassInfo>>>;

/// File-level data stored alongside the diagnostic subject cache so
/// that [`resolve_target_classes`] can compute the enclosing scope and
/// active variable definition from the `cursor_offset` without needing
/// a reference to the [`SymbolMap`].
struct DiagSubjectCacheFileData {
    /// Scope boundaries `(start_offset, end_offset)`.
    scopes: Vec<(u32, u32)>,
    /// Variable definition sites, cloned from the [`SymbolMap`].
    var_defs: Vec<crate::symbol_map::VarDefSite>,
    /// Narrowing block boundaries `(start_offset, end_offset)` for
    /// if-body, elseif-body, else-body, match-arm, and switch-case
    /// blocks.  Used to compute the innermost narrowing context for
    /// a given cursor offset so that accesses in the same block share
    /// a cache entry while accesses in different branches do not.
    narrowing_blocks: Vec<(u32, u32)>,
    /// Sorted offsets of `assert($var instanceof …)` statements.
    /// Used as sequential narrowing boundaries so that accesses
    /// before and after an assert get separate cache entries.
    assert_narrowing_offsets: Vec<u32>,
}

type DiagSubjectCacheState = (DiagSubjectCache, DiagSubjectCacheFileData);

thread_local! {
    static DIAG_SUBJECT_CACHE: RefCell<Option<DiagSubjectCacheState>> = const { RefCell::new(None) };
}

/// Guard that owns the diagnostic subject cache lifetime.
/// Created by [`with_diagnostic_subject_cache`].
pub(crate) struct DiagSubjectCacheGuard {
    owns_cache: bool,
}

impl Drop for DiagSubjectCacheGuard {
    fn drop(&mut self) {
        if self.owns_cache {
            DIAG_SUBJECT_CACHE.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
}

/// Activate the diagnostic subject cache for the current thread.
///
/// While the returned guard is alive, `resolve_target_classes` will
/// check and populate the cache.  Nested calls return a no-op guard.
///
/// After calling this, use [`set_diagnostic_subject_cache_scopes`] to
/// provide the scope boundaries for the file being diagnosed so that
/// the cache can distinguish variables in different methods.
pub(crate) fn with_diagnostic_subject_cache() -> DiagSubjectCacheGuard {
    let already_active = DIAG_SUBJECT_CACHE.with(|cell| cell.borrow().is_some());
    if already_active {
        return DiagSubjectCacheGuard { owns_cache: false };
    }
    DIAG_SUBJECT_CACHE.with(|cell| {
        *cell.borrow_mut() = Some((
            HashMap::new(),
            DiagSubjectCacheFileData {
                scopes: Vec::new(),
                var_defs: Vec::new(),
                narrowing_blocks: Vec::new(),
                assert_narrowing_offsets: Vec::new(),
            },
        ));
    });
    DiagSubjectCacheGuard { owns_cache: true }
}

/// Provide scope boundaries and variable definitions for the active
/// diagnostic subject cache.
///
/// Must be called while a [`DiagSubjectCacheGuard`] is alive.  The
/// scopes are `(start_offset, end_offset)` pairs for every
/// function, method, closure, and arrow function body in the file.
/// They are used to compute the enclosing scope for each
/// `cursor_offset`, ensuring that same-named variables in different
/// methods resolve independently.
///
/// The `var_defs` are cloned from the [`SymbolMap`] and used to
/// compute the active variable definition at each cursor offset,
/// ensuring that accesses before and after a variable reassignment
/// within the same method get independent cache entries.
///
/// The `narrowing_blocks` are `(start, end)` pairs for every
/// if-body, elseif-body, else-body, match-arm, and switch-case block
/// in the file.  They determine the innermost narrowing context for
/// each cursor offset so that accesses in the same block share a
/// cache entry while accesses in different instanceof-narrowing
/// branches get independent entries.
pub(crate) fn set_diagnostic_subject_cache_scopes(
    scopes: Vec<(u32, u32)>,
    var_defs: Vec<crate::symbol_map::VarDefSite>,
    narrowing_blocks: Vec<(u32, u32)>,
    assert_narrowing_offsets: Vec<u32>,
) {
    DIAG_SUBJECT_CACHE.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some((_map, file_data)) = borrow.as_mut() {
            file_data.scopes = scopes;
            file_data.var_defs = var_defs;
            file_data.narrowing_blocks = narrowing_blocks;
            file_data.assert_narrowing_offsets = assert_narrowing_offsets;
        }
    });
}

/// Find the enclosing scope start offset for a given cursor position
/// using the scope boundaries stored in the diagnostic subject cache.
///
/// Returns `0` when no scope contains the offset (top-level code) or
/// when the cache is not active.
fn diag_cache_enclosing_scope(cursor_offset: u32) -> u32 {
    DIAG_SUBJECT_CACHE.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some((_map, file_data)) => {
                let mut best: u32 = 0;
                for &(start, end) in &file_data.scopes {
                    if start <= cursor_offset && cursor_offset <= end && start > best {
                        best = start;
                    }
                }
                best
            }
            None => 0,
        }
    })
}

/// Find the innermost narrowing block (if/elseif/else body, match arm,
/// switch case) that contains the cursor offset, using the narrowing
/// block boundaries stored in the diagnostic subject cache.
///
/// Returns the block's start offset, or `0` when the offset is not
/// inside any narrowing block or when the cache is not active.  Two
/// variable accesses that return the same value will have identical
/// instanceof narrowing applied and can safely share a cache entry.
fn diag_cache_narrowing_block(cursor_offset: u32) -> u32 {
    DIAG_SUBJECT_CACHE.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some((_map, file_data)) => {
                let mut best: u32 = 0;
                for &(start, end) in &file_data.narrowing_blocks {
                    if start <= cursor_offset && cursor_offset <= end && start > best {
                        best = start;
                    }
                }
                best
            }
            None => 0,
        }
    })
}

/// Find the offset of the most recent `assert($var instanceof …)`
/// statement preceding `cursor_offset`, or `0` if there is none.
///
/// Used as a cache discriminator so that accesses before and after an
/// assert-instanceof in the same flat statement list get separate
/// cache entries.
fn diag_cache_assert_offset(cursor_offset: u32) -> u32 {
    DIAG_SUBJECT_CACHE.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some((_map, file_data)) => {
                match file_data
                    .assert_narrowing_offsets
                    .partition_point(|&o| o < cursor_offset)
                {
                    0 => 0,
                    i => file_data.assert_narrowing_offsets[i - 1],
                }
            }
            None => 0,
        }
    })
}

/// Compute the `var_def_offset` discriminator for a subject at a given
/// cursor offset.
///
/// For variable-based subjects (starting with `$`, excluding `$this`),
/// returns the `effective_from` offset of the most recent variable
/// definition visible at `cursor_offset`.  For non-variable subjects,
/// returns `0`.
///
/// This ensures that the diagnostic subject cache distinguishes
/// accesses to the same variable before and after a reassignment.
fn diag_cache_var_def_offset(subject: &str, cursor_offset: u32) -> u32 {
    if !subject.starts_with('$') || subject.starts_with("$this") {
        return 0;
    }
    // Extract the bare variable name without '$' (e.g. "file" from
    // "$file" or "$file->foo()").
    let after_dollar = &subject[1..];
    let var_name = after_dollar
        .find("->")
        .map(|i| &after_dollar[..i])
        .unwrap_or(after_dollar);

    DIAG_SUBJECT_CACHE.with(|cell| {
        let borrow = cell.borrow();
        match borrow.as_ref() {
            Some((_map, file_data)) => {
                let scope_start = {
                    let mut best: u32 = 0;
                    for &(start, end) in &file_data.scopes {
                        if start <= cursor_offset && cursor_offset <= end && start > best {
                            best = start;
                        }
                    }
                    best
                };
                file_data
                    .var_defs
                    .iter()
                    .rev()
                    .find(|d| {
                        d.name == var_name
                            && d.scope_start == scope_start
                            && d.effective_from <= cursor_offset
                    })
                    .map(|d| d.effective_from)
                    .unwrap_or(0)
            }
            None => 0,
        }
    })
}

/// Resolve a completion subject to all candidate class types.
///
/// When a variable is assigned different types in conditional branches
/// (e.g. an `if` block reassigns `$thing`), this returns every possible
/// type so the caller can try each one when looking up members.
///
/// Internally parses the subject string into a [`SubjectExpr`] and
/// dispatches via `match` for exhaustive, type-safe routing.
///
/// When a [`DiagSubjectCacheGuard`] is active on the current thread,
/// results are cached by `(subject_text, access_kind, scope_start)`
/// so that multiple diagnostic collectors sharing the same file avoid
/// redundant resolution work while keeping different method scopes
/// independent.
pub(crate) fn resolve_target_classes(
    subject: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Vec<Arc<ClassInfo>> {
    // ── Fast path: check the thread-local diagnostic cache ──────
    let scope_start = diag_cache_enclosing_scope(ctx.cursor_offset);
    let var_def_offset = diag_cache_var_def_offset(subject, ctx.cursor_offset);
    // For variable subjects (excluding $this), use the innermost
    // narrowing block (if/elseif/else body) as a cache discriminator
    // so that accesses inside different instanceof-narrowing contexts
    // get independent cache entries.  Accesses in the same block
    // share a cache entry because they receive identical narrowing.
    let narrowing_offset = if subject.starts_with('$') && !subject.starts_with("$this") {
        diag_cache_narrowing_block(ctx.cursor_offset)
    } else {
        0
    };
    let assert_offset = if subject.starts_with('$') && !subject.starts_with("$this") {
        diag_cache_assert_offset(ctx.cursor_offset)
    } else {
        0
    };
    let cache_key = (
        subject.to_string(),
        access_kind,
        scope_start,
        var_def_offset,
        narrowing_offset,
        assert_offset,
    );
    let cached = DIAG_SUBJECT_CACHE.with(|cell| {
        let borrow = cell.borrow();
        borrow
            .as_ref()
            .and_then(|(map, _)| map.get(&cache_key).cloned())
    });
    if let Some(result) = cached {
        return result;
    }

    let expr = SubjectExpr::parse(subject);
    let result = resolve_target_classes_expr(&expr, access_kind, ctx);

    // ── Populate the cache if active ────────────────────────────
    // Skip caching empty results when the variable resolution depth
    // guard has fired.  A depth-limited empty result does not mean
    // the variable is genuinely unresolvable — it only means the
    // recursion was too deep *this time*.  Caching such an empty
    // vec would poison the cache: a later top-level lookup (at
    // depth 0) would hit the cached empty entry and produce a
    // false "type could not be resolved" diagnostic.
    let skip_cache =
        result.is_empty() && super::variable::resolution::is_var_resolution_depth_limited();
    if !skip_cache {
        DIAG_SUBJECT_CACHE.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if let Some((map, _)) = borrow.as_mut() {
                map.insert(cache_key, result.clone());
            }
        });
    }

    result
}

/// Core dispatch for [`resolve_target_classes`], operating on a
/// pre-parsed [`SubjectExpr`].
pub(crate) fn resolve_target_classes_expr(
    expr: &SubjectExpr,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Vec<Arc<ClassInfo>> {
    let current_class = ctx.current_class;
    let all_classes = ctx.all_classes;
    let class_loader = ctx.class_loader;

    match expr {
        // ── Keywords that always mean "current class" ────────────
        SubjectExpr::This => {
            // Check for `@param-closure-this` override: when the cursor
            // is inside a closure passed as an argument to a function
            // whose parameter carries `@param-closure-this`, resolve
            // `$this` to the declared type instead of the lexical class.
            if let Some(override_cls) =
                super::variable::closure_resolution::find_closure_this_override(ctx)
            {
                return vec![Arc::new(override_cls)];
            }
            current_class
                .map(|cc| Arc::new(cc.clone()))
                .into_iter()
                .collect()
        }
        SubjectExpr::SelfKw | SubjectExpr::StaticKw => current_class
            .map(|cc| Arc::new(cc.clone()))
            .into_iter()
            .collect(),

        // ── `parent::` — resolve to the current class's parent ──
        SubjectExpr::Parent => {
            if let Some(cc) = current_class
                && let Some(ref parent_name) = cc.parent_class
            {
                if let Some(cls) = find_class_by_name(all_classes, parent_name) {
                    return vec![Arc::clone(cls)];
                }
                return class_loader(parent_name).into_iter().collect();
            }
            vec![]
        }

        // ── Inline array literal with index access ──────────────
        SubjectExpr::InlineArray { elements, .. } => {
            let mut element_classes = Vec::new();
            for elem_text in elements {
                let elem = elem_text.trim();
                if elem.is_empty() {
                    continue;
                }
                let elem_expr = SubjectExpr::parse(elem);
                let resolved = resolve_target_classes_expr(&elem_expr, AccessKind::Arrow, ctx);
                ClassInfo::extend_unique_arc(&mut element_classes, resolved);
            }
            element_classes
        }

        // ── Enum case / static member access ────────────────────
        SubjectExpr::StaticAccess { class, member } => {
            // Handle self/static/parent keywords — SubjectExpr::parse
            // produces StaticAccess for "self::MONTH", "static::FOO",
            // etc., but "self"/"static"/"parent" are keywords, not
            // class names, so find_class_by_name / class_loader won't
            // find them.
            let owner_classes: Vec<Arc<ClassInfo>> = match class.as_str() {
                "self" | "static" => current_class
                    .map(|cc| Arc::new(cc.clone()))
                    .into_iter()
                    .collect(),
                "parent" => {
                    if let Some(cc) = current_class
                        && let Some(ref parent_name) = cc.parent_class
                    {
                        if let Some(cls) = find_class_by_name(all_classes, parent_name) {
                            vec![Arc::clone(cls)]
                        } else {
                            class_loader(parent_name).into_iter().collect()
                        }
                    } else {
                        vec![]
                    }
                }
                _ => {
                    if let Some(cls) = find_class_by_name(all_classes, class) {
                        vec![Arc::clone(cls)]
                    } else {
                        class_loader(class).into_iter().collect()
                    }
                }
            };

            // When the member is a static property (starts with `$`),
            // resolve to the property's declared type instead of the
            // owning class.  This makes `self::$instance->method()`
            // resolve `method()` on the property's type, not on the
            // class that declares the static property.
            if let Some(prop_name) = member.strip_prefix('$') {
                let mut results = Vec::new();
                for cls in &owner_classes {
                    let resolved = super::type_resolution::resolve_property_types(
                        prop_name,
                        cls,
                        all_classes,
                        class_loader,
                    );
                    ClassInfo::extend_unique_arc(
                        &mut results,
                        resolved.into_iter().map(Arc::new).collect(),
                    );
                }
                if !results.is_empty() {
                    return results;
                }
            }

            owner_classes
        }

        // ── Bare class name ─────────────────────────────────────
        SubjectExpr::ClassName(name) => {
            if let Some(cls) = find_class_by_name(all_classes, name) {
                return vec![Arc::clone(cls)];
            }
            class_loader(name).into_iter().collect()
        }

        // ── `new ClassName` (without trailing call parens) ───────
        SubjectExpr::NewExpr { class_name } => {
            if let Some(cls) = find_class_by_name(all_classes, class_name) {
                return vec![Arc::clone(cls)];
            }
            class_loader(class_name).into_iter().collect()
        }

        // ── Call expression ─────────────────────────────────────
        SubjectExpr::CallExpr { callee, args_text } => {
            Backend::resolve_call_return_types_expr(callee, args_text, ctx)
        }

        // ── Property chain ──────────────────────────────────────
        SubjectExpr::PropertyChain { base, property } => {
            let base_classes = resolve_target_classes_expr(base, access_kind, ctx);
            let mut results = Vec::new();
            for cls in &base_classes {
                let resolved = super::type_resolution::resolve_property_types(
                    property,
                    cls,
                    all_classes,
                    class_loader,
                );
                ClassInfo::extend_unique_arc(
                    &mut results,
                    resolved.into_iter().map(Arc::new).collect(),
                );
            }

            // ── Property-level narrowing ────────────────────────
            // When the property chain resolves to a union (or a
            // broad interface type), an enclosing `instanceof`
            // check like `if ($this->prop instanceof Foo)` should
            // narrow the result set, just as it does for plain
            // variables.  Build the full access path (e.g.
            // `$this->timeline`) and run the narrowing walk.
            //
            // This also handles untyped properties: when the
            // property has no type hint, `results` is empty but
            // an `instanceof` check or `assert()` can still
            // provide a type via `apply_instanceof_inclusion`.
            //
            // Use a dummy class when outside a class body so that
            // property narrowing works in standalone functions and
            // top-level code (e.g. `$arg->value instanceof Foo`
            // inside a foreach).
            {
                let dummy_class;
                let effective_class = match current_class {
                    Some(cc) => cc,
                    None => {
                        dummy_class = ClassInfo::default();
                        &dummy_class
                    }
                };
                let full_path = format!("{}->{}", base.to_subject_text(), property);
                apply_property_narrowing(&full_path, effective_class, ctx, &mut results);
            }

            results
        }

        // ── Array access on variable or call expression ─────────
        SubjectExpr::ArrayAccess { base, segments } => {
            // When the base is a call expression (e.g. `$c->items()[0]`),
            // resolve the call's raw return type and use it as a candidate
            // for array-segment walking.  This mirrors the variable path
            // but sources the raw type from the method/function signature
            // instead of from docblock annotations or assignments.
            if let SubjectExpr::CallExpr { callee, args_text } = base.as_ref() {
                let call_raw = resolve_call_raw_return_type(callee, args_text, ctx);
                if let Some(raw) = call_raw {
                    let candidates = std::iter::once(raw);
                    if let Some(resolved) =
                        super::source::helpers::try_chained_array_access_with_candidates(
                            candidates,
                            segments,
                            current_class,
                            all_classes,
                            class_loader,
                        )
                    {
                        return resolved.into_iter().map(Arc::new).collect();
                    }
                }
                // If raw-type approach didn't work, fall back to resolving
                // the call normally (handles cases like `getItems()[0]`
                // where the return type is already a class with ArrayAccess).
                return vec![];
            }

            let base_var = base.to_subject_text();

            // Build candidate raw types from multiple strategies.
            // Each is tried as a complete pipeline (raw type →
            // segment walk → ClassInfo); the first that succeeds
            // through all segments wins.

            // ── Property chain raw type ─────────────────────────
            // When the base is a property chain (e.g. `$this->cache`,
            // `$obj->items`), resolve the owning class and extract
            // the property's raw type hint.  This preserves generic
            // parameters like `array<string, IntCollection>` or
            // `Collection<int, Translation>` that would be lost if
            // we resolved through `type_hint_to_classes` first.
            let property_raw_type = if let SubjectExpr::PropertyChain {
                base: prop_base,
                property,
            } = base.as_ref()
            {
                let owner_classes = resolve_target_classes_expr(prop_base, access_kind, ctx);
                owner_classes.iter().find_map(|cls| {
                    crate::inheritance::resolve_property_type_hint(cls, property, class_loader)
                        .map(|ty| ty.to_string())
                })
            } else {
                None
            };

            let docblock_type = docblock::find_iterable_raw_type_in_source(
                ctx.content,
                ctx.cursor_offset as usize,
                &base_var,
            );
            let ast_type = {
                let dummy_class;
                let effective_class = match current_class {
                    Some(cc) => cc,
                    None => {
                        dummy_class = ClassInfo::default();
                        &dummy_class
                    }
                };
                let resolved = crate::completion::variable::resolution::resolve_variable_types(
                    &base_var,
                    effective_class,
                    all_classes,
                    ctx.content,
                    ctx.cursor_offset,
                    class_loader,
                    Loaders::with_function(ctx.function_loader),
                );
                if resolved.is_empty() {
                    None
                } else {
                    Some(ResolvedType::type_strings_joined(&resolved))
                }
            };

            let candidates = property_raw_type
                .into_iter()
                .chain(docblock_type)
                .chain(ast_type);

            if let Some(resolved) = super::source::helpers::try_chained_array_access_with_candidates(
                candidates,
                segments,
                current_class,
                all_classes,
                class_loader,
            ) {
                return resolved.into_iter().map(Arc::new).collect();
            }
            // Segment walk failed — the base type does not have
            // array-shape, generic, or iterable annotations that
            // cover bracket access.  Return empty: `$var['key']` is
            // never the same type as `$var`.
            vec![]
        }

        // ── Bare variable ───────────────────────────────────────
        SubjectExpr::Variable(var_name) => resolve_variable_fallback(var_name, access_kind, ctx),

        // ── Callee-only variants (MethodCall, StaticMethodCall,
        //    FunctionCall) should not appear as top-level subjects;
        //    they are wrapped in CallExpr.  If they do appear
        //    (e.g. from a partial parse), treat as class name. ────
        SubjectExpr::MethodCall { .. }
        | SubjectExpr::StaticMethodCall { .. }
        | SubjectExpr::FunctionCall(_) => {
            let text = expr.to_subject_text();
            if let Some(cls) = find_class_by_name(all_classes, &text) {
                return vec![Arc::clone(cls)];
            }
            class_loader(&text).into_iter().collect()
        }
    }
}

/// Extract the raw return type string from a call expression's callee.
///
/// Given a `CallExpr`'s callee and arguments, resolves the owning class
/// (for method/static-method calls) or the function info (for standalone
/// functions), finds the matching method/function, and returns its raw
/// return type string (e.g. `"Item[]"`).  This is used by the
/// `ArrayAccess` handler to strip array dimensions and resolve the
/// element type when the base of `[0]` is a call expression.
fn resolve_call_raw_return_type(
    callee: &SubjectExpr,
    _args_text: &str,
    ctx: &ResolutionCtx<'_>,
) -> Option<String> {
    match callee {
        SubjectExpr::MethodCall { base, method } => {
            let base_classes = resolve_target_classes_expr(base, AccessKind::Arrow, ctx);
            for cls in &base_classes {
                // Use a fully-resolved class so that inherited docblock
                // return types (e.g. `list<Pen>` from an interface or
                // parent) are visible instead of the bare native hint.
                let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                    cls,
                    ctx.class_loader,
                    ctx.resolved_class_cache,
                );
                let found = merged
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(method));
                if let Some(m) = found {
                    if let Some(ref ret) = m.return_type {
                        return Some(ret.to_string());
                    }
                    // Method exists but has no return type.
                    // Only fall through to __call for virtual methods
                    // (from @method tags or @mixin). Real methods are
                    // invoked directly at runtime, not through __call.
                    if !m.is_virtual {
                        continue;
                    }
                }
                // __call fallback: method not found, or virtual method
                // without a return type.
                if let Some(m) = merged
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case("__call"))
                    && let Some(ref ret) = m.return_type
                {
                    return Some(ret.to_string());
                }
                // __call fallback: when the method is not found but the
                // class defines __call, use its return type so that
                // chains through dynamic calls (e.g. Builder where{Column})
                // preserve the type.
                if let Some(m) = merged
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case("__call"))
                    && let Some(ref ret) = m.return_type
                {
                    return Some(ret.to_string());
                }
            }
            None
        }
        SubjectExpr::StaticMethodCall { class, method } => {
            let owner = resolve_static_owner_class(class, ctx);
            if let Some(ref cls) = owner {
                let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                    cls,
                    ctx.class_loader,
                    ctx.resolved_class_cache,
                );
                let found = merged
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(method));
                if let Some(m) = found {
                    if let Some(ref ret) = m.return_type {
                        return Some(ret.to_string());
                    }
                    // Method exists but has no return type.
                    // Only fall through to __callStatic for virtual methods.
                    if !m.is_virtual {
                        return None;
                    }
                }
                // __callStatic fallback: method not found, or virtual
                // method without a return type.
                if let Some(m) = merged
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case("__callStatic"))
                    && let Some(ref ret) = m.return_type
                {
                    return Some(ret.to_string());
                }
            }
            None
        }
        SubjectExpr::FunctionCall(fn_name) => {
            if let Some(fl) = ctx.function_loader
                && let Some(func_info) = fl(fn_name)
            {
                return func_info.return_type_str();
            }
            None
        }
        _ => None,
    }
}

// ─── Enriched subject resolution for diagnostics ────────────────────────────

/// The outcome of resolving a subject for diagnostic purposes.
///
/// [`resolve_target_classes`] only returns classes and silently drops
/// scalar types.  Diagnostics need to know *why* resolution returned
/// empty — was the subject a scalar type (runtime crash), an
/// unresolvable class name (likely typo / missing import), or truly
/// untyped?  This enum carries that information so the diagnostic
/// collector can emit the right message without re-running resolution.
#[derive(Clone, Debug)]
pub(crate) enum SubjectOutcome {
    /// Subject resolved to one or more classes.
    Resolved(Vec<Arc<ClassInfo>>),
    /// Subject resolved to a scalar type — member access is always a
    /// runtime crash.  The string is the display name of the scalar
    /// type (e.g. `"int"`, `"string"`, `"bool|int"`).
    Scalar(String),
    /// Subject resolved to a class name that couldn't be loaded.
    UnresolvableClass(String),
    /// Subject type could not be resolved — no class information
    /// available.
    Untyped,
}

/// Resolve a subject to a [`SubjectOutcome`] in a single pass.
///
/// This is the unified entry point for diagnostic subject resolution.
/// It first tries [`resolve_target_classes`] (the same pipeline used
/// by completion and hover).  When that returns empty, it inspects the
/// raw resolved types to determine whether the subject is scalar,
/// an unresolvable class name, or truly untyped — without re-running
/// variable resolution or calling separate secondary helpers.
pub(crate) fn resolve_subject_outcome(
    subject: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> SubjectOutcome {
    let classes = resolve_target_classes(subject, access_kind, ctx);
    if !classes.is_empty() {
        return SubjectOutcome::Resolved(classes);
    }

    // ── Subject did not resolve to any class — determine why ────
    let expr = SubjectExpr::parse(subject);
    resolve_subject_outcome_from_expr(&expr, access_kind, ctx)
}

/// Inner dispatch for [`resolve_subject_outcome`], operating on a
/// pre-parsed [`SubjectExpr`].
fn resolve_subject_outcome_from_expr(
    expr: &SubjectExpr,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> SubjectOutcome {
    match expr {
        // ── Bare variable ───────────────────────────────────────
        SubjectExpr::Variable(var_name) => resolve_subject_outcome_variable(var_name, ctx),

        // ── Property chain: $user->age->value ───────────────────
        SubjectExpr::PropertyChain { base, property } => {
            resolve_subject_outcome_property_chain(base, property, access_kind, ctx)
        }

        // ── Call expression ─────────────────────────────────────
        SubjectExpr::CallExpr { callee, args_text } => {
            resolve_subject_outcome_call_expr(callee, args_text, access_kind, ctx)
        }

        _ => SubjectOutcome::Untyped,
    }
}

/// Resolve a bare variable subject to a [`SubjectOutcome`].
///
/// Re-uses `resolve_variable_types` (the same function that
/// `resolve_variable_fallback` calls) to get the raw resolved types.
/// If they are all scalar, returns `Scalar`.  If the raw type string
/// looks like a class name that can't be loaded, returns
/// `UnresolvableClass`.  Otherwise returns `Untyped`.
fn resolve_subject_outcome_variable(var_name: &str, ctx: &ResolutionCtx<'_>) -> SubjectOutcome {
    let default_class = ClassInfo::default();
    let effective_class = ctx.current_class.unwrap_or(&default_class);

    let resolved = super::variable::resolution::resolve_variable_types(
        var_name,
        effective_class,
        ctx.all_classes,
        ctx.content,
        ctx.cursor_offset,
        ctx.class_loader,
        Loaders::with_function(ctx.function_loader),
    );

    if !resolved.is_empty() {
        let joined = ResolvedType::types_joined(&resolved);
        if joined.all_members_primitive_scalar() {
            let display = joined
                .non_null_type()
                .map_or_else(|| joined.to_string(), |t| t.to_string());
            return SubjectOutcome::Scalar(display);
        }
        // The resolved types contain non-scalar, non-class entries
        // (e.g. type aliases we can't resolve).  Check for
        // unresolvable class names.
        let raw_type = ResolvedType::type_strings_joined(&resolved);
        if let Some(unresolved) = check_unresolvable_class_name(&raw_type, ctx.class_loader) {
            return SubjectOutcome::UnresolvableClass(unresolved);
        }
        return SubjectOutcome::Untyped;
    }

    // Variable resolution returned nothing.  Fall back to the hover
    // variable type resolver which also checks class-based foreach
    // resolution through @implements / @extends generics.
    if let Some(raw_type) = crate::hover::variable_type::resolve_variable_type_string(
        var_name,
        ctx.content,
        ctx.cursor_offset,
        ctx.current_class,
        ctx.all_classes,
        ctx.class_loader,
        Loaders::with_function(ctx.function_loader),
    ) && let Some(unresolved) = check_unresolvable_class_name(&raw_type, ctx.class_loader)
    {
        return SubjectOutcome::UnresolvableClass(unresolved);
    }

    SubjectOutcome::Untyped
}

/// Resolve a property chain subject to a [`SubjectOutcome`].
///
/// Resolves the base to classes, then looks up the property's type
/// hint.  If the type is purely scalar, returns `Scalar`.
fn resolve_subject_outcome_property_chain(
    base: &SubjectExpr,
    property: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> SubjectOutcome {
    let base_classes = resolve_target_classes_expr(base, access_kind, ctx);
    for cls in &base_classes {
        let resolved =
            resolve_class_fully_maybe_cached(cls, ctx.class_loader, ctx.resolved_class_cache);
        if let Some(parsed) = resolve_property_type_hint(&resolved, property, ctx.class_loader) {
            if parsed.all_members_primitive_scalar() {
                let display = parsed
                    .non_null_type()
                    .map_or_else(|| parsed.to_string(), |t| t.to_string());
                return SubjectOutcome::Scalar(display);
            }
            // Non-scalar, non-class type — treat as unresolvable.
            return SubjectOutcome::Untyped;
        }
    }
    SubjectOutcome::Untyped
}

/// Resolve a call expression subject to a [`SubjectOutcome`].
///
/// First tries `resolve_call_return_types_expr` (the normal path).
/// When that returns empty, inspects the raw return type hint of the
/// callable — if it's scalar, returns `Scalar`.
fn resolve_subject_outcome_call_expr(
    callee: &SubjectExpr,
    args_text: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> SubjectOutcome {
    let return_classes = Backend::resolve_call_return_types_expr(callee, args_text, ctx);
    if !return_classes.is_empty() {
        // Shouldn't happen (resolve_target_classes would have returned
        // these), but handle gracefully.
        return SubjectOutcome::Resolved(return_classes);
    }

    // Try to get the raw return type hint from the callable.
    if let Some(scalar) = resolve_call_scalar_return(callee, access_kind, ctx) {
        return SubjectOutcome::Scalar(scalar);
    }

    // Try unresolvable class detection for function calls.
    if let SubjectExpr::FunctionCall(fn_name) = callee
        && let Some(fl) = ctx.function_loader
        && let Some(func_info) = fl(fn_name.as_str())
        && let Some(raw_type) = func_info.return_type_str()
        && let Some(unresolved) = check_unresolvable_class_name(&raw_type, ctx.class_loader)
    {
        return SubjectOutcome::UnresolvableClass(unresolved);
    }

    SubjectOutcome::Untyped
}

/// Check whether a call expression's return type is a scalar.
///
/// Inspects the raw return type hint on the method or function without
/// going through the full class resolution pipeline.
fn resolve_call_scalar_return(
    callee: &SubjectExpr,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Option<String> {
    match callee {
        // Instance method call: $obj->getAge()
        SubjectExpr::MethodCall { base, method } => {
            let base_classes = resolve_target_classes_expr(base, access_kind, ctx);
            for cls in &base_classes {
                let resolved = resolve_class_fully_maybe_cached(
                    cls,
                    ctx.class_loader,
                    ctx.resolved_class_cache,
                );
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
            None
        }
        // Standalone function call: getInt()
        SubjectExpr::FunctionCall(fn_name) => {
            if let Some(fl) = ctx.function_loader
                && let Some(func_info) = fl(fn_name)
                && let Some(ref hint) = func_info.return_type
                && hint.all_members_primitive_scalar()
            {
                let display = hint
                    .non_null_type()
                    .map_or_else(|| hint.to_string(), |t| t.to_string());
                return Some(display);
            }
            None
        }
        // Static method call: Foo::getInt()
        SubjectExpr::StaticMethodCall { class, method } => {
            let cls = (ctx.class_loader)(class);
            if let Some(cls) = cls {
                let resolved = resolve_class_fully_maybe_cached(
                    &cls,
                    ctx.class_loader,
                    ctx.resolved_class_cache,
                );
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
            None
        }
        _ => None,
    }
}

/// Check whether a raw type string refers to a class that cannot be
/// loaded.
///
/// Returns `Some(class_name)` when the type looks like a class name
/// (not scalar, not a PHPDoc pseudo-type) but the class loader cannot
/// find it.  Returns `None` for scalars, unions, shapes, and types
/// that resolve successfully.
fn check_unresolvable_class_name(
    raw_type: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    let parsed = PhpType::parse(raw_type);
    if parsed.all_members_scalar() {
        return None;
    }

    let effective = parsed.non_null_type().unwrap_or_else(|| parsed.clone());
    let base = effective.base_name()?;

    if class_loader(base).is_none() {
        Some(base.to_string())
    } else {
        None
    }
}

/// Shared variable-resolution logic extracted from the former
/// bare-`$var` branch of `resolve_target_classes`.
fn resolve_variable_fallback(
    var_name: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Vec<Arc<ClassInfo>> {
    let current_class = ctx.current_class;
    let all_classes = ctx.all_classes;
    let class_loader = ctx.class_loader;
    let function_loader = ctx.function_loader;

    let dummy_class;
    let effective_class = match current_class {
        Some(cc) => cc,
        None => {
            dummy_class = ClassInfo::default();
            &dummy_class
        }
    };

    // ── `$var::` where `$var` holds a class-string ──
    if access_kind == AccessKind::DoubleColon {
        let class_string_targets =
            crate::completion::variable::class_string_resolution::resolve_class_string_targets(
                var_name,
                effective_class,
                all_classes,
                ctx.content,
                ctx.cursor_offset,
                class_loader,
            );
        if !class_string_targets.is_empty() {
            return class_string_targets.into_iter().map(Arc::new).collect();
        }
    }

    let resolved_types = super::variable::resolution::resolve_variable_types(
        var_name,
        effective_class,
        all_classes,
        ctx.content,
        ctx.cursor_offset,
        class_loader,
        Loaders::with_function(function_loader),
    );

    // ── `class-string<T>` unwrapping for `$var::` access ────────
    // When the variable's type is `class-string<T>` (e.g. from a
    // `@param class-string<BackedEnum> $class` annotation) and the
    // access kind is `::`, unwrap the inner type `T` and resolve it
    // to classes so that static members are offered against `T`.
    if access_kind == AccessKind::DoubleColon {
        let mut class_string_results: Vec<Arc<ClassInfo>> = Vec::new();
        for rt in &resolved_types {
            let inner = match &rt.type_string {
                PhpType::ClassString(Some(inner)) => Some(inner.as_ref()),
                // Handle `?class-string<T>` — unwrap nullable first.
                PhpType::Nullable(inner) => match inner.as_ref() {
                    PhpType::ClassString(Some(cs_inner)) => Some(cs_inner.as_ref()),
                    _ => None,
                },
                // Handle union types containing class-string<T>.
                PhpType::Union(members) => {
                    for member in members {
                        let cs_inner = match member {
                            PhpType::ClassString(Some(inner)) => Some(inner.as_ref()),
                            PhpType::Nullable(inner) => match inner.as_ref() {
                                PhpType::ClassString(Some(cs_inner)) => Some(cs_inner.as_ref()),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(inner_ty) = cs_inner {
                            let resolved = super::type_resolution::type_hint_to_classes_typed(
                                inner_ty,
                                &effective_class.name,
                                all_classes,
                                class_loader,
                            );
                            for cls in resolved {
                                ClassInfo::push_unique_arc(
                                    &mut class_string_results,
                                    Arc::new(cls),
                                );
                            }
                        }
                    }
                    None // already handled inline
                }
                _ => None,
            };
            if let Some(inner_ty) = inner {
                let resolved = super::type_resolution::type_hint_to_classes_typed(
                    inner_ty,
                    &effective_class.name,
                    all_classes,
                    class_loader,
                );
                for cls in resolved {
                    ClassInfo::push_unique_arc(&mut class_string_results, Arc::new(cls));
                }
            }
        }
        if !class_string_results.is_empty() {
            return class_string_results;
        }
    }

    let result: Vec<Arc<ClassInfo>> = ResolvedType::into_classes(resolved_types)
        .into_iter()
        .map(Arc::new)
        .collect();

    result
}

// ── Static owner class resolution ───────────────────────────────────

/// Resolve a static class reference (`self`, `static`, `parent`, or a
/// class name) to its `ClassInfo`.
///
/// Handles the `self`/`static`/`parent` keywords and falls back to
/// `class_loader` then `resolve_target_classes` for named classes.
pub(in crate::completion) fn resolve_static_owner_class(
    class: &str,
    rctx: &ResolutionCtx<'_>,
) -> Option<Arc<ClassInfo>> {
    if class == "self" || class == "static" {
        rctx.current_class.map(|cc| Arc::new(cc.clone()))
    } else if class == "parent" {
        rctx.current_class
            .and_then(|cc| cc.parent_class.as_ref())
            .and_then(|p| (rctx.class_loader)(p))
    } else {
        find_class_by_name(rctx.all_classes, class)
            .map(Arc::clone)
            .or_else(|| (rctx.class_loader)(class))
            .or_else(|| {
                resolve_target_classes(class, crate::AccessKind::DoubleColon, rctx)
                    .into_iter()
                    .next()
            })
    }
}

/// Apply instanceof / assert narrowing for a property-access path.
///
/// This is the property-level analog of the narrowing that
/// [`super::variable::resolution::walk_statements_for_assignments`]
/// performs for plain variables.  It re-parses the source, locates
/// the enclosing method body, and walks its statements with a
/// [`VarResolutionCtx`] whose `var_name` is the full property path
/// (e.g. `$this->timeline`).  The existing narrowing functions in
/// [`super::types::narrowing`] already support property paths via
/// [`super::types::narrowing::expr_to_subject_key`], so no changes
/// to those functions are required.
fn apply_property_narrowing(
    property_path: &str,
    current_class: &ClassInfo,
    rctx: &ResolutionCtx<'_>,
    results: &mut Vec<Arc<ClassInfo>>,
) {
    use crate::parser::with_parsed_program;

    // The narrowing walk functions operate on Vec<ClassInfo>, so unwrap
    // the Arcs, run narrowing, then re-wrap.
    let mut plain: Vec<ClassInfo> = results.drain(..).map(Arc::unwrap_or_clone).collect();

    with_parsed_program(
        rctx.content,
        "apply_property_narrowing",
        |program, _content| {
            let ctx = VarResolutionCtx {
                var_name: property_path,
                current_class,
                all_classes: rctx.all_classes,
                content: rctx.content,
                cursor_offset: rctx.cursor_offset,
                class_loader: rctx.class_loader,
                loaders: Loaders::with_function(rctx.function_loader),
                resolved_class_cache: None,
                enclosing_return_type: None,
                branch_aware: false,
            };
            walk_property_narrowing_in_statements(program.statements.iter(), &ctx, &mut plain);
        },
    );

    *results = plain.into_iter().map(Arc::new).collect();
}

/// Walk top-level statements to find the class + method containing the
/// cursor, then apply narrowing to `results` for the given property path.
fn walk_property_narrowing_in_statements<'b>(
    statements: impl Iterator<Item = &'b mago_syntax::ast::Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_span::HasSpan;
    use mago_syntax::ast::*;

    for stmt in statements {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    walk_property_narrowing_in_members(class.members.iter(), ctx, results);
                    return;
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    walk_property_narrowing_in_members(trait_def.members.iter(), ctx, results);
                    return;
                }
            }
            Statement::Namespace(ns) => {
                let ns_span = ns.span();
                if ctx.cursor_offset >= ns_span.start.offset
                    && ctx.cursor_offset <= ns_span.end.offset
                {
                    walk_property_narrowing_in_statements(ns.statements().iter(), ctx, results);
                    return;
                }
            }
            Statement::Function(func) => {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                    walk_property_narrowing_stmts(func.body.statements.iter(), ctx, results);
                    return;
                }
            }
            // ── Functions inside if-guards / blocks ──
            // The common PHP pattern `if (! function_exists('foo'))
            // { function foo(…) { … } }` nests the function
            // declaration inside an if body.  Recurse into blocks
            // and if-bodies so property narrowing still works.
            Statement::If(if_stmt) => {
                let if_span = stmt.span();
                if ctx.cursor_offset >= if_span.start.offset
                    && ctx.cursor_offset <= if_span.end.offset
                {
                    for inner in if_stmt.body.statements().iter() {
                        walk_property_narrowing_in_statements(std::iter::once(inner), ctx, results);
                    }
                }
            }
            Statement::Block(block) => {
                let blk_span = stmt.span();
                if ctx.cursor_offset >= blk_span.start.offset
                    && ctx.cursor_offset <= blk_span.end.offset
                {
                    walk_property_narrowing_in_statements(block.statements.iter(), ctx, results);
                }
            }
            _ => {}
        }
    }
}

/// Walk class members to find the method containing the cursor, then
/// apply instanceof / guard-clause narrowing for the property path.
fn walk_property_narrowing_in_members<'b>(
    members: impl Iterator<Item = &'b mago_syntax::ast::class_like::member::ClassLikeMember<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_syntax::ast::class_like::member::ClassLikeMember;
    use mago_syntax::ast::class_like::method::MethodBody;

    for member in members {
        if let ClassLikeMember::Method(method) = member {
            let body = match &method.body {
                MethodBody::Concrete(block) => block,
                _ => continue,
            };
            let body_start = body.left_brace.start.offset;
            let body_end = body.right_brace.end.offset;
            if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                walk_property_narrowing_stmts(body.statements.iter(), ctx, results);
                return;
            }
        }
    }
}

/// Walk statements applying only narrowing (no assignment scanning)
/// for a property path like `$this->prop`.
fn walk_property_narrowing_stmts<'b>(
    statements: impl Iterator<Item = &'b mago_syntax::ast::Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_span::HasSpan;
    use mago_syntax::ast::*;

    use super::types::narrowing;

    for stmt in statements {
        let stmt_span = stmt.span();
        // Only consider statements whose start is before the cursor.
        if stmt_span.start.offset >= ctx.cursor_offset {
            continue;
        }

        match stmt {
            Statement::If(if_stmt) => {
                walk_property_narrowing_if(if_stmt, stmt, ctx, results);
            }
            Statement::Block(block) => {
                walk_property_narrowing_stmts(block.statements.iter(), ctx, results);
            }
            Statement::Expression(expr_stmt) => {
                // assert($this->prop instanceof Foo) — unconditional
                narrowing::try_apply_assert_instanceof_narrowing(
                    expr_stmt.expression,
                    ctx,
                    results,
                );
            }
            Statement::Foreach(foreach) => match &foreach.body {
                ForeachBody::Statement(inner) => {
                    walk_property_narrowing_stmt(inner, ctx, results);
                }
                ForeachBody::ColonDelimited(body) => {
                    walk_property_narrowing_stmts(body.statements.iter(), ctx, results);
                }
            },
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(inner) => {
                    walk_property_narrowing_stmt(inner, ctx, results);
                }
                WhileBody::ColonDelimited(body) => {
                    walk_property_narrowing_stmts(body.statements.iter(), ctx, results);
                }
            },
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(inner) => {
                    walk_property_narrowing_stmt(inner, ctx, results);
                }
                ForBody::ColonDelimited(body) => {
                    walk_property_narrowing_stmts(body.statements.iter(), ctx, results);
                }
            },
            Statement::DoWhile(dw) => {
                walk_property_narrowing_stmt(dw.statement, ctx, results);
            }
            Statement::Try(try_stmt) => {
                walk_property_narrowing_stmts(try_stmt.block.statements.iter(), ctx, results);
                for catch in try_stmt.catch_clauses.iter() {
                    walk_property_narrowing_stmts(catch.block.statements.iter(), ctx, results);
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    walk_property_narrowing_stmts(finally.block.statements.iter(), ctx, results);
                }
            }
            Statement::Switch(switch) => {
                for case in switch.body.cases().iter() {
                    walk_property_narrowing_stmts(case.statements().iter(), ctx, results);
                }
            }
            _ => {}
        }
    }
}

/// Apply property-level narrowing inside an if / elseif / else chain.
fn walk_property_narrowing_if<'b>(
    if_stmt: &'b mago_syntax::ast::If<'b>,
    enclosing_stmt: &'b mago_syntax::ast::Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_span::HasSpan;
    use mago_syntax::ast::*;

    use super::types::narrowing;

    match &if_stmt.body {
        IfBody::Statement(body) => {
            // ── then-body narrowing ──
            narrowing::try_apply_instanceof_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
            );
            walk_property_narrowing_stmt(body.statement, ctx, results);

            // ── elseif narrowing ──
            for else_if in body.else_if_clauses.iter() {
                narrowing::try_apply_instanceof_narrowing(
                    else_if.condition,
                    else_if.statement.span(),
                    ctx,
                    results,
                );
                walk_property_narrowing_stmt(else_if.statement, ctx, results);
            }

            // ── else-body inverse narrowing ──
            if let Some(else_clause) = &body.else_clause {
                let else_span = else_clause.statement.span();
                narrowing::try_apply_instanceof_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                for else_if in body.else_if_clauses.iter() {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                }
                walk_property_narrowing_stmt(else_clause.statement, ctx, results);
            }
        }
        IfBody::ColonDelimited(body) => {
            let then_end = if !body.else_if_clauses.is_empty() {
                body.else_if_clauses
                    .first()
                    .unwrap()
                    .elseif
                    .span()
                    .start
                    .offset
            } else if let Some(ref ec) = body.else_clause {
                ec.r#else.span().start.offset
            } else {
                body.endif.span().start.offset
            };
            let then_span = mago_span::Span::new(
                body.colon.file_id,
                body.colon.start,
                mago_span::Position::new(then_end),
            );
            narrowing::try_apply_instanceof_narrowing(if_stmt.condition, then_span, ctx, results);
            walk_property_narrowing_stmts(body.statements.iter(), ctx, results);

            for else_if in body.else_if_clauses.iter() {
                let ei_span = mago_span::Span::new(
                    else_if.colon.file_id,
                    else_if.colon.start,
                    mago_span::Position::new(
                        else_if
                            .statements
                            .span(else_if.colon.file_id, else_if.colon.end)
                            .end
                            .offset,
                    ),
                );
                narrowing::try_apply_instanceof_narrowing(else_if.condition, ei_span, ctx, results);
                walk_property_narrowing_stmts(else_if.statements.iter(), ctx, results);
            }

            if let Some(else_clause) = &body.else_clause {
                let else_span = mago_span::Span::new(
                    else_clause.colon.file_id,
                    else_clause.colon.start,
                    mago_span::Position::new(
                        else_clause
                            .statements
                            .span(else_clause.colon.file_id, else_clause.colon.end)
                            .end
                            .offset,
                    ),
                );
                narrowing::try_apply_instanceof_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                for else_if in body.else_if_clauses.iter() {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                }
                walk_property_narrowing_stmts(else_clause.statements.iter(), ctx, results);
            }
        }
    }

    // ── Guard clause narrowing ──
    // When the then-body unconditionally exits and there are no
    // elseif / else branches, apply inverse narrowing after the if.
    if enclosing_stmt.span().end.offset < ctx.cursor_offset {
        narrowing::apply_guard_clause_narrowing(if_stmt, ctx, results);
    }
}

/// Dispatch a single statement to `walk_property_narrowing_stmts`.
fn walk_property_narrowing_stmt<'b>(
    stmt: &'b mago_syntax::ast::Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    walk_property_narrowing_stmts(std::iter::once(stmt), ctx, results);
}
