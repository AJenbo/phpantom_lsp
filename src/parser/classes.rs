use std::collections::HashMap;

use mago_span::HasSpan;
use mago_syntax::ast::class_like::method::MethodBody;
use mago_syntax::ast::class_like::trait_use::{
    TraitUseAdaptation, TraitUseMethodReference, TraitUseSpecification,
};
/// Class, interface, trait, and enum extraction.
///
/// Each class-like declaration is tagged with a [`ClassLikeKind`] so that
/// downstream consumers (e.g. `throw new` completion) can distinguish
/// concrete classes from interfaces, traits, and enums.
///
/// This module handles extracting `ClassInfo` from the PHP AST for all
/// class-like declarations: `class`, `interface`, `trait`, and `enum`.
/// It also extracts class-like members (methods, properties, constants,
/// trait uses) and merges in PHPDoc `@property`, `@method`, `@mixin`,
/// and `@deprecated` annotations from docblocks.
///
/// Anonymous classes (`new class { ... }`) are also extracted.  They are
/// given synthetic names of the form `__anonymous@<offset>` so that
/// [`find_class_at_offset`](Backend::find_class_at_offset) can resolve
/// `$this` inside their bodies.
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::types::*;

use super::DocblockCtx;

/// Docblock-derived metadata common to all class-like declarations.
///
/// Produced by [`extract_class_docblock`] and consumed by each match arm
/// in [`Backend::extract_classes_from_statements`] to avoid repeating
/// the same extraction calls for classes, interfaces, traits, and enums.
#[derive(Default)]
struct ClassDocblockInfo {
    /// Whether the class-level docblock contains `@deprecated`.
    is_deprecated: bool,
    /// `@template` parameters declared on the class-like.
    template_params: Vec<String>,
    /// Upper bounds for template parameters (`@template T of Bound`).
    template_param_bounds: HashMap<String, String>,
    /// Generic arguments from `@extends` / `@phpstan-extends`.
    extends_generics: Vec<(String, Vec<String>)>,
    /// Generic arguments from `@implements` / `@phpstan-implements`.
    implements_generics: Vec<(String, Vec<String>)>,
    /// Generic arguments from `@use` / `@phpstan-use`.
    use_generics: Vec<(String, Vec<String>)>,
    /// Type aliases from `@phpstan-type` / `@psalm-type`.
    type_aliases: HashMap<String, String>,
    /// Virtual properties from `@property` tags.
    extra_properties: Vec<PropertyInfo>,
    /// Virtual methods from `@method` tags.
    extra_methods: Vec<MethodInfo>,
    /// Mixin class names from `@mixin` tags.
    mixins: Vec<String>,
}

impl ClassDocblockInfo {
    /// Merge virtual `@property` and `@method` tags into existing member
    /// lists, skipping any that duplicate a real declaration.
    ///
    /// The extra vectors are drained so the remaining fields can still be
    /// read after calling this method.
    fn merge_into(&mut self, methods: &mut Vec<MethodInfo>, properties: &mut Vec<PropertyInfo>) {
        for prop in std::mem::take(&mut self.extra_properties) {
            if !properties.iter().any(|p| p.name == prop.name) {
                properties.push(prop);
            }
        }
        for method in std::mem::take(&mut self.extra_methods) {
            if !methods.iter().any(|m| m.name == method.name) {
                methods.push(method);
            }
        }
    }
}

/// Extract all docblock-derived metadata from a class-like AST node.
///
/// Returns [`ClassDocblockInfo::default()`] when no docblock context is
/// available or when the node has no preceding doc comment.
fn extract_class_docblock<'a>(
    node: &impl HasSpan,
    doc_ctx: Option<&DocblockCtx<'a>>,
) -> ClassDocblockInfo {
    let Some(ctx) = doc_ctx else {
        return ClassDocblockInfo::default();
    };
    let Some(doc_text) = docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, node)
    else {
        return ClassDocblockInfo::default();
    };

    let extra_properties = docblock::extract_property_tags(doc_text)
        .into_iter()
        .map(|(name, type_str)| PropertyInfo {
            name,
            type_hint: if type_str.is_empty() {
                None
            } else {
                Some(type_str)
            },
            is_static: false,
            visibility: Visibility::Public,
            is_deprecated: false,
        })
        .collect();

    let params_with_bounds = docblock::extract_template_params_with_bounds(doc_text);
    let template_params = params_with_bounds.iter().map(|(n, _)| n.clone()).collect();
    let template_param_bounds: HashMap<String, String> = params_with_bounds
        .into_iter()
        .filter_map(|(name, bound)| bound.map(|b| (name, b)))
        .collect();

    ClassDocblockInfo {
        is_deprecated: docblock::has_deprecated_tag(doc_text),
        template_params,
        template_param_bounds,
        extends_generics: docblock::extract_generics_tag(doc_text, "@extends"),
        implements_generics: docblock::extract_generics_tag(doc_text, "@implements"),
        use_generics: docblock::extract_generics_tag(doc_text, "@use"),
        type_aliases: docblock::extract_type_aliases(doc_text),
        extra_properties,
        extra_methods: docblock::extract_method_tags(doc_text),
        mixins: docblock::extract_mixin_tags(doc_text),
    }
}

impl Backend {
    /// Recursively walk statements and extract class information.
    /// This handles classes at the top level as well as classes nested
    /// inside namespace declarations.
    pub(crate) fn extract_classes_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        for statement in statements {
            match statement {
                Statement::Class(class) => {
                    let class_name = class.name.value.to_string();

                    let parent_class = class
                        .extends
                        .as_ref()
                        .and_then(|ext| ext.types.first().map(|ident| ident.value().to_string()));

                    let interfaces: Vec<String> = class
                        .implements
                        .as_ref()
                        .map(|imp| {
                            imp.types
                                .iter()
                                .map(|ident| ident.value().to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    let (
                        mut methods,
                        mut properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                    ) = Self::extract_class_like_members(class.members.iter(), doc_ctx);

                    let mut doc_info = extract_class_docblock(class, doc_ctx);
                    doc_info.merge_into(&mut methods, &mut properties);

                    let start_offset = class.left_brace.start.offset;
                    let end_offset = class.right_brace.end.offset;

                    classes.push(ClassInfo {
                        kind: ClassLikeKind::Class,
                        name: class_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        interfaces,
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: class.modifiers.contains_final(),
                        is_abstract: class.modifiers.contains_abstract(),
                        is_deprecated: doc_info.is_deprecated,
                        template_params: doc_info.template_params,
                        template_param_bounds: doc_info.template_param_bounds,
                        extends_generics: doc_info.extends_generics,
                        implements_generics: doc_info.implements_generics,
                        use_generics: doc_info.use_generics,
                        type_aliases: doc_info.type_aliases,
                        trait_precedences,
                        trait_aliases,
                    });

                    // Walk method bodies for anonymous classes.
                    Self::find_anonymous_classes_in_members(class.members.iter(), classes, doc_ctx);
                }
                Statement::Interface(iface) => {
                    let iface_name = iface.name.value.to_string();

                    // Interfaces can extend multiple parent interfaces.
                    // Store the first one in `parent_class` for backward
                    // compatibility with single-inheritance resolution,
                    // and all of them in `interfaces` so that transitive
                    // interface inheritance checks work correctly.
                    let all_parents: Vec<String> = iface
                        .extends
                        .as_ref()
                        .map(|ext| {
                            ext.types
                                .iter()
                                .map(|ident| ident.value().to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    let parent_class = all_parents.first().cloned();

                    let (
                        mut methods,
                        mut properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                    ) = Self::extract_class_like_members(iface.members.iter(), doc_ctx);

                    let mut doc_info = extract_class_docblock(iface, doc_ctx);
                    doc_info.merge_into(&mut methods, &mut properties);

                    let start_offset = iface.left_brace.start.offset;
                    let end_offset = iface.right_brace.end.offset;

                    classes.push(ClassInfo {
                        kind: ClassLikeKind::Interface,
                        name: iface_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        interfaces: all_parents,
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: false,
                        is_abstract: false,
                        is_deprecated: doc_info.is_deprecated,
                        template_params: doc_info.template_params,
                        template_param_bounds: doc_info.template_param_bounds,
                        extends_generics: doc_info.extends_generics,
                        implements_generics: doc_info.implements_generics,
                        use_generics: doc_info.use_generics,
                        type_aliases: doc_info.type_aliases,
                        trait_precedences,
                        trait_aliases,
                    });

                    // Walk method bodies for anonymous classes.
                    Self::find_anonymous_classes_in_members(iface.members.iter(), classes, doc_ctx);
                }
                Statement::Trait(trait_def) => {
                    let trait_name = trait_def.name.value.to_string();

                    let (
                        mut methods,
                        mut properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                    ) = Self::extract_class_like_members(trait_def.members.iter(), doc_ctx);

                    let mut doc_info = extract_class_docblock(trait_def, doc_ctx);
                    doc_info.merge_into(&mut methods, &mut properties);

                    let start_offset = trait_def.left_brace.start.offset;
                    let end_offset = trait_def.right_brace.end.offset;

                    classes.push(ClassInfo {
                        kind: ClassLikeKind::Trait,
                        name: trait_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class: None,
                        interfaces: vec![],
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: false,
                        is_abstract: false,
                        is_deprecated: doc_info.is_deprecated,
                        template_params: doc_info.template_params,
                        template_param_bounds: doc_info.template_param_bounds,
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: vec![],
                        type_aliases: HashMap::new(),
                        trait_precedences,
                        trait_aliases,
                    });

                    // Walk method bodies for anonymous classes.
                    Self::find_anonymous_classes_in_members(
                        trait_def.members.iter(),
                        classes,
                        doc_ctx,
                    );
                }
                Statement::Enum(enum_def) => {
                    let enum_name = enum_def.name.value.to_string();

                    let (mut methods, mut properties, constants, mut used_traits, _, _) =
                        Self::extract_class_like_members(enum_def.members.iter(), doc_ctx);

                    // Enums implicitly implement UnitEnum or BackedEnum.
                    // We add the interface as a fully-qualified name (leading
                    // backslash) so that `resolve_name` does not prepend the
                    // current namespace.  The class_loader / merge_traits_into
                    // path will pick up the interface from the SPL stubs and
                    // merge its methods (cases, from, tryFrom, …) automatically.
                    let implicit_interface = if enum_def.backing_type_hint.is_some() {
                        "\\BackedEnum"
                    } else {
                        "\\UnitEnum"
                    };
                    used_traits.push(implicit_interface.to_string());

                    let mut doc_info = extract_class_docblock(enum_def, doc_ctx);
                    doc_info.merge_into(&mut methods, &mut properties);

                    let interfaces: Vec<String> = enum_def
                        .implements
                        .as_ref()
                        .map(|imp| {
                            imp.types
                                .iter()
                                .map(|ident| ident.value().to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    let start_offset = enum_def.left_brace.start.offset;
                    let end_offset = enum_def.right_brace.end.offset;

                    // Enums are implicitly final and cannot be extended.
                    classes.push(ClassInfo {
                        kind: ClassLikeKind::Enum,
                        name: enum_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class: None,
                        interfaces,
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: true,
                        is_abstract: false,
                        is_deprecated: doc_info.is_deprecated,
                        template_params: vec![],
                        template_param_bounds: HashMap::new(),
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: vec![],
                        type_aliases: HashMap::new(),
                        trait_precedences: vec![],
                        trait_aliases: vec![],
                    });

                    // Walk method bodies for anonymous classes.
                    Self::find_anonymous_classes_in_members(
                        enum_def.members.iter(),
                        classes,
                        doc_ctx,
                    );
                }
                Statement::Namespace(namespace) => {
                    Self::extract_classes_from_statements(
                        namespace.statements().iter(),
                        classes,
                        doc_ctx,
                    );
                }
                _ => {
                    // Walk into all other statement types to find anonymous
                    // classes nested inside expressions, control flow, method
                    // bodies, closures, etc.
                    Self::find_anonymous_classes_in_statement(statement, classes, doc_ctx);
                }
            }
        }
    }

    // ─── Anonymous class extraction ─────────────────────────────────────

    /// Extract an anonymous class node into a [`ClassInfo`] with a
    /// synthetic name `__anonymous@<offset>`.
    fn extract_anonymous_class_info<'a>(
        anon: &AnonymousClass<'a>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) -> ClassInfo {
        let parent_class = anon
            .extends
            .as_ref()
            .and_then(|ext| ext.types.first().map(|ident| ident.value().to_string()));

        let interfaces: Vec<String> = anon
            .implements
            .as_ref()
            .map(|imp| {
                imp.types
                    .iter()
                    .map(|ident| ident.value().to_string())
                    .collect()
            })
            .unwrap_or_default();

        let (methods, properties, constants, used_traits, trait_precedences, trait_aliases) =
            Self::extract_class_like_members(anon.members.iter(), doc_ctx);

        let start_offset = anon.left_brace.start.offset;
        let end_offset = anon.right_brace.end.offset;
        let name = format!("__anonymous@{}", start_offset);

        ClassInfo {
            kind: ClassLikeKind::Class,
            name,
            methods,
            properties,
            constants,
            start_offset,
            end_offset,
            parent_class,
            interfaces,
            used_traits,
            mixins: vec![],
            is_final: false,
            is_abstract: false,
            is_deprecated: false,
            template_params: vec![],
            template_param_bounds: HashMap::new(),
            extends_generics: vec![],
            implements_generics: vec![],
            use_generics: vec![],
            type_aliases: HashMap::new(),
            trait_precedences,
            trait_aliases,
        }
    }

    /// Recursively walk a statement looking for anonymous classes in
    /// expressions and nested statement blocks.
    pub(crate) fn find_anonymous_classes_in_statement<'a>(
        statement: &'a Statement<'a>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        match statement {
            Statement::Expression(expr_stmt) => {
                Self::find_anonymous_classes_in_expression(expr_stmt.expression, classes, doc_ctx);
            }
            Statement::Return(ret) => {
                if let Some(value) = &ret.value {
                    Self::find_anonymous_classes_in_expression(value, classes, doc_ctx);
                }
            }
            Statement::Block(block) => {
                Self::walk_statements_for_anonymous_classes(
                    block.statements.iter(),
                    classes,
                    doc_ctx,
                );
            }
            Statement::If(if_stmt) => {
                Self::find_anonymous_classes_in_if_body(&if_stmt.body, classes, doc_ctx);
            }
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(stmt) => {
                    Self::find_anonymous_classes_in_statement(stmt, classes, doc_ctx);
                }
                WhileBody::ColonDelimited(body) => {
                    Self::walk_statements_for_anonymous_classes(
                        body.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
            },
            Statement::DoWhile(do_while) => {
                Self::find_anonymous_classes_in_statement(do_while.statement, classes, doc_ctx);
            }
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(stmt) => {
                    Self::find_anonymous_classes_in_statement(stmt, classes, doc_ctx);
                }
                ForBody::ColonDelimited(body) => {
                    Self::walk_statements_for_anonymous_classes(
                        body.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
            },
            Statement::Foreach(foreach_stmt) => match &foreach_stmt.body {
                ForeachBody::Statement(stmt) => {
                    Self::find_anonymous_classes_in_statement(stmt, classes, doc_ctx);
                }
                ForeachBody::ColonDelimited(body) => {
                    Self::walk_statements_for_anonymous_classes(
                        body.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
            },
            Statement::Switch(switch_stmt) => {
                let cases = match &switch_stmt.body {
                    SwitchBody::BraceDelimited(b) => &b.cases,
                    SwitchBody::ColonDelimited(b) => &b.cases,
                };
                for case in cases.iter() {
                    let stmts = match case {
                        SwitchCase::Expression(c) => &c.statements,
                        SwitchCase::Default(c) => &c.statements,
                    };
                    Self::walk_statements_for_anonymous_classes(stmts.iter(), classes, doc_ctx);
                }
            }
            Statement::Try(try_stmt) => {
                Self::walk_statements_for_anonymous_classes(
                    try_stmt.block.statements.iter(),
                    classes,
                    doc_ctx,
                );
                for catch in try_stmt.catch_clauses.iter() {
                    Self::walk_statements_for_anonymous_classes(
                        catch.block.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    Self::walk_statements_for_anonymous_classes(
                        finally.block.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
            }
            Statement::Function(func) => {
                Self::walk_statements_for_anonymous_classes(
                    func.body.statements.iter(),
                    classes,
                    doc_ctx,
                );
            }
            // Named class-like declarations: walk method bodies to find
            // anonymous classes used inside methods.
            Statement::Class(class) => {
                Self::find_anonymous_classes_in_members(class.members.iter(), classes, doc_ctx);
            }
            Statement::Interface(iface) => {
                Self::find_anonymous_classes_in_members(iface.members.iter(), classes, doc_ctx);
            }
            Statement::Trait(trait_def) => {
                Self::find_anonymous_classes_in_members(trait_def.members.iter(), classes, doc_ctx);
            }
            Statement::Enum(enum_def) => {
                Self::find_anonymous_classes_in_members(enum_def.members.iter(), classes, doc_ctx);
            }
            Statement::Namespace(ns) => {
                Self::walk_statements_for_anonymous_classes(
                    ns.statements().iter(),
                    classes,
                    doc_ctx,
                );
            }
            Statement::Echo(echo) => {
                for expr in echo.values.iter() {
                    Self::find_anonymous_classes_in_expression(expr, classes, doc_ctx);
                }
            }
            _ => {}
        }
    }

    /// Walk class-like member method bodies to find anonymous classes.
    fn find_anonymous_classes_in_members<'a>(
        members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        for member in members {
            if let ClassLikeMember::Method(method) = member
                && let MethodBody::Concrete(block) = &method.body
            {
                Self::walk_statements_for_anonymous_classes(
                    block.statements.iter(),
                    classes,
                    doc_ctx,
                );
            }
        }
    }

    /// Walk a sequence of statements, dispatching each to the
    /// anonymous-class finder.
    fn walk_statements_for_anonymous_classes<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        for stmt in statements {
            Self::find_anonymous_classes_in_statement(stmt, classes, doc_ctx);
        }
    }

    /// Helper: recurse into an `if` statement body for anonymous classes.
    fn find_anonymous_classes_in_if_body<'a>(
        body: &'a IfBody<'a>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        match body {
            IfBody::Statement(body) => {
                Self::find_anonymous_classes_in_statement(body.statement, classes, doc_ctx);
                for else_if in body.else_if_clauses.iter() {
                    Self::find_anonymous_classes_in_statement(else_if.statement, classes, doc_ctx);
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::find_anonymous_classes_in_statement(
                        else_clause.statement,
                        classes,
                        doc_ctx,
                    );
                }
            }
            IfBody::ColonDelimited(body) => {
                Self::walk_statements_for_anonymous_classes(
                    body.statements.iter(),
                    classes,
                    doc_ctx,
                );
                for else_if in body.else_if_clauses.iter() {
                    Self::walk_statements_for_anonymous_classes(
                        else_if.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::walk_statements_for_anonymous_classes(
                        else_clause.statements.iter(),
                        classes,
                        doc_ctx,
                    );
                }
            }
        }
    }

    /// Recursively walk an expression tree looking for
    /// `Expression::AnonymousClass` nodes.
    fn find_anonymous_classes_in_expression<'a>(
        expr: &'a Expression<'a>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        match expr {
            Expression::AnonymousClass(anon) => {
                let info = Self::extract_anonymous_class_info(anon, doc_ctx);
                classes.push(info);
                // Also recurse into the anonymous class's method bodies
                // to find nested anonymous classes.
                Self::find_anonymous_classes_in_members(anon.members.iter(), classes, doc_ctx);
            }
            Expression::Assignment(assignment) => {
                Self::find_anonymous_classes_in_expression(assignment.lhs, classes, doc_ctx);
                Self::find_anonymous_classes_in_expression(assignment.rhs, classes, doc_ctx);
            }
            Expression::Parenthesized(paren) => {
                Self::find_anonymous_classes_in_expression(paren.expression, classes, doc_ctx);
            }
            Expression::Binary(binary) => {
                Self::find_anonymous_classes_in_expression(binary.lhs, classes, doc_ctx);
                Self::find_anonymous_classes_in_expression(binary.rhs, classes, doc_ctx);
            }
            Expression::UnaryPrefix(unary) => {
                Self::find_anonymous_classes_in_expression(unary.operand, classes, doc_ctx);
            }
            Expression::UnaryPostfix(unary) => {
                Self::find_anonymous_classes_in_expression(unary.operand, classes, doc_ctx);
            }
            Expression::Conditional(cond) => {
                Self::find_anonymous_classes_in_expression(cond.condition, classes, doc_ctx);
                if let Some(then) = &cond.then {
                    Self::find_anonymous_classes_in_expression(then, classes, doc_ctx);
                }
                Self::find_anonymous_classes_in_expression(cond.r#else, classes, doc_ctx);
            }
            Expression::Call(call) => {
                Self::find_anonymous_classes_in_argument_list(
                    call.get_argument_list(),
                    classes,
                    doc_ctx,
                );
                // Also walk the object/class/function expression
                match call {
                    Call::Function(fc) => {
                        Self::find_anonymous_classes_in_expression(fc.function, classes, doc_ctx);
                    }
                    Call::Method(mc) => {
                        Self::find_anonymous_classes_in_expression(mc.object, classes, doc_ctx);
                    }
                    Call::NullSafeMethod(nmc) => {
                        Self::find_anonymous_classes_in_expression(nmc.object, classes, doc_ctx);
                    }
                    Call::StaticMethod(smc) => {
                        Self::find_anonymous_classes_in_expression(smc.class, classes, doc_ctx);
                    }
                }
            }
            Expression::Instantiation(inst) => {
                Self::find_anonymous_classes_in_expression(inst.class, classes, doc_ctx);
                if let Some(args) = &inst.argument_list {
                    Self::find_anonymous_classes_in_argument_list(args, classes, doc_ctx);
                }
            }
            Expression::Throw(throw) => {
                Self::find_anonymous_classes_in_expression(throw.exception, classes, doc_ctx);
            }
            Expression::Clone(clone) => {
                Self::find_anonymous_classes_in_expression(clone.object, classes, doc_ctx);
            }
            Expression::Yield(yld) => match yld {
                Yield::Value(yv) => {
                    if let Some(value) = &yv.value {
                        Self::find_anonymous_classes_in_expression(value, classes, doc_ctx);
                    }
                }
                Yield::Pair(yp) => {
                    Self::find_anonymous_classes_in_expression(yp.key, classes, doc_ctx);
                    Self::find_anonymous_classes_in_expression(yp.value, classes, doc_ctx);
                }
                Yield::From(yf) => {
                    Self::find_anonymous_classes_in_expression(yf.iterator, classes, doc_ctx);
                }
            },
            Expression::Match(match_expr) => {
                Self::find_anonymous_classes_in_expression(match_expr.expression, classes, doc_ctx);
                for arm in match_expr.arms.iter() {
                    let arm_expr = arm.expression();
                    Self::find_anonymous_classes_in_expression(arm_expr, classes, doc_ctx);
                }
            }
            Expression::Array(array) => {
                for element in array.elements.iter() {
                    Self::find_anonymous_classes_in_array_element(element, classes, doc_ctx);
                }
            }
            Expression::LegacyArray(array) => {
                for element in array.elements.iter() {
                    Self::find_anonymous_classes_in_array_element(element, classes, doc_ctx);
                }
            }
            Expression::ArrayAccess(access) => {
                Self::find_anonymous_classes_in_expression(access.array, classes, doc_ctx);
                Self::find_anonymous_classes_in_expression(access.index, classes, doc_ctx);
            }
            Expression::Access(access) => match access {
                Access::Property(pa) => {
                    Self::find_anonymous_classes_in_expression(pa.object, classes, doc_ctx);
                }
                Access::NullSafeProperty(npa) => {
                    Self::find_anonymous_classes_in_expression(npa.object, classes, doc_ctx);
                }
                Access::StaticProperty(spa) => {
                    Self::find_anonymous_classes_in_expression(spa.class, classes, doc_ctx);
                }
                Access::ClassConstant(cca) => {
                    Self::find_anonymous_classes_in_expression(cca.class, classes, doc_ctx);
                }
            },
            Expression::Closure(closure) => {
                Self::walk_statements_for_anonymous_classes(
                    closure.body.statements.iter(),
                    classes,
                    doc_ctx,
                );
            }
            Expression::ArrowFunction(arrow) => {
                Self::find_anonymous_classes_in_expression(arrow.expression, classes, doc_ctx);
            }
            // Terminal expressions that cannot contain anonymous classes.
            Expression::Literal(_)
            | Expression::Variable(_)
            | Expression::Identifier(_)
            | Expression::ConstantAccess(_)
            | Expression::MagicConstant(_)
            | Expression::Parent(_)
            | Expression::Static(_)
            | Expression::Self_(_)
            | Expression::Error(_) => {}
            // Catch-all for less common expression types (Construct,
            // CompositeString, List, Pipe, ArrayAppend, PartialApplication).
            // These rarely contain anonymous classes, but if they do,
            // we'll miss them — acceptable for a first implementation.
            _ => {}
        }
    }

    /// Walk an argument list to find anonymous classes in argument values.
    fn find_anonymous_classes_in_argument_list<'a>(
        args: &'a ArgumentList<'a>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        for arg in args.arguments.iter() {
            let expr = match arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            Self::find_anonymous_classes_in_expression(expr, classes, doc_ctx);
        }
    }

    /// Walk an array element to find anonymous classes in values/keys.
    fn find_anonymous_classes_in_array_element<'a>(
        element: &'a ArrayElement<'a>,
        classes: &mut Vec<ClassInfo>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        match element {
            ArrayElement::KeyValue(kv) => {
                Self::find_anonymous_classes_in_expression(kv.key, classes, doc_ctx);
                Self::find_anonymous_classes_in_expression(kv.value, classes, doc_ctx);
            }
            ArrayElement::Value(v) => {
                Self::find_anonymous_classes_in_expression(v.value, classes, doc_ctx);
            }
            ArrayElement::Variadic(v) => {
                Self::find_anonymous_classes_in_expression(v.value, classes, doc_ctx);
            }
            ArrayElement::Missing(_) => {}
        }
    }

    /// Extract methods, properties, constants, and used trait names from
    /// class-like members.
    ///
    /// This is shared between `Statement::Class`, `Statement::Interface`,
    /// and `Statement::Trait` since all use the same `ClassLikeMember`
    /// representation.
    ///
    /// When `doc_ctx` is provided, PHPDoc `@return` and `@var` tags are used
    /// to refine (or supply) type information for methods and properties.
    pub(crate) fn extract_class_like_members<'a>(
        members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) -> ExtractedMembers {
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut constants = Vec::new();
        let mut used_traits = Vec::new();
        let mut trait_precedences = Vec::new();
        let mut trait_aliases = Vec::new();

        for member in members {
            match member {
                ClassLikeMember::Method(method) => {
                    let name = method.name.value.to_string();
                    let parameters = Self::extract_parameters(&method.parameter_list);
                    let native_return_type = method
                        .return_type_hint
                        .as_ref()
                        .map(|rth| Self::extract_hint_string(&rth.hint));
                    let is_static = method.modifiers.iter().any(|m| m.is_static());
                    let visibility = Self::extract_visibility(method.modifiers.iter());

                    // Look up the PHPDoc `@return` tag (if any) and apply
                    // type override logic.  Also extract PHPStan conditional
                    // return types if present.  Also check for `@deprecated`.
                    // Additionally extract method-level `@template` params
                    // and their `@param` bindings for general template
                    // substitution at call sites.
                    let (
                        return_type,
                        conditional_return,
                        is_deprecated,
                        method_template_params,
                        method_template_bindings,
                    ) = if let Some(ctx) = doc_ctx {
                        let docblock_text =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, method);

                        let doc_type = docblock_text.and_then(docblock::extract_return_type);

                        let effective = docblock::resolve_effective_type(
                            native_return_type.as_deref(),
                            doc_type.as_deref(),
                        );

                        let conditional =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, method)
                                .and_then(docblock::extract_conditional_return_type);

                        // Extract method-level @template params and their
                        // @param bindings for general template substitution.
                        let tpl_params = docblock_text
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
                        // try to synthesize one from method-level @template
                        // annotations.  For example:
                        //   @template T
                        //   @param class-string<T> $class
                        //   @return T
                        // becomes a conditional that resolves T from the
                        // call-site argument (e.g. find(User::class) → User).
                        let conditional = conditional.or_else(|| {
                            let doc = docblock_text?;
                            docblock::synthesize_template_conditional(
                                doc,
                                &tpl_params,
                                effective.as_deref(),
                                false,
                            )
                        });

                        let deprecated = docblock_text.is_some_and(docblock::has_deprecated_tag);

                        (effective, conditional, deprecated, tpl_params, tpl_bindings)
                    } else {
                        (native_return_type, None, false, Vec::new(), Vec::new())
                    };

                    // Extract promoted properties from constructor parameters.
                    // A promoted property is a constructor parameter with a
                    // visibility modifier (e.g. `public`, `private`, `protected`).
                    //
                    // When the constructor has a docblock, `@param` annotations
                    // can provide a more specific type than the native hint
                    // (e.g. `@param list<User> $users` vs native `array $users`).
                    // We apply `resolve_effective_type()` to pick the winner.
                    if name == "__construct" {
                        // Fetch the constructor docblock once for all promoted params.
                        let constructor_docblock = doc_ctx.and_then(|ctx| {
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, method)
                        });

                        for param in method.parameter_list.parameters.iter() {
                            if param.is_promoted_property() {
                                let raw_name = param.variable.name.to_string();
                                let prop_name =
                                    raw_name.strip_prefix('$').unwrap_or(&raw_name).to_string();
                                let native_hint =
                                    param.hint.as_ref().map(|h| Self::extract_hint_string(h));
                                let prop_visibility =
                                    Self::extract_visibility(param.modifiers.iter());

                                // Check for a `@param` docblock annotation
                                // that overrides the native type hint.
                                let type_hint = if let Some(doc) = constructor_docblock {
                                    let param_doc_type =
                                        docblock::extract_param_raw_type(doc, &raw_name);
                                    docblock::resolve_effective_type(
                                        native_hint.as_deref(),
                                        param_doc_type.as_deref(),
                                    )
                                } else {
                                    native_hint
                                };

                                properties.push(PropertyInfo {
                                    name: prop_name,
                                    type_hint,
                                    is_static: false,
                                    visibility: prop_visibility,
                                    is_deprecated: false,
                                });
                            }
                        }
                    }

                    methods.push(MethodInfo {
                        name,
                        parameters,
                        return_type,
                        is_static,
                        visibility,
                        conditional_return,
                        is_deprecated,
                        template_params: method_template_params,
                        template_bindings: method_template_bindings,
                    });
                }
                ClassLikeMember::Property(property) => {
                    let mut prop_infos = Self::extract_property_info(property);

                    // Apply PHPDoc `@var` override and `@deprecated` for each property.
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, member)
                    {
                        let deprecated = docblock::has_deprecated_tag(doc_text);
                        if let Some(doc_type) = docblock::extract_var_type(doc_text) {
                            for prop in &mut prop_infos {
                                prop.type_hint = docblock::resolve_effective_type(
                                    prop.type_hint.as_deref(),
                                    Some(&doc_type),
                                );
                            }
                        }
                        if deprecated {
                            for prop in &mut prop_infos {
                                prop.is_deprecated = true;
                            }
                        }
                    }

                    properties.append(&mut prop_infos);
                }
                ClassLikeMember::Constant(constant) => {
                    let type_hint = constant.hint.as_ref().map(|h| Self::extract_hint_string(h));
                    let visibility = Self::extract_visibility(constant.modifiers.iter());
                    let is_deprecated = if let Some(ctx) = doc_ctx {
                        docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, member)
                            .is_some_and(docblock::has_deprecated_tag)
                    } else {
                        false
                    };
                    for item in constant.items.iter() {
                        constants.push(ConstantInfo {
                            name: item.name.value.to_string(),
                            type_hint: type_hint.clone(),
                            visibility,
                            is_deprecated,
                        });
                    }
                }
                ClassLikeMember::EnumCase(enum_case) => {
                    let case_name = enum_case.item.name().value.to_string();
                    constants.push(ConstantInfo {
                        name: case_name,
                        type_hint: None,
                        visibility: Visibility::Public,
                        is_deprecated: false,
                    });
                }
                ClassLikeMember::TraitUse(trait_use) => {
                    for trait_name_ident in trait_use.trait_names.iter() {
                        used_traits.push(trait_name_ident.value().to_string());
                    }

                    // Parse trait adaptation block (`{ ... }`) if present.
                    // This handles `insteadof` (precedence) and `as` (alias)
                    // declarations for resolving trait method conflicts.
                    if let TraitUseSpecification::Concrete(spec) = &trait_use.specification {
                        for adaptation in spec.adaptations.iter() {
                            match adaptation {
                                TraitUseAdaptation::Precedence(prec) => {
                                    let trait_name =
                                        prec.method_reference.trait_name.value().to_string();
                                    let method_name =
                                        prec.method_reference.method_name.value.to_string();
                                    let insteadof: Vec<String> = prec
                                        .trait_names
                                        .iter()
                                        .map(|id| id.value().to_string())
                                        .collect();
                                    trait_precedences.push(TraitPrecedence {
                                        trait_name,
                                        method_name,
                                        insteadof,
                                    });
                                }
                                TraitUseAdaptation::Alias(alias_adapt) => {
                                    let (trait_name, method_name) =
                                        match &alias_adapt.method_reference {
                                            TraitUseMethodReference::Identifier(ident) => {
                                                (None, ident.value.to_string())
                                            }
                                            TraitUseMethodReference::Absolute(abs) => (
                                                Some(abs.trait_name.value().to_string()),
                                                abs.method_name.value.to_string(),
                                            ),
                                        };
                                    let alias =
                                        alias_adapt.alias.as_ref().map(|a| a.value.to_string());
                                    let visibility = alias_adapt.visibility.as_ref().map(|m| {
                                        if m.is_private() {
                                            Visibility::Private
                                        } else if m.is_protected() {
                                            Visibility::Protected
                                        } else {
                                            Visibility::Public
                                        }
                                    });
                                    trait_aliases.push(TraitAlias {
                                        trait_name,
                                        method_name,
                                        alias,
                                        visibility,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        (
            methods,
            properties,
            constants,
            used_traits,
            trait_precedences,
            trait_aliases,
        )
    }
}
