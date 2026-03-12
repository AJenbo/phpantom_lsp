/// Standalone function and `define()` constant extraction.
///
/// This module handles extracting standalone (non-method) function
/// definitions and `define('NAME', value)` constant declarations from
/// the PHP AST.
use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::types::*;

use super::{
    DocblockCtx, extract_hint_string, extract_parameters, is_available_for_version,
    merge_deprecation_info,
};

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
                    // Skip functions whose #[PhpStormStubsElementAvailable]
                    // range excludes the target PHP version.
                    if let Some(ctx) = doc_ctx
                        && let Some(ver) = ctx.php_version
                        && !is_available_for_version(&func.attribute_lists, ctx, ver)
                    {
                        continue;
                    }

                    let name = func.name.value.to_string();
                    let name_offset = func.name.span.start.offset;
                    let php_version = doc_ctx.and_then(|ctx| ctx.php_version);
                    let mut parameters = extract_parameters(
                        &func.parameter_list,
                        doc_ctx.map(|ctx| ctx.content),
                        php_version,
                        doc_ctx,
                    );
                    let native_return_type = func
                        .return_type_hint
                        .as_ref()
                        .map(|rth| extract_hint_string(&rth.hint));

                    // Apply PHPDoc `@return` override for the function.
                    // Also extract PHPStan conditional return types,
                    // type assertion annotations, and `@deprecated` if present.
                    let (
                        return_type,
                        conditional_return,
                        type_assertions,
                        deprecation_message,
                        deprecated_replacement,
                        description,
                        return_description,
                        link,
                        func_template_params,
                        func_template_bindings,
                    ) = if let Some(ctx) = doc_ctx {
                        let docblock_text =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, func);

                        let doc_type = docblock_text.and_then(docblock::extract_return_type);

                        let effective = docblock::resolve_effective_type(
                            native_return_type.as_deref(),
                            doc_type.as_deref(),
                        );

                        let conditional =
                            docblock_text.and_then(docblock::extract_conditional_return_type);

                        // Extract function-level @template params and their
                        // @param bindings for generic type substitution at
                        // call sites.
                        let tpl_params: Vec<String> = docblock_text
                            .map(docblock::extract_template_params)
                            .unwrap_or_default();
                        let tpl_bindings = if !tpl_params.is_empty() {
                            docblock_text
                                .map(|doc| {
                                    docblock::extract_template_param_bindings(doc, &tpl_params)
                                })
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };

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

                        let depr_info = merge_deprecation_info(
                            docblock_text.and_then(docblock::extract_deprecation_message),
                            &func.attribute_lists,
                            Some(ctx),
                        );
                        let deprecation_message = depr_info.message;
                        let deprecated_replacement = depr_info.replacement;

                        let desc = docblock_text
                            .and_then(|doc| crate::hover::extract_docblock_description(Some(doc)));

                        let ret_desc = docblock_text.and_then(docblock::extract_return_description);

                        let link_url = docblock_text.and_then(docblock::extract_link_url);

                        (
                            effective,
                            conditional,
                            assertions,
                            deprecation_message,
                            deprecated_replacement,
                            desc,
                            ret_desc,
                            link_url,
                            tpl_params,
                            tpl_bindings,
                        )
                    } else {
                        // No docblock context available — attribute argument
                        // strings cannot be read without source text, so we
                        // skip #[Deprecated] extraction here.  In practice
                        // `doc_ctx` is always `Some` for real file parsing.
                        (
                            native_return_type.clone(),
                            None,
                            Vec::new(),
                            None,
                            None,
                            None,
                            None,
                            None,
                            Vec::new(),
                            Vec::new(),
                        )
                    };

                    // Merge `@param` docblock types into parameter type
                    // hints and populate per-parameter descriptions.
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, func)
                    {
                        for param in &mut parameters {
                            let param_doc_type =
                                docblock::extract_param_raw_type(doc_text, &param.name);
                            if let Some(ref doc_type) = param_doc_type {
                                let effective = docblock::resolve_effective_type(
                                    param.type_hint.as_deref(),
                                    Some(doc_type),
                                );
                                if effective.is_some() {
                                    param.type_hint = effective;
                                }
                            }
                            param.description =
                                docblock::extract_param_description(doc_text, &param.name);
                        }

                        // Populate `closure_this_type` from
                        // `@param-closure-this` tags so that `$this`
                        // inside a closure argument resolves to the
                        // declared type instead of the lexical class.
                        for (this_type, param_name) in
                            docblock::extract_param_closure_this(doc_text)
                        {
                            if let Some(param) =
                                parameters.iter_mut().find(|p| p.name == param_name)
                            {
                                param.closure_this_type = Some(this_type);
                            }
                        }
                    }

                    functions.push(FunctionInfo {
                        name,
                        name_offset,
                        parameters,
                        native_return_type,
                        return_type,
                        description,
                        return_description,
                        link,
                        namespace: current_namespace.clone(),
                        conditional_return,
                        type_assertions,
                        deprecation_message,
                        deprecated_replacement,
                        template_params: func_template_params,
                        template_bindings: func_template_bindings,
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

    /// Walk statements and extract constant names from `define()` calls
    /// and top-level `const` statements.
    ///
    /// Handles top-level `define('NAME', value)` calls, as well as those
    /// nested inside namespace blocks, block statements, and `if` guards
    /// (the common `if (!defined('X')) { define('X', …); }` pattern).
    /// Also handles `const FOO = 'bar';` statements at the top level or
    /// inside namespace blocks.
    ///
    /// The `content` parameter is the full source text of the file, used
    /// to extract the initializer value as a string slice.
    ///
    /// Uses the parsed AST rather than regex, so it piggybacks on the
    /// parse pass that `update_ast` already performs.
    pub(crate) fn extract_defines_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        defines: &mut Vec<(String, u32, Option<String>)>,
        content: &str,
    ) {
        for statement in statements {
            match statement {
                Statement::Expression(expr_stmt) => {
                    if let Some(entry) =
                        Self::try_extract_define_info(expr_stmt.expression, content)
                    {
                        defines.push(entry);
                    }
                }
                // Handle namespace-level const declarations
                Statement::Constant(const_decl) => {
                    for item in const_decl.items.iter() {
                        let start = item.value.span().start.offset as usize;
                        let end = item.value.span().end.offset as usize;
                        let value = content.get(start..end).map(|s| s.to_string());
                        defines.push((
                            item.name.value.to_string(),
                            item.name.span.start.offset,
                            value,
                        ));
                    }
                }
                Statement::Namespace(namespace) => {
                    Self::extract_defines_from_statements(
                        namespace.statements().iter(),
                        defines,
                        content,
                    );
                }
                Statement::Block(block) => {
                    Self::extract_defines_from_statements(
                        block.statements.iter(),
                        defines,
                        content,
                    );
                }
                Statement::If(if_stmt) => {
                    Self::extract_defines_from_if_body(&if_stmt.body, defines, content);
                }
                Statement::Class(class) => {
                    for member in class.members.iter() {
                        if let ClassLikeMember::Method(method) = member
                            && let MethodBody::Concrete(body) = &method.body
                        {
                            Self::extract_defines_from_statements(
                                body.statements.iter(),
                                defines,
                                content,
                            );
                        }
                    }
                }
                Statement::Trait(trait_def) => {
                    for member in trait_def.members.iter() {
                        if let ClassLikeMember::Method(method) = member
                            && let MethodBody::Concrete(body) = &method.body
                        {
                            Self::extract_defines_from_statements(
                                body.statements.iter(),
                                defines,
                                content,
                            );
                        }
                    }
                }
                Statement::Enum(enum_def) => {
                    for member in enum_def.members.iter() {
                        if let ClassLikeMember::Method(method) = member
                            && let MethodBody::Concrete(body) = &method.body
                        {
                            Self::extract_defines_from_statements(
                                body.statements.iter(),
                                defines,
                                content,
                            );
                        }
                    }
                }
                Statement::Function(func) => {
                    Self::extract_defines_from_statements(
                        func.body.statements.iter(),
                        defines,
                        content,
                    );
                }
                _ => {}
            }
        }
    }

    /// Helper: recurse into an `if` statement body to extract `define()`
    /// calls.  Mirrors `extract_functions_from_if_body`.
    fn extract_defines_from_if_body<'a>(
        body: &'a IfBody<'a>,
        defines: &mut Vec<(String, u32, Option<String>)>,
        content: &str,
    ) {
        match body {
            IfBody::Statement(body) => {
                Self::extract_defines_from_statements(
                    std::iter::once(body.statement),
                    defines,
                    content,
                );
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_defines_from_statements(
                        std::iter::once(else_if.statement),
                        defines,
                        content,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_defines_from_statements(
                        std::iter::once(else_clause.statement),
                        defines,
                        content,
                    );
                }
            }
            IfBody::ColonDelimited(body) => {
                Self::extract_defines_from_statements(body.statements.iter(), defines, content);
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_defines_from_statements(
                        else_if.statements.iter(),
                        defines,
                        content,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_defines_from_statements(
                        else_clause.statements.iter(),
                        defines,
                        content,
                    );
                }
            }
        }
    }

    /// Try to extract the constant name, byte offset, and value from a
    /// `define('NAME', value)` call expression.  Returns
    /// `Some((name, define_keyword_offset, value_text))` if the expression
    /// is a function call to `define` whose first argument is a string literal.
    fn try_extract_define_info(
        expr: &Expression<'_>,
        content: &str,
    ) -> Option<(String, u32, Option<String>)> {
        if let Expression::Call(Call::Function(func_call)) = expr {
            let ident = match func_call.function {
                Expression::Identifier(ident) => ident,
                _ => return None,
            };
            if !ident.value().eq_ignore_ascii_case("define") {
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
                && let Some(name) = lit_str.value
                && !name.is_empty()
            {
                let offset = ident.span().start.offset;
                // Extract the value from the second argument if present.
                let value_text = args.get(1).and_then(|arg| {
                    let val_expr = match arg {
                        Argument::Positional(pos) => pos.value,
                        Argument::Named(named) => named.value,
                    };
                    let start = val_expr.span().start.offset as usize;
                    let end = val_expr.span().end.offset as usize;
                    content.get(start..end).map(|s| s.to_string())
                });
                return Some((name.to_string(), offset, value_text));
            }
        }
        None
    }
}
