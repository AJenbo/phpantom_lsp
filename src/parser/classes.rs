use std::collections::HashMap;

use mago_span::HasSpan;
use mago_syntax::ast::attribute::AttributeList;
use mago_syntax::ast::class_like::enum_case::EnumCaseItem;
use mago_syntax::ast::class_like::member::ClassLikeMember;
use mago_syntax::ast::class_like::method::{Method, MethodBody};
use mago_syntax::ast::class_like::property::{Property, PropertyItem};
use mago_syntax::ast::class_like::trait_use::{
    TraitUseAdaptation, TraitUseMethodReference, TraitUseSpecification,
};
use mago_syntax::ast::sequence::Sequence;

/// Check whether a method has the `#[Scope]` attribute (Laravel 11+).
///
/// Scans the method's attribute lists for an attribute whose short name
/// is `Scope` (matching `#[Scope]`, `#[\Illuminate\Database\Eloquent\Attributes\Scope]`,
/// or any use-imported alias that ends with `Scope`).
fn has_scope_attribute(method: &Method<'_>) -> bool {
    for attr_list in method.attribute_lists.iter() {
        for attr in attr_list.attributes.iter() {
            if attr.name.last_segment() == "Scope" {
                return true;
            }
        }
    }
    false
}
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
/// [`find_class_at_offset`](crate::util::find_class_at_offset) can resolve
/// `$this` inside their bodies.
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::docblock::types::parse_generic_args;
use crate::types::*;
use crate::virtual_members::laravel::infer_relationship_from_body;

use super::{
    DocblockCtx, extract_hint_string, extract_parameters, extract_property_info,
    extract_visibility, is_available_for_version,
};

/// Docblock-derived metadata common to all class-like declarations.
///
/// Produced by [`extract_class_docblock`] and consumed by each match arm
/// in [`Backend::extract_classes_from_statements`] to avoid repeating
/// the same extraction calls for classes, interfaces, traits, and enums.
#[derive(Default)]
struct ClassDocblockInfo {
    /// Deprecation message from `@deprecated`, or `None` if not deprecated.
    deprecation_message: Option<String>,
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
    /// Mixin class names from `@mixin` tags.
    mixins: Vec<String>,
    /// URL from the `@link` tag in the class-level docblock.
    link: Option<String>,
    /// Raw class-level docblock text, preserved for deferred `@method` /
    /// `@property` parsing by the `PHPDocProvider`.
    raw_docblock: Option<String>,
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

    let params_with_bounds = docblock::extract_template_params_with_bounds(doc_text);
    let template_params = params_with_bounds.iter().map(|(n, _)| n.clone()).collect();
    let template_param_bounds: HashMap<String, String> = params_with_bounds
        .into_iter()
        .filter_map(|(name, bound)| bound.map(|b| (name, b)))
        .collect();

    ClassDocblockInfo {
        deprecation_message: docblock::extract_deprecation_message(doc_text),
        template_params,
        template_param_bounds,
        extends_generics: docblock::extract_generics_tag(doc_text, "@extends"),
        implements_generics: docblock::extract_generics_tag(doc_text, "@implements"),
        use_generics: docblock::extract_generics_tag(doc_text, "@use"),
        type_aliases: docblock::extract_type_aliases(doc_text),
        mixins: docblock::extract_mixin_tags(doc_text),
        link: docblock::extract_link_url(doc_text),
        raw_docblock: Some(doc_text.to_string()),
    }
}

/// Extract the custom collection class name from a `#[CollectedBy(X::class)]` attribute.
///
/// Scans the class's attribute lists for an attribute whose short name is
/// `CollectedBy` and extracts the first argument's text with `::class` stripped.
/// Returns `None` if no such attribute exists.
fn extract_collected_by_attribute(
    attribute_lists: &Sequence<'_, AttributeList<'_>>,
    content: &str,
) -> Option<String> {
    for attr_list in attribute_lists.iter() {
        for attr in attr_list.attributes.iter() {
            let short = attr.name.last_segment();
            if short != "CollectedBy" {
                continue;
            }
            let arg_list = attr.argument_list.as_ref()?;
            let first_arg = arg_list.arguments.first()?;
            let span = first_arg.span();
            let start = span.start.offset as usize;
            let end = span.end.offset as usize;
            let text = content.get(start..end)?;
            let class_name = text.trim_end_matches("::class").trim();
            if !class_name.is_empty() {
                return Some(class_name.to_string());
            }
        }
    }
    None
}

/// Determine the custom collection class for an Eloquent model.
///
/// Checks three sources in priority order:
///
/// 1. `#[CollectedBy(CustomCollection::class)]` attribute on the class.
/// 2. `/** @use HasCollection<CustomCollection> */` in `use_generics`.
/// 3. A `newCollection()` method override whose return type names the
///    custom collection class.
///
/// The attribute takes priority because it is the newer Laravel API.
fn extract_custom_collection(
    attribute_lists: &Sequence<'_, AttributeList<'_>>,
    use_generics: &[(String, Vec<String>)],
    methods: &[MethodInfo],
    content: &str,
) -> Option<String> {
    // 1. Try the #[CollectedBy] attribute first.
    if let Some(name) = extract_collected_by_attribute(attribute_lists, content) {
        return Some(name);
    }

    // 2. Fall back to @use HasCollection<X>.
    for (trait_name, args) in use_generics {
        let short = trait_name.rsplit('\\').next().unwrap_or(trait_name);
        if short == "HasCollection" && !args.is_empty() {
            return Some(args[0].clone());
        }
    }

    // 3. Fall back to newCollection() return type override.
    extract_custom_collection_from_new_collection(methods)
}

/// Extract the custom collection class from a `newCollection()` method
/// override.
///
/// Laravel models can override `newCollection()` to return a custom
/// collection class.  If the method's return type is not the standard
/// `Illuminate\Database\Eloquent\Collection` (or its short name
/// `Collection`), it is treated as a custom collection class.
///
/// Returns `None` if no `newCollection` method exists, if it has no
/// return type, or if the return type is the standard Eloquent
/// Collection.
fn extract_custom_collection_from_new_collection(methods: &[MethodInfo]) -> Option<String> {
    let method = methods.iter().find(|m| m.name == "newCollection")?;
    let return_type = method.return_type.as_deref()?;

    // Strip generic parameters (e.g. `TaskCollection<int, static>` → `TaskCollection`).
    let (base, _) = parse_generic_args(return_type);

    // Compare without leading backslash for the standard Collection check,
    // but preserve the original form so that `resolve_name` in
    // `resolve_parent_class_names` can correctly handle both FQN
    // (`\App\Collections\X`) and short names (`X`) via the use map.
    let stripped = base.strip_prefix('\\').unwrap_or(base);

    // Ignore the standard Eloquent Collection — that's the default, not
    // a custom override.
    if stripped == "Illuminate\\Database\\Eloquent\\Collection" || stripped == "Collection" {
        return None;
    }

    if stripped.is_empty() {
        return None;
    }

    Some(base.to_string())
}

/// Extract Eloquent cast definitions from a class's members.
///
/// Scans the class members for:
/// 1. A `$casts` property with an array initializer (`protected $casts = [...]`)
/// 2. A `casts()` method whose body contains a `return [...]` statement
///
/// Returns a list of `(column_name, cast_type)` pairs extracted from the
/// array literal text.  Both sources are merged: entries from the
/// `casts()` method take priority over `$casts` property entries when
/// the same column appears in both.  This matches Laravel's runtime
/// behaviour where `Model::casts()` overrides `$casts`.
fn extract_casts_definitions<'a>(
    members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
    content: &str,
) -> Vec<(String, String)> {
    let mut property_text: Option<String> = None;
    let mut method_text: Option<String> = None;

    for member in members {
        match member {
            ClassLikeMember::Property(Property::Plain(plain)) => {
                for item in plain.items.iter() {
                    let var_name = item.variable().name.to_string();
                    let stripped = var_name.strip_prefix('$').unwrap_or(&var_name);
                    if stripped != "casts" {
                        continue;
                    }
                    if let PropertyItem::Concrete(concrete) = item {
                        let span = concrete.value.span();
                        let start = span.start.offset as usize;
                        let end = span.end.offset as usize;
                        if let Some(text) = content.get(start..end) {
                            property_text = Some(text.to_string());
                        }
                    }
                }
            }
            ClassLikeMember::Method(method) if method.name.value == "casts" => {
                if let MethodBody::Concrete(block) = &method.body {
                    let start = block.left_brace.start.offset as usize;
                    let end = block.right_brace.end.offset as usize;
                    if let Some(text) = content.get(start..end) {
                        method_text = Some(text.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // Start with $casts property entries as the base.
    let mut merged: Vec<(String, String)> = Vec::new();

    if let Some(ref text) = property_text {
        merged = parse_casts_array(text);
    }

    // Merge casts() method entries on top — method entries override
    // property entries for the same column, matching Laravel's runtime
    // behaviour.
    if let Some(ref text) = method_text
        && let Some(arr_start) = text.find("return")
    {
        let after_return = &text[arr_start + 6..];
        if let Some(bracket_pos) = after_return.find('[') {
            let array_text = &after_return[bracket_pos..];
            let method_defs = parse_casts_array(array_text);
            for (key, value) in method_defs {
                if let Some(existing) = merged.iter_mut().find(|(k, _)| *k == key) {
                    existing.1 = value;
                } else {
                    merged.push((key, value));
                }
            }
        }
    }

    merged
}

/// Parse key-value pairs from a PHP array literal text.
///
/// Accepts text starting with `[` and extracts `'key' => 'value'` pairs.
/// Both single-quoted and double-quoted strings are supported for keys
/// and values.  Handles multi-line arrays and trailing commas.
///
/// Returns a list of `(key, value)` string pairs.
fn parse_casts_array(text: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let trimmed = text.trim();

    // Must start with `[`
    let inner = if let Some(s) = trimmed.strip_prefix('[') {
        // Strip trailing `]` if present
        s.strip_suffix(']').unwrap_or(s)
    } else {
        return results;
    };

    // Split on commas, handling each `'key' => 'value'` pair.
    // This simple approach works because cast arrays contain only
    // string literals — no nested arrays or complex expressions.
    for segment in inner.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        // Look for the `=>` arrow.
        let Some(arrow_pos) = segment.find("=>") else {
            continue;
        };

        let key_part = segment[..arrow_pos].trim();
        let value_part = segment[arrow_pos + 2..].trim();

        let key = extract_string_literal(key_part);
        let value = extract_string_literal(value_part);

        if let (Some(k), Some(v)) = (key, value)
            && !k.is_empty()
            && !v.is_empty()
        {
            results.push((k, v));
        }
    }

    results
}

/// Extract the string content from a PHP string literal.
///
/// Strips surrounding quotes (single or double) and returns the inner
/// text.  Returns `None` if the text is not a quoted string.
///
/// Also handles:
/// - `SomeCast::class` — returns `"SomeCast"`
/// - `Address::class.':argument'` — strips the concatenated argument
///   suffix and returns `"Address"`
fn extract_string_literal(text: &str) -> Option<String> {
    let t = text.trim();
    if ((t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')))
        && t.len() >= 2
    {
        return Some(t[1..t.len() - 1].to_string());
    }
    // For class-string cast values like `SomeCast::class` or
    // `SomeCast::class.':argument'`, extract the class name.
    // First, strip any `.':...'` or `.":...."` concatenation suffix.
    let without_concat = if let Some(dot_pos) = t.find(".'") {
        t[..dot_pos].trim()
    } else if let Some(dot_pos) = t.find(".\"") {
        t[..dot_pos].trim()
    } else {
        t
    };
    if let Some(before) = without_concat.strip_suffix("::class") {
        let name = before.trim().strip_prefix('\\').unwrap_or(before.trim());
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Extract Eloquent attribute defaults from a class's `$attributes` property.
///
/// Scans the class members for a `$attributes` property with an array
/// initializer (`protected $attributes = [...]`) and infers PHP types
/// from the literal default values.
///
/// Returns a list of `(column_name, php_type)` pairs.  For example,
/// `'role' => 'user'` produces `("role", "string")` and
/// `'is_active' => true` produces `("is_active", "bool")`.
fn extract_attributes_definitions<'a>(
    members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
    content: &str,
) -> Vec<(String, String)> {
    for member in members {
        if let ClassLikeMember::Property(Property::Plain(plain)) = member {
            for item in plain.items.iter() {
                let var_name = item.variable().name.to_string();
                let stripped = var_name.strip_prefix('$').unwrap_or(&var_name);
                if stripped != "attributes" {
                    continue;
                }
                if let PropertyItem::Concrete(concrete) = item {
                    let span = concrete.value.span();
                    let start = span.start.offset as usize;
                    let end = span.end.offset as usize;
                    if let Some(text) = content.get(start..end) {
                        return parse_attributes_array(text);
                    }
                }
            }
        }
    }
    Vec::new()
}

/// Parse key-value pairs from a PHP `$attributes` array literal and
/// infer types from the default values.
///
/// Accepts text starting with `[` and extracts `'key' => value` pairs
/// where `value` is a PHP literal (`true`, `false`, `null`, integer,
/// float, or string).
///
/// Returns a list of `(column_name, php_type)` pairs.
fn parse_attributes_array(text: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let trimmed = text.trim();

    let inner = if let Some(s) = trimmed.strip_prefix('[') {
        s.strip_suffix(']').unwrap_or(s)
    } else {
        return results;
    };

    for segment in inner.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let Some(arrow_pos) = segment.find("=>") else {
            continue;
        };

        let key_part = segment[..arrow_pos].trim();
        let value_part = segment[arrow_pos + 2..].trim();

        let Some(key) = extract_string_literal(key_part) else {
            continue;
        };
        if key.is_empty() {
            continue;
        }

        if let Some(php_type) = infer_type_from_literal(value_part) {
            results.push((key, php_type));
        }
    }

    results
}

/// Infer a PHP type from a literal value in source text.
///
/// Recognises:
/// - `true` / `false` → `"bool"`
/// - `null` → `"null"`
/// - Integer literals (e.g. `0`, `-42`) → `"int"`
/// - Float literals (e.g. `1.5`, `-0.1`) → `"float"`
/// - Single- or double-quoted strings → `"string"`
/// - Array literals `[...]` → `"array"`
///
/// Returns `None` for unrecognised expressions (function calls,
/// constants, variables, etc.).
fn infer_type_from_literal(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }

    let lower = v.to_lowercase();

    // Boolean literals
    if lower == "true" || lower == "false" {
        return Some("bool".to_string());
    }

    // Null literal
    if lower == "null" {
        return Some("null".to_string());
    }

    // String literals (single or double quoted)
    if (v.starts_with('\'') && v.ends_with('\'')) || (v.starts_with('"') && v.ends_with('"')) {
        return Some("string".to_string());
    }

    // Array literal
    if v.starts_with('[') {
        return Some("array".to_string());
    }

    // Numeric literals — try float first (contains `.`), then int.
    // Strip optional leading `-` for negative numbers.
    let numeric = v.strip_prefix('-').unwrap_or(v).trim();
    if !numeric.is_empty() && numeric.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        if numeric.contains('.') {
            return Some("float".to_string());
        }
        return Some("int".to_string());
    }

    None
}

/// Extract column names from `$fillable`, `$guarded`, and `$hidden` arrays.
///
/// These properties contain simple string lists of column names without
/// type information.  The `LaravelModelProvider` uses them as a
/// last-resort fallback, synthesizing `mixed`-typed virtual properties
/// for columns not already covered by `$casts` or `$attributes`.
///
/// All three arrays are merged; duplicates are removed (first occurrence
/// wins).
fn extract_column_names<'a>(
    members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
    content: &str,
) -> Vec<String> {
    let mut names = Vec::new();
    let targets = ["fillable", "guarded", "hidden", "visible"];

    for member in members {
        if let ClassLikeMember::Property(Property::Plain(plain)) = member {
            for item in plain.items.iter() {
                let var_name = item.variable().name.to_string();
                let stripped = var_name.strip_prefix('$').unwrap_or(&var_name);
                if !targets.contains(&stripped) {
                    continue;
                }
                if let PropertyItem::Concrete(concrete) = item {
                    let span = concrete.value.span();
                    let start = span.start.offset as usize;
                    let end = span.end.offset as usize;
                    if let Some(text) = content.get(start..end) {
                        for name in parse_string_list(text) {
                            if !names.contains(&name) {
                                names.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    names
}

/// Parse a PHP array literal containing only string values.
///
/// Accepts text starting with `[` and extracts bare string values
/// (no `=>` keys).  For example, `['name', 'email', 'password']`
/// returns `["name", "email", "password"]`.
fn parse_string_list(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let trimmed = text.trim();

    let inner = if let Some(s) = trimmed.strip_prefix('[') {
        s.strip_suffix(']').unwrap_or(s)
    } else {
        return results;
    };

    for segment in inner.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        // Skip key-value pairs (these belong to a different kind of array).
        if segment.contains("=>") {
            continue;
        }
        if let Some(s) = extract_string_literal(segment)
            && !s.is_empty()
        {
            results.push(s);
        }
    }

    results
}

/// Try to infer an Eloquent relationship return type from a method's body.
///
/// When a method has no `@return` annotation and no native return type
/// hint, this function extracts the method body text and scans it for
/// patterns like `$this->hasMany(Post::class)`.  If found, it returns
/// a synthesized return type string (e.g. `HasMany<Post>`).
///
/// This enables relationship property synthesis on models that don't
/// use Larastan-style `@return` annotations.
fn infer_relationship_from_method<'a>(
    method: &Method<'a>,
    doc_ctx: Option<&DocblockCtx<'a>>,
) -> Option<String> {
    let ctx = doc_ctx?;
    let MethodBody::Concrete(block) = &method.body else {
        return None;
    };
    let start = block.left_brace.start.offset as usize;
    let end = block.right_brace.end.offset as usize;
    if end > ctx.content.len() || start >= end {
        return None;
    }
    // Adjust to valid UTF-8 char boundaries.
    let start = ctx.content.floor_char_boundary(start);
    let end = ctx.content.floor_char_boundary(end);
    let body_text = &ctx.content[start..end];
    infer_relationship_from_body(body_text)
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
                        methods,
                        properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                        inline_use_generics,
                    ) = Self::extract_class_like_members(class.members.iter(), doc_ctx);

                    let doc_info = extract_class_docblock(class, doc_ctx);

                    let mut use_generics = doc_info.use_generics;
                    use_generics.extend(inline_use_generics);

                    let keyword_offset = class.class.span.start.offset;
                    let start_offset = class.left_brace.start.offset;
                    let end_offset = class.right_brace.end.offset;

                    let content = doc_ctx.map(|c| c.content).unwrap_or("");
                    let custom_collection = extract_custom_collection(
                        &class.attribute_lists,
                        &use_generics,
                        &methods,
                        content,
                    );

                    let casts_definitions =
                        extract_casts_definitions(class.members.iter(), content);

                    let attributes_definitions =
                        extract_attributes_definitions(class.members.iter(), content);

                    let column_names = extract_column_names(class.members.iter(), content);

                    classes.push(ClassInfo {
                        kind: ClassLikeKind::Class,
                        name: class_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        keyword_offset,
                        parent_class,
                        interfaces,
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: class.modifiers.contains_final(),
                        is_abstract: class.modifiers.contains_abstract(),
                        deprecation_message: doc_info.deprecation_message.clone(),
                        link: doc_info.link,
                        template_params: doc_info.template_params,
                        template_param_bounds: doc_info.template_param_bounds,
                        extends_generics: doc_info.extends_generics,
                        implements_generics: doc_info.implements_generics,
                        use_generics,
                        type_aliases: doc_info.type_aliases,
                        trait_precedences,
                        trait_aliases,
                        class_docblock: doc_info.raw_docblock,
                        file_namespace: None,
                        backed_type: None,
                        laravel: Some(Box::new(LaravelMetadata {
                            custom_collection,
                            casts_definitions,
                            attributes_definitions,
                            column_names,
                        })),
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
                        methods,
                        properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                        inline_use_generics,
                    ) = Self::extract_class_like_members(iface.members.iter(), doc_ctx);

                    let doc_info = extract_class_docblock(iface, doc_ctx);

                    let keyword_offset = iface.interface.span.start.offset;
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
                        keyword_offset,
                        parent_class,
                        interfaces: all_parents,
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: false,
                        is_abstract: false,
                        deprecation_message: doc_info.deprecation_message.clone(),
                        link: doc_info.link,
                        template_params: doc_info.template_params,
                        template_param_bounds: doc_info.template_param_bounds,
                        extends_generics: doc_info.extends_generics,
                        implements_generics: doc_info.implements_generics,
                        use_generics: {
                            let mut ug = doc_info.use_generics;
                            ug.extend(inline_use_generics);
                            ug
                        },
                        type_aliases: doc_info.type_aliases,
                        trait_precedences,
                        trait_aliases,
                        class_docblock: doc_info.raw_docblock,
                        file_namespace: None,
                        backed_type: None,
                        laravel: None,
                    });

                    // Walk method bodies for anonymous classes.
                    Self::find_anonymous_classes_in_members(iface.members.iter(), classes, doc_ctx);
                }
                Statement::Trait(trait_def) => {
                    let trait_name = trait_def.name.value.to_string();

                    let (
                        methods,
                        properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                        inline_use_generics,
                    ) = Self::extract_class_like_members(trait_def.members.iter(), doc_ctx);

                    let doc_info = extract_class_docblock(trait_def, doc_ctx);

                    let keyword_offset = trait_def.r#trait.span.start.offset;
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
                        keyword_offset,
                        parent_class: None,
                        interfaces: vec![],
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: false,
                        is_abstract: false,
                        deprecation_message: doc_info.deprecation_message.clone(),
                        link: doc_info.link,
                        template_params: doc_info.template_params,
                        template_param_bounds: doc_info.template_param_bounds,
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: {
                            let mut ug: Vec<(String, Vec<String>)> = vec![];
                            ug.extend(inline_use_generics);
                            ug
                        },
                        type_aliases: HashMap::new(),
                        trait_precedences,
                        trait_aliases,
                        class_docblock: doc_info.raw_docblock,
                        file_namespace: None,
                        backed_type: None,
                        laravel: None,
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

                    let (methods, properties, constants, mut used_traits, _, _, _) =
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

                    let doc_info = extract_class_docblock(enum_def, doc_ctx);

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

                    let keyword_offset = enum_def.r#enum.span.start.offset;
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
                        keyword_offset,
                        parent_class: None,
                        interfaces,
                        used_traits,
                        mixins: doc_info.mixins,
                        is_final: true,
                        is_abstract: false,
                        deprecation_message: doc_info.deprecation_message,
                        link: doc_info.link,
                        template_params: vec![],
                        template_param_bounds: HashMap::new(),
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: vec![],
                        type_aliases: HashMap::new(),
                        trait_precedences: vec![],
                        trait_aliases: vec![],
                        class_docblock: doc_info.raw_docblock,
                        file_namespace: None,
                        backed_type: enum_def
                            .backing_type_hint
                            .as_ref()
                            .map(|h| crate::parser::extract_hint_string(&h.hint)),
                        laravel: None,
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

        let (methods, properties, constants, used_traits, trait_precedences, trait_aliases, _) =
            Self::extract_class_like_members(anon.members.iter(), doc_ctx);

        let start_offset = anon.left_brace.start.offset;
        let end_offset = anon.right_brace.end.offset;
        // Anonymous classes don't have a meaningful keyword_offset for
        // go-to-definition purposes — use 0 ("not available").
        let keyword_offset = 0;
        let name = format!("__anonymous@{}", start_offset);

        ClassInfo {
            kind: ClassLikeKind::Class,
            name,
            methods,
            properties,
            constants,
            start_offset,
            end_offset,
            keyword_offset,
            parent_class,
            interfaces,
            used_traits,
            mixins: vec![],
            is_final: false,
            is_abstract: false,
            deprecation_message: None,
            template_params: vec![],
            template_param_bounds: HashMap::new(),
            extends_generics: vec![],
            implements_generics: vec![],
            use_generics: vec![],
            type_aliases: HashMap::new(),
            trait_precedences,
            trait_aliases,
            link: None,
            class_docblock: None,
            file_namespace: None,
            backed_type: None,
            laravel: None,
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
        let mut inline_use_generics: Vec<(String, Vec<String>)> = Vec::new();

        for member in members {
            match member {
                ClassLikeMember::Method(method) => {
                    // Skip methods whose #[PhpStormStubsElementAvailable]
                    // range excludes the target PHP version.
                    if let Some(ctx) = doc_ctx
                        && let Some(ver) = ctx.php_version
                        && !is_available_for_version(&method.attribute_lists, ctx, ver)
                    {
                        continue;
                    }

                    let name = method.name.value.to_string();
                    let name_offset = method.name.span.start.offset;
                    let php_version = doc_ctx.and_then(|ctx| ctx.php_version);
                    let mut parameters = extract_parameters(
                        &method.parameter_list,
                        doc_ctx.map(|ctx| ctx.content),
                        php_version,
                        doc_ctx,
                    );
                    let native_return_type = method
                        .return_type_hint
                        .as_ref()
                        .map(|rth| extract_hint_string(&rth.hint));
                    let is_static = method.modifiers.iter().any(|m| m.is_static());
                    let visibility = extract_visibility(method.modifiers.iter());

                    // Look up the PHPDoc `@return` tag (if any) and apply
                    // type override logic.  Also extract PHPStan conditional
                    // return types if present.  Also check for `@deprecated`.
                    // Additionally extract method-level `@template` params
                    // and their `@param` bindings for general template
                    // substitution at call sites.
                    let (
                        return_type,
                        conditional_return,
                        deprecation_message,
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

                        let deprecation_message =
                            docblock_text.and_then(docblock::extract_deprecation_message);

                        (
                            effective,
                            conditional,
                            deprecation_message,
                            tpl_params,
                            tpl_bindings,
                        )
                    } else {
                        (
                            native_return_type.clone(),
                            None,
                            None,
                            Vec::new(),
                            Vec::new(),
                        )
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
                                    param.hint.as_ref().map(|h| extract_hint_string(h));
                                let saved_native_hint = native_hint.clone();
                                let prop_visibility = extract_visibility(param.modifiers.iter());

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

                                let prop_name_offset = param.variable.span.start.offset;
                                properties.push(PropertyInfo {
                                    name: prop_name,
                                    name_offset: prop_name_offset,
                                    native_type_hint: saved_native_hint,
                                    type_hint,
                                    description: None,
                                    is_static: false,
                                    visibility: prop_visibility,
                                    deprecation_message: None,
                                    is_virtual: false,
                                });
                            }
                        }
                    }

                    // When no return type was resolved from docblocks or
                    // native type hints, try to infer an Eloquent
                    // relationship type from the method body text.
                    // For example, `$this->hasMany(Post::class)` produces
                    // a return type of `HasMany<Post>`.
                    let return_type = if return_type.is_none() {
                        infer_relationship_from_method(method, doc_ctx)
                    } else {
                        return_type
                    };

                    // Merge `@param` docblock types into parameter type
                    // hints so that callable signatures like
                    // `callable(User): void` are preserved.  This mirrors
                    // the promoted-property logic already used for
                    // constructor parameters.
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, method)
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
                        }
                    }

                    let has_scope_attr = has_scope_attribute(method);

                    // Extract description, return description, link, and
                    // per-parameter descriptions from the method's docblock.
                    let method_docblock_text = doc_ctx.and_then(|ctx| {
                        docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, method)
                    });

                    let method_description = method_docblock_text
                        .and_then(|doc| crate::hover::extract_docblock_description(Some(doc)));

                    let return_description =
                        method_docblock_text.and_then(docblock::extract_return_description);

                    let link = method_docblock_text.and_then(docblock::extract_link_url);

                    // Populate per-parameter descriptions from `@param` tags.
                    if let Some(doc_text) = method_docblock_text {
                        for param in &mut parameters {
                            param.description =
                                docblock::extract_param_description(doc_text, &param.name);
                        }
                    }

                    methods.push(MethodInfo {
                        name,
                        name_offset,
                        parameters,
                        native_return_type: native_return_type.clone(),
                        return_type,
                        description: method_description,
                        return_description,
                        link,
                        is_static,
                        visibility,
                        conditional_return,
                        deprecation_message,
                        template_params: method_template_params,
                        template_bindings: method_template_bindings,
                        has_scope_attribute: has_scope_attr,
                        is_virtual: false,
                    });
                }
                ClassLikeMember::Property(property) => {
                    let mut prop_infos = extract_property_info(property);

                    // Apply PHPDoc `@var` override, `@deprecated`, and description
                    // for each property.
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, member)
                    {
                        let deprecation_msg = docblock::extract_deprecation_message(doc_text);
                        if let Some(doc_type) = docblock::extract_var_type(doc_text) {
                            for prop in &mut prop_infos {
                                prop.type_hint = docblock::resolve_effective_type(
                                    prop.type_hint.as_deref(),
                                    Some(&doc_type),
                                );
                            }
                        }
                        if let Some(ref msg) = deprecation_msg {
                            for prop in &mut prop_infos {
                                prop.deprecation_message = Some(msg.clone());
                            }
                        }
                        let description =
                            crate::hover::extract_docblock_description(Some(doc_text))
                                .or_else(|| crate::hover::extract_var_description(doc_text));
                        if description.is_some() {
                            for prop in &mut prop_infos {
                                prop.description = description.clone();
                            }
                        }
                    }

                    properties.append(&mut prop_infos);
                }
                ClassLikeMember::Constant(constant) => {
                    let type_hint = constant.hint.as_ref().map(|h| extract_hint_string(h));
                    let visibility = extract_visibility(constant.modifiers.iter());
                    let deprecation_message = if let Some(ctx) = doc_ctx {
                        docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, member)
                            .and_then(docblock::extract_deprecation_message)
                    } else {
                        None
                    };
                    let const_description = doc_ctx
                        .and_then(|ctx| {
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, member)
                        })
                        .and_then(|doc| crate::hover::extract_docblock_description(Some(doc)));
                    for item in constant.items.iter() {
                        let value = doc_ctx.and_then(|ctx| {
                            let start = item.value.span().start.offset as usize;
                            let end = item.value.span().end.offset as usize;
                            ctx.content.get(start..end).map(|s| s.to_string())
                        });
                        constants.push(ConstantInfo {
                            name: item.name.value.to_string(),
                            name_offset: item.name.span.start.offset,
                            type_hint: type_hint.clone(),
                            visibility,
                            deprecation_message: deprecation_message.clone(),
                            description: const_description.clone(),
                            is_enum_case: false,
                            enum_value: None,
                            value,
                            is_virtual: false,
                        });
                    }
                }
                ClassLikeMember::EnumCase(enum_case) => {
                    let case_name = enum_case.item.name().value.to_string();
                    let case_name_offset = enum_case.item.name().span.start.offset;
                    let enum_value = if let EnumCaseItem::Backed(backed) = &enum_case.item {
                        let start = backed.value.span().start.offset as usize;
                        let end = backed.value.span().end.offset as usize;
                        doc_ctx
                            .and_then(|ctx| ctx.content.get(start..end))
                            .map(|s| s.to_string())
                    } else {
                        None
                    };
                    constants.push(ConstantInfo {
                        name: case_name,
                        name_offset: case_name_offset,
                        type_hint: None,
                        visibility: Visibility::Public,
                        deprecation_message: None,
                        description: None,
                        is_enum_case: true,
                        enum_value,
                        value: None,
                        is_virtual: false,
                    });
                }
                ClassLikeMember::TraitUse(trait_use) => {
                    for trait_name_ident in trait_use.trait_names.iter() {
                        used_traits.push(trait_name_ident.value().to_string());
                    }

                    // Extract `@use` generics from the docblock on the
                    // trait `use` statement itself.  In Laravel, the
                    // Eloquent Builder declares:
                    //
                    //   /** @use BuildsQueries<TModel> */
                    //   use BuildsQueries;
                    //
                    // This binds the trait's template parameter to the
                    // class's own template parameter.
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) = docblock::get_docblock_text_for_node(
                            ctx.trivias,
                            ctx.content,
                            trait_use,
                        )
                    {
                        let tags = docblock::extract_generics_tag(doc_text, "@use");
                        inline_use_generics.extend(tags);
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
            inline_use_generics,
        )
    }
}
