/// Standalone function and `define()` constant extraction.
///
/// This module handles extracting standalone (non-method) function
/// definitions and `define('NAME', value)` constant declarations from
/// the PHP AST.
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::types::*;

use super::DocblockCtx;

impl Backend {
    /// Extract standalone function definitions from a sequence of statements.
    ///
    /// Recurses into `Statement::Namespace` blocks, passing the namespace
    /// name down so that each `FunctionInfo` records which namespace it
    /// belongs to (if any).
    pub(crate) fn extract_functions_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        functions: &mut Vec<FunctionInfo>,
        current_namespace: &Option<String>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        for statement in statements {
            match statement {
                Statement::Function(func) => {
                    let name = func.name.value.to_string();
                    let parameters = Self::extract_parameters(&func.parameter_list);
                    let native_return_type = func
                        .return_type_hint
                        .as_ref()
                        .map(|rth| Self::extract_hint_string(&rth.hint));

                    // Apply PHPDoc `@return` override for the function.
                    // Also extract PHPStan conditional return types,
                    // type assertion annotations, and `@deprecated` if present.
                    let (return_type, conditional_return, type_assertions, is_deprecated) =
                        if let Some(ctx) = doc_ctx {
                            let docblock_text = docblock::get_docblock_text_for_node(
                                ctx.trivias,
                                ctx.content,
                                func,
                            );

                            let doc_type = docblock_text.and_then(docblock::extract_return_type);

                            let effective = docblock::resolve_effective_type(
                                native_return_type.as_deref(),
                                doc_type.as_deref(),
                            );

                            let conditional =
                                docblock_text.and_then(docblock::extract_conditional_return_type);

                            // If no explicit conditional return type was found,
                            // try to synthesize one from function-level @template
                            // annotations.  For example:
                            //   @template T
                            //   @param class-string<T> $class
                            //   @return T
                            // becomes a conditional that resolves T from the
                            // call-site argument (e.g. resolve(User::class) → User).
                            let conditional = conditional.or_else(|| {
                                let doc = docblock_text?;
                                let tpl_params = docblock::extract_template_params(doc);
                                docblock::synthesize_template_conditional(
                                    doc,
                                    &tpl_params,
                                    effective.as_deref(),
                                    false,
                                )
                            });

                            let assertions = docblock_text
                                .map(docblock::extract_type_assertions)
                                .unwrap_or_default();

                            let deprecated =
                                docblock_text.is_some_and(docblock::has_deprecated_tag);

                            (effective, conditional, assertions, deprecated)
                        } else {
                            (native_return_type, None, Vec::new(), false)
                        };

                    functions.push(FunctionInfo {
                        name,
                        parameters,
                        return_type,
                        namespace: current_namespace.clone(),
                        conditional_return,
                        type_assertions,
                        is_deprecated,
                    });
                }
                Statement::Namespace(namespace) => {
                    let ns_name = namespace
                        .name
                        .as_ref()
                        .map(|ident| ident.value().to_string())
                        .filter(|s| !s.is_empty());

                    // Merge: if we already have a namespace and the inner
                    // one is set, use the inner one; otherwise keep current.
                    let effective_ns = ns_name.or_else(|| current_namespace.clone());

                    Self::extract_functions_from_statements(
                        namespace.statements().iter(),
                        functions,
                        &effective_ns,
                        doc_ctx,
                    );
                }
                // Recurse into block statements `{ ... }` to find nested
                // function declarations.
                Statement::Block(block) => {
                    Self::extract_functions_from_statements(
                        block.statements.iter(),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                // Recurse into `if` bodies — this is critical for the very
                // common PHP pattern:
                //   if (! function_exists('session')) {
                //       function session(...) { ... }
                //   }
                Statement::If(if_stmt) => {
                    Self::extract_functions_from_if_body(
                        &if_stmt.body,
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                _ => {}
            }
        }
    }

    /// Helper: recurse into an `if` statement body to extract function
    /// declarations.  Handles both brace-delimited and colon-delimited
    /// `if` bodies, including `elseif` and `else` branches.
    fn extract_functions_from_if_body<'a>(
        body: &'a IfBody<'a>,
        functions: &mut Vec<FunctionInfo>,
        current_namespace: &Option<String>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        match body {
            IfBody::Statement(body) => {
                Self::extract_functions_from_statements(
                    std::iter::once(body.statement),
                    functions,
                    current_namespace,
                    doc_ctx,
                );
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_functions_from_statements(
                        std::iter::once(else_if.statement),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_functions_from_statements(
                        std::iter::once(else_clause.statement),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
            }
            IfBody::ColonDelimited(body) => {
                Self::extract_functions_from_statements(
                    body.statements.iter(),
                    functions,
                    current_namespace,
                    doc_ctx,
                );
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_functions_from_statements(
                        else_if.statements.iter(),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_functions_from_statements(
                        else_clause.statements.iter(),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
            }
        }
    }

    // ─── define() constant extraction ───────────────────────────────

    /// Walk statements and extract constant names from `define()` calls.
    ///
    /// Handles top-level `define('NAME', value)` calls, as well as those
    /// nested inside namespace blocks, block statements, and `if` guards
    /// (the common `if (!defined('X')) { define('X', …); }` pattern).
    ///
    /// Uses the parsed AST rather than regex, so it piggybacks on the
    /// parse pass that `update_ast` already performs.
    pub(crate) fn extract_defines_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        defines: &mut Vec<String>,
    ) {
        for statement in statements {
            match statement {
                Statement::Expression(expr_stmt) => {
                    if let Some(name) = Self::try_extract_define_name(expr_stmt.expression) {
                        defines.push(name);
                    }
                }
                // Handle namespace-level const declarations
                Statement::Constant(const_decl) => {
                    for item in const_decl.items.iter() {
                        defines.push(item.name.value.to_string());
                    }
                }
                Statement::Namespace(namespace) => {
                    Self::extract_defines_from_statements(namespace.statements().iter(), defines);
                }
                Statement::Block(block) => {
                    Self::extract_defines_from_statements(block.statements.iter(), defines);
                }
                Statement::If(if_stmt) => {
                    Self::extract_defines_from_if_body(&if_stmt.body, defines);
                }
                _ => {}
            }
        }
    }

    /// Helper: recurse into an `if` statement body to extract `define()`
    /// calls.  Mirrors `extract_functions_from_if_body`.
    fn extract_defines_from_if_body<'a>(body: &'a IfBody<'a>, defines: &mut Vec<String>) {
        match body {
            IfBody::Statement(body) => {
                Self::extract_defines_from_statements(std::iter::once(body.statement), defines);
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_defines_from_statements(
                        std::iter::once(else_if.statement),
                        defines,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_defines_from_statements(
                        std::iter::once(else_clause.statement),
                        defines,
                    );
                }
            }
            IfBody::ColonDelimited(body) => {
                Self::extract_defines_from_statements(body.statements.iter(), defines);
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_defines_from_statements(else_if.statements.iter(), defines);
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_defines_from_statements(else_clause.statements.iter(), defines);
                }
            }
        }
    }

    /// Try to extract the constant name from a `define('NAME', …)` call
    /// expression.  Returns `Some(name)` if the expression is a function
    /// call to `define` whose first argument is a string literal.
    fn try_extract_define_name(expr: &Expression<'_>) -> Option<String> {
        if let Expression::Call(Call::Function(func_call)) = expr {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value(),
                _ => return None,
            };
            if !func_name.eq_ignore_ascii_case("define") {
                return None;
            }
            let args: Vec<_> = func_call.argument_list.arguments.iter().collect();
            if args.is_empty() {
                return None;
            }
            let first_expr = match &args[0] {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            if let Expression::Literal(Literal::String(lit_str)) = first_expr
                && let Some(value) = lit_str.value
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
        }
        None
    }
}
