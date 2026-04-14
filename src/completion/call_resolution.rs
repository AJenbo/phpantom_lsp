//! Call expression and callable target resolution.
//!
//! ## Callable target cache
//!
//! During diagnostic passes, `resolve_instance_method_callable` is
//! called for every call site in the file.  Many different chain
//! expressions resolve to the same (class, method) pair — e.g.
//! `$q->where(...)`, `$query->where(...)`, and
//! `Product::query()->where(...)` all end up looking for `where` on
//! `Builder<Product>`.  The per-file callable-target cache
//! (`CALLABLE_TARGET_CACHE`) stores `Option<ResolvedCallableTarget>`
//! keyed by `(class_fqn, method_name_lower)` so these redundant
//! resolutions are free after the first hit.
///
/// This module contains the logic for resolving call expressions (method
/// calls, static calls, function calls, constructor calls) to their
/// return types, as well as resolving callable targets for signature help
/// and named-argument completion.
///
/// Split from [`super::resolver`] for navigability.  The entry points are:
///
/// - [`Backend::resolve_callable_target`]: resolves a call expression
///   string to a [`ResolvedCallableTarget`] with label, parameters, and
///   return type (used by signature help and named-argument completion).
/// - [`Backend::resolve_call_return_types_expr`]: resolves the return
///   type of a structured [`SubjectExpr`] callee + argument text to
///   zero or more `ClassInfo` values (used by the completion chain).
/// - [`Backend::resolve_method_return_types_with_args`]: resolves a
///   method's return type on a specific class, handling conditional
///   return types and template substitutions.
/// - [`Backend::build_method_template_subs`]: builds a template
///   substitution map for method-level `@template` parameters from
///   pre-split call-site argument texts.
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::Backend;
use crate::completion::variable::rhs_resolution::{TemplateBindingMode, classify_template_binding};
use crate::completion::variable::{ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS};
use crate::docblock;
use crate::php_type::PhpType;
use crate::subject_expr::SubjectExpr;
use crate::types::ClassLikeKind;
use crate::types::*;
use crate::util::{
    find_class_at_offset, is_self_or_static, position_to_offset, resolve_class_keyword,
};

use super::conditional_resolution::{
    VarClassStringResolver, resolve_conditional_with_text_args,
    resolve_conditional_with_text_args_and_defaults, resolve_conditional_without_args,
    resolve_conditional_without_args_and_defaults, split_call_subject, split_text_args,
};
use super::resolver::{Loaders, ResolutionCtx};
use crate::util::find_class_by_name;

use tower_lsp::lsp_types::Position;

/// Bundled parameters for [`Backend::resolve_method_return_types_with_args`].
///
/// Groups the resolution-context fields that are threaded through method
/// return-type resolution so the function stays within clippy's argument
/// limit.
pub(super) struct MethodReturnCtx<'a> {
    /// All classes known in the current file.
    pub all_classes: &'a [Arc<ClassInfo>],
    /// Cross-file class resolution callback.
    pub class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    /// Template substitution map (method-level `@template` bindings).
    pub template_subs: &'a HashMap<String, PhpType>,
    /// Resolves a variable name to class-string values (for conditional
    /// return type evaluation).
    pub var_resolver: VarClassStringResolver<'a>,
    /// Shared resolved-class cache (when available).
    pub cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
    /// The class at the call site (where `self::class` / `static::class`
    /// appears), as opposed to the class that owns the method being called.
    /// Used to resolve `self`/`static`/`parent` in conditional return types.
    pub calling_class_name: Option<&'a str>,
    /// Whether the call is a static method call (`Class::method()`).
    ///
    /// When `true`, the magic-method fallback checks `__callStatic`
    /// instead of `__call`.
    pub is_static: bool,
}

/// Build a [`VarClassStringResolver`] closure from a [`ResolutionCtx`].
///
/// The returned closure resolves a variable name (e.g. `"$requestType"`)
/// to the class names it holds as class-string values by delegating to
/// [`resolve_class_string_targets`](crate::completion::variable::class_string_resolution::resolve_class_string_targets).
pub(super) fn build_var_resolver<'a>(
    ctx: &'a ResolutionCtx<'a>,
) -> impl Fn(&str) -> Vec<String> + 'a {
    move |var_name: &str| -> Vec<String> {
        if let Some(cc) = ctx.current_class {
            crate::completion::variable::class_string_resolution::resolve_class_string_targets(
                var_name,
                cc,
                ctx.all_classes,
                ctx.content,
                ctx.cursor_offset,
                ctx.class_loader,
            )
            .iter()
            .map(|c| c.name.clone())
            .collect()
        } else {
            vec![]
        }
    }
}

// ─── Thread-local callable target cache ─────────────────────────────────────

thread_local! {
    /// When `Some`, `resolve_instance_method_callable` caches results
    /// by `"FQN::method_lower"`.  Activated by
    /// [`with_callable_target_cache`], cleared on guard drop.
    static CALLABLE_TARGET_CACHE: RefCell<Option<HashMap<String, Option<ResolvedCallableTarget>>>> =
        const { RefCell::new(None) };
}

/// RAII guard that clears the callable target cache on drop.
pub(crate) struct CallableTargetCacheGuard {
    owns: bool,
}

impl Drop for CallableTargetCacheGuard {
    fn drop(&mut self) {
        if self.owns {
            CALLABLE_TARGET_CACHE.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
}

/// Activate the thread-local callable target cache.
///
/// While the returned guard is alive, `resolve_instance_method_callable`
/// caches callable target resolutions by `"FQN::method_lower"` so
/// that the same method on the same class is resolved at most once per
/// diagnostic pass, regardless of how many different chain expressions
/// lead to it.
pub(crate) fn with_callable_target_cache() -> CallableTargetCacheGuard {
    let already_active = CALLABLE_TARGET_CACHE.with(|cell| cell.borrow().is_some());
    if already_active {
        return CallableTargetCacheGuard { owns: false };
    }
    CALLABLE_TARGET_CACHE.with(|cell| {
        *cell.borrow_mut() = Some(HashMap::new());
    });
    CallableTargetCacheGuard { owns: true }
}

impl Backend {
    /// Resolve an instance method base expression + method name to a
    /// [`ResolvedCallableTarget`].
    ///
    /// Resolves `base` to owner classes, merges each via
    /// `resolve_class_fully_with_generics`, and returns the first match
    /// for `method_name`.
    fn resolve_instance_method_callable(
        base: &SubjectExpr,
        method_name: &str,
        rctx: &ResolutionCtx<'_>,
        args_text: Option<&str>,
    ) -> Option<ResolvedCallableTarget> {
        let subject_text = base.to_subject_text();
        let resolved_types: Vec<ResolvedType> = if base.is_self_like() {
            rctx.current_class
                .map(|c| ResolvedType::from_class(c.clone()))
                .into_iter()
                .collect()
        } else {
            super::resolver::resolve_target_classes(&subject_text, crate::AccessKind::Arrow, rctx)
        };

        for rt in &resolved_types {
            let owner = match &rt.class_info {
                Some(ci) => Arc::new(ci.clone()),
                None => continue,
            };

            // Extract generic type arguments from the resolved type
            // string (e.g. `Collection<User>` → `[User]`) so we can
            // substitute class-level template parameters in the
            // method's parameter and return types.
            let generic_args: Vec<PhpType> = match &rt.type_string {
                PhpType::Generic(_, args) => args.clone(),
                _ => {
                    // When the resolved type has no generic annotation
                    // but the class declares template parameters (e.g.
                    // `$errors = new Collection()` without `<string>`),
                    // fill in default type args from declared upper
                    // bounds or `mixed`.  This follows PHPStan's
                    // `resolveToBounds()` semantics and prevents raw
                    // template names like `TValue` from leaking into
                    // method parameter and return types.
                    if !owner.template_params.is_empty() {
                        crate::inheritance::default_type_args(&owner)
                    } else {
                        vec![]
                    }
                }
            };

            // ── Callable target cache check ─────────────────────────
            // When args_text is None (argument_count diagnostics),
            // the callable target depends only on the resolved class
            // and method name, not on the specific chain expression.
            // Cache by "FQN::method_lower" so that `$q->where(...)`,
            // `$query->where(...)`, and `Product::query()->where(...)`
            // all share the result.
            //
            // When args_text is Some (type_error diagnostics with
            // method-level template substitution), the result depends
            // on the call-site arguments and cannot be cached this way.
            let method_lower = method_name.to_ascii_lowercase();
            let generic_arg_strings: Vec<String> =
                generic_args.iter().map(|a| a.to_string()).collect();
            let callable_cache_key = if args_text.is_none() {
                let fqn = owner.fqn();
                let key_str = if generic_arg_strings.is_empty() {
                    format!("{}::{}", fqn, method_lower)
                } else {
                    format!(
                        "{}<{}>::{}",
                        fqn,
                        generic_arg_strings.join(","),
                        method_lower
                    )
                };
                Some(key_str)
            } else {
                None
            };

            if let Some(ref key) = callable_cache_key {
                let cached = CALLABLE_TARGET_CACHE.with(|cell| {
                    let borrow = cell.borrow();
                    borrow.as_ref().and_then(|map| map.get(key).cloned())
                });
                match cached {
                    Some(Some(target)) => return Some(target),
                    Some(None) => continue,
                    None => {}
                }
            }

            // Always use a fully-resolved class so that inherited
            // docblock types (return types, parameter types,
            // descriptions) are visible in signature help.  The
            // candidate from `resolve_target_classes` may not have
            // gone through `resolve_class_fully` (e.g. bare `new X`
            // instantiation without generics).
            //
            // Use the fused resolve+substitute helper so that the
            // result of `apply_generic_args` is cached under
            // `(FQN, generic_args)`.  For Eloquent Builder<Model>
            // chains where the same generic class appears at dozens
            // of call sites, this avoids re-cloning and
            // re-substituting hundreds of virtual members each time.
            let effective = crate::virtual_members::resolve_class_fully_with_generics(
                &owner,
                rctx.class_loader,
                rctx.resolved_class_cache,
                &generic_arg_strings,
                &generic_args,
            );

            if let Some(m) = effective
                .methods
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&method_lower))
            {
                let mut result_method = m.clone();

                // Apply method-level template substitutions when
                // call-site argument text is available.
                if let Some(at) = args_text {
                    let split_args = crate::completion::types::conditional::split_text_args(at);
                    let method_subs = Self::build_method_template_subs(
                        &effective,
                        method_name,
                        &split_args,
                        rctx,
                    );
                    if !method_subs.is_empty() {
                        crate::inheritance::apply_substitution_to_method(
                            &mut result_method,
                            &method_subs,
                        );
                    }
                }

                let target = ResolvedCallableTarget {
                    parameters: result_method.parameters.clone(),
                    return_type: result_method.return_type.clone(),
                };

                // Store positive result in the callable target cache.
                if let Some(ref key) = callable_cache_key {
                    CALLABLE_TARGET_CACHE.with(|cell| {
                        let mut borrow = cell.borrow_mut();
                        if let Some(ref mut map) = *borrow {
                            map.insert(key.clone(), Some(target.clone()));
                        }
                    });
                }

                return Some(target);
            }

            // Fall back to __call / __callStatic — the candidate
            // directly may contain model-specific members (e.g.
            // Eloquent scope methods injected onto Builder<Model>)
            // that the FQN-keyed cache does not have.
            if let Some(m) = owner
                .methods
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(method_name))
            {
                let target = ResolvedCallableTarget {
                    parameters: m.parameters.clone(),
                    return_type: m.return_type.clone(),
                };

                // Store __call fallback in the callable target cache.
                if let Some(ref key) = callable_cache_key {
                    CALLABLE_TARGET_CACHE.with(|cell| {
                        let mut borrow = cell.borrow_mut();
                        if let Some(ref mut map) = *borrow {
                            map.insert(key.clone(), Some(target.clone()));
                        }
                    });
                }

                return Some(target);
            }

            // Store negative result (method not found) in the cache.
            if let Some(ref key) = callable_cache_key {
                CALLABLE_TARGET_CACHE.with(|cell| {
                    let mut borrow = cell.borrow_mut();
                    if let Some(ref mut map) = *borrow {
                        map.insert(key.clone(), None);
                    }
                });
            }
        }
        None
    }

    /// Resolve a static class reference + method name to a
    /// [`ResolvedCallableTarget`].
    ///
    /// Resolves the class via [`super::resolver::resolve_static_owner_class`], merges
    /// via `resolve_class_fully`, and looks up `method_name`.
    fn resolve_static_method_callable(
        class: &str,
        method_name: &str,
        rctx: &ResolutionCtx<'_>,
        args_text: Option<&str>,
    ) -> Option<ResolvedCallableTarget> {
        let owner = super::resolver::resolve_static_owner_class(class, rctx)?;

        // When the class has template params but no generic annotation
        // (e.g. `RunningBonus::collect(...)` where RunningBonus extends
        // Data without `@extends` generics), substitute class-level
        // template params with their declared upper bounds or `mixed`.
        let merged = if !owner.template_params.is_empty() {
            let default_args = crate::inheritance::default_type_args(&owner);
            let base = crate::virtual_members::resolve_class_fully_maybe_cached(
                &owner,
                rctx.class_loader,
                rctx.resolved_class_cache,
            );
            Arc::new(crate::inheritance::apply_generic_args(&base, &default_args))
        } else {
            crate::virtual_members::resolve_class_fully_maybe_cached(
                &owner,
                rctx.class_loader,
                rctx.resolved_class_cache,
            )
        };

        let m = merged
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(method_name))?;

        let mut result_method = m.clone();

        // Apply method-level template substitutions when call-site
        // argument text is available.
        if let Some(at) = args_text {
            let split_args = crate::completion::types::conditional::split_text_args(at);
            let method_subs =
                Self::build_method_template_subs(&merged, method_name, &split_args, rctx);
            if !method_subs.is_empty() {
                crate::inheritance::apply_substitution_to_method(&mut result_method, &method_subs);
            }
        }

        Some(ResolvedCallableTarget {
            parameters: result_method.parameters.clone(),
            return_type: result_method.return_type.clone(),
        })
    }

    /// Build a [`ResolvedCallableTarget`] from a resolved [`FunctionInfo`].
    fn function_to_callable(func: &FunctionInfo) -> ResolvedCallableTarget {
        ResolvedCallableTarget {
            parameters: func.parameters.clone(),
            return_type: func.return_type.clone(),
        }
    }

    /// Like [`Self::function_to_callable`] but resolves function-level
    /// `@template` parameters from call-site argument text before
    /// building the callable target.  Without this, functions like
    /// `throw_unless($cond)` would report `expects TValue` instead of
    /// the concrete type.
    fn function_to_callable_with_subs(
        func: &FunctionInfo,
        args_text: Option<&str>,
        rctx: &ResolutionCtx<'_>,
    ) -> ResolvedCallableTarget {
        if let Some(at) = args_text
            && !func.template_params.is_empty()
        {
            let split_args: Vec<String> =
                crate::completion::types::conditional::split_text_args(at)
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect();
            let subs = crate::completion::variable::rhs_resolution::build_function_template_subs(
                func,
                &split_args,
                rctx,
            );
            if !subs.is_empty() {
                let parameters: Vec<_> = func
                    .parameters
                    .iter()
                    .map(|p| {
                        let mut param = p.clone();
                        if let Some(ref mut hint) = param.type_hint {
                            *hint = hint.substitute(&subs);
                        }
                        param
                    })
                    .collect();
                return ResolvedCallableTarget {
                    parameters,
                    return_type: func.return_type.clone(),
                };
            }
        }
        Self::function_to_callable(func)
    }

    /// Resolve class name keywords (`self`, `static`, `parent`) to actual
    /// class names in the context of the current class.
    fn resolve_class_name_keyword(class_name: &str, current_class: Option<&ClassInfo>) -> String {
        resolve_class_keyword(class_name, current_class).unwrap_or_else(|| class_name.to_string())
    }

    /// Build a [`ResolvedCallableTarget`] for a constructor call.
    ///
    /// Loads and merges the class, then extracts `__construct` parameters.
    fn resolve_constructor_callable(
        class_name: &str,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        cache: &crate::virtual_members::ResolvedClassCache,
    ) -> Option<ResolvedCallableTarget> {
        let ci = class_loader(class_name)?;
        let merged = crate::virtual_members::resolve_class_fully_cached(&ci, class_loader, cache);
        if let Some(ctor) = merged.methods.iter().find(|m| m.name == "__construct") {
            Some(ResolvedCallableTarget {
                parameters: ctor.parameters.clone(),
                return_type: ctor.return_type.clone(),
            })
        } else {
            Some(ResolvedCallableTarget {
                parameters: vec![],
                return_type: None,
            })
        }
    }

    // ── Main callable target resolution ─────────────────────────────────

    /// Resolve a call expression string to the callable's owner class and
    /// method (or standalone function), returning a
    /// [`ResolvedCallableTarget`] with the label, parameters, and return
    /// type.
    ///
    /// This is the single shared implementation used by both signature
    /// help (`resolve_callable`) and named-argument completion
    /// (`resolve_named_arg_params`).  Each caller projects the fields it
    /// needs from the result.
    ///
    /// The `expr` parameter uses the same format as the symbol map's
    /// `CallSite::call_expression`:
    ///   - `"functionName"` for standalone function calls
    ///   - `"$subject->method"` for instance/null-safe method calls
    ///   - `"ClassName::method"` for static method calls
    ///   - `"new ClassName"` for constructor calls
    pub(crate) fn resolve_callable_target(
        &self,
        expr: &str,
        content: &str,
        position: Position,
        file_ctx: &FileContext,
    ) -> Option<ResolvedCallableTarget> {
        self.resolve_callable_target_with_args(expr, content, position, file_ctx, None)
    }

    /// Like [`resolve_callable_target`](Self::resolve_callable_target)
    /// but accepts optional raw argument text for method-level template
    /// substitution.
    ///
    /// When `call_args_text` is `Some("$user, 42")`, method-level
    /// `@template` parameters are resolved from the call-site argument
    /// types and substituted into the parameter types before returning.
    pub(crate) fn resolve_callable_target_with_args(
        &self,
        expr: &str,
        content: &str,
        position: Position,
        file_ctx: &FileContext,
        call_args_text: Option<&str>,
    ) -> Option<ResolvedCallableTarget> {
        let class_loader = self.class_loader(file_ctx);
        let function_loader_cl = self.function_loader(file_ctx);
        let cursor_offset = position_to_offset(content, position);
        let current_class = find_class_at_offset(&file_ctx.classes, cursor_offset);

        let rctx = ResolutionCtx {
            current_class,
            all_classes: &file_ctx.classes,
            content,
            cursor_offset,
            class_loader: &class_loader,
            resolved_class_cache: Some(&self.resolved_class_cache),
            function_loader: Some(&function_loader_cl),
            scope_var_resolver: None,
        };

        let parsed = SubjectExpr::parse(expr);

        // Unwrap `CallExpr` wrapper so downstream arms match the inner
        // callee directly.  Capture `args_text` from the parsed
        // expression; prefer the caller-supplied `call_args_text` when
        // available (it comes from the source content and is more
        // accurate for method-level template substitution).
        let (effective, args_text_from_parse) = match &parsed {
            SubjectExpr::CallExpr { callee, args_text } => {
                (callee.as_ref(), Some(args_text.as_str()))
            }
            other => (other, None),
        };

        let effective_args_text = call_args_text.or(args_text_from_parse);

        match effective {
            // ── Constructor: `new ClassName` or `new ClassName()` ────
            SubjectExpr::NewExpr { class_name } => {
                let resolved_class_name =
                    Self::resolve_class_name_keyword(class_name, rctx.current_class);
                Self::resolve_constructor_callable(
                    &resolved_class_name,
                    &class_loader,
                    &self.resolved_class_cache,
                )
            }

            // ── Instance method call: `$subject->method(…)` ─────────
            SubjectExpr::MethodCall { base, method } => {
                Self::resolve_instance_method_callable(base, method, &rctx, effective_args_text)
            }

            // ── Static method call: `Class::method(…)` ──────────────
            SubjectExpr::StaticMethodCall { class, method } => {
                Self::resolve_static_method_callable(class, method, &rctx, effective_args_text)
            }

            // ── Standalone function call: `functionName(…)` ─────────
            SubjectExpr::FunctionCall(name) => {
                let func =
                    self.resolve_function_name(name, &file_ctx.use_map, &file_ctx.namespace)?;
                Some(Self::function_to_callable_with_subs(
                    &func,
                    effective_args_text,
                    &rctx,
                ))
            }

            // ── Variable used as a callable target: `$fn(…)` ────────
            // Check for a first-class callable assignment and recurse.
            SubjectExpr::Variable(var_name) => {
                let callable_target =
                    Self::extract_callable_target_from_variable(var_name, content, cursor_offset)?;
                self.resolve_callable_target_with_args(
                    &callable_target,
                    content,
                    position,
                    file_ctx,
                    call_args_text,
                )
            }

            // ── Bare class name used as a function name ─────────────
            // Named-arg and signature-help contexts pass bare function
            // names like `"foo"` which `SubjectExpr::parse` produces
            // as `ClassName` (since it can't distinguish class names
            // from function names without context).
            SubjectExpr::ClassName(name) => {
                let func =
                    self.resolve_function_name(name, &file_ctx.use_map, &file_ctx.namespace)?;
                Some(Self::function_to_callable_with_subs(
                    &func,
                    effective_args_text,
                    &rctx,
                ))
            }

            // ── PropertyChain used as a callable target ──────────────
            // Named-arg and signature-help contexts pass expressions
            // like `"$this->method"` (without trailing `()`), which
            // `SubjectExpr::parse` produces as `PropertyChain`.  Treat
            // the trailing property as a method name.
            SubjectExpr::PropertyChain { base, property } => {
                Self::resolve_instance_method_callable(base, property, &rctx, effective_args_text)
            }

            // ── StaticAccess used as a callable target ──────────────
            // Same situation: `"ClassName::method"` without `()` parses
            // as `StaticAccess` rather than `StaticMethodCall`.
            SubjectExpr::StaticAccess { class, member } => {
                Self::resolve_static_method_callable(class, member, &rctx, effective_args_text)
            }

            // ── Anything else doesn't resolve to a callable ─────────
            _ => None,
        }
    }

    /// Resolve the return type of a call expression given a structured
    /// [`SubjectExpr`] callee and argument text, returning zero or more
    /// `ClassInfo` values.
    ///
    /// This is the primary entry point for call return type resolution.
    /// The callee should be one of the "callee" variants produced by
    /// `parse_callee`: [`SubjectExpr::MethodCall`],
    /// [`SubjectExpr::StaticMethodCall`], [`SubjectExpr::FunctionCall`],
    /// [`SubjectExpr::Variable`], or [`SubjectExpr::NewExpr`].
    /// Any other variant falls through to `resolve_target_classes_expr`.
    ///
    /// Like [`resolve_call_return_types_expr`](Self::resolve_call_return_types_expr)
    /// but also captures the raw return type hint (before class resolution)
    /// into `return_type_hint_out` when provided.  This preserves generic
    /// type parameters (e.g. `HasMany<Translation, Tag>`) that would
    /// otherwise be lost when converting to `Vec<Arc<ClassInfo>>`.
    pub(crate) fn resolve_call_return_types_expr_with_hint(
        callee: &SubjectExpr,
        text_args: &str,
        ctx: &ResolutionCtx<'_>,
        mut return_type_hint_out: Option<&mut Option<PhpType>>,
    ) -> Vec<Arc<ClassInfo>> {
        match callee {
            // ── Instance method call: base->method(…) ───────────────
            SubjectExpr::MethodCall { base, method } => {
                let method_name = method.as_str();

                // Resolve the base expression to class(es).
                let lhs_classes: Vec<Arc<ClassInfo>> = ResolvedType::into_arced_classes(
                    super::resolver::resolve_target_classes_expr(base, AccessKind::Arrow, ctx),
                );

                // Capture the raw return type hint while we iterate
                // the owner classes below.  We grab it from the first
                // owner that has a matching method — before the return
                // type gets flattened into ClassInfo.
                let mut hint_captured = false;
                let mut results = Vec::new();

                for owner in &lhs_classes {
                    // Capture the return type hint from the first owner
                    // that has the method, before class resolution loses
                    // generic parameters.  The resolve_class_fully call
                    // is cached, so this doesn't duplicate work done by
                    // resolve_method_return_types_with_args below.
                    if !hint_captured && let Some(ref mut hint_out) = return_type_hint_out {
                        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                            owner,
                            ctx.class_loader,
                            ctx.resolved_class_cache,
                        );
                        if let Some(m) = merged
                            .methods
                            .iter()
                            .find(|m| m.name.eq_ignore_ascii_case(method_name))
                        {
                            if let Some(ref ret) = m.return_type {
                                **hint_out = Some(ret.clone());
                            }
                            hint_captured = true;
                        }
                    }

                    let split_args = split_text_args(text_args);
                    let arg_refs = split_args.to_vec();
                    let template_subs =
                        Self::build_method_template_subs(owner, method_name, &arg_refs, ctx);
                    let var_resolver = build_var_resolver(ctx);
                    let mr_ctx = MethodReturnCtx {
                        all_classes: ctx.all_classes,
                        class_loader: ctx.class_loader,
                        template_subs: &template_subs,
                        var_resolver: Some(&var_resolver),
                        cache: ctx.resolved_class_cache,
                        calling_class_name: ctx.current_class.map(|c| c.name.as_str()),
                        is_static: false,
                    };
                    results.extend(Self::resolve_method_return_types_with_args(
                        owner,
                        method_name,
                        text_args,
                        &mr_ctx,
                    ));
                }
                results
            }

            // ── Static method call: Class::method(…) ────────────────
            SubjectExpr::StaticMethodCall { class, method } => {
                let method_name = method.as_str();

                let owner_class = if class.starts_with('$') {
                    // Variable holding a class-string (e.g. `$cls::make()`).
                    ResolvedType::into_arced_classes(super::resolver::resolve_target_classes(
                        class,
                        AccessKind::DoubleColon,
                        ctx,
                    ))
                    .into_iter()
                    .next()
                } else {
                    super::resolver::resolve_static_owner_class(class, ctx)
                };

                if let Some(ref owner) = owner_class {
                    // Capture return type hint for static method calls.
                    // The resolve_class_fully call is cached, so this
                    // doesn't duplicate work.
                    if let Some(ref mut hint_out) = return_type_hint_out {
                        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                            owner,
                            ctx.class_loader,
                            ctx.resolved_class_cache,
                        );
                        if let Some(m) = merged
                            .methods
                            .iter()
                            .find(|m| m.name.eq_ignore_ascii_case(method_name))
                            && let Some(ref ret) = m.return_type
                        {
                            **hint_out = Some(ret.clone());
                        }
                    }

                    let split_args = split_text_args(text_args);
                    let arg_refs = split_args.to_vec();
                    let template_subs =
                        Self::build_method_template_subs(owner, method_name, &arg_refs, ctx);
                    let var_resolver = build_var_resolver(ctx);
                    let mr_ctx = MethodReturnCtx {
                        all_classes: ctx.all_classes,
                        class_loader: ctx.class_loader,
                        template_subs: &template_subs,
                        var_resolver: Some(&var_resolver),
                        cache: ctx.resolved_class_cache,
                        calling_class_name: ctx.current_class.map(|c| c.name.as_str()),
                        is_static: true,
                    };
                    return Self::resolve_method_return_types_with_args(
                        owner,
                        method_name,
                        text_args,
                        &mr_ctx,
                    );
                }
                vec![]
            }

            // ── Standalone function call: app(…) / myHelper(…) ──────
            SubjectExpr::FunctionCall(func_name) => {
                let func_name = func_name.as_str();

                // Check for array element/preserving functions first.
                let is_array_element_func = ARRAY_ELEMENT_FUNCS
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(func_name));
                let is_array_preserving_func = ARRAY_PRESERVING_FUNCS
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(func_name));

                if (is_array_element_func || is_array_preserving_func)
                    && !text_args.is_empty()
                    && let Some(first_arg) = Self::extract_first_arg_text(text_args)
                {
                    let arg_raw_type = Self::resolve_inline_arg_raw_type(&first_arg, ctx);

                    if let Some(ref raw) = arg_raw_type
                        && let Some(element_type) = raw.extract_value_type(true)
                    {
                        let owner_name = ctx.current_class.map(|c| c.name.as_str()).unwrap_or("");
                        let classes: Vec<Arc<ClassInfo>> =
                            super::type_resolution::type_hint_to_classes_typed(
                                element_type,
                                owner_name,
                                ctx.all_classes,
                                ctx.class_loader,
                            )
                            .into_iter()
                            .map(Arc::new)
                            .collect();
                        if !classes.is_empty() {
                            return classes;
                        }
                    }
                }

                // Regular function lookup.
                if let Some(fl) = ctx.function_loader
                    && let Some(func_info) = fl(func_name)
                {
                    if let Some(ref cond) = func_info.conditional_return {
                        let var_resolver = build_var_resolver(ctx);
                        let resolved_type = if !text_args.is_empty() {
                            resolve_conditional_with_text_args(
                                cond,
                                &func_info.parameters,
                                text_args,
                                Some(&var_resolver),
                                ctx.current_class.map(|c| c.name.as_str()),
                                ctx.class_loader,
                            )
                        } else {
                            resolve_conditional_without_args(cond, &func_info.parameters)
                        };
                        if let Some(ref parsed_ty) = resolved_type {
                            let classes: Vec<Arc<ClassInfo>> =
                                super::type_resolution::type_hint_to_classes_typed(
                                    parsed_ty,
                                    "",
                                    ctx.all_classes,
                                    ctx.class_loader,
                                )
                                .into_iter()
                                .map(Arc::new)
                                .collect();
                            if !classes.is_empty() {
                                return classes;
                            }
                        }
                    }
                    // ── Function-level @template substitution ────────
                    // When the function has template params and bindings,
                    // infer concrete types from the arguments and apply
                    // substitution to the return type before resolving.
                    // Delegates to `build_function_template_subs` which
                    // handles Direct, ArrayElement, and GenericWrapper
                    // binding modes (e.g. `@param array<TKey, TValue>`).
                    if !func_info.template_params.is_empty() && func_info.return_type.is_some() {
                        let split_args: Vec<String> = if text_args.is_empty() {
                            vec![]
                        } else {
                            split_text_args(text_args)
                                .into_iter()
                                .map(|s| s.to_string())
                                .collect()
                        };
                        let subs = super::variable::rhs_resolution::build_function_template_subs(
                            &func_info,
                            &split_args,
                            ctx,
                        );

                        if !subs.is_empty()
                            && let Some(ref ret) = func_info.return_type
                        {
                            let substituted = ret.substitute(&subs);
                            let classes: Vec<Arc<ClassInfo>> =
                                super::type_resolution::type_hint_to_classes_typed(
                                    &substituted,
                                    "",
                                    ctx.all_classes,
                                    ctx.class_loader,
                                )
                                .into_iter()
                                .map(Arc::new)
                                .collect();
                            if !classes.is_empty() {
                                return classes;
                            }
                        }
                    }

                    if let Some(ref ret) = func_info.return_type {
                        // Capture the function's return type hint.
                        if let Some(ref mut hint_out) = return_type_hint_out {
                            **hint_out = Some(ret.clone());
                        }
                        return super::type_resolution::type_hint_to_classes_typed(
                            ret,
                            "",
                            ctx.all_classes,
                            ctx.class_loader,
                        )
                        .into_iter()
                        .map(Arc::new)
                        .collect();
                    }
                }

                vec![]
            }

            // ── Variable invocation: $fn(…) ─────────────────────────
            SubjectExpr::Variable(var_name) => {
                let content = ctx.content;
                let cursor_offset = ctx.cursor_offset;

                // 1. Try docblock annotation: `@var Closure(): User $fn`
                if let Some(raw_type) = crate::docblock::find_iterable_raw_type_in_source(
                    content,
                    cursor_offset as usize,
                    var_name,
                ) && let Some(ret_type) = raw_type.callable_return_type()
                {
                    let classes: Vec<Arc<ClassInfo>> =
                        super::type_resolution::type_hint_to_classes_typed(
                            ret_type,
                            "",
                            ctx.all_classes,
                            ctx.class_loader,
                        )
                        .into_iter()
                        .map(Arc::new)
                        .collect();
                    if !classes.is_empty() {
                        return classes;
                    }
                }

                // 2. Scan for closure/arrow-function literal assignment.
                if let Some(ret) =
                    super::source::helpers::extract_closure_return_type_from_assignment(
                        var_name,
                        content,
                        cursor_offset,
                    )
                {
                    let classes: Vec<Arc<ClassInfo>> =
                        super::type_resolution::type_hint_to_classes_typed(
                            &ret,
                            "",
                            ctx.all_classes,
                            ctx.class_loader,
                        )
                        .into_iter()
                        .map(Arc::new)
                        .collect();
                    if !classes.is_empty() {
                        return classes;
                    }
                }

                // 3. Scan for first-class callable assignment.
                if let Some(ret) =
                    super::source::helpers::extract_first_class_callable_return_type(var_name, ctx)
                {
                    let classes: Vec<Arc<ClassInfo>> =
                        super::type_resolution::type_hint_to_classes_typed(
                            &ret,
                            "",
                            ctx.all_classes,
                            ctx.class_loader,
                        )
                        .into_iter()
                        .map(Arc::new)
                        .collect();
                    if !classes.is_empty() {
                        return classes;
                    }
                }

                // 4. Resolve the variable's type and check for __invoke().
                //    When $f holds an object with an __invoke() method,
                //    $f() should return __invoke()'s return type.
                let var_classes = ResolvedType::into_arced_classes(
                    super::resolver::resolve_target_classes(var_name, AccessKind::Arrow, ctx),
                );
                for owner in &var_classes {
                    if let Some(invoke) = owner.methods.iter().find(|m| m.name == "__invoke")
                        && let Some(ref ret) = invoke.return_type
                    {
                        let classes: Vec<Arc<ClassInfo>> =
                            super::type_resolution::type_hint_to_classes_typed(
                                ret,
                                "",
                                ctx.all_classes,
                                ctx.class_loader,
                            )
                            .into_iter()
                            .map(Arc::new)
                            .collect();
                        if !classes.is_empty() {
                            return classes;
                        }
                    }
                }

                vec![]
            }

            // ── Constructor call: new ClassName(…) ──────────────────
            // A `NewExpr` callee means the call is `new Foo(…)` — the
            // return type is always the class itself.
            SubjectExpr::NewExpr { class_name } => find_class_by_name(ctx.all_classes, class_name)
                .map(Arc::clone)
                .or_else(|| (ctx.class_loader)(class_name))
                .into_iter()
                .collect(),

            // ── Any other callee form (e.g. a nested CallExpr used as
            //    a callee, a PropertyChain for `($this->prop)()`, or a
            //    ClassName that SubjectExpr::parse couldn't distinguish
            //    from a function name) ───────────────────────────────
            _ => {
                // Resolve the callee expression to class(es).
                let callee_classes = ResolvedType::into_arced_classes(
                    super::resolver::resolve_target_classes_expr(callee, AccessKind::Arrow, ctx),
                );

                // When the callee resolves to an object with __invoke(),
                // the call returns __invoke()'s return type, not the
                // object itself.  This handles `($this->formatter)()`.
                for owner in &callee_classes {
                    if let Some(invoke) = owner.methods.iter().find(|m| m.name == "__invoke")
                        && let Some(ref ret) = invoke.return_type
                    {
                        let classes: Vec<Arc<ClassInfo>> =
                            super::type_resolution::type_hint_to_classes_typed(
                                ret,
                                "",
                                ctx.all_classes,
                                ctx.class_loader,
                            )
                            .into_iter()
                            .map(Arc::new)
                            .collect();
                        if !classes.is_empty() {
                            return classes;
                        }
                    }
                }

                callee_classes
            }
        }
    }

    /// Thin wrapper around [`resolve_call_return_types_expr_with_hint`]
    /// that discards the return type hint.  Existing callers that don't
    /// need generic type preservation use this.
    pub(crate) fn resolve_call_return_types_expr(
        callee: &SubjectExpr,
        text_args: &str,
        ctx: &ResolutionCtx<'_>,
    ) -> Vec<Arc<ClassInfo>> {
        Self::resolve_call_return_types_expr_with_hint(callee, text_args, ctx, None)
    }

    /// Resolve a method call's return type, taking into account PHPStan
    /// conditional return types when `text_args` is provided, and
    /// method-level `@template` substitutions when `template_subs` is
    /// non-empty.
    ///
    /// This is the workhorse behind both `resolve_method_return_types`
    /// (which passes `""`) and the inline call-chain path (which passes
    /// the raw argument text from the source, e.g. `"CurrentCart::class"`).
    pub(super) fn resolve_method_return_types_with_args(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
        mr_ctx: &MethodReturnCtx<'_>,
    ) -> Vec<Arc<ClassInfo>> {
        let all_classes = mr_ctx.all_classes;
        let class_loader = mr_ctx.class_loader;
        let template_subs = mr_ctx.template_subs;
        let var_resolver = mr_ctx.var_resolver;
        // Helper: try to resolve a method's conditional return type, falling
        // back to template-substituted return type, then plain return type.
        let resolve_method = |method: &MethodInfo| -> Vec<Arc<ClassInfo>> {
            // Try conditional return type first (PHPStan syntax)
            if let Some(ref cond) = method.conditional_return {
                let resolved_type = if !text_args.is_empty() {
                    resolve_conditional_with_text_args_and_defaults(
                        cond,
                        &method.parameters,
                        text_args,
                        var_resolver,
                        mr_ctx.calling_class_name,
                        Some(&class_info.template_param_defaults),
                        mr_ctx.class_loader,
                    )
                } else {
                    resolve_conditional_without_args_and_defaults(
                        cond,
                        &method.parameters,
                        Some(&class_info.template_param_defaults),
                    )
                };
                if let Some(ref parsed) = resolved_type {
                    // Apply method-level template substitutions to the
                    // resolved conditional type (e.g. `TModel` → concrete
                    // class when TModel is a method-level @template param).
                    let effective = if !template_subs.is_empty() {
                        parsed.substitute(template_subs)
                    } else {
                        parsed.clone()
                    };
                    let classes: Vec<Arc<ClassInfo>> =
                        super::type_resolution::type_hint_to_classes_typed(
                            &effective,
                            &class_info.fqn(),
                            all_classes,
                            class_loader,
                        )
                        .into_iter()
                        .map(Arc::new)
                        .collect();
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }

            // Try method-level @template substitution on the return type.
            // This handles the general case where the return type references
            // a template param (e.g. `@return Collection<T>`) and we have
            // resolved bindings from the call-site arguments.
            if !template_subs.is_empty()
                && let Some(ref ret) = method.return_type
            {
                let substituted = ret.substitute(template_subs);
                if &substituted != ret {
                    let classes: Vec<Arc<ClassInfo>> =
                        super::type_resolution::type_hint_to_classes_typed(
                            &substituted,
                            &class_info.fqn(),
                            all_classes,
                            class_loader,
                        )
                        .into_iter()
                        .map(Arc::new)
                        .collect();
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }

            // Fall back to plain return type
            if let Some(ref ret) = method.return_type {
                // When the return type is `static`, `self`, or `$this`,
                // return the owning class directly.  This avoids a lookup
                // by short name (e.g. "Builder") which fails when the
                // class was loaded cross-file and the short name is not
                // in the current file's use-map or local classes.
                // Returning class_info preserves any generic substitutions
                // already applied (e.g. Builder<User> stays Builder<User>).
                // Match bare `self`/`static`/`$this` as well as nullable
                // (`?static`) and union (`static|null`) forms, plus
                // generic wrappers like `self<RuleError>`, `static<T>`.
                if ret.is_self_like() {
                    return vec![Arc::new(class_info.clone())];
                }
                let resolved = super::type_resolution::type_hint_to_classes_typed(
                    ret,
                    &class_info.fqn(),
                    all_classes,
                    class_loader,
                );
                return resolved.into_iter().map(Arc::new).collect();
            }
            vec![]
        };

        // Determine which magic method handles unknown calls for this
        // access kind: `__call` for instance calls, `__callStatic` for
        // static calls.
        let magic_name = if mr_ctx.is_static {
            "__callStatic"
        } else {
            "__call"
        };

        // First check the class itself
        if let Some(method) = class_info.methods.iter().find(|m| m.name == method_name) {
            let result = resolve_method(method);
            if !result.is_empty() {
                return result;
            }
            // Fall through to the merged class — the method may lack a
            // return type here but have one filled in from an interface
            // via `@implements` generic resolution.
        }

        // Walk up the inheritance chain (also merges interface members
        // with `@implements` generic substitutions applied).
        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
            class_info,
            class_loader,
            mr_ctx.cache,
        );

        // Look up the magic method once; used for both validation and
        // fallback below.
        let magic_method = merged
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(magic_name));

        if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
            if method.is_virtual {
                // ── Virtual method (from @method, @mixin, etc.) ─────
                // At runtime these are dispatched through __call /
                // __callStatic.  Validate the virtual method's return
                // type against the magic method's native return type
                // the same way we validate a concrete implementation
                // against an interface: the virtual type can only
                // *narrow* the native constraint, not contradict it.
                if let Some(ref virtual_ret) = method.return_type {
                    if let Some(magic) = magic_method {
                        if let Some(ref native_ret) = magic.native_return_type {
                            // The magic method has a native PHP type
                            // hint.  Check whether the virtual
                            // method's declared type is a valid
                            // narrowing of that native constraint.
                            if is_valid_virtual_narrowing(
                                virtual_ret,
                                native_ret,
                                class_info,
                                all_classes,
                                class_loader,
                            ) {
                                // Valid narrowing — trust the virtual
                                // method's declared type.
                                let result = resolve_method(method);
                                if !result.is_empty() {
                                    return result;
                                }
                            }
                            // Invalid narrowing (lie) or the virtual
                            // type failed to resolve.  Fall through
                            // to the magic-method fallback below,
                            // which will use __call's own return type.
                        } else {
                            // Magic method has no native type hint —
                            // trust the virtual method's declared type.
                            let result = resolve_method(method);
                            if !result.is_empty() {
                                return result;
                            }
                        }
                    } else {
                        // No magic method at all — trust the virtual
                        // method's declared type unconditionally.
                        let result = resolve_method(method);
                        if !result.is_empty() {
                            return result;
                        }
                    }
                }
                // Virtual method with no return type (or whose type
                // was rejected by the validation above).  Fall through
                // to the magic-method fallback below.
            } else {
                // ── Real method ─────────────────────────────────────
                // Real methods are invoked directly at runtime, never
                // through __call.  Use whatever resolve_method
                // returns, even if empty.
                return resolve_method(method);
            }
        }

        // ── Magic-method fallback ───────────────────────────────
        // Either the method was not found at all, or it was a virtual
        // method whose return type was absent or rejected by the
        // native-type validation.  Use the magic method's effective
        // return type (docblock-overridden if available, otherwise
        // native).  When the magic method returns `$this`/`static`/
        // `self`, this preserves the chain type (e.g. Builder<User>
        // stays Builder<User> through dynamic `where{Column}` calls).
        // When it returns `mixed`, no classes resolve and the caller
        // gets an empty vec — the same as before this fallback.
        if let Some(magic) = magic_method {
            let result = resolve_method(magic);
            if !result.is_empty() {
                return result;
            }
        }

        vec![]
    }
}

// ─── Virtual method narrowing ───────────────────────────────────────────────

/// Check whether a virtual method's return type is a valid narrowing of a
/// magic method's (`__call` / `__callStatic`) native return type.
///
/// At runtime, calls to virtual methods (from `@method` tags, `@mixin`
/// members, etc.) are dispatched through the magic method.  The magic
/// method's native PHP type hint is the runtime truth: the virtual
/// method's declared type can only *narrow* it (provide a more specific
/// subtype), not contradict it.
///
/// Returns `true` when the virtual type should be trusted, `false` when
/// it should be rejected in favour of the magic method's type.
///
/// # Examples
///
/// | `__call` native | `@method` type | Result |
/// |-----------------|----------------|--------|
/// | `mixed`         | `Frog`         | ✓ (anything narrows mixed) |
/// | `object`        | `Frog`         | ✓ (any class narrows object) |
/// | `static`        | `ChildClass`   | ✓ if ChildClass extends the owner |
/// | `Animal`        | `Dog`          | ✓ if Dog extends Animal |
/// | `Cement`        | `Frog`         | ✗ (unrelated classes) |
/// | `static`        | `Frog`         | ✗ if Frog does not extend the owner |
/// | `int`           | `string`       | ✗ (incompatible scalars) |
fn is_valid_virtual_narrowing(
    virtual_type: &PhpType,
    native_type: &PhpType,
    owner_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    // `mixed` and `void` impose no constraint — any type is valid.
    if native_type.is_mixed() || native_type.is_void() {
        return true;
    }

    // `object` — any class type is a valid narrowing.
    if native_type.is_object() {
        // Only reject if the virtual type is a non-object scalar.
        return !virtual_type.is_scalar();
    }

    // Self-like types (`static`, `self`, `$this`) resolve to the owner
    // class at runtime.  The virtual type must be the owner class itself
    // or a subclass of it.
    if native_type.is_self_like() {
        return is_type_subclass_of(virtual_type, &owner_class.fqn(), all_classes, class_loader);
    }

    // Both are concrete types.  For scalar-to-scalar, delegate to the
    // existing `should_override_type` check which handles compatible
    // refinements (e.g. `string` → `class-string<T>`).
    if native_type.is_scalar() {
        return crate::docblock::should_override_type_typed(virtual_type, native_type);
    }

    // Native is a class type — the virtual type must be the same class
    // or a subclass.
    if let Some(name) = native_type.base_name() {
        is_type_subclass_of(virtual_type, name, all_classes, class_loader)
    } else {
        false
    }
}

/// Check whether `candidate_type` is the same class as `ancestor_name` or
/// a subclass of it, by walking the parent chain.
///
/// Returns `true` when:
/// - The candidate type's base name matches `ancestor_name` (case-insensitive).
/// - The candidate class's parent chain includes `ancestor_name`.
/// - The candidate class cannot be resolved (benefit of the doubt).
fn is_type_subclass_of(
    candidate_type: &PhpType,
    ancestor_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    // Cannot extract a base name → not a class type → not a subclass.
    if candidate_type.base_name().is_none() {
        return false;
    }

    // Build a combined loader that checks local classes first.
    let combined_loader = |name: &str| -> Option<Arc<ClassInfo>> {
        find_class_by_name(all_classes, name)
            .cloned()
            .or_else(|| class_loader(name))
    };

    // Check if the candidate can be resolved at all.  When it cannot,
    // give the benefit of the doubt (e.g. trust an @method tag).
    if let Some(base) = candidate_type.base_name()
        && combined_loader(base).is_none()
    {
        return true;
    }

    crate::util::is_subtype_of_named(candidate_type, ancestor_name, &combined_loader)
}

impl Backend {
    /// Build a template substitution map for a method-level `@template` call.
    ///
    /// Finds the method on the class (or inherited), checks for template
    /// params and bindings, resolves argument types from the pre-split
    /// `arg_texts` slice using the call resolution context, and returns a
    /// `HashMap` mapping template parameter names to their resolved
    /// concrete types.
    ///
    /// Callers with an AST `ArgumentList` should extract per-argument text
    /// via [`extract_arg_texts_from_ast`] and convert to `&[&str]`.
    /// Callers with only raw text should use [`split_text_args`] first.
    ///
    /// Returns an empty map if the method has no template params, no
    /// bindings, or if argument types cannot be resolved.
    pub(super) fn build_method_template_subs(
        class_info: &ClassInfo,
        method_name: &str,
        arg_texts: &[&str],
        ctx: &ResolutionCtx<'_>,
    ) -> HashMap<String, PhpType> {
        // Find the method — first on the class directly, then via inheritance.
        let method = class_info
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .cloned()
            .or_else(|| {
                let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                    class_info,
                    ctx.class_loader,
                    ctx.resolved_class_cache,
                );
                merged
                    .methods
                    .iter()
                    .find(|m| m.name == method_name)
                    .cloned()
            });

        let method = match method {
            Some(m) if !m.template_params.is_empty() => m,
            _ => return HashMap::new(),
        };

        let mut subs = HashMap::new();

        for (tpl_name, param_name) in &method.template_bindings {
            // Find the parameter index for this binding.
            let param_idx = match method.parameters.iter().position(|p| p.name == *param_name) {
                Some(idx) => idx,
                None => continue,
            };

            // Get the corresponding argument text.
            let arg_text = match arg_texts.get(param_idx) {
                Some(text) => text.trim(),
                None => {
                    // No argument was provided at the call site.  If the
                    // parameter has a default value of `null`, resolve the
                    // template param to `null`.  This handles the common
                    // pattern `@template TDefault` with
                    // `@param TDefault $default = null` where the caller
                    // omits the argument — e.g. `Collection::first()`
                    // returns `TValue|TFirstDefault` and `TFirstDefault`
                    // should become `null` when no default is passed.
                    if let Some(param) = method.parameters.get(param_idx)
                        && param.default_value.as_deref().is_some_and(|d| d == "null")
                        && !subs.contains_key(tpl_name)
                    {
                        subs.insert(tpl_name.clone(), PhpType::null());
                    }
                    continue;
                }
            };

            // Classify how the template param appears in the parameter's
            // type hint (direct, array element, generic wrapper, or
            // callable return type).
            let param_hint = method
                .parameters
                .get(param_idx)
                .and_then(|p| p.type_hint.as_ref());
            let binding_mode = classify_template_binding(tpl_name, param_hint);

            match binding_mode {
                TemplateBindingMode::Direct => {
                    if let Some(resolved_type) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                        subs.insert(tpl_name.clone(), resolved_type);
                    }
                }
                TemplateBindingMode::GenericWrapper(ref wrapper_name, tpl_position) => {
                    if let Some(resolved_type) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                        // Special handling for class-string<T> to avoid double-wrapping
                        if wrapper_name == "class-string"
                            && tpl_position == 0
                            && let Some(inner) = resolved_type.unwrap_class_string_inner()
                        {
                            subs.insert(tpl_name.clone(), inner.clone());
                            continue;
                        }
                        subs.insert(tpl_name.clone(), resolved_type);
                    }
                }
                TemplateBindingMode::CallableReturnType => {
                    // `@param callable(...): T $cb` — extract the closure's
                    // return type annotation from the argument text.
                    if let Some(ret_type) =
                        super::source::helpers::extract_closure_return_type_from_text(arg_text)
                    {
                        subs.insert(tpl_name.clone(), ret_type);
                    }
                }
                TemplateBindingMode::CallableParamType(position) => {
                    // `@param Closure(T): void $cb` — extract the closure's
                    // parameter type annotation at the given position.
                    if let Some(param_type) =
                        super::source::helpers::extract_closure_param_type_from_text(
                            arg_text, position,
                        )
                    {
                        subs.insert(tpl_name.clone(), param_type);
                    }
                }
                TemplateBindingMode::ArrayElement => {
                    if let Some(resolved_type) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                        subs.insert(tpl_name.clone(), resolved_type);
                    }
                }
                TemplateBindingMode::ClassStringInner => {
                    if let Some(resolved_type) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                        // Unwrap `class-string<X>` → `X` so that the
                        // substitution doesn't double-wrap.
                        let unwrapped = match resolved_type {
                            PhpType::ClassString(Some(inner)) => *inner,
                            _ => resolved_type,
                        };
                        subs.insert(tpl_name.clone(), unwrapped);
                    }
                }
            }
        }

        // ── Fill in unbound method-level template params ────────
        // Any template parameter that was not bound from call-site
        // arguments is replaced with its declared upper bound
        // (`@template T of Foo` → `Foo`) or `mixed`.  This follows
        // PHPStan's `resolveToBounds()` semantics and prevents raw
        // template names like `TReduceReturnType` from leaking into
        // parameter and return types.
        for tpl_name in &method.template_params {
            if !subs.contains_key(tpl_name) {
                let fallback = method
                    .template_param_bounds
                    .get(tpl_name)
                    .cloned()
                    .unwrap_or_else(PhpType::mixed);
                subs.insert(tpl_name.clone(), fallback);
            }
        }

        subs
    }

    /// Resolve an argument text string to a type name.
    ///
    /// Handles common patterns:
    /// - `ClassName::class` → `ClassName`
    /// - `new ClassName(…)` → `ClassName`
    /// - `$this` / `self` / `static` → current class name
    /// - `$this->prop` → property type
    /// - `$var` → variable type via assignment scanning
    /// - `"hello"` / `'world'` → `string`
    /// - `42` / `-1` → `int`
    /// - `3.14` → `float`
    /// - `true` / `false` → `bool`
    /// - `null` → `null`
    /// - `[…]` → `array`
    /// - `EnumClass::Case` → `EnumClass`
    /// - `ClassName::CONSTANT` → constant's declared type
    pub(crate) fn resolve_arg_text_to_type(
        arg_text: &str,
        ctx: &ResolutionCtx<'_>,
    ) -> Option<PhpType> {
        let trimmed = arg_text.trim();

        // ── Literal values ──────────────────────────────────────
        if let Some(ty) = resolve_literal_type(trimmed) {
            return Some(ty);
        }

        // ClassName::class → ClassName
        if let Some(name) = trimmed.strip_suffix("::class")
            && !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
        {
            let resolved_name = if let Some(cls) = (ctx.class_loader)(name) {
                cls.fqn()
            } else {
                name.to_string()
            };
            return Some(PhpType::Named(resolved_name));
        }

        // When the expression contains a `->` chain (e.g.
        // `Country::DK->value`, `new Decimal($x)->toFixed(2)`),
        // skip the static-access and new-expression shortcuts —
        // they would match the prefix and ignore the chain.
        // Let `resolve_expression_to_type` handle the full chain.
        let has_arrow_chain = trimmed.contains("->");

        // ClassName::Member — enum cases and class constants.
        // Enum cases resolve to the enum type; class constants
        // resolve to the constant's declared type hint.
        if !has_arrow_chain && let Some(ty) = resolve_static_access_type(trimmed, ctx) {
            return Some(ty);
        }

        // new ClassName(…) → ClassName
        if !has_arrow_chain
            && let Some(class_name) = super::source::helpers::extract_new_expression_class(trimmed)
        {
            let resolved_name = if let Some(cls) = (ctx.class_loader)(&class_name) {
                cls.fqn()
            } else {
                class_name
            };
            return Some(PhpType::Named(resolved_name));
        }

        // $this / self / static → current class
        if is_self_or_static(trimmed) {
            return ctx.current_class.map(|c| PhpType::Named(c.name.clone()));
        }

        // General expression fallback: parse the argument text as a
        // SubjectExpr and try to resolve it to a type.  This handles
        // $var, $var->prop, $this->prop, $var->method(), method
        // chains, and any other expression pattern.
        if let Some(ty) = resolve_expression_to_type(trimmed, ctx) {
            return Some(ty);
        }

        None
    }
}

/// Resolve an arbitrary expression to a [`PhpType`].
///
/// Unlike [`super::resolver::resolve_target_classes`] (which returns
/// class-bearing `ResolvedType` values and drops scalars), this function
/// returns the *type* of the expression — including scalars like `string`,
/// `int`, `bool`, etc.
///
/// Strategy:
/// 1. Parse the expression with [`SubjectExpr::parse`].
/// 2. For property chains (`$var->prop`), resolve the base to classes
///    and look up the property's type hint directly.
/// 3. For method calls (`$var->method()`), resolve the base and look
///    up the method's return type.
/// 4. For everything else, fall back to [`super::resolver::resolve_target_classes`]
///    which handles variables, `$this`, `new …`, etc.
fn resolve_expression_to_type(text: &str, ctx: &ResolutionCtx<'_>) -> Option<PhpType> {
    let expr = SubjectExpr::parse(text);

    match &expr {
        // $var->prop or $this->prop — resolve base to classes, then
        // look up the property's type hint (which preserves scalars).
        SubjectExpr::PropertyChain { base, property } => {
            let base_text = base.to_subject_text();
            let base_classes = ResolvedType::into_arced_classes(
                super::resolver::resolve_target_classes(&base_text, crate::AccessKind::Arrow, ctx),
            );
            for cls in &base_classes {
                if let Some(hint) =
                    crate::inheritance::resolve_property_type_hint(cls, property, ctx.class_loader)
                {
                    return Some(hint);
                }
            }
            None
        }

        // $var->method() or $this->helper->getText() — resolve the
        // full call expression to return-type classes first; if that
        // yields nothing class-like, resolve the raw return type
        // (which preserves scalars).
        SubjectExpr::CallExpr { callee, args_text } => {
            // Try class-bearing resolution first (handles non-scalar returns).
            let classes = Backend::resolve_call_return_types_expr(callee, args_text, ctx);
            if let Some(first) = classes.first() {
                return Some(PhpType::Named(first.fqn()));
            }

            // Fall back to raw return type for scalar returns.
            if let SubjectExpr::MethodCall { base, method } = callee.as_ref() {
                let base_text = base.to_subject_text();
                let base_classes =
                    ResolvedType::into_arced_classes(super::resolver::resolve_target_classes(
                        &base_text,
                        crate::AccessKind::Arrow,
                        ctx,
                    ));
                for cls in &base_classes {
                    if let Some(rt) = crate::inheritance::resolve_method_return_type(
                        cls,
                        method,
                        ctx.class_loader,
                    ) {
                        return Some(rt);
                    }
                }
            }
            if let SubjectExpr::StaticMethodCall { class, method } = callee.as_ref()
                && let Some(owner) = super::resolver::resolve_static_owner_class(class, ctx)
                && let Some(rt) =
                    crate::inheritance::resolve_method_return_type(&owner, method, ctx.class_loader)
            {
                return Some(rt);
            }
            None
        }

        // Everything else ($var, $this, ClassName, new X, etc.) —
        // use the standard resolver which returns class-bearing types.
        _ => {
            let resolved =
                super::resolver::resolve_target_classes(text, crate::types::AccessKind::Arrow, ctx);
            if let Some(first) = resolved.first() {
                return Some(first.type_string.clone());
            }
            None
        }
    }
}

/// Resolve a `ClassName::Member` expression to a type.
///
/// Handles enum cases (`MyEnum::Case` → `MyEnum`) and class constants
/// (`Foo::BAR` → the constant's type hint, or the class itself as
/// fallback for untyped constants).
fn resolve_static_access_type(text: &str, ctx: &ResolutionCtx<'_>) -> Option<PhpType> {
    let (class_part, _member) = text.split_once("::")?;

    // Only accept identifier-like class names (no `$var::`, no whitespace).
    if class_part.is_empty()
        || class_part.starts_with('$')
        || !class_part
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
    {
        return None;
    }

    // Resolve `self` / `static` / `parent` to the actual class name.
    let class_name = if is_self_or_static(class_part) {
        ctx.current_class?.name.clone()
    } else if let Some(resolved) = resolve_class_keyword(class_part, ctx.current_class) {
        resolved
    } else {
        class_part.to_string()
    };

    let cls = (ctx.class_loader)(&class_name)?;

    // Enums: any `EnumName::Case` resolves to the enum type itself.
    if cls.kind == ClassLikeKind::Enum {
        return Some(PhpType::Named(cls.fqn()));
    }

    // Class constants: look up the constant and use its type hint
    // when available.  Fall back to the owning class type (which is
    // conservative but avoids leaving the raw template param name).
    let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
        &cls,
        ctx.class_loader,
        ctx.resolved_class_cache,
    );
    if let Some(constant) = merged.constants.iter().find(|c| c.name == _member)
        && let Some(ref hint) = constant.type_hint
    {
        return Some(hint.clone());
    }

    // Unknown member or untyped constant — we can't determine the
    // type, so return None and let the caller skip the diagnostic.
    None
}

/// Resolve a literal expression to its PHP type.
///
/// Returns `Some(PhpType)` for string literals (`"…"`, `'…'`), integer
/// literals (`42`, `-1`), float literals (`3.14`), boolean literals
/// (`true`, `false`), `null`, and array literals (`[…]`).
fn resolve_literal_type(text: &str) -> Option<PhpType> {
    // String literals: "…" or '…'
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        return Some(PhpType::Named("string".to_string()));
    }

    // null
    if text.eq_ignore_ascii_case("null") {
        return Some(PhpType::null());
    }

    // Boolean literals
    if text.eq_ignore_ascii_case("true") || text.eq_ignore_ascii_case("false") {
        return Some(PhpType::Named("bool".to_string()));
    }

    // Array literals: [...] or array(...)
    if (text.starts_with('[') && text.ends_with(']'))
        || (text.starts_with("array(") && text.ends_with(')'))
    {
        return Some(PhpType::Named("array".to_string()));
    }

    // Numeric literals — try int first, then float.
    // Strip an optional leading minus for negative literals.
    let numeric = text.strip_prefix('-').unwrap_or(text);
    if !numeric.is_empty()
        && numeric.bytes().all(|b| b.is_ascii_digit() || b == b'_')
        && numeric.bytes().any(|b| b.is_ascii_digit())
    {
        return Some(PhpType::Named("int".to_string()));
    }
    if !numeric.is_empty()
        && numeric
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'.' || b == b'_')
        && numeric.bytes().filter(|&b| b == b'.').count() == 1
        && numeric.bytes().any(|b| b.is_ascii_digit())
    {
        return Some(PhpType::Named("float".to_string()));
    }

    None
}

/// Like [`resolve_instance_method_callable`](Self::resolve_instance_method_callable)
impl Backend {
    /// Extract the first argument from a comma-separated argument text,
    /// respecting nested parentheses, brackets, and braces.
    fn extract_first_arg_text(args_text: &str) -> Option<String> {
        let trimmed = args_text.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut depth = 0i32;
        for (i, ch) in trimmed.char_indices() {
            match ch {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    let arg = trimmed[..i].trim();
                    if !arg.is_empty() {
                        return Some(arg.to_string());
                    }
                    return None;
                }
                _ => {}
            }
        }
        // Single (or last) argument.
        let arg = trimmed.trim();
        if !arg.is_empty() {
            Some(arg.to_string())
        } else {
            None
        }
    }

    /// Resolve the raw return type of an inline argument expression.
    ///
    /// Handles plain variables (`$customers`), call chains
    /// (`Customer::get()->all()`), and static calls (`ClassName::method()`).
    ///
    /// Returns the structured type (e.g. `array<int, Customer>`) so
    /// that the caller can extract element types from it.
    fn resolve_inline_arg_raw_type(arg_text: &str, ctx: &ResolutionCtx<'_>) -> Option<PhpType> {
        let current_class = ctx.current_class;
        let all_classes = ctx.all_classes;
        let class_loader = ctx.class_loader;

        // ── Plain variable: `$customers` ────────────────────────────────
        if arg_text.starts_with('$')
            && arg_text[1..]
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_')
        {
            // Try docblock annotation first (@var / @param).
            if let Some(raw) = docblock::find_iterable_raw_type_in_source(
                ctx.content,
                ctx.cursor_offset as usize,
                arg_text,
            ) {
                return Some(raw);
            }
            // Fall back to the unified variable resolution pipeline.
            let default_class = ClassInfo::default();
            let effective_class = current_class.unwrap_or(&default_class);
            let resolved = crate::completion::variable::resolution::resolve_variable_types(
                arg_text,
                effective_class,
                all_classes,
                ctx.content,
                ctx.cursor_offset,
                class_loader,
                Loaders::with_function(ctx.function_loader),
            );
            if !resolved.is_empty() {
                return Some(ResolvedType::types_joined(&resolved));
            }
            return None;
        }

        // ── Call expression ending with `)` ─────────────────────────────
        if arg_text.ends_with(')')
            && let Some((call_body, _args)) = split_call_subject(arg_text)
        {
            match SubjectExpr::parse_callee(call_body) {
                SubjectExpr::MethodCall { base, method } => {
                    let base_text = base.to_subject_text();
                    let lhs_classes = ResolvedType::into_arced_classes(
                        super::resolver::resolve_target_classes(&base_text, AccessKind::Arrow, ctx),
                    );
                    for cls in &lhs_classes {
                        if let Some(rt) = crate::inheritance::resolve_method_return_type(
                            cls,
                            &method,
                            class_loader,
                        ) {
                            return Some(rt);
                        }
                    }
                }
                SubjectExpr::StaticMethodCall { class, method } => {
                    let owner = if let Some(resolved) = resolve_class_keyword(&class, current_class)
                    {
                        class_loader(&resolved).map(Arc::unwrap_or_clone)
                    } else {
                        find_class_by_name(all_classes, &class)
                            .map(|arc| ClassInfo::clone(arc))
                            .or_else(|| class_loader(&class).map(Arc::unwrap_or_clone))
                    };
                    if let Some(ref cls) = owner
                        && let Some(rt) = crate::inheritance::resolve_method_return_type(
                            cls,
                            &method,
                            class_loader,
                        )
                    {
                        return Some(rt);
                    }
                }
                _ => {}
            }
        }

        // ── Property access: `$this->prop` or `$var->prop` ──────────────
        if let Some(pos) = arg_text.rfind("->") {
            // Strip trailing `?` from LHS when the operator was `?->`
            let lhs = arg_text[..pos]
                .strip_suffix('?')
                .unwrap_or(&arg_text[..pos]);
            let prop_name = &arg_text[pos + 2..];
            if !prop_name.is_empty() && prop_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let lhs_classes = ResolvedType::into_arced_classes(
                    super::resolver::resolve_target_classes(lhs, AccessKind::Arrow, ctx),
                );
                for cls in &lhs_classes {
                    if let Some(rt) =
                        crate::inheritance::resolve_property_type_hint(cls, prop_name, class_loader)
                    {
                        return Some(rt);
                    }
                }
            }
        }

        None
    }
}
