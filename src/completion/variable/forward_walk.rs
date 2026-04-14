/// Forward-walking scope model for variable type resolution.
///
/// This module implements a single top-to-bottom pass through a function
/// or method body, maintaining a mutable type map (`ScopeState`) that
/// records each variable's type as assignments are encountered.  When the
/// walk reaches the cursor position it stops and the caller reads the
/// target variable's type from the map — an O(1) `HashMap` lookup with
/// zero recursion.
///
/// # Architecture
///
/// The old backward scanner (now removed) resolved one variable at a
/// time by walking backward from the cursor, recursively calling itself
/// for each RHS variable reference.  That caused O(depth × file_size)
/// work per variable lookup.
///
/// This forward walker replaces that recursion with a single forward pass:
///
/// 1. Seed `ScopeState` with parameter types.
/// 2. Walk statements top-to-bottom.  At each assignment `$a = expr`,
///    evaluate `expr` by reading other variables from the scope (O(1)
///    map lookups) and store the result under `$a`.
/// 3. At the cursor, read the target variable from the scope.
///
/// There is no recursion on variable resolution, no depth limit, and
/// every variable resolved during the walk is available to subsequent
/// statements for free.
///
/// # Phases
///
/// - **Phase 1** (completion): wired into the completion path.  The
///   forward walker is called per-request with `cursor_offset` set to
///   the cursor position.  Only the target variable's type is read.
/// - **Phase 2** (diagnostics): [`build_diagnostic_scopes`] walks every
///   function/method body in the file once (`cursor_offset = u32::MAX`)
///   and records scope snapshots at each statement boundary in a
///   thread-local [`DIAGNOSTIC_SCOPE`] cache.  When
///   `resolve_variable_types` is called for a diagnostic span, it
///   checks the cache first via [`lookup_diagnostic_scope`] and returns
///   the pre-computed types in O(log N) time, eliminating the
///   O(N x depth x file_size) cost of per-span backward scanning.
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::argument::Argument;
use mago_syntax::ast::sequence::TokenSeparatedSequence;
use mago_syntax::ast::*;

use crate::completion::resolver::{Loaders, VarResolutionCtx};
use crate::completion::types::narrowing;
use crate::parser::{extract_hint_type, with_parsed_program};
use crate::php_type::PhpType;
use crate::types::{AccessKind, ClassInfo, ResolvedType};

// ─── Diagnostic scope cache (Phase 2) ───────────────────────────────────────
//
// During a diagnostic pass, `build_diagnostic_scopes` walks every
// function/method body in the file once and records a scope snapshot at
// each statement boundary.  The snapshots are stored in a thread-local
// `BTreeMap<u32, HashMap<String, Vec<ResolvedType>>>` keyed by byte
// offset.  When `resolve_variable_types` is called for a diagnostic
// member-access span, `lookup_diagnostic_scope` finds the nearest
// snapshot at-or-before the requested offset and returns the variable's
// types in O(log N) time — no backward scanning, no recursion.

/// Scope snapshot map: byte offset → variable name → resolved types.
type ScopeSnapshotMap = BTreeMap<u32, HashMap<String, Vec<ResolvedType>>>;

thread_local! {
    /// When `Some`, `lookup_diagnostic_scope` will consult this map.
    /// Activated by [`with_diagnostic_scope_cache`], cleared on guard
    /// drop.
    static DIAGNOSTIC_SCOPE: RefCell<Option<ScopeSnapshotMap>> =
        const { RefCell::new(None) };

    /// Set to `true` while `build_diagnostic_scopes` is populating the
    /// scope cache.  Code that would normally read from the cache should
    /// skip the lookup when this flag is set, because the cache is
    /// incomplete and may contain stale data from earlier offsets.
    static BUILDING_SCOPES: Cell<bool> = const { Cell::new(false) };
}

/// RAII guard that clears the diagnostic scope cache on drop.
pub(crate) struct DiagnosticScopeGuard {
    owns: bool,
}

impl Drop for DiagnosticScopeGuard {
    fn drop(&mut self) {
        if self.owns {
            DIAGNOSTIC_SCOPE.with(|cell| {
                *cell.borrow_mut() = None;
            });
        }
    }
}

/// RAII guard that resets [`BUILDING_SCOPES`] to `false` on drop.
pub(crate) struct BuildingScopesGuard;

impl Drop for BuildingScopesGuard {
    fn drop(&mut self) {
        BUILDING_SCOPES.with(|cell: &Cell<bool>| cell.set(false));
    }
}

/// Returns `true` while `build_diagnostic_scopes` is populating the
/// scope cache.
pub(crate) fn is_building_scopes() -> bool {
    BUILDING_SCOPES.with(|cell: &Cell<bool>| cell.get())
}

/// Activate the thread-local diagnostic scope cache.
///
/// Returns a guard that clears the cache on drop.  If the cache is
/// already active (nested call), the guard is a no-op.
pub(crate) fn with_diagnostic_scope_cache() -> DiagnosticScopeGuard {
    let already_active = DIAGNOSTIC_SCOPE.with(|cell| cell.borrow().is_some());
    if already_active {
        return DiagnosticScopeGuard { owns: false };
    }
    DIAGNOSTIC_SCOPE.with(|cell| {
        *cell.borrow_mut() = Some(BTreeMap::new());
    });
    DiagnosticScopeGuard { owns: true }
}

/// Look up a variable's types from the diagnostic scope cache.
///
/// Finds the scope snapshot at the largest offset that is ≤ `offset`,
/// then returns the variable's types from that snapshot.  Returns
/// `None` when the cache is not active or no snapshot covers the
/// requested offset.
pub(crate) fn lookup_diagnostic_scope(var_name: &str, offset: u32) -> Option<Vec<ResolvedType>> {
    DIAGNOSTIC_SCOPE.with(|cell| {
        let borrow = cell.borrow();
        let map = borrow.as_ref()?;
        // Find the snapshot at-or-before `offset`.
        let (_snap_offset, snap) = map.range(..=offset).next_back()?;
        // If the variable is in the snapshot, return its types.
        // If the snapshot exists but the variable is absent, the
        // forward walker has already walked this scope region and
        // determined the variable has no known type here.  Return
        // empty rather than `None` so the caller treats the variable
        // as unresolved at this position.
        let result = snap.get(var_name).cloned().unwrap_or_default();
        Some(result)
    })
}

/// Check whether the diagnostic scope cache is currently active.
pub(crate) fn is_diagnostic_scope_active() -> bool {
    DIAGNOSTIC_SCOPE.with(|cell| cell.borrow().is_some())
}

/// Insert a scope snapshot into the diagnostic scope cache at the given
/// byte offset.
fn record_scope_snapshot(offset: u32, scope: &ScopeState) {
    DIAGNOSTIC_SCOPE.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(ref mut map) = *borrow {
            map.insert(offset, scope.locals.clone());
        }
    });
}

/// Walk a sequence of statements for diagnostic scope building.
///
/// Unlike [`walk_body_forward`] (which stops at the cursor), this walks
/// the **entire** body and records a scope snapshot at every statement
/// boundary.  The snapshots are stored in the thread-local
/// [`DIAGNOSTIC_SCOPE`] cache.
///
/// For each statement, this also discovers closure and arrow function
/// expressions and walks their bodies with properly seeded scopes so
/// that variables inside closures are fully resolved.
fn walk_body_for_diagnostics<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    for stmt in statements {
        // Record the scope at this statement's start offset so that
        // any member-access span inside this statement can look up
        // variable types that were established by prior statements.
        record_scope_snapshot(stmt.span().start.offset, scope);

        process_statement(stmt, scope, ctx);

        // Walk closure and arrow function bodies found in this
        // statement.  Each closure gets a fresh scope seeded with
        // `use()` variables from the outer scope and its own parameter
        // types (with callable inference from the enclosing call's
        // signature).  Arrow functions inherit the outer scope with
        // their parameter types added on top.  The body is fully
        // walked so that scope snapshots are recorded for every
        // statement inside the closure/arrow function.
        walk_closures_in_statement(stmt, scope, ctx);

        // Also record at the statement's end offset, which covers
        // member accesses that appear after the last statement in
        // a block (e.g. the closing `}` region).
        record_scope_snapshot(stmt.span().end.offset, scope);
    }
}

/// Scan a statement for closure/arrow function expressions and walk
/// their bodies with properly seeded scopes.
///
/// For each closure found, a fresh scope is created and seeded with
/// `use()` variables from the outer scope plus the closure's own
/// parameters.  For each arrow function, the outer scope is cloned
/// and the arrow's parameters are added on top.  The body is then
/// fully walked so that scope snapshots are recorded for every
/// statement inside the closure/arrow function.
/// Scan a statement for closure/arrow function expressions and record
/// scope "shadow" snapshots inside their bodies.
///
/// When a closure/arrow function parameter shadows an outer variable
/// (e.g. `fn(Request $request)` where the outer scope has a different
/// `$request`), the scope cache would return the outer type for lookups
/// inside the closure body.  We fix this by recording a scope snapshot
/// at the closure body's start offset that removes shadowed variables.
/// The scope cache lookup then returns `None` for those variables,
/// calling `resolve_variable_types` which would re-enter the forward
/// walker.
///
/// This approach avoids walking the entire closure body (which would
/// override `resolve_variable_types` for ALL variables, including those
/// from foreach bindings over generic collections where the outer
/// resolver produces better types).
/// Scan a **single** statement's direct expressions for closure/arrow
/// function literals and walk their bodies with properly seeded scopes.
///
/// This function intentionally does **not** recurse into nested block
/// bodies (if/while/foreach/try/switch).  Those bodies are walked by
/// [`walk_body_forward`], which calls this function for each statement
/// it processes — at that point the scope already reflects narrowing,
/// foreach bindings, and other context from the enclosing block.
///
/// Only the expressions that are directly part of this statement (the
/// condition expression, the iteration expression, echo values, etc.)
/// are scanned for closures.  Closures inside nested block bodies will
/// be picked up when `walk_body_forward` processes the inner statements.
fn walk_closures_in_statement<'b>(
    stmt: &'b Statement<'b>,
    outer_scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    match stmt {
        Statement::Expression(expr_stmt) => {
            walk_closures_in_expr(expr_stmt.expression, outer_scope, ctx, None);
        }
        Statement::Return(ret) => {
            if let Some(val) = ret.value {
                walk_closures_in_expr(val, outer_scope, ctx, None);
            }
        }
        Statement::Echo(echo) => {
            for val in echo.values.iter() {
                walk_closures_in_expr(val, outer_scope, ctx, None);
            }
        }
        // For compound statements (if, while, foreach, etc.) only scan
        // the condition/iteration expression — not the block bodies.
        // Block bodies are walked by walk_body_forward which calls us
        // per-statement with the correct inner scope.
        Statement::If(if_stmt) => {
            walk_closures_in_expr(if_stmt.condition, outer_scope, ctx, None);
        }
        Statement::While(while_stmt) => {
            walk_closures_in_expr(while_stmt.condition, outer_scope, ctx, None);
        }
        Statement::DoWhile(dw) => {
            walk_closures_in_expr(dw.condition, outer_scope, ctx, None);
        }
        Statement::Foreach(foreach) => {
            walk_closures_in_expr(foreach.expression, outer_scope, ctx, None);
        }
        Statement::For(for_stmt) => {
            for init in for_stmt.initializations.iter() {
                walk_closures_in_expr(init, outer_scope, ctx, None);
            }
            for cond in for_stmt.conditions.iter() {
                walk_closures_in_expr(cond, outer_scope, ctx, None);
            }
            for update in for_stmt.increments.iter() {
                walk_closures_in_expr(update, outer_scope, ctx, None);
            }
        }
        Statement::Switch(switch) => {
            walk_closures_in_expr(switch.expression, outer_scope, ctx, None);
        }
        _ => {}
    }
}

/// Recursively scan an expression tree for closures/arrow functions
/// and walk their bodies with properly seeded scopes.
///
/// When a closure/arrow function is found:
/// 1. Build a scope for the closure body (fresh for closures, cloned
///    from outer for arrow functions).
/// 2. Seed the scope with parameter types (using callable inference
///    from the enclosing call's signature when parameters are untyped).
/// 3. Walk the body using [`walk_body_for_diagnostics`] so that scope
///    snapshots are recorded at every statement boundary.
///
/// The `inferred_params` argument carries callable parameter types
/// inferred from the enclosing call's signature.  When a closure is
/// found as a direct argument to a function/method call, the caller
/// passes the inferred types so untyped parameters get the correct
/// types.
fn walk_closures_in_expr<'b>(
    expr: &'b Expression<'b>,
    outer_scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inferred_params: Option<&[PhpType]>,
) {
    match expr {
        Expression::Closure(closure) => {
            // Build a fresh scope for the closure.
            let mut closure_scope = ScopeState::new();

            // PHP closures implicitly capture `$this` from the
            // enclosing class method.  Seed it from the outer scope
            // so that `$this->prop` inside the closure resolves
            // without calling `resolve_variable_types`.
            let this_types = outer_scope.get("$this");
            if !this_types.is_empty() {
                closure_scope.set("$this".to_string(), this_types.to_vec());
            }

            // Seed with `use(...)` variables from the outer scope.
            if let Some(ref use_clause) = closure.use_clause {
                for use_var in use_clause.variables.iter() {
                    let var_name = use_var.variable.name.to_string();
                    let from_outer = outer_scope.get(&var_name);
                    if !from_outer.is_empty() {
                        closure_scope.set(var_name, from_outer.to_vec());
                    }
                }
            }

            // Seed with parameter types, using callable inference when
            // available.  Filter out any inferred params whose base
            // type is unresolvable (e.g. PHPStan pseudo-types like
            // `collection-of<T>`) so they don't poison the scope —
            // the param simply won't be seeded, which is better than
            // skipping the entire closure body.
            let inferred = inferred_params.unwrap_or(&[]);
            let filtered_inferred = filter_resolvable_inferred_params(inferred, ctx);
            seed_closure_params(
                &mut closure_scope,
                &closure.parameter_list,
                closure.span().start.offset,
                &filtered_inferred,
                ctx,
            );

            // Record the scope at the body start.
            let body_span = closure.body.span();
            record_scope_snapshot(body_span.start.offset, &closure_scope);

            // Walk the closure body.
            walk_body_for_diagnostics(closure.body.statements.iter(), &mut closure_scope, ctx);

            // Record at body end (closure scope).
            record_scope_snapshot(body_span.end.offset, &closure_scope);

            // Restore the outer scope immediately after the closure body
            // so that code following the closure in the same expression
            // (e.g. `->where('product_id', $product->id)` after a
            // `whereHas(function (Builder $q) { ... })`) sees the outer
            // scope's variables, not the closure's.
            record_scope_snapshot(body_span.end.offset + 1, outer_scope);
        }
        Expression::ArrowFunction(arrow) => {
            // Arrow functions inherit the enclosing scope.
            let mut arrow_scope = outer_scope.clone();

            // Seed with parameter types, using callable inference when
            // available.
            let inferred = inferred_params.unwrap_or(&[]);
            let filtered_inferred = filter_resolvable_inferred_params(inferred, ctx);
            seed_closure_params(
                &mut arrow_scope,
                &arrow.parameter_list,
                arrow.span().start.offset,
                &filtered_inferred,
                ctx,
            );

            // Record the scope at the body expression.
            let body_span = arrow.expression.span();
            record_scope_snapshot(body_span.start.offset, &arrow_scope);
            record_scope_snapshot(body_span.end.offset, &arrow_scope);

            // Restore the outer scope after the arrow body (same
            // reasoning as for closures above).
            record_scope_snapshot(body_span.end.offset + 1, outer_scope);

            // Recurse into the body expression for nested closures.
            walk_closures_in_expr(arrow.expression, &arrow_scope, ctx, None);
        }
        // For call expressions, try to infer callable parameter types
        // from the function/method signature before recursing into
        // the arguments.
        Expression::Call(call) => {
            walk_closures_in_call(call, outer_scope, ctx);
        }
        // Recurse into sub-expressions that may contain closures.
        Expression::Parenthesized(inner) => {
            walk_closures_in_expr(inner.expression, outer_scope, ctx, None);
        }
        Expression::Assignment(assignment) => {
            walk_closures_in_expr(assignment.rhs, outer_scope, ctx, None);
        }
        Expression::Access(access) => match access {
            Access::Property(pa) => {
                walk_closures_in_expr(pa.object, outer_scope, ctx, None);
            }
            Access::NullSafeProperty(pa) => {
                walk_closures_in_expr(pa.object, outer_scope, ctx, None);
            }
            _ => {}
        },
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                let elem_expr = match elem {
                    ArrayElement::KeyValue(kv) => {
                        walk_closures_in_expr(kv.key, outer_scope, ctx, None);
                        kv.value
                    }
                    ArrayElement::Value(val) => val.value,
                    ArrayElement::Variadic(v) => v.value,
                    ArrayElement::Missing(_) => continue,
                };
                walk_closures_in_expr(elem_expr, outer_scope, ctx, None);
            }
        }
        Expression::LegacyArray(arr) => {
            for elem in arr.elements.iter() {
                let elem_expr = match elem {
                    ArrayElement::KeyValue(kv) => {
                        walk_closures_in_expr(kv.key, outer_scope, ctx, None);
                        kv.value
                    }
                    ArrayElement::Value(val) => val.value,
                    ArrayElement::Variadic(v) => v.value,
                    ArrayElement::Missing(_) => continue,
                };
                walk_closures_in_expr(elem_expr, outer_scope, ctx, None);
            }
        }
        Expression::Binary(bin) => {
            walk_closures_in_expr(bin.lhs, outer_scope, ctx, None);
            walk_closures_in_expr(bin.rhs, outer_scope, ctx, None);
        }
        Expression::UnaryPrefix(prefix) => {
            walk_closures_in_expr(prefix.operand, outer_scope, ctx, None);
        }
        Expression::Conditional(cond) => {
            walk_closures_in_expr(cond.condition, outer_scope, ctx, None);
            if let Some(then_expr) = cond.then {
                walk_closures_in_expr(then_expr, outer_scope, ctx, None);
            }
            walk_closures_in_expr(cond.r#else, outer_scope, ctx, None);
        }
        Expression::Match(m) => {
            walk_closures_in_expr(m.expression, outer_scope, ctx, None);
            for arm in m.arms.iter() {
                walk_closures_in_expr(arm.expression(), outer_scope, ctx, None);
            }
        }
        Expression::Instantiation(inst) => {
            if let Some(ref args) = inst.argument_list {
                walk_closures_in_call_args(&args.arguments, outer_scope, ctx, |_| vec![]);
            }
        }
        Expression::Yield(y) => match y {
            Yield::Value(yv) => {
                if let Some(val) = &yv.value {
                    walk_closures_in_expr(val, outer_scope, ctx, None);
                }
            }
            Yield::Pair(yp) => {
                walk_closures_in_expr(yp.key, outer_scope, ctx, None);
                walk_closures_in_expr(yp.value, outer_scope, ctx, None);
            }
            Yield::From(yf) => {
                walk_closures_in_expr(yf.iterator, outer_scope, ctx, None);
            }
        },
        Expression::Throw(t) => {
            walk_closures_in_expr(t.exception, outer_scope, ctx, None);
        }
        Expression::Clone(c) => {
            walk_closures_in_expr(c.object, outer_scope, ctx, None);
        }
        Expression::Pipe(p) => {
            walk_closures_in_expr(p.input, outer_scope, ctx, None);
            walk_closures_in_expr(p.callable, outer_scope, ctx, None);
        }
        _ => {}
    }
}

/// Handle a call expression: infer callable parameter types from the
/// function/method signature and pass them when walking closure arguments.
fn walk_closures_in_call<'b>(
    call: &'b Call<'b>,
    outer_scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    match call {
        Call::Function(fc) => {
            // Recurse into the function expression (for closures in
            // chained calls like `$fn()($anotherClosure)`).
            walk_closures_in_expr(fc.function, outer_scope, ctx, None);

            let func_name = match fc.function {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                _ => None,
            };
            walk_closures_in_call_args(&fc.argument_list.arguments, outer_scope, ctx, |arg_idx| {
                if let Some(ref name) = func_name {
                    infer_callable_params_from_function_fw(
                        name,
                        arg_idx,
                        &fc.argument_list.arguments,
                        outer_scope,
                        ctx,
                    )
                } else {
                    vec![]
                }
            });
        }
        Call::Method(mc) => {
            walk_closures_in_expr(mc.object, outer_scope, ctx, None);

            let method_name = if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                Some(ident.value.to_string())
            } else {
                None
            };
            let obj_span = mc.object.span();
            let first_arg = extract_first_arg_string_fw(&mc.argument_list.arguments, ctx.content);
            walk_closures_in_call_args(&mc.argument_list.arguments, outer_scope, ctx, |arg_idx| {
                if let Some(ref name) = method_name {
                    infer_callable_params_from_receiver_fw(
                        obj_span.start.offset,
                        obj_span.end.offset,
                        name,
                        arg_idx,
                        first_arg.as_deref(),
                        outer_scope,
                        ctx,
                    )
                } else {
                    vec![]
                }
            });
        }
        Call::NullSafeMethod(mc) => {
            walk_closures_in_expr(mc.object, outer_scope, ctx, None);

            let method_name = if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                Some(ident.value.to_string())
            } else {
                None
            };
            let obj_span = mc.object.span();
            let first_arg = extract_first_arg_string_fw(&mc.argument_list.arguments, ctx.content);
            walk_closures_in_call_args(&mc.argument_list.arguments, outer_scope, ctx, |arg_idx| {
                if let Some(ref name) = method_name {
                    infer_callable_params_from_receiver_fw(
                        obj_span.start.offset,
                        obj_span.end.offset,
                        name,
                        arg_idx,
                        first_arg.as_deref(),
                        outer_scope,
                        ctx,
                    )
                } else {
                    vec![]
                }
            });
        }
        Call::StaticMethod(sc) => {
            walk_closures_in_expr(sc.class, outer_scope, ctx, None);

            let method_name = if let ClassLikeMemberSelector::Identifier(ident) = &sc.method {
                Some(ident.value.to_string())
            } else {
                None
            };
            let first_arg = extract_first_arg_string_fw(&sc.argument_list.arguments, ctx.content);
            walk_closures_in_call_args(&sc.argument_list.arguments, outer_scope, ctx, |arg_idx| {
                if let Some(ref name) = method_name {
                    infer_callable_params_from_static_receiver_fw(
                        sc.class,
                        name,
                        arg_idx,
                        first_arg.as_deref(),
                        outer_scope,
                        ctx,
                    )
                } else {
                    vec![]
                }
            });
        }
    }
}

/// Walk the arguments of a call expression, invoking `infer_fn` for
/// each argument index to get inferred callable parameter types.
/// When an argument is a closure/arrow function, the inferred types
/// are passed through so untyped parameters get the correct types.
fn walk_closures_in_call_args<'b, F>(
    arguments: &'b TokenSeparatedSequence<'b, Argument<'b>>,
    outer_scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    infer_fn: F,
) where
    F: Fn(usize) -> Vec<PhpType>,
{
    for (arg_idx, arg) in arguments.iter().enumerate() {
        let arg_expr = match arg {
            Argument::Positional(a) => a.value,
            Argument::Named(a) => a.value,
        };
        match arg_expr {
            Expression::Closure(_) | Expression::ArrowFunction(_) => {
                let inferred = infer_fn(arg_idx);
                walk_closures_in_expr(
                    arg_expr,
                    outer_scope,
                    ctx,
                    if inferred.is_empty() {
                        None
                    } else {
                        Some(&inferred)
                    },
                );
            }
            _ => {
                walk_closures_in_expr(arg_expr, outer_scope, ctx, None);
            }
        }
    }
}

/// Seed a closure/arrow function scope with parameter types, using
/// inferred callable types as fallback for untyped parameters.
///
/// This mirrors [`seed_params`] but additionally accepts `inferred_types`
/// from the enclosing call's callable signature.  When a parameter has
/// no explicit type hint, the corresponding inferred type (matched by
/// positional index) is used instead.
fn seed_closure_params(
    scope: &mut ScopeState,
    parameter_list: &FunctionLikeParameterList<'_>,
    fn_span_start: u32,
    inferred_types: &[PhpType],
    ctx: &ForwardWalkCtx<'_>,
) {
    for (idx, param) in parameter_list.parameters.iter().enumerate() {
        let pname = param.variable.name.to_string();
        let is_variadic = param.ellipsis.is_some();

        let native_type = param.hint.as_ref().map(|h| extract_hint_type(h));

        // Check the `@param` docblock annotation.
        let raw_docblock_type = crate::docblock::find_iterable_raw_type_in_source(
            ctx.content,
            fn_span_start as usize,
            &pname,
        );

        let effective_type = crate::docblock::resolve_effective_type_typed(
            native_type.as_ref(),
            raw_docblock_type.as_ref(),
        );

        // Substitute method-level template params with their bounds.
        let effective_type = effective_type.map(|ty| {
            let ty = super::resolution::substitute_template_param_bounds(
                ty,
                ctx.content,
                fn_span_start as usize,
            );
            super::resolution::substitute_class_string_template_bounds(
                ty,
                ctx.content,
                fn_span_start as usize,
            )
        });

        let inferred_for_idx = inferred_types.get(idx);

        // When the explicit hint is a bare class name and the inferred
        // type is the same class WITH generic args, prefer the inferred
        // type (preserves template substitution).
        let use_inferred_over_explicit = if let Some(ref eff) = effective_type
            && let Some(inferred) = inferred_for_idx
        {
            super::closure_resolution::inferred_type_is_more_specific_pub(eff, inferred)
        } else {
            false
        };

        let mut param_results = if use_inferred_over_explicit {
            let pi = inferred_for_idx.unwrap();
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                pi,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            if !resolved.is_empty() {
                ResolvedType::from_classes_with_hint(resolved, pi.clone())
            } else if pi.is_informative() {
                // The inferred type is more specific (e.g.
                // `array<int, array<string, string>>` vs bare `array`)
                // but doesn't resolve to a class.  Preserve the type
                // string so the parameter is still seeded in scope.
                vec![ResolvedType::from_type_string(pi.clone())]
            } else {
                vec![]
            }
        } else if let Some(ref eff) = effective_type {
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                eff,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            if !resolved.is_empty() {
                // Check if inferred is a subtype and more specific.
                if let Some(inferred) = inferred_for_idx {
                    let inferred_resolved =
                        crate::completion::type_resolution::type_hint_to_classes_typed(
                            inferred,
                            &ctx.current_class.name,
                            ctx.all_classes,
                            ctx.class_loader,
                        );
                    if !inferred_resolved.is_empty()
                        && inferred_resolved.iter().all(|inferred_cls| {
                            resolved.iter().any(|explicit_cls| {
                                crate::util::is_subtype_of_names(
                                    &inferred_cls.fqn(),
                                    &explicit_cls.fqn(),
                                    ctx.class_loader,
                                )
                            })
                        })
                    {
                        ResolvedType::from_classes_with_hint(inferred_resolved, inferred.clone())
                    } else {
                        ResolvedType::from_classes_with_hint(resolved, eff.clone())
                    }
                } else {
                    ResolvedType::from_classes_with_hint(resolved, eff.clone())
                }
            } else {
                // The explicit hint didn't resolve to a class.
                // Try docblock for a richer type.
                if let Some(ref parsed_dt) = raw_docblock_type {
                    let doc_resolved =
                        crate::completion::type_resolution::type_hint_to_classes_typed(
                            parsed_dt,
                            &ctx.current_class.name,
                            ctx.all_classes,
                            ctx.class_loader,
                        );
                    if !doc_resolved.is_empty() {
                        ResolvedType::from_classes_with_hint(doc_resolved, parsed_dt.clone())
                    } else {
                        let best_type = raw_docblock_type
                            .clone()
                            .or_else(|| effective_type.clone())
                            .unwrap_or_else(PhpType::untyped);
                        vec![ResolvedType::from_type_string(best_type)]
                    }
                } else {
                    let best_type = effective_type.clone().unwrap_or_else(PhpType::untyped);
                    vec![ResolvedType::from_type_string(best_type)]
                }
            }
        } else if let Some(inferred) = inferred_for_idx {
            // No explicit type — use the inferred type from the
            // callable signature.
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                inferred,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            if !resolved.is_empty() {
                ResolvedType::from_classes_with_hint(resolved, inferred.clone())
            } else if inferred.is_informative() {
                vec![ResolvedType::from_type_string(inferred.clone())]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Variadic parameter wrapping.
        if is_variadic && !param_results.is_empty() {
            for rt in &mut param_results {
                rt.type_string = PhpType::list(rt.type_string.clone());
                rt.class_info = None;
            }
        }

        if !param_results.is_empty() {
            scope.seed(pname, param_results);
        }
    }
}

// ─── Callable parameter inference for the forward walker ────────────────────
//
// These functions mirror the inference logic in `closure_resolution.rs`
// but operate with a `ForwardWalkCtx` + `ScopeState` instead of a
// `VarResolutionCtx`.  They build a temporary `VarResolutionCtx` with
// a scope-based variable resolver injected so that variable lookups
// during receiver resolution read from the forward walker's scope.

/// Infer callable parameter types for a closure passed at position
/// `arg_idx` to a standalone function call.
fn infer_callable_params_from_function_fw(
    func_name: &str,
    arg_idx: usize,
    arguments: &TokenSeparatedSequence<'_, Argument<'_>>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    let scope_locals = &scope.locals;
    let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
        scope_locals.get(var_name).cloned().unwrap_or_default()
    };
    let var_ctx = ctx.var_ctx_for_with_scope("$__infer", ctx.cursor_offset, &scope_resolver);
    let rctx = var_ctx.as_resolution_ctx();
    let func_info = if let Some(fl) = rctx.function_loader {
        fl(func_name)
    } else {
        None
    };
    if let Some(fi) = func_info {
        let mut params = extract_callable_params_at_fw(&fi.parameters, arg_idx);

        if !params.is_empty() && !fi.template_params.is_empty() && !fi.template_bindings.is_empty()
        {
            let arg_texts = extract_argument_texts_fw(arguments, ctx.content);
            let subs = super::rhs_resolution::build_function_template_subs(&fi, &arg_texts, &rctx);
            if !subs.is_empty() {
                params = params.into_iter().map(|p| p.substitute(&subs)).collect();
            }
        }

        params
    } else {
        vec![]
    }
}

/// Infer callable parameter types for a closure passed at position
/// `arg_idx` to an instance method call.
fn infer_callable_params_from_receiver_fw(
    obj_start: u32,
    obj_end: u32,
    method_name: &str,
    arg_idx: usize,
    first_arg_text: Option<&str>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    let start = obj_start as usize;
    let end = obj_end as usize;
    if end > ctx.content.len() {
        return vec![];
    }
    let obj_text = ctx.content[start..end].trim();
    let scope_locals = &scope.locals;
    let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
        scope_locals.get(var_name).cloned().unwrap_or_default()
    };
    let var_ctx = ctx.var_ctx_for_with_scope("$__infer", obj_start, &scope_resolver);
    let rctx = var_ctx.as_resolution_ctx();
    // Keep the raw ResolvedTypes so we can extract generic args from
    // the receiver's type_string (e.g. `Builder<Product>` carries the
    // concrete `Product` arg that must substitute `TModel`).
    let resolved_types =
        crate::completion::resolver::resolve_target_classes(obj_text, AccessKind::Arrow, &rctx);
    let receiver_classes = ResolvedType::into_arced_classes(resolved_types.clone());

    // For relation-query methods (whereHas, etc.), override the closure
    // parameter type with Builder<RelatedModel>.
    if let Some(override_params) = super::closure_resolution::try_relation_query_override_pub(
        &receiver_classes,
        method_name,
        first_arg_text,
        ctx.class_loader,
    ) {
        return override_params;
    }

    let params = find_callable_params_on_classes_fw(&receiver_classes, method_name, arg_idx, ctx);

    // Build a template substitution map from the receiver's generic
    // args.  When the receiver resolves to e.g. `Builder<Product>`,
    // the type_string is `Generic("Builder", [Named("Product")])`.
    // We extract those args, pair them with the class's @template
    // params (e.g. `TModel`), and substitute so that callable params
    // like `Closure(Builder<TModel>)` become `Closure(Builder<Product>)`.
    let template_subs = build_receiver_template_subs(&resolved_types, &receiver_classes, ctx);

    // Apply template substitution, then replace `$this`/`static`
    // tokens with the receiver's full type.
    let params = if !template_subs.is_empty() {
        params
            .into_iter()
            .map(|p| p.substitute(&template_subs))
            .collect()
    } else {
        params
    };

    if let Some(receiver) = receiver_classes.first() {
        let receiver_type =
            super::closure_resolution::build_receiver_self_type_pub(receiver, ctx.class_loader);
        params
            .into_iter()
            .map(|p| p.replace_self_with_type(&receiver_type))
            .collect()
    } else {
        params
    }
}

/// Filter inferred callable param types, replacing any param whose type
/// has an unresolvable base (e.g. PHPStan pseudo-types like
/// `collection-of<T>`) with `PhpType::mixed()`.  `mixed` is not
/// considered informative by `seed_closure_params`, so the param simply
/// won't be seeded — much better than skipping the entire closure body.
fn filter_resolvable_inferred_params(
    inferred: &[PhpType],
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    inferred
        .iter()
        .map(|ty| {
            if has_unresolvable_base(ty, ctx) {
                PhpType::mixed()
            } else {
                ty.clone()
            }
        })
        .collect()
}

/// Check whether a type has a base name that looks class-like but
/// doesn't resolve to any known class in the project or stubs.
fn has_unresolvable_base(ty: &PhpType, ctx: &ForwardWalkCtx<'_>) -> bool {
    match ty {
        PhpType::Named(name) => is_unresolvable_class_name(name, ctx),
        PhpType::Generic(base, args) => {
            is_unresolvable_class_name(base, ctx)
                || args.iter().any(|a| has_unresolvable_base(a, ctx))
        }
        PhpType::Union(parts) | PhpType::Intersection(parts) => {
            parts.iter().any(|p| has_unresolvable_base(p, ctx))
        }
        PhpType::Nullable(inner) => has_unresolvable_base(inner, ctx),
        PhpType::Callable {
            params,
            return_type,
            ..
        } => {
            if let Some(ret) = return_type
                && has_unresolvable_base(ret, ctx)
            {
                return true;
            }
            params
                .iter()
                .any(|p| has_unresolvable_base(&p.type_hint, ctx))
        }
        _ => false,
    }
}

/// A class name is "unresolvable" if it:
/// 1. Contains a hyphen (e.g. `collection-of`, `non-empty-list`) — these
///    are PHPStan pseudo-types that aren't real PHP classes.
/// 2. Is not a scalar/builtin/special type.
/// 3. Doesn't resolve to a class in the project or stubs.
///
/// We only flag hyphenated names because they are guaranteed to not be
/// valid PHP class names.  Non-hyphenated names that fail resolution
/// might just be missing from the index (vendor code, etc.) and
/// shouldn't trigger the guard.
fn is_unresolvable_class_name(name: &str, _ctx: &ForwardWalkCtx<'_>) -> bool {
    // Hyphenated names are never valid PHP class names.  PHPStan uses
    // them for pseudo-types like `collection-of`, `non-empty-list`,
    // `non-empty-array`, `non-empty-string`, `class-string`, etc.
    // `class-string` is handled elsewhere, but the rest are not
    // resolvable as classes.
    if name.contains('-') {
        // Allow well-known pseudo-types that we DO handle elsewhere.
        let lower = name.to_ascii_lowercase();
        if lower == "class-string"
            || lower == "array-key"
            || lower == "non-empty-string"
            || lower == "non-empty-array"
            || lower == "non-empty-list"
            || lower == "non-falsy-string"
            || lower == "numeric-string"
            || lower == "literal-string"
            || lower == "callable-string"
        {
            return false;
        }
        return true;
    }
    false
}

/// Build a template substitution map from the receiver's resolved types.
///
/// When the receiver resolves to a generic type like `Builder<Product>`,
/// this extracts the generic args from the `type_string` and pairs them
/// with the class's `@template` parameters to produce a substitution map
/// (e.g. `{TModel => Product}`).  This enables callable parameter types
/// that reference template params to be fully substituted.
///
/// When the `type_string` is self-like (`static`, `self`, `$this`) —
/// which happens when a method returns `static` on a generic class —
/// the function reconstructs the generic args from the class_info's
/// method return types via `build_receiver_self_type_pub`.  This
/// preserves generic context through method chains like
/// `Model::where(…)->orderBy(…)->each(fn)` where intermediate steps
/// return `static`.
fn build_receiver_template_subs(
    resolved_types: &[ResolvedType],
    receiver_classes: &[Arc<ClassInfo>],
    ctx: &ForwardWalkCtx<'_>,
) -> HashMap<String, PhpType> {
    // Use the first resolved type that has generic args and a matching
    // class with template params.
    for rt in resolved_types {
        let generic_args = match &rt.type_string {
            PhpType::Generic(_, args) if !args.is_empty() => args,
            _ => continue,
        };
        // Find the matching class info (by FQN or short name).
        let base_name = rt.type_string.base_name().unwrap_or_default();
        let class = receiver_classes.iter().find(|c| {
            c.fqn() == base_name
                || c.name == base_name
                || crate::util::short_name(&c.fqn()) == crate::util::short_name(base_name)
        });
        if let Some(cls) = class
            && !cls.template_params.is_empty()
        {
            return crate::inheritance::build_generic_subs(cls, generic_args);
        }
    }

    // Fallback: when the type_string is self-like (e.g. `Named("static")`)
    // but the class has template params, reconstruct the generic args from
    // the class_info's method return types.  This handles method chains
    // where `static` returns lose generic context in the type_string.
    for rt in resolved_types {
        if !rt.type_string.is_self_like() {
            continue;
        }
        let cls = match &rt.class_info {
            Some(c) if !c.template_params.is_empty() => c,
            _ => continue,
        };
        let reconstructed =
            super::closure_resolution::build_receiver_self_type_pub(cls, ctx.class_loader);
        if let PhpType::Generic(_, ref args) = reconstructed
            && !args.is_empty()
        {
            return crate::inheritance::build_generic_subs(cls, args);
        }
    }

    HashMap::new()
}

/// Infer callable parameter types for a closure passed at position
/// `arg_idx` to a static method call.
fn infer_callable_params_from_static_receiver_fw(
    class_expr: &Expression<'_>,
    method_name: &str,
    arg_idx: usize,
    first_arg_text: Option<&str>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    let _ = scope; // scope not needed for static receiver resolution

    let class_name = match class_expr {
        Expression::Self_(_) => Some(ctx.current_class.name.clone()),
        Expression::Static(_) => Some(ctx.current_class.name.clone()),
        Expression::Identifier(ident) => Some(ident.value().to_string()),
        Expression::Parent(_) => ctx.current_class.parent_class.clone(),
        _ => None,
    };
    let owner = class_name.and_then(|name| {
        ctx.all_classes
            .iter()
            .find(|c| c.name == name)
            .map(|c| ClassInfo::clone(c))
            .or_else(|| (ctx.class_loader)(&name).map(Arc::unwrap_or_clone))
    });
    if let Some(ref cls) = owner {
        // For relation-query methods, override with Builder<RelatedModel>.
        if let Some(override_params) = super::closure_resolution::try_relation_query_override_pub(
            &[Arc::new(cls.clone())],
            method_name,
            first_arg_text,
            ctx.class_loader,
        ) {
            return override_params;
        }

        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        let params = find_callable_params_on_method_fw(&resolved, method_name, arg_idx);

        // Build a template substitution map from the owner class.
        // When the owner is a generic class (e.g. `Builder<Customer>`
        // via `@extends`), reconstruct its full generic type and pair
        // the args with the class's @template params so that callable
        // params like `Closure(Collection<int, TModel>)` become
        // `Closure(Collection<int, Customer>)`.
        let receiver_type =
            super::closure_resolution::build_receiver_self_type_pub(cls, ctx.class_loader);
        let template_subs = if let PhpType::Generic(_, ref args) = receiver_type
            && !args.is_empty()
            && !cls.template_params.is_empty()
        {
            crate::inheritance::build_generic_subs(cls, args)
        } else {
            HashMap::new()
        };

        let params = if !template_subs.is_empty() {
            params
                .into_iter()
                .map(|p| p.substitute(&template_subs))
                .collect()
        } else {
            params
        };

        params
            .into_iter()
            .map(|p| p.replace_self_with_type(&receiver_type))
            .collect()
    } else {
        vec![]
    }
}

/// Search for method `method_name` on each of `classes` and extract
/// callable parameter types at `arg_idx`.
///
/// Tries the input class first — when the receiver came from a generic
/// instantiation (e.g. `Stream<int, Product>`), its `class_info` already
/// has template substitutions applied by `type_hint_to_classes_typed` →
/// `resolve_class_fully_with_generics`.  Extracting callable params from
/// that class preserves the concrete types (e.g. `callable(Product)`
/// instead of `callable(TVal)`).
///
/// Falls back to `resolve_class_fully_maybe_cached` only when the method
/// isn't found on the input class — this handles methods that come
/// exclusively from virtual member providers or late-merged traits.
fn find_callable_params_on_classes_fw(
    classes: &[Arc<ClassInfo>],
    method_name: &str,
    arg_idx: usize,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    for cls in classes {
        // First: try the class as-is.  When it came from a generic
        // instantiation, template params are already substituted in
        // all method signatures.  Re-resolving via
        // `resolve_class_fully_maybe_cached` would load the base
        // class definition (keyed by FQN with empty generic args),
        // discarding those substitutions.
        let result = find_callable_params_on_method_fw(cls, method_name, arg_idx);
        if !result.is_empty() {
            return result;
        }

        // Fallback: the method wasn't found on the input class.
        // This can happen when the method comes from a virtual member
        // provider, a late-merged trait, or a mixin that wasn't
        // included in the original resolution.  Re-resolve fully
        // and try again.
        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        let result = find_callable_params_on_method_fw(&resolved, method_name, arg_idx);
        if !result.is_empty() {
            return result;
        }
    }
    vec![]
}

/// Look up method `method_name` on `class` and extract callable
/// parameter types from the parameter at position `arg_idx`.
fn find_callable_params_on_method_fw(
    class: &ClassInfo,
    method_name: &str,
    arg_idx: usize,
) -> Vec<PhpType> {
    let method = class.methods.iter().find(|m| m.name == method_name);
    if let Some(m) = method {
        extract_callable_params_at_fw(&m.parameters, arg_idx)
    } else {
        vec![]
    }
}

/// Given a list of parameters, look at `arg_idx` and extract callable
/// parameter types if the type hint is `callable(...)` or `Closure(...)`.
fn extract_callable_params_at_fw(
    params: &[crate::types::ParameterInfo],
    arg_idx: usize,
) -> Vec<PhpType> {
    let param = params.get(arg_idx);
    if let Some(p) = param
        && let Some(ref hint) = p.type_hint
        && let Some(callable_params) = hint.callable_param_types()
    {
        return callable_params
            .iter()
            .map(|cp| cp.type_hint.clone())
            .collect();
    }
    vec![]
}

/// Extract the text of each argument from a call's argument list.
fn extract_argument_texts_fw(
    arguments: &TokenSeparatedSequence<'_, Argument<'_>>,
    content: &str,
) -> Vec<String> {
    arguments
        .iter()
        .map(|arg| {
            let span = match arg {
                Argument::Positional(pos) => pos.value.span(),
                Argument::Named(named) => named.value.span(),
            };
            let start = span.start.offset as usize;
            let end = span.end.offset as usize;
            if end <= content.len() {
                content[start..end].to_string()
            } else {
                String::new()
            }
        })
        .collect()
}

/// Extract the text of the first positional argument, stripping quotes.
fn extract_first_arg_string_fw(
    arguments: &TokenSeparatedSequence<'_, Argument<'_>>,
    content: &str,
) -> Option<String> {
    let first = arguments.iter().next()?;
    let expr = match first {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    };
    let span = expr.span();
    let start = span.start.offset as usize;
    let end = span.end.offset as usize;
    let raw = content.get(start..end)?.trim();

    if raw.len() >= 2
        && ((raw.starts_with('\'') && raw.ends_with('\''))
            || (raw.starts_with('"') && raw.ends_with('"')))
    {
        Some(raw[1..raw.len() - 1].to_string())
    } else {
        None
    }
}

/// Build diagnostic scope snapshots for every function/method body in
/// the file.
///
/// Parses the file, iterates all top-level and class-level
/// function/method bodies, runs the forward walker on each, and stores
/// scope snapshots in the thread-local [`DIAGNOSTIC_SCOPE`] cache.
///
/// The caller must have activated the cache via
/// [`with_diagnostic_scope_cache`] before calling this function.
pub(crate) fn build_diagnostic_scopes(
    content: &str,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    loaders: Loaders<'_>,
    resolved_class_cache: Option<&crate::virtual_members::ResolvedClassCache>,
) {
    if !is_diagnostic_scope_active() {
        return;
    }

    // Mark that we are building the scope cache so that nested
    // resolution calls (e.g. resolve_variable_types) do not read
    // from the partially-populated cache.
    BUILDING_SCOPES.with(|cell: &Cell<bool>| cell.set(true));
    let _building_guard = BuildingScopesGuard;

    let default_class = ClassInfo::default();
    let diag_ctx = DiagnosticWalkCtx {
        content,
        local_classes,
        class_loader,
        loaders,
        resolved_class_cache,
    };

    with_parsed_program(content, "build_diagnostic_scopes", |program, _content| {
        // Walk all top-level statements, analyzing function/method
        // bodies AND top-level code (assignments, expressions, if,
        // foreach, etc.) that lives outside any function body.
        walk_top_level_statements(program.statements.iter(), &default_class, &diag_ctx);
    });
}

/// Recursively walk the AST to find function and method bodies, running
/// the forward walker on each.
/// Seed `$this` in the scope when inside a non-static class method.
///
/// This creates a `ResolvedType` from the enclosing `ClassInfo` and
/// stores it under `"$this"`.  The scope-based variable resolver then
/// returns this entry for any `$this` lookup, eliminating the need to
/// remain unresolved.
fn seed_this(scope: &mut ScopeState, current_class: &ClassInfo) {
    if current_class.name.is_empty() {
        return;
    }
    scope.set(
        "$this".to_string(),
        vec![ResolvedType::from_class(current_class.clone())],
    );
}

/// Walk a sequence of top-level (or namespace-level) statements,
/// maintaining a shared `ScopeState` for code that lives outside any
/// function or class body.  Function/class/trait/interface/enum bodies
/// are analyzed in isolation (as before), but top-level assignments,
/// expressions, if/foreach/while/for/try/switch, and other statements
/// are walked through the forward walker so that scope snapshots are
/// recorded.  This ensures variable accesses resolve from the scope
/// cache.
fn walk_top_level_statements<'a, 'b: 'a>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    default_class: &ClassInfo,
    diag_ctx: &DiagnosticWalkCtx<'_>,
) {
    let ctx = ForwardWalkCtx {
        current_class: default_class,
        all_classes: diag_ctx.local_classes,
        content: diag_ctx.content,
        cursor_offset: u32::MAX,
        class_loader: diag_ctx.class_loader,
        loaders: diag_ctx.loaders,
        resolved_class_cache: diag_ctx.resolved_class_cache,
        enclosing_return_type: None,
    };

    let mut top_level_scope = ScopeState::new();

    // Seed superglobals for top-level code.
    seed_superglobals(&mut top_level_scope);

    for stmt in statements {
        match stmt {
            Statement::Namespace(ns) => {
                // Recurse into namespace body with a fresh scope.
                walk_top_level_statements(ns.statements().iter(), default_class, diag_ctx);
            }
            Statement::Class(class) => {
                let enclosing = find_enclosing_class_for_offset(
                    diag_ctx.local_classes,
                    class.left_brace.start.offset,
                )
                .unwrap_or(default_class);
                for member in class.members.iter() {
                    walk_class_member_body(member, enclosing, diag_ctx);
                }
            }
            Statement::Interface(iface) => {
                let enclosing = find_enclosing_class_for_offset(
                    diag_ctx.local_classes,
                    iface.left_brace.start.offset,
                )
                .unwrap_or(default_class);
                for member in iface.members.iter() {
                    walk_class_member_body(member, enclosing, diag_ctx);
                }
            }
            Statement::Trait(trait_def) => {
                let enclosing = find_enclosing_class_for_offset(
                    diag_ctx.local_classes,
                    trait_def.left_brace.start.offset,
                )
                .unwrap_or(default_class);
                for member in trait_def.members.iter() {
                    walk_class_member_body(member, enclosing, diag_ctx);
                }
            }
            Statement::Enum(enum_def) => {
                let enclosing = find_enclosing_class_for_offset(
                    diag_ctx.local_classes,
                    enum_def.left_brace.start.offset,
                )
                .unwrap_or(default_class);
                for member in enum_def.members.iter() {
                    walk_class_member_body(member, enclosing, diag_ctx);
                }
            }
            Statement::Function(func) => {
                analyze_function_body(
                    func.parameter_list.parameters.iter(),
                    func.body.statements.iter(),
                    func.span().start.offset,
                    default_class,
                    None,
                    true, // standalone functions have no `$this`
                    diag_ctx,
                );
            }
            // Top-level code: walk it with the shared scope so that
            // variable assignments accumulate and subsequent accesses
            // can be served from the scope cache instead of remaining
            // unresolved.
            _ => {
                record_scope_snapshot(stmt.span().start.offset, &top_level_scope);
                process_statement(stmt, &mut top_level_scope, &ctx);
                walk_closures_in_statement(stmt, &top_level_scope, &ctx);
                record_scope_snapshot(stmt.span().end.offset, &top_level_scope);
            }
        }
    }
}

/// Walk a class member to find method bodies and run the forward walker.
fn walk_class_member_body<'b>(
    member: &'b mago_syntax::ast::class_like::member::ClassLikeMember<'b>,
    enclosing_class: &ClassInfo,
    diag_ctx: &DiagnosticWalkCtx<'_>,
) {
    use mago_syntax::ast::class_like::member::ClassLikeMember;
    use mago_syntax::ast::class_like::method::MethodBody;

    if let ClassLikeMember::Method(method) = member
        && let MethodBody::Concrete(block) = &method.body
    {
        let method_name = method.name.value.to_string();
        let is_static = method.modifiers.contains_static();
        analyze_function_body(
            method.parameter_list.parameters.iter(),
            block.statements.iter(),
            method.span().start.offset,
            enclosing_class,
            Some(&method_name),
            is_static,
            diag_ctx,
        );
    }
}

/// Bundles the immutable context needed by [`analyze_function_body`] and
/// the AST walkers so we don't pass 5+ individual arguments everywhere.
struct DiagnosticWalkCtx<'a> {
    content: &'a str,
    local_classes: &'a [Arc<ClassInfo>],
    class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    loaders: Loaders<'a>,
    resolved_class_cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
}

/// Run the forward walker on a single function/method body and record
/// scope snapshots for diagnostics.
///
/// `is_static` indicates whether this is a static method.  When `false`
/// and `current_class` has a non-empty name, `$this` is seeded in the
/// scope so that expressions like `$this->prop` and `foreach ($this->items as $item)`
/// can resolve without remaining unresolved.
fn analyze_function_body<'b>(
    parameters: impl Iterator<Item = &'b FunctionLikeParameter<'b>>,
    body_statements: impl Iterator<Item = &'b Statement<'b>>,
    fn_span_start: u32,
    current_class: &ClassInfo,
    method_name: Option<&str>,
    is_static: bool,
    diag_ctx: &DiagnosticWalkCtx<'_>,
) {
    let ctx = ForwardWalkCtx {
        current_class,
        all_classes: diag_ctx.local_classes,
        content: diag_ctx.content,
        cursor_offset: u32::MAX,
        class_loader: diag_ctx.class_loader,
        loaders: diag_ctx.loaders,
        resolved_class_cache: diag_ctx.resolved_class_cache,
        enclosing_return_type: None,
    };

    let mut scope = ScopeState::new();

    // Seed `$this` for non-static class methods so that expressions
    // referencing `$this` (e.g. `$this->prop`, `foreach ($this->items …)`)
    // resolve from the scope instead of falling through to the backward
    // scanner.
    if !is_static {
        seed_this(&mut scope, current_class);
    }

    // Seed scope with parameter types.
    // Detect whether this method has a #[Scope] attribute by scanning
    // the source text around the method span for `#[Scope]`.
    let has_scope_attr = method_name
        .map(|_| detect_scope_attribute_from_source(diag_ctx.content, fn_span_start as usize))
        .unwrap_or(false);
    seed_params(
        &mut scope,
        parameters,
        fn_span_start,
        method_name,
        has_scope_attr,
        &ctx,
    );

    // Seed superglobals so that accesses like `$_SERVER['key']` don't
    // remain unresolved.
    seed_superglobals(&mut scope);

    // Record the scope right at the function body start so that
    // member accesses on parameters before any assignment are covered.
    record_scope_snapshot(fn_span_start, &scope);

    // Walk the entire body, recording snapshots at each statement.
    walk_body_for_diagnostics(body_statements, &mut scope, &ctx);
}

/// Find the innermost class whose body span contains `offset`.
///
/// This is the diagnostic-module equivalent of
/// [`find_innermost_enclosing_class`](crate::diagnostics::helpers::find_innermost_enclosing_class).
fn find_enclosing_class_for_offset(
    local_classes: &[Arc<ClassInfo>],
    offset: u32,
) -> Option<&ClassInfo> {
    local_classes
        .iter()
        .filter(|c| offset >= c.start_offset && offset <= c.end_offset)
        .min_by_key(|c| c.end_offset.saturating_sub(c.start_offset))
        .map(|c| c.as_ref())
}

// ─── Core data structures ───────────────────────────────────────────────────

/// The type-state of all variables at a single program point.
///
/// This is the equivalent of PHPStan's `expressionTypes` map and Mago's
/// `BlockContext.locals`.  It is created once at the start of a function
/// body analysis, seeded with parameter types, and passed as `&mut` through
/// the forward walk.
#[derive(Clone, Debug)]
pub(crate) struct ScopeState {
    /// Variable name (with `$` prefix, e.g. `"$foo"`) → resolved types.
    ///
    /// This is the single source of truth for all variable types at the
    /// current program point.  Every variable that has been assigned,
    /// declared as a parameter, or bound by a foreach/catch before the
    /// current statement has an entry here.
    pub locals: HashMap<String, Vec<ResolvedType>>,
}

impl ScopeState {
    /// Create an empty scope.
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
        }
    }

    /// Look up a variable's types.  Returns an empty slice when the
    /// variable has not been assigned.
    pub fn get(&self, var_name: &str) -> &[ResolvedType] {
        self.locals.get(var_name).map_or(&[], |v| v.as_slice())
    }

    /// Check whether a variable exists in scope (even if its type list is empty).
    pub fn contains(&self, var_name: &str) -> bool {
        self.locals.contains_key(var_name)
    }

    /// Insert or overwrite a variable's types.
    pub fn set(&mut self, var_name: String, types: Vec<ResolvedType>) {
        if types.is_empty() {
            return;
        }
        self.locals.insert(var_name, types);
    }

    /// Record that a variable exists in scope with an empty type list.
    /// This prevents the variable from appearing unseen by the forward
    /// walker.
    pub fn set_empty(&mut self, var_name: String) {
        self.locals.entry(var_name).or_default();
    }

    /// Insert a variable's types from parameter seeding.
    pub fn seed(&mut self, var_name: String, types: Vec<ResolvedType>) {
        if types.is_empty() {
            return;
        }
        self.locals.insert(var_name, types);
    }

    /// Remove a variable (e.g. after `unset($x)`).
    pub fn remove(&mut self, var_name: &str) {
        self.locals.remove(var_name);
    }

    /// Merge another scope into `self`.
    ///
    /// For each variable:
    /// - Present in both: union the type sets (variable was assigned
    ///   in both branches).
    /// - Present in only one: keep it with the existing types (variable
    ///   was assigned in only one branch — it *might* have those types).
    pub fn merge_branch(&mut self, other: &ScopeState) {
        for (name, other_types) in &other.locals {
            let entry = self.locals.entry(name.clone()).or_default();
            ResolvedType::extend_unique(entry, other_types.clone());
        }
    }
}

/// Context for the forward walk.
///
/// Bundles the immutable context that every statement/expression handler
/// needs — the class loader, function loader, current class info, source
/// text, etc.  The mutable `ScopeState` is passed separately as `&mut`.
pub(crate) struct ForwardWalkCtx<'a> {
    /// The class containing the method being analyzed (or a dummy for
    /// top-level functions).
    pub current_class: &'a ClassInfo,
    /// All classes known in the current file.
    pub all_classes: &'a [Arc<ClassInfo>],
    /// Full source text of the current file.
    pub content: &'a str,
    /// Byte offset of the cursor.  The walk stops when a statement's
    /// start offset reaches or exceeds this value.
    pub cursor_offset: u32,
    /// Cross-file class resolution callback.
    pub class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    /// Cross-file loader callbacks (function loader, constant loader).
    pub loaders: Loaders<'a>,
    /// Shared cache of fully-resolved classes.
    pub resolved_class_cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
    /// The `@return` type of the enclosing function/method, if known.
    /// Used for generator yield inference.
    pub enclosing_return_type: Option<PhpType>,
}

impl<'a> ForwardWalkCtx<'a> {
    /// Return a copy of this context with a different `cursor_offset`.
    ///
    /// Used by the two-pass loop strategy: pass 1 runs with
    /// `cursor_offset = u32::MAX` so the entire loop body is walked
    /// and all assignments are discovered, even those after the real
    /// cursor position.
    fn with_cursor_offset(&self, cursor_offset: u32) -> ForwardWalkCtx<'a> {
        ForwardWalkCtx {
            current_class: self.current_class,
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset,
            class_loader: self.class_loader,
            loaders: self.loaders,
            resolved_class_cache: self.resolved_class_cache,
            enclosing_return_type: self.enclosing_return_type.clone(),
        }
    }

    /// Build a [`VarResolutionCtx`] with a scope-based variable
    /// resolver.  Used by [`resolve_rhs_with_scope`] so that
    /// `resolve_rhs_expression` and its sub-functions read variable
    /// types from the forward walker's in-progress `ScopeState`
    /// instead of re-entering `resolve_variable_types`.
    fn var_ctx_for_with_scope<'b>(
        &'b self,
        var_name: &'b str,
        cursor_offset: u32,
        scope_resolver: &'b dyn Fn(&str) -> Vec<ResolvedType>,
    ) -> VarResolutionCtx<'b>
    where
        'a: 'b,
    {
        VarResolutionCtx {
            var_name,
            current_class: self.current_class,
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset,
            class_loader: self.class_loader,
            loaders: self.loaders,
            resolved_class_cache: self.resolved_class_cache,
            enclosing_return_type: self.enclosing_return_type.clone(),
            branch_aware: false,
            match_arm_narrowing: HashMap::new(),
            scope_var_resolver: Some(scope_resolver),
        }
    }
}

// ─── Forward walk entry point ───────────────────────────────────────────────

/// Walk a sequence of statements top-to-bottom, updating `scope` at
/// each step.  Stops when a statement's start offset reaches or exceeds
/// `ctx.cursor_offset`.
///
/// After this function returns, `scope.get("$varName")` contains the
/// types of `$varName` at the cursor position.
pub(crate) fn walk_body_forward<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // When the diagnostic scope cache is active, record snapshots at
    // every statement boundary — even inside branches (if/else, try,
    // foreach, loops).  Without this, member accesses inside branch
    // bodies would only see the scope from before the branch started,
    // missing assignments made inside the branch and causing false-
    // positive diagnostics.
    let record_snapshots = is_diagnostic_scope_active();

    for stmt in statements {
        // Stop when we reach the cursor.
        if stmt.span().start.offset >= ctx.cursor_offset {
            break;
        }

        // Check whether the cursor is inside a closure/arrow function
        // within this statement.  If so, we need to resolve within
        // that closure's scope instead.
        let stmt_span = stmt.span();
        if ctx.cursor_offset >= stmt_span.start.offset
            && ctx.cursor_offset <= stmt_span.end.offset
            && try_enter_closure(stmt, scope, ctx)
        {
            return;
        }

        // On the completion path, when the cursor is inside a ternary
        // instanceof branch or match(true) arm, apply narrowing to the
        // scope so the variable lookup sees the narrowed type.
        let cursor_inside_stmt = ctx.cursor_offset >= stmt_span.start.offset
            && ctx.cursor_offset <= stmt_span.end.offset;

        if record_snapshots {
            record_scope_snapshot(stmt_span.start.offset, scope);
        }

        process_statement(stmt, scope, ctx);

        if cursor_inside_stmt && !record_snapshots {
            let expr_opt = match stmt {
                Statement::Expression(es) => Some(es.expression),
                Statement::Return(ret) => ret.value,
                _ => None,
            };
            if let Some(expr) = expr_opt {
                apply_cursor_ternary_narrowing(expr, scope, ctx);
            }

            // Also apply narrowing inside if/while/for conditions.
            // E.g. `if ($e instanceof Foo && $e->errorInfo)` — the
            // cursor on `$e->errorInfo` needs instanceof narrowing.
            match stmt {
                Statement::If(if_stmt) => {
                    let cond_span = if_stmt.condition.span();
                    if ctx.cursor_offset >= cond_span.start.offset
                        && ctx.cursor_offset <= cond_span.end.offset
                    {
                        apply_cursor_ternary_narrowing(if_stmt.condition, scope, ctx);
                    }
                }
                Statement::While(while_stmt) => {
                    let cond_span = while_stmt.condition.span();
                    if ctx.cursor_offset >= cond_span.start.offset
                        && ctx.cursor_offset <= cond_span.end.offset
                    {
                        apply_cursor_ternary_narrowing(while_stmt.condition, scope, ctx);
                    }
                }
                _ => {}
            }
        }

        // When the diagnostic scope cache is active, walk closure and
        // arrow function bodies found in this statement.  This is the
        // same call that `walk_body_for_diagnostics` makes for
        // top-level statements, but here it also covers closures
        // inside branch bodies (if/else, foreach, try, etc.) where
        // the scope reflects narrowing and bindings from the enclosing
        // block.
        if record_snapshots {
            walk_closures_in_statement(stmt, scope, ctx);
            record_scope_snapshot(stmt_span.end.offset, scope);
        }
    }
}

/// Resolve the target variable from a method body using the forward
/// walker.
///
/// This is the main entry point called from `resolve_variable_in_members`.
/// It seeds the scope with parameter types and walks the method body
/// forward to the cursor.
pub(crate) fn resolve_in_method_body<'b>(
    var_name: &str,
    parameters: impl Iterator<Item = &'b FunctionLikeParameter<'b>>,
    body_statements: impl Iterator<Item = &'b Statement<'b>>,
    method_span_start: u32,
    method_ctx: Option<(&str, bool)>,
    is_static: bool,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    let mut scope = ScopeState::new();

    // Seed `$this` for non-static class methods.
    if !is_static {
        seed_this(&mut scope, ctx.current_class);
    }

    // Seed scope with parameter types.
    let method_name = method_ctx.map(|(n, _)| n);
    let has_scope_attr = method_ctx.is_some_and(|(_, s)| s);
    seed_params(
        &mut scope,
        parameters,
        method_span_start,
        method_name,
        has_scope_attr,
        ctx,
    );

    // Walk the body forward.
    walk_body_forward(body_statements, &mut scope, ctx);

    // Read the target variable from the scope.
    // Return `Some(types)` when the variable exists in scope (even if
    // the type list is empty — that means "unknown/narrowed-away"),
    // and `None` when the variable was never seen by the forward walker.
    if scope.contains(var_name) {
        let types = scope.get(var_name).to_vec();
        // When the variable is in scope but has no resolved types and
        // the enclosing function returns a Generator, try reverse
        // inference from yield statements.
        if types.is_empty()
            && let Some(inferred) = try_generator_yield_inference(var_name, ctx)
        {
            return Some(inferred);
        }
        Some(types)
    } else {
        // Variable was never assigned.  Try generator yield reverse
        // inference: if the variable appears as `yield $var` and the
        // enclosing function returns Generator<TKey, TValue>, infer
        // the variable's type as TValue.
        if let Some(inferred) = try_generator_yield_inference(var_name, ctx) {
            return Some(inferred);
        }
        None
    }
}

/// Resolve the target variable from a standalone function body using
/// the forward walker.
/// Detect whether a method has a `#[Scope]` attribute by scanning the
/// source text around the method span.  The attribute list precedes or
/// is part of the method node, so we search a window around the offset.
fn detect_scope_attribute_from_source(content: &str, method_offset: usize) -> bool {
    // Search backwards from the method offset for `#[Scope]` or
    // `#[\...\Scope]` in the preceding ~500 characters.
    let search_start = method_offset.saturating_sub(500);
    let region = &content[search_start..content.len().min(method_offset + 200)];
    // Find occurrences of `#[` and check if any contain `Scope`.
    let mut pos = 0;
    while let Some(bracket_pos) = region[pos..].find("#[") {
        let abs = pos + bracket_pos;
        if let Some(end) = region[abs..].find(']') {
            let attr_text = &region[abs..abs + end + 1];
            if attr_text.contains("Scope") {
                return true;
            }
            pos = abs + end + 1;
        } else {
            break;
        }
    }
    false
}

pub(crate) fn resolve_in_function_body<'b>(
    var_name: &str,
    func: &'b Function<'b>,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    let mut scope = ScopeState::new();

    // Seed scope with parameter types.
    seed_params(
        &mut scope,
        func.parameter_list.parameters.iter(),
        func.span().start.offset,
        None,
        false, // standalone functions are never scope methods
        ctx,
    );

    // Walk the body forward.
    walk_body_forward(func.body.statements.iter(), &mut scope, ctx);

    // Read the target variable.
    // Return `Some` when the variable exists in scope (even with
    // empty types), `None` when it was never seen.
    if scope.contains(var_name) {
        let types = scope.get(var_name).to_vec();
        if types.is_empty()
            && let Some(inferred) = try_generator_yield_inference(var_name, ctx)
        {
            return Some(inferred);
        }
        Some(types)
    } else {
        if let Some(inferred) = try_generator_yield_inference(var_name, ctx) {
            return Some(inferred);
        }
        None
    }
}

/// Resolve the target variable from top-level code (outside any
/// function or class body) using the forward walker.
///
/// Seeds superglobals, then walks all top-level statements forward to
/// the cursor, skipping class/function/interface/enum/trait declarations
/// (which have their own isolated scopes).
pub(crate) fn resolve_in_top_level<'b>(
    var_name: &str,
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    let mut scope = ScopeState::new();

    // Seed superglobals so that `$_GET`, `$_POST`, etc. resolve.
    seed_superglobals(&mut scope);

    // Walk the top-level statements forward.
    walk_body_forward(statements, &mut scope, ctx);

    // Return `Some` when the variable exists in scope (even with
    // empty types), `None` when it was never seen.
    if scope.contains(var_name) {
        Some(scope.get(var_name).to_vec())
    } else {
        None
    }
}

// ─── Generator yield reverse inference ──────────────────────────────────────

/// When the enclosing function/method returns a `Generator<TKey, TValue>`,
/// scan the source text for `yield $varName` and infer the variable's type
/// as `TValue`.  This handles the pattern where a variable is yielded but
/// never explicitly assigned — its type comes from the Generator's return
/// type annotation.
fn try_generator_yield_inference(
    var_name: &str,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    let return_type = ctx.enclosing_return_type.as_ref()?;
    let value_type = return_type.extract_value_type(false)?;

    // Scan the source text for `yield $varName` within the enclosing
    // function body.  We search a window around the cursor.
    let cursor = ctx.cursor_offset as usize;
    let content = ctx.content;

    // Find the enclosing function body boundaries by scanning backward
    // for the opening `{`.
    let search_before = content.get(..cursor).unwrap_or("");
    let mut brace_depth = 0i32;
    let mut body_start = None;
    for (i, ch) in search_before.char_indices().rev() {
        match ch {
            '}' => brace_depth += 1,
            '{' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    body_start = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    let start = body_start?;

    // Find the matching closing `}`.
    let after_open = content.get(start..).unwrap_or("");
    let mut depth = 0i32;
    let mut body_end = content.len();
    for (i, ch) in after_open.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    body_end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }

    let body = content.get(start..body_end).unwrap_or("");

    // Look for `yield $varName` or `=> $varName` in yield context.
    let yield_pattern = format!("yield {}", var_name);
    let has_yield = body.contains(&yield_pattern);

    let yield_pair_needle = format!("=> {}", var_name);
    let has_yield_pair = body.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.contains("yield ") && trimmed.contains(&yield_pair_needle)
    });

    if !has_yield && !has_yield_pair {
        return None;
    }

    let classes = crate::completion::type_resolution::type_hint_to_classes_typed(
        value_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    if classes.is_empty() {
        return None;
    }
    Some(ResolvedType::from_classes(classes))
}

// ─── Parameter seeding ──────────────────────────────────────────────────────

/// Seed the scope with types from function/method parameters.
///
/// For each parameter, resolves its type from:
/// 1. The native type hint
/// 2. The `@param` docblock annotation (which may be more specific)
/// 3. The merged class info (from parent/interface inheritance)
/// 4. Eloquent scope Builder enrichment
fn seed_params<'b>(
    scope: &mut ScopeState,
    parameters: impl Iterator<Item = &'b FunctionLikeParameter<'b>>,
    method_span_start: u32,
    method_name: Option<&str>,
    has_scope_attr: bool,
    ctx: &ForwardWalkCtx<'_>,
) {
    for param in parameters {
        let pname = param.variable.name.to_string();
        let is_variadic = param.ellipsis.is_some();
        let native_type = param.hint.as_ref().map(|h| extract_hint_type(h));

        let param_results = resolve_param_type(
            &pname,
            native_type.as_ref(),
            is_variadic,
            method_span_start,
            method_name,
            has_scope_attr,
            ctx,
        );

        if !param_results.is_empty() {
            scope.seed(pname, param_results);
        } else {
            // Seed untyped parameters with empty types so they exist
            // in scope.  This allows instanceof narrowing to find them
            // (apply_condition_narrowing iterates scope.locals.keys()).
            scope.set_empty(pname);
        }
    }
}

/// Resolve a single parameter's type through the full resolution
/// pipeline: native hint → Eloquent Builder enrichment → docblock
/// `@param` → template substitution → merged class fallback →
/// type-string-only fallback.
///
/// Used by [`seed_params`] (forward walker) and
/// [`super::resolution::resolve_abstract_method_param`] (abstract
/// methods with no body).
pub(super) fn resolve_param_type(
    pname: &str,
    native_type: Option<&PhpType>,
    is_variadic: bool,
    method_span_start: u32,
    method_name: Option<&str>,
    has_scope_attr: bool,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    // Eloquent scope Builder enrichment: when the enclosing class
    // extends Eloquent Model and this is a scope method (convention
    // or #[Scope] attribute), enrich bare `Builder` to
    // `Builder<EnclosingModel>`.
    let enriched_type = native_type.and_then(|nt| {
        if let Some(mname) = method_name {
            super::resolution::enrich_builder_type_in_scope(
                nt,
                mname,
                has_scope_attr,
                ctx.current_class,
                ctx.class_loader,
            )
        } else {
            None
        }
    });

    let type_for_resolution: Option<&PhpType> = enriched_type.as_ref().or(native_type);

    // Check the `@param` docblock annotation.
    let raw_docblock_type = crate::docblock::find_iterable_raw_type_in_source(
        ctx.content,
        method_span_start as usize,
        pname,
    );

    // Pick the effective type: docblock overrides native when it is
    // a compatible refinement.  Use the enriched type (e.g.
    // `Builder<User>`) rather than the bare native type so that
    // the generic args survive into the resolved ClassInfo.
    let native_for_effective = enriched_type.as_ref().or(native_type).cloned();
    let doc_parsed = raw_docblock_type.clone();
    let effective_type = crate::docblock::resolve_effective_type_typed(
        native_for_effective.as_ref(),
        doc_parsed.as_ref(),
    );

    // Substitute method-level template params with their bounds.
    let effective_type = effective_type.map(|ty| {
        let ty = super::resolution::substitute_template_param_bounds(
            ty,
            ctx.content,
            method_span_start as usize,
        );
        // Also substitute inside class-string<T> so that
        // `class-string<T>` with `@template T of Foo` becomes
        // `class-string<Foo>`.
        super::resolution::substitute_class_string_template_bounds(
            ty,
            ctx.content,
            method_span_start as usize,
        )
    });

    let mut resolved_from_effective = effective_type
        .as_ref()
        .map(|ty| {
            crate::completion::type_resolution::type_hint_to_classes_typed(
                ty,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            )
        })
        .unwrap_or_default();

    // When the effective type is `class-string<Foo>`, the base
    // type `class-string` doesn't resolve to a class.  Unwrap the
    // inner type and resolve it so that `$class::KEY` finds
    // static members on `Foo`.
    if resolved_from_effective.is_empty()
        && let Some(ref eff) = effective_type
        && let Some(inner) = eff.unwrap_class_string_inner()
    {
        let inner_resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
            inner,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if !inner_resolved.is_empty() {
            resolved_from_effective = inner_resolved;
        }
    }

    let mut param_results = if !resolved_from_effective.is_empty() {
        ResolvedType::from_classes_with_hint(
            resolved_from_effective,
            effective_type.unwrap_or_else(|| {
                type_for_resolution
                    .cloned()
                    .unwrap_or_else(PhpType::untyped)
            }),
        )
    } else if let Some(ref rdt) = raw_docblock_type {
        let parsed_docblock = rdt.clone();
        let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
            &parsed_docblock,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if !resolved.is_empty() {
            ResolvedType::from_classes_with_hint(resolved, parsed_docblock)
        } else {
            // Try the merged class for a richer type.
            try_resolve_from_merged_class(pname, method_name, ctx).unwrap_or_else(|| {
                build_type_string_only_result(
                    raw_docblock_type.as_ref(),
                    type_for_resolution,
                    ctx.content,
                    method_span_start as usize,
                )
            })
        }
    } else {
        // Try the merged class.
        try_resolve_from_merged_class(pname, method_name, ctx).unwrap_or_else(|| {
            build_type_string_only_result(
                raw_docblock_type.as_ref(),
                type_for_resolution,
                ctx.content,
                method_span_start as usize,
            )
        })
    };

    // Variadic parameter wrapping.
    if is_variadic && !param_results.is_empty() {
        for rt in &mut param_results {
            rt.type_string = PhpType::list(rt.type_string.clone());
            rt.class_info = None;
        }
    }

    param_results
}

/// Try to resolve a parameter type from the fully-merged class info
/// (with interface members merged and `@implements` generics applied).
///
/// When a class declares `@implements CastsAttributes<Decimal, Decimal>`
/// and the interface method `set()` has a generic parameter `TSet $value`,
/// the merged class will have `set($value: Decimal)`.  This function
/// looks up the merged method and returns the substituted parameter type.
fn try_resolve_from_merged_class(
    pname: &str,
    method_name: Option<&str>,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    let method_name = method_name?;

    // Only attempt this for real classes (not the default/dummy class
    // used for top-level functions).
    if ctx.current_class.name.is_empty() {
        return None;
    }

    let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
        ctx.current_class,
        ctx.class_loader,
        ctx.resolved_class_cache,
    );

    let merged_method = merged.methods.iter().find(|m| m.name == method_name)?;

    // Find the matching parameter by name.
    // ParameterInfo.name includes the `$` prefix.
    let merged_param = merged_method.parameters.iter().find(|p| p.name == pname)?;
    let hint = merged_param.type_hint.as_ref()?;

    let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
        hint,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );

    if !resolved.is_empty() {
        Some(ResolvedType::from_classes_with_hint(resolved, hint.clone()))
    } else {
        // The merged type doesn't resolve to a class (e.g. `list<Pen>`,
        // `array<string, int>`).  Return a type-string-only result so
        // the merged hint (which may be richer than the native type
        // from the child's signature, e.g. `list<Pen>` vs bare `array`)
        // is preserved in the scope.  This allows array-access
        // resolution to extract the element type from `list<Pen>`.
        Some(vec![ResolvedType::from_type_string(hint.clone())])
    }
}

/// Build a type-string-only `ResolvedType` result for a parameter whose
/// type does not resolve to any class.
fn build_type_string_only_result(
    raw_docblock_type: Option<&PhpType>,
    type_for_resolution: Option<&PhpType>,
    content: &str,
    method_span_start: usize,
) -> Vec<ResolvedType> {
    let best_type = if let Some(rdt) = raw_docblock_type {
        Some(rdt.clone())
    } else {
        type_for_resolution.cloned()
    };
    if let Some(mut parsed) = best_type {
        parsed = super::resolution::substitute_class_string_template_bounds(
            parsed,
            content,
            method_span_start,
        );
        vec![ResolvedType::from_type_string(parsed)]
    } else {
        vec![]
    }
}

// ─── Statement processing ───────────────────────────────────────────────────

/// Process a single statement, updating `scope` with any variable
/// assignments, narrowing, or control-flow effects.
fn process_statement<'b>(
    stmt: &'b Statement<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    match stmt {
        Statement::Expression(expr_stmt) => {
            process_expression_statement(expr_stmt, scope, ctx);
        }
        Statement::Foreach(foreach) => {
            process_foreach(foreach, scope, ctx);
        }
        Statement::If(if_stmt) => {
            process_if(if_stmt, stmt, scope, ctx);
        }
        Statement::While(while_stmt) => {
            process_while(while_stmt, scope, ctx);
        }
        Statement::For(for_stmt) => {
            process_for(for_stmt, scope, ctx);
        }
        Statement::DoWhile(dw) => {
            process_do_while(dw, scope, ctx);
        }
        Statement::Try(try_stmt) => {
            process_try(try_stmt, scope, ctx);
        }
        Statement::Switch(switch) => {
            process_switch(switch, scope, ctx);
        }
        Statement::Block(block) => {
            walk_body_forward(block.statements.iter(), scope, ctx);
        }
        Statement::Unset(unset_stmt) => {
            for val in unset_stmt.values.iter() {
                if let Expression::Variable(Variable::Direct(dv)) = val {
                    scope.remove(dv.name);
                }
            }
        }
        Statement::Namespace(ns) => {
            walk_body_forward(ns.statements().iter(), scope, ctx);
        }
        Statement::Return(ret) => {
            if let Some(val) = ret.value {
                // Record `&&` chain snapshots so that member accesses
                // after an instanceof/null guard see the narrowed type.
                // E.g. `return $x instanceof Foo && $x->bar()`
                record_and_chain_snapshots(val, scope, ctx);

                // Record narrowed snapshots inside match(true) arms
                // and ternary instanceof branches.
                if is_diagnostic_scope_active() {
                    record_match_ternary_snapshots(val, scope, ctx);
                }
            }
        }
        _ => {}
    }
}

// ─── Expression statement handling ──────────────────────────────────────────

/// Process an expression statement: handle assignments, assert narrowing,
/// pass-by-reference type inference, etc.
fn process_expression_statement<'b>(
    expr_stmt: &'b ExpressionStatement<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let expr = expr_stmt.expression;

    // Try inline `/** @var Type $x */` override first.
    match try_process_inline_var_override(expr, stmt_offset(expr), scope, ctx) {
        VarOverrideResult::NamedVar => {
            // Re-record the scope snapshot at this expression's offset
            // so that variable lookups within the same statement (e.g.
            // `$app` in `$client = $app->make(...)` where a preceding
            // `@var` block declared `$app`) see the updated types.
            // The snapshot recorded by `walk_body_for_diagnostics` at
            // the statement start was taken *before* the `@var`
            // override was applied.
            record_scope_snapshot(stmt_offset(expr), scope);
            return;
        }
        VarOverrideResult::NoVar => {
            // A `@var Type` (no variable name) was applied to the
            // assignment LHS.  The override already set the LHS type,
            // so skip further assignment processing to avoid the RHS
            // overwriting the docblock type.
            return;
        }
        VarOverrideResult::None => {}
    }

    // Record intermediate scope snapshots within `&&` chains so that
    // member accesses after an instanceof/null guard see the narrowed
    // type.  E.g. `$x instanceof Foo && $x->bar()` as an expression
    // statement.
    record_and_chain_snapshots(expr, scope, ctx);

    // Record narrowed snapshots inside match(true) arms and ternary
    // instanceof branches within this expression.
    if is_diagnostic_scope_active() {
        record_match_ternary_snapshots(expr, scope, ctx);
    }

    // Process assignments.
    process_assignment_expr(expr, scope, ctx);

    // Process pass-by-reference parameter type inference.
    process_pass_by_ref(expr, scope, ctx);

    // Process assert narrowing.
    process_assert_narrowing(expr, scope, ctx);
}

/// Get the byte offset of an expression (used for cursor comparisons).
fn stmt_offset(expr: &Expression<'_>) -> u32 {
    expr.span().start.offset
}

// ─── `&&` chain narrowing for diagnostic scope snapshots ────────────────────

/// Collect operands of a `&&` chain into a left-to-right list.
///
/// `a && b && c` is parsed as `(a && b) && c`.  This function flattens
/// it into `[a, b, c]`.  Non-`&&` expressions return a single-element
/// list.
fn collect_and_chain_operands<'b>(expr: &'b Expression<'b>) -> Vec<&'b Expression<'b>> {
    let mut operands = Vec::new();
    collect_and_chain_operands_inner(expr, &mut operands);
    operands
}

fn collect_and_chain_operands_inner<'b>(
    expr: &'b Expression<'b>,
    out: &mut Vec<&'b Expression<'b>>,
) {
    if let Expression::Binary(bin) = expr
        && matches!(
            bin.operator,
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
        )
    {
        collect_and_chain_operands_inner(bin.lhs, out);
        collect_and_chain_operands_inner(bin.rhs, out);
        return;
    }
    // Also unwrap parenthesised `&&` chains.
    if let Expression::Parenthesized(inner) = expr {
        let inner_ops = collect_and_chain_operands(inner.expression);
        if inner_ops.len() > 1 {
            out.extend(inner_ops);
            return;
        }
    }
    out.push(expr);
}

/// Walk an expression tree looking for `match(true)` arms and ternary
/// `instanceof` patterns.  When found, clone the scope, apply per-arm
/// or per-branch narrowing, and record scope snapshots so that member
/// accesses inside the narrowed context see the correct variable types.
///
/// Unlike [`record_scope_snapshot_recursive`], this function does NOT
/// record snapshots at every sub-expression offset.  It only writes
/// snapshots at offsets inside match arms and ternary branches where
/// narrowing applies.  This avoids polluting the scope cache with
/// redundant entries that could conflict with `&&`-chain snapshots.
fn record_match_ternary_snapshots<'b>(
    expr: &'b Expression<'b>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    match expr {
        Expression::Match(match_expr) if match_expr.expression.is_true() => {
            for arm in match_expr.arms.iter() {
                match arm {
                    MatchArm::Expression(expr_arm) => {
                        let mut arm_scope = scope.clone();
                        for condition in expr_arm.conditions.iter() {
                            apply_condition_narrowing(condition, &mut arm_scope, ctx);
                        }
                        record_scope_snapshot(expr_arm.expression.span().start.offset, &arm_scope);
                        record_scope_snapshot_recursive(expr_arm.expression, &arm_scope);
                        // Recurse into the arm body for nested patterns.
                        record_match_ternary_snapshots(expr_arm.expression, &arm_scope, ctx);
                    }
                    MatchArm::Default(def_arm) => {
                        record_scope_snapshot(def_arm.expression.span().start.offset, scope);
                        record_scope_snapshot_recursive(def_arm.expression, scope);
                        record_match_ternary_snapshots(def_arm.expression, scope, ctx);
                    }
                }
            }
        }
        Expression::Conditional(conditional) => {
            // Only apply narrowing when the condition contains an
            // instanceof check (simple or compound OR).  General
            // truthiness/null narrowing is too broad and can produce
            // incorrect scope snapshots for arbitrary ternaries.
            let has_instanceof = {
                let var_names: Vec<String> = scope.locals.keys().cloned().collect();
                var_names.iter().any(|vn| {
                    narrowing::try_extract_instanceof(conditional.condition, vn).is_some()
                        || narrowing::try_extract_compound_or_instanceof(conditional.condition, vn)
                            .is_some()
                })
            };
            if has_instanceof {
                let mut then_scope = scope.clone();
                apply_condition_narrowing(conditional.condition, &mut then_scope, ctx);
                if let Some(then_expr) = conditional.then {
                    record_scope_snapshot(then_expr.span().start.offset, &then_scope);
                    record_scope_snapshot_recursive(then_expr, &then_scope);
                    record_match_ternary_snapshots(then_expr, &then_scope, ctx);
                }
                let mut else_scope = scope.clone();
                apply_condition_narrowing_inverse(conditional.condition, &mut else_scope, ctx);
                record_scope_snapshot(conditional.r#else.span().start.offset, &else_scope);
                record_scope_snapshot_recursive(conditional.r#else, &else_scope);
                record_match_ternary_snapshots(conditional.r#else, &else_scope, ctx);
            } else {
                // No instanceof — just recurse for nested patterns.
                if let Some(then_expr) = conditional.then {
                    record_match_ternary_snapshots(then_expr, scope, ctx);
                }
                record_match_ternary_snapshots(conditional.r#else, scope, ctx);
            }
        }
        Expression::Assignment(assignment) => {
            record_match_ternary_snapshots(assignment.rhs, scope, ctx);
        }
        Expression::Parenthesized(inner) => {
            record_match_ternary_snapshots(inner.expression, scope, ctx);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => {
                    record_match_ternary_snapshots(fc.function, scope, ctx);
                    &fc.argument_list
                }
                Call::Method(mc) => {
                    record_match_ternary_snapshots(mc.object, scope, ctx);
                    &mc.argument_list
                }
                Call::NullSafeMethod(mc) => {
                    record_match_ternary_snapshots(mc.object, scope, ctx);
                    &mc.argument_list
                }
                Call::StaticMethod(sc) => &sc.argument_list,
            };
            for arg in args.arguments.iter() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                record_match_ternary_snapshots(arg_expr, scope, ctx);
            }
        }
        Expression::Binary(bin) => {
            record_match_ternary_snapshots(bin.lhs, scope, ctx);
            record_match_ternary_snapshots(bin.rhs, scope, ctx);
        }
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                let elem_expr = match elem {
                    ArrayElement::KeyValue(kv) => {
                        record_match_ternary_snapshots(kv.key, scope, ctx);
                        kv.value
                    }
                    ArrayElement::Value(val) => val.value,
                    ArrayElement::Variadic(v) => v.value,
                    ArrayElement::Missing(_) => continue,
                };
                record_match_ternary_snapshots(elem_expr, scope, ctx);
            }
        }
        // Match expressions where the subject is NOT `true` — just
        // recurse into arm expressions.
        Expression::Match(match_expr) => {
            for arm in match_expr.arms.iter() {
                let arm_expr = match arm {
                    MatchArm::Expression(e) => e.expression,
                    MatchArm::Default(d) => d.expression,
                };
                record_match_ternary_snapshots(arm_expr, scope, ctx);
            }
        }
        _ => {}
    }
}

// ─── Completion-path ternary/match(true) narrowing ──────────────────────────

/// Walk an expression tree looking for a `match(true)` arm or ternary
/// `instanceof` branch that contains the cursor.  When found, apply
/// the appropriate narrowing to `scope` so that variable lookups see
/// the narrowed type.
///
/// This is the completion-path counterpart of
/// [`record_match_ternary_snapshots`], which records scope snapshots
/// for the diagnostic path.  Here we modify the live scope in-place
/// because the completion path only needs one variable's type at one
/// cursor position.
fn apply_cursor_ternary_narrowing<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let cursor = ctx.cursor_offset;
    let span = expr.span();
    if cursor < span.start.offset || cursor > span.end.offset {
        return;
    }

    match expr {
        Expression::Match(match_expr) if match_expr.expression.is_true() => {
            for arm in match_expr.arms.iter() {
                match arm {
                    MatchArm::Expression(expr_arm) => {
                        let arm_span = expr_arm.expression.span();
                        if cursor >= arm_span.start.offset && cursor <= arm_span.end.offset {
                            for condition in expr_arm.conditions.iter() {
                                apply_condition_narrowing(condition, scope, ctx);
                            }
                            // Recurse into the arm body for nested patterns.
                            apply_cursor_ternary_narrowing(expr_arm.expression, scope, ctx);
                            return;
                        }
                    }
                    MatchArm::Default(def_arm) => {
                        let arm_span = def_arm.expression.span();
                        if cursor >= arm_span.start.offset && cursor <= arm_span.end.offset {
                            apply_cursor_ternary_narrowing(def_arm.expression, scope, ctx);
                            return;
                        }
                    }
                }
            }
        }
        Expression::Conditional(conditional) => {
            // Check if the condition contains an instanceof check for
            // any variable currently in scope.
            let has_instanceof = {
                let var_names: Vec<String> = scope.locals.keys().cloned().collect();
                var_names.iter().any(|vn| {
                    narrowing::try_extract_instanceof(conditional.condition, vn).is_some()
                        || narrowing::try_extract_instanceof_with_negation(
                            conditional.condition,
                            vn,
                        )
                        .is_some()
                        || narrowing::try_extract_compound_or_instanceof(conditional.condition, vn)
                            .is_some()
                })
            };
            if has_instanceof {
                if let Some(then_expr) = conditional.then {
                    let then_span = then_expr.span();
                    if cursor >= then_span.start.offset && cursor <= then_span.end.offset {
                        apply_condition_narrowing(conditional.condition, scope, ctx);
                        apply_cursor_ternary_narrowing(then_expr, scope, ctx);
                        return;
                    }
                }
                let else_span = conditional.r#else.span();
                if cursor >= else_span.start.offset && cursor <= else_span.end.offset {
                    apply_condition_narrowing_inverse(conditional.condition, scope, ctx);
                    apply_cursor_ternary_narrowing(conditional.r#else, scope, ctx);
                }
            } else {
                // No instanceof — just recurse for nested patterns.
                if let Some(then_expr) = conditional.then {
                    apply_cursor_ternary_narrowing(then_expr, scope, ctx);
                }
                apply_cursor_ternary_narrowing(conditional.r#else, scope, ctx);
            }
        }
        Expression::Assignment(assignment) => {
            apply_cursor_ternary_narrowing(assignment.rhs, scope, ctx);
        }
        Expression::Parenthesized(inner) => {
            apply_cursor_ternary_narrowing(inner.expression, scope, ctx);
        }
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => {
                    apply_cursor_ternary_narrowing(fc.function, scope, ctx);
                    &fc.argument_list
                }
                Call::Method(mc) => {
                    apply_cursor_ternary_narrowing(mc.object, scope, ctx);
                    &mc.argument_list
                }
                Call::NullSafeMethod(mc) => {
                    apply_cursor_ternary_narrowing(mc.object, scope, ctx);
                    &mc.argument_list
                }
                Call::StaticMethod(_) => return,
            };
            for arg in args.arguments.iter() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                apply_cursor_ternary_narrowing(arg_expr, scope, ctx);
            }
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            // `&&` chain: apply narrowing from LHS operands when the
            // cursor is in the RHS.  E.g. `$x instanceof Foo && $x->bar()`
            // narrows `$x` to `Foo` for the `$x->bar()` operand.
            let operands = collect_and_chain_operands(expr);
            if operands.len() >= 2 {
                let mut narrowed = false;
                for (i, operand) in operands.iter().enumerate() {
                    let op_span = operand.span();
                    if cursor >= op_span.start.offset && cursor <= op_span.end.offset {
                        // Cursor is inside this operand — apply
                        // narrowing from all preceding operands.
                        // (Already applied cumulatively in the loop.)
                        narrowed = true;
                        apply_cursor_ternary_narrowing(operand, scope, ctx);
                        break;
                    }
                    // Apply this operand's narrowing for subsequent operands.
                    if i < operands.len() - 1 {
                        apply_condition_narrowing(operand, scope, ctx);
                    }
                }
                if !narrowed {
                    // Cursor not inside any operand — just recurse.
                    apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
                    apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
                }
            } else {
                apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
                apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
            }
        }
        Expression::Binary(bin) => {
            apply_cursor_ternary_narrowing(bin.lhs, scope, ctx);
            apply_cursor_ternary_narrowing(bin.rhs, scope, ctx);
        }
        // Non-`true` match expressions — recurse into arms.
        Expression::Match(match_expr) => {
            for arm in match_expr.arms.iter() {
                let arm_expr = match arm {
                    MatchArm::Expression(e) => e.expression,
                    MatchArm::Default(d) => d.expression,
                };
                apply_cursor_ternary_narrowing(arm_expr, scope, ctx);
            }
        }
        _ => {}
    }
}

/// Record intermediate scope snapshots within `&&` chains.
///
/// When the diagnostic scope cache is active and an expression contains
/// a `&&` chain, this function:
///
/// 1. Collects the operands left-to-right.
/// 2. For each operand after the first, applies instanceof and null
///    narrowing from all previous operands to a temporary scope.
/// 3. Records a scope snapshot at the operand's byte offset so that
///    diagnostic member-access lookups within the operand see the
///    narrowed types.
///
/// This fixes patterns like:
/// - `return $x instanceof Foo && $x->bar()` — `$x` narrowed to `Foo`
///   for the `$x->bar()` span.
/// - `$x !== null && $x->method()` — `$x` narrowed to non-null for
///   the `$x->method()` span.
///
/// The narrowing is applied only to snapshots — it does NOT mutate the
/// caller's scope, so subsequent statements see the original types.
fn record_and_chain_snapshots<'b>(
    expr: &'b Expression<'b>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    if !is_diagnostic_scope_active() {
        return;
    }

    let operands = collect_and_chain_operands(expr);
    if operands.len() < 2 {
        // Not a `&&` chain — nothing to do.  The scope snapshot at
        // the statement boundary already covers single expressions.
        // Match(true) and ternary narrowing are handled separately
        // by `record_match_ternary_snapshots`.
        return;
    }

    // Apply narrowing cumulatively: each operand sees the narrowing
    // from all previous operands.
    let mut narrowed_scope = scope.clone();
    for (i, operand) in operands.iter().enumerate() {
        if i == 0 {
            // First operand: apply its narrowing for subsequent operands.
            apply_condition_narrowing(operand, &mut narrowed_scope, ctx);
            continue;
        }

        // Record a snapshot at this operand's start offset so that
        // member accesses within it see the narrowed types.
        record_scope_snapshot(operand.span().start.offset, &narrowed_scope);

        // Also recurse into sub-expressions of this operand that might
        // contain member accesses at deeper byte offsets.  For example,
        // `is_array($x->errorInfo)` — the access `$x->errorInfo` is
        // inside a function call argument.
        record_scope_snapshot_recursive(operand, &narrowed_scope);

        // Apply this operand's narrowing for the next operand.
        apply_condition_narrowing(operand, &mut narrowed_scope, ctx);
    }
}

/// Recursively record scope snapshots at every sub-expression offset
/// within an expression.  This ensures that member accesses nested
/// inside function calls, array accesses, ternaries, etc. within a
/// `&&` chain operand see the narrowed scope.
fn record_scope_snapshot_recursive(expr: &Expression<'_>, scope: &ScopeState) {
    match expr {
        Expression::Call(call) => {
            let args = match call {
                Call::Function(fc) => {
                    // Record at the function call's argument list.
                    for arg in fc.argument_list.arguments.iter() {
                        let arg_expr = match arg {
                            Argument::Positional(a) => a.value,
                            Argument::Named(a) => a.value,
                        };
                        record_scope_snapshot(arg_expr.span().start.offset, scope);
                        record_scope_snapshot_recursive(arg_expr, scope);
                    }
                    return;
                }
                Call::Method(mc) => {
                    record_scope_snapshot(mc.object.span().start.offset, scope);
                    record_scope_snapshot_recursive(mc.object, scope);
                    &mc.argument_list
                }
                Call::NullSafeMethod(mc) => {
                    record_scope_snapshot(mc.object.span().start.offset, scope);
                    record_scope_snapshot_recursive(mc.object, scope);
                    &mc.argument_list
                }
                Call::StaticMethod(sc) => &sc.argument_list,
            };
            for arg in args.arguments.iter() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                record_scope_snapshot(arg_expr.span().start.offset, scope);
                record_scope_snapshot_recursive(arg_expr, scope);
            }
        }
        Expression::Access(access) => match access {
            Access::Property(pa) => {
                record_scope_snapshot(pa.object.span().start.offset, scope);
                record_scope_snapshot_recursive(pa.object, scope);
            }
            Access::NullSafeProperty(pa) => {
                record_scope_snapshot(pa.object.span().start.offset, scope);
                record_scope_snapshot_recursive(pa.object, scope);
            }
            Access::StaticProperty(sp) => {
                record_scope_snapshot(sp.span().start.offset, scope);
            }
            Access::ClassConstant(cc) => {
                record_scope_snapshot(cc.span().start.offset, scope);
            }
        },
        Expression::Parenthesized(inner) => {
            record_scope_snapshot(inner.expression.span().start.offset, scope);
            record_scope_snapshot_recursive(inner.expression, scope);
        }
        Expression::Binary(bin) => {
            record_scope_snapshot(bin.lhs.span().start.offset, scope);
            record_scope_snapshot_recursive(bin.lhs, scope);
            record_scope_snapshot(bin.rhs.span().start.offset, scope);
            record_scope_snapshot_recursive(bin.rhs, scope);
        }
        Expression::UnaryPrefix(prefix) => {
            record_scope_snapshot(prefix.operand.span().start.offset, scope);
            record_scope_snapshot_recursive(prefix.operand, scope);
        }
        Expression::Conditional(conditional) => {
            if let Some(then_expr) = conditional.then {
                record_scope_snapshot(then_expr.span().start.offset, scope);
                record_scope_snapshot_recursive(then_expr, scope);
            }
            record_scope_snapshot(conditional.r#else.span().start.offset, scope);
            record_scope_snapshot_recursive(conditional.r#else, scope);
        }
        Expression::ArrayAccess(aa) => {
            record_scope_snapshot(aa.array.span().start.offset, scope);
            record_scope_snapshot_recursive(aa.array, scope);
        }
        _ => {}
    }
}

/// Try to process an inline `/** @var Type $x */` docblock override.
///
/// Returns `true` if an override was found and applied.
/// Result of [`try_process_inline_var_override`].
enum VarOverrideResult {
    /// No `@var` docblock found.
    None,
    /// A `@var Type $varName` block (with explicit variable name) was
    /// applied.  The caller should re-record the scope snapshot so that
    /// lookups within the same statement see the updated types.
    NamedVar,
    /// A `@var Type` block (without variable name) was applied to the
    /// assignment LHS.  The caller must NOT re-record the snapshot
    /// because the LHS variable should not be visible in the RHS.
    NoVar,
}

fn try_process_inline_var_override<'b>(
    expr: &'b Expression<'b>,
    expr_offset: u32,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> VarOverrideResult {
    // Parse the inline @var docblock at this expression's position.
    let offset = expr_offset as usize;
    if offset == 0 {
        return VarOverrideResult::None;
    }

    // Look for `/** @var Type $varName */` before this expression.
    let before = &ctx.content[..offset.min(ctx.content.len())];
    let trimmed = before.trim_end();

    // Quick check: does it end with `*/`?
    if !trimmed.ends_with("*/") {
        return VarOverrideResult::None;
    }

    // Find the docblock start.
    let doc_end = trimmed.len();
    let doc_start = if let Some(pos) = trimmed.rfind("/**") {
        pos
    } else {
        return VarOverrideResult::None;
    };

    let doc_text = &trimmed[doc_start..doc_end];

    // Try multi-@var first: a single docblock may declare several
    // variables (e.g. `/** @var App $app  @var array{…} $params */`).
    let multi = parse_all_inline_var_docblocks(doc_text, ctx);
    if !multi.is_empty() {
        // When the cursor is inside the RHS of an assignment, skip
        // overriding the LHS variable so that hover/completion on the
        // RHS sees the pre-override type.  E.g.:
        //   /** @var array<string, mixed> $response */
        //   $response = $response->json();
        // Hovering on the RHS `$response` should show `ApiResponse`,
        // not `array<string, mixed>`.
        let skip_var: Option<String> = if let Expression::Assignment(assignment) = expr {
            let rhs_span = assignment.rhs.span();
            let cursor_in_rhs = ctx.cursor_offset >= rhs_span.start.offset
                && ctx.cursor_offset <= rhs_span.end.offset;
            if cursor_in_rhs {
                if let Expression::Variable(Variable::Direct(dv)) = assignment.lhs {
                    Some(dv.name.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        for (var_name, php_type) in &multi {
            if skip_var.as_deref() == Some(var_name.as_str()) {
                continue;
            }
            let resolved = resolve_type_to_resolved_types(php_type, ctx);
            scope.set(var_name.clone(), resolved);
        }
        // After processing the immediate docblock, scan backwards for
        // additional standalone docblocks that precede it.  This handles
        // patterns like:
        //   /** @var App $app  @var array{…} $params */
        //   /** @var Client $client */
        //   $client = $app->make(Client::class);
        // where the first block is separated from the expression by
        // another docblock.
        apply_preceding_var_docblocks(&trimmed[..doc_start], scope, ctx);

        // When the @var variable names all differ from the assignment
        // LHS, return None so the caller continues processing the
        // assignment.  E.g.:
        //   /** @var Foo[] $items */
        //   $item = array_shift($items);
        // The @var sets `$items` in scope (done above), and the caller
        // must also process `$item = array_shift($items)`.
        //
        // When any @var name matches the LHS, return NamedVar so the
        // caller skips the assignment (the @var type is authoritative).
        if let Expression::Assignment(assignment) = expr
            && let Expression::Variable(Variable::Direct(dv)) = assignment.lhs
        {
            let lhs_name = dv.name.to_string();
            if !multi.iter().any(|(n, _)| *n == lhs_name) {
                return VarOverrideResult::None;
            }
        }
        return VarOverrideResult::NamedVar;
    }

    // Also check for `/** @var Type */` without variable name — this
    // applies to the immediately following expression if it's a simple
    // variable or assignment.
    if let Some(php_type) = parse_inline_var_docblock_no_var(doc_text, ctx) {
        let resolved = resolve_type_to_resolved_types(&php_type, ctx);
        if let Expression::Assignment(assignment) = expr {
            if let Expression::Variable(Variable::Direct(dv)) = assignment.lhs {
                // When the cursor is inside the RHS, skip the override
                // so that the variable retains its pre-assignment type.
                // E.g. `/** @var array<string, mixed> */ $data = $data->toArray()`
                // — the cursor on `$data->` in the RHS should see Data, not array.
                let rhs_span = assignment.rhs.span();
                let cursor_in_rhs = ctx.cursor_offset >= rhs_span.start.offset
                    && ctx.cursor_offset <= rhs_span.end.offset;
                if cursor_in_rhs {
                    return VarOverrideResult::None;
                }

                // Scalar-blocking: when the RHS resolves to a concrete
                // scalar type (string, int, bool, etc.), reject a class
                // `@var` override.  E.g. `/** @var Session */ $s =
                // $this->getName()` where `getName()` returns `string`
                // should NOT override `$s` to `Session`.
                let native_type = resolve_rhs_native_type(assignment.rhs, scope, ctx);
                if let Some(ref native) = native_type
                    && !crate::docblock::should_override_type_typed(&php_type, native)
                {
                    // The override was rejected (scalar blocking).
                    return VarOverrideResult::None;
                }

                let var_name = dv.name.to_string();
                scope.set(var_name, resolved);
                // Scan for preceding docblocks.
                apply_preceding_var_docblocks(&trimmed[..doc_start], scope, ctx);
                return VarOverrideResult::NoVar;
            }
        } else if let Expression::Variable(Variable::Direct(dv)) = expr {
            let var_name = dv.name.to_string();
            scope.set(var_name, resolved);
            apply_preceding_var_docblocks(&trimmed[..doc_start], scope, ctx);
            return VarOverrideResult::NoVar;
        }
    }

    VarOverrideResult::None
}

/// Extract the native type of an RHS expression using the current scope.
///
/// Used by [`try_process_inline_var_override`] to determine whether a
/// `@var` override should be blocked by a scalar native type.
///
/// This delegates to [`super::resolution::extract_native_type_from_rhs`]
/// via a `VarResolutionCtx` that has scope-based variable resolution.
/// That function already handles method calls, function calls, static
/// calls, casts, literals, and other patterns — including extracting
/// scalar return types from method signatures.
fn resolve_rhs_native_type(
    rhs: &Expression<'_>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<PhpType> {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = move |vn: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(vn).cloned().unwrap_or_default()
    };
    let var_ctx = ctx.var_ctx_for_with_scope("$__rhs_check", 0, &scope_resolver);
    super::resolution::extract_native_type_from_rhs(rhs, &var_ctx)
}

/// Scan backwards through `before` (content before a docblock we already
/// processed) for additional standalone `/** @var Type $var */` blocks.
/// Each discovered block's `@var` tags are applied to `scope`.  Stops as
/// soon as the text no longer ends with `*/` (after trimming).
fn apply_preceding_var_docblocks(before: &str, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    let mut remaining = before.trim_end();
    // Keep scanning as long as the preceding text ends with a docblock.
    while remaining.ends_with("*/") {
        let doc_end = remaining.len();
        let doc_start = match remaining.rfind("/**") {
            Some(pos) => pos,
            None => break,
        };
        let doc_text = &remaining[doc_start..doc_end];
        let vars = parse_all_inline_var_docblocks(doc_text, ctx);
        if vars.is_empty() {
            // Not a @var docblock — stop scanning.
            break;
        }
        for (var_name, php_type) in &vars {
            let resolved = resolve_type_to_resolved_types(php_type, ctx);
            scope.set(var_name.clone(), resolved);
        }
        remaining = remaining[..doc_start].trim_end();
    }
}

/// Parse `/** @var Type $varName */` and return (var_name, PhpType).
/// Resolve a [`PhpType`] to a complete `Vec<ResolvedType>` with
/// `class_info` populated when possible.  Falls back to a
/// type-string-only entry for scalars and unresolvable types.
fn resolve_type_to_resolved_types(
    php_type: &PhpType,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    let classes = crate::completion::type_resolution::type_hint_to_classes_typed(
        php_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );
    if !classes.is_empty() {
        ResolvedType::from_classes_with_hint(classes, php_type.clone())
    } else {
        vec![ResolvedType::from_type_string(php_type.clone())]
    }
}

/// Parse ALL `@var Type $varName` pairs from a docblock.  Returns an
/// empty vec when none are found.  Handles multi-line docblocks like:
/// ```text
/// /**
///  * @var App                      $app
///  * @var array{indexName: string} $params
///  */
/// ```
fn parse_all_inline_var_docblocks(
    doc_text: &str,
    _ctx: &ForwardWalkCtx<'_>,
) -> Vec<(String, PhpType)> {
    let inner = match doc_text
        .strip_prefix("/**")
        .and_then(|s| s.strip_suffix("*/"))
    {
        Some(s) => s,
        None => return vec![],
    };

    let mut results = Vec::new();

    // Split on `@var` and process each occurrence.
    let mut search_from = 0;
    while let Some(pos) = inner[search_from..].find("@var") {
        let abs_pos = search_from + pos;
        let after = inner[abs_pos + 4..].trim_start();

        // Find the `$` that starts the variable name.  The type string
        // may contain spaces (e.g. `array<string, int>`).
        if let Some(dollar_pos) = after.find('$') {
            if dollar_pos > 0
                && let type_str = after[..dollar_pos].trim()
                && !type_str.is_empty()
                && let rest = &after[dollar_pos..]
                && let Some(var_name) = rest.split_whitespace().next()
                && !var_name.is_empty()
            {
                let php_type = PhpType::parse(type_str);
                results.push((var_name.to_string(), php_type));
            }
            search_from = abs_pos + 4 + dollar_pos + 1;
        } else {
            // No `$` after this @var — skip it.
            search_from = abs_pos + 4;
        }
    }

    results
}

fn parse_inline_var_docblock(
    doc_text: &str,
    _ctx: &ForwardWalkCtx<'_>,
) -> Option<(String, PhpType)> {
    // Strip `/**` and `*/`
    let inner = doc_text.strip_prefix("/**")?.strip_suffix("*/")?.trim();

    // Look for `@var Type $varName`
    let inner = inner
        .strip_prefix("@var")
        .or_else(|| inner.strip_prefix("* @var"))?;
    let inner = inner.trim_start();

    // Split into type and variable parts.
    // The type may contain spaces (e.g. `array<string, int>`) so we need
    // to find the `$` that starts the variable name.
    let dollar_pos = inner.find('$')?;
    if dollar_pos == 0 {
        // `@var $var Type` format — variable comes first.
        // Not the typical format, skip for now.
        return None;
    }

    let type_str = inner[..dollar_pos].trim();
    let rest = &inner[dollar_pos..];
    let var_name = rest.split_whitespace().next()?;

    if type_str.is_empty() || var_name.is_empty() {
        return None;
    }

    let php_type = PhpType::parse(type_str);

    Some((var_name.to_string(), php_type))
}

/// Parse `/** @var Type */` (without variable name) and return the PhpType.
fn parse_inline_var_docblock_no_var(doc_text: &str, _ctx: &ForwardWalkCtx<'_>) -> Option<PhpType> {
    let inner = doc_text.strip_prefix("/**")?.strip_suffix("*/")?.trim();

    let inner = inner
        .strip_prefix("@var")
        .or_else(|| inner.strip_prefix("* @var"))?;
    let inner = inner.trim();

    // If there's a `$` it has a variable name — not the no-var form.
    if inner.contains('$') {
        return None;
    }

    if inner.is_empty() {
        return None;
    }

    Some(PhpType::parse(inner))
}

/// Process assignment expressions, updating the scope.
fn process_assignment_expr<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    if let Expression::Assignment(assignment) = expr {
        if !assignment.operator.is_assign() {
            return;
        }

        // Array destructuring: `[$a, $b] = …` / `list($a, $b) = …`
        if matches!(assignment.lhs, Expression::Array(_) | Expression::List(_)) {
            process_destructuring_assignment(assignment, scope, ctx);
            return;
        }

        // Array key assignment: `$var['key'] = expr;`
        if let Expression::ArrayAccess(array_access) = assignment.lhs {
            process_array_key_assignment(array_access, assignment, scope, ctx);
            return;
        }

        // Array push: `$var[] = expr;`
        if let Expression::ArrayAppend(array_append) = assignment.lhs {
            if let Expression::Variable(Variable::Direct(dv)) = array_append.array {
                let var_name = dv.name.to_string();
                let rhs_types = resolve_rhs_with_scope(assignment.rhs, scope, ctx);
                if !rhs_types.is_empty() {
                    let value_type = ResolvedType::types_joined(&rhs_types);
                    let base_type = scope
                        .get(&var_name)
                        .last()
                        .map(|rt| rt.type_string.clone())
                        .unwrap_or_else(PhpType::array);
                    if !base_type.is_array_shape() {
                        let merged = super::resolution::merge_push_type(&base_type, &value_type);
                        scope.set(var_name, vec![ResolvedType::from_type_string(merged)]);
                    }
                }
            }
            return;
        }

        // Simple variable assignment: `$var = expr;`
        let lhs_name = match assignment.lhs {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => return,
        };

        // When the cursor is inside the RHS of this assignment, skip
        // storing the new type so that variable lookups within the RHS
        // see the pre-assignment type.  E.g. in `$request = new Bar(
        // name: $request->)`, the cursor on `$request->` should see
        // the old `Foo` type, not the new `Bar` type.
        let rhs_span = assignment.rhs.span();
        let cursor_in_rhs =
            ctx.cursor_offset >= rhs_span.start.offset && ctx.cursor_offset <= rhs_span.end.offset;
        if cursor_in_rhs {
            return;
        }

        let rhs_types = resolve_rhs_with_scope(assignment.rhs, scope, ctx);
        if !rhs_types.is_empty() {
            scope.set(lhs_name, rhs_types);
        } else if !scope.contains(&lhs_name) {
            scope.set_empty(lhs_name);
        }
    }
}

/// Resolve the type of an RHS expression using the current scope.
///
/// This is the key integration point: instead of calling
/// `resolve_variable_types` (which would recurse), we build a
/// `VarResolutionCtx` that already has the answer for any variable
/// references in the RHS — the forward walker has already resolved
/// them.
///
/// We delegate to `resolve_rhs_expression` with a `VarResolutionCtx`
/// whose `scope_var_resolver` reads directly from the forward walker's
/// in-progress `ScopeState`.  For bare variable references in the RHS,
/// we intercept them and return the scope-based result directly.
fn resolve_rhs_with_scope<'b>(
    rhs: &'b Expression<'b>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    // For bare variable references, read directly from scope.
    // This is the O(1) path that replaces the recursive backward scan.
    if let Expression::Variable(Variable::Direct(dv)) = rhs {
        let var_name = dv.name.to_string();
        let from_scope = scope.get(&var_name);
        if !from_scope.is_empty() {
            return from_scope.to_vec();
        }
        // Variable not in scope — fall through to rhs_resolution which
        // handles some special patterns.
    }

    // ── Foo::class → class-string<Foo> ──────────────────────────
    // `Foo::class` is parsed as `Access::ClassConstant` with the
    // identifier `class`.  resolve_rhs_expression doesn't return a
    // useful type for this (it looks for a constant named "class"
    // on the class and finds nothing).  Handle it here so that
    // subsequent `new $var` can resolve the class-string.
    if let Expression::Access(Access::ClassConstant(cca)) = rhs
        && let ClassLikeConstantSelector::Identifier(ident) = &cca.constant
        && ident.value == "class"
    {
        let class_name = match cca.class {
            Expression::Identifier(id) => Some(id.value().to_string()),
            Expression::Self_(_) | Expression::Static(_) => {
                if !ctx.current_class.name.is_empty() {
                    Some(ctx.current_class.name.clone())
                } else {
                    None
                }
            }
            Expression::Parent(_) => ctx.current_class.parent_class.clone(),
            _ => None,
        };
        if let Some(name) = class_name {
            let resolved_name = name.strip_prefix('\\').unwrap_or(&name);
            // Resolve the class so we can store a proper ResolvedType
            // with class_info.  This allows `new $var` to work.
            let classes = crate::completion::type_resolution::type_hint_to_classes_typed(
                &PhpType::Named(resolved_name.to_string()),
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            if !classes.is_empty() {
                return ResolvedType::from_classes(classes);
            }
            // Even if we can't resolve the class, return a type-string-only result
            // so the variable is non-empty in scope.
            return vec![ResolvedType::from_type_string(PhpType::Named(
                resolved_name.to_string(),
            ))];
        }
    }

    // ── Fast paths for expressions whose type is known structurally ──
    // These avoid the full resolve_rhs_expression round-trip for
    // common patterns where the result type depends only on the
    // expression kind, not on the operand types.

    // Type casts: (int)$x → int, (string)$x → string, etc.
    if let Expression::UnaryPrefix(prefix) = rhs {
        use mago_syntax::ast::unary::UnaryPrefixOperator;
        let cast_type = match &prefix.operator {
            UnaryPrefixOperator::IntCast(..) | UnaryPrefixOperator::IntegerCast(..) => {
                Some(PhpType::int())
            }
            UnaryPrefixOperator::StringCast(..) | UnaryPrefixOperator::BinaryCast(..) => {
                Some(PhpType::string())
            }
            UnaryPrefixOperator::FloatCast(..)
            | UnaryPrefixOperator::DoubleCast(..)
            | UnaryPrefixOperator::RealCast(..) => Some(PhpType::float()),
            UnaryPrefixOperator::BoolCast(..) | UnaryPrefixOperator::BooleanCast(..) => {
                Some(PhpType::bool())
            }
            UnaryPrefixOperator::ArrayCast(..) => Some(PhpType::array()),
            UnaryPrefixOperator::ObjectCast(..) => Some(PhpType::Named("stdClass".into())),
            UnaryPrefixOperator::UnsetCast(..) => Some(PhpType::Named("null".into())),
            UnaryPrefixOperator::Negation(_) | UnaryPrefixOperator::Plus(_) => {
                // Unary +/- preserves int or float; conservatively
                // return int|float.
                Some(PhpType::Union(vec![PhpType::int(), PhpType::float()]))
            }
            UnaryPrefixOperator::BitwiseNot(_) => Some(PhpType::int()),
            UnaryPrefixOperator::Not(_) => Some(PhpType::bool()),
            _ => None,
        };
        if let Some(ty) = cast_type {
            return vec![ResolvedType::from_type_string(ty)];
        }
    }

    // For all other expressions, delegate to the existing RHS resolver
    // with a scope-based variable resolver injected.  When
    // `resolve_rhs_expression` (or its sub-functions like
    // `resolve_rhs_method_call_inner`, `resolve_rhs_property_access`)
    // need to resolve a variable's type, they call `resolve_var_types`
    // which checks `scope_var_resolver` first.  This reads directly
    // from the forward walker's in-progress `ScopeState`, bypassing
    // `resolve_variable_types` entirely.
    let rhs_offset = rhs.span().start.offset;
    let dummy_var = "$__rhs";
    let scope_locals = &scope.locals;
    let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
        scope_locals.get(var_name).cloned().unwrap_or_default()
    };
    let var_ctx = ctx.var_ctx_for_with_scope(dummy_var, rhs_offset, &scope_resolver);

    let result = super::rhs_resolution::resolve_rhs_expression(rhs, &var_ctx);
    if !result.is_empty() {
        return result;
    }

    // ── Structural fallbacks ────────────────────────────────────
    // When resolve_rhs_expression returns empty, infer the type
    // purely from the expression structure.  These only fire as a
    // last resort so they never override a more precise result.

    // String literals (including interpolated/composite strings).
    if matches!(
        rhs,
        Expression::Literal(Literal::String(_)) | Expression::CompositeString(_)
    ) {
        return vec![ResolvedType::from_type_string(PhpType::string())];
    }

    // Integer literals.
    if matches!(rhs, Expression::Literal(Literal::Integer(_))) {
        return vec![ResolvedType::from_type_string(PhpType::int())];
    }

    // Float literals.
    if matches!(rhs, Expression::Literal(Literal::Float(_))) {
        return vec![ResolvedType::from_type_string(PhpType::float())];
    }

    // Boolean and null literals.
    if matches!(
        rhs,
        Expression::Literal(Literal::True(_) | Literal::False(_))
    ) {
        return vec![ResolvedType::from_type_string(PhpType::bool())];
    }
    if matches!(rhs, Expression::Literal(Literal::Null(_))) {
        return vec![ResolvedType::from_type_string(PhpType::Named(
            "null".into(),
        ))];
    }

    // Binary operators — the result type depends on the operator kind.
    if let Expression::Binary(binary) = rhs {
        use mago_syntax::ast::binary::BinaryOperator;

        // instanceof, comparison, logical: always bool.
        if binary.operator.is_instanceof()
            || binary.operator.is_comparison()
            || binary.operator.is_logical()
        {
            return vec![ResolvedType::from_type_string(PhpType::bool())];
        }

        // Concatenation (.): always string.
        if matches!(binary.operator, BinaryOperator::StringConcat(_)) {
            return vec![ResolvedType::from_type_string(PhpType::string())];
        }

        // Modulo (%): always int.
        if matches!(binary.operator, BinaryOperator::Modulo(_)) {
            return vec![ResolvedType::from_type_string(PhpType::int())];
        }

        // Arithmetic: -, *, /, ** are always numeric.  + is excluded
        // because PHP overloads it for array union.
        if matches!(
            binary.operator,
            BinaryOperator::Subtraction(_)
                | BinaryOperator::Multiplication(_)
                | BinaryOperator::Division(_)
                | BinaryOperator::Exponentiation(_)
        ) {
            return vec![ResolvedType::from_type_string(PhpType::Union(vec![
                PhpType::int(),
                PhpType::float(),
            ]))];
        }

        // Bitwise operators (&, |, ^, <<, >>): always int.
        if matches!(
            binary.operator,
            BinaryOperator::BitwiseAnd(_)
                | BinaryOperator::BitwiseOr(_)
                | BinaryOperator::BitwiseXor(_)
                | BinaryOperator::LeftShift(_)
                | BinaryOperator::RightShift(_)
        ) {
            return vec![ResolvedType::from_type_string(PhpType::int())];
        }
    }

    // ── Subject pipeline fallback ───────────────────────────────
    // When resolve_rhs_expression and the structural fallbacks both
    // return empty, try the full subject resolution pipeline
    // (resolve_target_classes).  This handles method calls and
    // static calls that resolve_rhs_expression cannot resolve
    // because the receiver or intermediate types are only reachable
    // through the subject pipeline's broader strategies (e.g.
    // docblock @return types, merged inheritance, virtual members).
    //
    // Property access (Expression::Access) is intentionally excluded
    // because resolve_target_classes resolves the *subject* (what
    // you'd complete after `->`) rather than the property's value
    // type.  For Eloquent relations like `$this->model->orderProducts`,
    // the subject pipeline returns the element type instead of the
    // collection, which breaks foreach value binding.  Property
    // access RHS resolution is handled by resolve_rhs_expression's
    // own property resolution path.
    if matches!(rhs, Expression::Call(_) | Expression::Instantiation(_)) {
        let rhs_span = rhs.span();
        let rhs_start = rhs_span.start.offset as usize;
        let rhs_end = rhs_span.end.offset as usize;
        if let Some(rhs_text) = ctx.content.get(rhs_start..rhs_end) {
            let rhs_text = rhs_text.trim();
            if !rhs_text.is_empty() {
                let subject_result = resolve_rhs_via_subject(rhs_text, scope, ctx);
                if !subject_result.is_empty() {
                    return subject_result;
                }
            }
        }
    }

    result
}

/// Resolve an RHS expression through the full subject pipeline.
///
/// This is a last-resort fallback for expressions that
/// `resolve_rhs_expression` can't handle.  It extracts the
/// expression text and passes it to `resolve_target_classes`, which
/// goes through SubjectExpr parsing, property/method chain
/// resolution, and the full type resolution infrastructure.
///
/// Only called for method calls, property access, static calls, and
/// instantiation — expression kinds that typically produce
/// object-typed results resolvable through the subject pipeline.
fn resolve_rhs_via_subject(
    rhs_text: &str,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<ResolvedType> {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = move |var_name: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(var_name).cloned().unwrap_or_default()
    };
    let var_ctx = ctx.var_ctx_for_with_scope("$__rhs_subject", 0, &scope_resolver);
    let rctx = var_ctx.as_resolution_ctx();

    // Determine the access kind from the expression text.
    let access_kind = if rhs_text.contains("::") {
        crate::types::AccessKind::DoubleColon
    } else {
        crate::types::AccessKind::Arrow
    };

    crate::completion::resolver::resolve_target_classes(rhs_text, access_kind, &rctx)
}

/// Process array destructuring assignments.
fn process_destructuring_assignment<'b>(
    assignment: &'b Assignment<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Delegate to the existing destructuring resolution.
    // We need to resolve the RHS type and then extract element types
    // for each destructured variable.
    //
    // Resolve via a `VarResolutionCtx` with a scope-based variable
    // resolver.

    // Extract all variable names from the LHS.
    let var_names = extract_destructured_var_names(assignment.lhs);

    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(var_name).cloned().unwrap_or_default()
    };

    for vname in var_names {
        let var_ctx = VarResolutionCtx {
            var_name: &vname,
            current_class: ctx.current_class,
            all_classes: ctx.all_classes,
            content: ctx.content,
            cursor_offset: assignment.span().start.offset,
            class_loader: ctx.class_loader,
            loaders: ctx.loaders,
            resolved_class_cache: ctx.resolved_class_cache,
            enclosing_return_type: ctx.enclosing_return_type.clone(),
            branch_aware: false,
            match_arm_narrowing: HashMap::new(),
            scope_var_resolver: Some(&scope_resolver),
        };
        let mut var_results: Vec<ResolvedType> = Vec::new();
        super::foreach_resolution::try_resolve_destructured_type(
            assignment,
            &var_ctx,
            &mut var_results,
            false,
        );
        if !var_results.is_empty() {
            scope.set(vname, var_results);
        }
    }
}

/// Extract all variable names from a destructuring LHS expression.
fn extract_destructured_var_names(expr: &Expression<'_>) -> Vec<String> {
    let mut names = Vec::new();
    match expr {
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                match elem {
                    ArrayElement::KeyValue(kv) => {
                        if let Expression::Variable(Variable::Direct(dv)) = kv.value {
                            names.push(dv.name.to_string());
                        }
                    }
                    ArrayElement::Value(val) => {
                        if let Expression::Variable(Variable::Direct(dv)) = val.value {
                            names.push(dv.name.to_string());
                        }
                    }
                    ArrayElement::Variadic(_) | ArrayElement::Missing(_) => {}
                }
            }
        }
        Expression::List(list) => {
            for elem in list.elements.iter() {
                match elem {
                    ArrayElement::KeyValue(kv) => {
                        if let Expression::Variable(Variable::Direct(dv)) = kv.value {
                            names.push(dv.name.to_string());
                        }
                    }
                    ArrayElement::Value(val) => {
                        if let Expression::Variable(Variable::Direct(dv)) = val.value {
                            names.push(dv.name.to_string());
                        }
                    }
                    ArrayElement::Variadic(_) | ArrayElement::Missing(_) => {}
                }
            }
        }
        _ => {}
    }
    names
}

/// Process array key assignment: `$var['key'] = expr;`
fn process_array_key_assignment<'b>(
    _array_access: &'b ArrayAccess<'b>,
    assignment: &'b Assignment<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Delegate to the existing check_expression_for_assignment
    // infrastructure for array key assignments.  This handles
    // both string-keyed shape building and generic element tracking.
    //
    // We iterate over all variables currently in scope and check
    // whether the assignment targets any of them.
    // For simplicity in Phase 1, use the existing path.
    // Extract the base variable name from the array access.
    if let Some((base_name, key_chain)) =
        super::resolution::extract_nested_array_access_chain(_array_access)
    {
        // Resolve the RHS value type.
        let rhs_types = resolve_rhs_with_scope(assignment.rhs, scope, ctx);
        let value_php_type = if !rhs_types.is_empty() {
            ResolvedType::types_joined(&rhs_types)
        } else {
            PhpType::mixed()
        };
        let base_type = scope
            .get(&base_name)
            .last()
            .map(|rt| rt.type_string.clone())
            .unwrap_or_else(PhpType::array);

        // Extract all keys in the chain.
        let all_string_keys: Option<Vec<String>> = key_chain
            .iter()
            .map(|idx| super::resolution::extract_array_key_for_shape(idx))
            .collect();

        if let Some(keys) = all_string_keys {
            let merged =
                super::resolution::merge_nested_shape_keys(&base_type, &keys, &value_php_type);
            scope.set(base_name, vec![ResolvedType::from_type_string(merged)]);
        } else if key_chain.len() == 1 && !base_type.is_array_shape() {
            let rhs_offset = assignment.span().start.offset;
            let scope_locals = &scope.locals;
            let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
                scope_locals.get(var_name).cloned().unwrap_or_default()
            };
            let rhs_ctx = ctx.var_ctx_for_with_scope("$__idx", rhs_offset, &scope_resolver);
            let key_php_type = super::resolution::infer_array_key_type(key_chain[0], &rhs_ctx);
            let merged =
                super::resolution::merge_keyed_type(&base_type, &key_php_type, &value_php_type);
            scope.set(base_name, vec![ResolvedType::from_type_string(merged)]);
        }
    }
}

/// Process pass-by-reference parameter type inference.
fn process_pass_by_ref<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // When a function call passes a variable to a parameter declared
    // as `Type &$param`, the variable acquires that type after the call.
    //
    // We need to check both variables already in scope AND variables
    // that appear as arguments but don't exist in scope yet (e.g.
    // `$matches` in `preg_match($pattern, $subject, $matches)`).
    //
    // Phase 1: use the existing `try_apply_pass_by_reference_type`
    // infrastructure for variables already in scope (works for class
    // types like `Type &$param`).
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(var_name).cloned().unwrap_or_default()
    };

    // Collect all variable names that appear as arguments in this
    // expression, including ones not yet in scope.
    let mut all_var_names: Vec<String> = scope.locals.keys().cloned().collect();
    for arg_var in extract_call_arg_variables(expr) {
        if !all_var_names.contains(&arg_var) {
            all_var_names.push(arg_var);
        }
    }

    for var_name in all_var_names {
        let var_ctx = VarResolutionCtx {
            var_name: &var_name,
            current_class: ctx.current_class,
            all_classes: ctx.all_classes,
            content: ctx.content,
            cursor_offset: ctx.cursor_offset,
            class_loader: ctx.class_loader,
            loaders: ctx.loaders,
            resolved_class_cache: ctx.resolved_class_cache,
            enclosing_return_type: ctx.enclosing_return_type.clone(),
            branch_aware: false,
            match_arm_narrowing: HashMap::new(),
            scope_var_resolver: Some(&scope_resolver),
        };
        let before = scope.get(&var_name).to_vec();
        let mut results = before.clone();
        super::resolution::try_apply_pass_by_reference_type(expr, &var_ctx, &mut results, false);
        if results.len() != before.len() {
            scope.set(var_name.clone(), results);
        }
    }

    // Phase 2: for variables NOT yet in scope that are passed to
    // pass-by-reference parameters with primitive type hints (e.g.
    // `array &$matches` in `preg_match`), store the type hint
    // directly.  `try_apply_pass_by_reference_type` only produces
    // results for class-based type hints; primitive types like
    // `array`, `int`, `string` return empty from
    // `type_hint_to_classes_typed` and are missed.
    seed_pass_by_ref_primitives(expr, scope, ctx);
}

/// Seed PHP superglobals (`$_SERVER`, `$_GET`, `$_POST`, etc.) into the
/// scope as `array` so that accesses on them resolve correctly.
/// PHP makes these available in every scope without
/// an explicit `global` declaration.
fn seed_superglobals(scope: &mut ScopeState) {
    let array_type = vec![ResolvedType::from_type_string(PhpType::Named(
        "array".to_string(),
    ))];
    for name in [
        "$_SERVER",
        "$_GET",
        "$_POST",
        "$_COOKIE",
        "$_REQUEST",
        "$_FILES",
        "$_ENV",
        "$_SESSION",
        "$GLOBALS",
    ] {
        scope.set(name.to_string(), array_type.clone());
    }
}

/// Recursively walk an expression tree to find function call
/// sub-expressions and seed pass-by-reference primitive types for each.
/// This handles patterns like `if (preg_match($pattern, $subject, $matches))`
/// and `if (preg_match(..., $matches) === 1)` where the call is nested
/// inside a comparison or logical expression rather than appearing as a
/// standalone expression statement.
///
/// Only uses [`seed_pass_by_ref_primitives`] (not the full
/// [`process_pass_by_ref`]) to avoid triggering recursive variable
/// resolution through `try_apply_pass_by_reference_type`, which would
/// inflate the fallthrough counter for every variable already in scope.
fn seed_pass_by_ref_in_condition<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    match expr {
        // Direct call expressions — seed primitive pass-by-ref types.
        Expression::Call(_) => {
            seed_pass_by_ref_primitives(expr, scope, ctx);
        }
        // Binary operators (e.g. `preg_match(...) === 1`, `a && b`)
        // — recurse into both sides.
        Expression::Binary(bin) => {
            seed_pass_by_ref_in_condition(bin.lhs, scope, ctx);
            seed_pass_by_ref_in_condition(bin.rhs, scope, ctx);
        }
        // Unary prefix (e.g. `!preg_match(...)`) — recurse into operand.
        Expression::UnaryPrefix(unary) => {
            seed_pass_by_ref_in_condition(unary.operand, scope, ctx);
        }
        // Unary postfix — recurse into operand.
        Expression::UnaryPostfix(unary) => {
            seed_pass_by_ref_in_condition(unary.operand, scope, ctx);
        }
        // Parenthesized — recurse into inner expression.
        Expression::Parenthesized(paren) => {
            seed_pass_by_ref_in_condition(paren.expression, scope, ctx);
        }
        // Assignment in condition (e.g. `if ($x = preg_match(..., $m))`)
        // — recurse into the RHS.
        Expression::Assignment(assignment) => {
            seed_pass_by_ref_in_condition(assignment.rhs, scope, ctx);
        }
        _ => {}
    }
}

/// For each variable argument in a call expression that is passed to a
/// pass-by-reference parameter with a primitive type hint (e.g.
/// `array &$matches`), seed the variable in scope if it isn't already
/// there.  This complements [`process_pass_by_ref`] which handles
/// class-typed parameters via `try_apply_pass_by_reference_type`.
fn seed_pass_by_ref_primitives<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Resolve the called function/method's parameters.
    let (arg_list, parameters) = match expr {
        Expression::Call(Call::Function(func_call)) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return,
            };
            let fl = match ctx.loaders.function_loader {
                Some(fl) => fl,
                None => return,
            };
            let func_info = match fl(&func_name) {
                Some(fi) => fi,
                None => return,
            };
            (&func_call.argument_list, func_info.parameters)
        }
        Expression::Call(Call::Method(mc)) => {
            let method_name = match &mc.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
                _ => return,
            };
            let receiver_class = match mc.object {
                Expression::Variable(Variable::Direct(dv)) if dv.name == "$this" => {
                    Some(ctx.current_class.name.clone())
                }
                Expression::Variable(Variable::Direct(dv)) => {
                    let types = scope.get(dv.name);
                    types.iter().find_map(|rt| {
                        let name = rt.type_string.base_name()?;
                        if crate::php_type::is_primitive_scalar_name(name) {
                            None
                        } else {
                            Some(name.to_string())
                        }
                    })
                }
                _ => return,
            };
            let class_name = match receiver_class {
                Some(n) => n,
                None => return,
            };
            let cls = match (ctx.class_loader)(&class_name) {
                Some(c) => c,
                None => return,
            };
            let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                &cls,
                ctx.class_loader,
                ctx.resolved_class_cache,
            );
            let method = match merged.methods.iter().find(|m| m.name == method_name) {
                Some(m) => m,
                None => return,
            };
            (&mc.argument_list, method.parameters.clone())
        }
        Expression::Call(Call::NullSafeMethod(mc)) => {
            let method_name = match &mc.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
                _ => return,
            };
            let receiver_class = match mc.object {
                Expression::Variable(Variable::Direct(dv)) if dv.name == "$this" => {
                    Some(ctx.current_class.name.clone())
                }
                Expression::Variable(Variable::Direct(dv)) => {
                    let types = scope.get(dv.name);
                    types.iter().find_map(|rt| {
                        let name = rt.type_string.base_name()?;
                        if crate::php_type::is_primitive_scalar_name(name) {
                            None
                        } else {
                            Some(name.to_string())
                        }
                    })
                }
                _ => return,
            };
            let class_name = match receiver_class {
                Some(n) => n,
                None => return,
            };
            let cls = match (ctx.class_loader)(&class_name) {
                Some(c) => c,
                None => return,
            };
            let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                &cls,
                ctx.class_loader,
                ctx.resolved_class_cache,
            );
            let method = match merged.methods.iter().find(|m| m.name == method_name) {
                Some(m) => m,
                None => return,
            };
            (&mc.argument_list, method.parameters.clone())
        }
        Expression::Call(Call::StaticMethod(sc)) => {
            let method_name = match &sc.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
                _ => return,
            };
            let class_name = match sc.class {
                Expression::Self_(_) | Expression::Static(_) => ctx.current_class.name.clone(),
                Expression::Parent(_) => match &ctx.current_class.parent_class {
                    Some(p) => p.clone(),
                    None => return,
                },
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return,
            };
            let cls = match (ctx.class_loader)(&class_name) {
                Some(c) => c,
                None => return,
            };
            let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                &cls,
                ctx.class_loader,
                ctx.resolved_class_cache,
            );
            let method = match merged.methods.iter().find(|m| m.name == method_name) {
                Some(m) => m,
                None => return,
            };
            (&sc.argument_list, method.parameters.clone())
        }
        _ => return,
    };

    for (i, arg) in arg_list.arguments.iter().enumerate() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };

        // Only handle direct variable arguments.
        let var_name = match arg_expr {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => continue,
        };

        // Skip if already in scope (Phase 1 handled it).
        if !scope.get(&var_name).is_empty() {
            continue;
        }

        // Check if the corresponding parameter is pass-by-reference.
        if let Some(param) = parameters.get(i)
            && param.is_reference
        {
            if let Some(type_hint) = &param.type_hint {
                scope.set(
                    var_name,
                    vec![ResolvedType::from_type_string(type_hint.clone())],
                );
            } else {
                // Untyped pass-by-reference parameters (e.g. `&$matches`
                // in `preg_match`, `&$result` in `parse_str`) are most
                // commonly arrays.  Seed as `array` so that subsequent
                // array accesses like `$matches[1]` don't fall through
                // to the backward scanner.
                scope.set(
                    var_name,
                    vec![ResolvedType::from_type_string(PhpType::Named(
                        "array".to_string(),
                    ))],
                );
            }
        }
    }
}

/// Extract all `$variable` names that appear as direct arguments in a
/// call expression.  Used by [`process_pass_by_ref`] to discover
/// variables that may be introduced by pass-by-reference parameters
/// (e.g. `$matches` in `preg_match($pattern, $subject, $matches)`).
fn extract_call_arg_variables<'b>(expr: &'b Expression<'b>) -> Vec<String> {
    let arg_list = match expr {
        Expression::Call(Call::Function(fc)) => &fc.argument_list,
        Expression::Call(Call::Method(mc)) => &mc.argument_list,
        Expression::Call(Call::NullSafeMethod(mc)) => &mc.argument_list,
        Expression::Call(Call::StaticMethod(sc)) => &sc.argument_list,
        Expression::Instantiation(inst) => match &inst.argument_list {
            Some(al) => al,
            None => return vec![],
        },
        _ => return vec![],
    };
    let mut vars = Vec::new();
    for arg in arg_list.arguments.iter() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
            vars.push(dv.name.to_string());
        }
    }
    vars
}

/// Process assert narrowing (assert($x instanceof Foo), @phpstan-assert, etc.)
fn process_assert_narrowing<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // ── Handle assert($x instanceof Foo) for variables NOT yet in scope ──
    // When a foreach binds a variable but the iterable element type is
    // unknown, the variable won't be in the scope map.  A subsequent
    // `assert($x instanceof Foo)` should add it with the asserted type.
    if let Expression::Call(Call::Function(fc)) = expr
        && matches!(fc.function, Expression::Identifier(ident) if ident.value() == "assert")
        && let Some(arg) = fc.argument_list.arguments.first()
    {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        if let Expression::Binary(bin) = arg_expr
            && bin.operator.is_instanceof()
            && let Expression::Variable(Variable::Direct(dv)) = bin.lhs
        {
            let var_name = dv.name.to_string();
            if scope.get(&var_name).is_empty() {
                // Variable not in scope — seed it with the asserted type.
                let class_name = match bin.rhs {
                    Expression::Identifier(ident) => Some(ident.value().to_string()),
                    Expression::Self_(_) => Some(ctx.current_class.name.clone()),
                    Expression::Static(_) => Some(ctx.current_class.name.clone()),
                    Expression::Parent(_) => ctx.current_class.parent_class.clone(),
                    _ => None,
                };
                if let Some(name) = class_name {
                    let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                        &PhpType::Named(name.clone()),
                        &ctx.current_class.name,
                        ctx.all_classes,
                        ctx.class_loader,
                    );
                    if !resolved.is_empty() {
                        scope.set(
                            var_name,
                            ResolvedType::from_classes_with_hint(resolved, PhpType::Named(name)),
                        );
                    } else {
                        scope.set(
                            var_name,
                            vec![ResolvedType::from_type_string(PhpType::Named(name))],
                        );
                    }
                }
            }
        }
    }

    // Apply assert narrowing to each variable in scope.
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = |var_name: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(var_name).cloned().unwrap_or_default()
    };
    let var_names: Vec<String> = scope.locals.keys().cloned().collect();
    for var_name in var_names {
        let var_ctx = VarResolutionCtx {
            var_name: &var_name,
            current_class: ctx.current_class,
            all_classes: ctx.all_classes,
            content: ctx.content,
            cursor_offset: ctx.cursor_offset,
            class_loader: ctx.class_loader,
            loaders: ctx.loaders,
            resolved_class_cache: ctx.resolved_class_cache,
            enclosing_return_type: ctx.enclosing_return_type.clone(),
            branch_aware: false,
            match_arm_narrowing: HashMap::new(),
            scope_var_resolver: Some(&scope_resolver),
        };
        let before = scope.get(&var_name).to_vec();
        let mut results = before.clone();

        // assert($x instanceof Foo)
        ResolvedType::apply_narrowing(&mut results, |classes| {
            narrowing::try_apply_assert_instanceof_narrowing(expr, &var_ctx, classes);
        });

        // @phpstan-assert / @psalm-assert
        ResolvedType::apply_narrowing(&mut results, |classes| {
            narrowing::try_apply_custom_assert_narrowing(expr, &var_ctx, classes);
        });

        if resolved_types_differ(&results, &before) {
            if results.is_empty() {
                // Narrowing removed all types (e.g. assert($x instanceof
                // UnresolvableClass)).  Explicitly clear the variable so
                // that diagnostics see "unknown type" and suppress false
                // positives.  `scope.set()` is a no-op for empty vecs.
                scope.locals.insert(var_name.clone(), vec![]);
            } else {
                scope.set(var_name.clone(), results);
            }
        }
    }
}

/// Compare two `ResolvedType` slices by their observable identity
/// (type string + class FQN).  `ResolvedType` intentionally does not
/// implement `PartialEq` because `ClassInfo` is a large struct where
/// field-by-field equality is too expensive and semantically wrong.
/// This lightweight comparison detects when narrowing changed the
/// resolved type (e.g. replaced `BaseCatalogFeature` with `self`).
fn resolved_types_differ(a: &[ResolvedType], b: &[ResolvedType]) -> bool {
    if a.len() != b.len() {
        return true;
    }
    for (ra, rb) in a.iter().zip(b.iter()) {
        if ra.type_string != rb.type_string {
            return true;
        }
        match (&ra.class_info, &rb.class_info) {
            (Some(ca), Some(cb)) => {
                if ca.fqn() != cb.fqn() {
                    return true;
                }
            }
            (None, None) => {}
            _ => return true,
        }
    }
    false
}

// ─── Control flow handling ──────────────────────────────────────────────────

/// Process an `if` statement with branch merging.
fn process_if<'b>(
    if_stmt: &'b If<'b>,
    enclosing_stmt: &'b Statement<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Record `&&` chain snapshots for the condition expression so that
    // member accesses after an instanceof/null guard within the condition
    // see the narrowed type.  E.g. `if ($x !== null && $x->method())`
    // — the `$x->method()` span needs `$x` narrowed to non-null.
    record_and_chain_snapshots(if_stmt.condition, scope, ctx);

    // Check if the cursor is inside the condition expression.
    // If so, apply inline && narrowing.
    let cond_span = if_stmt.condition.span();
    if ctx.cursor_offset >= cond_span.start.offset && ctx.cursor_offset <= cond_span.end.offset {
        // Cursor is in the condition — scope is already correct.
        return;
    }

    // Assignment in condition: `if ($x = expr())`
    process_condition_assignment(if_stmt.condition, scope, ctx);

    // Pass-by-reference in condition: `if (preg_match(..., $matches))`
    seed_pass_by_ref_in_condition(if_stmt.condition, scope, ctx);

    // Record a snapshot after condition processing so that variables
    // seeded by pass-by-reference (e.g. `$matches` from `preg_match`)
    // are visible in the then-body and elseif/else bodies.  Without
    // this, the pre-statement snapshot (recorded by the outer
    // `walk_body_forward` before `process_if` runs) would be the
    // nearest floor entry, and it predates the seeding.
    if is_diagnostic_scope_active() {
        let body_start = match &if_stmt.body {
            IfBody::Statement(body) => body.statement.span().start.offset,
            IfBody::ColonDelimited(body) => body.colon.start.offset,
        };
        record_scope_snapshot(body_start, scope);
    }

    match &if_stmt.body {
        IfBody::Statement(body) => {
            process_if_statement_body(if_stmt, body, enclosing_stmt, scope, ctx);
        }
        IfBody::ColonDelimited(body) => {
            process_if_colon_body(if_stmt, body, enclosing_stmt, scope, ctx);
        }
    }
}

/// Process if with statement body (brace-style).
fn process_if_statement_body<'b>(
    if_stmt: &'b If<'b>,
    body: &'b IfStatementBody<'b>,
    enclosing_stmt: &'b Statement<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let then_span = body.statement.span();
    let cursor_in_then =
        ctx.cursor_offset >= then_span.start.offset && ctx.cursor_offset <= then_span.end.offset;

    // Check if cursor is in any elseif body.
    let cursor_in_elseif = body.else_if_clauses.iter().any(|ei| {
        let sp = ei.statement.span();
        ctx.cursor_offset >= sp.start.offset && ctx.cursor_offset <= sp.end.offset
    });

    // Check if cursor is in else body.
    let cursor_in_else = body.else_clause.as_ref().is_some_and(|ec| {
        let sp = ec.statement.span();
        ctx.cursor_offset >= sp.start.offset && ctx.cursor_offset <= sp.end.offset
    });

    if cursor_in_then {
        // Cursor is inside the then-branch.  Apply instanceof narrowing
        // and walk only this branch.
        apply_condition_narrowing(if_stmt.condition, scope, ctx);
        walk_body_forward(std::iter::once(body.statement), scope, ctx);
        return;
    }

    if cursor_in_elseif {
        // Find which elseif contains the cursor.
        for ei in body.else_if_clauses.iter() {
            let sp = ei.statement.span();
            if ctx.cursor_offset >= sp.start.offset && ctx.cursor_offset <= sp.end.offset {
                // Apply negated narrowing from the if condition, then
                // positive narrowing from this elseif condition.
                apply_condition_narrowing_inverse(if_stmt.condition, scope, ctx);
                // Also apply inverse narrowing for preceding elseifs.
                for prev_ei in body.else_if_clauses.iter() {
                    if std::ptr::eq(prev_ei, ei) {
                        break;
                    }
                    apply_condition_narrowing_inverse(prev_ei.condition, scope, ctx);
                }
                apply_condition_narrowing(ei.condition, scope, ctx);
                process_condition_assignment(ei.condition, scope, ctx);
                seed_pass_by_ref_in_condition(ei.condition, scope, ctx);
                walk_body_forward(std::iter::once(ei.statement), scope, ctx);
                return;
            }
        }
        return;
    }

    if cursor_in_else && let Some(ref else_clause) = body.else_clause {
        // Apply inverse narrowing from all conditions.
        apply_condition_narrowing_inverse(if_stmt.condition, scope, ctx);
        for ei in body.else_if_clauses.iter() {
            apply_condition_narrowing_inverse(ei.condition, scope, ctx);
        }
        walk_body_forward(std::iter::once(else_clause.statement), scope, ctx);
        return;
    }

    // Cursor is AFTER the if/else block.  We need to merge all branches.
    let pre_if_scope = scope.clone();

    // Walk each branch independently and merge results.
    let mut then_scope = scope.clone();
    apply_condition_narrowing(if_stmt.condition, &mut then_scope, ctx);
    walk_body_forward(std::iter::once(body.statement), &mut then_scope, ctx);
    let then_exits = statement_unconditionally_exits(body.statement);

    let mut elseif_scopes: Vec<(ScopeState, bool)> = Vec::new();
    for ei in body.else_if_clauses.iter() {
        let mut ei_scope = pre_if_scope.clone();
        apply_condition_narrowing_inverse(if_stmt.condition, &mut ei_scope, ctx);
        for (prev_idx, prev_ei) in body.else_if_clauses.iter().enumerate() {
            if std::ptr::eq(prev_ei, ei) {
                break;
            }
            apply_condition_narrowing_inverse(prev_ei.condition, &mut ei_scope, ctx);
            let _ = prev_idx;
        }
        apply_condition_narrowing(ei.condition, &mut ei_scope, ctx);
        process_condition_assignment(ei.condition, &mut ei_scope, ctx);
        seed_pass_by_ref_in_condition(ei.condition, &mut ei_scope, ctx);
        walk_body_forward(std::iter::once(ei.statement), &mut ei_scope, ctx);
        let exits = statement_unconditionally_exits(ei.statement);
        elseif_scopes.push((ei_scope, exits));
    }

    let (else_scope, else_exits) = if let Some(ref else_clause) = body.else_clause {
        let mut else_scope = pre_if_scope.clone();
        apply_condition_narrowing_inverse(if_stmt.condition, &mut else_scope, ctx);
        for ei in body.else_if_clauses.iter() {
            apply_condition_narrowing_inverse(ei.condition, &mut else_scope, ctx);
        }
        walk_body_forward(std::iter::once(else_clause.statement), &mut else_scope, ctx);
        let exits = statement_unconditionally_exits(else_clause.statement);
        (Some(else_scope), exits)
    } else {
        (None, false)
    };

    // Merge: collect all surviving (non-exiting) branch scopes.
    // Branches that exit via break/continue are loop-local exits —
    // their variable assignments flow to the post-loop scope, so
    // they must be included in the merge alongside truly surviving
    // branches.
    let mut surviving_scopes: Vec<&ScopeState> = Vec::new();

    let then_exits_via_loop = exits_via_loop_control(body.statement);
    if !then_exits || then_exits_via_loop {
        surviving_scopes.push(&then_scope);
    }
    for (ei_scope, ei_exits) in &elseif_scopes {
        if !ei_exits {
            surviving_scopes.push(ei_scope);
        }
    }
    if let Some(ref es) = else_scope {
        if !else_exits {
            surviving_scopes.push(es);
        }
    } else {
        // No else clause — the pre-if scope is an implicit surviving path.
        surviving_scopes.push(&pre_if_scope);
    }

    if surviving_scopes.is_empty() {
        // All branches exit — theoretically unreachable code after.
        // Keep the pre-if scope.
        *scope = pre_if_scope;
    } else if surviving_scopes.len() == 1 {
        *scope = surviving_scopes[0].clone();
    } else {
        // Merge all surviving scopes.
        let mut merged = surviving_scopes[0].clone();
        for s in &surviving_scopes[1..] {
            merged.merge_branch(s);
        }
        *scope = merged;
    }

    // Guard clause narrowing: when the if body unconditionally exits
    // and there are no elseif/else branches, apply inverse narrowing.
    // This applies to ALL exit types (return, throw, break, continue)
    // because the code after the if in the current scope does not
    // execute in that path.  Break/continue branch scopes are already
    // included in `surviving_scopes` above so their variable
    // assignments are preserved in the merge.
    if enclosing_stmt.span().end.offset < ctx.cursor_offset
        && then_exits
        && body.else_if_clauses.is_empty()
        && body.else_clause.is_none()
    {
        apply_condition_narrowing_inverse(if_stmt.condition, scope, ctx);
        apply_guard_clause_null_narrowing(if_stmt, scope, ctx);
    }
}

/// Process if with colon-delimited body.
fn process_if_colon_body<'b>(
    if_stmt: &'b If<'b>,
    body: &'b IfColonDelimitedBody<'b>,
    _enclosing_stmt: &'b Statement<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Simplified handling for colon-delimited if.
    // Check if cursor is inside the then-body.
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

    let then_start = body.colon.start.offset;
    let cursor_in_then = ctx.cursor_offset >= then_start && ctx.cursor_offset < then_end;

    if cursor_in_then {
        apply_condition_narrowing(if_stmt.condition, scope, ctx);
        walk_body_forward(body.statements.iter(), scope, ctx);
        return;
    }

    // Check elseif clauses.
    for ei in body.else_if_clauses.iter() {
        let ei_start = ei.colon.start.offset;
        let ei_end = ei
            .statements
            .last()
            .map(|s| s.span().end.offset)
            .unwrap_or(ei_start);
        if ctx.cursor_offset >= ei_start && ctx.cursor_offset <= ei_end {
            apply_condition_narrowing_inverse(if_stmt.condition, scope, ctx);
            apply_condition_narrowing(ei.condition, scope, ctx);
            process_condition_assignment(ei.condition, scope, ctx);
            seed_pass_by_ref_in_condition(ei.condition, scope, ctx);
            walk_body_forward(ei.statements.iter(), scope, ctx);
            return;
        }
    }

    // Check else clause.
    if let Some(ref else_clause) = body.else_clause {
        let ec_start = else_clause.colon.start.offset;
        let ec_end = else_clause
            .statements
            .last()
            .map(|s| s.span().end.offset)
            .unwrap_or(ec_start);
        if ctx.cursor_offset >= ec_start && ctx.cursor_offset <= ec_end {
            apply_condition_narrowing_inverse(if_stmt.condition, scope, ctx);
            for ei in body.else_if_clauses.iter() {
                apply_condition_narrowing_inverse(ei.condition, scope, ctx);
            }
            walk_body_forward(else_clause.statements.iter(), scope, ctx);
            return;
        }
    }

    // Cursor is after the if — merge branches.
    let pre_if_scope = scope.clone();
    let mut then_scope = scope.clone();
    apply_condition_narrowing(if_stmt.condition, &mut then_scope, ctx);
    walk_body_forward(body.statements.iter(), &mut then_scope, ctx);

    let mut all_scopes = vec![then_scope];
    for ei in body.else_if_clauses.iter() {
        let mut ei_scope = pre_if_scope.clone();
        apply_condition_narrowing(ei.condition, &mut ei_scope, ctx);
        process_condition_assignment(ei.condition, &mut ei_scope, ctx);
        seed_pass_by_ref_in_condition(ei.condition, &mut ei_scope, ctx);
        walk_body_forward(ei.statements.iter(), &mut ei_scope, ctx);
        all_scopes.push(ei_scope);
    }
    if let Some(ref else_clause) = body.else_clause {
        let mut else_scope = pre_if_scope.clone();
        apply_condition_narrowing_inverse(if_stmt.condition, &mut else_scope, ctx);
        walk_body_forward(else_clause.statements.iter(), &mut else_scope, ctx);
        all_scopes.push(else_scope);
    } else {
        all_scopes.push(pre_if_scope);
    }

    // Merge all surviving scopes.
    if let Some(first) = all_scopes.first() {
        let mut merged = first.clone();
        for s in &all_scopes[1..] {
            merged.merge_branch(s);
        }
        *scope = merged;
    }
}

/// Process a `foreach` statement.
fn process_foreach<'b>(foreach: &'b Foreach<'b>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    // Resolve the iterable expression's type.
    let iter_type = resolve_foreach_iterable_type(foreach, scope, ctx);

    let pre_loop_scope = scope.clone();

    // When the cursor is inside the loop body (completion path), pass 1
    // must walk the ENTIRE body to discover assignments after the cursor.
    // Create a context with cursor_offset = u32::MAX for pass 1; pass 2
    // uses the real cursor_offset so it stops at the cursor as usual.
    let body_span = match &foreach.body {
        ForeachBody::Statement(inner) => inner.span(),
        ForeachBody::ColonDelimited(body) => body.span(),
    };
    let cursor_in_body =
        ctx.cursor_offset >= body_span.start.offset && ctx.cursor_offset <= body_span.end.offset;
    let pass1_ctx = if cursor_in_body && !is_diagnostic_scope_active() {
        ctx.with_cursor_offset(u32::MAX)
    } else {
        ctx.with_cursor_offset(ctx.cursor_offset)
    };

    // Bind the value variable (and optionally the key variable).
    match &foreach.target {
        ForeachTarget::Value(val) => {
            bind_foreach_value(val.value, &iter_type, scope, ctx);
        }
        ForeachTarget::KeyValue(kv) => {
            bind_foreach_key(kv.key, &iter_type, scope, ctx);
            bind_foreach_value(kv.value, &iter_type, scope, ctx);
        }
    }

    // Docblock fallback: when `bind_foreach_value` could not determine
    // the element type from the iterable (e.g. the iterable is `mixed`
    // or unresolvable), check for an inline `/** @var Type $var */`
    // docblock preceding the foreach keyword and use it to seed the
    // value variable.  This handles the common PHP pattern:
    //   /** @var Mandate $mandate */
    //   foreach ($mandates as $mandate) { ... }
    let value_var_name = match &foreach.target {
        ForeachTarget::Value(val) => {
            if let Expression::Variable(Variable::Direct(dv)) = val.value {
                Some(dv.name.to_string())
            } else {
                None
            }
        }
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.value {
                Some(dv.name.to_string())
            } else {
                None
            }
        }
    };
    if let Some(ref vn) = value_var_name
        && scope.get(vn).is_empty()
    {
        let foreach_offset = foreach.foreach.span().start.offset as usize;
        let before = &ctx.content[..foreach_offset.min(ctx.content.len())];
        let trimmed = before.trim_end();
        if trimmed.ends_with("*/")
            && let Some(doc_start) = trimmed.rfind("/**")
        {
            let doc_text = &trimmed[doc_start..trimmed.len()];
            // Only use the named-variable form `/** @var Type $var */`
            // and only when the variable name matches the foreach
            // value variable.  The no-var form `/** @var Type */`
            // must NOT be used here — it would incorrectly override
            // the iterable variable or other unrelated variables.
            if let Some((doc_var, php_type)) = parse_inline_var_docblock(doc_text, ctx)
                && doc_var == *vn
            {
                let resolved = resolve_type_to_resolved_types(&php_type, ctx);
                scope.set(vn.clone(), resolved);
            }
        }
    }

    // Two-pass loop body walk.
    //
    // The forward walker only processes the loop body once, so a
    // variable assigned late in the body (e.g. `$lastPaidEnd =
    // $periodEnd` at the end) is invisible to earlier statements
    // that guard on it (e.g. `if ($lastPaidEnd !== null &&
    // $lastPaidEnd->diffInDays(...))`).
    //
    // Fix: walk the body once to collect all assignments, merge
    // the resulting scope back into the pre-loop state, re-bind
    // the foreach variables, and walk the body a second time.
    // The second pass sees the merged types from the first
    // iteration, matching PHP's runtime behaviour on iteration ≥ 2.
    //
    // Only the second pass records diagnostic scope snapshots
    // (the first pass runs with a temporary scope that is
    // discarded after merging).

    // ── Pass 1: discover types assigned in the loop body ────
    // Use a temporary scope so the first pass does not record
    // diagnostic snapshots that would be overwritten by pass 2.
    let pass1_active = is_diagnostic_scope_active();
    if pass1_active {
        // Temporarily deactivate snapshot recording for pass 1.
        // We do this by NOT calling walk_body_for_diagnostics;
        // instead we use walk_body_forward which only records
        // snapshots when the cache is active AND record_snapshots
        // is true.  Since walk_body_forward checks
        // is_diagnostic_scope_active() itself, we rely on the
        // fact that walk_body_forward records snapshots — but
        // those will be overwritten by pass 2 at the same offsets.
    }
    match &foreach.body {
        ForeachBody::Statement(inner) => {
            walk_body_forward(std::iter::once(*inner), scope, &pass1_ctx);
        }
        ForeachBody::ColonDelimited(body) => {
            walk_body_forward(body.statements.iter(), scope, &pass1_ctx);
        }
    }

    // ── Merge pass-1 types into the pre-loop scope ──────────
    // Variables assigned in pass 1 get their types merged with
    // the pre-loop scope, so pass 2 sees them at every point.
    let mut pass2_scope = pre_loop_scope.clone();
    pass2_scope.merge_branch(scope);

    // Re-bind foreach target variables on the merged scope so
    // the foreach value/key types are present for pass 2.
    match &foreach.target {
        ForeachTarget::Value(val) => {
            bind_foreach_value(val.value, &iter_type, &mut pass2_scope, ctx);
        }
        ForeachTarget::KeyValue(kv) => {
            bind_foreach_key(kv.key, &iter_type, &mut pass2_scope, ctx);
            bind_foreach_value(kv.value, &iter_type, &mut pass2_scope, ctx);
        }
    }

    // ── Pass 2: re-walk with the merged types ───────────────
    match &foreach.body {
        ForeachBody::Statement(inner) => {
            walk_body_forward(std::iter::once(*inner), &mut pass2_scope, ctx);
        }
        ForeachBody::ColonDelimited(body) => {
            walk_body_forward(body.statements.iter(), &mut pass2_scope, ctx);
        }
    }

    // Use the pass-2 scope as the final scope, merged with the
    // pre-loop scope (the iterable might be empty, so the loop
    // body might not execute at all).
    *scope = pre_loop_scope;
    scope.merge_branch(&pass2_scope);
}

/// Resolve the iterable expression's type for a foreach.
fn resolve_foreach_iterable_type<'b>(
    foreach: &'b Foreach<'b>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<PhpType> {
    // Try direct scope lookup for bare variable iterators.
    if let Expression::Variable(Variable::Direct(dv)) = foreach.expression {
        let var_name = dv.name.to_string();
        let from_scope = scope.get(&var_name);
        if !from_scope.is_empty() {
            return Some(ResolvedType::types_joined(from_scope));
        }
    }

    // Fall back to resolve_rhs_expression for complex expressions.
    let resolved = resolve_rhs_with_scope(foreach.expression, scope, ctx);
    if !resolved.is_empty() {
        let joined = ResolvedType::types_joined(&resolved);
        // Expand type aliases (e.g. `@phpstan-type UserList array<int, User>`)
        // so that `extract_value_type` can see the underlying generic type.
        let expanded = crate::completion::type_resolution::resolve_type_alias_typed(
            &joined,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        )
        .unwrap_or(joined);
        return Some(expanded);
    }

    // Fallback: for simple `$variable` iterators, check for an inline
    // `/** @var Type $var */` or `@param` annotation near the foreach.
    // This mirrors the backward scanner's `find_iterable_raw_type_in_source`
    // fallback and handles cases where the variable's type comes from a
    // docblock rather than an assignment.
    if let Expression::Variable(Variable::Direct(dv)) = foreach.expression {
        let var_name = dv.name.to_string();
        let foreach_offset = foreach.foreach.span().start.offset as usize;
        if let Some(docblock_type) = crate::docblock::find_iterable_raw_type_in_source(
            ctx.content,
            foreach_offset,
            &var_name,
        ) {
            // Expand type aliases on the docblock result too.
            let expanded = crate::completion::type_resolution::resolve_type_alias_typed(
                &docblock_type,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            )
            .unwrap_or(docblock_type);
            return Some(expanded);
        }
    }

    // Final fallback: resolve the foreach expression as a "subject"
    // through the full resolver pipeline (SubjectExpr::parse →
    // property/method chain resolution).  This mirrors the backward
    // scanner's `resolve_foreach_expression_to_classes` and handles
    // cases like `$this->getItems()` or `self::fetchAll()` where
    // the expression type wasn't captured by scope lookup or
    // resolve_rhs_expression above.
    if let Some(iter_type) = resolve_foreach_expr_via_subject(foreach.expression, scope, ctx) {
        return Some(iter_type);
    }

    None
}

/// Resolve a foreach expression to a `PhpType` by treating it as a
/// subject string and going through the full resolver pipeline.
///
/// This is the forward walker's equivalent of the backward scanner's
/// `resolve_foreach_expression_to_classes`.  It extracts the expression
/// text, calls `resolve_target_classes` to get `ClassInfo` objects, and
/// constructs a `PhpType::Named` from the first resolved class.
fn resolve_foreach_expr_via_subject<'b>(
    expression: &'b Expression<'b>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<PhpType> {
    let expr_span = expression.span();
    let expr_start = expr_span.start.offset as usize;
    let expr_end = expr_span.end.offset as usize;
    let expr_text = ctx.content.get(expr_start..expr_end)?.trim();
    if expr_text.is_empty() {
        return None;
    }

    // Build a ResolutionCtx from the forward walker's context.
    let scope_snapshot = scope.locals.clone();
    let scope_resolver = move |var_name: &str| -> Vec<ResolvedType> {
        scope_snapshot.get(var_name).cloned().unwrap_or_default()
    };
    let var_ctx = ctx.var_ctx_for_with_scope("$__foreach", expr_span.start.offset, &scope_resolver);
    let rctx = var_ctx.as_resolution_ctx();

    let resolved = crate::completion::resolver::resolve_target_classes(
        expr_text,
        crate::types::AccessKind::Arrow,
        &rctx,
    );

    if resolved.is_empty() {
        return None;
    }

    // Construct a PhpType from the resolved classes.  If any resolved
    // type has a structured type_string (e.g. `list<User>`,
    // `Collection<int, Product>`), prefer that — it carries generic
    // parameters that `extract_value_type` can use.
    for rt in &resolved {
        if rt.type_string.has_type_structure() {
            let expanded = crate::completion::type_resolution::resolve_type_alias_typed(
                &rt.type_string,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            )
            .unwrap_or_else(|| rt.type_string.clone());
            return Some(expanded);
        }
    }

    // Fall back to the class name — `bind_foreach_value` Strategy 2
    // will resolve it through inheritance to find element types.
    // Use `fqn()` (not `name`) so that the returned `PhpType::Named`
    // carries the fully-qualified class name.  `ClassInfo.name` is
    // always the short name (e.g. `OrderProductCollection`), while
    // `fqn()` combines namespace + name into the FQN that the class
    // loader needs to find and merge the class.
    let first = resolved.first()?;
    let name = first
        .class_info
        .as_ref()
        .map(|c| c.fqn())
        .or_else(|| first.type_string.base_name().map(|s| s.to_string()))?;

    Some(PhpType::Named(name))
}

/// Bind a foreach value variable from the iterable's element type.
///
/// Resolution strategy:
/// 1. Try `PhpType::extract_value_type` — works for types that already
///    carry generic parameters (e.g. `list<User>`, `array<int, Order>`,
///    `Collection<int, Product>`).
/// 2. Class-based fallback — when the type is a bare class name (e.g.
///    `OrderProductCollection`), resolve it to `ClassInfo`, merge
///    inheritance, and extract the element type from `@extends` /
///    `@implements` generics.  This mirrors what
///    `try_resolve_foreach_value_type` does in the backward scanner.
fn bind_foreach_value<'b>(
    value_expr: &'b Expression<'b>,
    iter_type: &Option<PhpType>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    if let Expression::Variable(Variable::Direct(dv)) = value_expr {
        let var_name = dv.name.to_string();
        if let Some(it) = iter_type {
            // Strategy 1: extract from the type's own generic parameters.
            let value_php_type = it.extract_value_type(false);
            if let Some(vt) = value_php_type {
                let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                    vt,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
                if !resolved.is_empty() {
                    scope.set(
                        var_name,
                        ResolvedType::from_classes_with_hint(resolved, vt.clone()),
                    );
                } else {
                    scope.set(var_name, vec![ResolvedType::from_type_string(vt.clone())]);
                }
                return;
            }

            // Strategy 2: class-based fallback for bare collection names.
            // Resolve the iterable type to ClassInfo, merge inheritance,
            // and extract the element type from @extends/@implements generics.
            // Skip when the extracted element type is an unsubstituted
            // template parameter (e.g. `TValue` from `ArrayObject<TKey, TValue>`)
            // — binding it would produce incorrect scope entries that
            // override the backward scanner's correct resolution via
            // inline `/** @var */` docblocks.
            let element_via_class = resolve_iterable_element_via_class(it, ctx);
            if let Some(element_type) = element_via_class
                && !is_unsubstituted_template_param(&element_type)
            {
                let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                    &element_type,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
                if !resolved.is_empty() {
                    scope.set(
                        var_name.clone(),
                        ResolvedType::from_classes_with_hint(resolved, element_type),
                    );
                } else {
                    scope.set(
                        var_name.clone(),
                        vec![ResolvedType::from_type_string(element_type)],
                    );
                }
            }

            // Strategy 3: union type fallback — try each member individually.
            // When the iterable is a union like `ProductCollection|Product`,
            // neither `extract_value_type` nor `resolve_iterable_element_via_class`
            // works on the union as a whole.  Walk each member and use the
            // first one that yields an element type.
            if let PhpType::Union(members) = it {
                for member in members {
                    // Try extract_value_type on each member (handles generic collections).
                    if let Some(vt) = member.extract_value_type(false) {
                        let resolved =
                            crate::completion::type_resolution::type_hint_to_classes_typed(
                                vt,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                        if !resolved.is_empty() {
                            scope.set(
                                var_name,
                                ResolvedType::from_classes_with_hint(resolved, vt.clone()),
                            );
                        } else {
                            scope.set(var_name, vec![ResolvedType::from_type_string(vt.clone())]);
                        }
                        return;
                    }
                    // Try class-based element extraction on each member.
                    if let Some(element_type) = resolve_iterable_element_via_class(member, ctx)
                        && !is_unsubstituted_template_param(&element_type)
                    {
                        let resolved =
                            crate::completion::type_resolution::type_hint_to_classes_typed(
                                &element_type,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                        if !resolved.is_empty() {
                            scope.set(
                                var_name.clone(),
                                ResolvedType::from_classes_with_hint(resolved, element_type),
                            );
                        } else {
                            scope.set(
                                var_name.clone(),
                                vec![ResolvedType::from_type_string(element_type)],
                            );
                        }
                        return;
                    }
                }
            }
        }
        // Couldn't determine the element type.  Store empty in the
        // scope so that `lookup_diagnostic_scope` returns
        // `Some(vec![])` instead of `None`, preventing a pointless
        // fallthrough to the backward scanner.
        if !scope.contains(&var_name) {
            scope.set_empty(var_name);
        }
    } else if let Expression::Array(_) | Expression::List(_) = value_expr {
        // Array/list destructuring in foreach: `foreach ($items as [$a, $b])`
        // For now, skip — this is a complex pattern.
    }
}

/// Check whether a `PhpType` looks like an unsubstituted template
/// parameter (e.g. `TValue`, `TKey`, `TModel`).  These are bare named
/// types whose name starts with `T` followed by an uppercase letter
/// and are not known PHP built-in types.
fn is_unsubstituted_template_param(ty: &PhpType) -> bool {
    let name = match ty {
        PhpType::Named(n) => n.as_str(),
        _ => return false,
    };
    let bytes = name.as_bytes();
    bytes.len() >= 2 && bytes[0] == b'T' && bytes[1].is_ascii_uppercase()
}

/// Resolve the element type of an iterable via class inheritance.
///
/// When the iterable type is a bare class name (e.g. `OrderProductCollection`),
/// this resolves it to `ClassInfo`, merges the full inheritance chain, and
/// extracts the element type from `@extends` / `@implements` generics using
/// [`extract_iterable_element_type_from_class`].
fn resolve_iterable_element_via_class(
    iter_type: &PhpType,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<PhpType> {
    // Only attempt for Named types (bare class names without generics).
    // Generic types like `Collection<int, User>` are handled by
    // extract_value_type above.
    let class_name = match iter_type {
        PhpType::Named(name) => name.as_str(),
        _ => return None,
    };

    // Resolve the class name to ClassInfo.
    let classes = crate::completion::type_resolution::type_hint_to_classes_typed(
        iter_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );

    if classes.is_empty() {
        // Try direct class loader as fallback (handles FQN names).
        let cls = (ctx.class_loader)(class_name)?;
        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
            &cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        return super::foreach_resolution::extract_iterable_element_type_from_class(
            &merged,
            ctx.class_loader,
        );
    }

    for cls in &classes {
        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
            cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        let element_type = super::foreach_resolution::extract_iterable_element_type_from_class(
            &merged,
            ctx.class_loader,
        );
        if element_type.is_some() {
            return element_type;
        }
    }

    None
}

/// Bind a foreach key variable.
fn bind_foreach_key<'b>(
    key_expr: &'b Expression<'b>,
    iter_type: &Option<PhpType>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    if let Expression::Variable(Variable::Direct(dv)) = key_expr {
        let var_name = dv.name.to_string();
        if let Some(it) = iter_type {
            let key_php_type = it.extract_key_type(false);
            if let Some(kt) = key_php_type {
                let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                    kt,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
                if !resolved.is_empty() {
                    scope.set(
                        var_name,
                        ResolvedType::from_classes_with_hint(resolved, kt.clone()),
                    );
                } else {
                    scope.set(var_name, vec![ResolvedType::from_type_string(kt.clone())]);
                }
                return;
            }
        }
        // Default: key is int|string.
        scope.set(
            var_name,
            vec![ResolvedType::from_type_string(PhpType::Union(vec![
                PhpType::int(),
                PhpType::string(),
            ]))],
        );
    }
}

/// Process a `while` loop.
///
/// Uses the same two-pass strategy as `process_foreach` and
/// `process_for`: the first pass discovers all variable assignments
/// inside the loop body, the results are merged back into the
/// pre-loop scope, and the second pass re-walks with full visibility
/// of loop-carried assignments.
fn process_while<'b>(while_stmt: &'b While<'b>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    // Record `&&` chain snapshots for the while condition.
    record_and_chain_snapshots(while_stmt.condition, scope, ctx);

    let pre_loop_scope = scope.clone();

    // The while body executes when the condition is truthy, so apply
    // condition narrowing (instanceof, phpstan-assert-if-true, etc.).
    // This must happen AFTER saving pre_loop_scope so the narrowing
    // only affects the loop body, not the post-loop scope.
    apply_condition_narrowing(while_stmt.condition, scope, ctx);

    // When the cursor is inside the loop body (completion path), pass 1
    // must walk the ENTIRE body to discover assignments after the cursor.
    let body_span = match &while_stmt.body {
        WhileBody::Statement(inner) => inner.span(),
        WhileBody::ColonDelimited(body) => body.span(),
    };
    let cursor_in_body =
        ctx.cursor_offset >= body_span.start.offset && ctx.cursor_offset <= body_span.end.offset;
    let pass1_ctx = if cursor_in_body && !is_diagnostic_scope_active() {
        ctx.with_cursor_offset(u32::MAX)
    } else {
        ctx.with_cursor_offset(ctx.cursor_offset)
    };

    // Assignment in condition: `while ($x = expr())`
    process_condition_assignment(while_stmt.condition, scope, ctx);

    // Pass-by-reference in condition: `while (preg_match(..., $matches))`
    seed_pass_by_ref_in_condition(while_stmt.condition, scope, ctx);

    // Record a snapshot after condition processing (same reasoning as
    // the corresponding snapshot in `process_if`).
    if is_diagnostic_scope_active() {
        let body_start = match &while_stmt.body {
            WhileBody::Statement(inner) => inner.span().start.offset,
            WhileBody::ColonDelimited(body) => body.colon.start.offset,
        };
        record_scope_snapshot(body_start, scope);
    }

    // ── Pass 1: discover types assigned in the loop body ────
    match &while_stmt.body {
        WhileBody::Statement(inner) => {
            walk_body_forward(std::iter::once(*inner), scope, &pass1_ctx);
        }
        WhileBody::ColonDelimited(body) => {
            walk_body_forward(body.statements.iter(), scope, &pass1_ctx);
        }
    }

    // ── Merge pass-1 types into the pre-loop scope ──────────
    let mut pass2_scope = pre_loop_scope.clone();
    pass2_scope.merge_branch(scope);

    // Re-apply condition narrowing, assignments, and pass-by-ref on the
    // merged scope so they remain visible in pass 2.
    apply_condition_narrowing(while_stmt.condition, &mut pass2_scope, ctx);
    process_condition_assignment(while_stmt.condition, &mut pass2_scope, ctx);
    seed_pass_by_ref_in_condition(while_stmt.condition, &mut pass2_scope, ctx);

    // ── Pass 2: re-walk with the merged types ───────────────
    match &while_stmt.body {
        WhileBody::Statement(inner) => {
            walk_body_forward(std::iter::once(*inner), &mut pass2_scope, ctx);
        }
        WhileBody::ColonDelimited(body) => {
            walk_body_forward(body.statements.iter(), &mut pass2_scope, ctx);
        }
    }

    // When the cursor is inside the loop body (completion path), keep
    // the pass-2 scope with condition narrowing applied.  The post-loop
    // merge would erase the narrowing (since the loop might not execute),
    // but the cursor IS inside the body, so the condition is true.
    if cursor_in_body && !is_diagnostic_scope_active() {
        *scope = pass2_scope;
        return;
    }

    // The loop body might not execute at all (condition false on
    // first check), so merge with the pre-loop scope.
    *scope = pre_loop_scope;
    scope.merge_branch(&pass2_scope);
}

/// Process a `for` loop.
///
/// Uses the same two-pass strategy as `process_foreach`: the first
/// pass discovers all variable assignments inside the loop body, the
/// results are merged back into the pre-loop scope, and the second
/// pass re-walks with full visibility of loop-carried assignments.
/// This handles the common pattern where a variable is assigned late
/// in the body but referenced (or guarded on) earlier.
fn process_for<'b>(for_stmt: &'b For<'b>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    // Process initializer expressions (e.g. `$i = 0`).
    for init_expr in for_stmt.initializations.iter() {
        process_assignment_expr(init_expr, scope, ctx);
    }

    // Process condition assignments (e.g. `for (; $x = nextItem(); )`)
    // and pass-by-ref in conditions (e.g. `for (; preg_match(..., $m); )`).
    for cond_expr in for_stmt.conditions.iter() {
        process_condition_assignment(cond_expr, scope, ctx);
        seed_pass_by_ref_in_condition(cond_expr, scope, ctx);
    }

    let pre_loop_scope = scope.clone();

    // When the cursor is inside the loop body (completion path), pass 1
    // must walk the ENTIRE body to discover assignments after the cursor.
    let body_span = match &for_stmt.body {
        ForBody::Statement(inner) => inner.span(),
        ForBody::ColonDelimited(body) => body.span(),
    };
    let cursor_in_body =
        ctx.cursor_offset >= body_span.start.offset && ctx.cursor_offset <= body_span.end.offset;
    let pass1_ctx = if cursor_in_body && !is_diagnostic_scope_active() {
        ctx.with_cursor_offset(u32::MAX)
    } else {
        ctx.with_cursor_offset(ctx.cursor_offset)
    };

    // ── Pass 1: discover types assigned in the loop body ────
    match &for_stmt.body {
        ForBody::Statement(inner) => {
            walk_body_forward(std::iter::once(*inner), scope, &pass1_ctx);
        }
        ForBody::ColonDelimited(body) => {
            walk_body_forward(body.statements.iter(), scope, &pass1_ctx);
        }
    }

    // ── Merge pass-1 types into the pre-loop scope ──────────
    let mut pass2_scope = pre_loop_scope.clone();
    pass2_scope.merge_branch(scope);

    // Re-apply initializers on the merged scope so they remain
    // visible in pass 2.
    for init_expr in for_stmt.initializations.iter() {
        process_assignment_expr(init_expr, &mut pass2_scope, ctx);
    }

    // ── Pass 2: re-walk with the merged types ───────────────
    match &for_stmt.body {
        ForBody::Statement(inner) => {
            walk_body_forward(std::iter::once(*inner), &mut pass2_scope, ctx);
        }
        ForBody::ColonDelimited(body) => {
            walk_body_forward(body.statements.iter(), &mut pass2_scope, ctx);
        }
    }

    // The loop body might not execute at all (condition false on
    // first check), so merge with the pre-loop scope.
    *scope = pre_loop_scope;
    scope.merge_branch(&pass2_scope);
}

/// Process a `do-while` loop.
///
/// Uses the same two-pass strategy as `process_foreach` and
/// `process_for`: the first pass discovers all variable assignments
/// inside the body, the results are merged back, and the second pass
/// re-walks with full visibility of loop-carried assignments.
///
/// Unlike `for`/`while`, the body of a `do-while` always executes at
/// least once, so we do NOT merge with a pre-loop scope — the final
/// scope is the pass-2 result directly.
fn process_do_while<'b>(dw: &'b DoWhile<'b>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    let pre_loop_scope = scope.clone();

    // ── Pass 1: discover types assigned in the body ─────────
    walk_body_forward(std::iter::once(dw.statement), scope, ctx);

    // ── Merge pass-1 types into the pre-loop scope ──────────
    let mut pass2_scope = pre_loop_scope;
    pass2_scope.merge_branch(scope);

    // Process the do-while condition between passes.  The condition
    // is evaluated after the body, so assignments and pass-by-ref
    // seeding from the condition (e.g. `do { … } while (($x =
    // next()) !== null)`) should be visible on iteration ≥ 2.
    process_condition_assignment(dw.condition, &mut pass2_scope, ctx);
    seed_pass_by_ref_in_condition(dw.condition, &mut pass2_scope, ctx);

    // ── Pass 2: re-walk with the merged types ───────────────
    walk_body_forward(std::iter::once(dw.statement), &mut pass2_scope, ctx);

    *scope = pass2_scope;
}

/// Process a `try-catch-finally` statement.
fn process_try<'b>(try_stmt: &'b Try<'b>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    let pre_try_scope = scope.clone();

    // Check if cursor is inside the try body.
    let try_body_span = try_stmt.block.span();
    let cursor_in_try = ctx.cursor_offset >= try_body_span.start.offset
        && ctx.cursor_offset <= try_body_span.end.offset;

    if cursor_in_try {
        // Walk only the try body.
        walk_body_forward(try_stmt.block.statements.iter(), scope, ctx);
        return;
    }

    // Check if cursor is inside a catch block.
    for catch in try_stmt.catch_clauses.iter() {
        let catch_span = catch.block.span();
        if ctx.cursor_offset >= catch_span.start.offset
            && ctx.cursor_offset <= catch_span.end.offset
        {
            // Bind the caught exception variable.
            if let Some(ref var) = catch.variable {
                let var_name = var.name.to_string();
                let parsed_hint = extract_hint_type(&catch.hint);
                let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                    &parsed_hint,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
                let exception_types = ResolvedType::from_classes_with_hint(resolved, parsed_hint);
                // Merge pre-try scope (since the exception could have
                // been thrown at any point in the try body) with the
                // catch variable.
                *scope = pre_try_scope.clone();
                if !exception_types.is_empty() {
                    scope.set(var_name, exception_types);
                }
            } else {
                *scope = pre_try_scope.clone();
            }
            walk_body_forward(catch.block.statements.iter(), scope, ctx);
            return;
        }
    }

    // Check if cursor is inside the finally block.
    if let Some(ref finally) = try_stmt.finally_clause {
        let finally_span = finally.block.span();
        if ctx.cursor_offset >= finally_span.start.offset
            && ctx.cursor_offset <= finally_span.end.offset
        {
            // In finally, merge all possible paths.
            walk_body_forward(try_stmt.block.statements.iter(), scope, ctx);
            walk_body_forward(finally.block.statements.iter(), scope, ctx);
            return;
        }
    }

    // Cursor is after the try/catch/finally.  Walk the try body and
    // merge all catch scopes.
    walk_body_forward(try_stmt.block.statements.iter(), scope, ctx);
    let try_scope = scope.clone();

    let mut all_scopes = vec![try_scope];
    for catch in try_stmt.catch_clauses.iter() {
        let mut catch_scope = pre_try_scope.clone();
        if let Some(ref var) = catch.variable {
            let var_name = var.name.to_string();
            let parsed_hint = extract_hint_type(&catch.hint);
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                &parsed_hint,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            let exception_types = ResolvedType::from_classes_with_hint(resolved, parsed_hint);
            if !exception_types.is_empty() {
                catch_scope.set(var_name, exception_types);
            }
        }
        walk_body_forward(catch.block.statements.iter(), &mut catch_scope, ctx);
        all_scopes.push(catch_scope);
    }

    // Merge all scopes.
    let mut merged = all_scopes[0].clone();
    for s in &all_scopes[1..] {
        merged.merge_branch(s);
    }
    *scope = merged;

    // Walk the finally block if present.
    if let Some(ref finally) = try_stmt.finally_clause {
        walk_body_forward(finally.block.statements.iter(), scope, ctx);
    }
}

/// Process a `switch` statement.
///
/// Each case arm is walked on a clone of the pre-switch scope so that
/// assignments in one arm don't leak into another.  After all arms are
/// walked, the resulting scopes are merged (union of types), matching
/// the runtime behaviour where only one arm executes.
///
/// Fall-through cases (cases with no statements) share their scope
/// with the next non-empty case, mirroring PHP semantics.
fn process_switch<'b>(switch: &'b Switch<'b>, scope: &mut ScopeState, ctx: &ForwardWalkCtx<'_>) {
    let pre_switch_scope = scope.clone();
    let cases: Vec<_> = switch.body.cases().iter().collect();

    if cases.is_empty() {
        return;
    }

    let mut branch_scopes: Vec<ScopeState> = Vec::new();
    let mut has_default = false;

    // Walk cases, accumulating fall-through groups.
    let mut accumulated_stmts: Vec<&Statement<'b>> = Vec::new();
    for case in &cases {
        if case.is_default() {
            has_default = true;
        }

        let stmts: Vec<_> = case.statements().iter().collect();
        if stmts.is_empty() {
            // Fall-through: no statements, will share scope with next case.
            continue;
        }

        accumulated_stmts.extend(stmts);

        let mut case_scope = pre_switch_scope.clone();
        walk_body_forward(accumulated_stmts.iter().copied(), &mut case_scope, ctx);
        branch_scopes.push(case_scope);
        accumulated_stmts.clear();
    }

    // Handle trailing fall-through cases (empty cases at the end).
    if !accumulated_stmts.is_empty() {
        let mut case_scope = pre_switch_scope.clone();
        walk_body_forward(accumulated_stmts.iter().copied(), &mut case_scope, ctx);
        branch_scopes.push(case_scope);
    }

    if branch_scopes.is_empty() {
        return;
    }

    // Merge all branch scopes.
    let mut merged = branch_scopes[0].clone();
    for s in &branch_scopes[1..] {
        merged.merge_branch(s);
    }

    // If there is no default case, the switch might not execute any
    // arm at all, so merge with the pre-switch scope.
    if !has_default {
        merged.merge_branch(&pre_switch_scope);
    }

    *scope = merged;
}

// ─── Narrowing helpers ──────────────────────────────────────────────────────

/// Apply condition-based narrowing (instanceof, null check, type guard)
/// to the scope.  This narrows types for the "truthy" branch.
fn apply_condition_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Decompose `&&` chains so that `$x instanceof Foo && $x instanceof Bar`
    // applies both narrowings as a union (intersection semantics: the
    // variable satisfies both checks, so members from both types are
    // available).
    let operands = collect_and_chain_operands(condition);

    // First pass: collect all instanceof extractions per variable across
    // all `&&` operands.  This prevents later operands from overwriting
    // earlier ones when both narrow the same variable.
    let scope_snapshot = scope.locals.clone();
    let scope_resolver =
        |vn: &str| -> Vec<ResolvedType> { scope_snapshot.get(vn).cloned().unwrap_or_default() };
    let mut var_names: Vec<String> = scope.locals.keys().cloned().collect();
    // Include variables from instanceof conditions that may not be in
    // scope yet (e.g. undeclared variables used in instanceof checks).
    for name in collect_condition_var_names(condition) {
        if !var_names.contains(&name) {
            var_names.push(name);
        }
    }

    // Track which variables have been narrowed by instanceof across
    // `&&` operands so we can merge them into a union.
    let mut instanceof_results: HashMap<String, Vec<ResolvedType>> = HashMap::new();

    for operand in &operands {
        for var_name in &var_names {
            // Compound OR instanceof: `$x instanceof A || $x instanceof B`
            if let Some(classes) = narrowing::try_extract_compound_or_instanceof(operand, var_name)
                && !classes.is_empty()
            {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                let union = narrowing::resolve_class_names_to_union(&classes, &var_ctx);
                if !union.is_empty() {
                    let entry = instanceof_results.entry(var_name.clone()).or_default();
                    ResolvedType::extend_unique(
                        entry,
                        union.into_iter().map(ResolvedType::from_class).collect(),
                    );
                }
                continue;
            }

            // Single instanceof (including negated, is_a, get_class).
            if let Some(extraction) =
                narrowing::try_extract_instanceof_with_negation(operand, var_name)
            {
                let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
                if extraction.negated {
                    // Negated instanceof: apply exclusion to the current
                    // scope immediately (each negation removes one type).
                    let mut results = scope.get(var_name).to_vec();
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_exclusion(
                            &extraction.class_type,
                            &var_ctx,
                            classes,
                        );
                    });
                    if !results.is_empty() {
                        scope.set(var_name.clone(), results);
                    }
                } else {
                    // Positive instanceof: resolve and accumulate into
                    // the per-variable union.  For a single operand this
                    // produces `[Foo]`; for `&& instanceof Bar` it
                    // accumulates `[Foo, Bar]`.
                    let mut single = Vec::new();
                    ResolvedType::apply_narrowing(&mut single, |classes| {
                        narrowing::apply_instanceof_inclusion(
                            &extraction.class_type,
                            extraction.exact,
                            &var_ctx,
                            classes,
                        );
                    });
                    if !single.is_empty() {
                        let entry = instanceof_results.entry(var_name.clone()).or_default();
                        ResolvedType::extend_unique(entry, single);
                    } else {
                        // Target class is unresolvable — mark variable
                        // as empty so diagnostics suppress false positives.
                        instanceof_results.entry(var_name.clone()).or_default();
                    }
                }
            }
        }
    }

    // Apply the accumulated instanceof narrowing results to the scope.
    for (var_name, narrowed) in instanceof_results {
        if !narrowed.is_empty() {
            let existing = scope.get(&var_name);
            if existing.is_empty() {
                // Untyped variable — instanceof provides the type.
                scope.set(var_name, narrowed);
            } else {
                // Typed variable — filter the existing union to only
                // types present in the narrowed set.  This correctly
                // handles both single instanceof (`Dog|Cat` → `Dog`)
                // and OR instanceof (`Dog|Cat|Other` → `Dog|Cat`).
                //
                // When the narrowed type is NOT in the existing union
                // (e.g. `MockInterface` narrowed to `MolliePayment`),
                // this is an intersection case — apply via
                // apply_instanceof_inclusion which has interface
                // intersection logic.
                let narrowed_fqns: Vec<String> = narrowed
                    .iter()
                    .filter_map(|rt| rt.class_info.as_ref().map(|c| c.fqn()))
                    .collect();

                // Try filtering: keep existing entries whose class is
                // in the narrowed set.
                let filtered: Vec<ResolvedType> = existing
                    .iter()
                    .filter(|rt| {
                        rt.class_info
                            .as_ref()
                            .is_some_and(|c| narrowed_fqns.contains(&c.fqn()))
                    })
                    .cloned()
                    .collect();

                if !filtered.is_empty() {
                    // Filter matched — use the filtered results
                    // (preserves richer type info from original resolution).
                    scope.set(var_name, filtered);
                } else {
                    // No overlap between existing and narrowed types.
                    // This is the intersection case (e.g. MockInterface
                    // narrowed to MolliePayment).  Use
                    // apply_instanceof_inclusion which produces the
                    // intersection when one side is an interface.
                    let mut results = existing.to_vec();
                    // Apply all narrowed classes as a single group by
                    // building a union type.
                    let union_type = if narrowed_fqns.len() == 1 {
                        PhpType::Named(narrowed_fqns[0].clone())
                    } else {
                        PhpType::Union(
                            narrowed_fqns
                                .iter()
                                .map(|n| PhpType::Named(n.clone()))
                                .collect(),
                        )
                    };
                    let var_ctx = build_var_ctx(&var_name, ctx, &scope_resolver);
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_inclusion(
                            &union_type,
                            false,
                            &var_ctx,
                            classes,
                        );
                    });
                    if !results.is_empty() {
                        scope.set(var_name, results);
                    } else {
                        // Fallback: use the narrowed types directly.
                        scope.set(var_name, narrowed);
                    }
                }
            }
        } else {
            // Empty narrowed list means the target was unresolvable.
            scope.locals.insert(var_name, vec![]);
        }
    }

    // Type guard narrowing: `is_object($x)`, `is_array($x)`, etc.
    apply_type_guard_narrowing_truthy(condition, scope);

    // Null narrowing: `if ($x !== null)` — remove null from scope.
    apply_null_narrowing_truthy(condition, scope, ctx);

    // @phpstan-assert-if-true / -if-false narrowing.
    apply_phpstan_assert_condition_narrowing(condition, scope, ctx, false);

    // in_array($var, $haystack, true) narrowing.
    apply_in_array_narrowing(condition, scope, ctx, false);
}

/// Apply inverse narrowing for a single condition expression (not
/// decomposed).  Called by [`apply_condition_narrowing_inverse`] for
/// each operand in a `&&` chain, or for the whole condition when it
/// is not a chain.
fn apply_condition_narrowing_inverse_single<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver =
        |vn: &str| -> Vec<ResolvedType> { scope_snapshot.get(vn).cloned().unwrap_or_default() };
    // Include variables from instanceof conditions that may not be in
    // scope yet (e.g. `if (!$foobar instanceof Foobar) { break; }`
    // where `$foobar` was never assigned).  After the guard clause,
    // `$foobar` must be `Foobar`.
    let mut var_names: Vec<String> = scope.locals.keys().cloned().collect();
    for name in collect_condition_var_names(condition) {
        if !var_names.contains(&name) {
            var_names.push(name);
        }
    }
    for var_name in &var_names {
        if let Some(classes) = narrowing::try_extract_compound_or_instanceof(condition, var_name)
            && !classes.is_empty()
        {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let mut results = scope.get(var_name).to_vec();
            for cls_type in &classes {
                ResolvedType::apply_narrowing(&mut results, |class_list| {
                    narrowing::apply_instanceof_exclusion(cls_type, &var_ctx, class_list);
                });
            }
            if !results.is_empty() {
                scope.set(var_name.clone(), results);
            }
            continue;
        }

        if let Some(extraction) =
            narrowing::try_extract_instanceof_with_negation(condition, var_name)
        {
            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let mut results = scope.get(var_name).to_vec();
            if extraction.negated {
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_inclusion(
                        &extraction.class_type,
                        extraction.exact,
                        &var_ctx,
                        classes,
                    );
                });
            } else {
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_exclusion(
                        &extraction.class_type,
                        &var_ctx,
                        classes,
                    );
                });
            }
            if !results.is_empty() {
                scope.set(var_name.clone(), results);
            }
        }
    }
}

/// Apply inverse condition-based narrowing (for else branches and
/// guard clauses).
fn apply_condition_narrowing_inverse<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Decompose `&&` chains so that each operand is processed
    // individually.  For guard clauses like
    // `if (!$x instanceof A && !$x instanceof B) { return; }`,
    // the inverse (code after the guard) means `$x IS A || $x IS B`.
    //
    // De Morgan: NOT (!A && !B) = A || B.  Each operand's inverse
    // produces one branch of the union.  We clone the scope for each
    // operand, apply the inverse, then merge (union) all results back
    // into the main scope.
    let operands = collect_and_chain_operands(condition);
    if operands.len() > 1 {
        let base_scope = scope.clone();
        let mut branch_scopes: Vec<ScopeState> = Vec::new();
        for operand in &operands {
            let mut branch = base_scope.clone();
            apply_condition_narrowing_inverse_single(operand, &mut branch, ctx);
            branch_scopes.push(branch);
        }
        // Merge all branch scopes (union of all narrowed types).
        if let Some(first) = branch_scopes.first() {
            let mut merged = first.clone();
            for branch in &branch_scopes[1..] {
                merged.merge_branch(branch);
            }
            *scope = merged;
        }
        // Type guard, null, phpstan-assert, and in_array narrowing
        // operate on the full condition expression.
        apply_type_guard_narrowing_inverse(condition, scope);
        apply_null_narrowing_inverse(condition, scope);
        apply_phpstan_assert_condition_narrowing(condition, scope, ctx, true);
        apply_in_array_narrowing(condition, scope, ctx, true);
        return;
    }

    apply_condition_narrowing_inverse_single(condition, scope, ctx);

    // Inverse type guard narrowing: `if (is_object($x))` in else → exclude object.
    apply_type_guard_narrowing_inverse(condition, scope);

    // Inverse null narrowing: `if ($x === null)` after guard → remove null.
    apply_null_narrowing_inverse(condition, scope);

    // Inverse @phpstan-assert-if-true / -if-false narrowing.
    apply_phpstan_assert_condition_narrowing(condition, scope, ctx, true);

    // Inverse in_array narrowing: exclude the element type in the else branch.
    apply_in_array_narrowing(condition, scope, ctx, true);
}

/// Apply `in_array($var, $haystack, true)` narrowing.
///
/// When `inverted` is false (truthy branch / while body), the variable is
/// narrowed to the haystack's element type (inclusion).  When `inverted` is
/// true (else branch / guard clause inverse), the variable is narrowed by
/// excluding the element type.
fn apply_in_array_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inverted: bool,
) {
    let scope_snapshot = scope.locals.clone();
    let scope_resolver =
        |vn: &str| -> Vec<ResolvedType> { scope_snapshot.get(vn).cloned().unwrap_or_default() };

    // Unwrap parentheses and detect negation.
    let (inner, negated) = narrowing::unwrap_condition_negation(condition);

    // Check every variable in scope as the potential needle.
    let var_names: Vec<String> = scope.locals.keys().cloned().collect();
    for var_name in &var_names {
        if let Some(haystack_expr) = narrowing::try_extract_in_array(inner, var_name) {
            // Resolve the haystack's type from the scope to extract the
            // element type.  This replaces the backward scanner's
            // `resolve_arg_raw_type` with a scope-based lookup.
            let element_type = resolve_in_array_element_type_fw(haystack_expr, scope, ctx);
            let element_type = match element_type {
                Some(et) => et,
                None => continue,
            };

            // Determine whether to include or exclude:
            // - truthy + positive  → include (var IS in haystack)
            // - truthy + negated   → exclude (var is NOT in haystack)
            // - inverse + positive → exclude
            // - inverse + negated  → include
            let should_exclude = inverted ^ negated;

            let var_ctx = build_var_ctx(var_name, ctx, &scope_resolver);
            let mut results = scope.get(var_name).to_vec();

            if should_exclude {
                // Skip exclusion when it would remove ALL type information.
                let would_remove_all = {
                    let mut test = results.clone();
                    ResolvedType::apply_narrowing(&mut test, |classes| {
                        narrowing::apply_instanceof_exclusion(&element_type, &var_ctx, classes);
                    });
                    test.is_empty()
                };
                if !would_remove_all {
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_exclusion(&element_type, &var_ctx, classes);
                    });
                }
            } else {
                ResolvedType::apply_narrowing(&mut results, |classes| {
                    narrowing::apply_instanceof_inclusion(&element_type, false, &var_ctx, classes);
                });
            }

            if !results.is_empty() {
                scope.set(var_name.clone(), results);
            }
        }
    }
}

/// Resolve the element type of a haystack expression for `in_array`
/// narrowing, using the forward walker's scope instead of the backward
/// scanner.
fn resolve_in_array_element_type_fw(
    haystack_expr: &Expression<'_>,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Option<PhpType> {
    // If the haystack is a simple variable, look it up in the scope.
    if let Expression::Variable(Variable::Direct(dv)) = haystack_expr {
        let var_name = dv.name.to_string();
        let types = scope.get(&var_name);
        if !types.is_empty() {
            let joined = ResolvedType::types_joined(types);
            if let Some(elem) = joined.extract_element_type() {
                return Some(elem.clone());
            }
            // Try extracting value type for generic collections.
            if let Some(val) = joined.extract_value_type(true) {
                return Some(val.clone());
            }
        }
        // Fall back to docblock annotation.
        let offset = haystack_expr.span().start.offset as usize;
        let from_docblock =
            crate::docblock::find_iterable_raw_type_in_source(ctx.content, offset, &var_name);
        if let Some(raw) = from_docblock
            && let Some(elem) = raw.extract_element_type()
        {
            return Some(elem.clone());
        }
        return None;
    }

    // For non-variable expressions (method calls, property access, etc.),
    // try resolving via the expression resolution pipeline.
    let scope_snapshot = scope.locals.clone();
    let scope_resolver =
        |vn: &str| -> Vec<ResolvedType> { scope_snapshot.get(vn).cloned().unwrap_or_default() };
    let var_ctx = build_var_ctx("", ctx, &scope_resolver);
    let raw_type =
        crate::completion::variable::resolution::resolve_arg_raw_type(haystack_expr, &var_ctx);
    raw_type.and_then(|t| t.extract_element_type().cloned())
}

/// Apply null narrowing for the truthy branch.
/// Build a [`VarResolutionCtx`] from a variable name and forward-walk context.
///
/// Shared helper used by the narrowing functions in this module to avoid
/// repeating the struct construction at every call site.
/// Apply `@phpstan-assert-if-true` / `@phpstan-assert-if-false` narrowing
/// from a function or static/instance method call used as a condition.
///
/// When `inverted` is false we are in the truthy branch (then-body or
/// while-body).  When `inverted` is true we are in the else branch or
/// applying guard-clause inverse narrowing.
fn apply_phpstan_assert_condition_narrowing<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inverted: bool,
) {
    use crate::types::AssertionKind;

    // Unwrap parentheses and detect negation (`!func($var)`).
    let (func_call_expr, condition_negated) = narrowing::unwrap_condition_negation(condition);

    let call = match func_call_expr {
        Expression::Call(c) => c,
        _ => return,
    };

    // Determine whether the function returned true in this branch.
    let function_returned_true = !(inverted ^ condition_negated);

    let scope_snapshot = scope.locals.clone();
    let scope_resolver =
        |vn: &str| -> Vec<ResolvedType> { scope_snapshot.get(vn).cloned().unwrap_or_default() };

    // Try to extract assertion info from function calls and static method calls.
    match call {
        Call::Function(func_call) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return,
            };
            let func_info = match ctx.loaders.function_loader {
                Some(fl) => match fl(&func_name) {
                    Some(fi) => fi,
                    None => return,
                },
                None => return,
            };
            if func_info.type_assertions.is_empty() {
                return;
            }
            for assertion in &func_info.type_assertions {
                let applies_positively = match assertion.kind {
                    AssertionKind::IfTrue => function_returned_true,
                    AssertionKind::IfFalse => !function_returned_true,
                    AssertionKind::Always => continue,
                };
                if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                    &func_call.argument_list,
                    &assertion.param_name,
                    &func_info.parameters,
                ) {
                    let should_exclude = assertion.negated ^ !applies_positively;
                    let var_ctx = build_var_ctx(&arg_var, ctx, &scope_resolver);
                    let mut results = scope.get(&arg_var).to_vec();
                    if should_exclude {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_exclusion(
                                &assertion.asserted_type,
                                &var_ctx,
                                classes,
                            );
                        });
                    } else {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_inclusion(
                                &assertion.asserted_type,
                                false,
                                &var_ctx,
                                classes,
                            );
                        });
                    }
                    if !results.is_empty() {
                        scope.set(arg_var, results);
                    }
                }
            }
        }
        Call::StaticMethod(static_call) => {
            let class_name = match static_call.class {
                Expression::Identifier(ident) => ident.value().to_string(),
                Expression::Self_(_) | Expression::Static(_) => ctx.current_class.name.clone(),
                _ => return,
            };
            let method_name = match &static_call.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
                _ => return,
            };
            let class_info = match (ctx.class_loader)(&class_name) {
                Some(ci) => ci,
                None => return,
            };
            let method = match class_info
                .methods
                .iter()
                .find(|m| m.name == method_name && m.is_static)
            {
                Some(m) => m.clone(),
                None => return,
            };
            if method.type_assertions.is_empty() {
                return;
            }
            for assertion in &method.type_assertions {
                let applies_positively = match assertion.kind {
                    AssertionKind::IfTrue => function_returned_true,
                    AssertionKind::IfFalse => !function_returned_true,
                    AssertionKind::Always => continue,
                };
                if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                    &static_call.argument_list,
                    &assertion.param_name,
                    &method.parameters,
                ) {
                    let should_exclude = assertion.negated ^ !applies_positively;
                    // Resolve `self`/`static`/`$this` in the asserted type
                    // against the declaring class, not the enclosing class.
                    let resolved_assert_type = if assertion.asserted_type.contains_self_ref() {
                        assertion.asserted_type.replace_self(&class_info.fqn())
                    } else {
                        assertion.asserted_type.clone()
                    };
                    let var_ctx = build_var_ctx(&arg_var, ctx, &scope_resolver);
                    let mut results = scope.get(&arg_var).to_vec();
                    if should_exclude {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_exclusion(
                                &resolved_assert_type,
                                &var_ctx,
                                classes,
                            );
                        });
                    } else {
                        ResolvedType::apply_narrowing(&mut results, |classes| {
                            narrowing::apply_instanceof_inclusion(
                                &resolved_assert_type,
                                false,
                                &var_ctx,
                                classes,
                            );
                        });
                    }
                    if !results.is_empty() {
                        scope.set(arg_var, results);
                    }
                }
            }
        }
        Call::Method(method_call) => {
            // Instance method: `$var->method()` with `@phpstan-assert-if-true Type $this`
            let receiver_var = match method_call.object {
                Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
                _ => return,
            };
            let method_name = match &method_call.method {
                ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
                _ => return,
            };
            // Resolve the receiver's type to find the method's assertions.
            let receiver_types = scope.get(&receiver_var);
            if receiver_types.is_empty() {
                return;
            }
            // Collect assertions from all candidate classes.
            let mut to_apply: Vec<(crate::php_type::PhpType, bool, String)> = Vec::new();
            for rt in receiver_types {
                let class_info = match (ctx.class_loader)(&rt.type_string.to_string()) {
                    Some(ci) => ci,
                    None => {
                        continue;
                    }
                };
                let method = match class_info
                    .methods
                    .iter()
                    .find(|m| m.name == method_name && !m.is_static)
                {
                    Some(m) => m,
                    None => continue,
                };
                for assertion in &method.type_assertions {
                    let applies_positively = match assertion.kind {
                        AssertionKind::IfTrue => function_returned_true,
                        AssertionKind::IfFalse => !function_returned_true,
                        AssertionKind::Always => continue,
                    };
                    let should_exclude = assertion.negated ^ !applies_positively;
                    // Resolve `self`/`static`/`$this` in the asserted type
                    // against the *declaring* class (e.g. `Decimal`), not the
                    // enclosing class (e.g. `Monetary`).  Without this,
                    // `@phpstan-assert-if-false self<true> $this` on
                    // `Decimal::isZero()` would narrow $denominator to
                    // `Monetary` instead of `Decimal`.
                    let resolved_type = if assertion.asserted_type.contains_self_ref() {
                        assertion.asserted_type.replace_self(&class_info.fqn())
                    } else {
                        assertion.asserted_type.clone()
                    };
                    if assertion.param_name == "$this" {
                        // Narrows the receiver variable itself.
                        to_apply.push((resolved_type, should_exclude, receiver_var.clone()));
                    } else if let Some(arg_var) = narrowing::find_assertion_arg_variable(
                        &method_call.argument_list,
                        &assertion.param_name,
                        &method.parameters,
                    ) {
                        to_apply.push((resolved_type, should_exclude, arg_var));
                    }
                }
            }
            for (asserted_type, should_exclude, target_var) in to_apply {
                let var_ctx = build_var_ctx(&target_var, ctx, &scope_resolver);
                let mut results = scope.get(&target_var).to_vec();
                if should_exclude {
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_exclusion(&asserted_type, &var_ctx, classes);
                    });
                } else {
                    ResolvedType::apply_narrowing(&mut results, |classes| {
                        narrowing::apply_instanceof_inclusion(
                            &asserted_type,
                            false,
                            &var_ctx,
                            classes,
                        );
                    });
                }
                if !results.is_empty() {
                    scope.set(target_var, results);
                }
            }
        }
        _ => {}
    }
}

fn build_var_ctx<'a>(
    var_name: &'a str,
    ctx: &'a ForwardWalkCtx<'_>,
    scope_resolver: &'a dyn Fn(&str) -> Vec<ResolvedType>,
) -> VarResolutionCtx<'a> {
    VarResolutionCtx {
        var_name,
        current_class: ctx.current_class,
        all_classes: ctx.all_classes,
        content: ctx.content,
        cursor_offset: ctx.cursor_offset,
        class_loader: ctx.class_loader,
        loaders: ctx.loaders,
        resolved_class_cache: ctx.resolved_class_cache,
        enclosing_return_type: ctx.enclosing_return_type.clone(),
        branch_aware: false,
        match_arm_narrowing: HashMap::new(),
        scope_var_resolver: Some(scope_resolver),
    }
}

///
/// Handles `$x !== null`, `$x != null`, `isset($x)`, `!empty($x)`,
/// `!is_null($x)`, and truthiness checks.
/// Apply type-guard narrowing in the truthy branch.
///
/// When `is_object($var)` (or `is_array`, `is_string`, etc.) appears
/// in a condition, narrow the variable's type.  For `mixed` variables,
/// this replaces `mixed` with the guard's canonical type (e.g. `object`).
/// For union types, it filters to only the members that match the guard.
///
/// Handles compound `&&` conditions by decomposing them into individual
/// operands and applying each type guard found.  For example,
/// `is_object($data) && property_exists($data, 'error_link')` applies
/// the `is_object` guard to `$data`.
fn apply_type_guard_narrowing_truthy(condition: &Expression<'_>, scope: &mut ScopeState) {
    apply_type_guard_on_operands(condition, scope, true);
}

/// Apply type-guard narrowing in the inverse (else) branch.
///
/// When `is_object($var)` appears in a condition, the else branch
/// knows the variable is NOT an object — filter out object-like
/// members from the union type.
fn apply_type_guard_narrowing_inverse(condition: &Expression<'_>, scope: &mut ScopeState) {
    apply_type_guard_on_operands(condition, scope, false);
}

/// Shared implementation for truthy and inverse type-guard narrowing.
///
/// Decomposes `&&` chains into individual operands and applies each
/// type guard found.  When `truthy` is `true`, applies inclusion
/// narrowing (then-body); when `false`, applies exclusion (else-body).
fn apply_type_guard_on_operands(condition: &Expression<'_>, scope: &mut ScopeState, truthy: bool) {
    // Decompose `&&` chains so that `is_object($x) && is_string($y)`
    // applies both guards.
    let operands = collect_and_chain_operands(condition);
    let var_names: Vec<String> = scope.locals.keys().cloned().collect();
    for operand in &operands {
        for var_name in &var_names {
            if let Some((kind, negated)) = narrowing::try_extract_type_guard(operand, var_name) {
                // When the guard is negated (e.g. `!is_object($x)`),
                // flip the inclusion/exclusion logic: the truthy branch
                // of a negated guard means the variable is NOT the
                // guarded type, and vice versa.
                let effective_truthy = if negated { !truthy } else { truthy };
                let mut results = scope.get(var_name).to_vec();
                if !results.is_empty() {
                    if effective_truthy {
                        narrowing::apply_type_guard_inclusion(kind, &mut results);
                    } else {
                        narrowing::apply_type_guard_exclusion(kind, &mut results);
                    }
                    if !results.is_empty() {
                        scope.set(var_name.clone(), results);
                    }
                }
            }
        }
    }
}

fn apply_null_narrowing_truthy<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    _ctx: &ForwardWalkCtx<'_>,
) {
    // Check for `$x !== null` or `$x != null` or `null !== $x` etc.
    if let Some(var_name) = extract_non_null_check_var(condition) {
        strip_null_from_scope(&var_name, scope);
    }
}

/// Apply inverse null narrowing (for guard clause: `if ($x === null) { return; }`).
fn apply_null_narrowing_inverse<'b>(condition: &'b Expression<'b>, scope: &mut ScopeState) {
    // When the condition is `$x === null` (equality check for null),
    // the inverse (else/guard) means $x is NOT null.
    if let Some(var_name) = extract_null_equality_check_var(condition) {
        strip_null_from_scope(&var_name, scope);
    }
    // When the condition is `!$x` or `empty($x)`, the inverse means
    // $x is truthy — remove null.
    if let Some(var_name) = extract_falsy_check_var(condition) {
        strip_null_from_scope(&var_name, scope);
    }
}

/// Extract variable name from `$x !== null` or `null !== $x` patterns.
fn extract_non_null_check_var(expr: &Expression<'_>) -> Option<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    match inner {
        Expression::Binary(bin) => {
            let is_not_identical = matches!(bin.operator, BinaryOperator::NotIdentical(_));
            let is_not_equal = matches!(bin.operator, BinaryOperator::NotEqual(_));
            let is_identical = matches!(bin.operator, BinaryOperator::Identical(_));
            let is_equal = matches!(bin.operator, BinaryOperator::Equal(_));

            // `$x !== null` or `null !== $x`
            if (is_not_identical || is_not_equal) && !negated
                || (is_identical || is_equal) && negated
            {
                if is_null_expr(bin.rhs) {
                    return expr_to_var_name(bin.lhs);
                }
                if is_null_expr(bin.lhs) {
                    return expr_to_var_name(bin.rhs);
                }
            }
            None
        }
        // `isset($x)` — parsed as a Call in mago
        Expression::Call(Call::Function(fc)) if matches!(fc.function, Expression::Identifier(ident) if ident.value() == "isset") =>
        {
            if !negated {
                for arg in fc.argument_list.arguments.iter() {
                    let arg_expr = match arg {
                        Argument::Positional(a) => a.value,
                        Argument::Named(a) => a.value,
                    };
                    if let Some(name) = expr_to_var_name(arg_expr) {
                        return Some(name);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract variable name from `$x === null` or `null === $x` patterns.
fn extract_null_equality_check_var(expr: &Expression<'_>) -> Option<String> {
    let (inner, negated) = narrowing::unwrap_condition_negation(expr);
    match inner {
        Expression::Binary(bin) => {
            let is_identical = matches!(bin.operator, BinaryOperator::Identical(_));
            let is_equal = matches!(bin.operator, BinaryOperator::Equal(_));

            if (is_identical || is_equal) && !negated {
                if is_null_expr(bin.rhs) {
                    return expr_to_var_name(bin.lhs);
                }
                if is_null_expr(bin.lhs) {
                    return expr_to_var_name(bin.rhs);
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract variable name from falsy checks: `!$x`, `empty($x)`.
fn extract_falsy_check_var(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            expr_to_var_name(prefix.operand)
        }
        // `empty($x)` — parsed as a Call in mago
        Expression::Call(Call::Function(fc)) if matches!(fc.function, Expression::Identifier(ident) if ident.value() == "empty") => {
            if let Some(arg) = fc.argument_list.arguments.first() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                expr_to_var_name(arg_expr)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if an expression is `null`.
fn is_null_expr(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Literal(Literal::Null(_)) => true,
        Expression::ConstantAccess(ca) => {
            let name = ca.name.value();
            let clean = crate::util::strip_fqn_prefix(name);
            clean.eq_ignore_ascii_case("null")
        }
        _ => false,
    }
}

/// Extract a direct variable name from an expression.
fn expr_to_var_name(expr: &Expression<'_>) -> Option<String> {
    if let Expression::Variable(Variable::Direct(dv)) = expr {
        Some(dv.name.to_string())
    } else {
        None
    }
}

/// Strip `null` from a variable's type in the scope.
fn strip_null_from_scope(var_name: &str, scope: &mut ScopeState) {
    let types = scope.get(var_name).to_vec();
    if types.is_empty() {
        return;
    }

    let stripped: Vec<ResolvedType> = types
        .into_iter()
        .filter_map(|mut rt| match rt.type_string.non_null_type() {
            Some(non_null) => {
                rt.type_string = non_null;
                Some(rt)
            }
            None if rt.type_string == PhpType::null() => None,
            None => Some(rt),
        })
        .collect();

    if !stripped.is_empty() {
        scope.set(var_name.to_string(), stripped);
    }
}

/// Apply guard-clause null narrowing.
fn apply_guard_clause_null_narrowing<'b>(
    if_stmt: &'b If<'b>,
    scope: &mut ScopeState,
    _ctx: &ForwardWalkCtx<'_>,
) {
    // When `if ($x === null) { return; }`, strip null from $x after.
    // When `if (!$x) { return; }`, strip null from $x after.
    if let Some(var_name) = extract_null_equality_check_var(if_stmt.condition) {
        strip_null_from_scope(&var_name, scope);
    }
    if let Some(var_name) = extract_falsy_check_var(if_stmt.condition) {
        strip_null_from_scope(&var_name, scope);
    }
    // `if ($x !== null)` with return doesn't narrow after — the
    // remaining code is the null path.  This is handled by the
    // inverse narrowing in the guard clause logic.
}

/// Process assignment in a condition: `if ($x = expr())`
fn process_condition_assignment<'b>(
    condition: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) {
    // Direct assignment: `if ($x = expr())`
    if let Expression::Assignment(assignment) = condition
        && assignment.operator.is_assign()
        && let Expression::Variable(Variable::Direct(dv)) = assignment.lhs
    {
        let var_name = dv.name.to_string();
        let rhs_types = resolve_rhs_with_scope(assignment.rhs, scope, ctx);
        if !rhs_types.is_empty() {
            scope.set(var_name, rhs_types);
        }
        return;
    }
    // Parenthesized conditions: `if (($x = expr()))`
    if let Expression::Parenthesized(inner) = condition {
        process_condition_assignment(inner.expression, scope, ctx);
        return;
    }
    // Assignment inside a binary comparison:
    //   `if (($x = expr()) !== null)` or `if (null !== ($x = expr()))`
    if let Expression::Binary(bin) = condition {
        // Check LHS directly: `($x = expr()) !== null`
        if let Expression::Assignment(_) = bin.lhs {
            process_condition_assignment(bin.lhs, scope, ctx);
            return;
        }
        if let Expression::Parenthesized(p) = bin.lhs
            && let Expression::Assignment(_) = p.expression
        {
            process_condition_assignment(p.expression, scope, ctx);
            return;
        }
        // Check RHS: `null !== ($x = expr())`
        if let Expression::Assignment(_) = bin.rhs {
            process_condition_assignment(bin.rhs, scope, ctx);
            return;
        }
        if let Expression::Parenthesized(p) = bin.rhs
            && let Expression::Assignment(_) = p.expression
        {
            process_condition_assignment(p.expression, scope, ctx);
        }
    }
}

/// Extract variable names referenced in instanceof / is_a / get_class
/// conditions.  This catches variables that are not yet in scope but
/// are used in guard clauses like `if (!$x instanceof Foo) { return; }`.
fn collect_condition_var_names(expr: &Expression<'_>) -> Vec<String> {
    let mut names = Vec::new();
    collect_condition_var_names_inner(expr, &mut names);
    names
}

fn collect_condition_var_names_inner(expr: &Expression<'_>, names: &mut Vec<String>) {
    match expr {
        Expression::Binary(bin) if bin.operator.is_instanceof() => {
            if let Expression::Variable(Variable::Direct(dv)) = bin.lhs {
                let name = dv.name.to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
            collect_condition_var_names_inner(prefix.operand, names);
        }
        Expression::Parenthesized(p) => {
            collect_condition_var_names_inner(p.expression, names);
        }
        Expression::Binary(bin)
            if matches!(
                bin.operator,
                BinaryOperator::And(_)
                    | BinaryOperator::LowAnd(_)
                    | BinaryOperator::Or(_)
                    | BinaryOperator::LowOr(_)
            ) =>
        {
            collect_condition_var_names_inner(bin.lhs, names);
            collect_condition_var_names_inner(bin.rhs, names);
        }
        // is_a($var, ...) and get_class($var) === ...
        Expression::Call(Call::Function(func_call)) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value(),
                _ => return,
            };
            if (func_name == "is_a" || func_name == "get_class")
                && let Some(first_arg) = func_call.argument_list.arguments.first()
            {
                let arg_expr = match first_arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
                    let name = dv.name.to_string();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Check if a statement unconditionally exits (return/throw/continue/break).
fn statement_unconditionally_exits(stmt: &Statement<'_>) -> bool {
    narrowing::statement_unconditionally_exits(stmt)
}

/// Check whether a statement exits via `break` or `continue` (loop-local
/// exit) rather than `return` or `throw` (function exit).
///
/// When an if-branch exits via `break`/`continue`, the variable
/// assignments made in that branch still flow to the post-loop scope.
/// The if-merge should include these branch scopes in the surviving
/// set so that the merged post-if scope reflects the assignments.
fn exits_via_loop_control(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::Break(_) | Statement::Continue(_) => true,
        Statement::Block(block) => block.statements.last().is_some_and(exits_via_loop_control),
        _ => false,
    }
}

// ─── Closure handling ───────────────────────────────────────────────────────

/// Try to enter a closure or arrow function if the cursor is inside one.
///
/// Returns `true` if the cursor was inside a closure and the scope was
/// updated accordingly.
fn try_enter_closure<'b>(
    stmt: &'b Statement<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> bool {
    // Walk the statement's expression tree looking for closures/arrow
    // functions that contain the cursor.
    if let Statement::Expression(expr_stmt) = stmt {
        return try_enter_closure_expr(expr_stmt.expression, scope, ctx, None);
    }
    if let Statement::Return(ret) = stmt
        && let Some(val) = ret.value
    {
        return try_enter_closure_expr(val, scope, ctx, None);
    }
    // Closures/arrow functions can appear inside if/while/for/switch
    // conditions (e.g. `if (array_any($items, fn($x) => $x->...))`).
    // Recurse into these condition expressions so the forward walker
    // can enter the closure scope.
    if let Statement::If(if_stmt) = stmt {
        if try_enter_closure_expr(if_stmt.condition, scope, ctx, None) {
            return true;
        }
        // Also check elseif conditions and bodies for closures.
        match &if_stmt.body {
            IfBody::Statement(body) => {
                for ei in body.else_if_clauses.iter() {
                    if try_enter_closure_expr(ei.condition, scope, ctx, None) {
                        return true;
                    }
                }
            }
            IfBody::ColonDelimited(body) => {
                for ei in body.else_if_clauses.iter() {
                    if try_enter_closure_expr(ei.condition, scope, ctx, None) {
                        return true;
                    }
                }
            }
        }
    }
    if let Statement::While(while_stmt) = stmt
        && try_enter_closure_expr(while_stmt.condition, scope, ctx, None)
    {
        return true;
    }
    if let Statement::For(for_stmt) = stmt {
        for cond in for_stmt.conditions.iter() {
            if try_enter_closure_expr(cond, scope, ctx, None) {
                return true;
            }
        }
    }
    if let Statement::Switch(switch) = stmt {
        if try_enter_closure_expr(switch.expression, scope, ctx, None) {
            return true;
        }
        for case in switch.body.cases().iter() {
            if let Some(cond) = case.expression()
                && try_enter_closure_expr(cond, scope, ctx, None)
            {
                return true;
            }
        }
    }
    false
}

/// Recursively search an expression for a closure/arrow function
/// containing the cursor.
fn try_enter_closure_expr<'b>(
    expr: &'b Expression<'b>,
    scope: &mut ScopeState,
    ctx: &ForwardWalkCtx<'_>,
    inferred_params: Option<&[PhpType]>,
) -> bool {
    match expr {
        Expression::Closure(closure) => {
            let body_span = closure.body.span();
            if ctx.cursor_offset >= body_span.start.offset
                && ctx.cursor_offset <= body_span.end.offset
            {
                // Create a fresh scope for the closure (closures have
                // isolated scope in PHP).
                let mut closure_scope = ScopeState::new();

                // PHP closures implicitly capture `$this` from the
                // enclosing class method.
                let this_types = scope.get("$this");
                if !this_types.is_empty() {
                    closure_scope.set("$this".to_string(), this_types.to_vec());
                }

                // Seed with `use(...)` variables from the outer scope.
                if let Some(ref use_clause) = closure.use_clause {
                    for use_var in use_clause.variables.iter() {
                        let var_name = use_var.variable.name.to_string();
                        let from_outer = scope.get(&var_name);
                        if !from_outer.is_empty() {
                            closure_scope.set(var_name, from_outer.to_vec());
                        }
                    }
                }

                // Seed with parameter types, using callable inference
                // when available (mirroring the diagnostic path's
                // seed_closure_params logic).
                let inferred = inferred_params.unwrap_or(&[]);
                let filtered_inferred = filter_resolvable_inferred_params(inferred, ctx);
                seed_closure_params(
                    &mut closure_scope,
                    &closure.parameter_list,
                    closure.span().start.offset,
                    &filtered_inferred,
                    ctx,
                );

                // Walk the closure body.
                walk_body_forward(closure.body.statements.iter(), &mut closure_scope, ctx);

                // Replace the outer scope with the closure scope.
                *scope = closure_scope;
                return true;
            }
        }
        Expression::ArrowFunction(arrow) => {
            let body_span = arrow.expression.span();
            if ctx.cursor_offset >= body_span.start.offset
                && ctx.cursor_offset <= body_span.end.offset
            {
                // Arrow functions inherit the enclosing scope.
                // Seed with parameter types, using callable inference
                // when available.
                let inferred = inferred_params.unwrap_or(&[]);
                let filtered_inferred = filter_resolvable_inferred_params(inferred, ctx);
                seed_closure_params(
                    scope,
                    &arrow.parameter_list,
                    arrow.span().start.offset,
                    &filtered_inferred,
                    ctx,
                );
                // The body is a single expression.  Recurse into it
                // to find nested closures/arrow functions that may
                // contain the cursor (e.g. a closure passed as an
                // argument inside the arrow body).
                try_enter_closure_expr(arrow.expression, scope, ctx, None);
                return true;
            }
        }
        // Recurse into sub-expressions that might contain closures.
        Expression::Parenthesized(inner) => {
            return try_enter_closure_expr(inner.expression, scope, ctx, None);
        }
        Expression::Assignment(assignment) => {
            // Process the assignment first so the LHS var is in scope.
            process_assignment_expr(expr, scope, ctx);
            return try_enter_closure_expr(assignment.rhs, scope, ctx, None);
        }
        Expression::Call(call) => {
            // Check if any argument is a closure containing the cursor.
            // Infer callable parameter types from the function/method
            // signature so closure params get generic-substituted types
            // (mirroring the diagnostic path's walk_closures_in_call).
            let args = match call {
                Call::Function(fc) => &fc.argument_list,
                Call::Method(mc) => &mc.argument_list,
                Call::NullSafeMethod(mc) => &mc.argument_list,
                Call::StaticMethod(sc) => &sc.argument_list,
            };
            for (arg_idx, arg) in args.arguments.iter().enumerate() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                // Only infer callable params when the argument is a
                // closure or arrow function (or wraps one).
                let inferred = infer_callable_params_for_call(call, arg_idx, scope, ctx);
                let inferred_opt = if inferred.is_empty() {
                    None
                } else {
                    Some(inferred.as_slice())
                };
                if try_enter_closure_expr(arg_expr, scope, ctx, inferred_opt) {
                    return true;
                }
            }
        }
        Expression::Access(access) => match access {
            Access::Property(pa) => {
                return try_enter_closure_expr(pa.object, scope, ctx, None);
            }
            Access::NullSafeProperty(pa) => {
                return try_enter_closure_expr(pa.object, scope, ctx, None);
            }
            _ => {}
        },
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                let elem_expr = match elem {
                    ArrayElement::KeyValue(kv) => kv.value,
                    ArrayElement::Value(val) => val.value,
                    ArrayElement::Variadic(v) => v.value,
                    ArrayElement::Missing(_) => continue,
                };
                if try_enter_closure_expr(elem_expr, scope, ctx, None) {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

/// Infer callable parameter types for a specific argument index of a
/// call expression.  This reuses the same inference functions as the
/// diagnostic path (`infer_callable_params_from_function_fw`,
/// `infer_callable_params_from_receiver_fw`,
/// `infer_callable_params_from_static_receiver_fw`) so that closure
/// parameters on the completion/hover path receive the same
/// generic-substituted types.
fn infer_callable_params_for_call(
    call: &Call<'_>,
    arg_idx: usize,
    scope: &ScopeState,
    ctx: &ForwardWalkCtx<'_>,
) -> Vec<PhpType> {
    match call {
        Call::Function(fc) => {
            let func_name = match fc.function {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                _ => None,
            };
            if let Some(ref name) = func_name {
                infer_callable_params_from_function_fw(
                    name,
                    arg_idx,
                    &fc.argument_list.arguments,
                    scope,
                    ctx,
                )
            } else {
                vec![]
            }
        }
        Call::Method(mc) => {
            let method_name = if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                Some(ident.value.to_string())
            } else {
                None
            };
            if let Some(ref name) = method_name {
                let obj_span = mc.object.span();
                let first_arg =
                    extract_first_arg_string_fw(&mc.argument_list.arguments, ctx.content);
                infer_callable_params_from_receiver_fw(
                    obj_span.start.offset,
                    obj_span.end.offset,
                    name,
                    arg_idx,
                    first_arg.as_deref(),
                    scope,
                    ctx,
                )
            } else {
                vec![]
            }
        }
        Call::NullSafeMethod(mc) => {
            let method_name = if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                Some(ident.value.to_string())
            } else {
                None
            };
            if let Some(ref name) = method_name {
                let obj_span = mc.object.span();
                let first_arg =
                    extract_first_arg_string_fw(&mc.argument_list.arguments, ctx.content);
                infer_callable_params_from_receiver_fw(
                    obj_span.start.offset,
                    obj_span.end.offset,
                    name,
                    arg_idx,
                    first_arg.as_deref(),
                    scope,
                    ctx,
                )
            } else {
                vec![]
            }
        }
        Call::StaticMethod(sc) => {
            let method_name = if let ClassLikeMemberSelector::Identifier(ident) = &sc.method {
                Some(ident.value.to_string())
            } else {
                None
            };
            if let Some(ref name) = method_name {
                let first_arg =
                    extract_first_arg_string_fw(&sc.argument_list.arguments, ctx.content);
                infer_callable_params_from_static_receiver_fw(
                    sc.class,
                    name,
                    arg_idx,
                    first_arg.as_deref(),
                    scope,
                    ctx,
                )
            } else {
                vec![]
            }
        }
    }
}
