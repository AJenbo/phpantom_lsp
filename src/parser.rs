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
use crate::docblock;
use crate::types::Visibility;
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

    /// Extract property information from a class member Property node.
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

        let doc_ctx = DocblockCtx {
            trivias: program.trivia.as_slice(),
            content,
        };

        let mut classes = Vec::new();
        Self::extract_classes_from_statements(
            program.statements.iter(),
            &mut classes,
            Some(&doc_ctx),
        );
        classes
    }

    /// Parse PHP source text and extract standalone function definitions.
    ///
    /// Returns a list of `FunctionInfo` for every `function` declaration
    /// found at the top level (or inside a namespace block).
    pub fn parse_functions(&self, content: &str) -> Vec<FunctionInfo> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let doc_ctx = DocblockCtx {
            trivias: program.trivia.as_slice(),
            content,
        };

        let mut functions = Vec::new();
        Self::extract_functions_from_statements(
            program.statements.iter(),
            &mut functions,
            &None,
            Some(&doc_ctx),
        );
        functions
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
    /// Extract standalone function definitions from a sequence of statements.
    ///
    /// Recurses into `Statement::Namespace` blocks, passing the namespace
    /// name down so that each `FunctionInfo` records which namespace it
    /// belongs to (if any).
    pub(crate) fn extract_functions_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        functions: &mut Vec<FunctionInfo>,
        current_namespace: &Option<String>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        for statement in statements {
            match statement {
                Statement::Function(func) => {
                    let name = func.name.value.to_string();
                    let parameters = Self::extract_parameters(&func.parameter_list);
                    let native_return_type = func
                        .return_type_hint
                        .as_ref()
                        .map(|rth| Self::extract_hint_string(&rth.hint));

                    // Apply PHPDoc `@return` override for the function.
                    // Also extract PHPStan conditional return types if present.
                    let (return_type, conditional_return) = if let Some(ctx) = doc_ctx {
                        let docblock_text =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, func);

                        let doc_type = docblock_text.and_then(docblock::extract_return_type);

                        let effective = docblock::resolve_effective_type(
                            native_return_type.as_deref(),
                            doc_type.as_deref(),
                        );

                        let conditional =
                            docblock_text.and_then(docblock::extract_conditional_return_type);

                        (effective, conditional)
                    } else {
                        (native_return_type, None)
                    };

                    functions.push(FunctionInfo {
                        name,
                        parameters,
                        return_type,
                        namespace: current_namespace.clone(),
                        conditional_return,
                    });
                }
                Statement::Namespace(namespace) => {
                    let ns_name = namespace
                        .name
                        .as_ref()
                        .map(|ident| ident.value().to_string())
                        .filter(|s| !s.is_empty());

                    // Merge: if we already have a namespace and the inner
                    // one is set, use the inner one; otherwise keep current.
                    let effective_ns = ns_name.or_else(|| current_namespace.clone());

                    Self::extract_functions_from_statements(
                        namespace.statements().iter(),
                        functions,
                        &effective_ns,
                        doc_ctx,
                    );
                }
                // Recurse into block statements `{ ... }` to find nested
                // function declarations.
                Statement::Block(block) => {
                    Self::extract_functions_from_statements(
                        block.statements.iter(),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                // Recurse into `if` bodies — this is critical for the very
                // common PHP pattern:
                //   if (! function_exists('session')) {
                //       function session(...) { ... }
                //   }
                Statement::If(if_stmt) => {
                    Self::extract_functions_from_if_body(
                        &if_stmt.body,
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                _ => {}
            }
        }
    }

    /// Helper: recurse into an `if` statement body to extract function
    /// declarations.  Handles both brace-delimited and colon-delimited
    /// `if` bodies, including `elseif` and `else` branches.
    fn extract_functions_from_if_body<'a>(
        body: &'a IfBody<'a>,
        functions: &mut Vec<FunctionInfo>,
        current_namespace: &Option<String>,
        doc_ctx: Option<&DocblockCtx<'a>>,
    ) {
        match body {
            IfBody::Statement(body) => {
                Self::extract_functions_from_statements(
                    std::iter::once(body.statement),
                    functions,
                    current_namespace,
                    doc_ctx,
                );
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_functions_from_statements(
                        std::iter::once(else_if.statement),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_functions_from_statements(
                        std::iter::once(else_clause.statement),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
            }
            IfBody::ColonDelimited(body) => {
                Self::extract_functions_from_statements(
                    body.statements.iter(),
                    functions,
                    current_namespace,
                    doc_ctx,
                );
                for else_if in body.else_if_clauses.iter() {
                    Self::extract_functions_from_statements(
                        else_if.statements.iter(),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
                if let Some(else_clause) = &body.else_clause {
                    Self::extract_functions_from_statements(
                        else_clause.statements.iter(),
                        functions,
                        current_namespace,
                        doc_ctx,
                    );
                }
            }
        }
    }

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

                    // Extract @property, @method, and @mixin tags from the class-level docblock.
                    // These declare magic properties/methods accessible via __get/__set/__call.
                    let mut mixins = Vec::new();
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, class)
                    {
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

                    // Extract @property and @method tags from the interface-level docblock.
                    let mut mixins = Vec::new();
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, iface)
                    {
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
                    });
                }
                Statement::Trait(trait_def) => {
                    let trait_name = trait_def.name.value.to_string();

                    let (mut methods, mut properties, constants, used_traits) =
                        Self::extract_class_like_members(trait_def.members.iter(), doc_ctx);

                    // Extract @property and @method tags from the trait-level docblock.
                    let mut mixins = Vec::new();
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) = docblock::get_docblock_text_for_node(
                            ctx.trivias,
                            ctx.content,
                            trait_def,
                        )
                    {
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
    fn extract_class_like_members<'a>(
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
                    // return types if present.
                    let (return_type, conditional_return) = if let Some(ctx) = doc_ctx {
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

                        (effective, conditional)
                    } else {
                        (native_return_type, None)
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
                    });
                }
                ClassLikeMember::Property(property) => {
                    let mut prop_infos = Self::extract_property_info(property);

                    // Apply PHPDoc `@var` override for each property.
                    if let Some(ctx) = doc_ctx
                        && let Some(doc_text) =
                            docblock::get_docblock_text_for_node(ctx.trivias, ctx.content, member)
                        && let Some(doc_type) = docblock::extract_var_type(doc_text)
                    {
                        for prop in &mut prop_infos {
                            prop.type_hint = docblock::resolve_effective_type(
                                prop.type_hint.as_deref(),
                                Some(&doc_type),
                            );
                        }
                    }

                    properties.append(&mut prop_infos);
                }
                ClassLikeMember::Constant(constant) => {
                    let type_hint = constant.hint.as_ref().map(|h| Self::extract_hint_string(h));
                    let visibility = Self::extract_visibility(constant.modifiers.iter());
                    for item in constant.items.iter() {
                        constants.push(ConstantInfo {
                            name: item.name.value.to_string(),
                            type_hint: type_hint.clone(),
                            visibility,
                        });
                    }
                }
                ClassLikeMember::TraitUse(trait_use) => {
                    for trait_name_ident in trait_use.trait_names.iter() {
                        used_traits.push(trait_name_ident.value().to_string());
                    }
                }
                _ => {}
            }
        }

        (methods, properties, constants, used_traits)
    }

    /// Update the ast_map, use_map, and namespace_map for a given file URI
    /// by parsing its content.
    pub fn update_ast(&self, uri: &str, content: &str) {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let doc_ctx = DocblockCtx {
            trivias: program.trivia.as_slice(),
            content,
        };

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
                            Statement::Class(_) | Statement::Interface(_) | Statement::Trait(_) => {
                                Self::extract_classes_from_statements(
                                    std::iter::once(inner),
                                    &mut classes,
                                    Some(&doc_ctx),
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
                                    Some(&doc_ctx),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Class(_) | Statement::Interface(_) | Statement::Trait(_) => {
                    Self::extract_classes_from_statements(
                        std::iter::once(statement),
                        &mut classes,
                        Some(&doc_ctx),
                    );
                }
                _ => {}
            }
        }

        // Extract standalone functions (including those inside if-guards
        // like `if (! function_exists('...'))`) using the shared helper
        // which recurses into if/block statements.
        let mut functions = Vec::new();
        Self::extract_functions_from_statements(
            program.statements.iter(),
            &mut functions,
            &namespace,
            Some(&doc_ctx),
        );
        if !functions.is_empty()
            && let Ok(mut fmap) = self.global_functions.lock()
        {
            for func_info in functions {
                let fqn = if let Some(ref ns) = func_info.namespace {
                    format!("{}\\{}", ns, &func_info.name)
                } else {
                    func_info.name.clone()
                };

                // Insert both the FQN and the short name so that
                // callers using bare `func()` can resolve.
                fmap.insert(fqn.clone(), (uri.to_string(), func_info.clone()));
                if func_info.namespace.is_some() {
                    fmap.entry(func_info.name.clone())
                        .or_insert_with(|| (uri.to_string(), func_info));
                }
            }
        }

        // Post-process: resolve parent_class short names to fully-qualified
        // names using the file's use_map and namespace so that cross-file
        // inheritance resolution can find parent classes via PSR-4.
        Self::resolve_parent_class_names(&mut classes, &use_map, &namespace);

        let uri_string = uri.to_string();

        // Populate the class_index with FQN → URI mappings for every class
        // found in this file.  This enables reliable lookup of classes that
        // don't follow PSR-4 conventions (e.g. classes defined in Composer
        // autoload_files.php entries).
        if let Ok(mut idx) = self.class_index.lock() {
            for class in &classes {
                let fqn = if let Some(ref ns) = namespace {
                    format!("{}\\{}", ns, &class.name)
                } else {
                    class.name.clone()
                };
                idx.insert(fqn, uri_string.clone());
            }
        }

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

    /// Resolve `parent_class` short names in a list of `ClassInfo` to
    /// fully-qualified names using the file's `use_map` and `namespace`.
    ///
    /// Rules (matching PHP name resolution):
    ///   1. Already fully-qualified (`\Foo\Bar`) → strip leading `\`
    ///   2. Qualified (`Foo\Bar`) → if first segment is in use_map, expand it;
    ///      otherwise prepend current namespace
    ///   3. Unqualified (`Bar`) → check use_map; otherwise prepend namespace
    ///   4. No namespace and not in use_map → keep as-is
    pub(crate) fn resolve_parent_class_names(
        classes: &mut [ClassInfo],
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) {
        for class in classes.iter_mut() {
            if let Some(ref parent) = class.parent_class {
                let resolved = Self::resolve_name(parent, use_map, namespace);
                class.parent_class = Some(resolved);
            }
            // Resolve trait names to fully-qualified names
            class.used_traits = class
                .used_traits
                .iter()
                .map(|t| Self::resolve_name(t, use_map, namespace))
                .collect();

            // Resolve mixin names to fully-qualified names
            class.mixins = class
                .mixins
                .iter()
                .map(|m| Self::resolve_name(m, use_map, namespace))
                .collect();
        }
    }

    /// Resolve a class name to its fully-qualified form given a use_map and
    /// namespace context.
    fn resolve_name(
        name: &str,
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) -> String {
        // 1. Already fully-qualified
        if let Some(stripped) = name.strip_prefix('\\') {
            return stripped.to_string();
        }

        // 2/3. Check if the (first segment of the) name is in the use_map
        if let Some(pos) = name.find('\\') {
            // Qualified name — check first segment
            let first = &name[..pos];
            let rest = &name[pos..]; // includes leading '\'
            if let Some(fqn) = use_map.get(first) {
                return format!("{}{}", fqn, rest);
            }
        } else {
            // Unqualified name — check directly
            if let Some(fqn) = use_map.get(name) {
                return fqn.clone();
            }
        }

        // 4. Prepend current namespace if available
        if let Some(ns) = namespace {
            format!("{}\\{}", ns, name)
        } else {
            name.to_string()
        }
    }
}
