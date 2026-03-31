/// Call expression and callable target resolution.
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
///   call-site argument text.
use std::collections::HashMap;
use std::sync::Arc;

use crate::Backend;
use crate::completion::variable::{ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS};
use crate::docblock;
use crate::php_type::PhpType;
use crate::types::*;
use crate::util::{find_class_at_offset, position_to_offset};

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

impl Backend {
    /// Resolve an instance method base expression + method name to a
    /// [`ResolvedCallableTarget`].
    ///
    /// Resolves `base` to owner classes, merges each via
    /// `resolve_class_fully`, and returns the first match for
    /// `method_name`.
    fn resolve_instance_method_callable(
        base: &SubjectExpr,
        method_name: &str,
        rctx: &ResolutionCtx<'_>,
    ) -> Option<ResolvedCallableTarget> {
        let subject_text = base.to_subject_text();
        let owner_classes: Vec<Arc<ClassInfo>> = if base.is_self_like() {
            rctx.current_class
                .map(|c| Arc::new(c.clone()))
                .into_iter()
                .collect()
        } else {
            super::resolver::resolve_target_classes(&subject_text, crate::AccessKind::Arrow, rctx)
        };

        for owner in &owner_classes {
            // `resolve_target_classes` already returns fully-resolved
            // classes (via `type_hint_to_classes` which calls
            // `resolve_class_fully` and injects model-specific scope
            // methods).  Check the candidate directly first so that
            // model-specific members (e.g. Eloquent scope methods
            // injected onto Builder<Model>) are found even when the
            // FQN-keyed resolved_class_cache holds a stale or
            // differently-scoped entry for the same base class.
            if let Some(m) = owner
                .methods
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(method_name))
            {
                // Skip #[Scope]-attributed methods on unresolved
                // classes — the raw parsed method still has the
                // `$query` parameter that the virtual member provider
                // strips.  Fall through to full resolution so the
                // synthesized scope method is used instead.
                if !m.has_scope_attribute {
                    return Some(ResolvedCallableTarget {
                        parameters: m.parameters.clone(),
                        return_type: m.return_type.clone(),
                    });
                }
            }

            // Fall back to full resolution for candidates that were
            // produced by a path that skips full resolution (e.g.
            // bare class name lookup).
            let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                owner,
                rctx.class_loader,
                rctx.resolved_class_cache,
            );
            if let Some(m) = merged
                .methods
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(method_name))
            {
                return Some(ResolvedCallableTarget {
                    parameters: m.parameters.clone(),
                    return_type: m.return_type.clone(),
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
    ) -> Option<ResolvedCallableTarget> {
        let owner = super::resolver::resolve_static_owner_class(class, rctx)?;
        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
            &owner,
            rctx.class_loader,
            rctx.resolved_class_cache,
        );
        let m = merged
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(method_name))?;
        Some(ResolvedCallableTarget {
            parameters: m.parameters.clone(),
            return_type: m.return_type.clone(),
        })
    }

    /// Build a [`ResolvedCallableTarget`] from a resolved [`FunctionInfo`].
    fn function_to_callable(func: &FunctionInfo) -> ResolvedCallableTarget {
        ResolvedCallableTarget {
            parameters: func.parameters.clone(),
            return_type: func.return_type.clone(),
        }
    }

    /// Resolve class name keywords (`self`, `static`, `parent`) to actual
    /// class names in the context of the current class.
    fn resolve_class_name_keyword(class_name: &str, current_class: Option<&ClassInfo>) -> String {
        match class_name {
            "self" | "static" => current_class
                .map(|c| c.name.clone())
                .unwrap_or_else(|| class_name.to_string()),
            "parent" => current_class
                .and_then(|c| c.parent_class.clone())
                .unwrap_or_else(|| class_name.to_string()),
            _ => class_name.to_string(),
        }
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
        };

        let parsed = SubjectExpr::parse(expr);

        // Unwrap `CallExpr` wrapper so downstream arms match the inner
        // callee directly.  The `args_text` is unused here (it matters
        // for return-type resolution, not for callable target lookup).
        let effective = match &parsed {
            SubjectExpr::CallExpr { callee, .. } => callee.as_ref(),
            other => other,
        };

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
                Self::resolve_instance_method_callable(base, method, &rctx)
            }

            // ── Static method call: `Class::method(…)` ──────────────
            SubjectExpr::StaticMethodCall { class, method } => {
                Self::resolve_static_method_callable(class, method, &rctx)
            }

            // ── Standalone function call: `functionName(…)` ─────────
            SubjectExpr::FunctionCall(name) => {
                let func =
                    self.resolve_function_name(name, &file_ctx.use_map, &file_ctx.namespace)?;
                Some(Self::function_to_callable(&func))
            }

            // ── Variable used as a callable target: `$fn(…)` ────────
            // Check for a first-class callable assignment and recurse.
            SubjectExpr::Variable(var_name) => {
                let callable_target =
                    Self::extract_callable_target_from_variable(var_name, content, cursor_offset)?;
                self.resolve_callable_target(&callable_target, content, position, file_ctx)
            }

            // ── Bare class name used as a function name ─────────────
            // Named-arg and signature-help contexts pass bare function
            // names like `"foo"` which `SubjectExpr::parse` produces
            // as `ClassName` (since it can't distinguish class names
            // from function names without context).
            SubjectExpr::ClassName(name) => {
                let func =
                    self.resolve_function_name(name, &file_ctx.use_map, &file_ctx.namespace)?;
                Some(Self::function_to_callable(&func))
            }

            // ── PropertyChain used as a callable target ──────────────
            // Named-arg and signature-help contexts pass expressions
            // like `"$this->method"` (without trailing `()`), which
            // `SubjectExpr::parse` produces as `PropertyChain`.  Treat
            // the trailing property as a method name.
            SubjectExpr::PropertyChain { base, property } => {
                Self::resolve_instance_method_callable(base, property, &rctx)
            }

            // ── StaticAccess used as a callable target ──────────────
            // Same situation: `"ClassName::method"` without `()` parses
            // as `StaticAccess` rather than `StaticMethodCall`.
            SubjectExpr::StaticAccess { class, member } => {
                Self::resolve_static_method_callable(class, member, &rctx)
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
    pub(crate) fn resolve_call_return_types_expr(
        callee: &SubjectExpr,
        text_args: &str,
        ctx: &ResolutionCtx<'_>,
    ) -> Vec<Arc<ClassInfo>> {
        match callee {
            // ── Instance method call: base->method(…) ───────────────
            SubjectExpr::MethodCall { base, method } => {
                let method_name = method.as_str();

                // Resolve the base expression to class(es).
                let lhs_classes: Vec<Arc<ClassInfo>> =
                    super::resolver::resolve_target_classes_expr(base, AccessKind::Arrow, ctx);

                let mut results = Vec::new();
                for owner in &lhs_classes {
                    let template_subs = if !text_args.is_empty() {
                        Self::build_method_template_subs(owner, method_name, text_args, ctx)
                    } else {
                        HashMap::new()
                    };
                    let var_resolver = build_var_resolver(ctx);
                    let mr_ctx = MethodReturnCtx {
                        all_classes: ctx.all_classes,
                        class_loader: ctx.class_loader,
                        template_subs: &template_subs,
                        var_resolver: Some(&var_resolver),
                        cache: ctx.resolved_class_cache,
                        calling_class_name: ctx.current_class.map(|c| c.name.as_str()),
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
                    super::resolver::resolve_target_classes(class, AccessKind::DoubleColon, ctx)
                        .into_iter()
                        .next()
                } else {
                    super::resolver::resolve_static_owner_class(class, ctx)
                };

                if let Some(ref owner) = owner_class {
                    let template_subs = if !text_args.is_empty() {
                        Self::build_method_template_subs(owner, method_name, text_args, ctx)
                    } else {
                        HashMap::new()
                    };
                    let var_resolver = build_var_resolver(ctx);
                    let mr_ctx = MethodReturnCtx {
                        all_classes: ctx.all_classes,
                        class_loader: ctx.class_loader,
                        template_subs: &template_subs,
                        var_resolver: Some(&var_resolver),
                        cache: ctx.resolved_class_cache,
                        calling_class_name: ctx.current_class.map(|c| c.name.as_str()),
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
                        && let Some(element_type) =
                            crate::php_type::PhpType::parse(raw).extract_value_type(true)
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
                            )
                        } else {
                            resolve_conditional_without_args(cond, &func_info.parameters)
                        };
                        if let Some(ref ty) = resolved_type {
                            let classes: Vec<Arc<ClassInfo>> =
                                super::type_resolution::type_hint_to_classes(
                                    ty,
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
                    if !func_info.template_params.is_empty()
                        && !func_info.template_bindings.is_empty()
                        && func_info.return_type.is_some()
                        && !text_args.is_empty()
                    {
                        let subs = super::variable::rhs_resolution::build_function_template_subs(
                            &func_info, text_args, ctx,
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
                ) && let Some(ret_type) =
                    crate::php_type::PhpType::parse(&raw_type).callable_return_type()
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
                        super::type_resolution::type_hint_to_classes(
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
                        super::type_resolution::type_hint_to_classes(
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
                let var_classes =
                    super::resolver::resolve_target_classes(var_name, AccessKind::Arrow, ctx);
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
                let callee_classes =
                    super::resolver::resolve_target_classes_expr(callee, AccessKind::Arrow, ctx);

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
                    )
                } else {
                    resolve_conditional_without_args_and_defaults(
                        cond,
                        &method.parameters,
                        Some(&class_info.template_param_defaults),
                    )
                };
                if let Some(ref ty) = resolved_type {
                    // Apply method-level template substitutions to the
                    // resolved conditional type (e.g. `TModel` → concrete
                    // class when TModel is a method-level @template param).
                    let parsed = PhpType::parse(ty);
                    let effective = if !template_subs.is_empty() {
                        parsed.substitute(template_subs)
                    } else {
                        parsed
                    };
                    let classes: Vec<Arc<ClassInfo>> =
                        super::type_resolution::type_hint_to_classes_typed(
                            &effective,
                            &class_info.name,
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
                            &class_info.name,
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
                // Match bare `self`/`static`/`$this` as well as generic
                // forms like `self<RuleError>`, `static<T>`, etc.
                let is_self_like = match ret {
                    PhpType::Named(n) => n == "static" || n == "self" || n == "$this",
                    PhpType::Generic(n, _) => n == "static" || n == "self" || n == "$this",
                    _ => false,
                };
                if is_self_like {
                    return vec![Arc::new(class_info.clone())];
                }
                return super::type_resolution::type_hint_to_classes_typed(
                    ret,
                    &class_info.name,
                    all_classes,
                    class_loader,
                )
                .into_iter()
                .map(Arc::new)
                .collect();
            }
            vec![]
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
        if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
            return resolve_method(method);
        }

        vec![]
    }

    /// Build a template substitution map for a method-level `@template` call.
    ///
    /// Finds the method on the class (or inherited), checks for template
    /// params and bindings, resolves argument types from `text_args` using
    /// the call resolution context, and returns a `HashMap` mapping template
    /// parameter names to their resolved concrete types.
    ///
    /// Returns an empty map if the method has no template params, no
    /// bindings, or if argument types cannot be resolved.
    pub(super) fn build_method_template_subs(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
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
            Some(m) if !m.template_params.is_empty() && !m.template_bindings.is_empty() => m,
            _ => return HashMap::new(),
        };

        let args = split_text_args(text_args);
        let mut subs = HashMap::new();

        for (tpl_name, param_name) in &method.template_bindings {
            // Find the parameter index for this binding.
            let param_idx = match method.parameters.iter().position(|p| p.name == *param_name) {
                Some(idx) => idx,
                None => continue,
            };

            // Get the corresponding argument text.
            let arg_text = match args.get(param_idx) {
                Some(text) => text.trim(),
                None => continue,
            };

            // Try to resolve the argument text to a type name.
            if let Some(type_name) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                subs.insert(tpl_name.clone(), PhpType::parse(&type_name));
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
    pub(crate) fn resolve_arg_text_to_type(
        arg_text: &str,
        ctx: &ResolutionCtx<'_>,
    ) -> Option<String> {
        let trimmed = arg_text.trim();

        // ClassName::class → ClassName
        if let Some(name) = trimmed.strip_suffix("::class")
            && !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
        {
            return Some(name.to_string());
        }

        // new ClassName(…) → ClassName
        if let Some(class_name) = super::source::helpers::extract_new_expression_class(trimmed) {
            return Some(class_name);
        }

        // $this / self / static → current class
        if trimmed == "$this" || trimmed == "self" || trimmed == "static" {
            return ctx.current_class.map(|c| c.name.clone());
        }

        // $this->prop → property type
        if let Some(prop) = trimmed
            .strip_prefix("$this->")
            .or_else(|| trimmed.strip_prefix("$this?->"))
            && prop.chars().all(|c| c.is_alphanumeric() || c == '_')
            && let Some(owner) = ctx.current_class
        {
            let types = super::type_resolution::resolve_property_types(
                prop,
                owner,
                ctx.all_classes,
                ctx.class_loader,
            );
            if let Some(first) = types.first() {
                return Some(first.name.clone());
            }
        }

        // $var → resolve variable type
        if trimmed.starts_with('$') {
            let classes = super::resolver::resolve_target_classes(
                trimmed,
                crate::types::AccessKind::Arrow,
                ctx,
            );
            if let Some(first) = classes.first() {
                return Some(first.name.clone());
            }
        }

        None
    }

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

    /// Resolve the raw return type string of an inline argument expression.
    ///
    /// Handles plain variables (`$customers`), call chains
    /// (`Customer::get()->all()`), and static calls (`ClassName::method()`).
    ///
    /// Returns the raw type string (e.g. `"array<int, Customer>"`) so
    /// that the caller can extract element types from it.
    fn resolve_inline_arg_raw_type(arg_text: &str, ctx: &ResolutionCtx<'_>) -> Option<String> {
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
                return Some(ResolvedType::type_strings_joined(&resolved));
            }
            return None;
        }

        // ── Call expression ending with `)` ─────────────────────────────
        if arg_text.ends_with(')')
            && let Some((call_body, _args)) = split_call_subject(arg_text)
        {
            // Instance method chain: `expr->method()`
            if let Some(pos) = call_body.rfind("->") {
                // Strip trailing `?` from LHS when the operator was `?->`
                let lhs = call_body[..pos]
                    .strip_suffix('?')
                    .unwrap_or(&call_body[..pos]);
                let method_name = &call_body[pos + 2..];

                let lhs_classes =
                    super::resolver::resolve_target_classes(lhs, AccessKind::Arrow, ctx);
                for cls in &lhs_classes {
                    if let Some(rt) = crate::inheritance::resolve_method_return_type(
                        cls,
                        method_name,
                        class_loader,
                    ) {
                        return Some(rt.to_string());
                    }
                }
            }

            // Static call: `ClassName::method()`
            if let Some(pos) = call_body.rfind("::") {
                let class_part = &call_body[..pos];
                let method_name = &call_body[pos + 2..];

                let owner = if class_part == "self" || class_part == "static" {
                    current_class.cloned()
                } else {
                    find_class_by_name(all_classes, class_part)
                        .map(|arc| ClassInfo::clone(arc))
                        .or_else(|| class_loader(class_part).map(Arc::unwrap_or_clone))
                };
                if let Some(ref cls) = owner
                    && let Some(rt) = crate::inheritance::resolve_method_return_type(
                        cls,
                        method_name,
                        class_loader,
                    )
                {
                    return Some(rt.to_string());
                }
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
                let lhs_classes =
                    super::resolver::resolve_target_classes(lhs, AccessKind::Arrow, ctx);
                for cls in &lhs_classes {
                    if let Some(rt) =
                        crate::inheritance::resolve_property_type_hint(cls, prop_name, class_loader)
                            .map(|t| t.to_string())
                    {
                        return Some(rt);
                    }
                }
            }
        }

        None
    }
}
