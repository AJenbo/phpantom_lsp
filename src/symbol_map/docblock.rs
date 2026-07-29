//! Docblock symbol extraction helpers for the symbol map.
//!
//! This module scans PHPDoc comment blocks for the symbols the symbol map
//! cares about — type references in `@param`, `@return`, `@var`, `@extends`
//! and friends, the members `@method` and `@property` declare, `@template`
//! parameters, `@see` references, and the `$variables` a docblock names — and
//! emits [`SymbolSpan`] entries with file-level byte offsets.
//!
//! It works from the PHPDoc CST produced by `mago-phpdoc-syntax`, via
//! [`with_docblock_cst`].  The parser is anchored at the docblock's own
//! position in the file, so every type, identifier and variable node already
//! carries the offset we need, including nodes buried inside a type written
//! across continuation lines.  Nothing here re-derives tag structure from the
//! raw text: the grammar has already split type from variable from prose.

use mago_database::file::FileId;
use mago_phpdoc_syntax::cst::r#type as type_ast;
use mago_phpdoc_syntax::cst::{
    AssertPattern, Element, MethodTagValue, PropertyTagValue, Tag, TagValue,
    TemplateTagValue as CstTemplateTagValue, TemplateTagValueVariance, Text, Variable,
};
use mago_span::{HasSpan, Position, Span};
use mago_syntax::cst::*;

use crate::docblock::{TagKind, tag_kind};

use crate::docblock::parser::{type_text, value_span, with_docblock_cst};
use crate::php_type::PhpType;
use crate::types::TemplateVariance;

use super::{
    ClassRefContext, SelfStaticParentKind, SubjectText, SymbolKind, SymbolSpan,
    self_static_parent_kind,
};
use crate::util::strip_fqn_prefix;

// ─── Navigability filter ────────────────────────────────────────────────────

/// Returns `true` when a type name refers to a class/interface that the
/// user should be able to navigate to.
///
/// Uses simple string splitting instead of `PhpType::parse()` + `base_name()`
/// because this is called for every type span during symbol-map extraction
/// and must stay allocation-free.
pub(crate) fn is_navigable_type(name: &str) -> bool {
    let base = name.split('<').next().unwrap_or(name);
    let base = base.split('{').next().unwrap_or(base);
    let base = base.trim();
    if base.is_empty() {
        return false;
    }
    !crate::php_type::is_keyword_type(base)
}

// ─── Span construction helpers ──────────────────────────────────────────────

/// Construct a `ClassReference` `SymbolSpan` from a raw identifier string.
///
/// Detects whether the name is fully-qualified (leading `\`) and sets
/// `is_fqn` accordingly.  The leading `\` is stripped from the stored
/// `name` in all cases.
pub(super) fn class_ref_span(start: u32, end: u32, raw_name: &str) -> SymbolSpan {
    let is_fqn = raw_name.starts_with('\\');
    let name = crate::atom::atom(strip_fqn_prefix(raw_name));
    SymbolSpan {
        start,
        end,
        kind: SymbolKind::ClassReference {
            name,
            is_fqn,
            context: ClassRefContext::Other,
        },
    }
}

/// Like [`class_ref_span`] but with an explicit [`ClassRefContext`].
pub(super) fn class_ref_span_ctx(
    start: u32,
    end: u32,
    raw_name: &str,
    ctx: ClassRefContext,
) -> SymbolSpan {
    let is_fqn = raw_name.starts_with('\\');
    let name = crate::atom::atom(strip_fqn_prefix(raw_name));
    SymbolSpan {
        start,
        end,
        kind: SymbolKind::ClassReference {
            name,
            is_fqn,
            context: ctx,
        },
    }
}

// ─── Docblock text retrieval ────────────────────────────────────────────────

/// Like [`crate::docblock::get_docblock_text_for_node`] but also returns
/// the byte offset of the `/**` opening within the file.
pub fn get_docblock_text_with_offset<'a>(
    trivia: &'a [Trivia<'a>],
    content: &str,
    node: &impl HasSpan,
) -> Option<(&'a str, u32)> {
    use crate::atom::bytes_to_str;
    let node_start = node.span().start.offset;
    let candidate_idx = trivia.partition_point(|t| t.span.start.offset < node_start);
    if candidate_idx == 0 {
        return None;
    }

    let content_bytes = content.as_bytes();
    let mut covered_from = node_start;

    for i in (0..candidate_idx).rev() {
        let t = &trivia[i];
        let t_end = t.span.end.offset;

        let gap = content_bytes
            .get(t_end as usize..covered_from as usize)
            .unwrap_or(&[]);
        if !gap.iter().all(u8::is_ascii_whitespace) {
            return None;
        }

        match t.kind {
            TriviaKind::DocBlockComment => {
                return Some((bytes_to_str(t.value), t.span.start.offset));
            }
            TriviaKind::WhiteSpace
            | TriviaKind::SingleLineComment
            | TriviaKind::MultiLineComment
            | TriviaKind::HashComment => {
                covered_from = t.span.start.offset;
            }
        }
    }

    None
}

// ─── Docblock tag scanning ──────────────────────────────────────────────────

/// What a docblock declares beyond the [`SymbolSpan`] entries that
/// [`extract_docblock_symbols`] pushes directly.
#[derive(Debug, Default)]
pub(super) struct DocblockSymbols {
    /// `@template` parameter definitions, as
    /// `(name, offset of the name token, bound, variance)`.
    pub templates: Vec<(String, u32, Option<PhpType>, TemplateVariance)>,
    /// The parameters this docblock names: the variable of a `@param` tag, and
    /// the subject of any conditional type (`$strict` in
    /// `($strict is true ? A : B)`).
    ///
    /// Each entry is `(name_without_dollar, offset_of_the_dollar)`.  Callers
    /// turn them into [`SymbolKind::Variable`] spans and `DocblockParam`
    /// definition sites, so rename and find-references cover parameter names
    /// mentioned in docblocks.
    pub param_vars: Vec<(String, u32)>,
    /// The variables an inline `@var Type $name` declares, in the same shape
    /// as [`Self::param_vars`].
    pub var_vars: Vec<(String, u32)>,
}

/// Where a docblock walk writes what it finds.
struct DocblockSink<'a> {
    spans: &'a mut Vec<SymbolSpan>,
    found: &'a mut DocblockSymbols,
}

/// Scan a docblock for the symbols the symbol map needs and emit
/// `SymbolSpan` entries with file-level byte offsets.
pub(super) fn extract_docblock_symbols(
    docblock: &str,
    base_offset: u32,
    spans: &mut Vec<SymbolSpan>,
) -> DocblockSymbols {
    // Inline `{@see ...}` references sit in free text rather than in a tag
    // value of their own, so they are found by scanning the raw docblock.
    extract_inline_see_symbols(docblock, base_offset, spans);

    let mut found = DocblockSymbols::default();
    let mut sink = DocblockSink {
        spans,
        found: &mut found,
    };

    // Tags whose `static` modifier has to be blanked out before the grammar
    // will read them, as `(element index, offset of the keyword)`.
    let mut recoveries: Vec<(usize, usize)> = Vec::new();

    with_docblock_cst(docblock, docblock_span(docblock, base_offset), |document| {
        for (index, element) in document.elements.iter().enumerate() {
            let Element::Tag(tag) = element else { continue };
            if let Some(keyword) = emit_tag_symbols(tag, docblock, base_offset, &mut sink) {
                recoveries.push((index, keyword));
            }
        }
    });

    if !recoveries.is_empty() {
        recover_static_method_tags(docblock, base_offset, &recoveries, &mut sink);
    }

    found
}

/// The span a docblock occupies in the file, which anchors the PHPDoc parser
/// so that every node it produces reports a file offset.
fn docblock_span(docblock: &str, base_offset: u32) -> Span {
    Span::new(
        FileId::zero(),
        Position::new(base_offset),
        Position::new(base_offset + docblock.len() as u32),
    )
}

/// Emit the symbols one tag declares.
///
/// Returns the docblock-relative offset of a `static` modifier that has to be
/// blanked out before the tag can be read; see [`recover_static_method_tags`].
/// Tags the grammar could not parse at all yield nothing: their type text is
/// not a type, so guessing at a class name from it would only produce a
/// reference that resolves to nothing.
fn emit_tag_symbols(
    tag: &Tag<'_>,
    docblock: &str,
    base_offset: u32,
    sink: &mut DocblockSink<'_>,
) -> Option<usize> {
    match &tag.value {
        // ── Tags that lead with a type ──────────────────────────────
        TagValue::Param(value) => {
            emit_type_symbols(value.r#type, sink);
            if let Some(parameter) = value.parameter {
                push_variable_reference(&parameter, &mut sink.found.param_vars);
            }
        }
        TagValue::TypelessParam(value) => {
            push_variable_reference(&value.parameter, &mut sink.found.param_vars);
        }
        TagValue::Return(value) | TagValue::RealReturn(value) => {
            emit_type_symbols(value.r#type, sink);
        }
        TagValue::Var(value) => {
            emit_type_symbols(value.r#type, sink);
            if let Some(variable) = value.variable {
                push_variable_reference(&variable, &mut sink.found.var_vars);
            }
        }
        TagValue::Throws(value) => emit_type_symbols(value.r#type, sink),
        TagValue::Mixin(value) => emit_type_symbols(value.r#type, sink),
        TagValue::Extends(value) => emit_type_symbols(value.r#type, sink),
        TagValue::Implements(value) => emit_type_symbols(value.r#type, sink),
        TagValue::Use(value) => emit_type_symbols(value.r#type, sink),
        TagValue::RequireExtends(value) => emit_type_symbols(value.r#type, sink),
        TagValue::RequireImplements(value) => emit_type_symbols(value.r#type, sink),
        TagValue::Sealed(value) => emit_type_symbols(value.r#type, sink),
        TagValue::Assert(value)
        | TagValue::AssertIfTrue(value)
        | TagValue::AssertIfFalse(value) => {
            if let AssertPattern::Type(asserted) = value.pattern {
                emit_type_symbols(asserted, sink);
            }
        }

        // ── Tags that declare a member or a template parameter ──────
        TagValue::Method(value) => emit_method_tag_symbols(value, value.is_static(), sink),
        TagValue::Property(value)
        | TagValue::PropertyRead(value)
        | TagValue::PropertyWrite(value) => emit_property_tag_symbols(value, sink),
        TagValue::Template(value) => emit_template_tag_symbols(value, docblock, base_offset, sink),

        // ── Tags the grammar keeps as free text ─────────────────────
        TagValue::Generic(text) if tag_kind(tag) == TagKind::See => {
            emit_see_tag_symbol(&text.value, docblock, base_offset, sink.spans);
        }
        TagValue::Invalid(_) if tag_kind(tag) == TagKind::Method => {
            return static_modifier_offset(tag, docblock, base_offset);
        }
        _ => {}
    }

    None
}

/// The `static` modifier of a `@method` tag, and a blank of the same width.
///
/// Overwriting the keyword rather than removing it keeps every following byte
/// at its original offset; see [`recover_static_method_tags`].
const STATIC_MODIFIER: &str = "static";
const BLANKED_MODIFIER: &str = "      ";
const _: () = assert!(STATIC_MODIFIER.len() == BLANKED_MODIFIER.len());

/// The docblock-relative offset of a `static` modifier leading a tag value.
fn static_modifier_offset(tag: &Tag<'_>, docblock: &str, base_offset: u32) -> Option<usize> {
    let value_start = value_span(tag, docblock, base_offset)
        .start
        .offset
        .saturating_sub(base_offset) as usize;
    let rest = docblock.get(value_start..)?.strip_prefix(STATIC_MODIFIER)?;

    rest.starts_with(char::is_whitespace).then_some(value_start)
}

/// Re-emit the `@method` tags the PHPDoc grammar rejected over their `static`
/// modifier.
///
/// `mago-phpdoc-syntax` cannot tell `@method static (…) name()` from a method
/// literally called `static` whose parameter list follows, so a parenthesised
/// return type after the modifier makes the whole tag unparseable.
/// `docblock::virtual_members` recovers the signature by re-parsing the tag
/// without the modifier; the symbol map needs the signature *and* its
/// original offsets, so the keyword is blanked out instead of removed and the
/// recovered CST still reports file-accurate spans.  Blanking cannot merge or
/// split tags, so the element indices recorded during the first walk still
/// identify the same tags.
fn recover_static_method_tags(
    docblock: &str,
    base_offset: u32,
    recoveries: &[(usize, usize)],
    sink: &mut DocblockSink<'_>,
) {
    let mut patched = String::from(docblock);
    for &(_, keyword) in recoveries {
        patched.replace_range(keyword..keyword + STATIC_MODIFIER.len(), BLANKED_MODIFIER);
    }

    with_docblock_cst(&patched, docblock_span(&patched, base_offset), |document| {
        for (index, element) in document.elements.iter().enumerate() {
            if !recoveries.iter().any(|&(recovered, _)| recovered == index) {
                continue;
            }
            let Element::Tag(tag) = element else { continue };
            if let TagValue::Method(value) = &tag.value {
                emit_method_tag_symbols(value, true, sink);
            }
        }
    });
}

/// Emit the return type, name and parameter types of a `@method` tag.
///
/// `is_static` is passed in rather than read off `value` so a tag recovered by
/// [`recover_static_method_tags`], whose modifier has been blanked out, still
/// declares a static member.
fn emit_method_tag_symbols(
    value: &MethodTagValue<'_>,
    is_static: bool,
    sink: &mut DocblockSink<'_>,
) {
    if let Some(return_type) = value.return_type {
        emit_type_symbols(return_type, sink);
    }

    sink.spans.push(SymbolSpan {
        start: value.name.span.start.offset,
        end: value.name.span.end.offset,
        kind: SymbolKind::MemberDeclaration {
            name: crate::atom::atom_bytes(value.name.value),
            is_static,
        },
    });

    if let Some(templates) = value.templates {
        for entry in templates.entries.iter() {
            if let Some(bound) = entry.template.bound {
                emit_type_symbols(bound.r#type, sink);
            }
        }
    }

    for parameter in value.parameters.entries.iter() {
        if let Some(declared) = parameter.r#type {
            emit_type_symbols(declared, sink);
        }
    }
}

/// Emit the type and the member name of a `@property` tag (or one of its
/// `-read` / `-write` variants).
fn emit_property_tag_symbols(value: &PropertyTagValue<'_>, sink: &mut DocblockSink<'_>) {
    if let Some(declared) = value.r#type {
        emit_type_symbols(declared, sink);
    }

    let Some((name, dollar_offset)) = variable_name_and_offset(&value.variable) else {
        return;
    };
    sink.spans.push(SymbolSpan {
        start: dollar_offset + 1,
        end: dollar_offset + 1 + name.len() as u32,
        kind: SymbolKind::MemberDeclaration {
            name: crate::atom::atom(name),
            is_static: false,
        },
    });
}

/// Record a `@template` declaration and emit the spans of its bound.
fn emit_template_tag_symbols(
    value: &CstTemplateTagValue<'_>,
    docblock: &str,
    base_offset: u32,
    sink: &mut DocblockSink<'_>,
) {
    let bound = value.bound.map(|bound| {
        emit_type_symbols(bound.r#type, sink);
        PhpType::parse(&type_text(docblock, base_offset, bound.r#type.span()))
    });

    let variance = match value.variance {
        TemplateTagValueVariance::Invariant => TemplateVariance::Invariant,
        TemplateTagValueVariance::Covariant => TemplateVariance::Covariant,
        TemplateTagValueVariance::Contravariant => TemplateVariance::Contravariant,
    };

    sink.found.templates.push((
        crate::atom::bytes_to_str(value.name.value).to_owned(),
        value.name.span.start.offset,
        bound,
        variance,
    ));
}

/// Emit the symbol an `@see` tag references.
///
/// The PHPDoc grammar keeps `@see` as free text, so the reference is the first
/// whitespace-delimited token of it; the text node supplies the file offset.
fn emit_see_tag_symbol(
    text: &Text<'_>,
    docblock: &str,
    base_offset: u32,
    spans: &mut Vec<SymbolSpan>,
) {
    let start = text.span.start.offset.saturating_sub(base_offset) as usize;
    let end = (text.span.end.offset.saturating_sub(base_offset) as usize).min(docblock.len());
    let Some(raw) = docblock.get(start..end) else {
        return;
    };
    let trimmed = raw.trim_start();
    let Some(reference) = trimmed.split_whitespace().next() else {
        return;
    };

    let offset = text.span.start.offset + (raw.len() - trimmed.len()) as u32;
    emit_see_reference(reference, offset, spans);
}

/// The name and `$` offset of a variable token, with the `$` stripped from the
/// name.
fn variable_name_and_offset<'a>(variable: &Variable<'a>) -> Option<(&'a str, u32)> {
    let name = crate::atom::bytes_to_str(variable.value).strip_prefix('$')?;

    (!name.is_empty()).then_some((name, variable.span.start.offset))
}

/// Record a reference to a `$variable` named in a docblock.
fn push_variable_reference(variable: &Variable<'_>, into: &mut Vec<(String, u32)>) {
    if let Some((name, dollar_offset)) = variable_name_and_offset(variable) {
        into.push((name.to_owned(), dollar_offset));
    }
}

// ─── Type span emission ─────────────────────────────────────────────────────

/// Walk a PHPDoc type node and emit [`SymbolSpan`] entries for every navigable
/// type reference (class names, `self`, `static`, `parent`, `$this`), plus a
/// reference for the `$parameter` a conditional type is keyed on.
///
/// Node spans are already file offsets, so nothing has to be adjusted here.
fn emit_type_symbols(ty: &type_ast::Type<'_>, sink: &mut DocblockSink<'_>) {
    use crate::atom::bytes_to_str;
    match ty {
        // ── Composite types ─────────────────────────────────────────
        type_ast::Type::Union(u) => {
            emit_type_symbols(u.left, sink);
            emit_type_symbols(u.right, sink);
        }
        type_ast::Type::Intersection(i) => {
            emit_type_symbols(i.left, sink);
            emit_type_symbols(i.right, sink);
        }
        type_ast::Type::Nullable(n) => {
            emit_type_symbols(n.inner, sink);
        }
        type_ast::Type::Parenthesized(p) => {
            emit_type_symbols(p.inner, sink);
        }

        // ── Named / Reference types ─────────────────────────────────
        type_ast::Type::Reference(r) => {
            let name = crate::php_type::reference_kind_name(&r.kind);
            let id_span = r.kind.span();
            let id_start = id_span.start.offset;
            let id_end = id_span.end.offset;

            // Emit a span for the identifier itself.
            emit_identifier_span(name, id_start, id_end, sink.spans);

            // Recurse into generic parameters if present.
            if let Some(params) = &r.parameters {
                emit_generic_params(params, sink);
            }
        }

        // ── Array-like types with optional generic parameters ───────
        type_ast::Type::Array(a) => {
            if let Some(params) = &a.parameters {
                emit_generic_params(params, sink);
            }
        }
        type_ast::Type::NonEmptyArray(a) => {
            if let Some(params) = &a.parameters {
                emit_generic_params(params, sink);
            }
        }
        type_ast::Type::AssociativeArray(a) => {
            if let Some(params) = &a.parameters {
                emit_generic_params(params, sink);
            }
        }
        type_ast::Type::List(l) => {
            if let Some(params) = &l.parameters {
                emit_generic_params(params, sink);
            }
        }
        type_ast::Type::NonEmptyList(l) => {
            if let Some(params) = &l.parameters {
                emit_generic_params(params, sink);
            }
        }
        type_ast::Type::Iterable(i) => {
            if let Some(params) = &i.parameters {
                emit_generic_params(params, sink);
            }
        }

        // ── Slice: T[] ──────────────────────────────────────────────
        type_ast::Type::Slice(s) => {
            emit_type_symbols(s.inner, sink);
        }

        // ── Shape types ─────────────────────────────────────────────
        type_ast::Type::Shape(s) => {
            for field in &s.fields {
                emit_type_symbols(field.value, sink);
            }
        }

        // ── Object type (with optional shape) ───────────────────────
        type_ast::Type::Object(o) => {
            if let Some(props) = &o.properties {
                for field in &props.fields {
                    emit_type_symbols(field.value, sink);
                }
            }
        }

        // ── Callable types ──────────────────────────────────────────
        type_ast::Type::Callable(c) => {
            // Emit span for the callable keyword if it's navigable
            // (e.g. `Closure` is a class, `callable` is not).
            let kw_name = bytes_to_str(c.keyword.value);
            let kw_start = c.keyword.span.start.offset;
            let kw_end = c.keyword.span.end.offset;
            emit_identifier_span(kw_name, kw_start, kw_end, sink.spans);

            // Recurse into parameter types and return type.
            if let Some(spec) = &c.specification {
                for param in &spec.parameters.entries {
                    if let Some(param_type) = &param.parameter_type {
                        emit_type_symbols(param_type, sink);
                    }
                }
                if let Some(ret) = &spec.return_type {
                    emit_type_symbols(ret.return_type, sink);
                }
            }
        }

        // ── Conditional types ───────────────────────────────────────
        type_ast::Type::Conditional(c) => {
            // `($strict is true ? A : B)` keys the type on a parameter, so the
            // subject is a reference that has to be renamed with it.
            if let type_ast::Type::Variable(subject) = c.subject {
                push_variable_reference(subject, &mut sink.found.param_vars);
            }
            emit_type_symbols(c.target, sink);
            emit_type_symbols(c.then, sink);
            emit_type_symbols(c.r#else, sink);
        }

        // ── class-string / interface-string / enum-string / trait-string ─
        type_ast::Type::ClassString(c) => {
            if let Some(param) = &c.parameter {
                emit_type_symbols(&param.entry.inner, sink);
            }
        }
        type_ast::Type::InterfaceString(i) => {
            if let Some(param) = &i.parameter {
                emit_type_symbols(&param.entry.inner, sink);
            }
        }
        type_ast::Type::EnumString(e) => {
            if let Some(param) = &e.parameter {
                emit_type_symbols(&param.entry.inner, sink);
            }
        }
        type_ast::Type::TraitString(t) => {
            if let Some(param) = &t.parameter {
                emit_type_symbols(&param.entry.inner, sink);
            }
        }

        // ── key-of / value-of ───────────────────────────────────────
        type_ast::Type::KeyOf(k) => {
            emit_type_symbols(&k.parameter.entry.inner, sink);
        }
        type_ast::Type::ValueOf(v) => {
            emit_type_symbols(&v.parameter.entry.inner, sink);
        }

        // ── Index access: T[K] ─────────────────────────────────────
        type_ast::Type::IndexAccess(i) => {
            emit_type_symbols(i.target, sink);
            emit_type_symbols(i.index, sink);
        }

        // ── int-mask / int-mask-of ──────────────────────────────────
        type_ast::Type::IntMask(m) => {
            for entry in &m.parameters.entries {
                emit_type_symbols(&entry.inner, sink);
            }
        }
        type_ast::Type::IntMaskOf(m) => {
            emit_type_symbols(&m.parameter.entry.inner, sink);
        }

        // ── properties-of ───────────────────────────────────────────
        type_ast::Type::PropertiesOf(p) => {
            emit_type_symbols(&p.parameter.entry.inner, sink);
        }

        // ── Negated / Posited literals ──────────────────────────────
        type_ast::Type::Negated(_) | type_ast::Type::Posited(_) => {
            // Numeric literals — not navigable.
        }

        // ── Variable ($this) ────────────────────────────────────────
        type_ast::Type::ThisVariable(v) => {
            let start = v.span.start.offset;
            let end = v.span.end.offset;
            sink.spans.push(SymbolSpan {
                start,
                end,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::This),
            });
        }
        type_ast::Type::Variable(v) if v.value == b"$this" => {
            let start = v.span.start.offset;
            let end = v.span.end.offset;
            sink.spans.push(SymbolSpan {
                start,
                end,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::This),
            });
        }
        // Other variables (parameter names leaked from @param) are skipped.
        type_ast::Type::Variable(_) => {}

        // ── Member / Alias references ───────────────────────────────
        type_ast::Type::MemberReference(_) | type_ast::Type::AliasReference(_) => {
            // These are rare PHPStan types — not navigable in our system.
        }

        // ── Keyword types (int, string, bool, void, etc.) ───────────
        // All keyword types are non-navigable *except* `static`, `self`,
        // and `parent` which should produce SelfStaticParent spans.
        type_ast::Type::Mixed(k)
        | type_ast::Type::NonEmptyMixed(k)
        | type_ast::Type::Null(k)
        | type_ast::Type::Void(k)
        | type_ast::Type::Never(k)
        | type_ast::Type::Resource(k)
        | type_ast::Type::ClosedResource(k)
        | type_ast::Type::OpenResource(k)
        | type_ast::Type::True(k)
        | type_ast::Type::False(k)
        | type_ast::Type::Bool(k)
        | type_ast::Type::Float(k)
        | type_ast::Type::Int(k)
        | type_ast::Type::PositiveInt(k)
        | type_ast::Type::NegativeInt(k)
        | type_ast::Type::NonPositiveInt(k)
        | type_ast::Type::NonNegativeInt(k)
        | type_ast::Type::String(k)
        | type_ast::Type::StringableObject(k)
        | type_ast::Type::ArrayKey(k)
        | type_ast::Type::Numeric(k)
        | type_ast::Type::Scalar(k)
        | type_ast::Type::NumericString(k)
        | type_ast::Type::NonEmptyString(k)
        | type_ast::Type::NonEmptyLowercaseString(k)
        | type_ast::Type::LowercaseString(k)
        | type_ast::Type::NonEmptyUppercaseString(k)
        | type_ast::Type::UppercaseString(k)
        | type_ast::Type::TruthyString(k)
        | type_ast::Type::NonFalsyString(k)
        | type_ast::Type::UnspecifiedLiteralInt(k)
        | type_ast::Type::UnspecifiedLiteralString(k)
        | type_ast::Type::UnspecifiedLiteralFloat(k)
        | type_ast::Type::NonEmptyUnspecifiedLiteralString(k) => {
            // `static`, `self`, and `parent` are parsed as keywords by
            // mago but should still produce SelfStaticParent spans.
            let name = bytes_to_str(k.value);
            if let Some(ssp_kind) = self_static_parent_kind(name) {
                let start = k.span.start.offset;
                let end = k.span.end.offset;
                sink.spans.push(SymbolSpan {
                    start,
                    end,
                    kind: SymbolKind::SelfStaticParent(ssp_kind),
                });
            }
            // All other keywords (int, string, void, etc.) are non-navigable.
        }

        // ── Literal types ───────────────────────────────────────────
        type_ast::Type::LiteralInt(_)
        | type_ast::Type::LiteralFloat(_)
        | type_ast::Type::LiteralString(_) => {
            // Literals are not navigable.
        }

        // ── int range ───────────────────────────────────────────────
        type_ast::Type::IntRange(_) => {
            // int<min, max> — not navigable.
        }

        // ── Catch-all (non_exhaustive) ──────────────────────────────
        _ => {}
    }
}

/// Emit a span for a type identifier (class name, or self/static/parent).
///
/// Checks the `NON_NAVIGABLE` list and emits either a `ClassReference` or
/// `SelfStaticParent` span as appropriate.
fn emit_identifier_span(name: &str, start: u32, end: u32, spans: &mut Vec<SymbolSpan>) {
    // Handle `self`, `static`, `parent` — they're class-like but get
    // a special span kind.
    if let Some(ssp_kind) = self_static_parent_kind(name) {
        spans.push(SymbolSpan {
            start,
            end,
            kind: SymbolKind::SelfStaticParent(ssp_kind),
        });
        return;
    }

    // Check navigability (strips leading `\` for the check).
    let check_name = strip_fqn_prefix(name).trim();
    if is_navigable_type(check_name) {
        let is_fqn = name.starts_with('\\');
        let display_name = crate::atom::atom(strip_fqn_prefix(name).trim());
        spans.push(SymbolSpan {
            start,
            end,
            kind: SymbolKind::ClassReference {
                name: display_name,
                is_fqn,
                context: ClassRefContext::Other,
            },
        });
    }
}

/// Recurse into generic type parameters (`<T, U, V>`).
fn emit_generic_params(params: &type_ast::GenericParameters<'_>, sink: &mut DocblockSink<'_>) {
    for entry in &params.entries {
        emit_type_symbols(&entry.inner, sink);
    }
}

// ─── @see tag symbol extraction ─────────────────────────────────────────────

/// Scan raw docblock text for inline `{@see ...}` references.
///
/// The CST does model these (as a `TextSegment::InlineTag`), but they can turn
/// up in any prose: the free text before the first tag, and the description of
/// every tag that has one.  Scanning the raw string reaches all of them in one
/// pass, without a `Text` visitor threaded through each tag shape.
fn extract_inline_see_symbols(docblock: &str, base_offset: u32, spans: &mut Vec<SymbolSpan>) {
    let mut search_from = 0;
    while let Some(open) = docblock[search_from..].find("{@see ") {
        let abs_open = search_from + open;
        let after_tag = abs_open + 6; // length of "{@see "
        if let Some(close) = docblock[after_tag..].find('}') {
            let reference = docblock[after_tag..after_tag + close].trim();
            if !reference.is_empty() {
                // The reference token starts after `{@see `.
                let ref_start = after_tag
                    + (docblock[after_tag..after_tag + close].len()
                        - docblock[after_tag..after_tag + close].trim_start().len());
                let first_token = reference.split_whitespace().next().unwrap_or("");
                if !first_token.is_empty() {
                    emit_see_reference(first_token, base_offset + ref_start as u32, spans);
                }
            }
            search_from = after_tag + close + 1;
        } else {
            break;
        }
    }
}

/// Parse a single `@see` reference token and emit the appropriate symbol span.
///
/// Supported forms:
/// - `ClassName` → `ClassReference`
/// - `\Fully\Qualified\Name` → `ClassReference` (FQN)
/// - `ClassName::method()` → `MemberAccess` (method call)
/// - `ClassName::$property` → `MemberAccess` (static property)
/// - `ClassName::CONSTANT` → `MemberAccess` (static constant)
/// - `ClassName#method()` → `MemberAccess` (legacy phpDocumentor instance
///   member fragment syntax)
/// - `function()` → `FunctionCall` (standalone function, no `::` or `#`)
/// - `http://...` / `https://...` → skipped (URLs)
fn emit_see_reference(reference: &str, file_offset: u32, spans: &mut Vec<SymbolSpan>) {
    // Skip URLs.
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return;
    }

    // Strip trailing `()` if present (used on both methods and functions).
    let reference = reference.strip_suffix("()").unwrap_or(reference);

    // `@see` references that contain `\` are almost always fully-qualified
    // class names (e.g. `@see App\Models\User`).  Without a leading `\`,
    // `class_ref_span` would set `is_fqn = false`, causing downstream
    // consumers to prepend the current file's namespace and produce a
    // doubled name like `App\Models\App\Models\User`.  Treat any
    // backslash-containing reference as FQN by prepending `\`.
    // `file_offset` points at the original (pre-prefix) token, so the number
    // of synthetic characters we prepend must be subtracted back out of every
    // offset computed on the lengthened string, otherwise the emitted spans
    // are shifted one byte to the right.
    let owned_reference;
    let (reference, prefix_len) = if reference.contains('\\') && !reference.starts_with('\\') {
        owned_reference = format!("\\{reference}");
        (&owned_reference as &str, 1u32)
    } else {
        (reference, 0u32)
    };

    // Check for `Class::member` form.
    if let Some(sep_pos) = reference.find("::") {
        let class_part = &reference[..sep_pos];
        let member_part = &reference[sep_pos + 2..];

        if class_part.is_empty() || member_part.is_empty() {
            return;
        }

        // Accept regular class names and self/static/parent.
        let clean_class = class_part.trim_start_matches('\\');
        let is_self_like = self_static_parent_kind(clean_class).is_some();
        if !is_self_like && !is_navigable_type(clean_class) {
            return;
        }

        // Emit a ClassReference or SelfStaticParent span for the class
        // portion. Lengths and the separator position were measured on the
        // prefixed string, so undo the synthetic prefix to land on the
        // original source bytes.
        let class_start = file_offset;
        let class_end = file_offset + class_part.len() as u32 - prefix_len;
        // Pass `class_part` (which keeps any leading `\`, including the
        // synthetic one prepended above) so the emitted ClassReference
        // carries the correct `is_fqn` flag. Passing the stripped
        // `clean_class` would drop the flag and make downstream
        // consumers re-prefix the current namespace, doubling it.
        emit_identifier_span(class_part, class_start, class_end, spans);

        // Emit a MemberAccess span for the member portion.
        let member_start = file_offset + sep_pos as u32 + 2 - prefix_len;
        let is_property = member_part.starts_with('$');
        let member_name = if is_property {
            &member_part[1..] // strip $
        } else {
            member_part
        };
        if !member_name.is_empty() {
            let member_end = member_start + member_part.len() as u32;
            spans.push(SymbolSpan {
                start: member_start,
                end: member_end,
                kind: SymbolKind::MemberAccess {
                    subject_text: SubjectText::owned(clean_class.to_string()),
                    member_name: crate::atom::atom(member_name),
                    is_static: true,
                    is_method_call: false,
                    is_docblock_reference: true,
                    is_array_callable: false,
                },
            });
        }
    } else if let Some(sep_pos) = reference.find('#') {
        // Legacy phpDocumentor fragment syntax: `Class#member` refers to
        // an instance property or method, unlike `Class::member`.
        let class_part = &reference[..sep_pos];
        let member_part = &reference[sep_pos + 1..];

        if class_part.is_empty() || member_part.is_empty() {
            return;
        }

        let clean_class = class_part.trim_start_matches('\\');
        let is_self_like = self_static_parent_kind(clean_class).is_some();
        if !is_self_like && !is_navigable_type(clean_class) {
            return;
        }

        let class_start = file_offset;
        let class_end = file_offset + class_part.len() as u32 - prefix_len;
        emit_identifier_span(class_part, class_start, class_end, spans);

        let member_start = file_offset + sep_pos as u32 + 1 - prefix_len;
        let member_end = member_start + member_part.len() as u32;
        spans.push(SymbolSpan {
            start: member_start,
            end: member_end,
            kind: SymbolKind::MemberAccess {
                subject_text: SubjectText::owned(clean_class.to_string()),
                member_name: crate::atom::atom(member_part),
                is_static: false,
                is_method_call: false,
                is_docblock_reference: true,
                is_array_callable: false,
            },
        });
    } else {
        // No `::` or `#` — either a class name or a standalone function.
        // If it looks like a class (starts with uppercase or `\`),
        // emit as ClassReference; otherwise skip.
        let clean = reference.trim_start_matches('\\');
        let self_like = self_static_parent_kind(clean);
        if clean.is_empty() || (self_like.is_none() && !is_navigable_type(clean)) {
            return;
        }

        if self_like.is_some() {
            let start = file_offset;
            let end = file_offset + reference.len() as u32 - prefix_len;
            emit_identifier_span(clean, start, end, spans);
            return;
        }

        // Class names start with uppercase; function names start with
        // lowercase.  PHP convention, not enforced, but a good heuristic.
        let first_char = clean.chars().next().unwrap_or('a');
        if first_char.is_ascii_uppercase() {
            let start = file_offset;
            let end = file_offset + reference.len() as u32 - prefix_len;
            spans.push(class_ref_span(start, end, reference));
        } else {
            // Lowercase first char — treat as function reference.
            let start = file_offset;
            let end = file_offset + reference.len() as u32 - prefix_len;
            spans.push(SymbolSpan {
                start,
                end,
                kind: SymbolKind::FunctionCall {
                    name: crate::atom::atom(clean),
                    is_definition: false,
                },
            });
        }
    }
}
