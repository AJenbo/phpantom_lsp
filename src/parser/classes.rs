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

                    // Extract interface names from `implements` clause
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

                    // Extract @property, @method, @mixin, @template, @extends,
                    // @implements, @deprecated, and @phpstan-type / @psalm-type
                    // tags from the class-level docblock.
                    let mut mixins = Vec::new();
                    let mut template_params = Vec::new();
                    let mut extends_generics = Vec::new();
                    let mut implements_generics = Vec::new();
                    let mut use_generics = Vec::new();
                    let mut type_aliases = std::collections::HashMap::new();
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
                        use_generics = docblock::extract_generics_tag(doc_text, "@use");
                        type_aliases = docblock::extract_type_aliases(doc_text);

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
                    let is_abstract = class.modifiers.contains_abstract();

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
                        mixins,
                        is_final,
                        is_abstract,
                        is_deprecated: class_deprecated,
                        template_params,
                        extends_generics,
                        implements_generics,
                        use_generics,
                        type_aliases,
                        trait_precedences,
                        trait_aliases,
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

                    let (
                        mut methods,
                        mut properties,
                        constants,
                        used_traits,
                        trait_precedences,
                        trait_aliases,
                    ) = Self::extract_class_like_members(iface.members.iter(), doc_ctx);

                    // Extract @property, @method, @mixin, @template, @extends,
                    // @implements, @deprecated, and @phpstan-type / @psalm-type
                    // tags from the interface-level docblock.
                    let mut mixins = Vec::new();
                    let mut template_params = Vec::new();
                    let mut extends_generics = Vec::new();
                    let mut implements_generics = Vec::new();
                    let mut use_generics = Vec::new();
                    let mut type_aliases = std::collections::HashMap::new();
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
                        use_generics = docblock::extract_generics_tag(doc_text, "@use");
                        type_aliases = docblock::extract_type_aliases(doc_text);

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
                        kind: ClassLikeKind::Interface,
                        name: iface_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        interfaces: vec![],
                        used_traits,
                        mixins,
                        is_final: false,
                        is_abstract: false,
                        is_deprecated: iface_deprecated,
                        template_params,
                        extends_generics,
                        implements_generics,
                        use_generics,
                        type_aliases,
                        trait_precedences,
                        trait_aliases,
                    });
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
                        mixins,
                        is_final: false,
                        is_abstract: false,
                        is_deprecated: trait_deprecated,
                        template_params,
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: vec![],
                        type_aliases: std::collections::HashMap::new(),
                        trait_precedences,
                        trait_aliases,
                    });
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

                    // Extract interface names from `implements` clause
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

                    // Enums are implicitly final — they cannot be extended.
                    classes.push(ClassInfo {
                        kind: ClassLikeKind::Enum,
                        name: enum_name,
                        methods,
                        properties,
                        constants,
                        start_offset,
                        end_offset,
                        parent_class,
                        interfaces,
                        used_traits,
                        mixins,
                        is_final: true,
                        is_abstract: false,
                        is_deprecated: enum_deprecated,
                        template_params: vec![],
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: vec![],
                        type_aliases: std::collections::HashMap::new(),
                        trait_precedences: vec![],
                        trait_aliases: vec![],
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
