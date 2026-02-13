/// PHP parsing and AST extraction.
///
/// This module contains the logic for parsing PHP source text using the
/// mago_syntax parser and extracting class information (methods, properties,
/// constants), `use` statement mappings, and namespace declarations from
/// the resulting AST.
use std::collections::HashMap;

use bumpalo::Bump;
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

use crate::Backend;
use crate::types::*;

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

    /// Extract property information from a class member Property node.
    pub(crate) fn extract_property_info(property: &Property) -> Vec<PropertyInfo> {
        let is_static = property.modifiers().iter().any(|m| m.is_static());

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
                }
            })
            .collect()
    }

    /// Parse PHP source text and extract class information.
    /// Returns a Vec of ClassInfo for all classes found in the file.
    pub fn parse_php(&self, content: &str) -> Vec<ClassInfo> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let mut classes = Vec::new();
        Self::extract_classes_from_statements(program.statements.iter(), &mut classes);
        classes
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
    pub fn parse_use_statements(&self, content: &str) -> HashMap<String, String> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let mut use_map = HashMap::new();
        Self::extract_use_statements_from_statements(program.statements.iter(), &mut use_map);
        use_map
    }

    /// Parse PHP source text and extract the declared namespace (if any).
    ///
    /// Returns the namespace string (e.g. `"Klarna\Rest\Checkout"`) or
    /// `None` if the file has no namespace declaration.
    pub fn parse_namespace(&self, content: &str) -> Option<String> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        Self::extract_namespace_from_statements(program.statements.iter())
    }

    /// Walk statements and extract `use` statement mappings.
    fn extract_use_statements_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        use_map: &mut HashMap<String, String>,
    ) {
        for statement in statements {
            match statement {
                Statement::Use(use_stmt) => {
                    Self::extract_use_items(&use_stmt.items, use_map);
                }
                Statement::Namespace(namespace) => {
                    // Recurse into namespace bodies to find use statements
                    Self::extract_use_statements_from_statements(
                        namespace.statements().iter(),
                        use_map,
                    );
                }
                _ => {}
            }
        }
    }

    /// Extract individual use items from a `UseItems` node.
    fn extract_use_items(items: &UseItems, use_map: &mut HashMap<String, String>) {
        match items {
            UseItems::Sequence(seq) => {
                // `use Foo\Bar;` or `use Foo\Bar, Baz\Qux;`
                for item in seq.items.iter() {
                    Self::register_use_item(item, None, use_map);
                }
            }
            UseItems::TypedSequence(seq) => {
                // `use function Foo\bar;` or `use const Foo\BAR;`
                // We only care about class imports, skip function/const
                if seq.r#type.is_function() || seq.r#type.is_const() {
                    return;
                }
                for item in seq.items.iter() {
                    Self::register_use_item(item, None, use_map);
                }
            }
            UseItems::TypedList(list) => {
                // `use function Foo\{bar, baz};` — skip function/const
                if list.r#type.is_function() || list.r#type.is_const() {
                    return;
                }
                let prefix = list.namespace.value();
                for item in list.items.iter() {
                    Self::register_use_item(item, Some(prefix), use_map);
                }
            }
            UseItems::MixedList(list) => {
                // `use Foo\{Bar, function baz, const QUX};`
                let prefix = list.namespace.value();
                for maybe_typed in list.items.iter() {
                    // Skip function/const imports
                    if let Some(ref t) = maybe_typed.r#type
                        && (t.is_function() || t.is_const())
                    {
                        continue;
                    }
                    Self::register_use_item(&maybe_typed.item, Some(prefix), use_map);
                }
            }
        }
    }

    /// Register a single `UseItem` into the use_map.
    ///
    /// If `group_prefix` is `Some`, the item name is relative to that prefix
    /// (e.g. for `use Foo\{Bar}`, prefix is `"Foo"` and item name is `"Bar"`,
    /// giving FQN `"Foo\Bar"`).
    fn register_use_item(
        item: &UseItem,
        group_prefix: Option<&str>,
        use_map: &mut HashMap<String, String>,
    ) {
        let item_name = item.name.value();

        // Build the fully-qualified name
        let fqn = if let Some(prefix) = group_prefix {
            format!("{}\\{}", prefix, item_name)
        } else {
            item_name.to_string()
        };

        // The short (imported) name is either the alias or the last segment
        let short_name = if let Some(ref alias) = item.alias {
            alias.identifier.value.to_string()
        } else {
            // Last segment of the FQN
            fqn.rsplit('\\').next().unwrap_or(&fqn).to_string()
        };

        use_map.insert(short_name, fqn);
    }

    /// Walk statements and extract the first namespace declaration found.
    fn extract_namespace_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
    ) -> Option<String> {
        for statement in statements {
            if let Statement::Namespace(namespace) = statement {
                // The namespace name is an `Option<Identifier>`.
                // Both implicit (`namespace Foo;`) and brace-delimited
                // (`namespace Foo { ... }`) forms may have a name.
                if let Some(ident) = &namespace.name {
                    let name = ident.value();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }

    /// Recursively walk statements and extract class information.
    /// This handles classes at the top level as well as classes nested
    /// inside namespace declarations.
    pub(crate) fn extract_classes_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        classes: &mut Vec<ClassInfo>,
    ) {
        for statement in statements {
            match statement {
                Statement::Class(class) => {
                    let class_name = class.name.value.to_string();

                    let mut methods = Vec::new();
                    let mut properties = Vec::new();
                    let mut constants = Vec::new();

                    for member in class.members.iter() {
                        match member {
                            ClassLikeMember::Method(method) => {
                                let name = method.name.value.to_string();
                                let parameters = Self::extract_parameters(&method.parameter_list);
                                let return_type = method
                                    .return_type_hint
                                    .as_ref()
                                    .map(|rth| Self::extract_hint_string(&rth.hint));
                                let is_static = method.modifiers.iter().any(|m| m.is_static());

                                methods.push(MethodInfo {
                                    name,
                                    parameters,
                                    return_type,
                                    is_static,
                                });
                            }
                            ClassLikeMember::Property(property) => {
                                let mut prop_infos = Self::extract_property_info(property);
                                properties.append(&mut prop_infos);
                            }
                            ClassLikeMember::Constant(constant) => {
                                let type_hint =
                                    constant.hint.as_ref().map(|h| Self::extract_hint_string(h));
                                for item in constant.items.iter() {
                                    constants.push(ConstantInfo {
                                        name: item.name.value.to_string(),
                                        type_hint: type_hint.clone(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }

                    let start_offset = class.left_brace.start.offset;
                    let end_offset = class.right_brace.end.offset;

                    classes.push(ClassInfo {
                        name: class_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                    });
                }
                Statement::Namespace(namespace) => {
                    Self::extract_classes_from_statements(namespace.statements().iter(), classes);
                }
                _ => {}
            }
        }
    }

    /// Update the ast_map, use_map, and namespace_map for a given file URI
    /// by parsing its content.
    pub(crate) fn update_ast(&self, uri: &str, content: &str) {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        // Extract all three in a single parse pass
        let mut classes = Vec::new();
        let mut use_map = HashMap::new();
        let mut namespace: Option<String> = None;

        for statement in program.statements.iter() {
            match statement {
                Statement::Use(use_stmt) => {
                    Self::extract_use_items(&use_stmt.items, &mut use_map);
                }
                Statement::Namespace(ns) => {
                    // Capture namespace name
                    if let Some(ident) = &ns.name {
                        let name = ident.value();
                        if !name.is_empty() && namespace.is_none() {
                            namespace = Some(name.to_string());
                        }
                    }
                    // Recurse into namespace body for classes and use statements
                    for inner in ns.statements().iter() {
                        match inner {
                            Statement::Use(use_stmt) => {
                                Self::extract_use_items(&use_stmt.items, &mut use_map);
                            }
                            Statement::Class(_) => {
                                Self::extract_classes_from_statements(
                                    std::iter::once(inner),
                                    &mut classes,
                                );
                            }
                            Statement::Namespace(inner_ns) => {
                                // Nested namespaces (rare but valid)
                                Self::extract_use_statements_from_statements(
                                    inner_ns.statements().iter(),
                                    &mut use_map,
                                );
                                Self::extract_classes_from_statements(
                                    inner_ns.statements().iter(),
                                    &mut classes,
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Class(_) => {
                    Self::extract_classes_from_statements(std::iter::once(statement), &mut classes);
                }
                _ => {}
            }
        }

        let uri_string = uri.to_string();

        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri_string.clone(), classes);
        }
        if let Ok(mut map) = self.use_map.lock() {
            map.insert(uri_string.clone(), use_map);
        }
        if let Ok(mut map) = self.namespace_map.lock() {
            map.insert(uri_string, namespace);
        }
    }
}
