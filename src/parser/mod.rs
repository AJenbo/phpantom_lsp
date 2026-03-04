use std::collections::HashMap;

/// PHP parsing and AST extraction.
///
/// This module contains the logic for parsing PHP source text using the
/// mago_syntax parser and extracting class information (methods, properties,
/// constants), `use` statement mappings, and namespace declarations from
/// the resulting AST.
///
/// Sub-modules:
/// - [`classes`]: Class, interface, trait, and enum extraction
/// - [`functions`]: Standalone function and `define()` constant extraction
/// - [`use_statements`]: `use` statement and namespace extraction
/// - [`ast_update`]: The `update_ast` orchestrator and name resolution
mod ast_update;
mod classes;
mod functions;
mod use_statements;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::types::*;

/// Context for resolving PHPDoc type annotations from docblock comments.
///
/// Bundles the program's trivia (comments/whitespace) and the raw source
/// text so that extraction functions can look up the `/** ... */` comment
/// preceding any AST node and parse `@return` / `@var` tags from it.
pub(crate) struct DocblockCtx<'a> {
    pub trivias: &'a [Trivia<'a>],
    pub content: &'a str,
    /// Target PHP version for version-aware stub filtering.
    ///
    /// When `Some`, elements annotated with
    /// `#[PhpStormStubsElementAvailable]` whose version range excludes
    /// this version are filtered out during extraction.  Set when
    /// parsing phpstorm-stubs; left as `None` for user code (where the
    /// attribute is never used).
    pub php_version: Option<PhpVersion>,
    /// Use-statement map for the file being parsed.
    ///
    /// Maps short (imported or aliased) names to their fully-qualified
    /// equivalents, e.g. `"Available"` → `"JetBrains\PhpStorm\Internal\PhpStormStubsElementAvailable"`.
    /// Used to resolve attribute names that appear under an alias.
    pub use_map: HashMap<String, String>,
}

/// FQN constants for the JetBrains stub attributes we recognise.
/// Matching is done on the last segment of the resolved FQN so that
/// `#[PhpStormStubsElementAvailable]`, `#[\JetBrains\PhpStorm\Internal\PhpStormStubsElementAvailable]`,
/// and any `use ... as Alias` form all work.
const ATTR_ELEMENT_AVAILABLE: &str = "PhpStormStubsElementAvailable";

impl DocblockCtx<'_> {
    /// Resolve an attribute's short name through the file's use-map and
    /// return the last segment of the resolved FQN.
    ///
    /// For example, if the file has
    /// `use JetBrains\PhpStorm\Internal\PhpStormStubsElementAvailable as Available;`
    /// then `resolve_attr_last_segment("Available")` returns
    /// `"PhpStormStubsElementAvailable"`.
    ///
    /// When the name is not in the use-map, returns `None` (the caller
    /// should fall back to the original name).
    fn resolve_attr_last_segment(&self, short_name: &str) -> Option<&str> {
        let fqn = self.use_map.get(short_name)?;
        Some(fqn.rsplit('\\').next().unwrap_or(fqn))
    }

    /// Check whether `attr_short_name` resolves to `PhpStormStubsElementAvailable`.
    pub(crate) fn is_element_available_attr(&self, attr_short_name: &str) -> bool {
        let canonical = self
            .resolve_attr_last_segment(attr_short_name)
            .unwrap_or(attr_short_name);
        canonical == ATTR_ELEMENT_AVAILABLE
    }
}

// ─── PhpStormStubsElementAvailable Attribute Parsing ────────────────────────

/// Version range extracted from a `#[PhpStormStubsElementAvailable]` attribute.
///
/// Both bounds are inclusive.  `None` means unbounded in that direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VersionAvailability {
    /// Earliest PHP version where the element is available (inclusive).
    pub from: Option<PhpVersion>,
    /// Latest PHP version where the element is available (inclusive).
    pub to: Option<PhpVersion>,
}

/// Check whether a function or method is available for the given PHP version
/// based on its `#[PhpStormStubsElementAvailable]` attributes.
///
/// Returns `true` when:
///   - The element has no `PhpStormStubsElementAvailable` attribute (always available).
///   - The element has the attribute and the version falls within its range.
///
/// Returns `false` when the attribute is present and the version is outside the range.
pub(crate) fn is_available_for_version(
    attribute_lists: &Sequence<'_, attribute::AttributeList<'_>>,
    ctx: &DocblockCtx<'_>,
    php_version: PhpVersion,
) -> bool {
    if let Some(avail) = extract_version_availability(attribute_lists, ctx) {
        php_version.matches_range(avail.from, avail.to)
    } else {
        // No version attribute → always available.
        true
    }
}

/// Check whether a parameter is available for the given PHP version
/// based on its `#[PhpStormStubsElementAvailable]` attributes.
///
/// Same logic as [`is_available_for_version`] but operates on a single
/// parameter's attribute lists.
pub(crate) fn is_param_available_for_version(
    param: &function_like::parameter::FunctionLikeParameter<'_>,
    ctx: &DocblockCtx<'_>,
    php_version: PhpVersion,
) -> bool {
    if let Some(avail) = extract_version_availability(&param.attribute_lists, ctx) {
        php_version.matches_range(avail.from, avail.to)
    } else {
        true
    }
}

/// Extract the `from` / `to` version range from a
/// `#[PhpStormStubsElementAvailable(...)]` attribute, if present.
///
/// Supports both named and positional argument forms:
///   - `#[PhpStormStubsElementAvailable(from: '8.0')]`
///   - `#[PhpStormStubsElementAvailable(from: '8.0', to: '8.4')]`
///   - `#[PhpStormStubsElementAvailable(to: '7.4')]`
///   - `#[PhpStormStubsElementAvailable('8.1')]` (positional → treated as `from`)
///
/// Attribute names are resolved through the [`DocblockCtx`] use-map so
/// that aliases like `ElementAvailable` or `Available` (used in some
/// stub files) are recognised.
///
/// Returns `None` when the attribute is not present.
fn extract_version_availability(
    attribute_lists: &Sequence<'_, attribute::AttributeList<'_>>,
    ctx: &DocblockCtx<'_>,
) -> Option<VersionAvailability> {
    for attr_list in attribute_lists.iter() {
        for attr in attr_list.attributes.iter() {
            if !ctx.is_element_available_attr(attr.name.last_segment()) {
                continue;
            }

            let arg_list = attr.argument_list.as_ref()?;
            let mut from: Option<PhpVersion> = None;
            let mut to: Option<PhpVersion> = None;

            for arg in arg_list.arguments.iter() {
                match arg {
                    argument::Argument::Named(named) => {
                        let name = named.name.value.to_string();
                        let value = extract_string_literal_value(named.value, ctx.content);
                        if let Some(ver_str) = value {
                            let ver = PhpVersion::from_composer_constraint(&ver_str);
                            match name.as_str() {
                                "from" => from = ver,
                                "to" => to = ver,
                                _ => {}
                            }
                        }
                    }
                    argument::Argument::Positional(positional) => {
                        // Positional argument is treated as `from`.
                        let value = extract_string_literal_value(positional.value, ctx.content);
                        if let Some(ver_str) = value {
                            from = PhpVersion::from_composer_constraint(&ver_str);
                        }
                    }
                }
            }

            return Some(VersionAvailability { from, to });
        }
    }

    None
}

/// Extract the string value from a literal string expression by reading
/// the source text between the expression's span and stripping quotes.
fn extract_string_literal_value(
    expr: &expression::Expression<'_>,
    content: &str,
) -> Option<String> {
    let span = expr.span();
    let start = span.start.offset as usize;
    let end = span.end.offset as usize;
    let raw = content.get(start..end)?;
    // Strip surrounding quotes (single or double).
    let trimmed = raw.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
    {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse `content` with the mago-syntax parser and pass the resulting
/// `Program` (plus the content string) to `f`.
///
/// Handles the boilerplate that every parse entry-point needs:
/// allocating a `Bump` arena, creating a `FileId`, calling
/// `parse_file_content`, and wrapping the whole thing in
/// `catch_unwind` so that a parser panic doesn't crash the LSP
/// server.  On panic the error is logged (using `method_name` for
/// context) and `T::default()` is returned.
pub(crate) fn with_parsed_program<T: Default>(
    content: &str,
    method_name: &str,
    f: impl FnOnce(&Program<'_>, &str) -> T,
) -> T {
    let content_owned = content.to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let arena = bumpalo::Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = mago_syntax::parser::parse_file_content(&arena, file_id, &content_owned);
        f(program, &content_owned)
    }));

    match result {
        Ok(value) => value,
        Err(_) => {
            log::error!("PHPantom: parser panicked in {}", method_name);
            T::default()
        }
    }
}

/// Extract a string representation of a type hint from the AST.
pub(crate) fn extract_hint_string(hint: &Hint) -> String {
    match hint {
        Hint::Identifier(ident) => ident.value().to_string(),
        Hint::Nullable(nullable) => {
            format!("?{}", extract_hint_string(nullable.hint))
        }
        Hint::Union(union) => {
            let left = extract_hint_string(union.left);
            let right = extract_hint_string(union.right);
            format!("{}|{}", left, right)
        }
        Hint::Intersection(intersection) => {
            let left = extract_hint_string(intersection.left);
            let right = extract_hint_string(intersection.right);
            format!("{}&{}", left, right)
        }
        Hint::Void(ident)
        | Hint::Never(ident)
        | Hint::Float(ident)
        | Hint::Bool(ident)
        | Hint::Integer(ident)
        | Hint::String(ident)
        | Hint::Object(ident)
        | Hint::Mixed(ident)
        | Hint::Iterable(ident) => ident.value.to_string(),
        Hint::Null(keyword)
        | Hint::True(keyword)
        | Hint::False(keyword)
        | Hint::Array(keyword)
        | Hint::Callable(keyword)
        | Hint::Static(keyword)
        | Hint::Self_(keyword)
        | Hint::Parent(keyword) => keyword.value.to_string(),
        Hint::Parenthesized(paren) => {
            format!("({})", extract_hint_string(paren.hint))
        }
    }
}

/// Extract parameter information from a method's parameter list.
///
/// When `content` is provided, default value expressions are extracted
/// from the source text using AST span offsets.  Pass `None` when the
/// source text is not available (the `default_value` field will be `None`
/// for every parameter in that case).
///
/// When `php_version` is `Some`, parameters annotated with
/// `#[PhpStormStubsElementAvailable]` whose version range excludes the
/// target version are filtered out.  When `None`, all parameters are
/// included.
pub(crate) fn extract_parameters(
    parameter_list: &FunctionLikeParameterList,
    content: Option<&str>,
    php_version: Option<PhpVersion>,
    doc_ctx: Option<&DocblockCtx<'_>>,
) -> Vec<ParameterInfo> {
    parameter_list
        .parameters
        .iter()
        .filter(|param| {
            // When a PHP version is configured, skip parameters that are
            // not available for that version.
            if let Some(ver) = php_version
                && let Some(ctx) = doc_ctx
            {
                is_param_available_for_version(param, ctx, ver)
            } else {
                true
            }
        })
        .map(|param| {
            let name = param.variable.name.to_string();
            let is_variadic = param.ellipsis.is_some();
            let is_reference = param.ampersand.is_some();
            let has_default = param.default_value.is_some();
            let is_required = !has_default && !is_variadic;

            let type_hint = param.hint.as_ref().map(|h| extract_hint_string(h));

            let default_value = content.and_then(|src| {
                let dv = param.default_value.as_ref()?;
                let span = dv.value.span();
                let start = span.start.offset as usize;
                let end = span.end.offset as usize;
                src.get(start..end).map(|s| s.trim().to_string())
            });

            ParameterInfo {
                name,
                is_required,
                native_type_hint: type_hint.clone(),
                type_hint,
                description: None,
                default_value,
                is_variadic,
                is_reference,
            }
        })
        .collect()
}

/// Extract visibility from a set of modifiers.
/// Defaults to `Public` if no visibility modifier is present.
pub(crate) fn extract_visibility<'a>(
    modifiers: impl Iterator<Item = &'a Modifier<'a>>,
) -> Visibility {
    for m in modifiers {
        if m.is_private() {
            return Visibility::Private;
        }
        if m.is_protected() {
            return Visibility::Protected;
        }
        if m.is_public() {
            return Visibility::Public;
        }
    }
    Visibility::Public
}

/// Extract property information from a class member Property node.
pub(crate) fn extract_property_info(property: &Property) -> Vec<PropertyInfo> {
    let is_static = property.modifiers().iter().any(|m| m.is_static());
    let visibility = extract_visibility(property.modifiers().iter());

    let type_hint = property.hint().map(|h| extract_hint_string(h));

    property
        .variables()
        .iter()
        .map(|var| {
            let raw_name = var.name.to_string();
            // Strip the leading `$` for property names since PHP access
            // syntax is `$this->name` not `$this->$name`.
            let name = if let Some(stripped) = raw_name.strip_prefix('$') {
                stripped.to_string()
            } else {
                raw_name
            };

            PropertyInfo {
                name,
                name_offset: var.span.start.offset,
                type_hint: type_hint.clone(),
                native_type_hint: type_hint.clone(),
                description: None,
                is_static,
                visibility,
                deprecation_message: None,
                is_virtual: false,
            }
        })
        .collect()
}

use crate::Backend;

impl Backend {
    /// Parse PHP source text and extract class information.
    /// Returns a Vec of ClassInfo for all classes found in the file.
    pub fn parse_php(&self, content: &str) -> Vec<ClassInfo> {
        Self::parse_php_versioned(content, None)
    }

    /// Version-aware variant of [`parse_php`].
    ///
    /// When `php_version` is `Some`, elements annotated with
    /// `#[PhpStormStubsElementAvailable]` whose version range excludes
    /// the target version are filtered out during extraction.
    pub fn parse_php_versioned(content: &str, php_version: Option<PhpVersion>) -> Vec<ClassInfo> {
        with_parsed_program(content, "parse_php", |program, content| {
            let mut use_map = HashMap::new();
            Self::extract_use_statements_from_statements(program.statements.iter(), &mut use_map);

            let doc_ctx = DocblockCtx {
                trivias: program.trivia.as_slice(),
                content,
                php_version,
                use_map,
            };

            let mut classes = Vec::new();
            Self::extract_classes_from_statements(
                program.statements.iter(),
                &mut classes,
                Some(&doc_ctx),
            );
            classes
        })
    }

    /// Parse PHP source text and extract standalone function definitions.
    ///
    /// Returns a list of `FunctionInfo` for every `function` declaration
    /// found at the top level (or inside a namespace block).
    pub fn parse_functions(&self, content: &str) -> Vec<FunctionInfo> {
        self.parse_functions_versioned(content, None)
    }

    /// Version-aware variant of [`parse_functions`].
    ///
    /// When `php_version` is `Some`, functions and parameters annotated
    /// with `#[PhpStormStubsElementAvailable]` whose version range
    /// excludes the target version are filtered out.
    pub fn parse_functions_versioned(
        &self,
        content: &str,
        php_version: Option<PhpVersion>,
    ) -> Vec<FunctionInfo> {
        with_parsed_program(content, "parse_functions", |program, content| {
            let mut use_map = HashMap::new();
            Self::extract_use_statements_from_statements(program.statements.iter(), &mut use_map);

            let doc_ctx = DocblockCtx {
                trivias: program.trivia.as_slice(),
                content,
                php_version,
                use_map,
            };

            let mut functions = Vec::new();
            Self::extract_functions_from_statements(
                program.statements.iter(),
                &mut functions,
                &None,
                Some(&doc_ctx),
            );
            functions
        })
    }

    /// Parse PHP source text and extract constant names from `define()` calls.
    ///
    /// Returns a list of `(name, define_keyword_offset)` pairs for every
    /// `define('NAME', …)` call found at the top level, inside namespace
    /// blocks, block statements, or `if` guards.
    pub fn parse_defines(&self, content: &str) -> Vec<(String, u32)> {
        with_parsed_program(content, "parse_defines", |program, _content| {
            let mut defines = Vec::new();
            Self::extract_defines_from_statements(program.statements.iter(), &mut defines);
            defines
        })
    }

    /// Parse PHP source text and extract `use` statement mappings.
    ///
    /// Returns a `HashMap` mapping short (imported) names to their
    /// fully-qualified equivalents.
    ///
    /// For example, `use Klarna\Rest\Resource;` produces
    /// `"Resource" → "Klarna\Rest\Resource"`.
    ///
    /// Handles:
    ///   - Simple use: `use Foo\Bar;`
    ///   - Aliased use: `use Foo\Bar as Baz;`
    ///   - Grouped use: `use Foo\{Bar, Baz};`
    ///   - Mixed grouped use: `use Foo\{Bar, function baz, const QUX};`
    ///     (function / const imports are skipped — we only track classes)
    ///   - Use statements inside namespace bodies
    pub(crate) fn parse_use_statements(&self, content: &str) -> HashMap<String, String> {
        with_parsed_program(content, "parse_use_statements", |program, _content| {
            let mut use_map = HashMap::new();
            Self::extract_use_statements_from_statements(program.statements.iter(), &mut use_map);
            use_map
        })
    }

    /// Parse PHP source text and extract the declared namespace (if any).
    ///
    /// Returns the namespace string (e.g. `"Klarna\Rest\Checkout"`) or
    /// `None` if the file has no namespace declaration.
    pub(crate) fn parse_namespace(&self, content: &str) -> Option<String> {
        with_parsed_program(content, "parse_namespace", |program, _content| {
            Self::extract_namespace_from_statements(program.statements.iter())
        })
    }
}
