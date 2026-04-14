//! Variable type resolution for hover.
//!
//! Unlike the completion pipeline (which resolves variables to
//! `Vec<ClassInfo>`), hover needs the **full type** so that
//! generic parameters and scalar types are preserved.  For example,
//! a parameter typed as `\Generator<int, Pencil>` should display
//! exactly that, not just `Generator`.
//!
//! The entry point is [`resolve_variable_type`], which walks
//! the AST to find the variable's definition context and returns the
//! effective type as a [`PhpType`] (docblock overriding native where
//! applicable).

use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::docblock;
use crate::parser::{extract_hint_type, with_parsed_program};
use crate::php_type::PhpType;
use crate::types::{AccessKind, ClassInfo, ResolvedType};
use crate::util::short_name;

use crate::completion::resolver::Loaders;
use crate::completion::variable::resolution::resolve_variable_types_branch_aware;

/// Context for closure parameter type resolution in hover.
///
/// Carries the class resolution infrastructure needed to infer callable
/// parameter types from enclosing call expressions, so that a closure
/// parameter typed as a parent class (e.g. `Model`) can be narrowed to
/// the inferred subclass (e.g. `BrandTranslation`).
struct HoverClosureCtx<'a> {
    current_class: Option<&'a ClassInfo>,
    all_classes: &'a [Arc<ClassInfo>],
    class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    content: &'a str,
}

/// Check whether `cursor_offset` falls inside the RHS of an assignment
/// like `$var = $var->…` on the same line.  Used by hover to avoid
/// applying an inline `@var` cast to the RHS reference.
fn is_cursor_in_self_assignment_rhs(content: &str, cursor_offset: usize, var_name: &str) -> bool {
    // Find the line containing the cursor.
    let line_start = content[..cursor_offset]
        .rfind('\n')
        .map_or(0, |pos| pos + 1);
    let line_end = content[cursor_offset..]
        .find('\n')
        .map_or(content.len(), |pos| cursor_offset + pos);
    let line = &content[line_start..line_end];

    // Look for `$var = ` (plain assignment) on this line.
    let needle = format!("{} = ", var_name);
    if let Some(assign_pos) = line.find(&needle) {
        // The RHS starts right after `$var = `.
        let rhs_start_in_line = assign_pos + needle.len();
        let cursor_in_line = cursor_offset - line_start;
        // Cursor must be in the RHS portion and the RHS must mention
        // the same variable (self-referential assignment).
        let rhs = &line[rhs_start_in_line..];
        if cursor_in_line >= rhs_start_in_line && rhs.contains(var_name) {
            return true;
        }
    }
    false
}

/// Resolve the type for a variable at `cursor_offset` for hover.
///
/// Tries, in order:
/// 1. Inline `/** @var Type $var */` docblock override
/// 2. Parameter type (native + `@param` → effective)
/// 3. Foreach value/key binding (iterable element type from `@param`/`@var`)
/// 4. Catch variable (catch clause hint)
/// 5. Assignment type via [`resolve_variable_types_branch_aware`]
///
/// Returns `None` when no type information could be determined.
pub(crate) fn resolve_variable_type(
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: Option<&ClassInfo>,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    loaders: Loaders<'_>,
) -> Option<PhpType> {
    // 1. Inline @var override: `/** @var Type $var */`
    //
    // Skip when the cursor is inside the RHS of an assignment whose
    // LHS is the same variable.  The `@var` cast applies only
    // *after* the assignment completes, so hovering over `$data` in
    // `/** @var T */ $data = $data->toArray()` should show the
    // previous type, not the cast.
    if let Some(var_type) =
        docblock::find_var_raw_type_in_source(content, cursor_offset as usize, var_name)
        && !is_cursor_in_self_assignment_rhs(content, cursor_offset as usize, var_name)
    {
        return Some(var_type);
    }

    // 2–4. AST-based: parameter, foreach, catch
    let closure_ctx = HoverClosureCtx {
        current_class,
        all_classes,
        class_loader,
        content,
    };
    let ast_result: Option<PhpType> =
        with_parsed_program(content, "resolve_variable_type", |program, _| {
            find_variable_type_in_statements(
                program.statements.iter(),
                var_name,
                content,
                cursor_offset,
                &closure_ctx,
            )
        });

    // 4b. Foreach binding via class resolution.
    //     When the AST path above found no inline docblock annotation for
    //     the iterated expression, try resolving the foreach expression to
    //     a class and extract the iterable element type from its
    //     `@implements` / `@extends` generics.  This handles cases like
    //     `@implements IteratorAggregate<array<string, Foo|Bar>>`.
    if ast_result.is_none() {
        let dummy_class;
        let effective_class = match current_class {
            Some(cc) => cc,
            None => {
                dummy_class = ClassInfo::default();
                &dummy_class
            }
        };
        let foreach_result: Option<PhpType> =
            with_parsed_program(content, "resolve_foreach_via_class", |program, _| {
                resolve_foreach_type_via_class(
                    &program.statements.iter().collect::<Vec<_>>(),
                    var_name,
                    content,
                    cursor_offset,
                    effective_class,
                    all_classes,
                    class_loader,
                )
            });
        if foreach_result.is_some() {
            return foreach_result;
        }
    }

    // 5. Assignment type via unified pipeline (branch-aware) — tried
    //    before falling back to the parameter type from step 2–4 so
    //    that reassignments like
    //    `$markets = $markets ?: Country::getActiveCountries();`
    //    resolve to the richer assignment type (`array<int, Country>`)
    //    instead of the bare parameter type (`array`).
    let dummy_class;
    let effective_class = match current_class {
        Some(cc) => cc,
        None => {
            dummy_class = ClassInfo::default();
            &dummy_class
        }
    };
    let resolved = resolve_variable_types_branch_aware(
        var_name,
        effective_class,
        all_classes,
        content,
        cursor_offset,
        class_loader,
        loaders,
    );
    if !resolved.is_empty() {
        let joined = ResolvedType::types_joined(&resolved);
        // When the AST-based result (step 2–4) carries richer type
        // information than the unified pipeline (e.g. the docblock
        // says `Generator<int, Pencil>` but the pipeline only
        // resolved the bare class name `Generator`), prefer the
        // AST result.  Skip when the only difference is a trailing
        // `|null` — the unified pipeline may have stripped it via
        // guard clause narrowing.
        if let Some(ref ast) = ast_result
            && !ast.equivalent(&joined)
            && ast.has_type_structure()
        {
            // Check that the extra detail is not just `|null`.
            // If the AST type without null is equivalent to the
            // joined type, the only difference is a trailing null
            // component — prefer the narrowed `joined` result.
            let only_null_diff = ast.non_null_type().is_some_and(|nn| nn.equivalent(&joined));
            if !only_null_diff {
                return ast_result;
            }
        }
        return Some(joined);
    }

    // Fall back to the AST-based parameter/foreach/catch type.
    ast_result
}

/// Walk top-level statements to find the scope containing the cursor
/// and extract the variable's type string from its definition site.
fn find_variable_type_in_statements<'a, I>(
    statements: I,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType>
where
    I: Iterator<Item = &'a Statement<'a>>,
{
    let stmts: Vec<&Statement> = statements.collect();

    for &stmt in &stmts {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        class.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    );
                }
            }
            Statement::Interface(iface) => {
                let start = iface.left_brace.start.offset;
                let end = iface.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        iface.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    );
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        trait_def.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    );
                }
            }
            Statement::Enum(enum_def) => {
                let start = enum_def.left_brace.start.offset;
                let end = enum_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        enum_def.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    );
                }
            }
            Statement::Namespace(ns) => {
                if let Some(t) = find_variable_type_in_statements(
                    ns.statements().iter(),
                    var_name,
                    content,
                    cursor_offset,
                    closure_ctx,
                ) {
                    return Some(t);
                }
            }
            Statement::Function(func) => {
                let param_span = func.parameter_list.span();
                if cursor_offset >= param_span.start.offset
                    && cursor_offset <= param_span.end.offset
                {
                    return find_type_in_params(
                        &func.parameter_list,
                        var_name,
                        content,
                        cursor_offset,
                        func.span().start.offset as usize,
                    );
                }
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    // Check body constructs (foreach, catch, closures)
                    if let Some(t) = find_type_in_body_stmts(
                        &func.body.statements.iter().collect::<Vec<_>>(),
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    ) {
                        return Some(t);
                    }
                    // Fall back to parameter type
                    return find_type_in_params(
                        &func.parameter_list,
                        var_name,
                        content,
                        cursor_offset,
                        func.span().start.offset as usize,
                    );
                }
            }
            Statement::Try(_) => {
                let stmt_span = stmt.span();
                if cursor_offset >= stmt_span.start.offset
                    && cursor_offset <= stmt_span.end.offset
                    && let Some(t) = find_type_in_catch(stmt, var_name, cursor_offset)
                {
                    return Some(t);
                }
            }
            Statement::Foreach(foreach) => {
                let stmt_span = stmt.span();
                if cursor_offset >= stmt_span.start.offset
                    && cursor_offset <= stmt_span.end.offset
                    && let Some(t) =
                        find_type_in_foreach(foreach, var_name, content, cursor_offset, closure_ctx)
                {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}

/// Search class members for a method containing the cursor, then
/// extract the variable's type string.
fn find_type_in_class_members<'a, I>(
    members: I,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType>
where
    I: Iterator<Item = &'a ClassLikeMember<'a>>,
{
    for member in members {
        if let ClassLikeMember::Method(method) = member {
            // Check parameter list span
            let param_span = method.parameter_list.span();
            if cursor_offset >= param_span.start.offset && cursor_offset <= param_span.end.offset {
                return find_type_in_params(
                    &method.parameter_list,
                    var_name,
                    content,
                    cursor_offset,
                    method.span().start.offset as usize,
                );
            }

            if let MethodBody::Concrete(body) = &method.body {
                let body_start = body.left_brace.start.offset;
                let body_end = body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    // Check body constructs first
                    if let Some(t) = find_type_in_body_stmts(
                        &body.statements.iter().collect::<Vec<_>>(),
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    ) {
                        return Some(t);
                    }
                    // Fall back to parameter type
                    return find_type_in_params(
                        &method.parameter_list,
                        var_name,
                        content,
                        cursor_offset,
                        method.span().start.offset as usize,
                    );
                }
            }
        }
    }
    None
}

/// Extract the effective type for a parameter matching `var_name`.
///
/// Uses the native type hint and `@param` docblock type, preferring the
/// docblock when it is more specific (via `resolve_effective_type`).
fn find_type_in_params(
    parameter_list: &FunctionLikeParameterList<'_>,
    var_name: &str,
    content: &str,
    _cursor_offset: u32,
    method_start_offset: usize,
) -> Option<PhpType> {
    for param in parameter_list.parameters.iter() {
        let pname = param.variable.name.to_string();
        if pname != var_name {
            continue;
        }

        let native_parsed = param.hint.as_ref().map(|h| extract_hint_type(h));

        // Try @param docblock type
        let docblock_type =
            docblock::find_iterable_raw_type_in_source(content, method_start_offset, var_name)
                .or_else(|| {
                    // find_iterable_raw_type_in_source looks for @var/@param
                    // with the variable name. Try extract_param_raw_type on
                    // the raw docblock text as well.
                    find_method_docblock(content, method_start_offset)
                        .and_then(|doc| docblock::extract_param_raw_type(&doc, &pname))
                });

        let effective =
            docblock::resolve_effective_type_typed(native_parsed.as_ref(), docblock_type.as_ref());

        if effective.is_some() {
            return effective;
        }

        // Return native type if no effective type
        return native_parsed;
    }
    None
}

/// Walk body statements looking for foreach/catch/closure types.
fn find_type_in_body_stmts(
    stmts: &[&Statement<'_>],
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType> {
    for &stmt in stmts {
        let stmt_span = stmt.span();
        if cursor_offset < stmt_span.start.offset || cursor_offset > stmt_span.end.offset {
            continue;
        }

        // Catch variable type hints
        if let Some(t) = find_type_in_catch(stmt, var_name, cursor_offset) {
            return Some(t);
        }

        // Foreach bindings
        if let Statement::Foreach(foreach) = stmt
            && let Some(t) =
                find_type_in_foreach(foreach, var_name, content, cursor_offset, closure_ctx)
        {
            return Some(t);
        }

        // Closure/arrow function parameters
        if let Some(t) =
            find_type_in_closure_stmt(stmt, var_name, content, cursor_offset, closure_ctx)
        {
            return Some(t);
        }
    }
    None
}

/// Extract the catch clause type hint for a matching variable.
fn find_type_in_catch(stmt: &Statement<'_>, var_name: &str, cursor_offset: u32) -> Option<PhpType> {
    match stmt {
        Statement::Try(try_stmt) => {
            for catch in try_stmt.catch_clauses.iter() {
                if let Some(ref var) = catch.variable
                    && var.name == var_name
                {
                    let var_start = var.span.start.offset;
                    let var_end = var.span.end.offset;
                    if cursor_offset >= var_start && cursor_offset < var_end {
                        return Some(extract_hint_type(&catch.hint));
                    }
                }
                // Recurse into catch block
                let catch_span = catch.block.span();
                if cursor_offset >= catch_span.start.offset
                    && cursor_offset <= catch_span.end.offset
                {
                    for inner in catch.block.statements.iter() {
                        if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                            return Some(t);
                        }
                    }
                }
            }
            // Recurse into try block
            let try_span = try_stmt.block.span();
            if cursor_offset >= try_span.start.offset && cursor_offset <= try_span.end.offset {
                for inner in try_stmt.block.statements.iter() {
                    if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                        return Some(t);
                    }
                }
            }
            // Recurse into finally block
            if let Some(ref finally) = try_stmt.finally_clause {
                let finally_span = finally.block.span();
                if cursor_offset >= finally_span.start.offset
                    && cursor_offset <= finally_span.end.offset
                {
                    for inner in finally.block.statements.iter() {
                        if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                            return Some(t);
                        }
                    }
                }
            }
            None
        }
        Statement::If(if_stmt) => {
            for inner in if_stmt.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::Foreach(foreach) => {
            for inner in foreach.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::While(while_stmt) => {
            for inner in while_stmt.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::DoWhile(do_while) => {
            find_type_in_catch(do_while.statement, var_name, cursor_offset)
        }
        Statement::For(for_stmt) => {
            for inner in for_stmt.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::Switch(switch) => {
            for case in switch.body.cases() {
                for inner in case.statements().iter() {
                    if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                        return Some(t);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the type of a foreach value/key binding from the iterable's
/// docblock annotation.
///
/// For `foreach ($pencils as $pencil)` where `$pencils` has
/// `@param \Generator<int, Pencil>`, this returns `Pencil`.
fn find_type_in_foreach(
    foreach: &Foreach<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType> {
    // Check if the cursor is on the foreach value or key variable, or inside the body
    let foreach_start = foreach.foreach.span().start.offset;
    let body_end = foreach.body.span().end.offset;
    if cursor_offset < foreach_start || cursor_offset > body_end {
        return None;
    }

    // Determine if this foreach's value or key variable matches var_name
    let is_value_var = match &foreach.target {
        ForeachTarget::Value(val) => {
            if let Expression::Variable(Variable::Direct(dv)) = val.value {
                dv.name == var_name
            } else {
                false
            }
        }
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.value {
                dv.name == var_name
            } else {
                false
            }
        }
    };

    let is_key_var = match &foreach.target {
        ForeachTarget::Value(_) => false,
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.key {
                dv.name == var_name
            } else {
                false
            }
        }
    };

    if !is_value_var && !is_key_var {
        // Recurse into body for nested foreach/catch
        match &foreach.body {
            ForeachBody::Statement(inner) => {
                return find_type_in_body_stmts(
                    &[inner],
                    var_name,
                    content,
                    cursor_offset,
                    closure_ctx,
                );
            }
            ForeachBody::ColonDelimited(body) => {
                let stmts: Vec<&Statement> = body.statements.iter().collect();
                return find_type_in_body_stmts(
                    &stmts,
                    var_name,
                    content,
                    cursor_offset,
                    closure_ctx,
                );
            }
        }
    }

    // Get the iterable expression's type from docblock annotations
    let foreach_offset = foreach.foreach.span().start.offset as usize;
    let iterable_type = extract_foreach_expression_type(foreach, content, foreach_offset);

    if let Some(ref it) = iterable_type {
        if is_value_var {
            // Extract value type: `list<User>` → `User`,
            // `Generator<int, Pencil>` → `Pencil`,
            // `User[]` → `User`
            if let Some(element_type) = it.extract_value_type(true) {
                return Some(element_type.clone());
            }
        } else if is_key_var && let Some(key_type) = it.extract_key_type(true) {
            return Some(key_type.clone());
        }
    }

    None
}

/// Extract the iterable type from the foreach expression's surrounding
/// docblock annotations, returning the already-parsed `PhpType` directly.
fn extract_foreach_expression_type(
    foreach: &Foreach<'_>,
    content: &str,
    foreach_offset: usize,
) -> Option<PhpType> {
    let expr_span = foreach.expression.span();
    let expr_start = expr_span.start.offset as usize;
    let expr_end = expr_span.end.offset as usize;
    let expr_text = content.get(expr_start..expr_end)?.trim();

    if !expr_text.starts_with('$') || expr_text.contains("->") || expr_text.contains("::") {
        return None;
    }

    docblock::find_iterable_raw_type_in_source(content, foreach_offset, expr_text)
}

// ─── Class-resolution-based foreach type extraction ─────────────────────────

/// Known iterable interface short names whose generic parameters carry
/// the element (value) type.
const ITERABLE_IFACE_NAMES: &[&str] = &[
    "Iterator",
    "IteratorAggregate",
    "Traversable",
    "ArrayAccess",
    "Enumerable",
];

/// Resolve a foreach binding variable's type string by resolving the
/// iterated expression to a class and inspecting its `@implements` /
/// `@extends` generic annotations.
///
/// This is a fallback for the AST-only path (`find_type_in_foreach`)
/// which relies on inline docblock annotations.  When no annotation
/// is present, this function uses the class loader to resolve the
/// iterated expression's type and extract the element type from
/// `implements_generics` or `extends_generics`.
fn resolve_foreach_type_via_class(
    stmts: &[&Statement<'_>],
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    for stmt in stmts {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return resolve_foreach_in_members(
                        class.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                        current_class,
                        all_classes,
                        class_loader,
                    );
                }
            }
            Statement::Function(func) => {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    let body_stmts: Vec<&Statement> = func.body.statements.iter().collect();
                    return resolve_foreach_in_body(
                        &body_stmts,
                        var_name,
                        content,
                        cursor_offset,
                        current_class,
                        all_classes,
                        class_loader,
                    );
                }
            }
            Statement::Foreach(foreach) => {
                let stmt_span = stmt.span();
                if cursor_offset >= stmt_span.start.offset && cursor_offset <= stmt_span.end.offset
                {
                    return resolve_foreach_binding_via_class(
                        foreach,
                        var_name,
                        content,
                        cursor_offset,
                        current_class,
                        all_classes,
                        class_loader,
                    );
                }
            }
            _ => {}
        }
    }
    None
}

/// Walk class members looking for foreach bindings resolvable via class
/// loader.
fn resolve_foreach_in_members<'a>(
    members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    for member in members {
        if let ClassLikeMember::Method(method) = member
            && let MethodBody::Concrete(block) = &method.body
        {
            let blk_start = block.left_brace.start.offset;
            let blk_end = block.right_brace.end.offset;
            if cursor_offset >= blk_start && cursor_offset <= blk_end {
                let body_stmts: Vec<&Statement> = block.statements.iter().collect();
                return resolve_foreach_in_body(
                    &body_stmts,
                    var_name,
                    content,
                    cursor_offset,
                    current_class,
                    all_classes,
                    class_loader,
                );
            }
        }
    }
    None
}

/// Walk body statements looking for foreach bindings resolvable via class
/// loader.
fn resolve_foreach_in_body(
    stmts: &[&Statement<'_>],
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    for stmt in stmts {
        if let Statement::Foreach(foreach) = stmt {
            let stmt_span = stmt.span();
            if cursor_offset >= stmt_span.start.offset && cursor_offset <= stmt_span.end.offset {
                return resolve_foreach_binding_via_class(
                    foreach,
                    var_name,
                    content,
                    cursor_offset,
                    current_class,
                    all_classes,
                    class_loader,
                );
            }
        }
        // Recurse into blocks (if, while, for, etc.)
        if let Some(inner_stmts) = extract_body_statements(stmt) {
            let inner: Vec<&Statement> = inner_stmts.collect();
            if let Some(result) = resolve_foreach_in_body(
                &inner,
                var_name,
                content,
                cursor_offset,
                current_class,
                all_classes,
                class_loader,
            ) {
                return Some(result);
            }
        }
    }
    None
}

/// Extract body statements from control-flow constructs for recursion.
fn extract_body_statements<'a, 'b>(
    stmt: &'b Statement<'a>,
) -> Option<Box<dyn Iterator<Item = &'b Statement<'a>> + 'b>> {
    match stmt {
        Statement::If(if_stmt) => {
            let stmts: Vec<&Statement> = if_stmt.body.statements().iter().collect();
            Some(Box::new(stmts.into_iter()))
        }
        Statement::While(w) => Some(Box::new(w.body.statements().iter())),
        Statement::For(f) => Some(Box::new(f.body.statements().iter())),
        Statement::Block(b) => Some(Box::new(b.statements.iter())),
        _ => None,
    }
}

/// Resolve a single foreach binding by resolving the iterated expression
/// to a class and checking `implements_generics` / `extends_generics`.
fn resolve_foreach_binding_via_class(
    foreach: &Foreach<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    let foreach_start = foreach.foreach.span().start.offset;
    let body_end = foreach.body.span().end.offset;
    if cursor_offset < foreach_start || cursor_offset > body_end {
        return None;
    }

    // Check if the cursor variable matches the foreach value or key.
    let is_value_var = match &foreach.target {
        ForeachTarget::Value(val) => {
            if let Expression::Variable(Variable::Direct(dv)) = val.value {
                dv.name == var_name
            } else {
                false
            }
        }
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.value {
                dv.name == var_name
            } else {
                false
            }
        }
    };

    let is_key_var = match &foreach.target {
        ForeachTarget::Value(_) => false,
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.key {
                dv.name == var_name
            } else {
                false
            }
        }
    };

    if !is_value_var && !is_key_var {
        // Recurse into body for nested foreach
        let body_stmts: Vec<&Statement> = foreach.body.statements().iter().collect();
        return resolve_foreach_in_body(
            &body_stmts,
            var_name,
            content,
            cursor_offset,
            current_class,
            all_classes,
            class_loader,
        );
    }

    // Resolve the iterated expression to classes.
    let expr_span = foreach.expression.span();
    let expr_start = expr_span.start.offset as usize;
    let expr_end = expr_span.end.offset as usize;
    let expr_text = content.get(expr_start..expr_end)?.trim();

    // Resolve the expression to ClassInfo values.
    let iterable_classes = resolve_expression_to_classes(
        expr_text,
        content,
        cursor_offset,
        current_class,
        all_classes,
        class_loader,
    );

    for cls in &iterable_classes {
        let merged = crate::virtual_members::resolve_class_fully(cls, class_loader);
        if is_value_var {
            if let Some(value_type) =
                extract_iterable_type_from_merged(&merged, class_loader, false)
            {
                return Some(value_type);
            }
        } else if is_key_var
            && let Some(key_type) = extract_iterable_type_from_merged(&merged, class_loader, true)
        {
            return Some(key_type);
        }
    }

    None
}

/// Resolve a simple expression text to `ClassInfo` values.
///
/// Handles `$var` (via `@var` docblock or assignment scanning) and
/// `new ClassName()` patterns.
fn resolve_expression_to_classes(
    expr_text: &str,
    content: &str,
    cursor_offset: u32,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<ClassInfo> {
    // Simple variable: try to resolve its type.
    if expr_text.starts_with('$') && !expr_text.contains("->") && !expr_text.contains("::") {
        // Try @var annotation.
        if let Some(raw_type) =
            docblock::find_var_raw_type_in_source(content, cursor_offset as usize, expr_text)
        {
            return crate::completion::type_resolution::type_hint_to_classes_typed(
                &raw_type,
                &current_class.name,
                all_classes,
                class_loader,
            );
        }
        // Try variable resolution.
        let types = ResolvedType::into_classes(
            crate::completion::variable::resolution::resolve_variable_types(
                expr_text,
                current_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                Loaders::default(),
            ),
        );
        if !types.is_empty() {
            return types;
        }
    }

    vec![]
}

/// Extract the iterable value or key type from a merged class's
/// `implements_generics` and `extends_generics`, also checking
/// transitively through interfaces.
fn extract_iterable_type_from_merged(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    is_key: bool,
) -> Option<PhpType> {
    // 1. Direct check on implements_generics.
    for (name, args) in &class.implements_generics {
        let s = short_name(name);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            if is_key && args.len() >= 2 {
                return Some(args[0].clone());
            } else if !is_key && !args.is_empty() {
                return Some(args.last().unwrap().clone());
            }
        }
    }

    // 2. Check implements_generics for non-iterable interfaces that
    //    themselves extend a known iterable interface (transitive check).
    for (name, args) in &class.implements_generics {
        let s = short_name(name);
        if !ITERABLE_IFACE_NAMES.contains(&s)
            && let Some(iface) = class_loader(name)
            && is_transitive_iterable(&iface, class_loader)
            && !args.is_empty()
        {
            if is_key && args.len() >= 2 {
                return Some(args[0].clone());
            } else if !is_key {
                return Some(args.last().unwrap().clone());
            }
        }
    }

    // 3. Check extends_generics — but only when the parent class is
    //    itself a known iterable or transitively extends one.
    //    `@extends Test1<int>` does NOT describe iteration types; only
    //    `@extends Collection<int, User>` (where Collection is iterable)
    //    does.
    for (name, args) in &class.extends_generics {
        let s = short_name(name);
        let parent_is_iterable = ITERABLE_IFACE_NAMES.contains(&s)
            || class_loader(name)
                .map(|p| is_transitive_iterable(&p, class_loader))
                .unwrap_or(false);
        if parent_is_iterable {
            if is_key && args.len() >= 2 {
                return Some(args[0].clone());
            } else if !is_key && !args.is_empty() {
                return Some(args.last().unwrap().clone());
            }
        }
    }

    None
}

/// Check whether an interface transitively extends a known iterable
/// interface (e.g. `TypedCollection extends IteratorAggregate`).
fn is_transitive_iterable(
    iface: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    is_transitive_iterable_depth(iface, class_loader, 0)
}

/// Recursive helper with a depth guard to prevent infinite loops
/// from circular interface hierarchies.
fn is_transitive_iterable_depth(
    iface: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    depth: u32,
) -> bool {
    if depth > 10 {
        return false;
    }
    // Check direct interfaces.
    for parent in &iface.interfaces {
        let s = short_name(parent);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            return true;
        }
        // Recurse into non-iterable interfaces to check if they
        // transitively extend an iterable (e.g. ReflectionCollection
        // extends IteratorAggregate).
        if let Some(parent_info) = class_loader(parent)
            && is_transitive_iterable_depth(&parent_info, class_loader, depth + 1)
        {
            return true;
        }
    }
    // Check extends_generics for the interface-extends-interface pattern.
    for (name, _) in &iface.extends_generics {
        let s = short_name(name);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            return true;
        }
        if let Some(parent_info) = class_loader(name)
            && is_transitive_iterable_depth(&parent_info, class_loader, depth + 1)
        {
            return true;
        }
    }
    // Check parent class (interfaces use `parent_class` for extends).
    if let Some(ref parent_name) = iface.parent_class {
        let s = short_name(parent_name);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            return true;
        }
        if let Some(parent) = class_loader(parent_name) {
            return is_transitive_iterable_depth(&parent, class_loader, depth + 1);
        }
    }
    false
}

/// Recursively search a statement for closure/arrow function parameters.
fn find_type_in_closure_stmt(
    stmt: &Statement<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType> {
    match stmt {
        Statement::Expression(expr_stmt) => find_type_in_closure_expr(
            expr_stmt.expression,
            var_name,
            content,
            cursor_offset,
            closure_ctx,
        ),
        Statement::Return(ret) => ret.value.and_then(|expr| {
            find_type_in_closure_expr(expr, var_name, content, cursor_offset, closure_ctx)
        }),
        Statement::If(if_stmt) => {
            for inner in if_stmt.body.statements() {
                if let Some(t) =
                    find_type_in_closure_stmt(inner, var_name, content, cursor_offset, closure_ctx)
                {
                    return Some(t);
                }
            }
            None
        }
        Statement::Foreach(foreach) => {
            for inner in foreach.body.statements() {
                if let Some(t) =
                    find_type_in_closure_stmt(inner, var_name, content, cursor_offset, closure_ctx)
                {
                    return Some(t);
                }
            }
            None
        }
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(t) =
                    find_type_in_closure_stmt(inner, var_name, content, cursor_offset, closure_ctx)
                {
                    return Some(t);
                }
            }
            None
        }
        Statement::Try(try_stmt) => {
            for inner in try_stmt.block.statements.iter() {
                if let Some(t) =
                    find_type_in_closure_stmt(inner, var_name, content, cursor_offset, closure_ctx)
                {
                    return Some(t);
                }
            }
            for catch in try_stmt.catch_clauses.iter() {
                for inner in catch.block.statements.iter() {
                    if let Some(t) = find_type_in_closure_stmt(
                        inner,
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    ) {
                        return Some(t);
                    }
                }
            }
            if let Some(ref finally) = try_stmt.finally_clause {
                for inner in finally.block.statements.iter() {
                    if let Some(t) = find_type_in_closure_stmt(
                        inner,
                        var_name,
                        content,
                        cursor_offset,
                        closure_ctx,
                    ) {
                        return Some(t);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Check if an expression contains a closure/arrow function whose
/// parameter matches the target variable.
fn find_type_in_closure_expr(
    expr: &Expression<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType> {
    match expr {
        Expression::Closure(closure) => {
            let closure_span = closure.span();
            if cursor_offset >= closure_span.start.offset
                && cursor_offset <= closure_span.end.offset
            {
                // Check closure parameters
                let result = find_type_in_params(
                    &closure.parameter_list,
                    var_name,
                    content,
                    cursor_offset,
                    closure_span.start.offset as usize,
                );
                if result.is_some() {
                    return result;
                }
                // Recurse into body
                let stmts: Vec<&Statement> = closure.body.statements.iter().collect();
                return find_type_in_body_stmts(
                    &stmts,
                    var_name,
                    content,
                    cursor_offset,
                    closure_ctx,
                );
            }
            None
        }
        Expression::ArrowFunction(arrow) => {
            let arrow_span = arrow.span();
            if cursor_offset >= arrow_span.start.offset && cursor_offset <= arrow_span.end.offset {
                return find_type_in_params(
                    &arrow.parameter_list,
                    var_name,
                    content,
                    cursor_offset,
                    arrow_span.start.offset as usize,
                );
            }
            None
        }
        Expression::Call(call) => {
            find_type_in_closure_call(call, var_name, content, cursor_offset, closure_ctx)
        }
        Expression::Parenthesized(paren) => find_type_in_closure_expr(
            paren.expression,
            var_name,
            content,
            cursor_offset,
            closure_ctx,
        ),
        Expression::Assignment(assign) => {
            find_type_in_closure_expr(assign.rhs, var_name, content, cursor_offset, closure_ctx)
        }
        _ => None,
    }
}

/// Resolve a closure parameter type inside a call expression, applying
/// callable parameter inference when the inferred type is a subclass of
/// the explicit type hint.
///
/// For example, `$collection->each(function (Model $item) { $item-> })`
/// where the collection is `Collection<int, BrandTranslation>` and
/// `BrandTranslation extends Model` — the inferred `BrandTranslation`
/// type is preferred over the explicit `Model` hint.
fn find_type_in_closure_call(
    call: &Call<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType> {
    let arg_list = call.get_argument_list();

    for (arg_idx, arg) in arg_list.arguments.iter().enumerate() {
        let arg_expr: &Expression<'_> = arg.value();
        let arg_span = arg_expr.span();
        if cursor_offset < arg_span.start.offset || cursor_offset > arg_span.end.offset {
            continue;
        }

        // Check if this argument is a closure/arrow-function whose
        // parameter list contains our target variable.
        let (param_list, body_start, body_end) = match arg_expr {
            Expression::Closure(closure) => {
                let bs = closure.body.left_brace.start.offset;
                let be = closure.body.right_brace.end.offset;
                (Some(&closure.parameter_list), bs, be)
            }
            Expression::ArrowFunction(arrow) => {
                let arrow_body_span = arrow.expression.span();
                (
                    Some(&arrow.parameter_list),
                    arrow.arrow.start.offset,
                    arrow_body_span.end.offset,
                )
            }
            _ => (None, 0, 0),
        };

        if let Some(params) = param_list
            && cursor_offset >= body_start
            && cursor_offset <= body_end
        {
            // Find which parameter index matches var_name.
            let param_hit = params
                .parameters
                .iter()
                .enumerate()
                .find(|(_, p)| *p.variable.name == *var_name);

            if let Some((param_idx, param)) = param_hit {
                // Get the explicit type hint as a PhpType directly
                // (no stringify-then-reparse roundtrip).
                let explicit_type = param.hint.as_ref().map(|h| extract_hint_type(h));

                // If there's an explicit type, check whether the
                // inferred callable parameter type is more specific.
                if let Some(ref et) = explicit_type
                    && let Some(inferred) = try_infer_more_specific_type_from_call(
                        call,
                        arg_idx,
                        param_idx,
                        et,
                        closure_ctx,
                    )
                {
                    // For closures, recurse into the body for
                    // nested constructs before returning.
                    if let Expression::Closure(closure) = arg_expr {
                        let stmts: Vec<&Statement> = closure.body.statements.iter().collect();
                        if let Some(inner) = find_type_in_body_stmts(
                            &stmts,
                            var_name,
                            content,
                            cursor_offset,
                            closure_ctx,
                        ) {
                            return Some(inner);
                        }
                    }
                    return Some(inferred);
                }
            }
        }

        // Fall back to the regular closure/arrow-function parameter
        // resolution (handles no-hint params, body recursion, etc.).
        if let Some(t) =
            find_type_in_closure_expr(arg_expr, var_name, content, cursor_offset, closure_ctx)
        {
            return Some(t);
        }
    }

    None
}

/// Try to infer callable parameter types from the enclosing call
/// expression and check whether the inferred type is a subclass of
/// `explicit_type`.  Returns the inferred type when it is more
/// specific, or `None` otherwise.
fn try_infer_more_specific_type_from_call(
    call: &Call<'_>,
    arg_idx: usize,
    param_idx: usize,
    explicit_type: &PhpType,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<PhpType> {
    let inferred_types = infer_callable_param_types_for_call(call, arg_idx, closure_ctx)?;
    let inferred = inferred_types.get(param_idx)?;

    // Resolve both the explicit and inferred types to ClassInfo to
    // check the inheritance relationship.
    let dummy = ClassInfo::default();
    let current_name = closure_ctx
        .current_class
        .map(|c| c.name.as_str())
        .unwrap_or(&dummy.name);

    let explicit_classes = crate::completion::type_resolution::type_hint_to_classes_typed(
        explicit_type,
        current_name,
        closure_ctx.all_classes,
        closure_ctx.class_loader,
    );
    if explicit_classes.is_empty() {
        return None;
    }

    let inferred_classes = crate::completion::type_resolution::type_hint_to_classes_typed(
        inferred,
        current_name,
        closure_ctx.all_classes,
        closure_ctx.class_loader,
    );
    if inferred_classes.is_empty() {
        return None;
    }

    // The inferred type is more specific when every inferred class is a
    // subtype of at least one explicit class.
    let all_subtypes = inferred_classes.iter().all(|inferred_cls| {
        explicit_classes.iter().any(|explicit_cls| {
            crate::util::is_subtype_of_names(
                &inferred_cls.fqn(),
                &explicit_cls.fqn(),
                closure_ctx.class_loader,
            )
        })
    });

    // Also ensure the inferred type is actually different (not the same
    // class), otherwise there's no benefit.
    let is_same = inferred_classes.len() == explicit_classes.len()
        && inferred_classes
            .iter()
            .zip(explicit_classes.iter())
            .all(|(a, b)| a.name == b.name);

    if all_subtypes && !is_same {
        Some(inferred.clone())
    } else {
        None
    }
}

/// Extract callable parameter types from the method/function being called
/// at the given argument index.
fn infer_callable_param_types_for_call(
    call: &Call<'_>,
    arg_idx: usize,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<Vec<PhpType>> {
    match call {
        Call::Method(mc) => {
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let method_name = ident.value.to_string();
                let obj_span = mc.object.span();
                let start = obj_span.start.offset as usize;
                let end = obj_span.end.offset as usize;
                if end > closure_ctx.content.len() {
                    return None;
                }
                let obj_text = closure_ctx.content[start..end].trim();

                let rctx = crate::completion::resolver::ResolutionCtx {
                    current_class: closure_ctx.current_class,
                    all_classes: closure_ctx.all_classes,
                    content: closure_ctx.content,
                    // Use the receiver's end offset so variable
                    // assignments preceding the call are visible.
                    cursor_offset: obj_span.end.offset,
                    class_loader: closure_ctx.class_loader,
                    function_loader: None,
                    scope_var_resolver: None,
                    resolved_class_cache: None,
                };
                let receiver_classes = ResolvedType::into_arced_classes(
                    crate::completion::resolver::resolve_target_classes(
                        obj_text,
                        AccessKind::Arrow,
                        &rctx,
                    ),
                );
                find_callable_params_on_receiver_classes(
                    &receiver_classes,
                    &method_name,
                    arg_idx,
                    closure_ctx,
                )
            } else {
                None
            }
        }
        Call::NullSafeMethod(mc) => {
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let method_name = ident.value.to_string();
                let obj_span = mc.object.span();
                let start = obj_span.start.offset as usize;
                let end = obj_span.end.offset as usize;
                if end > closure_ctx.content.len() {
                    return None;
                }
                let obj_text = closure_ctx.content[start..end].trim();

                let rctx = crate::completion::resolver::ResolutionCtx {
                    current_class: closure_ctx.current_class,
                    all_classes: closure_ctx.all_classes,
                    content: closure_ctx.content,
                    cursor_offset: obj_span.end.offset,
                    class_loader: closure_ctx.class_loader,
                    function_loader: None,
                    scope_var_resolver: None,
                    resolved_class_cache: None,
                };
                let receiver_classes = ResolvedType::into_arced_classes(
                    crate::completion::resolver::resolve_target_classes(
                        obj_text,
                        AccessKind::Arrow,
                        &rctx,
                    ),
                );
                find_callable_params_on_receiver_classes(
                    &receiver_classes,
                    &method_name,
                    arg_idx,
                    closure_ctx,
                )
            } else {
                None
            }
        }
        Call::StaticMethod(sc) => {
            if let ClassLikeMemberSelector::Identifier(ident) = &sc.method {
                let method_name = ident.value.to_string();
                let class_name = match sc.class {
                    Expression::Self_(_) | Expression::Static(_) => {
                        closure_ctx.current_class.map(|c| c.name.clone())
                    }
                    Expression::Identifier(id) => Some(id.value().to_string()),
                    Expression::Parent(_) => closure_ctx
                        .current_class
                        .and_then(|c| c.parent_class.clone()),
                    _ => None,
                };
                let cls = class_name.and_then(|name| {
                    closure_ctx
                        .all_classes
                        .iter()
                        .find(|c| c.name == name)
                        .map(|c| ClassInfo::clone(c))
                        .or_else(|| (closure_ctx.class_loader)(&name).map(Arc::unwrap_or_clone))
                });
                if let Some(ref c) = cls {
                    let resolved =
                        crate::virtual_members::resolve_class_fully(c, closure_ctx.class_loader);
                    let receiver_fqn = c.fqn();
                    find_callable_params_on_method(&resolved, &method_name, arg_idx).map(|params| {
                        params
                            .into_iter()
                            .map(|ty| ty.replace_self(&receiver_fqn))
                            .collect()
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        Call::Function(fc) => {
            let func_name = match fc.function {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                _ => None,
            };
            if let Some(name) = func_name {
                // No function_loader available in this context; skip.
                let _ = name;
            }
            None
        }
    }
}

/// Look up the method on receiver classes and extract callable param types.
fn find_callable_params_on_receiver_classes(
    classes: &[Arc<ClassInfo>],
    method_name: &str,
    arg_idx: usize,
    closure_ctx: &HoverClosureCtx<'_>,
) -> Option<Vec<PhpType>> {
    for cls in classes {
        let resolved = crate::virtual_members::resolve_class_fully(cls, closure_ctx.class_loader);
        if let Some(params) = find_callable_params_on_method(&resolved, method_name, arg_idx) {
            // Replace `$this`/`static`/`self` with the receiver FQN.
            let receiver_fqn = cls.fqn();
            let result = params
                .into_iter()
                .map(|ty| ty.replace_self(&receiver_fqn))
                .collect();
            return Some(result);
        }
    }
    None
}

/// Extract callable parameter types from a method's parameter at `arg_idx`.
fn find_callable_params_on_method(
    class: &ClassInfo,
    method_name: &str,
    arg_idx: usize,
) -> Option<Vec<PhpType>> {
    let method = class.methods.iter().find(|m| m.name == method_name)?;
    let param = method.parameters.get(arg_idx)?;
    let hint = param.type_hint.as_ref()?;
    let callable_params = hint.callable_param_types()?;
    if callable_params.is_empty() {
        None
    } else {
        Some(
            callable_params
                .iter()
                .map(|cp| cp.type_hint.clone())
                .collect(),
        )
    }
}

/// Extract the raw docblock text for a method/function at the given
/// source offset by scanning backwards for `/** ... */`.
fn find_method_docblock(content: &str, method_start: usize) -> Option<String> {
    let before = content.get(..method_start)?;
    let trimmed = before.trim_end();
    if !trimmed.ends_with("*/") {
        return None;
    }
    let doc_end = trimmed.len();
    let doc_start = trimmed.rfind("/**")?;
    Some(trimmed[doc_start..doc_end].to_string())
}
