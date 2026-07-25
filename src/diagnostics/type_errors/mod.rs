//! Argument type mismatch diagnostics.
//!
//! Walk the precomputed [`CallSite`] entries in the symbol map and flag
//! every call where an argument's resolved type is incompatible with
//! the declared parameter type.
//!
//! This is Phase 1 of the type error diagnostic suite. Only clearly
//! incompatible types are flagged — when in doubt (unresolved types,
//! `mixed`, complex generics), the diagnostic is suppressed to avoid
//! false positives.

mod compatibility;

pub(super) use compatibility::is_type_compatible;

use std::collections::{HashMap, HashSet};

use mago_span::HasSpan;
use mago_syntax::cst::argument::Argument;
use mago_syntax::cst::expression::Expression;
use mago_syntax::cst::literal::Literal;
use mago_syntax::cst::statement::Statement;
use mago_syntax::cst::{PartialArgument, Program};
use mago_syntax::walker::Walker;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::atom::bytes_to_str;
use crate::parser::{with_parse_cache, with_parsed_program};
use crate::php_type::{PhpType, is_array_like_name};
use crate::type_engine::resolver::{Loaders, VarResolutionCtx};
use crate::type_engine::variable::foreach_resolution::resolve_expression_type;
use crate::types::{ClassInfo, ResolvedCallableTarget};

use super::helpers::{find_innermost_enclosing_class, make_diagnostic};

/// Diagnostic code used for argument type mismatch diagnostics.
pub(crate) const TYPE_MISMATCH_ARGUMENT_CODE: &str = "type_mismatch_argument";

// ── Resolved argument info ──────────────────────────────────────────────────

/// A single argument's resolved type plus the byte range of the
/// expression in source.  Collected inside `with_parsed_program` so
/// we don't need to keep AST references alive.
struct ResolvedArg {
    /// The resolved type of the argument expression.
    ty: PhpType,
    /// Byte offset of the argument expression start (inclusive).
    start: usize,
    /// Byte offset of the argument expression end (exclusive).
    end: usize,
    /// String literal values extracted from array argument elements,
    /// with their byte offset spans.  Used for `model-property<Model>`
    /// validation when the param type is an array with `model-property`
    /// in a generic position.
    array_string_literals: Vec<(String, usize, usize)>,
}

/// All resolved argument types for a single call site.
struct ResolvedCallArgs {
    args: Vec<ResolvedArg>,
}

/// Returns `true` when the file declares `strict_types=1`.
///
/// Scans the top-level statements of the parsed program for a
/// `declare(strict_types=1)` directive.  In PHP this must appear as
/// the very first statement (after `<?php`), but we check all
/// top-level statements for robustness.
pub(super) fn has_strict_types(program: &Program<'_>) -> bool {
    for stmt in program.statements.iter() {
        if let Statement::Declare(declare) = stmt {
            for item in declare.items.iter() {
                if bytes_to_str(item.name.value).eq_ignore_ascii_case("strict_types")
                    && let Expression::Literal(Literal::Integer(i)) = item.value
                    && bytes_to_str(i.raw) == "1"
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract string literal values from array expression elements.
///
/// Collects both keys and values so that `model-property<Model>`
/// validation works regardless of whether the model-property type
/// is in the key or value position of the param's array generic.
fn extract_array_string_literals(expr: &Expression<'_>) -> Vec<(String, usize, usize)> {
    use mago_syntax::cst::array::ArrayElement;

    let elements = match expr {
        Expression::Array(arr) => &arr.elements,
        Expression::LegacyArray(arr) => &arr.elements,
        _ => return Vec::new(),
    };

    let mut literals = Vec::new();
    let mut push_string = |s: &mago_syntax::cst::literal::LiteralString| {
        if let Some(content) = crate::text_scan::unquote_php_string(bytes_to_str(s.raw)) {
            let start = s.span.start.offset as usize;
            let end = s.span.end.offset as usize;
            literals.push((content.to_string(), start, end));
        }
    };
    for elem in elements.iter() {
        match elem {
            ArrayElement::KeyValue(kv) => {
                if let Expression::Literal(Literal::String(s)) = kv.key {
                    push_string(s);
                }
                if let Expression::Literal(Literal::String(s)) = kv.value {
                    push_string(s);
                }
            }
            ArrayElement::Value(v) => {
                if let Expression::Literal(Literal::String(s)) = v.value {
                    push_string(s);
                }
            }
            _ => {}
        }
    }
    literals
}

/// Extract the model name from a `model-property<Model>` type that
/// appears as a generic argument of an array/list type.
fn extract_model_property_from_array_type(ty: &PhpType) -> Option<String> {
    let PhpType::Generic(name, args) = ty else {
        return None;
    };
    if !is_array_like_name(name) && !name.eq_ignore_ascii_case("list") {
        return None;
    }
    for arg in args {
        if let PhpType::Generic(n, inner) = arg
            && n.eq_ignore_ascii_case("model-property")
            && inner.len() == 1
        {
            return inner[0].base_name().map(|s| s.to_string());
        }
    }
    None
}

// ── AST walking: collect argument expressions keyed by args_start ───────────

/// Try to register the argument expressions of an [`ArgumentList`] if its
/// `args_start` offset matches one of the call sites we are interested in.
fn try_collect_argument_list<'a>(
    arg_list: &'a mago_syntax::cst::argument::ArgumentList<'a>,
    call_site_starts: &HashSet<u32>,
    result: &mut HashMap<u32, Vec<(&'a Expression<'a>, usize, usize)>>,
) {
    let args_start = arg_list.left_parenthesis.end.offset;
    if !call_site_starts.contains(&args_start) {
        return;
    }
    let expressions: Vec<(&'a Expression<'a>, usize, usize)> = arg_list
        .arguments
        .iter()
        .map(|arg| {
            let value = match arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            let start = value.span().start.offset as usize;
            let end = value.span().end.offset as usize;
            (value, start, end)
        })
        .collect();
    result.insert(args_start, expressions);
}

/// Try to register the partial argument expressions of an [`PartialArgumentList`] if its
/// `args_start` offset matches one of the call sites we are interested in.
fn try_collect_partial_argument_list<'a>(
    arg_list: &'a mago_syntax::cst::argument::PartialArgumentList<'a>,
    call_site_starts: &HashSet<u32>,
    result: &mut HashMap<u32, Vec<(&'a Expression<'a>, usize, usize)>>,
) {
    let args_start = arg_list.left_parenthesis.end.offset;
    if !call_site_starts.contains(&args_start) {
        return;
    }
    // Placeholders have no expression and therefore no type to validate.
    // Keep only supplied positional/named arguments.
    let expressions: Vec<(&'a Expression<'a>, usize, usize)> = arg_list
        .arguments
        .iter()
        .filter_map(|arg| {
            let value = match arg {
                PartialArgument::Positional(pos) => pos.value,
                PartialArgument::Named(named) => named.value,
                PartialArgument::NamedPlaceholder(_)
                | PartialArgument::Placeholder(_)
                | PartialArgument::VariadicPlaceholder(_) => return None,
            };
            let start = value.span().start.offset as usize;
            let end = value.span().end.offset as usize;
            Some((value, start, end))
        })
        .collect();
    result.insert(args_start, expressions);
}

// ── AST walk: collect call/instantiation argument lists ────────────────────

/// Walker that records the argument expressions of every call site whose
/// `args_start` offset is one we care about (`starts`). The generated
/// traversal visits every argument list in the tree; the two overrides
/// register the ones in `starts`, and default recursion descends into
/// argument values to find nested calls.
struct CallArgWalker<'s> {
    starts: &'s HashSet<u32>,
}

impl<'a, 's>
    mago_syntax::walker::Walker<'a, 'a, HashMap<u32, Vec<(&'a Expression<'a>, usize, usize)>>>
    for CallArgWalker<'s>
{
    fn walk_in_argument_list(
        &self,
        node: &'a mago_syntax::cst::argument::ArgumentList<'a>,
        context: &mut HashMap<u32, Vec<(&'a Expression<'a>, usize, usize)>>,
    ) {
        try_collect_argument_list(node, self.starts, context);
    }

    fn walk_in_partial_argument_list(
        &self,
        node: &'a mago_syntax::cst::argument::PartialArgumentList<'a>,
        context: &mut HashMap<u32, Vec<(&'a Expression<'a>, usize, usize)>>,
    ) {
        try_collect_partial_argument_list(node, self.starts, context);
    }
}

// ── Main diagnostic collection ──────────────────────────────────────────────

impl Backend {
    /// Collect argument type mismatch diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub fn collect_argument_type_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Gather context under locks ──────────────────────────────
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_ctx = self.file_context(uri);

        // Activate the thread-local parse cache so that every call to
        // `with_parsed_program(content, …)` in the resolution pipeline
        // reuses the same parsed AST instead of re-parsing the file.
        let _parse_guard = with_parse_cache(content);

        // Build the set of call site args_start offsets for the AST walk.
        // Only include sites without argument unpacking.
        let call_site_starts: HashSet<u32> = symbol_map
            .call_sites
            .iter()
            .filter(|cs| !cs.has_unpacking)
            .map(|cs| cs.args_start)
            .collect();

        if call_site_starts.is_empty() {
            return;
        }

        let class_loader = self.class_loader(&file_ctx);
        let function_loader_cl = self.function_loader(&file_ctx);
        let constant_loader_cl = self.constant_loader();
        let default_class = ClassInfo::default();

        // Walk the AST once, collect argument expressions, and resolve
        // their types — all inside the `with_parsed_program` closure so
        // AST references never escape the arena lifetime.
        let (resolved_map, strict_types): (HashMap<u32, ResolvedCallArgs>, bool) =
            with_parsed_program(content, "type_error_diagnostics", |program, _content| {
                let strict_types = has_strict_types(program);

                // Phase 1: walk the AST and collect raw argument expressions
                // keyed by args_start offset.
                let mut expr_map: HashMap<u32, Vec<(&Expression<'_>, usize, usize)>> =
                    HashMap::new();
                let walker = CallArgWalker {
                    starts: &call_site_starts,
                };
                for stmt in program.statements.iter() {
                    walker.walk_statement(stmt, &mut expr_map);
                }

                // Phase 2: resolve types for each collected expression.
                let mut result: HashMap<u32, ResolvedCallArgs> = HashMap::new();
                for (args_start, exprs) in &expr_map {
                    let enclosing = find_innermost_enclosing_class(&file_ctx.classes, *args_start);
                    let current_class_info = enclosing.unwrap_or(&default_class);

                    let config_resolver = |key: &str| self.resolve_config_type(key);
                    let loaders = Loaders {
                        function_loader: Some(&function_loader_cl),
                        constant_loader: Some(&constant_loader_cl),
                        config_resolver: Some(&config_resolver),
                    };

                    let var_ctx = VarResolutionCtx {
                        var_name: "",
                        top_level_scope: None,
                        current_class: current_class_info,
                        all_classes: &file_ctx.classes,
                        content,
                        cursor_offset: *args_start,
                        class_loader: &class_loader,
                        loaders,
                        resolved_class_cache: Some(&self.resolved_class_cache),
                        enclosing_return_type: None,
                        branch_aware: true,
                        match_arm_narrowing: HashMap::new(),
                        scope_var_resolver: None,
                    };

                    let mut resolved_args = Vec::with_capacity(exprs.len());
                    for &(arg_expr, start, end) in exprs {
                        // Use the argument expression's own start offset
                        // as the cursor position so that variable
                        // resolution only sees assignments *before* this
                        // expression.  Without this, `$type = Enum::from($type)`
                        // would resolve the RHS `$type` to `Enum` (the
                        // result of the assignment on the same line)
                        // instead of the prior `string` value from the
                        // foreach key.
                        let arg_ctx = var_ctx.with_cursor_offset(start as u32);
                        let ty = resolve_expression_type(arg_expr, &arg_ctx)
                            .unwrap_or_else(PhpType::untyped);
                        // Narrow scalar literals to their precise literal
                        // type so that e.g. `'desc'` matches `'asc'|'desc'`
                        // and `2` matches `1|2|3`.  The general expression
                        // resolver returns the widened base type (`string`,
                        // `int`, `float`) which is correct for variable
                        // tracking, but for argument diagnostics we need
                        // the exact value to compare against literal types.
                        //
                        // Numeric literals use the parsed value rather than
                        // the raw source text so that non-decimal forms
                        // (`0xFF`, `0b1010`, `0o17`, `1_000`) still match
                        // `int`/`float`/`numeric` parameters and decimal
                        // literal unions (`1|2|3`).  The raw text would not
                        // parse back into a number and would be flagged as
                        // an incompatible argument.
                        let ty = match arg_expr {
                            Expression::Literal(Literal::String(s)) => {
                                PhpType::literal_string_raw(bytes_to_str(s.raw).to_string())
                            }
                            Expression::Literal(Literal::Integer(i)) => match i.value {
                                Some(value) => PhpType::literal_int(value.to_string()),
                                None => ty,
                            },
                            Expression::Literal(Literal::Float(f)) => {
                                PhpType::literal_float(bytes_to_str(f.raw).to_string())
                            }
                            // Negative numeric literals (`-1`, `-1.5`) parse
                            // as a unary negation wrapping the literal, not
                            // as a single `Literal` node, so they need their
                            // own case to be narrowed the same way.
                            Expression::UnaryPrefix(unary)
                                if matches!(
                                    unary.operator,
                                    mago_syntax::cst::unary::UnaryPrefixOperator::Negation(_)
                                ) =>
                            {
                                match unary.operand {
                                    Expression::Literal(Literal::Integer(i)) => match i.value {
                                        Some(value) => PhpType::literal_int(format!("-{value}")),
                                        None => ty,
                                    },
                                    Expression::Literal(Literal::Float(f)) => {
                                        PhpType::literal_float(format!("-{}", bytes_to_str(f.raw)))
                                    }
                                    _ => ty,
                                }
                            }
                            _ => ty,
                        };
                        // Resolve any short class names in the arg type
                        // to FQN via the class loader.  Variable
                        // resolution may return raw docblock names
                        // (e.g. `SubscriptionProduct` instead of its
                        // FQN) — normalise them so that comparisons
                        // against parameter types (which are already
                        // FQN from resolve_parent_class_names) succeed.
                        let ty = ty.resolve_names(&|name: &str| {
                            // Don't FQN-ify anonymous class names — they
                            // use synthetic names (`__anonymous@<offset>`)
                            // that are only resolvable via the local-class
                            // shortcut in the class loader.  Prepending
                            // the namespace would break that lookup.
                            if name.contains("__anonymous@") {
                                return name.to_string();
                            }
                            if let Some(cls) = class_loader(name) {
                                cls.fqn().to_string()
                            } else {
                                name.to_string()
                            }
                        });
                        // Expand @phpstan-type / @psalm-type aliases so
                        // that e.g. `Payload` becomes `array{name: string,
                        // phone: string}` before the compatibility check.
                        let ty = crate::type_engine::types::resolution::resolve_type_alias_typed(
                            &ty,
                            &current_class_info.fqn(),
                            &file_ctx.classes,
                            &class_loader,
                        )
                        .unwrap_or(ty);
                        let array_string_literals = extract_array_string_literals(arg_expr);
                        resolved_args.push(ResolvedArg {
                            ty,
                            start,
                            end,
                            array_string_literals,
                        });
                    }
                    result.insert(
                        *args_start,
                        ResolvedCallArgs {
                            args: resolved_args,
                        },
                    );
                }
                (result, strict_types)
            });

        // Call-expression resolution cache: avoids re-resolving the
        // same call expression (e.g. `ClassName::method`) at every
        // call site that uses it.
        //
        // Only expressions that are guaranteed to resolve to the same
        // target everywhere in the file are cached.  Variable-based
        // calls (`$listener->handle`, `$repo->save`) are NOT cached
        // because the same variable name can hold different types in
        // different methods or after reassignment.  Static calls
        // (`Foo::bar`) and plain function calls (`array_map`) are
        // safe to cache.
        let mut call_cache: HashMap<String, Option<ResolvedCallableTarget>> = HashMap::new();

        // ── Walk every call site ────────────────────────────────────
        for call_site in &symbol_map.call_sites {
            // Skip calls with argument unpacking — actual types of
            // individual arguments are unknown.
            if call_site.has_unpacking {
                continue;
            }

            let expr = &call_site.call_expression;

            // Look up or populate the call expression cache.
            // Variable-based calls are resolved fresh every time
            // because the receiver variable may hold different types
            // at different call sites (e.g. `$listener->handle` in
            // two different test methods with different assignments).
            let is_variable_call = expr.starts_with('$');

            // Extract the raw argument text from the source so that
            // method-level @template parameters can be resolved from
            // the call-site argument types.
            let call_args_text: Option<&str> = {
                let start = call_site.args_start as usize;
                let end = call_site.args_end as usize;
                if let Some(slice) = content.get(start..end) {
                    let trimmed = slice.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                } else {
                    None
                }
            };

            // Resolve the callable target.  Class-level template
            // substitution happens inside resolve_callable_target
            // (driven by the variable's generic type string).
            // Method-level template substitution uses the per-site
            // argument text extracted above.
            //
            // Variable-based calls are always resolved per-site
            // because the receiver variable may hold different types
            // at different call sites.  Static/function calls are
            // also resolved per-site when argument text is available
            // (method-level template subs depend on per-site args);
            // only zero-arg calls can be cached.
            let resolved = if is_variable_call || call_args_text.is_some() {
                let position = crate::text_position::offset_to_position(
                    content,
                    call_site.args_start as usize,
                );
                self.resolve_callable_target_with_args(
                    expr,
                    content,
                    position,
                    &file_ctx,
                    call_args_text,
                )
            } else {
                call_cache
                    .entry(expr.clone())
                    .or_insert_with(|| {
                        let position = crate::text_position::offset_to_position(
                            content,
                            call_site.args_start as usize,
                        );
                        self.resolve_callable_target(expr, content, position, &file_ctx)
                    })
                    .clone()
            };

            // Resolve the call expression to a callable target.
            let resolved = match resolved {
                Some(r) => r,
                None => continue,
            };

            // Get resolved argument types for this call site.
            let resolved_args = match resolved_map.get(&call_site.args_start) {
                Some(c) => c,
                None => continue,
            };

            let params = &resolved.parameters;

            // Track how many positional args we've seen so far for
            // mapping positional args to parameter indices.
            let mut positional_idx: usize = 0;

            // Check each argument against its parameter.
            for (arg_idx, resolved_arg) in resolved_args.args.iter().enumerate() {
                // Skip spread arguments.
                if call_site.spread_arg_indices.contains(&(arg_idx as u32)) {
                    continue;
                }

                // Find the corresponding parameter.
                let param = if call_site.named_arg_indices.contains(&(arg_idx as u32)) {
                    // Named argument: look up parameter by name.
                    let name_pos = call_site
                        .named_arg_indices
                        .iter()
                        .position(|&i| i == arg_idx as u32);
                    match name_pos {
                        Some(idx) => {
                            let param_name = &call_site.named_arg_names[idx];
                            params
                                .iter()
                                .find(|p| p.name.trim_start_matches('$') == param_name.as_str())
                        }
                        None => continue,
                    }
                } else {
                    // Positional argument.
                    let p = params.get(positional_idx);
                    positional_idx += 1;
                    p
                };

                let param = match param {
                    Some(p) => p,
                    None => continue, // Extra argument beyond declared params
                };

                // Skip if parameter has no type hint.
                let param_type = match &param.type_hint {
                    Some(t) if !t.is_untyped() && !t.is_mixed() => t,
                    _ => continue,
                };

                // Skip variadic parameters — hard to match individual
                // arg types to the variadic param's inner type.
                if param.is_variadic {
                    continue;
                }

                let arg_type = &resolved_arg.ty;

                // Skip unresolved / empty / Raw("") sentinel types.
                if arg_type.is_untyped()
                    || arg_type.is_empty()
                    || matches!(arg_type, PhpType::Raw(s) if s.is_empty())
                {
                    continue;
                }

                // Check compatibility.
                if is_type_compatible(arg_type, param_type, &class_loader, strict_types) {
                    // Even when the array types are compatible, validate
                    // string literals against model-property<Model> when
                    // the param type is an array with model-property in
                    // a generic position.
                    if !resolved_arg.array_string_literals.is_empty()
                        && let Some(model_fqn) = extract_model_property_from_array_type(param_type)
                        && let Some(cls) = class_loader(&model_fqn)
                    {
                        let resolved = crate::virtual_members::resolve_class_fully_cached(
                            &cls,
                            &class_loader,
                            &self.resolved_class_cache,
                        );
                        let columns: Vec<String> = resolved
                            .properties
                            .iter()
                            .map(|p| p.name.to_string())
                            .collect();
                        for (lit, lit_start, lit_end) in &resolved_arg.array_string_literals {
                            let found = columns.iter().any(|col| col == lit);
                            if !found
                                && let Some(range) = self
                                    .offset_range_to_lsp_range(uri, content, *lit_start, *lit_end)
                            {
                                out.push(make_diagnostic(
                                    range,
                                    DiagnosticSeverity::ERROR,
                                    TYPE_MISMATCH_ARGUMENT_CODE,
                                    format!(
                                        "'{}' is not a known property of {}",
                                        lit,
                                        model_fqn.rsplit('\\').next().unwrap_or(&model_fqn),
                                    ),
                                ));
                            }
                        }
                    }
                    continue;
                }

                // When the function has overloaded signatures, check
                // whether the argument is compatible with the same
                // positional parameter in any overload.  Only emit the
                // diagnostic when ALL signatures reject it.
                if !resolved.overloads.is_empty() {
                    let compatible_with_overload = resolved.overloads.iter().any(|alt_params| {
                        if let Some(alt_param) = alt_params.get(positional_idx.saturating_sub(1)) {
                            if let Some(ref alt_type) = alt_param.type_hint
                                && !alt_type.is_untyped()
                                && !alt_type.is_mixed()
                            {
                                return is_type_compatible(
                                    arg_type,
                                    alt_type,
                                    &class_loader,
                                    strict_types,
                                );
                            }
                            true // no type hint on alt param = compatible
                        } else {
                            // This overload has fewer params — the arg
                            // doesn't correspond to any parameter, so
                            // it's not relevant for this check.
                            false
                        }
                    });
                    if compatible_with_overload {
                        continue;
                    }
                }

                // Emit diagnostic.
                let range = match self.offset_range_to_lsp_range(
                    uri,
                    content,
                    resolved_arg.start,
                    resolved_arg.end,
                ) {
                    Some(r) => r,
                    None => continue,
                };

                let param_name = &param.name;
                // Always show full type names (FQN) so the developer
                // can actually find and fix the types.  Short names
                // strip the namespace which is the very information
                // needed to resolve the mismatch.
                let message = format!(
                    "Argument {} ({}) expects {}, got {}",
                    arg_idx + 1,
                    param_name,
                    param_type,
                    arg_type,
                );

                out.push(make_diagnostic(
                    range,
                    DiagnosticSeverity::ERROR,
                    TYPE_MISMATCH_ARGUMENT_CODE,
                    message,
                ));
            }
        }
    }
}
