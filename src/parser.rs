/// PHP parsing and AST extraction.
///
/// This module contains the logic for parsing PHP source text using the
/// mago_syntax parser and extracting class information (methods, properties,
/// constants) from the resulting AST.
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

    /// Update the ast_map for a given file URI by parsing its content.
    pub(crate) fn update_ast(&self, uri: &str, content: &str) {
        let classes = self.parse_php(content);
        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri.to_string(), classes);
        }
    }
}
