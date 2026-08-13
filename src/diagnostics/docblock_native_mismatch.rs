//! Diagnostics for a docblock type that contradicts its native type hint.
//!
//! A `@param` or `@return` annotation is there to *refine* the native
//! declaration, not to disagree with it.  When the native hint admits `null`
//! and the annotation does not, the two describe different sets of values:
//!
//! ```php
//! /** @param string $name */
//! function greet(?string $name): void {}
//! ```
//!
//! A caller reading the signature may legally pass `null`, and the callee has
//! been promised it never receives one.  Whichever of the two is wrong, they
//! cannot both be right, so the declaration is flagged.
//!
//! Only annotations whose nullability the type expression itself settles are
//! checked.  A bare class-like name is not one of them: `@param T $x` may be a
//! `@template` parameter and `@param UserId $x` may be an imported
//! `@psalm-type` alias, either of which can resolve to a nullable type.

use mago_span::HasSpan;
use mago_syntax::cst::class_like::member::ClassLikeMember;
use mago_syntax::cst::declare::DeclareBody;
use mago_syntax::cst::function_like::parameter::FunctionLikeParameterList;
use mago_syntax::cst::function_like::r#return::FunctionLikeReturnTypeHint;
use mago_syntax::cst::sequence::Sequence;
use mago_syntax::cst::statement::Statement;
use mago_syntax::cst::*;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::atom::bytes_to_str;
use crate::docblock::{
    DocblockInfo, extract_param_raw_type_from_info, extract_return_type_from_info,
    get_docblock_info_for_node,
};
use crate::parser::{extract_hint_type, with_parsed_program};
use crate::php_type::{PhpType, TypeKind, is_keyword_type};

use super::helpers::make_diagnostic;

/// Diagnostic code used for docblock/native type-hint contradictions.
pub(crate) const DOCBLOCK_NATIVE_MISMATCH_CODE: &str = "docblock_native_mismatch";

/// One flagged annotation: the source range to underline and the message.
struct Finding {
    start: usize,
    end: usize,
    message: String,
}

/// The program trivia and source text needed to look up a node's docblock.
struct Ctx<'a> {
    trivia: &'a [Trivia<'a>],
    content: &'a str,
}

impl Backend {
    /// Flag every `@param`/`@return` annotation that denies `null` where the
    /// native type hint it annotates admits it.
    pub fn collect_docblock_native_mismatch_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        let findings = with_parsed_program(content, "docblock_native_mismatch", |program, _| {
            let ctx = Ctx {
                trivia: program.trivia.as_slice(),
                content,
            };
            let mut findings: Vec<Finding> = Vec::new();
            for stmt in program.statements.iter() {
                walk_statement(stmt, &ctx, &mut findings);
            }
            findings
        });

        for finding in findings {
            if let Some(range) =
                self.offset_range_to_lsp_range(uri, content, finding.start, finding.end)
            {
                out.push(make_diagnostic(
                    range,
                    DiagnosticSeverity::WARNING,
                    DOCBLOCK_NATIVE_MISMATCH_CODE,
                    finding.message,
                ));
            }
        }
    }
}

/// Descend through the statement forms that can hold a declaration.
fn walk_statement(stmt: &Statement<'_>, ctx: &Ctx<'_>, out: &mut Vec<Finding>) {
    match stmt {
        Statement::Namespace(ns) => {
            for inner in ns.statements().iter() {
                walk_statement(inner, ctx, out);
            }
        }
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                walk_statement(inner, ctx, out);
            }
        }
        Statement::Declare(declare) => match &declare.body {
            DeclareBody::Statement(inner) => walk_statement(inner, ctx, out),
            DeclareBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    walk_statement(inner, ctx, out);
                }
            }
        },
        Statement::Class(class) => walk_members(&class.members, ctx, out),
        Statement::Interface(interface) => walk_members(&interface.members, ctx, out),
        Statement::Trait(trait_def) => walk_members(&trait_def.members, ctx, out),
        Statement::Enum(enum_def) => walk_members(&enum_def.members, ctx, out),
        Statement::Function(func) => check_declaration(
            func,
            &func.parameter_list,
            func.return_type_hint.as_ref(),
            ctx,
            out,
        ),
        _ => {}
    }
}

fn walk_members<'arena>(
    members: &Sequence<'arena, ClassLikeMember<'arena>>,
    ctx: &Ctx<'_>,
    out: &mut Vec<Finding>,
) {
    for member in members.iter() {
        if let ClassLikeMember::Method(method) = member {
            check_declaration(
                method,
                &method.parameter_list,
                method.return_type_hint.as_ref(),
                ctx,
                out,
            );
        }
    }
}

/// Compare every annotated parameter and the return type of one declaration
/// against the docblock immediately above it.
fn check_declaration(
    node: &impl HasSpan,
    parameters: &FunctionLikeParameterList<'_>,
    return_type_hint: Option<&FunctionLikeReturnTypeHint<'_>>,
    ctx: &Ctx<'_>,
    out: &mut Vec<Finding>,
) {
    let Some(info) = get_docblock_info_for_node(ctx.trivia, ctx.content, node) else {
        return;
    };

    for param in parameters.parameters.iter() {
        let Some(hint) = param.hint.as_ref() else {
            continue;
        };
        let name = bytes_to_str(param.variable.name);
        let native = extract_hint_type(hint);
        let Some(documented) = extract_param_raw_type_from_info(&info, name) else {
            continue;
        };
        if !denies_null_the_native_hint_admits(&documented, &native) {
            continue;
        }
        out.push(Finding {
            start: hint.span().start.offset as usize,
            end: param.variable.span.end.offset as usize,
            message: format!(
                "Documented type '{}' for {} does not accept null, but the native type hint '{}' does",
                documented, name, native
            ),
        });
    }

    check_return(&info, return_type_hint, out);
}

fn check_return(
    info: &DocblockInfo,
    return_type_hint: Option<&FunctionLikeReturnTypeHint<'_>>,
    out: &mut Vec<Finding>,
) {
    let Some(hint) = return_type_hint else {
        return;
    };
    let native = extract_hint_type(&hint.hint);
    let Some(documented) = extract_return_type_from_info(info) else {
        return;
    };
    if !denies_null_the_native_hint_admits(&documented, &native) {
        return;
    }
    let span = hint.hint.span();
    out.push(Finding {
        start: span.start.offset as usize,
        end: span.end.offset as usize,
        message: format!(
            "Documented return type '{}' does not accept null, but the native type hint '{}' does",
            documented, native
        ),
    });
}

/// Whether `documented` rules out a `null` that `native` allows.
fn denies_null_the_native_hint_admits(documented: &PhpType, native: &PhpType) -> bool {
    // `mixed` admits null without ever spelling it, so narrowing it through a
    // docblock is the annotation doing its job rather than a contradiction.
    // This mirrors the effective-type merge in `resolve_effective_type_typed`.
    if !native.accepts_null() || native.is_mixed() || native.non_null_type().is_none() {
        return false;
    }

    !documented.accepts_null() && nullability_is_decidable(documented)
}

/// Whether the type expression itself settles the question "does this admit
/// null?".
///
/// A bare class-like name does not: it may be a `@template` parameter or a
/// `@psalm-type` alias standing in for a nullable type.  Everything the
/// keyword vocabulary spells (`string`, `list<int>`, `array{...}`, literals,
/// `static`, …) does, and so does every parameterised or structural form,
/// none of which can be null whatever its arguments resolve to.
fn nullability_is_decidable(ty: &PhpType) -> bool {
    match ty.kind() {
        TypeKind::Named(name) => is_keyword_type(name),
        TypeKind::Nullable(inner) => nullability_is_decidable(inner),
        TypeKind::Union(members) | TypeKind::Intersection(members) => {
            members.iter().all(nullability_is_decidable)
        }
        // A type expression that was never evaluated (its operand was a
        // template that went unsubstituted, or a constant we could not read)
        // names no set of values, so it cannot be asked about null either.
        TypeKind::Raw(_)
        | TypeKind::Conditional(_)
        | TypeKind::KeyOf(_)
        | TypeKind::ValueOf(_)
        | TypeKind::IndexAccess(..) => false,
        _ => true,
    }
}
