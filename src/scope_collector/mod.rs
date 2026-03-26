//! ScopeCollector — forward-pass variable read/write analysis.
//!
//! This module provides a lightweight forward-pass AST walker that
//! collects every variable read and write with byte offsets across a
//! function/method/closure body.  It is shared infrastructure used by:
//!
//! - A2 (Extract Function)
//! - A4 (Inline Variable)
//! - A5 (Extract Variable)
//! - A6 (Inline Function/Method)
//! - A7 (Extract Constant)
//! - D8 (Undefined variable diagnostic)
//! - Document highlights (all occurrences of a variable in scope)
//!
//! Unlike the existing backward-walk variable resolution in
//! `completion/variable/resolution.rs` (which resolves the type of a
//! single variable at a specific cursor position), the `ScopeCollector`
//! walks **forward** through an entire function body and records _all_
//! variable definitions and usages.
//!
//! # Key concepts
//!
//! - **Frame** = scope boundary.  Each function body, closure, arrow
//!   function, and `catch` block opens a new frame.  Variables defined
//!   inside a frame are local to it.  Closures capture via `use()`;
//!   arrow functions capture by value.  `foreach`, `if`, `for` blocks
//!   do _not_ open new frames in PHP — variables leak into the
//!   enclosing scope.
//!
//! - **VarAccess** = a single read or write of a variable, with name,
//!   byte offset, and access kind.
//!
//! - **ScopeMap** = the result of collecting.  Contains all accesses
//!   organised by frame, plus a query API for extracting parameter sets,
//!   return value sets, and local sets for a given byte range.

#[cfg(test)]
mod tests;

use mago_span::HasSpan;
use mago_syntax::ast::*;

// ─── Core types ─────────────────────────────────────────────────────────────

/// Whether a variable access is a read or a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessKind {
    /// The variable is being read (e.g. `foo($x)`, `return $x`).
    Read,
    /// The variable is being written (e.g. `$x = …`, parameter decl,
    /// foreach binding, catch binding).
    Write,
    /// The variable is being both read and written (e.g. `$x .= …`,
    /// `$x++`, `$x--`, `$x += …`).
    ReadWrite,
}

/// A single variable access (read or write) at a specific byte offset.
#[derive(Debug, Clone)]
pub(crate) struct VarAccess {
    /// Variable name **with** `$` prefix (e.g. `$x`, `$this`).
    pub name: String,
    /// Byte offset of the `$` character in the source.
    pub offset: u32,
    /// Whether this is a read, write, or read-write access.
    pub kind: AccessKind,
}

/// A scope frame representing a function, closure, or arrow function body.
///
/// Each frame records its own variable accesses.  Frames form a tree
/// (closures nested inside functions, etc.), but for the initial
/// implementation we store them in a flat vec and use byte-range
/// containment to determine nesting.
#[derive(Debug, Clone)]
pub(crate) struct Frame {
    /// Byte offset of the frame's opening boundary.
    ///
    /// For functions/methods: the opening `{` of the body.
    /// For closures: the opening `{` of the body.
    /// For arrow functions: the `=>` token offset.
    /// For catch blocks: the opening `{` of the catch body.
    /// For top-level code: `0`.
    pub start: u32,
    /// Byte offset of the frame's closing boundary.
    ///
    /// For functions/methods/closures/catch: the closing `}`.
    /// For arrow functions: the end of the body expression.
    /// For top-level code: `u32::MAX`.
    pub end: u32,
    /// What kind of scope boundary this frame represents.
    pub kind: FrameKind,
    /// Variables explicitly captured via `use($x, &$y)` in closures.
    /// Each entry is `(name_with_dollar, is_by_reference)`.
    pub captures: Vec<(String, bool)>,
}

/// The kind of scope boundary a [`Frame`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameKind {
    /// Top-level code (outside any function/class).
    TopLevel,
    /// Named function: `function foo() { … }`
    Function,
    /// Class method (regular, static, abstract with body, etc.).
    Method,
    /// Closure: `function($x) use($y) { … }`
    Closure,
    /// Arrow function: `fn($x) => expr`
    ArrowFunction,
    /// Catch block: `catch (E $e) { … }`
    Catch,
}

/// The result of a scope collection pass.
///
/// Contains all variable accesses organised by frame, plus a query API
/// for extracting parameter / return-value / local sets for a given
/// byte range.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScopeMap {
    /// All variable accesses across all frames, in source order.
    pub accesses: Vec<VarAccess>,
    /// All scope frames, sorted by `start` offset.
    pub frames: Vec<Frame>,
    /// Whether `$this`, `self::`, `static::`, or `parent::` appears
    /// anywhere in the collected region.  Set during collection.
    pub has_this_or_self: bool,
    /// Whether any by-reference parameter (`&$var`) was encountered.
    pub has_reference_params: bool,
}

/// Variables classified by their role relative to a byte range.
///
/// Returned by [`ScopeMap::classify_range`].
#[derive(Debug, Clone, Default)]
pub(crate) struct RangeClassification {
    /// Variables **read** inside `[start, end)` whose most recent
    /// write is **before** `start`.  These would become parameters
    /// of an extracted function.
    pub parameters: Vec<String>,
    /// Variables **written** inside `[start, end)` that are **read
    /// after** `end` in the enclosing scope.  These would become
    /// return values of an extracted function.
    pub return_values: Vec<String>,
    /// Variables whose entire lifetime (first write to last read) is
    /// contained within `[start, end)`.  These stay inside the
    /// extracted function.
    pub locals: Vec<String>,
    /// Whether `$this`, `self::`, `static::`, or `parent::` appears
    /// in the range.
    pub uses_this: bool,
    /// Variables that are written by reference (`&$var`) inside the
    /// range.
    pub reference_writes: Vec<String>,
}

// ─── ScopeMap query API ─────────────────────────────────────────────────────

impl ScopeMap {
    /// Find the innermost frame that fully contains the given offset.
    pub(crate) fn enclosing_frame(&self, offset: u32) -> Option<&Frame> {
        // Iterate in reverse so we find the innermost (most recently
        // opened) frame first.  Frames are sorted by start offset.
        self.frames
            .iter()
            .rev()
            .find(|f| offset >= f.start && offset <= f.end)
    }

    /// Find the innermost frame that fully contains the given range.
    pub(crate) fn enclosing_frame_for_range(&self, start: u32, end: u32) -> Option<&Frame> {
        self.frames
            .iter()
            .rev()
            .find(|f| start >= f.start && end <= f.end)
    }

    /// Return all accesses of variable `name` within the given frame,
    /// excluding accesses that fall inside a nested frame (closure or
    /// arrow function).
    pub(crate) fn accesses_in_frame<'a>(
        &'a self,
        name: &str,
        frame: &Frame,
    ) -> Vec<&'a VarAccess> {
        self.accesses
            .iter()
            .filter(|a| a.name == name && a.offset >= frame.start && a.offset <= frame.end)
            .filter(|a| {
                !self.frames.iter().any(|f| {
                    f.start > frame.start
                        && f.end < frame.end
                        && a.offset >= f.start
                        && a.offset <= f.end
                        && f.kind != FrameKind::Catch
                })
            })
            .collect()
    }

    /// Classify variables relative to a byte range `[start, end)`.
    ///
    /// This is the primary query for Extract Function: it determines
    /// which variables become parameters, return values, or locals.
    pub(crate) fn classify_range(&self, start: u32, end: u32) -> RangeClassification {
        let frame = match self.enclosing_frame_for_range(start, end) {
            Some(f) => f,
            None => return RangeClassification::default(),
        };

        // Collect all unique variable names accessed within the range
        // (excluding nested frames and pseudo-variables).
        let mut var_names: Vec<String> = Vec::new();
        for access in &self.accesses {
            if access.offset >= start
                && access.offset < end
                && !var_names.contains(&access.name)
                && access.name != "$this"
                && access.name != "self"
                && access.name != "static"
                && access.name != "parent"
            {
                // Skip if inside a nested frame.
                let in_nested = self.frames.iter().any(|f| {
                    f.start > frame.start
                        && f.end < frame.end
                        && access.offset >= f.start
                        && access.offset <= f.end
                        && f.kind != FrameKind::Catch
                });
                if !in_nested {
                    var_names.push(access.name.clone());
                }
            }
        }

        let mut result = RangeClassification::default();

        // Check for $this / self / static / parent usage in range.
        result.uses_this = self.accesses.iter().any(|a| {
            a.offset >= start
                && a.offset < end
                && (a.name == "$this"
                    || a.name == "self"
                    || a.name == "static"
                    || a.name == "parent")
        });

        for var_name in &var_names {
            let frame_accesses = self.accesses_in_frame(var_name, frame);

            let has_write_before = frame_accesses.iter().any(|a| {
                a.offset < start && matches!(a.kind, AccessKind::Write | AccessKind::ReadWrite)
            });

            let has_read_inside = frame_accesses.iter().any(|a| {
                a.offset >= start
                    && a.offset < end
                    && matches!(a.kind, AccessKind::Read | AccessKind::ReadWrite)
            });

            let has_write_inside = frame_accesses.iter().any(|a| {
                a.offset >= start
                    && a.offset < end
                    && matches!(a.kind, AccessKind::Write | AccessKind::ReadWrite)
            });

            let has_read_after = frame_accesses.iter().any(|a| {
                a.offset >= end && matches!(a.kind, AccessKind::Read | AccessKind::ReadWrite)
            });

            let first_write = frame_accesses
                .iter()
                .filter(|a| matches!(a.kind, AccessKind::Write | AccessKind::ReadWrite))
                .min_by_key(|a| a.offset);

            let last_read = frame_accesses
                .iter()
                .filter(|a| matches!(a.kind, AccessKind::Read | AccessKind::ReadWrite))
                .max_by_key(|a| a.offset);

            // Variable whose entire lifetime is within [start, end).
            let entirely_inside =
                first_write.is_some_and(|w| w.offset >= start && w.offset < end)
                    && last_read.map_or(true, |r| r.offset < end)
                    && !has_write_before
                    && !has_read_after;

            if entirely_inside {
                result.locals.push(var_name.clone());
            } else if has_read_inside && has_write_before && !has_write_inside {
                // Read-only inside the range, written before → parameter.
                result.parameters.push(var_name.clone());
            } else if has_read_inside && has_write_before && has_write_inside {
                // Both read and written inside, also written before →
                // parameter (the initial value matters).
                result.parameters.push(var_name.clone());
                if has_read_after {
                    result.return_values.push(var_name.clone());
                }
            } else if has_write_inside && has_read_after {
                // Written inside, read after → return value.
                if has_write_before || has_read_inside {
                    if !result.parameters.contains(var_name) {
                        result.parameters.push(var_name.clone());
                    }
                }
                if !result.return_values.contains(var_name) {
                    result.return_values.push(var_name.clone());
                }
            } else if has_read_inside && !has_write_before && !has_write_inside {
                // Read inside but never written — could be a global or
                // an undefined variable.  Treat as parameter.
                result.parameters.push(var_name.clone());
            } else if has_write_inside && !has_read_inside && !has_read_after {
                // Written inside but never read anywhere — local (dead write).
                result.locals.push(var_name.clone());
            }
        }

        // Sort for deterministic output.
        result.parameters.sort();
        result.return_values.sort();
        result.locals.sort();
        result.reference_writes.sort();

        result
    }

    /// Return all unique variable names accessed within the enclosing
    /// frame of the given offset.
    pub(crate) fn variables_in_scope(&self, offset: u32) -> Vec<String> {
        let frame = match self.enclosing_frame(offset) {
            Some(f) => f,
            None => return Vec::new(),
        };

        let mut names: Vec<String> = Vec::new();
        for access in &self.accesses {
            if access.offset >= frame.start
                && access.offset <= frame.end
                && !names.contains(&access.name)
                && access.name != "$this"
            {
                names.push(access.name.clone());
            }
        }
        names.sort();
        names
    }

    /// Return all offsets where variable `name` is accessed within the
    /// enclosing frame of the given offset.  Useful for document
    /// highlights / find-references within a scope.
    pub(crate) fn all_occurrences(&self, name: &str, offset: u32) -> Vec<(u32, AccessKind)> {
        let frame = match self.enclosing_frame(offset) {
            Some(f) => f,
            None => return Vec::new(),
        };

        self.accesses_in_frame(name, frame)
            .into_iter()
            .map(|a| (a.offset, a.kind))
            .collect()
    }
}

// ─── Collection (forward-pass AST walker) ───────────────────────────────────

/// Internal state for the forward-pass walker.
struct Collector {
    accesses: Vec<VarAccess>,
    frames: Vec<Frame>,
    has_this_or_self: bool,
    has_reference_params: bool,
    /// Stack of frame start offsets for determining the current scope.
    frame_stack: Vec<u32>,
}

impl Collector {
    fn new() -> Self {
        Self {
            accesses: Vec::new(),
            frames: Vec::new(),
            has_this_or_self: false,
            has_reference_params: false,
            frame_stack: Vec::new(),
        }
    }

    fn push_access(&mut self, name: String, offset: u32, kind: AccessKind) {
        self.accesses.push(VarAccess { name, offset, kind });
    }

    fn push_frame(&mut self, frame: Frame) {
        self.frame_stack.push(frame.start);
        self.frames.push(frame);
    }

    fn pop_frame(&mut self) {
        self.frame_stack.pop();
    }
}

/// Collect all variable reads and writes within a function/method body.
///
/// `body_start` and `body_end` are the byte offsets of the opening `{`
/// and closing `}` of the function body.  The returned [`ScopeMap`]
/// contains a single top-level frame plus any nested frames (closures,
/// arrow functions, catch blocks).
pub(crate) fn collect_scope(
    statements: &[Statement<'_>],
    body_start: u32,
    body_end: u32,
) -> ScopeMap {
    let mut collector = Collector::new();

    collector.push_frame(Frame {
        start: body_start,
        end: body_end,
        kind: FrameKind::Function,
        captures: Vec::new(),
    });

    for stmt in statements {
        walk_statement(stmt, &mut collector);
    }

    collector.pop_frame();

    collector.frames.sort_by_key(|f| f.start);

    ScopeMap {
        accesses: collector.accesses,
        frames: collector.frames,
        has_this_or_self: collector.has_this_or_self,
        has_reference_params: collector.has_reference_params,
    }
}

/// Collect scope information for a set of function parameters.
///
/// Records each parameter as a `Write` access at its offset.
pub(crate) fn collect_parameters(
    params: &FunctionLikeParameterList<'_>,
    collector_accesses: &mut Vec<VarAccess>,
    collector_has_reference: &mut bool,
) {
    for param in params.parameters.iter() {
        let name = param.variable.name.to_string();
        let offset = param.variable.span().start.offset;
        collector_accesses.push(VarAccess {
            name,
            offset,
            kind: AccessKind::Write,
        });
        if param.ampersand.is_some() {
            *collector_has_reference = true;
        }
        if let Some(ref default) = param.default_value {
            let mut tmp = Collector::new();
            walk_expression(default.value, &mut tmp);
            collector_accesses.extend(tmp.accesses);
        }
    }
}

/// Convenience: collect scope from a full method or function AST node.
///
/// Includes parameter declarations and the body.
pub(crate) fn collect_function_scope<'a>(
    params: &FunctionLikeParameterList<'a>,
    body: &[Statement<'a>],
    body_start: u32,
    body_end: u32,
) -> ScopeMap {
    let mut collector = Collector::new();

    collector.push_frame(Frame {
        start: body_start,
        end: body_end,
        kind: FrameKind::Function,
        captures: Vec::new(),
    });

    // Record parameters as writes.
    collect_parameters(
        params,
        &mut collector.accesses,
        &mut collector.has_reference_params,
    );

    for stmt in body {
        walk_statement(stmt, &mut collector);
    }

    collector.pop_frame();

    collector.frames.sort_by_key(|f| f.start);

    ScopeMap {
        accesses: collector.accesses,
        frames: collector.frames,
        has_this_or_self: collector.has_this_or_self,
        has_reference_params: collector.has_reference_params,
    }
}

// ─── Statement walker ───────────────────────────────────────────────────────

fn walk_statement(stmt: &Statement<'_>, collector: &mut Collector) {
    match stmt {
        Statement::Expression(expr_stmt) => {
            walk_expression(expr_stmt.expression, collector);
        }
        Statement::Return(ret) => {
            if let Some(val) = ret.value {
                walk_expression(val, collector);
            }
        }
        Statement::Echo(echo) => {
            for val in echo.values.iter() {
                walk_expression(val, collector);
            }
        }
        Statement::If(if_stmt) => {
            walk_expression(if_stmt.condition, collector);
            match &if_stmt.body {
                IfBody::Statement(if_body) => {
                    walk_if_statement_body(if_body, collector);
                }
                IfBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        walk_statement(s, collector);
                    }
                    for clause in body.else_if_clauses.iter() {
                        walk_expression(clause.condition, collector);
                        for s in clause.statements.iter() {
                            walk_statement(s, collector);
                        }
                    }
                    if let Some(ref else_clause) = body.else_clause {
                        for s in else_clause.statements.iter() {
                            walk_statement(s, collector);
                        }
                    }
                }
            }
        }
        Statement::Foreach(foreach) => {
            walk_expression(foreach.expression, collector);
            // The key and value bindings are writes.
            if let Some(key_expr) = foreach.target.key() {
                walk_expression_as_write(key_expr, collector);
            }
            walk_expression_as_write(foreach.target.value(), collector);

            match &foreach.body {
                ForeachBody::Statement(inner) => {
                    walk_statement(inner, collector);
                }
                ForeachBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        walk_statement(s, collector);
                    }
                }
            }
        }
        Statement::While(while_stmt) => {
            walk_expression(while_stmt.condition, collector);
            match &while_stmt.body {
                WhileBody::Statement(inner) => {
                    walk_statement(inner, collector);
                }
                WhileBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        walk_statement(s, collector);
                    }
                }
            }
        }
        Statement::DoWhile(dw) => {
            walk_statement(dw.statement, collector);
            walk_expression(dw.condition, collector);
        }
        Statement::For(for_stmt) => {
            for init in for_stmt.initializations.iter() {
                walk_expression(init, collector);
            }
            for cond in for_stmt.conditions.iter() {
                walk_expression(cond, collector);
            }
            for inc in for_stmt.increments.iter() {
                walk_expression(inc, collector);
            }
            match &for_stmt.body {
                ForBody::Statement(inner) => {
                    walk_statement(inner, collector);
                }
                ForBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        walk_statement(s, collector);
                    }
                }
            }
        }
        Statement::Switch(switch) => {
            walk_expression(switch.expression, collector);
            for case in switch.body.cases().iter() {
                match case {
                    SwitchCase::Expression(c) => {
                        walk_expression(c.expression, collector);
                        for s in c.statements.iter() {
                            walk_statement(s, collector);
                        }
                    }
                    SwitchCase::Default(c) => {
                        for s in c.statements.iter() {
                            walk_statement(s, collector);
                        }
                    }
                }
            }
        }
        Statement::Try(try_stmt) => {
            for s in try_stmt.block.statements.iter() {
                walk_statement(s, collector);
            }
            for catch in try_stmt.catch_clauses.iter() {
                let catch_start = catch.block.left_brace.start.offset;
                let catch_end = catch.block.right_brace.end.offset;
                collector.push_frame(Frame {
                    start: catch_start,
                    end: catch_end,
                    kind: FrameKind::Catch,
                    captures: Vec::new(),
                });
                if let Some(ref var) = catch.variable {
                    let name = var.name.to_string();
                    collector.push_access(name, var.span().start.offset, AccessKind::Write);
                }
                for s in catch.block.statements.iter() {
                    walk_statement(s, collector);
                }
                collector.pop_frame();
            }
            if let Some(ref finally) = try_stmt.finally_clause {
                for s in finally.block.statements.iter() {
                    walk_statement(s, collector);
                }
            }
        }
        Statement::Block(block) => {
            for s in block.statements.iter() {
                walk_statement(s, collector);
            }
        }
        Statement::Unset(unset) => {
            for val in unset.values.iter() {
                // Unset is conceptually a write (destroying the variable).
                walk_expression_as_write(val, collector);
            }
        }
        Statement::Global(global) => {
            for var in global.variables.iter() {
                if let Variable::Direct(dv) = var {
                    let name = dv.name.to_string();
                    collector.push_access(name, dv.span().start.offset, AccessKind::Write);
                }
            }
        }
        Statement::Static(static_stmt) => {
            for item in static_stmt.items.iter() {
                let dv = item.variable();
                let name = dv.name.to_string();
                collector.push_access(name, dv.span().start.offset, AccessKind::Write);
                // Note: we don't walk the default value expression of
                // static items — it's evaluated once at first call and
                // rarely contains variable references.
            }
        }
        Statement::Namespace(ns) => {
            for s in ns.statements().iter() {
                walk_statement(s, collector);
            }
        }
        // Skip class/interface/trait/enum/function declarations — they have
        // their own scopes and don't leak variables.
        Statement::Class(_)
        | Statement::Interface(_)
        | Statement::Trait(_)
        | Statement::Enum(_)
        | Statement::Function(_) => {}

        // Labels / gotos / breaks / continues / declares / nops / use —
        // no variable accesses.
        _ => {}
    }
}

fn walk_if_statement_body(if_body: &IfStatementBody<'_>, collector: &mut Collector) {
    walk_statement(if_body.statement, collector);
    for clause in if_body.else_if_clauses.iter() {
        walk_expression(clause.condition, collector);
        walk_statement(clause.statement, collector);
    }
    if let Some(ref else_clause) = if_body.else_clause {
        walk_statement(else_clause.statement, collector);
    }
}

// ─── Expression walker ──────────────────────────────────────────────────────

fn walk_expression(expr: &Expression<'_>, collector: &mut Collector) {
    match expr {
        Expression::Variable(var) => {
            walk_variable_read(var, collector);
        }
        Expression::Assignment(assignment) => {
            walk_assignment(assignment, collector);
        }
        // ── Function / method / static-method calls ──
        Expression::Call(call) => {
            match call {
                Call::Function(func_call) => {
                    walk_expression(func_call.function, collector);
                    walk_arguments(&func_call.argument_list, collector);
                }
                Call::Method(method_call) => {
                    walk_expression(method_call.object, collector);
                    // method selector is not a variable read
                    walk_arguments(&method_call.argument_list, collector);
                }
                Call::NullSafeMethod(method_call) => {
                    walk_expression(method_call.object, collector);
                    walk_arguments(&method_call.argument_list, collector);
                }
                Call::StaticMethod(static_call) => {
                    walk_expression(static_call.class, collector);
                    walk_arguments(&static_call.argument_list, collector);
                }
            }
        }
        // ── Property / constant access ──
        Expression::Access(access) => match access {
            Access::Property(pa) => {
                walk_expression(pa.object, collector);
                // property selector is not a variable read
            }
            Access::NullSafeProperty(pa) => {
                walk_expression(pa.object, collector);
            }
            Access::StaticProperty(spa) => {
                walk_expression(spa.class, collector);
                // Static property access: the property name variable is a read.
                if let Variable::Direct(dv) = &spa.property {
                    let name = dv.name.to_string();
                    collector.push_access(name, dv.span().start.offset, AccessKind::Read);
                }
            }
            Access::ClassConstant(cca) => {
                walk_expression(cca.class, collector);
            }
        },
        Expression::ArrayAccess(access) => {
            walk_expression(access.array, collector);
            walk_expression(access.index, collector);
        }
        Expression::ArrayAppend(append) => {
            walk_expression(append.array, collector);
        }
        Expression::Array(array) => {
            for element in array.elements.iter() {
                walk_array_element(element, collector);
            }
        }
        Expression::LegacyArray(array) => {
            for element in array.elements.iter() {
                walk_array_element(element, collector);
            }
        }
        Expression::List(list) => {
            // In read position, list entries are reads.
            for element in list.elements.iter() {
                walk_array_element(element, collector);
            }
        }
        Expression::Closure(closure) => {
            walk_closure(closure, collector);
        }
        Expression::ArrowFunction(arrow) => {
            walk_arrow_function(arrow, collector);
        }
        Expression::Parenthesized(paren) => {
            walk_expression(paren.expression, collector);
        }
        Expression::UnaryPrefix(unary) => {
            walk_expression(unary.operand, collector);
        }
        Expression::UnaryPostfix(unary) => {
            // `$x++` and `$x--` are read-write.
            if let Expression::Variable(Variable::Direct(dv)) = unary.operand {
                let name = dv.name.to_string();
                collector.push_access(name, dv.span().start.offset, AccessKind::ReadWrite);
            } else {
                walk_expression(unary.operand, collector);
            }
        }
        Expression::Binary(binary) => {
            walk_expression(binary.lhs, collector);
            walk_expression(binary.rhs, collector);
        }
        // ── Ternary / short ternary / null coalescing ──
        Expression::Conditional(cond) => {
            walk_expression(cond.condition, collector);
            if let Some(then_expr) = cond.then {
                walk_expression(then_expr, collector);
            }
            walk_expression(cond.r#else, collector);
        }
        Expression::Instantiation(inst) => {
            walk_expression(inst.class, collector);
            if let Some(ref args) = inst.argument_list {
                walk_arguments(args, collector);
            }
        }
        Expression::Throw(throw) => {
            walk_expression(throw.exception, collector);
        }
        Expression::Yield(yield_expr) => match yield_expr {
            Yield::Value(yv) => {
                if let Some(val) = yv.value {
                    walk_expression(val, collector);
                }
            }
            Yield::Pair(yp) => {
                walk_expression(yp.key, collector);
                walk_expression(yp.value, collector);
            }
            Yield::From(yf) => {
                walk_expression(yf.iterator, collector);
            }
        },
        Expression::Clone(clone) => {
            walk_expression(clone.object, collector);
        }
        Expression::Match(match_expr) => {
            walk_expression(match_expr.expression, collector);
            for arm in match_expr.arms.iter() {
                match arm {
                    MatchArm::Expression(expr_arm) => {
                        for cond in expr_arm.conditions.iter() {
                            walk_expression(cond, collector);
                        }
                        walk_expression(expr_arm.expression, collector);
                    }
                    MatchArm::Default(default_arm) => {
                        walk_expression(default_arm.expression, collector);
                    }
                }
            }
        }
        // ── self / static / parent keywords ──
        Expression::Self_(_) => {
            collector.has_this_or_self = true;
            collector.push_access("self".to_string(), expr.span().start.offset, AccessKind::Read);
        }
        Expression::Static(_) => {
            collector.has_this_or_self = true;
            collector.push_access(
                "static".to_string(),
                expr.span().start.offset,
                AccessKind::Read,
            );
        }
        Expression::Parent(_) => {
            collector.has_this_or_self = true;
            collector.push_access(
                "parent".to_string(),
                expr.span().start.offset,
                AccessKind::Read,
            );
        }
        // ── Language constructs ──
        Expression::Construct(construct) => match construct {
            Construct::Isset(isset) => {
                for val in isset.values.iter() {
                    walk_expression(val, collector);
                }
            }
            Construct::Empty(empty) => {
                walk_expression(empty.value, collector);
            }
            Construct::Eval(eval) => {
                walk_expression(eval.value, collector);
            }
            Construct::Include(inc) => {
                walk_expression(inc.value, collector);
            }
            Construct::IncludeOnce(inc) => {
                walk_expression(inc.value, collector);
            }
            Construct::Require(req) => {
                walk_expression(req.value, collector);
            }
            Construct::RequireOnce(req) => {
                walk_expression(req.value, collector);
            }
            Construct::Print(print) => {
                walk_expression(print.value, collector);
            }
            Construct::Exit(exit) => {
                if let Some(ref args) = exit.arguments {
                    walk_arguments(args, collector);
                }
            }
            Construct::Die(die) => {
                if let Some(ref args) = die.arguments {
                    walk_arguments(args, collector);
                }
            }
        },
        // ── Composite strings (interpolation, heredoc, shell-exec) ──
        Expression::CompositeString(composite) => {
            for part in composite.parts().iter() {
                match part {
                    StringPart::Expression(inner_expr) => {
                        walk_expression(inner_expr, collector);
                    }
                    StringPart::BracedExpression(braced) => {
                        walk_expression(braced.expression, collector);
                    }
                    StringPart::Literal(_) => {}
                }
            }
        }
        // ── Constant access ──
        Expression::ConstantAccess(_) => {
            // No variable accesses in standalone constant references.
        }
        // ── Pipe operator (PHP 8.5) ──
        Expression::Pipe(pipe) => {
            walk_expression(pipe.input, collector);
            walk_expression(pipe.callable, collector);
        }
        // ── First-class callable / partial application ──
        Expression::PartialApplication(partial) => match partial {
            PartialApplication::Function(func_pa) => {
                walk_expression(func_pa.function, collector);
            }
            PartialApplication::Method(method_pa) => {
                walk_expression(method_pa.object, collector);
            }
            PartialApplication::StaticMethod(static_pa) => {
                walk_expression(static_pa.class, collector);
            }
        },
        // ── Anonymous class ──
        Expression::AnonymousClass(anon) => {
            if let Some(ref args) = anon.argument_list {
                walk_arguments(args, collector);
            }
            // Skip members — anonymous class body is a separate scope.
        }

        // Non-navigable expressions (literals, identifiers, magic constants, etc.)
        Expression::Literal(_)
        | Expression::MagicConstant(_)
        | Expression::Identifier(_)
        | Expression::Error(_) => {}

        // Catch-all for any remaining expression types.
        _ => {}
    }
}

/// Walk an expression that appears in a write position (LHS of
/// assignment, foreach binding, unset argument, etc.).
fn walk_expression_as_write(expr: &Expression<'_>, collector: &mut Collector) {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => {
            let name = dv.name.to_string();
            collector.push_access(name, dv.span().start.offset, AccessKind::Write);
        }
        Expression::Variable(Variable::Indirect(iv)) => {
            walk_expression(iv.expression, collector);
        }
        Expression::Variable(Variable::Nested(nv)) => {
            walk_variable_read(nv.variable, collector);
        }
        Expression::Array(array) => {
            // Array destructuring: `[$a, $b] = …`
            for element in array.elements.iter() {
                match element {
                    ArrayElement::KeyValue(kv) => {
                        walk_expression_as_write(kv.value, collector);
                    }
                    ArrayElement::Value(v) => {
                        walk_expression_as_write(v.value, collector);
                    }
                    ArrayElement::Variadic(spread) => {
                        walk_expression_as_write(spread.value, collector);
                    }
                    ArrayElement::Missing(_) => {}
                }
            }
        }
        Expression::List(list) => {
            // list() destructuring: `list($a, $b) = …`
            for entry in list.elements.iter() {
                match entry {
                    ArrayElement::KeyValue(kv) => {
                        walk_expression_as_write(kv.value, collector);
                    }
                    ArrayElement::Value(v) => {
                        walk_expression_as_write(v.value, collector);
                    }
                    ArrayElement::Variadic(spread) => {
                        walk_expression_as_write(spread.value, collector);
                    }
                    ArrayElement::Missing(_) => {}
                }
            }
        }
        Expression::ArrayAccess(access) => {
            // `$arr[0] = …` — the array itself is being read-written.
            if let Expression::Variable(Variable::Direct(dv)) = access.array {
                let name = dv.name.to_string();
                collector.push_access(name, dv.span().start.offset, AccessKind::ReadWrite);
            } else {
                walk_expression(access.array, collector);
            }
            walk_expression(access.index, collector);
        }
        Expression::ArrayAppend(append) => {
            // `$arr[] = …` — the array itself is being read-written.
            if let Expression::Variable(Variable::Direct(dv)) = append.array {
                let name = dv.name.to_string();
                collector.push_access(name, dv.span().start.offset, AccessKind::ReadWrite);
            } else {
                walk_expression(append.array, collector);
            }
        }
        Expression::Access(Access::Property(pa)) => {
            // `$obj->prop = …` — $obj is read, prop is written.
            walk_expression(pa.object, collector);
        }
        Expression::Access(Access::NullSafeProperty(pa)) => {
            walk_expression(pa.object, collector);
        }
        Expression::Access(Access::StaticProperty(spa)) => {
            walk_expression(spa.class, collector);
        }
        _ => {
            // For anything else, walk as read.
            walk_expression(expr, collector);
        }
    }
}

/// Walk a variable in read position.
fn walk_variable_read(var: &Variable<'_>, collector: &mut Collector) {
    match var {
        Variable::Direct(dv) => {
            let name = dv.name.to_string();
            let offset = dv.span().start.offset;
            if name == "$this" {
                collector.has_this_or_self = true;
            }
            collector.push_access(name, offset, AccessKind::Read);
        }
        Variable::Indirect(iv) => {
            walk_expression(iv.expression, collector);
        }
        Variable::Nested(nv) => {
            walk_variable_read(nv.variable, collector);
        }
    }
}

/// Walk an assignment expression.
fn walk_assignment(assignment: &Assignment<'_>, collector: &mut Collector) {
    // Determine if this is a compound assignment (`+=`, `.=`, etc.)
    let is_compound = !assignment.operator.is_assign();

    if is_compound {
        // Compound assignment: LHS is both read and written.
        if let Expression::Variable(Variable::Direct(dv)) = assignment.lhs {
            let name = dv.name.to_string();
            collector.push_access(name, dv.span().start.offset, AccessKind::ReadWrite);
        } else {
            walk_expression(assignment.lhs, collector);
        }
    } else {
        // Simple assignment: LHS is written.
        walk_expression_as_write(assignment.lhs, collector);
    }

    // RHS is always read.
    walk_expression(assignment.rhs, collector);
}

/// Walk arguments in a function/method call.
fn walk_arguments(args: &ArgumentList<'_>, collector: &mut Collector) {
    for arg in args.arguments.iter() {
        walk_expression(arg.value(), collector);
    }
}

/// Walk array elements in a read context.
fn walk_array_element(element: &ArrayElement<'_>, collector: &mut Collector) {
    match element {
        ArrayElement::KeyValue(kv) => {
            walk_expression(kv.key, collector);
            walk_expression(kv.value, collector);
        }
        ArrayElement::Value(v) => {
            walk_expression(v.value, collector);
        }
        ArrayElement::Variadic(spread) => {
            walk_expression(spread.value, collector);
        }
        ArrayElement::Missing(_) => {}
    }
}

/// Walk a closure expression.
///
/// Creates a new frame for the closure body.  Records `use()` captures
/// and parameter declarations as writes in the new frame.
fn walk_closure(closure: &Closure<'_>, collector: &mut Collector) {
    let body_start = closure.body.left_brace.start.offset;
    let body_end = closure.body.right_brace.end.offset;

    let mut captures = Vec::new();
    if let Some(ref use_clause) = closure.use_clause {
        for var in use_clause.variables.iter() {
            let name = var.variable.name.to_string();
            let is_ref = var.ampersand.is_some();
            captures.push((name.clone(), is_ref));

            // The captured variable is a read in the outer scope at
            // the `use(...)` site.
            collector.push_access(name, var.variable.span().start.offset, AccessKind::Read);
        }
    }

    collector.push_frame(Frame {
        start: body_start,
        end: body_end,
        kind: FrameKind::Closure,
        captures: captures.clone(),
    });

    // Record parameters as writes in the closure frame.
    for param in closure.parameter_list.parameters.iter() {
        let name = param.variable.name.to_string();
        let offset = param.variable.span().start.offset;
        collector.push_access(name, offset, AccessKind::Write);
        if param.ampersand.is_some() {
            collector.has_reference_params = true;
        }
    }

    // Record captures as writes in the closure frame.
    for (cap_name, _is_ref) in &captures {
        collector.push_access(cap_name.clone(), body_start, AccessKind::Write);
    }

    for stmt in closure.body.statements.iter() {
        walk_statement(stmt, collector);
    }

    collector.pop_frame();
}

/// Walk an arrow function expression.
///
/// Creates a new frame for the arrow function body expression.  Arrow
/// functions implicitly capture all outer variables by value.
fn walk_arrow_function(arrow: &ArrowFunction<'_>, collector: &mut Collector) {
    let body_start = arrow.arrow.start.offset;
    let body_end = arrow.expression.span().end.offset;

    collector.push_frame(Frame {
        start: body_start,
        end: body_end,
        kind: FrameKind::ArrowFunction,
        captures: Vec::new(), // Arrow functions capture implicitly.
    });

    // Record parameters as writes.
    for param in arrow.parameter_list.parameters.iter() {
        let name = param.variable.name.to_string();
        let offset = param.variable.span().start.offset;
        collector.push_access(name, offset, AccessKind::Write);
        if param.ampersand.is_some() {
            collector.has_reference_params = true;
        }
    }

    walk_expression(arrow.expression, collector);

    collector.pop_frame();
}