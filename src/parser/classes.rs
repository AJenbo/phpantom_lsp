/// Class, interface, trait, and enum extraction.
///
/// This module handles extracting `ClassInfo` from the PHP AST for all
/// class-like declarations: `class`, `interface`, `trait`, and `enum`.
/// It also extracts class-like members (methods, properties, constants,
/// trait uses) and merges in PHPDoc `@property`, `@method`, `@mixin`,
/// and `@deprecated` annotations from docblocks.
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::types::*;

use super::DocblockCtx;

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

                    // Extract parent class name from `extends` clause
                    let parent_class = class
                        .extends
                        .as_ref()
                        .and_then(|ext| ext.types.first().map(|ident| ident.value().to_string()));

                    let (mut methods, mut properties, constants, used_traits) =
                        Self::extract_class_like_members(class.members.iter(), doc_ctx);

                    // Extract @property, @method, @mixin, @template, @extends,
                    // @implements, and @deprecated tags from the class-level docblock.
                    let mut mixins = Vec::new();
                    let mut template_params = Vec::new();
                    let mut extends_generics = Vec::new();
                    let mut implements_generics = Vec::new();
                    let mut class_deprecated = false;
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, class)
                    {
                        class_deprecated = docblock::has_deprecated_tag(doc_text);
                        template_params = docblock::extract_template_params(doc_text);
                        extends_generics = docblock::extract_generics_tag(doc_text, "@extends");
                        implements_generics =
                            docblock::extract_generics_tag(doc_text, "@implements");

                        for (name, type_str) in docblock::extract_property_tags(doc_text) {
                            // Only add if not already declared as a real property.
                            if !properties.iter().any(|p| p.name == name) {
                                properties.push(PropertyInfo {
                                    name,
                                    type_hint: if type_str.is_empty() {
                                        None
                                    } else {
                                        Some(type_str)
                                    },
                                    is_static: false,
                                    visibility: Visibility::Public,
                                    is_deprecated: false,
                                });
                            }
                        }

                        for method_info in docblock::extract_method_tags(doc_text) {
                            // Only add if not already declared as a real method.
                            if !methods.iter().any(|m| m.name == method_info.name) {
                                methods.push(method_info);
                            }
                        }

                        mixins = docblock::extract_mixin_tags(doc_text);
                    }

                    let start_offset = class.left_brace.start.offset;
                    let end_offset = class.right_brace.end.offset;

                    let is_final = class.modifiers.contains_final();

                    classes.push(ClassInfo {
                        name: class_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        used_traits,
                        mixins,
                        is_final,
                        is_deprecated: class_deprecated,
                        template_params,
                        extends_generics,
                        implements_generics,
                    });
                }
                Statement::Interface(iface) => {
                    let iface_name = iface.name.value.to_string();

                    // Interfaces use `extends` for parent interfaces;
                    // take the first one for single-inheritance resolution.
                    let parent_class = iface
                        .extends
                        .as_ref()
                        .and_then(|ext| ext.types.first().map(|ident| ident.value().to_string()));

                    let (mut methods, mut properties, constants, used_traits) =
                        Self::extract_class_like_members(iface.members.iter(), doc_ctx);

                    // Extract @property, @method, @mixin, @template, @extends,
                    // @implements, and @deprecated tags from the interface-level
                    // docblock.
                    let mut mixins = Vec::new();
                    let mut template_params = Vec::new();
                    let mut extends_generics = Vec::new();
                    let mut implements_generics = Vec::new();
                    let mut iface_deprecated = false;
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, iface)
                    {
                        iface_deprecated = docblock::has_deprecated_tag(doc_text);
                        template_params = docblock::extract_template_params(doc_text);
                        extends_generics = docblock::extract_generics_tag(doc_text, "@extends");
                        implements_generics =
                            docblock::extract_generics_tag(doc_text, "@implements");

                        for (name, type_str) in docblock::extract_property_tags(doc_text) {
                            if !properties.iter().any(|p| p.name == name) {
                                properties.push(PropertyInfo {
                                    name,
                                    type_hint: if type_str.is_empty() {
                                        None
                                    } else {
                                        Some(type_str)
                                    },
                                    is_static: false,
                                    visibility: Visibility::Public,
                                    is_deprecated: false,
                                });
                            }
                        }

                        for method_info in docblock::extract_method_tags(doc_text) {
                            if !methods.iter().any(|m| m.name == method_info.name) {
                                methods.push(method_info);
                            }
                        }

                        mixins = docblock::extract_mixin_tags(doc_text);
                    }

                    let start_offset = iface.left_brace.start.offset;
                    let end_offset = iface.right_brace.end.offset;

                    classes.push(ClassInfo {
                        name: iface_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        used_traits,
                        mixins,
                        is_final: false,
                        is_deprecated: iface_deprecated,
                        template_params,
                        extends_generics,
                        implements_generics,
                    });
                }
                Statement::Trait(trait_def) => {
                    let trait_name = trait_def.name.value.to_string();

                    let (mut methods, mut properties, constants, used_traits) =
                        Self::extract_class_like_members(trait_def.members.iter(), doc_ctx);

                    // Extract @property, @method, @mixin, @template, and
                    // @deprecated tags from the trait-level docblock.
                    let mut mixins = Vec::new();
                    let mut template_params = Vec::new();
                    let mut trait_deprecated = false;
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) = docblock::get_docblock_text_for_node(
                            ctx.trivias,
                            ctx.content,
                            trait_def,
                        )
                    {
                        trait_deprecated = docblock::has_deprecated_tag(doc_text);
                        template_params = docblock::extract_template_params(doc_text);

                        for (name, type_str) in docblock::extract_property_tags(doc_text) {
                            if !properties.iter().any(|p| p.name == name) {
                                properties.push(PropertyInfo {
                                    name,
                                    type_hint: if type_str.is_empty() {
                                        None
                                    } else {
                                        Some(type_str)
                                    },
                                    is_static: false,
                                    visibility: Visibility::Public,
                                    is_deprecated: false,
                                });
                            }
                        }

                        for method_info in docblock::extract_method_tags(doc_text) {
                            if !methods.iter().any(|m| m.name == method_info.name) {
                                methods.push(method_info);
                            }
                        }

                        mixins = docblock::extract_mixin_tags(doc_text);
                    }

                    let start_offset = trait_def.left_brace.start.offset;
                    let end_offset = trait_def.right_brace.end.offset;

                    classes.push(ClassInfo {
                        name: trait_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class: None,
                        used_traits,
                        mixins,
                        is_final: false,
                        is_deprecated: trait_deprecated,
                        template_params,
                        extends_generics: vec![],
                        implements_generics: vec![],
                    });
                }
                Statement::Enum(enum_def) => {
                    let enum_name = enum_def.name.value.to_string();

                    let (mut methods, mut properties, constants, mut used_traits) =
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

                    // Extract @property, @method, @mixin, and @deprecated tags
                    // from the enum-level docblock.
                    let mut mixins = Vec::new();
                    let mut enum_deprecated = false;
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, enum_def)
                    {
                        enum_deprecated = docblock::has_deprecated_tag(doc_text);

                        for (name, type_str) in docblock::extract_property_tags(doc_text) {
                            if !properties.iter().any(|p| p.name == name) {
                                properties.push(PropertyInfo {
                                    name,
                                    type_hint: if type_str.is_empty() {
                                        None
                                    } else {
                                        Some(type_str)
                                    },
                                    is_static: false,
                                    visibility: Visibility::Public,
                                    is_deprecated: false,
                                });
                            }
                        }

                        for method_info in docblock::extract_method_tags(doc_text) {
                            if !methods.iter().any(|m| m.name == method_info.name) {
                                methods.push(method_info);
                            }
                        }

                        mixins = docblock::extract_mixin_tags(doc_text);
                    }

                    // Enums can implement interfaces but cannot extend classes.
                    let parent_class = None;

                    let start_offset = enum_def.left_brace.start.offset;
                    let end_offset = enum_def.right_brace.end.offset;

                    // Enums are implicitly final — they cannot be extended.
                    classes.push(ClassInfo {
                        name: enum_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        used_traits,
                        mixins,
                        is_final: true,
                        is_deprecated: enum_deprecated,
                        template_params: vec![],
                        extends_generics: vec![],
                        implements_generics: vec![],
                    });
                }
                Statement::Namespace(namespace) => {
                    Self::extract_classes_from_statements(
                        namespace.statements().iter(),
                        classes,
                        doc_ctx,
                    );
                }
                _ => {}
            }
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
    ) -> (
        Vec<MethodInfo>,
        Vec<PropertyInfo>,
        Vec<ConstantInfo>,
        Vec<String>,
    ) {
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut constants = Vec::new();
        let mut used_traits = Vec::new();

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
                    let (return_type, conditional_return, is_deprecated) = if let Some(ctx) =
                        doc_ctx
                    {
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

                        let deprecated = docblock_text.is_some_and(docblock::has_deprecated_tag);

                        (effective, conditional, deprecated)
                    } else {
                        (native_return_type, None, false)
                    };

                    // Extract promoted properties from constructor parameters.
                    // A promoted property is a constructor parameter with a
                    // visibility modifier (e.g. `public`, `private`, `protected`).
                    if name == "__construct" {
                        for param in method.parameter_list.parameters.iter() {
                            if param.is_promoted_property() {
                                let raw_name = param.variable.name.to_string();
                                let prop_name =
                                    raw_name.strip_prefix('$').unwrap_or(&raw_name).to_string();
                                let type_hint =
                                    param.hint.as_ref().map(|h| Self::extract_hint_string(h));
                                let prop_visibility =
                                    Self::extract_visibility(param.modifiers.iter());

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
                }
            }
        }

        (methods, properties, constants, used_traits)
    }
}
