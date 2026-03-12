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
    /// The file-level namespace, if any.
    ///
    /// Used by [`is_known_deprecated_attr`] to distinguish unqualified
    /// `#[Deprecated]` in the global namespace (which is the native PHP
    /// 8.4 attribute) from `#[Deprecated]` inside a user namespace (which
    /// would resolve to `App\Deprecated`, not the built-in).
    pub namespace: Option<String>,
}

/// FQN constants for the JetBrains stub attributes we recognise.
/// Matching is done on the last segment of the resolved FQN so that
/// `#[PhpStormStubsElementAvailable]`, `#[\JetBrains\PhpStorm\Internal\PhpStormStubsElementAvailable]`,
/// and any `use ... as Alias` form all work.
const ATTR_ELEMENT_AVAILABLE: &str = "PhpStormStubsElementAvailable";

/// Fully-qualified names (without leading `\`) that we recognise as
/// deprecation attributes.  Only the native PHP 8.4 `\Deprecated` and
/// the JetBrains stubs `\JetBrains\PhpStorm\Deprecated` should match.
const DEPRECATED_FQNS: &[&str] = &["Deprecated", "JetBrains\\PhpStorm\\Deprecated"];

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

/// Deprecation metadata extracted from a `#[Deprecated]` attribute.
///
/// phpstorm-stubs annotate ~362 elements with this attribute.  The three
/// fields mirror the attribute's named arguments:
///
/// - `reason` — human-readable explanation (may also appear as a positional arg).
/// - `since` — PHP version when the element was deprecated.
/// - `replacement` — code template for auto-replacement, wired to the
///   "Replace deprecated call" code action.
///
/// When only a bare `#[Deprecated]` is present, all three fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DeprecatedAttribute {
    pub reason: Option<String>,
    pub since: Option<String>,
    /// Code template for auto-replacement, e.g.
    /// `"exif_read_data(%parametersList%)"`.
    ///
    /// Template variables (`%parametersList%`, `%parameter0%`, `%class%`)
    /// are expanded at call sites by the "Replace deprecated call" code
    /// action.
    pub replacement: Option<String>,
}

/// Combined deprecation metadata returned by [`merge_deprecation_info`].
///
/// Bundles the human-readable message with the optional replacement
/// template so callers can populate both `deprecation_message` and
/// `deprecated_replacement` on the type structs in a single pass.
pub(crate) struct DeprecationInfo {
    /// Human-readable message (may be empty for bare `@deprecated` / `#[Deprecated]`).
    pub message: Option<String>,
    /// Replacement code template from `#[Deprecated(replacement: "...")]`.
    pub replacement: Option<String>,
}

impl DeprecatedAttribute {
    /// Build a deprecation message string suitable for storing in
    /// `deprecation_message` on `ClassInfo`, `MethodInfo`, etc.
    ///
    /// Combines `reason` and `since` into a single human-readable line.
    /// Returns an empty string when neither field is set (bare
    /// `#[Deprecated]`).
    pub fn to_message(&self) -> String {
        match (&self.reason, &self.since) {
            (Some(reason), Some(since)) => format!("{} (since PHP {})", reason, since),
            (Some(reason), None) => reason.clone(),
            (None, Some(since)) => format!("since PHP {}", since),
            (None, None) => String::new(),
        }
    }
}

/// Merge a docblock `@deprecated` message with a `#[Deprecated]` attribute.
///
/// The docblock tag takes priority (it is author-written and often more
/// specific).  When the docblock has no `@deprecated` tag, falls back to
/// the `#[Deprecated]` attribute if present.
///
/// **Version-aware suppression:** when the `#[Deprecated]` attribute has a
/// `since` field and the target PHP version (from `DocblockCtx`) is older
/// than `since`, the element is not considered deprecated and `None` is
/// returned for the message.  Docblock `@deprecated` tags have no
/// structured `since` data and are always honoured.
pub(crate) fn merge_deprecation_info(
    docblock_msg: Option<String>,
    attribute_lists: &Sequence<'_, attribute::AttributeList<'_>>,
    doc_ctx: Option<&DocblockCtx<'_>>,
) -> DeprecationInfo {
    // Docblock @deprecated always wins — it has no `since` field so
    // version-aware suppression does not apply.
    if docblock_msg.is_some() {
        return DeprecationInfo {
            message: docblock_msg,
            replacement: None,
        };
    }

    let Some(ctx) = doc_ctx else {
        return DeprecationInfo {
            message: None,
            replacement: None,
        };
    };

    let Some(attr) = extract_deprecated_attribute(attribute_lists, ctx) else {
        return DeprecationInfo {
            message: None,
            replacement: None,
        };
    };

    // Version-aware suppression: if the attribute declares `since` and
    // the project targets an older PHP version, this element is not yet
    // deprecated from the user's perspective.
    if let Some(since_str) = &attr.since
        && let Some(target) = ctx.php_version
        && let Some(since_ver) = PhpVersion::from_composer_constraint(since_str)
        && (target.major, target.minor) < (since_ver.major, since_ver.minor)
    {
        return DeprecationInfo {
            message: None,
            replacement: None,
        };
    }

    DeprecationInfo {
        message: Some(attr.to_message()),
        replacement: attr.replacement,
    }
}

/// Extract `#[Deprecated]` metadata from an element's attribute lists.
///
/// Supports the syntactic forms found in phpstorm-stubs:
///
/// - `#[Deprecated]` — bare, no arguments.
/// - `#[Deprecated("reason text")]` — positional reason.
/// - `#[Deprecated(reason: "...", since: "7.2")]` — named arguments.
/// - `#[Deprecated("reason", replacement: "...", since: "7.2")]` — mixed.
///
/// Attribute names are resolved through the [`DocblockCtx`] use-map so
/// that aliases (unlikely for `Deprecated` but technically possible) are
/// handled correctly.
///
/// Returns `None` when no `#[Deprecated]` attribute is present.
pub(crate) fn extract_deprecated_attribute(
    attribute_lists: &Sequence<'_, attribute::AttributeList<'_>>,
    ctx: &DocblockCtx<'_>,
) -> Option<DeprecatedAttribute> {
    for attr_list in attribute_lists.iter() {
        for attr in attr_list.attributes.iter() {
            if !is_known_deprecated_attr(&attr.name, ctx) {
                continue;
            }

            // Bare #[Deprecated] — no argument list at all.
            let Some(arg_list) = attr.argument_list.as_ref() else {
                return Some(DeprecatedAttribute::default());
            };

            let mut reason: Option<String> = None;
            let mut since: Option<String> = None;
            let mut replacement: Option<String> = None;

            for arg in arg_list.arguments.iter() {
                match arg {
                    argument::Argument::Named(named) => {
                        let name = named.name.value.to_string();
                        let value = extract_string_literal_value(named.value, ctx.content);
                        match name.as_str() {
                            // JetBrains stubs use `reason:`, native PHP 8.4
                            // `\Deprecated` uses `message:`.  Both mean the
                            // same thing — accept either.
                            "reason" | "message" => reason = value,
                            "since" => since = value,
                            "replacement" => replacement = value,
                            _ => {}
                        }
                    }
                    argument::Argument::Positional(positional) => {
                        // First positional argument is the reason/message.
                        if reason.is_none() {
                            reason = extract_string_literal_value(positional.value, ctx.content);
                        }
                    }
                }
            }

            return Some(DeprecatedAttribute {
                reason,
                since,
                replacement,
            });
        }
    }

    None
}

/// Check whether an attribute identifier refers to one of the known
/// deprecation attributes (`\Deprecated` or `\JetBrains\PhpStorm\Deprecated`).
///
/// The matching rules mirror PHP's attribute name resolution:
///
/// - **Fully-qualified** (`\Deprecated`, `\JetBrains\PhpStorm\Deprecated`):
///   strip the leading `\` and compare against [`DEPRECATED_FQNS`].
/// - **Qualified** (`JetBrains\PhpStorm\Deprecated`): resolve the first
///   segment through the use-map, then compare the resolved FQN.  If the
///   first segment is not in the use-map, prepend the file namespace.
/// - **Local/unqualified** (`Deprecated`): look up the short name in the
///   use-map.  If found, compare the resolved FQN.  If not found, the
///   name is in the current namespace. Only matches if the file is in the
///   global namespace (i.e. the bare name equals a known FQN).
fn is_known_deprecated_attr(name: &Identifier<'_>, ctx: &DocblockCtx<'_>) -> bool {
    match name {
        Identifier::FullyQualified(fq) => {
            let stripped = fq.value.strip_prefix('\\').unwrap_or(fq.value);
            DEPRECATED_FQNS.contains(&stripped)
        }
        Identifier::Qualified(q) => {
            // Resolve the first segment via the use-map, then rebuild.
            let first_seg = q.value.split('\\').next().unwrap_or(q.value);
            if let Some(resolved_prefix) = ctx.use_map.get(first_seg) {
                let rest = &q.value[first_seg.len()..]; // includes leading '\'
                let fqn = format!("{}{}", resolved_prefix, rest);
                DEPRECATED_FQNS.contains(&fqn.as_str())
            } else {
                // No use-map entry — prepend file namespace if present.
                let fqn = if let Some(ns) = &ctx.namespace {
                    format!("{}\\{}", ns, q.value)
                } else {
                    q.value.to_string()
                };
                DEPRECATED_FQNS.contains(&fqn.as_str())
            }
        }
        Identifier::Local(local) => {
            // Check use-map first (e.g. `use JetBrains\PhpStorm\Deprecated;`)
            if let Some(fqn) = ctx.use_map.get(local.value) {
                DEPRECATED_FQNS.contains(&fqn.as_str())
            } else {
                // No import — the name lives in the current namespace.
                // Only matches if the file is in the global namespace.
                let fqn = if let Some(ns) = &ctx.namespace {
                    format!("{}\\{}", ns, local.value)
                } else {
                    local.value.to_string()
                };
                DEPRECATED_FQNS.contains(&fqn.as_str())
            }
        }
    }
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
                closure_this_type: None,
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
                deprecated_replacement: None,
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
            let namespace = Self::extract_namespace_from_statements(program.statements.iter());

            let doc_ctx = DocblockCtx {
                trivias: program.trivia.as_slice(),
                content,
                php_version,
                use_map,
                namespace,
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
            let namespace = Self::extract_namespace_from_statements(program.statements.iter());

            let doc_ctx = DocblockCtx {
                trivias: program.trivia.as_slice(),
                content,
                php_version,
                use_map,
                namespace,
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

    /// Parse PHP source text and extract constant names from `define()` calls
    /// and top-level `const` statements.
    ///
    /// Returns a list of `(name, offset, value)` tuples for every
    /// `define('NAME', value)` call or `const NAME = value;` statement
    /// found at the top level, inside namespace blocks, block statements,
    /// or `if` guards.
    pub fn parse_defines(&self, content: &str) -> Vec<(String, u32, Option<String>)> {
        with_parsed_program(content, "parse_defines", |program, content| {
            let mut defines = Vec::new();
            Self::extract_defines_from_statements(program.statements.iter(), &mut defines, content);
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
