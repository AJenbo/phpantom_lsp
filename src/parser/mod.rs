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

use mago_syntax::ast::*;

use crate::Backend;
use crate::types::*;

/// Context for resolving PHPDoc type annotations from docblock comments.
///
/// Bundles the program's trivia (comments/whitespace) and the raw source
/// text so that extraction functions can look up the `/** ... */` comment
/// preceding any AST node and parse `@return` / `@var` tags from it.
pub(crate) struct DocblockCtx<'a> {
    pub trivias: &'a [Trivia<'a>],
    pub content: &'a str,
}

impl Backend {
    /// Extract a string representation of a type hint from the AST.
    pub(crate) fn extract_hint_string(hint: &Hint) -> String {
        match hint {
            Hint::Identifier(ident) => ident.value().to_string(),
            Hint::Nullable(nullable) => {
                format!("?{}", Self::extract_hint_string(nullable.hint))
            }
            Hint::Union(union) => {
                let left = Self::extract_hint_string(union.left);
                let right = Self::extract_hint_string(union.right);
                format!("{}|{}", left, right)
            }
            Hint::Intersection(intersection) => {
                let left = Self::extract_hint_string(intersection.left);
                let right = Self::extract_hint_string(intersection.right);
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
                format!("({})", Self::extract_hint_string(paren.hint))
            }
        }
    }

    /// Extract parameter information from a method's parameter list.
    pub(crate) fn extract_parameters(
        parameter_list: &FunctionLikeParameterList,
    ) -> Vec<ParameterInfo> {
        parameter_list
            .parameters
            .iter()
            .map(|param| {
                let name = param.variable.name.to_string();
                let is_variadic = param.ellipsis.is_some();
                let is_reference = param.ampersand.is_some();
                let has_default = param.default_value.is_some();
                let is_required = !has_default && !is_variadic;

                let type_hint = param.hint.as_ref().map(|h| Self::extract_hint_string(h));

                ParameterInfo {
                    name,
                    is_required,
                    type_hint,
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
        let visibility = Self::extract_visibility(property.modifiers().iter());

        let type_hint = property.hint().map(|h| Self::extract_hint_string(h));

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
                    type_hint: type_hint.clone(),
                    is_static,
                    visibility,
                    is_deprecated: false,
                }
            })
            .collect()
    }

    /// Parse PHP source text and extract class information.
    /// Returns a Vec of ClassInfo for all classes found in the file.
    pub fn parse_php(&self, content: &str) -> Vec<ClassInfo> {
        let content_owned = content.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arena = bumpalo::Bump::new();
            let file_id = mago_database::file::FileId::new("input.php");
            let program = mago_syntax::parser::parse_file_content(&arena, file_id, &content_owned);

            let doc_ctx = DocblockCtx {
                trivias: program.trivia.as_slice(),
                content: &content_owned,
            };

            let mut classes = Vec::new();
            Self::extract_classes_from_statements(
                program.statements.iter(),
                &mut classes,
                Some(&doc_ctx),
            );
            classes
        }));

        match result {
            Ok(classes) => classes,
            Err(_) => {
                log::error!("PHPantomLSP: parser panicked in parse_php");
                Vec::new()
            }
        }
    }

    /// Parse PHP source text and extract standalone function definitions.
    ///
    /// Returns a list of `FunctionInfo` for every `function` declaration
    /// found at the top level (or inside a namespace block).
    pub fn parse_functions(&self, content: &str) -> Vec<FunctionInfo> {
        let content_owned = content.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arena = bumpalo::Bump::new();
            let file_id = mago_database::file::FileId::new("input.php");
            let program = mago_syntax::parser::parse_file_content(&arena, file_id, &content_owned);

            let doc_ctx = DocblockCtx {
                trivias: program.trivia.as_slice(),
                content: &content_owned,
            };

            let mut functions = Vec::new();
            Self::extract_functions_from_statements(
                program.statements.iter(),
                &mut functions,
                &None,
                Some(&doc_ctx),
            );
            functions
        }));

        match result {
            Ok(functions) => functions,
            Err(_) => {
                log::error!("PHPantomLSP: parser panicked in parse_functions");
                Vec::new()
            }
        }
    }

    /// Parse PHP source text and extract constant names from `define()` calls.
    ///
    /// Returns a list of constant name strings for every `define('NAME', …)`
    /// call found at the top level, inside namespace blocks, block
    /// statements, or `if` guards.
    pub fn parse_defines(&self, content: &str) -> Vec<String> {
        let content_owned = content.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arena = bumpalo::Bump::new();
            let file_id = mago_database::file::FileId::new("input.php");
            let program = mago_syntax::parser::parse_file_content(&arena, file_id, &content_owned);

            let mut defines = Vec::new();
            Self::extract_defines_from_statements(program.statements.iter(), &mut defines);
            defines
        }));

        match result {
            Ok(defines) => defines,
            Err(_) => {
                log::error!("PHPantomLSP: parser panicked in parse_defines");
                Vec::new()
            }
        }
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
    pub fn parse_use_statements(&self, content: &str) -> std::collections::HashMap<String, String> {
        let content_owned = content.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arena = bumpalo::Bump::new();
            let file_id = mago_database::file::FileId::new("input.php");
            let program = mago_syntax::parser::parse_file_content(&arena, file_id, &content_owned);

            let mut use_map = std::collections::HashMap::new();
            Self::extract_use_statements_from_statements(program.statements.iter(), &mut use_map);
            use_map
        }));

        match result {
            Ok(use_map) => use_map,
            Err(_) => {
                log::error!("PHPantomLSP: parser panicked in parse_use_statements");
                std::collections::HashMap::new()
            }
        }
    }

    /// Parse PHP source text and extract the declared namespace (if any).
    ///
    /// Returns the namespace string (e.g. `"Klarna\Rest\Checkout"`) or
    /// `None` if the file has no namespace declaration.
    pub fn parse_namespace(&self, content: &str) -> Option<String> {
        let content_owned = content.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let arena = bumpalo::Bump::new();
            let file_id = mago_database::file::FileId::new("input.php");
            let program = mago_syntax::parser::parse_file_content(&arena, file_id, &content_owned);

            Self::extract_namespace_from_statements(program.statements.iter())
        }));

        match result {
            Ok(ns) => ns,
            Err(_) => {
                log::error!("PHPantomLSP: parser panicked in parse_namespace");
                None
            }
        }
    }
}
