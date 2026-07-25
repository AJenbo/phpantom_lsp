//! Shared AST walker for collecting `return` statement expressions
//! from a function or method body.
//!
//! Recurses into control-flow blocks (if/else, for, while, switch,
//! try/catch/finally, block, declare) but never into a nested
//! function declaration — closures and arrow functions are
//! expressions, so a statement-level walk never visits their bodies.
//!
//! Used by the return-type-mismatch diagnostic
//! ([`crate::diagnostics::return_type_errors`]) and by return-type
//! inference for code actions
//! ([`crate::code_actions::phpstan::fix_return_type`]).

use mago_span::HasSpan;
use mago_syntax::cst::expression::Expression;
use mago_syntax::cst::statement::Statement;

/// Collect `(expression, start_offset, end_offset)` for every `return`
/// reachable from `stmts` without crossing into a nested function/closure
/// scope.  Bare `return;` yields `expression: None` with the span of the
/// `return` keyword itself.
pub(crate) fn collect_returns<'a>(
    stmts: impl Iterator<Item = &'a Statement<'a>>,
    returns: &mut Vec<(Option<&'a Expression<'a>>, usize, usize)>,
) {
    for stmt in stmts {
        collect_returns_from_stmt(stmt, returns);
    }
}

fn collect_returns_from_stmt<'a>(
    stmt: &Statement<'a>,
    returns: &mut Vec<(Option<&'a Expression<'a>>, usize, usize)>,
) {
    match stmt {
        Statement::Return(ret) => {
            if let Some(val) = ret.value {
                let span = val.span();
                returns.push((
                    Some(val),
                    span.start.offset as usize,
                    span.end.offset as usize,
                ));
            } else {
                // Bare `return;` — use the `return` keyword span.
                let kw_span = ret.r#return.span;
                returns.push((
                    None,
                    kw_span.start.offset as usize,
                    kw_span.end.offset as usize,
                ));
            }
        }
        Statement::Namespace(ns) => {
            for inner in ns.statements().iter() {
                collect_returns_from_stmt(inner, returns);
            }
        }
        Statement::If(if_stmt) => {
            collect_returns_from_if_body(&if_stmt.body, returns);
        }
        Statement::While(w) => {
            for s in w.body.statements() {
                collect_returns_from_stmt(s, returns);
            }
        }
        Statement::DoWhile(dw) => {
            collect_returns_from_stmt(dw.statement, returns);
        }
        Statement::For(f) => {
            for s in f.body.statements() {
                collect_returns_from_stmt(s, returns);
            }
        }
        Statement::Foreach(fe) => {
            for s in fe.body.statements() {
                collect_returns_from_stmt(s, returns);
            }
        }
        Statement::Switch(sw) => {
            collect_returns_from_switch_body(&sw.body, returns);
        }
        Statement::Try(t) => {
            for s in t.block.statements.iter() {
                collect_returns_from_stmt(s, returns);
            }
            for catch in t.catch_clauses.iter() {
                for s in catch.block.statements.iter() {
                    collect_returns_from_stmt(s, returns);
                }
            }
            if let Some(ref finally) = t.finally_clause {
                for s in finally.block.statements.iter() {
                    collect_returns_from_stmt(s, returns);
                }
            }
        }
        Statement::Block(block) => {
            for s in block.statements.iter() {
                collect_returns_from_stmt(s, returns);
            }
        }
        Statement::Declare(declare) => {
            use mago_syntax::cst::declare::DeclareBody;
            match &declare.body {
                DeclareBody::Statement(inner) => {
                    collect_returns_from_stmt(inner, returns);
                }
                DeclareBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        collect_returns_from_stmt(s, returns);
                    }
                }
            }
        }
        // Do NOT recurse into closures, arrow functions, or nested
        // functions — they have their own return types.
        Statement::Function(_) => {}
        Statement::Class(_)
        | Statement::Interface(_)
        | Statement::Trait(_)
        | Statement::Enum(_) => {}
        _ => {}
    }
}

fn collect_returns_from_if_body<'a>(
    body: &mago_syntax::cst::control_flow::r#if::IfBody<'a>,
    returns: &mut Vec<(Option<&'a Expression<'a>>, usize, usize)>,
) {
    use mago_syntax::cst::control_flow::r#if::IfBody;
    match body {
        IfBody::Statement(inner) => {
            collect_returns_from_stmt(inner.statement, returns);
            for c in inner.else_if_clauses.iter() {
                collect_returns_from_stmt(c.statement, returns);
            }
            if let Some(ref c) = inner.else_clause {
                collect_returns_from_stmt(c.statement, returns);
            }
        }
        IfBody::ColonDelimited(body) => {
            for s in body.statements.iter() {
                collect_returns_from_stmt(s, returns);
            }
            for c in body.else_if_clauses.iter() {
                for s in c.statements.iter() {
                    collect_returns_from_stmt(s, returns);
                }
            }
            if let Some(ref c) = body.else_clause {
                for s in c.statements.iter() {
                    collect_returns_from_stmt(s, returns);
                }
            }
        }
    }
}

fn collect_returns_from_switch_body<'a>(
    body: &mago_syntax::cst::control_flow::switch::SwitchBody<'a>,
    returns: &mut Vec<(Option<&'a Expression<'a>>, usize, usize)>,
) {
    use mago_syntax::cst::control_flow::switch::SwitchBody;
    match body {
        SwitchBody::BraceDelimited(b) => {
            for case in b.cases.iter() {
                for s in case.statements().iter() {
                    collect_returns_from_stmt(s, returns);
                }
            }
        }
        SwitchBody::ColonDelimited(b) => {
            for case in b.cases.iter() {
                for s in case.statements().iter() {
                    collect_returns_from_stmt(s, returns);
                }
            }
        }
    }
}
