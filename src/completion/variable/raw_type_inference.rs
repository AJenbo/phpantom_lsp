/// Raw type inference for variable assignments.
///
/// This module resolves the raw type string of a variable's most recent
/// assignment by walking the AST.  It handles:
///
///   - Base assignments: `$var = expr;`
///   - Incremental key assignments: `$var['key'] = expr;`
///   - Push assignments: `$var[] = expr;`
///   - Array literals with element type inference
///   - Known array functions (array_filter, array_map, array_pop, etc.)
///   - Generator yield reverse-inference
///   - `assert($var instanceof ClassName)` narrowing
///   - Inline `/** @var Type */` docblock overrides on assignments
///
/// The primary entry point is [`resolve_variable_assignment_raw_type`],
/// which re-parses the file and returns a PHPStan-style type string
/// (e.g. `"list<User>"`, `"array{name: string, age: int}"`, `"ClassName"`).
///
/// All functions in this module are free functions (not methods on
/// `Backend`).  Cross-module dependencies use their canonical module paths.
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use super::{ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS};

use crate::docblock;
use crate::parser::{extract_hint_string, with_parsed_program};
use crate::types::ClassInfo;

use crate::completion::array_shape::build_list_type_from_push_types;
use crate::completion::resolver::{FunctionLoaderFn, VarResolutionCtx};

/// Accumulates base assignments, incremental key assignments, and push
/// assignments for a single variable while scanning statements.
///
/// When a new base assignment (`$var = expr;`) is found, previously
/// collected incremental/push entries are discarded.  Incremental key
/// assignments (`$var['key'] = expr;`) and push assignments
/// (`$var[] = expr;`) are merged into the base type at finalisation.
struct AssignmentAccumulator {
    /// Raw type string from the most recent `$var = expr;`.
    base_type: Option<String>,
    /// `(key, type)` pairs from `$var['key'] = expr;` after the base.
    incremental_keys: Vec<(String, String)>,
    /// Type strings from `$var[] = expr;` after the base.
    push_types: Vec<String>,
}

impl AssignmentAccumulator {
    fn new() -> Self {
        Self {
            base_type: None,
            incremental_keys: Vec::new(),
            push_types: Vec::new(),
        }
    }

    /// Record a new base assignment, clearing previous incremental/push
    /// entries since they preceded this assignment.
    fn set_base(&mut self, raw_type: String) {
        self.base_type = Some(raw_type);
        self.incremental_keys.clear();
        self.push_types.clear();
    }

    /// Record an incremental key assignment (`$var['key'] = expr;`).
    /// If the key already exists, the type is overridden.
    fn add_incremental_key(&mut self, key: String, value_type: String) {
        if let Some(existing) = self.incremental_keys.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = value_type;
        } else {
            self.incremental_keys.push((key, value_type));
        }
    }

    /// Record a push assignment (`$var[] = expr;`).
    fn add_push_type(&mut self, value_type: String) {
        self.push_types.push(value_type);
    }

    /// Merge another accumulator's results into this one.
    ///
    /// Used when recursing into block-like constructs (loops,
    /// try/catch).  The inner accumulator's base replaces ours if
    /// present; incremental/push entries are appended.
    fn merge(&mut self, other: AssignmentAccumulator) {
        if let Some(base) = other.base_type {
            self.set_base(base);
        }
        for (k, v) in other.incremental_keys {
            self.add_incremental_key(k, v);
        }
        self.push_types.extend(other.push_types);
    }

    /// Merge a branch accumulator by **unioning** its base type with
    /// ours instead of overwriting.
    ///
    /// Used for if/else and try/catch branches where the variable may
    /// be assigned a different type in each branch.  The result after
    /// the whole if/else is the union of all branch types (e.g.
    /// `Lamp|Faucet`).  Incremental keys and push types are still
    /// appended normally.
    fn merge_union(&mut self, other: AssignmentAccumulator) {
        if let Some(other_base) = other.base_type {
            match self.base_type.take() {
                Some(existing) => {
                    // Union the two base types, deduplicating components.
                    let mut parts: Vec<&str> = existing.split('|').collect();
                    for part in other_base.split('|') {
                        if !parts.contains(&part) {
                            parts.push(part);
                        }
                    }
                    // Reborrow issue: collect owned strings.
                    let joined = parts.join("|");
                    self.base_type = Some(joined);
                    // Do NOT clear incremental/push — they may have
                    // accumulated from earlier branches.
                }
                None => {
                    self.base_type = Some(other_base);
                }
            }
        }
        for (k, v) in other.incremental_keys {
            self.add_incremental_key(k, v);
        }
        self.push_types.extend(other.push_types);
    }

    /// Produce the final raw type string by merging base, incremental
    /// keys, and push types.
    ///
    /// Logic mirrors what the former `extract_raw_type_from_assignment_text`
    /// did in the now-removed `text_resolution.rs`:
    /// - Parse the base type as an array shape and merge incremental keys
    /// - If string-keyed entries exist, produce `array{…}`
    /// - Otherwise, if push types exist, produce `list<…>`
    /// - Otherwise, return the base type as-is
    fn finalize(self) -> Option<String> {
        if self.base_type.is_none()
            && self.incremental_keys.is_empty()
            && self.push_types.is_empty()
        {
            return None;
        }

        let has_modifications = !self.incremental_keys.is_empty() || !self.push_types.is_empty();
        if !has_modifications {
            return self.base_type;
        }

        // Start from the base type's shape entries (if any).
        let mut entries: Vec<(String, String)> = Vec::new();
        let mut positional_types: Vec<String> = Vec::new();

        if let Some(ref base) = self.base_type {
            // Try to parse as an array shape to extract existing entries.
            if let Some(parsed) = crate::docblock::parse_array_shape(base) {
                for entry in &parsed {
                    entries.push((entry.key.clone(), entry.value_type.clone()));
                }
            } else if let Some(elem) = crate::docblock::types::extract_generic_value_type(base) {
                // Base is `list<Type>` — seed positional types.
                positional_types.push(elem);
            }
        }

        // Merge incremental key assignments.
        for (k, v) in &self.incremental_keys {
            if let Some(existing) = entries.iter_mut().find(|(ek, _)| ek == k) {
                existing.1 = v.clone();
            } else {
                entries.push((k.clone(), v.clone()));
            }
        }

        // If there are string-keyed entries, prefer the array shape.
        if !entries.is_empty() {
            let shape_parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            return Some(format!("array{{{}}}", shape_parts.join(", ")));
        }

        // No string-keyed entries — try push-style list inference.
        let mut all_types = positional_types;
        all_types.extend(self.push_types.clone());
        if let Some(list_type) = build_list_type_from_push_types(&all_types) {
            return Some(list_type);
        }

        self.base_type
    }
}

/// Resolve a variable's raw type string by walking the AST.
///
/// Re-parses the file and searches for the most recent assignment to
/// `var_name` before `cursor_offset`.  When the RHS is an array
/// literal (`[new Foo(), new Bar()]`), infers the element types and
/// returns a `list<Foo|Bar>` string.  For call expressions and
/// property access, delegates to [`extract_rhs_iterable_raw_type`].
///
/// This is the AST-based variable assignment scanner, used as a
/// candidate source for
/// [`try_chained_array_access_with_candidates`](crate::completion::source::helpers)
/// when resolving array access chains.
pub(crate) fn resolve_variable_assignment_raw_type(
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: Option<&ClassInfo>,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoaderFn<'_>,
) -> Option<String> {
    let dummy_class;
    let effective_class = match current_class {
        Some(cc) => cc,
        None => {
            dummy_class = ClassInfo::default();
            &dummy_class
        }
    };

    with_parsed_program(
        content,
        "resolve_variable_assignment_raw_type",
        |program, _content| {
            let ctx = VarResolutionCtx {
                var_name,
                current_class: effective_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
                resolved_class_cache: None,
                enclosing_return_type: None,
            };

            find_variable_assignment_raw_type(program.statements.iter(), &ctx)
        },
    )
}

/// Walk statements to find the enclosing scope and the most recent
/// assignment to the target variable, returning its raw type string.
fn find_variable_assignment_raw_type<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    let stmts: Vec<&Statement> = statements.collect();

    // Check if the cursor is inside a class/function body and recurse.
    for &stmt in &stmts {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    return find_assignment_raw_type_in_members(class.members.iter(), ctx);
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    return find_assignment_raw_type_in_members(trait_def.members.iter(), ctx);
                }
            }
            Statement::Enum(enum_def) => {
                let start = enum_def.left_brace.start.offset;
                let end = enum_def.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    return find_assignment_raw_type_in_members(enum_def.members.iter(), ctx);
                }
            }
            Statement::Interface(iface) => {
                let start = iface.left_brace.start.offset;
                let end = iface.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    return find_assignment_raw_type_in_members(iface.members.iter(), ctx);
                }
            }
            Statement::Namespace(ns) => {
                if let Some(result) = find_variable_assignment_raw_type(ns.statements().iter(), ctx)
                {
                    return Some(result);
                }
            }
            Statement::Function(func) => {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                    // Check parameter type hints first — `function f(int $x)`
                    // means `$x` has raw type `int` even without an assignment.
                    for param in func.parameter_list.parameters.iter() {
                        if param.variable.name == ctx.var_name
                            && let Some(ref hint) = param.hint
                        {
                            return Some(crate::parser::extract_hint_string(hint));
                        }
                    }
                    return scan_statements_for_assignment_raw_type(
                        func.body.statements.iter(),
                        ctx,
                    );
                }
            }
            _ => {}
        }
    }

    // Top-level code — scan all statements.
    scan_statements_for_assignment_raw_type(stmts.into_iter(), ctx)
}

/// Scan class-like members for a method body that contains the cursor,
/// then search that method's statements for the variable assignment.
fn find_assignment_raw_type_in_members<'b>(
    members: impl Iterator<Item = &'b ClassLikeMember<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    for member in members {
        if let ClassLikeMember::Method(method) = member
            && let MethodBody::Concrete(block) = &method.body
        {
            let start = block.left_brace.start.offset;
            let end = block.right_brace.end.offset;
            if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                return scan_statements_for_assignment_raw_type(block.statements.iter(), ctx);
            }
        }
    }
    None
}

/// Walk statements linearly, tracking the most recent assignment to
/// the target variable, and return the raw type string of its RHS.
fn scan_statements_for_assignment_raw_type<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    let acc = accumulate_assignment_raw_types(statements, ctx);
    acc.finalize()
}

/// Walk statements linearly, accumulating base, incremental key, and
/// push assignments for the target variable.
fn accumulate_assignment_raw_types<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> AssignmentAccumulator {
    let mut acc = AssignmentAccumulator::new();

    for stmt in statements {
        if stmt.span().start.offset >= ctx.cursor_offset {
            break;
        }

        // Recurse into block-like constructs.
        match stmt {
            Statement::Block(block) => {
                let inner = accumulate_assignment_raw_types(block.statements.iter(), ctx);
                acc.merge(inner);
            }
            Statement::If(if_stmt) => {
                // When the cursor is *inside* one of the if/else branches,
                // only that branch's type applies — don't union with the
                // other branches.  The union is only correct *after* the
                // entire if/else structure.
                let if_end = stmt.span().end.offset;
                if ctx.cursor_offset <= if_end {
                    // Cursor is inside this if statement — find which
                    // branch contains it and use only that branch.
                    let inner = accumulate_if_branch_at_cursor(if_stmt, ctx);
                    acc.merge(inner);
                } else {
                    // Cursor is past the if statement — union all branches.
                    let inner = accumulate_if_assignment_raw_types(if_stmt, ctx);
                    acc.merge(inner);
                }
            }
            Statement::Foreach(foreach) => {
                let body_stmts: Box<dyn Iterator<Item = &Statement>> = match &foreach.body {
                    ForeachBody::Statement(s) => Box::new(std::iter::once(*s)),
                    ForeachBody::ColonDelimited(b) => Box::new(b.statements.iter()),
                };
                let inner = accumulate_assignment_raw_types(body_stmts, ctx);
                acc.merge(inner);
            }
            Statement::While(while_stmt) => {
                let body_stmts: Box<dyn Iterator<Item = &Statement>> = match &while_stmt.body {
                    WhileBody::Statement(s) => Box::new(std::iter::once(*s)),
                    WhileBody::ColonDelimited(b) => Box::new(b.statements.iter()),
                };
                let inner = accumulate_assignment_raw_types(body_stmts, ctx);
                acc.merge(inner);
            }
            Statement::Try(try_stmt) => {
                let inner = accumulate_assignment_raw_types(try_stmt.block.statements.iter(), ctx);
                acc.merge(inner);
                for catch in try_stmt.catch_clauses.iter() {
                    let inner = accumulate_assignment_raw_types(catch.block.statements.iter(), ctx);
                    acc.merge(inner);
                }
                if let Some(ref finally) = try_stmt.finally_clause {
                    let inner =
                        accumulate_assignment_raw_types(finally.block.statements.iter(), ctx);
                    acc.merge(inner);
                }
            }
            Statement::Expression(expr_stmt) => {
                // Check for inline `/** @var Type */` override before
                // the assignment.  This mirrors the logic in
                // `walk_statements_for_assignments` so that the raw
                // type path also respects @var overrides.
                if try_inline_var_override_raw(
                    expr_stmt.expression,
                    stmt.span().start.offset as usize,
                    ctx,
                    &mut acc,
                ) {
                    continue;
                }

                check_expression_for_raw_type(expr_stmt.expression, ctx, &mut acc);

                // Check for `assert($var instanceof ClassName)` after
                // the assignment.  When found, override the base type
                // with the instanceof class name so that hover shows
                // the narrowed type.
                check_assert_instanceof_for_raw_type(expr_stmt.expression, ctx, &mut acc);
            }
            _ => {}
        }
    }

    acc
}

/// Check for `assert($var instanceof ClassName)` and override the
/// accumulator's base type with the class name when found.
///
/// This enables the raw type path (used by hover) to respect
/// instanceof narrowing via assert statements.
fn check_assert_instanceof_for_raw_type<'b>(
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
    acc: &mut AssignmentAccumulator,
) {
    // Unwrap parenthesised wrapper on the whole expression.
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };

    if let Expression::Call(Call::Function(func_call)) = expr
        && let Expression::Identifier(ident) = func_call.function
        && ident.value() == "assert"
        && let Some(first_arg) = func_call.argument_list.arguments.iter().next()
    {
        let arg_expr = match first_arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };

        // Extract `$var instanceof ClassName` from the argument.
        if let Some(cls_name) = try_extract_instanceof_class(arg_expr, ctx.var_name) {
            acc.set_base(cls_name);
        }
    }
}

/// Try to extract the class name from `$var instanceof ClassName`,
/// handling parenthesisation.  Returns `None` when the expression
/// is not an instanceof check for the given variable.
fn try_extract_instanceof_class<'b>(expr: &'b Expression<'b>, var_name: &str) -> Option<String> {
    match expr {
        Expression::Parenthesized(inner) => {
            try_extract_instanceof_class(inner.expression, var_name)
        }
        Expression::Binary(bin) if bin.operator.is_instanceof() => {
            // LHS must be our variable.
            let lhs_name = match bin.lhs {
                Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
                _ => return None,
            };
            if lhs_name != var_name {
                return None;
            }
            // RHS is the class name.
            match bin.rhs {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Try to resolve a variable's type from an inline `/** @var Type */`
/// docblock that immediately precedes an assignment statement.
///
/// Returns `true` when the override was applied (and the caller should
/// skip normal assignment resolution for this statement).
fn try_inline_var_override_raw<'b>(
    expr: &'b Expression<'b>,
    stmt_start: usize,
    ctx: &VarResolutionCtx<'_>,
    acc: &mut AssignmentAccumulator,
) -> bool {
    // Must be an assignment to our target variable.
    let assignment = match expr {
        Expression::Assignment(a) if a.operator.is_assign() => a,
        _ => return false,
    };
    let lhs_name = match assignment.lhs {
        Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
        _ => return false,
    };
    if lhs_name != ctx.var_name {
        return false;
    }

    // Look for a `/** @var Type [$var] */` docblock right before this
    // statement.
    let (var_type, var_name_opt) = match docblock::find_inline_var_docblock(ctx.content, stmt_start)
    {
        Some(pair) => pair,
        None => return false,
    };

    // If the annotation includes a variable name, it must match.
    if let Some(ref vn) = var_name_opt
        && vn != ctx.var_name
    {
        return false;
    }

    acc.set_base(var_type);
    true
}

/// Check a single expression for base, incremental key, or push
/// assignments to the target variable, updating the accumulator.
fn check_expression_for_raw_type<'b>(
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
    acc: &mut AssignmentAccumulator,
) {
    let assignment = match expr {
        Expression::Assignment(a) if a.operator.is_assign() => a,
        _ => return,
    };

    // Use the assignment's own start offset as cursor_offset so that
    // any recursive variable resolution only considers assignments
    // *before* this one.  Without this, a self-referential assignment
    // like `$numbers['sub_price'] = $numbers['sub_price']->add(…)`
    // would infinitely recurse: resolving the RHS `$numbers['sub_price']`
    // triggers resolution of `$numbers`, which re-discovers the same
    // assignment, resolves its RHS again, and so on until a stack
    // overflow crashes the process.
    //
    // This mirrors the protection in `check_expression_for_assignment`.
    let rhs_ctx = ctx.with_cursor_offset(assignment.span().start.offset);

    match assignment.lhs {
        // ── Base assignment: `$var = expr;` ──
        Expression::Variable(Variable::Direct(dv)) if dv.name == ctx.var_name => {
            if let Some(raw) = resolve_rhs_raw_type(assignment.rhs, &rhs_ctx) {
                acc.set_base(raw);
            }
        }
        // ── Incremental key assignment: `$var['key'] = expr;` ──
        Expression::ArrayAccess(array_access) => {
            if let Expression::Variable(Variable::Direct(dv)) = array_access.array
                && dv.name == ctx.var_name
            {
                let key = extract_array_key_text(array_access.index);
                // Skip numeric-only keys — they are not string-keyed shape entries.
                if key != "mixed" && !key.chars().all(|c| c.is_ascii_digit()) {
                    let value_type = infer_expression_type_string(assignment.rhs, &rhs_ctx);
                    acc.add_incremental_key(key, value_type);
                }
            }
        }
        // ── Push assignment: `$var[] = expr;` ──
        Expression::ArrayAppend(array_append) => {
            if let Expression::Variable(Variable::Direct(dv)) = array_append.array
                && dv.name == ctx.var_name
            {
                let value_type = infer_expression_type_string(assignment.rhs, &rhs_ctx);
                acc.add_push_type(value_type);
            }
        }
        _ => {}
    }
}

/// Infer a type string from an AST expression, used for incremental
/// key and push assignment values.
///
/// Returns a PHPStan-style type string (`"string"`, `"int"`,
/// `"ClassName"`, `"array{…}"`, `"mixed"`, etc.).
fn infer_expression_type_string<'b>(
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> String {
    match expr {
        Expression::Literal(Literal::String(_)) => "string".to_string(),
        Expression::Literal(Literal::Integer(_)) => "int".to_string(),
        Expression::Literal(Literal::Float(_)) => "float".to_string(),
        Expression::Literal(Literal::True(_) | Literal::False(_)) => "bool".to_string(),
        Expression::Literal(Literal::Null(_)) => "null".to_string(),
        Expression::Instantiation(inst) => match inst.class {
            Expression::Identifier(ident) => ident.value().to_string(),
            Expression::Self_(_) => ctx.current_class.name.clone(),
            Expression::Static(_) => ctx.current_class.name.clone(),
            _ => "mixed".to_string(),
        },
        Expression::Array(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx)
            .unwrap_or_else(|| "array".to_string()),
        Expression::LegacyArray(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx)
            .unwrap_or_else(|| "array".to_string()),
        Expression::Parenthesized(p) => infer_expression_type_string(p.expression, ctx),
        // For calls and property access, try the iterable extractor.
        _ => super::foreach_resolution::extract_rhs_iterable_raw_type(expr, ctx)
            .unwrap_or_else(|| "mixed".to_string()),
    }
}

/// Accumulate assignment raw types from all branches of an if
/// statement, **unioning** the base types from each branch.
///
/// For example, `if (…) { $x = new Lamp(); } else { $x = new Faucet(); }`
/// produces a base type of `Lamp|Faucet` rather than just `Faucet`.
fn accumulate_if_assignment_raw_types(
    if_stmt: &If<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> AssignmentAccumulator {
    let mut acc = AssignmentAccumulator::new();

    match &if_stmt.body {
        IfBody::Statement(body) => {
            let inner = accumulate_assignment_raw_types(std::iter::once(body.statement), ctx);
            acc.merge_union(inner);
            for else_if in body.else_if_clauses.iter() {
                let inner =
                    accumulate_assignment_raw_types(std::iter::once(else_if.statement), ctx);
                acc.merge_union(inner);
            }
            if let Some(ref else_clause) = body.else_clause {
                let inner =
                    accumulate_assignment_raw_types(std::iter::once(else_clause.statement), ctx);
                acc.merge_union(inner);
            }
        }
        IfBody::ColonDelimited(body) => {
            let inner = accumulate_assignment_raw_types(body.statements.iter(), ctx);
            acc.merge_union(inner);
            for else_if in body.else_if_clauses.iter() {
                let inner = accumulate_assignment_raw_types(else_if.statements.iter(), ctx);
                acc.merge_union(inner);
            }
            if let Some(ref else_clause) = body.else_clause {
                let inner = accumulate_assignment_raw_types(else_clause.statements.iter(), ctx);
                acc.merge_union(inner);
            }
        }
    }

    acc
}

/// When the cursor is inside an if/else statement, find which specific
/// branch contains it and return only that branch's assignments.
///
/// This prevents hover from showing `Lamp|Faucet` inside the else branch
/// when only `Faucet` is assigned there.  The union is only appropriate
/// after the entire if/else exits.
fn accumulate_if_branch_at_cursor(
    if_stmt: &If<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> AssignmentAccumulator {
    match &if_stmt.body {
        IfBody::Statement(body) => {
            let then_span = body.statement.span();
            if ctx.cursor_offset >= then_span.start.offset
                && ctx.cursor_offset <= then_span.end.offset
            {
                return accumulate_assignment_raw_types(std::iter::once(body.statement), ctx);
            }
            for else_if in body.else_if_clauses.iter() {
                let ei_span = else_if.statement.span();
                if ctx.cursor_offset >= ei_span.start.offset
                    && ctx.cursor_offset <= ei_span.end.offset
                {
                    return accumulate_assignment_raw_types(
                        std::iter::once(else_if.statement),
                        ctx,
                    );
                }
            }
            if let Some(ref else_clause) = body.else_clause {
                let el_span = else_clause.statement.span();
                if ctx.cursor_offset >= el_span.start.offset
                    && ctx.cursor_offset <= el_span.end.offset
                {
                    return accumulate_assignment_raw_types(
                        std::iter::once(else_clause.statement),
                        ctx,
                    );
                }
            }
        }
        IfBody::ColonDelimited(body) => {
            // For colon-delimited if bodies, approximate the span of
            // each branch using the colon/keyword boundaries.
            let then_start = body.colon.start.offset;
            let then_end = body
                .else_if_clauses
                .first()
                .map(|ei| ei.elseif.span().start.offset)
                .or_else(|| {
                    body.else_clause
                        .as_ref()
                        .map(|ec| ec.r#else.span().start.offset)
                })
                .unwrap_or(body.endif.span().start.offset);
            if ctx.cursor_offset >= then_start && ctx.cursor_offset < then_end {
                return accumulate_assignment_raw_types(body.statements.iter(), ctx);
            }
            for (i, else_if) in body.else_if_clauses.iter().enumerate() {
                let ei_start = else_if.colon.start.offset;
                let ei_end = body
                    .else_if_clauses
                    .get(i + 1)
                    .map(|next| next.elseif.span().start.offset)
                    .or_else(|| {
                        body.else_clause
                            .as_ref()
                            .map(|ec| ec.r#else.span().start.offset)
                    })
                    .unwrap_or(body.endif.span().start.offset);
                if ctx.cursor_offset >= ei_start && ctx.cursor_offset < ei_end {
                    return accumulate_assignment_raw_types(else_if.statements.iter(), ctx);
                }
            }
            if let Some(ref else_clause) = body.else_clause {
                let el_start = else_clause.colon.start.offset;
                let el_end = body.endif.span().start.offset;
                if ctx.cursor_offset >= el_start && ctx.cursor_offset < el_end {
                    return accumulate_assignment_raw_types(else_clause.statements.iter(), ctx);
                }
            }
        }
    }

    // Couldn't determine which branch — fall back to union.
    accumulate_if_assignment_raw_types(if_stmt, ctx)
}

/// Extract a raw type string from an expression.
///
/// Handles array literals (producing `list<Type>`), instantiations,
/// call expressions, and property access.  For call expressions and
/// property access, delegates to [`extract_rhs_iterable_raw_type`].
fn resolve_rhs_raw_type<'b>(rhs: &'b Expression<'b>, ctx: &VarResolutionCtx<'_>) -> Option<String> {
    match rhs {
        // ── Scalar literals: `1`, `'hello'`, `true`, `null` ──
        Expression::Literal(Literal::Integer(_)) => Some("int".to_string()),
        Expression::Literal(Literal::Float(_)) => Some("float".to_string()),
        Expression::Literal(Literal::String(_)) => Some("string".to_string()),
        Expression::Literal(Literal::True(_) | Literal::False(_)) => Some("bool".to_string()),
        Expression::Literal(Literal::Null(_)) => Some("null".to_string()),
        // ── Array literal: `[new Foo(), new Bar()]` → `list<Foo|Bar>` ──
        Expression::Array(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx),
        Expression::LegacyArray(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx),
        // ── `new ClassName(…)` → class name ──
        Expression::Instantiation(inst) => match inst.class {
            Expression::Identifier(ident) => Some(ident.value().to_string()),
            Expression::Self_(_) => Some(ctx.current_class.name.clone()),
            Expression::Static(_) => Some(ctx.current_class.name.clone()),
            _ => None,
        },
        // ── Parenthesized: unwrap ──
        Expression::Parenthesized(p) => resolve_rhs_raw_type(p.expression, ctx),
        // ── Ternary / elvis: `$a ? $b : $c` or `$a ?: $c` ──
        // Resolve both branches and union the types.  For the elvis
        // operator (`$a ?: $c`), `then` is `None` and the condition
        // itself is the "then" branch.
        Expression::Conditional(cond_expr) => {
            let then_expr = cond_expr.then.unwrap_or(cond_expr.condition);
            let then_type = resolve_rhs_raw_type(then_expr, ctx);
            let else_type = resolve_rhs_raw_type(cond_expr.r#else, ctx);
            union_raw_types(then_type, else_type)
        }
        // ── Null coalesce: `$a ?? $b` ──
        Expression::Binary(binary) if binary.operator.is_null_coalesce() => {
            let lhs_type = resolve_rhs_raw_type(binary.lhs, ctx);
            let rhs_type = resolve_rhs_raw_type(binary.rhs, ctx);
            union_raw_types(lhs_type, rhs_type)
        }
        // ── Match expression: union all arm result types ──
        Expression::Match(match_expr) => {
            let mut combined: Option<String> = None;
            for arm in match_expr.arms.iter() {
                let arm_type = resolve_rhs_raw_type(arm.expression(), ctx);
                combined = union_raw_types(combined, arm_type);
            }
            combined
        }
        // ── Call / property access — delegate to iterable extractor,
        //    with a source-scan fallback for standalone function calls
        //    when no `function_loader` is available. ──
        _ => super::foreach_resolution::extract_rhs_iterable_raw_type(rhs, ctx).or_else(|| {
            // When function_loader is None, standalone function calls
            // like `$user = getUser()` won't resolve through the
            // iterable extractor.  Fall back to scanning the source
            // for the function's @return docblock.
            if ctx.function_loader.is_none()
                && let Expression::Call(Call::Function(func_call)) = rhs
                && let Expression::Identifier(ident) = func_call.function
            {
                return crate::completion::source::helpers::extract_function_return_from_source(
                    ident.value(),
                    ctx.content,
                );
            }
            None
        }),
    }
}

/// Combine two optional raw type strings into a union.
///
/// When both sides resolve to the same type string, returns a single
/// (non-duplicated) type.  When they differ, joins them with `|`.
/// Returns `None` only when both inputs are `None`.
fn union_raw_types(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => {
            if a == b {
                Some(a)
            } else {
                Some(format!("{}|{}", a, b))
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Infer a `list<Type>` raw type string from an array literal's
/// elements by resolving each value expression.
fn infer_array_literal_raw_type<'b>(
    elements: impl Iterator<Item = &'b ArrayElement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    let mut types: Vec<String> = Vec::new();
    let mut has_string_keys = false;
    let mut shape_parts: Vec<String> = Vec::new();

    for elem in elements {
        match elem {
            ArrayElement::KeyValue(kv) => {
                has_string_keys = true;
                // Extract key text.
                let key_text = extract_array_key_text(kv.key);
                // Resolve value type.
                let value_type =
                    infer_element_type(kv.value, ctx).unwrap_or_else(|| "mixed".to_string());
                shape_parts.push(format!("{}: {}", key_text, value_type));
            }
            ArrayElement::Value(v) => {
                if let Some(t) = infer_element_type(v.value, ctx)
                    && !types.contains(&t)
                {
                    types.push(t);
                }
            }
            ArrayElement::Variadic(v) => {
                // Spread: `...$other` — try to resolve iterable element type.
                if let Some(raw) = resolve_rhs_raw_type(v.value, ctx)
                    && let Some(elem) = docblock::types::extract_generic_value_type(&raw)
                    && !types.contains(&elem)
                {
                    types.push(elem);
                }
            }
            ArrayElement::Missing(_) => {}
        }
    }

    if has_string_keys && !shape_parts.is_empty() {
        return Some(format!("array{{{}}}", shape_parts.join(", ")));
    }

    if types.is_empty() {
        return None;
    }

    let union = types.join("|");
    Some(format!("list<{}>", union))
}

/// Extract a string representation of an array key expression.
fn extract_array_key_text<'b>(key: &'b Expression<'b>) -> String {
    match key {
        Expression::Literal(Literal::String(s)) => {
            // `value` is the unquoted content; fall back to `raw` trimmed.
            s.value.map(|v| v.to_string()).unwrap_or_else(|| {
                let raw = s.raw;
                raw.strip_prefix('\'')
                    .and_then(|r| r.strip_suffix('\''))
                    .or_else(|| raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')))
                    .unwrap_or(raw)
                    .to_string()
            })
        }
        Expression::Literal(Literal::Integer(i)) => i.raw.to_string(),
        _ => "mixed".to_string(),
    }
}

/// Infer the type of a single array element value expression.
fn infer_element_type<'b>(value: &'b Expression<'b>, ctx: &VarResolutionCtx<'_>) -> Option<String> {
    match value {
        // ── Scalar literals ──
        Expression::Literal(Literal::String(_)) => Some("string".to_string()),
        Expression::Literal(Literal::Integer(_)) => Some("int".to_string()),
        Expression::Literal(Literal::Float(_)) => Some("float".to_string()),
        Expression::Literal(Literal::True(_) | Literal::False(_)) => Some("bool".to_string()),
        Expression::Literal(Literal::Null(_)) => Some("null".to_string()),
        // ── Nested array literals ──
        Expression::Array(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx)
            .or_else(|| Some("array".to_string())),
        Expression::LegacyArray(arr) => infer_array_literal_raw_type(arr.elements.iter(), ctx)
            .or_else(|| Some("array".to_string())),
        // ── Object instantiation ──
        Expression::Instantiation(inst) => match inst.class {
            Expression::Identifier(ident) => Some(ident.value().to_string()),
            Expression::Self_(_) => Some(ctx.current_class.name.clone()),
            Expression::Static(_) => Some(ctx.current_class.name.clone()),
            _ => None,
        },
        Expression::Call(_) => {
            // Resolve call return type via the iterable extractor.
            super::foreach_resolution::extract_rhs_iterable_raw_type(value, ctx)
        }
        Expression::Variable(Variable::Direct(dv)) => {
            let var_text = dv.name.to_string();
            let offset = value.span().start.offset as usize;
            docblock::find_iterable_raw_type_in_source(ctx.content, offset, &var_text)
        }
        // ── Parenthesized ──
        Expression::Parenthesized(p) => infer_element_type(p.expression, ctx),
        _ => None,
    }
}

/// For known array functions, resolve the **raw output type** string
/// (e.g. `"list<User>"`) from the input arguments.
///
/// Used by `extract_rhs_iterable_raw_type` so that foreach and
/// destructuring over `array_filter(...)` etc. preserve element types.
pub(in crate::completion) fn resolve_array_func_raw_type(
    func_name: &str,
    args: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    // Type-preserving functions: output array has same element type.
    if ARRAY_PRESERVING_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        let arr_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
        // If the raw type already has generic params, return it as-is
        // so downstream `extract_generic_value_type` can extract the
        // element type.  Otherwise it's a plain class name and we
        // can't infer element type.
        if docblock::types::extract_generic_value_type(&raw).is_some() {
            return Some(raw);
        }
    }

    // array_map: callback is first arg, array is second.
    // The callback's return type determines the output element type.
    if func_name.eq_ignore_ascii_case("array_map")
        && let Some(element_type) = extract_array_map_element_type(args, ctx)
    {
        return Some(format!("list<{}>", element_type));
    }

    // iterator_to_array: converts an iterator to an array, preserving
    // the value type.  `iterator_to_array($iter)` where `$iter` is
    // `Iterator<int, Foo>` produces `array<int, Foo>`.
    if func_name.eq_ignore_ascii_case("iterator_to_array") {
        let iter_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(iter_expr, ctx)?;
        if docblock::types::extract_generic_value_type(&raw).is_some() {
            return Some(raw);
        }
    }

    // Element-extracting functions: wrap element type in list<> so
    // it can be used as an iterable raw type.
    if ARRAY_ELEMENT_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        let arr_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
        if docblock::types::extract_generic_value_type(&raw).is_some() {
            return Some(raw);
        }
    }

    None
}

/// For known array functions, resolve the **element type** string
/// (e.g. `"User"`) for the output.
///
/// Used by `resolve_rhs_expression` so that `$item = array_pop($users)`
/// resolves `$item` to `User`.  This handles both element-extracting
/// functions (array_pop, current, etc.) and `array_map` (via callback
/// return type).
pub(in crate::completion) fn resolve_array_func_element_type(
    func_name: &str,
    args: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    // Element-extracting functions: return the element type directly.
    if ARRAY_ELEMENT_FUNCS
        .iter()
        .any(|f| f.eq_ignore_ascii_case(func_name))
    {
        let arr_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
        return docblock::types::extract_generic_value_type(&raw);
    }

    // array_map: callback return type is the element type.
    if func_name.eq_ignore_ascii_case("array_map") {
        return extract_array_map_element_type(args, ctx);
    }

    // iterator_to_array: the element type is the iterator's value type.
    if func_name.eq_ignore_ascii_case("iterator_to_array") {
        let iter_expr = super::resolution::first_arg_expr(args)?;
        let raw = super::resolution::resolve_arg_raw_type(iter_expr, ctx)?;
        return docblock::types::extract_generic_value_type(&raw);
    }

    None
}

/// Extract the raw text of a function/method argument list from source.
///
/// Returns the text between the parentheses (exclusive), trimmed.
/// For example, an argument list `($user, $role)` returns `"$user, $role"`.
pub(in crate::completion) fn extract_argument_text(
    argument_list: &mago_syntax::ast::ArgumentList<'_>,
    content: &str,
) -> String {
    let left = argument_list.left_parenthesis.span().end.offset as usize;
    let right = argument_list.right_parenthesis.span().start.offset as usize;
    if right > left && right <= content.len() {
        content[left..right].trim().to_string()
    } else {
        String::new()
    }
}

/// Extract the output element type for `array_map($callback, $array)`.
///
/// Strategy:
/// 1. If the callback (first arg) is a closure/arrow function with a
///    return type hint, use that.
/// 2. Otherwise, fall back to the **input array's** element type
///    (assumes the callback preserves type, which is a reasonable
///    default when no return type is declared).
fn extract_array_map_element_type(
    args: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    let callback_expr = super::resolution::first_arg_expr(args)?;

    // Try to get the callback's return type hint.
    let return_hint = match callback_expr {
        Expression::Closure(closure) => closure
            .return_type_hint
            .as_ref()
            .map(|rth| extract_hint_string(&rth.hint)),
        Expression::ArrowFunction(arrow) => arrow
            .return_type_hint
            .as_ref()
            .map(|rth| extract_hint_string(&rth.hint)),
        _ => None,
    };

    if let Some(hint) = return_hint {
        let cleaned = docblock::clean_type(&hint);
        if !cleaned.is_empty() && !docblock::types::is_scalar(&cleaned) {
            return Some(cleaned);
        }
    }

    // Fallback: use the input array's element type.
    let arr_expr = super::resolution::nth_arg_expr(args, 1)?;
    let raw = super::resolution::resolve_arg_raw_type(arr_expr, ctx)?;
    docblock::types::extract_generic_value_type(&raw)
}

/// Reverse-infer a variable's type from `yield $var` statements when
/// the enclosing function declares `@return Generator<TKey, TValue, …>`.
///
/// Scans the source text around the cursor for `yield $varName`
/// patterns within the enclosing function body.  When found, extracts
/// the TValue (2nd generic parameter) from the Generator return type
/// and resolves it to `ClassInfo`.
///
/// This is a fallback used only when normal assignment-based resolution
/// produced no results — the developer is inside a generator body and
/// using a variable that is yielded but was not explicitly typed via
/// an assignment or parameter.
pub(in crate::completion) fn try_infer_from_generator_yield(
    return_type: &str,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    // Only applies to Generator return types.
    let value_type = match crate::docblock::extract_generator_value_type_raw(return_type) {
        Some(vt) => vt,
        None => return vec![],
    };

    // Scan the source text for `yield $varName` or `yield $varName;`
    // within a reasonable window around the cursor.  We look at the
    // enclosing function body (everything between the outermost `{`
    // and `}` that contains the cursor).
    let var_name = ctx.var_name;
    let content = ctx.content;
    let cursor = ctx.cursor_offset as usize;

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

    let start = match body_start {
        Some(s) => s,
        None => return vec![],
    };

    // Find the matching closing `}` by scanning forward from the
    // opening brace.
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

    // Look for `yield $varName` (not `yield from` or `yield $key => $varName`).
    // We check for simple patterns:
    //   - `yield $varName;`
    //   - `yield $varName `  (before semicolon or end of expression)
    let yield_pattern = format!("yield {}", var_name);
    let has_yield = body.contains(&yield_pattern);

    // Also check for `yield $key => $varName` pattern — the variable
    // is the value part in a key-value yield.
    let yield_pair_needle = format!("=> {}", var_name);
    let has_yield_pair = body.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.contains("yield ") && trimmed.contains(&yield_pair_needle)
    });

    if !has_yield && !has_yield_pair {
        return vec![];
    }

    crate::completion::type_resolution::type_hint_to_classes(
        &value_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    )
}
