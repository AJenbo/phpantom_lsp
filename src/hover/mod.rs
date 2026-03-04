//! Hover support (`textDocument/hover`).
//!
//! This module resolves the symbol under the cursor and returns a
//! human-readable description including type information, method
//! signatures, and docblock descriptions.
//!
//! The implementation reuses the same symbol-map lookup that powers
//! go-to-definition, and the same type-resolution pipeline that
//! powers completion.

mod formatting;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::resolver::ResolutionCtx;
use crate::docblock::extract_template_params_full;
use crate::symbol_map::{SymbolKind, SymbolSpan, VarDefKind};
use crate::types::*;
use crate::util::{find_class_at_offset, position_to_offset};

use formatting::*;

// ─── Origin Indicators ─────────────────────────────────────────────────────

/// Describes the origin of a member relative to the class it appears on.
///
/// Used to render a subtle indicator line above the code block in hover
/// popups so that the user can see at a glance whether a member overrides
/// a parent, implements an interface contract, or was synthesized.
enum MemberOrigin {
    /// The member overrides a parent class method/property/constant.
    Override(String),
    /// The member implements an interface method/constant.
    Implements(String),
    /// The member is virtual (synthesized from `@method`, `@property`,
    /// `@mixin`, or a framework provider).
    Virtual,
}

/// Check whether the **raw** (unmerged) class declares a member with the
/// given name and kind.
///
/// The `owner` passed to hover methods is fully resolved (inheritance +
/// virtual providers merged in).  To distinguish "this class overrides
/// the parent's method" from "this class merely inherits it", we load
/// the raw class from the class_loader and check its own member lists.
fn raw_class_has_member(
    owner: &ClassInfo,
    member_name: &str,
    member_kind: &MemberKindForOrigin,
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
) -> bool {
    // Build the FQN the same way the class loader expects.
    let fqn = match &owner.file_namespace {
        Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, owner.name),
        _ => owner.name.clone(),
    };

    // Load the raw class.  If the loader returns None (e.g. the class
    // is only known through the current file's AST and not yet indexed),
    // fall back to assuming the member is declared — this avoids hiding
    // indicators when the project is only partially indexed.
    let raw = match class_loader(&fqn) {
        Some(c) => c,
        None => return true,
    };

    match member_kind {
        MemberKindForOrigin::Method => raw
            .methods
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(member_name)),
        MemberKindForOrigin::Property => raw.properties.iter().any(|p| p.name == member_name),
        MemberKindForOrigin::Constant => raw.constants.iter().any(|c| c.name == member_name),
    }
}

/// Build the origin indicator lines for a member.
///
/// Checks whether the member is actually declared on the owner class
/// (not just inherited), then inspects the parent class and implemented
/// interfaces (via `class_loader`) to determine whether the member
/// overrides a parent or implements an interface contract.  Also checks
/// `is_virtual` for synthesized members.
///
/// Returns a (possibly empty) string of Markdown lines to prepend to the
/// hover content.
fn build_origin_lines(
    member_name: &str,
    owner: &ClassInfo,
    is_virtual: bool,
    member_kind: MemberKindForOrigin,
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
) -> String {
    let mut origins: Vec<MemberOrigin> = Vec::new();

    if is_virtual {
        origins.push(MemberOrigin::Virtual);
    }

    // Only check for override / implements when the member is actually
    // declared on the owner class itself (not merely inherited from a
    // parent).  Without this gate, an inherited method would incorrectly
    // show "overrides ParentClass".
    let declared_on_owner = raw_class_has_member(owner, member_name, &member_kind, class_loader);

    if declared_on_owner {
        // Check parent class for override.
        if let Some(ref parent_name) = owner.parent_class
            && let Some(parent) = class_loader(parent_name)
        {
            let has_member = match member_kind {
                MemberKindForOrigin::Method => parent
                    .methods
                    .iter()
                    .any(|m| m.name.eq_ignore_ascii_case(member_name)),
                MemberKindForOrigin::Property => {
                    parent.properties.iter().any(|p| p.name == member_name)
                }
                MemberKindForOrigin::Constant => {
                    parent.constants.iter().any(|c| c.name == member_name)
                }
            };
            if has_member {
                origins.push(MemberOrigin::Override(short_name(parent_name).to_string()));
            }
        }

        // Check interfaces for implements.
        for iface_name in &owner.interfaces {
            if let Some(iface) = class_loader(iface_name) {
                let has_member = match member_kind {
                    MemberKindForOrigin::Method => iface
                        .methods
                        .iter()
                        .any(|m| m.name.eq_ignore_ascii_case(member_name)),
                    MemberKindForOrigin::Property => {
                        iface.properties.iter().any(|p| p.name == member_name)
                    }
                    MemberKindForOrigin::Constant => {
                        iface.constants.iter().any(|c| c.name == member_name)
                    }
                };
                if has_member {
                    origins.push(MemberOrigin::Implements(short_name(iface_name).to_string()));
                }
            }
        }
    }

    if origins.is_empty() {
        return String::new();
    }

    let parts: Vec<String> = origins
        .iter()
        .map(|o| match o {
            MemberOrigin::Override(name) => format!("↑ overrides **{}**", name),
            MemberOrigin::Implements(name) => format!("◆ implements **{}**", name),
            MemberOrigin::Virtual => "👻 virtual".to_string(),
        })
        .collect();

    // Join with " · " when multiple apply (e.g. override + implements).
    format!("{}\n\n", parts.join(" · "))
}

/// The kind of member being checked for origin indicators.
///
/// This is separate from `MemberKind` in the definition module because
/// origin checking only needs the three broad categories.
enum MemberKindForOrigin {
    Method,
    Property,
    Constant,
}

// Re-export `pub(crate)` items so external callers keep using `crate::hover::`.
pub(crate) use formatting::{
    extract_docblock_description, extract_var_description, types_equivalent,
};

impl Backend {
    /// Handle a `textDocument/hover` request.
    ///
    /// Returns `Some(Hover)` when the symbol under the cursor can be
    /// resolved to a meaningful description, or `None` when resolution
    /// fails or the cursor is not on a navigable symbol.
    pub fn handle_hover(&self, uri: &str, content: &str, position: Position) -> Option<Hover> {
        let offset = position_to_offset(content, position);

        // Fast path: consult precomputed symbol map.
        if let Some(symbol) = self.lookup_symbol_map_for_hover(uri, offset)
            && let Some(Some(mut hover)) =
                crate::util::catch_panic_unwind_safe("hover", uri, Some(position), || {
                    self.hover_from_symbol(&symbol, uri, content, offset)
                })
        {
            hover.range = Some(symbol_span_to_range(content, &symbol));
            return Some(hover);
        }

        // Retry with offset - 1 for cursor at end-of-token (same
        // heuristic as go-to-definition).
        if offset > 0
            && let Some(symbol) = self.lookup_symbol_map_for_hover(uri, offset - 1)
            && let Some(Some(mut hover)) =
                crate::util::catch_panic_unwind_safe("hover", uri, Some(position), || {
                    self.hover_from_symbol(&symbol, uri, content, offset - 1)
                })
        {
            hover.range = Some(symbol_span_to_range(content, &symbol));
            return Some(hover);
        }

        None
    }

    /// Look up the symbol at the given byte offset for hover purposes.
    fn lookup_symbol_map_for_hover(&self, uri: &str, offset: u32) -> Option<SymbolSpan> {
        let maps = self.symbol_maps.lock().ok()?;
        let map = maps.get(uri)?;
        map.lookup(offset).cloned()
    }

    /// Dispatch a symbol-map hit to the appropriate hover path.
    fn hover_from_symbol(
        &self,
        symbol: &SymbolSpan,
        uri: &str,
        content: &str,
        cursor_offset: u32,
    ) -> Option<Hover> {
        let kind = &symbol.kind;
        let ctx = self.file_context(uri);
        let current_class = find_class_at_offset(&ctx.classes, cursor_offset);
        let class_loader = self.class_loader(&ctx);
        let function_loader = self.function_loader(&ctx);

        match kind {
            SymbolKind::Variable { name } => {
                // Suppress hover when the cursor is on a variable at its
                // definition site (parameter, foreach binding, catch, etc.)
                // — the type is already visible in the signature.
                // Assignments are the exception: the RHS type is not
                // obvious from the source text, so hover is useful there.
                if let Some(def_kind) = self.lookup_var_def_kind_at(uri, name, cursor_offset)
                    && !matches!(def_kind, VarDefKind::Assignment)
                {
                    return None;
                }
                self.hover_variable(name, uri, content, cursor_offset, current_class, &ctx)
            }

            SymbolKind::MemberAccess {
                subject_text,
                member_name,
                is_static,
                is_method_call,
            } => {
                let rctx = ResolutionCtx {
                    current_class,
                    all_classes: &ctx.classes,
                    content,
                    cursor_offset,
                    class_loader: &class_loader,
                    resolved_class_cache: Some(&self.resolved_class_cache),
                    function_loader: Some(&function_loader),
                };

                let access_kind = if *is_static {
                    AccessKind::DoubleColon
                } else {
                    AccessKind::Arrow
                };

                let candidates = crate::completion::resolver::resolve_target_classes(
                    subject_text,
                    access_kind,
                    &rctx,
                );

                for target_class in &candidates {
                    let merged = crate::virtual_members::resolve_class_fully_cached(
                        target_class,
                        &class_loader,
                        &self.resolved_class_cache,
                    );

                    if *is_method_call {
                        if let Some(method) = merged
                            .methods
                            .iter()
                            .find(|m| m.name.eq_ignore_ascii_case(member_name))
                        {
                            return Some(self.hover_for_method(method, &merged, &class_loader));
                        }
                    } else {
                        // Try property first, then constant
                        if let Some(prop) =
                            merged.properties.iter().find(|p| p.name == *member_name)
                        {
                            return Some(self.hover_for_property(prop, &merged, &class_loader));
                        }
                        if let Some(constant) =
                            merged.constants.iter().find(|c| c.name == *member_name)
                        {
                            return Some(self.hover_for_constant(constant, &merged, &class_loader));
                        }
                        // Could also be a method reference without call parens
                        if let Some(method) = merged
                            .methods
                            .iter()
                            .find(|m| m.name.eq_ignore_ascii_case(member_name))
                        {
                            return Some(self.hover_for_method(method, &merged, &class_loader));
                        }
                    }
                }
                None
            }

            SymbolKind::ClassReference { name, is_fqn } => {
                // Check whether this class reference is in a `new ClassName` context.
                // If so, show the __construct method hover instead of the class hover.
                let before = &content[..symbol.start as usize];
                let trimmed = before.trim_end();
                let is_new_context = trimmed.ends_with("new")
                    && trimmed
                        .as_bytes()
                        .get(trimmed.len().wrapping_sub(4))
                        .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');

                let resolved_name;
                let lookup_name = if *is_fqn {
                    resolved_name = format!("\\{}", name);
                    &resolved_name
                } else {
                    name.as_str()
                };

                if is_new_context && let Some(cls) = class_loader(lookup_name) {
                    let merged = crate::virtual_members::resolve_class_fully_cached(
                        &cls,
                        &class_loader,
                        &self.resolved_class_cache,
                    );
                    if let Some(constructor) = merged
                        .methods
                        .iter()
                        .find(|m| m.name.eq_ignore_ascii_case("__construct"))
                    {
                        return Some(self.hover_for_method(constructor, &merged, &class_loader));
                    }
                }

                self.hover_class_reference(
                    lookup_name,
                    *is_fqn,
                    uri,
                    &ctx,
                    &class_loader,
                    cursor_offset,
                )
            }

            SymbolKind::ClassDeclaration { .. } => {
                // The user is already at the definition site — showing
                // hover here would just repeat what they can already see.
                None
            }

            SymbolKind::FunctionCall { name } => {
                self.hover_function_call(name, &ctx, &function_loader)
            }

            SymbolKind::SelfStaticParent { keyword } => {
                // `$this` is represented as SelfStaticParent { keyword: "static" }
                // in the symbol map.  Detect it by checking the source text.
                // The cursor may land anywhere inside the `$this` token (5 bytes),
                // so look up to 4 bytes back for the `$` and check for `$this`.
                let is_this = keyword == "static" && {
                    let off = cursor_offset as usize;
                    let search_start = off.saturating_sub(4);
                    let window = content.get(search_start..off + 5).unwrap_or("");
                    window.contains("$this")
                };

                let resolved = match keyword.as_str() {
                    "self" | "static" => current_class.cloned(),
                    "parent" => current_class
                        .and_then(|cc| cc.parent_class.as_ref())
                        .and_then(|parent_name| class_loader(parent_name)),
                    _ => None,
                };
                if let Some(cls) = resolved {
                    let mut lines = Vec::new();

                    if let Some(desc) = extract_docblock_description(cls.class_docblock.as_deref())
                    {
                        lines.push(desc);
                    }

                    if let Some(ref msg) = cls.deprecation_message {
                        lines.push(format_deprecation_line(msg));
                    }

                    let ns_line = namespace_line(&cls.file_namespace);
                    if is_this {
                        lines.push(format!(
                            "```php\n<?php\n{}$this = {}\n```",
                            ns_line, cls.name
                        ));
                    } else {
                        lines.push(format!(
                            "```php\n<?php\n{}{} = {}\n```",
                            ns_line, keyword, cls.name
                        ));
                    }

                    Some(make_hover(lines.join("\n\n")))
                } else {
                    let display = if is_this { "$this" } else { keyword };
                    Some(make_hover(format!("```php\n<?php\n{}\n```", display)))
                }
            }

            SymbolKind::ConstantReference { name } => {
                // Try to find the constant in global defines
                let _defines = self.global_defines.lock().ok()?;
                Some(make_hover(format!("```php\n<?php\nconst {};\n```", name)))
            }
        }
    }

    /// Produce hover information for a variable.
    fn hover_variable(
        &self,
        name: &str,
        _uri: &str,
        content: &str,
        cursor_offset: u32,
        current_class: Option<&ClassInfo>,
        ctx: &FileContext,
    ) -> Option<Hover> {
        let var_name = format!("${}", name);

        // $this resolves to the enclosing class
        if name == "this" {
            if let Some(cc) = current_class {
                let ns_line = namespace_line(&cc.file_namespace);
                return Some(make_hover(format!(
                    "```php\n<?php\n{}$this = {}\n```",
                    ns_line, cc.name
                )));
            }
            return Some(make_hover("```php\n<?php\n$this\n```".to_string()));
        }

        let class_loader = self.class_loader(ctx);
        let function_loader = self.function_loader(ctx);

        // Use the dummy class approach same as completion for top-level code
        let dummy_class;
        let effective_class = match current_class {
            Some(cc) => cc,
            None => {
                dummy_class = ClassInfo::default();
                &dummy_class
            }
        };

        let types = crate::completion::variable::resolution::resolve_variable_types(
            &var_name,
            effective_class,
            &ctx.classes,
            content,
            cursor_offset,
            &class_loader,
            Some(&function_loader as &dyn Fn(&str) -> Option<FunctionInfo>),
        );

        if types.is_empty() {
            return Some(make_hover(format!("```php\n<?php\n{}\n```", var_name)));
        }

        let type_names: Vec<&str> = types.iter().map(|c| c.name.as_str()).collect();
        let type_str = type_names.join("|");

        Some(make_hover(format!(
            "```php\n<?php\n{} = {}\n```",
            var_name, type_str
        )))
    }

    /// Produce hover information for a class reference.
    fn hover_class_reference(
        &self,
        name: &str,
        _is_fqn: bool,
        uri: &str,
        _ctx: &FileContext,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        cursor_offset: u32,
    ) -> Option<Hover> {
        // The caller already prepends `\` for FQN names, so we can
        // call class_loader directly.
        let class_info = class_loader(name);

        if let Some(cls) = class_info {
            Some(self.hover_for_class_info(&cls))
        } else {
            // Check whether this is a template parameter in scope.
            let bare_name = name.strip_prefix('\\').unwrap_or(name);
            if let Some(tpl) = self.find_template_def_for_hover(uri, bare_name, cursor_offset) {
                return Some(tpl);
            }
            // Unknown class, just show the name
            Some(make_hover(format!(
                "```php\n<?php\nclass {};\n```",
                bare_name
            )))
        }
    }

    /// Check whether `name` is a `@template` parameter in scope at
    /// `cursor_offset` and, if so, produce a hover showing the template
    /// name and its upper bound.
    fn find_template_def_for_hover(
        &self,
        uri: &str,
        name: &str,
        cursor_offset: u32,
    ) -> Option<Hover> {
        let maps = self.symbol_maps.lock().ok()?;
        let map = maps.get(uri)?;
        let def = map.find_template_def(name, cursor_offset)?;

        let bound_display = if let Some(ref bound) = def.bound {
            format!(" of `{}`", bound)
        } else {
            String::new()
        };

        Some(make_hover(format!(
            "**{}** `{}`{}",
            def.variance.tag_name(),
            def.name,
            bound_display
        )))
    }

    /// Produce hover information for a function call.
    fn hover_function_call(
        &self,
        name: &str,
        _ctx: &FileContext,
        function_loader: &dyn Fn(&str) -> Option<FunctionInfo>,
    ) -> Option<Hover> {
        if let Some(func) = function_loader(name) {
            Some(hover_for_function(&func))
        } else {
            Some(make_hover(format!(
                "```php\n<?php\nfunction {}();\n```",
                name
            )))
        }
    }

    /// Build hover content for a method.
    fn hover_for_method(
        &self,
        method: &MethodInfo,
        owner: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Hover {
        let visibility = format_visibility(method.visibility);
        let static_kw = if method.is_static { "static " } else { "" };
        let native_params = format_native_params(&method.parameters);

        // Use native return type in the code block, effective type as docblock annotation.
        let native_ret = method
            .native_return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        let member_line = format!(
            "{}{}function {}({}){};",
            visibility, static_kw, method.name, native_params, native_ret
        );

        let mut lines = Vec::new();

        // Origin indicator (override / implements / virtual).
        let origin = build_origin_lines(
            &method.name,
            owner,
            method.is_virtual,
            MemberKindForOrigin::Method,
            class_loader,
        );
        if !origin.is_empty() {
            // `build_origin_lines` already includes a trailing "\n\n".
            lines.push(origin.trim_end().to_string());
        }

        if let Some(ref desc) = method.description {
            lines.push(desc.clone());
        }

        if let Some(ref msg) = method.deprecation_message {
            lines.push(format_deprecation_line(msg));
        }

        if let Some(ref url) = method.link {
            lines.push(format!("[{}]({})", url, url));
        }

        // Build the readable param/return section as markdown.
        if let Some(section) = build_param_return_section(
            &method.parameters,
            method.return_type.as_deref(),
            method.native_return_type.as_deref(),
            method.return_description.as_deref(),
        ) {
            lines.push(section);
        }

        let code = build_class_member_block(
            &owner.name,
            &owner.file_namespace,
            owner_kind_keyword(owner),
            &owner_name_suffix(owner),
            &member_line,
        );
        lines.push(code);

        make_hover(lines.join("\n\n"))
    }

    /// Build hover content for a property.
    fn hover_for_property(
        &self,
        property: &PropertyInfo,
        owner: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Hover {
        let visibility = format_visibility(property.visibility);
        let static_kw = if property.is_static { "static " } else { "" };

        // Use native type hint in the code block, effective type as docblock annotation.
        let native_type = property
            .native_type_hint
            .as_ref()
            .map(|t| format!("{} ", t))
            .unwrap_or_default();

        let member_line = format!(
            "{}{}{}${};",
            visibility, static_kw, native_type, property.name
        );

        // Build the docblock annotation showing the effective type
        // when it differs from the native one.
        let var_annotation = build_var_annotation(
            property.type_hint.as_deref(),
            property.native_type_hint.as_deref(),
        );

        let mut lines = Vec::new();

        // Origin indicator (override / implements / virtual).
        let origin = build_origin_lines(
            &property.name,
            owner,
            property.is_virtual,
            MemberKindForOrigin::Property,
            class_loader,
        );
        if !origin.is_empty() {
            lines.push(origin.trim_end().to_string());
        }

        if let Some(ref desc) = property.description {
            lines.push(desc.clone());
        }

        if let Some(ref msg) = property.deprecation_message {
            lines.push(format_deprecation_line(msg));
        }

        let code = build_class_member_block_with_var(
            &owner.name,
            &owner.file_namespace,
            owner_kind_keyword(owner),
            &owner_name_suffix(owner),
            &var_annotation,
            &member_line,
        );
        lines.push(code);

        make_hover(lines.join("\n\n"))
    }

    /// Build hover content for a class constant.
    fn hover_for_constant(
        &self,
        constant: &ConstantInfo,
        owner: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Hover {
        let member_line = if constant.is_enum_case {
            if let Some(ref val) = constant.enum_value {
                format!("case {} = {};", constant.name, val)
            } else {
                format!("case {};", constant.name)
            }
        } else {
            let visibility = format_visibility(constant.visibility);
            let type_hint = constant
                .type_hint
                .as_ref()
                .map(|t| format!(": {}", t))
                .unwrap_or_default();
            let value_suffix = constant
                .value
                .as_ref()
                .map(|v| format!(" = {}", v))
                .unwrap_or_default();
            format!(
                "{}const {}{}{};",
                visibility, constant.name, type_hint, value_suffix
            )
        };

        let mut lines = Vec::new();

        // Origin indicator (implements / virtual).
        let origin = build_origin_lines(
            &constant.name,
            owner,
            constant.is_virtual,
            MemberKindForOrigin::Constant,
            class_loader,
        );
        if !origin.is_empty() {
            lines.push(origin.trim_end().to_string());
        }

        if let Some(ref desc) = constant.description {
            lines.push(desc.clone());
        }

        if let Some(ref msg) = constant.deprecation_message {
            lines.push(format_deprecation_line(msg));
        }

        // Constants don't have a native vs effective type split, so no doc annotation.
        let code = build_class_member_block(
            &owner.name,
            &owner.file_namespace,
            owner_kind_keyword(owner),
            &owner_name_suffix(owner),
            &member_line,
        );
        lines.push(code);

        make_hover(lines.join("\n\n"))
    }

    /// Build hover content for a class/interface/trait/enum.
    fn hover_for_class_info(&self, cls: &ClassInfo) -> Hover {
        let kind_str = match cls.kind {
            ClassLikeKind::Class => {
                if cls.is_abstract {
                    "abstract class"
                } else if cls.is_final {
                    "final class"
                } else {
                    "class"
                }
            }
            ClassLikeKind::Interface => "interface",
            ClassLikeKind::Trait => "trait",
            ClassLikeKind::Enum => "enum",
        };

        let mut extends_implements = String::new();

        // For interfaces, `parent_class` is the first element of
        // `interfaces` (both come from the same `extends` clause),
        // so skip it to avoid duplicating the name.
        if cls.kind != ClassLikeKind::Interface
            && let Some(ref parent) = cls.parent_class
        {
            extends_implements.push_str(&format!(" extends {}", short_name(parent)));
        }

        if !cls.interfaces.is_empty() {
            let keyword = if cls.kind == ClassLikeKind::Interface {
                "extends"
            } else {
                "implements"
            };
            let short_ifaces: Vec<&str> = cls.interfaces.iter().map(|i| short_name(i)).collect();
            extends_implements.push_str(&format!(" {} {}", keyword, short_ifaces.join(", ")));
        }

        let signature = format!("{} {}{}", kind_str, cls.name, extends_implements);
        let ns_line = namespace_line(&cls.file_namespace);

        let mut lines = Vec::new();

        if let Some(desc) = extract_docblock_description(cls.class_docblock.as_deref()) {
            lines.push(desc);
        }

        if let Some(ref msg) = cls.deprecation_message {
            lines.push(format_deprecation_line(msg));
        }

        if let Some(ref url) = cls.link {
            lines.push(format!("[{}]({})", url, url));
        }

        // Show template parameters with variance and bounds.
        if let Some(ref docblock) = cls.class_docblock {
            let tpl_entries: Vec<String> = extract_template_params_full(docblock)
                .into_iter()
                .map(|(name, bound, variance)| {
                    let bound_display = bound
                        .map(|b| format!(" of `{}`", shorten_type_string(&b)))
                        .unwrap_or_default();
                    format!("**{}** `{}`{}", variance.tag_name(), name, bound_display)
                })
                .collect();
            if !tpl_entries.is_empty() {
                lines.push(tpl_entries.join("  \n"));
            }
        }

        // For enums, show cases inside the code block.
        // For traits, show public method signatures inside the code block.
        let body_lines = if cls.kind == ClassLikeKind::Enum {
            build_enum_case_body(cls)
        } else if cls.kind == ClassLikeKind::Trait {
            build_trait_summary_body(cls)
        } else {
            String::new()
        };

        if body_lines.is_empty() {
            lines.push(format!("```php\n<?php\n{}{}\n```", ns_line, signature));
        } else {
            lines.push(format!(
                "```php\n<?php\n{}{} {{\n{}}}\n```",
                ns_line, signature, body_lines
            ));
        }

        make_hover(lines.join("\n\n"))
    }
}

/// Maximum number of enum cases or trait methods to show before
/// truncating with a "and N more…" comment.
const MAX_BODY_ITEMS: usize = 30;

/// Build the body lines for an enum hover showing its cases.
///
/// Only enum cases are shown (not regular class constants).
/// Each case is rendered as `    case Name = 'value';` or `    case Name;`.
/// If there are more than [`MAX_BODY_ITEMS`] cases, the list is truncated
/// with a `// and N more…` comment.
fn build_enum_case_body(cls: &ClassInfo) -> String {
    let cases: Vec<&ConstantInfo> = cls.constants.iter().filter(|c| c.is_enum_case).collect();

    if cases.is_empty() {
        return String::new();
    }

    let mut body = String::new();
    let shown = cases.len().min(MAX_BODY_ITEMS);

    for case in &cases[..shown] {
        if let Some(ref val) = case.enum_value {
            body.push_str(&format!("    case {} = {};\n", case.name, val));
        } else {
            body.push_str(&format!("    case {};\n", case.name));
        }
    }

    if cases.len() > MAX_BODY_ITEMS {
        body.push_str(&format!(
            "    // and {} more…\n",
            cases.len() - MAX_BODY_ITEMS
        ));
    }

    body
}

/// Build the body lines for a trait hover showing public member signatures.
///
/// Shows public methods (one-line signatures without bodies), public
/// properties, and public constants. Uses native types only and short
/// (unqualified) class names for a scannable summary.
///
/// If there are more than [`MAX_BODY_ITEMS`] members, the list is
/// truncated with a `// and N more…` comment.
fn build_trait_summary_body(cls: &ClassInfo) -> String {
    let mut member_lines: Vec<String> = Vec::new();

    // Public constants.
    for constant in &cls.constants {
        if constant.visibility != Visibility::Public {
            continue;
        }
        let type_hint = constant
            .type_hint
            .as_ref()
            .map(|t| format!(": {}", t))
            .unwrap_or_default();
        let value_suffix = constant
            .value
            .as_ref()
            .map(|v| format!(" = {}", v))
            .unwrap_or_default();
        member_lines.push(format!(
            "    const {}{}{};",
            constant.name, type_hint, value_suffix
        ));
    }

    // Public properties.
    for prop in &cls.properties {
        if prop.visibility != Visibility::Public {
            continue;
        }
        let static_kw = if prop.is_static { "static " } else { "" };
        let native_type = prop
            .native_type_hint
            .as_ref()
            .map(|t| format!("{} ", t))
            .unwrap_or_default();
        member_lines.push(format!(
            "    public {}{}${};",
            static_kw, native_type, prop.name
        ));
    }

    // Public methods.
    for method in &cls.methods {
        if method.visibility != Visibility::Public {
            continue;
        }
        let static_kw = if method.is_static { "static " } else { "" };
        let native_params = format_native_params(&method.parameters);
        let native_ret = method
            .native_return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();
        member_lines.push(format!(
            "    public {}function {}({}){};",
            static_kw, method.name, native_params, native_ret
        ));
    }

    if member_lines.is_empty() {
        return String::new();
    }

    let shown = member_lines.len().min(MAX_BODY_ITEMS);
    let mut body: String = member_lines[..shown].join("\n");
    body.push('\n');

    if member_lines.len() > MAX_BODY_ITEMS {
        body.push_str(&format!(
            "    // and {} more…\n",
            member_lines.len() - MAX_BODY_ITEMS
        ));
    }

    body
}

#[cfg(test)]
mod tests;
