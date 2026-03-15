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
pub(crate) mod variable_type;

use std::sync::Arc;
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
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
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
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
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

/// Result of searching for a member on a [`ClassInfo`] for hover purposes.
///
/// Returned by [`Backend::find_member_for_hover`] so the caller can
/// dispatch to the correct `hover_for_*` method without repeating the
/// lookup logic.
enum HoverMemberHit {
    Method(Box<MethodInfo>),
    Property(PropertyInfo),
    Constant(ConstantInfo),
}

impl Backend {
    /// Search `class` for a member matching `member_name`.
    ///
    /// When `is_method_call` is true, only methods are considered.
    /// Otherwise properties and constants are tried first, with a
    /// final fallback to methods (handles method references without
    /// call parentheses).
    fn find_member_for_hover(
        class: &ClassInfo,
        member_name: &str,
        is_method_call: bool,
    ) -> Option<HoverMemberHit> {
        if is_method_call {
            class
                .methods
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(member_name))
                .map(|m| HoverMemberHit::Method(Box::new(m.clone())))
        } else {
            if let Some(prop) = class.properties.iter().find(|p| p.name == member_name) {
                return Some(HoverMemberHit::Property(prop.clone()));
            }
            if let Some(constant) = class.constants.iter().find(|c| c.name == member_name) {
                return Some(HoverMemberHit::Constant(constant.clone()));
            }
            class
                .methods
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(member_name))
                .map(|m| HoverMemberHit::Method(Box::new(m.clone())))
        }
    }

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
        let maps = self.symbol_maps.read();
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
                // definition site where the type is already visible in
                // the signature (parameters, properties, static/global
                // declarations).  For assignments, foreach bindings, and
                // catch bindings the resolved type is not obvious from the
                // source text, so hover is useful there.
                if let Some(def_kind) = self.lookup_var_def_kind_at(uri, name, cursor_offset)
                    && !matches!(
                        def_kind,
                        VarDefKind::Assignment
                            | VarDefKind::Foreach
                            | VarDefKind::Catch
                            | VarDefKind::ArrayDestructuring
                            | VarDefKind::ListDestructuring
                    )
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
                ..
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
                    // `resolve_target_classes` already returns fully-resolved
                    // classes (via `type_hint_to_classes` which calls
                    // `resolve_class_fully` and injects model-specific scope
                    // methods).  Check the candidate directly first so that
                    // model-specific members (e.g. Eloquent scope methods
                    // injected onto Builder<Model>) are found even when the
                    // FQN-keyed resolved_class_cache holds a stale or
                    // differently-scoped entry for the same base class.
                    //
                    // Fall back to `resolve_class_fully_cached` only when
                    // the member is not on the candidate — this covers
                    // cases where the candidate was produced by a path that
                    // skips full resolution (e.g. bare class name lookup).
                    let find_result =
                        Self::find_member_for_hover(target_class, member_name, *is_method_call);

                    let (member_result, owner) = if find_result.is_some() {
                        (find_result, target_class.clone())
                    } else {
                        let merged = crate::virtual_members::resolve_class_fully_cached(
                            target_class,
                            &class_loader,
                            &self.resolved_class_cache,
                        );
                        let result =
                            Self::find_member_for_hover(&merged, member_name, *is_method_call);
                        (result, merged)
                    };

                    match member_result {
                        Some(HoverMemberHit::Method(ref method)) => {
                            return Some(self.hover_for_method(method, &owner, &class_loader));
                        }
                        Some(HoverMemberHit::Property(prop)) => {
                            return Some(self.hover_for_property(&prop, &owner, &class_loader));
                        }
                        Some(HoverMemberHit::Constant(constant)) => {
                            return Some(self.hover_for_constant(&constant, &owner, &class_loader));
                        }
                        None => {}
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

                if is_new_context && let Some(cls) = class_loader(name) {
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

                self.hover_class_reference(name, *is_fqn, uri, &ctx, &class_loader, cursor_offset)
            }

            SymbolKind::ClassDeclaration { .. } | SymbolKind::MemberDeclaration { .. } => {
                // The user is already at the definition site — showing
                // hover here would just repeat what they can already see.
                None
            }

            SymbolKind::FunctionCall { name, .. } => {
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
                        .and_then(|parent_name| {
                            class_loader(parent_name).map(Arc::unwrap_or_clone)
                        }),
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
                // Look up the constant value from global_defines or stubs.
                // If not found, check the autoload constant index
                // (populated by the `find_symbols` byte-level scan for
                // both non-Composer and Composer projects) and lazily
                // parse the defining file via `update_ast`.  As a last
                // resort, try lazily parsing known autoload files for
                // constants the byte-level scanner missed (e.g. inside
                // `if (!defined(...))` guards).
                let value_text = self
                    .global_defines
                    .read()
                    .get(name.as_str())
                    .and_then(|info| info.value.clone())
                    .or_else(|| {
                        let path = self
                            .autoload_constant_index
                            .read()
                            .get(name.as_str())
                            .cloned();
                        if let Some(path) = path
                            && let Ok(content) = std::fs::read_to_string(&path)
                        {
                            let file_uri = format!("file://{}", path.display());
                            self.update_ast(&file_uri, &content);
                            return self
                                .global_defines
                                .read()
                                .get(name.as_str())
                                .and_then(|info| info.value.clone());
                        }
                        None
                    })
                    .or_else(|| {
                        // Last-resort: lazily parse known autoload files
                        // for constants missed by the byte-level scanner.
                        let paths = self.autoload_file_paths.read().clone();
                        for path in &paths {
                            let uri = format!("file://{}", path.display());
                            if self.ast_map.read().contains_key(&uri) {
                                continue;
                            }
                            if let Ok(content) = std::fs::read_to_string(path) {
                                self.update_ast(&uri, &content);
                                let val = self
                                    .global_defines
                                    .read()
                                    .get(name.as_str())
                                    .and_then(|info| info.value.clone());
                                if val.is_some() {
                                    return val;
                                }
                            }
                        }
                        None
                    });

                let code = if let Some(ref val) = value_text {
                    format!("```php\n<?php\nconst {} = {};\n```", name, val)
                } else {
                    format!("```php\n<?php\nconst {};\n```", name)
                };
                Some(make_hover(code))
            }
        }
    }

    /// Produce hover information for a variable.
    fn hover_variable(
        &self,
        name: &str,
        uri: &str,
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

        // Try the type-string path first.  This preserves generic
        // parameters (e.g. `Generator<int, Pencil>`) and scalar types
        // (e.g. `int`) that the ClassInfo-based path would lose.
        if let Some(type_str) = variable_type::resolve_variable_type_string(
            &var_name,
            content,
            cursor_offset,
            current_class,
            &ctx.classes,
            &class_loader,
            Some(&function_loader as &dyn Fn(&str) -> Option<FunctionInfo>),
        ) {
            // When the type is a template parameter, show its variance
            // and bound (e.g. "**template-covariant** `TNode` of `AstNode`")
            // above the code block so the user sees the constraint.
            let template_line = self.find_template_info_for_type(&type_str, uri, cursor_offset);

            let hover_body = build_variable_hover_body(
                &var_name,
                &type_str,
                &class_loader,
                template_line.as_deref(),
            );
            return Some(make_hover(hover_body));
        }

        // Fall back to ClassInfo-based resolution (handles cases the
        // type-string path doesn't cover, such as instanceof narrowing
        // and complex call chains).
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

        let hover_body = build_variable_hover_body(&var_name, &type_str, &class_loader, None);
        Some(make_hover(hover_body))
    }

    /// Produce hover information for a class reference.
    fn hover_class_reference(
        &self,
        name: &str,
        _is_fqn: bool,
        uri: &str,
        _ctx: &FileContext,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        cursor_offset: u32,
    ) -> Option<Hover> {
        let class_info = class_loader(name);

        if let Some(cls) = class_info {
            Some(self.hover_for_class_info(&cls))
        } else {
            // Check whether this is a template parameter in scope.
            if let Some(tpl) = self.find_template_def_for_hover(uri, name, cursor_offset) {
                return Some(tpl);
            }
            None
        }
    }

    /// Build a template-info line for a type string that might be a
    /// template parameter.  Returns `None` when the type is not a
    /// template param in scope.
    ///
    /// For example, `"TNode"` at a cursor inside a class with
    /// `@template-covariant TNode of AstNode` returns
    /// `Some("**template-covariant** \`TNode\` of \`AstNode\`")`.
    fn find_template_info_for_type(
        &self,
        type_str: &str,
        uri: &str,
        cursor_offset: u32,
    ) -> Option<String> {
        // Only bare names (no `\`, `<`, `|`) can be template params.
        let name = type_str.trim();
        if name.is_empty()
            || name.contains('\\')
            || name.contains('<')
            || name.contains('|')
            || name.contains('&')
        {
            return None;
        }

        let maps = self.symbol_maps.read();
        let map = maps.get(uri)?;
        let def = map.find_template_def(name, cursor_offset)?;

        let bound_display = if let Some(ref bound) = def.bound {
            format!(" of `{}`", shorten_type_string(bound))
        } else {
            String::new()
        };

        Some(format!(
            "**{}** `{}`{}",
            def.variance.tag_name(),
            def.name,
            bound_display
        ))
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
        let maps = self.symbol_maps.read();
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
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
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

        // When the return type or a parameter type is a template
        // parameter on the method or owning class, show the template's
        // variance and bound so the user understands the constraint.
        // Method-level templates take priority over class-level ones.
        let mut seen_templates = Vec::new();
        if let Some(ref ret) = method.return_type
            && let Some(tpl_line) = find_template_info_in_method_or_class(ret, method, owner)
        {
            seen_templates.push(ret.clone());
            lines.push(tpl_line);
        }
        for param in &method.parameters {
            if let Some(ref hint) = param.type_hint
                && !seen_templates.iter().any(|s| s == hint)
                && let Some(tpl_line) = find_template_info_in_method_or_class(hint, method, owner)
            {
                seen_templates.push(hint.clone());
                lines.push(tpl_line);
            }
        }

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

        for url in &method.links {
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
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
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

        // When the property type is a template parameter on the owning
        // class, show the template's variance and bound so the user
        // understands the constraint (e.g. "**template-covariant**
        // `TNode` of `AstNode`").
        if let Some(ref type_hint) = property.type_hint
            && let Some(tpl_line) = find_template_info_in_class(type_hint, owner)
        {
            lines.push(tpl_line);
        }

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
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
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

        for url in &cls.links {
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

/// Resolve the namespace for a type string by loading the base type
/// through the class loader, falling back to parsing FQN strings.
///
/// Extracts the first class-like name from the type string (before any
/// `<` generic params), resolves it via the class loader, and returns
/// the resolved class's `file_namespace`.  When the class loader cannot
/// find the type (e.g. a cross-file FQN like `\App\Models\User` that
/// is not loaded), falls back to extracting the namespace directly from
/// the FQN string.
/// Build the hover body for a variable, rendering union types as
/// separate code blocks separated by a horizontal rule (`---`).
///
/// For a single type (or scalar/generic) this produces one code block
/// showing e.g. `$user = User`.
///
/// For a union like `Lamp|Faucet` it produces two code blocks
/// (`$ambiguous = Lamp` and `$ambiguous = Faucet`) joined by a
/// markdown horizontal rule so the editor renders a visible divider.
fn build_variable_hover_body(
    var_name: &str,
    type_str: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    template_line: Option<&str>,
) -> String {
    // Split the type string on `|` at the top level (respecting
    // angle brackets so that `Generator<int, Foo>|null` is not
    // split inside the generic parameters).
    let parts = split_top_level_union(type_str);

    // Count how many parts are non-trivial class types (not scalars,
    // not `null`, not `void`, etc.).  Only render separate blocks when
    // there are 2+ class-like types; a simple `Foo|null` should stay
    // in one block.
    let class_like_count = parts
        .iter()
        .filter(|p| !crate::docblock::type_strings::is_scalar(p) && **p != "null" && **p != "void")
        .count();

    // When there is only one component, or only one class-like type
    // (the rest being scalars / null), render a single code block.
    if parts.len() <= 1 || class_like_count < 2 {
        let short_type = shorten_type_string(type_str);
        let ns = resolve_type_namespace(type_str, class_loader);
        let ns_line = namespace_line(&ns);
        let code_block = format!(
            "```php\n<?php\n{}{} = {}\n```",
            ns_line, var_name, short_type
        );
        return if let Some(tpl) = template_line {
            format!("{}\n\n{}", tpl, code_block)
        } else {
            code_block
        };
    }

    // Multiple union branches — render each as its own code block
    // separated by a markdown horizontal rule.
    let mut blocks: Vec<String> = Vec::with_capacity(parts.len());
    for part in &parts {
        let short = shorten_type_string(part);
        let ns = resolve_type_namespace(part, class_loader);
        let ns_line = namespace_line(&ns);
        blocks.push(format!(
            "```php\n<?php\n{}{} = {}\n```",
            ns_line, var_name, short
        ));
    }

    let body = blocks.join("\n\n---\n\n");
    if let Some(tpl) = template_line {
        format!("{}\n\n{}", tpl, body)
    } else {
        body
    }
}

/// Split a type string on top-level `|` characters, respecting
/// angle brackets (`<…>`) and parentheses (`(…)`) so that
/// `Generator<int, Foo>|null` splits into `["Generator<int, Foo>", "null"]`
/// rather than breaking inside the generic parameters.
fn split_top_level_union(type_str: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (i, ch) in type_str.char_indices() {
        match ch {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(&type_str[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&type_str[start..]);
    parts
}

fn resolve_type_namespace(
    type_str: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    // Find the base type name: take everything before `<`, `|`, `&`,
    // `?`, or `[`.
    let base_end = type_str
        .find(['<', '|', '&', '?', '['])
        .unwrap_or(type_str.len());
    let base = type_str[..base_end].trim();

    if base.is_empty() {
        return None;
    }

    if let Some(cls) = class_loader(base) {
        return cls
            .file_namespace
            .as_ref()
            .filter(|ns| !ns.is_empty() && !ns.starts_with("___"))
            .cloned();
    }

    // Fallback: parse the namespace from the FQN string itself.
    // E.g. `App\Models\User` → `App\Models`.
    // Strip leading `\` — input may be a raw docblock type like
    // `\App\Models\User` that hasn't been through resolve_type_string.
    let canonical = base.strip_prefix('\\').unwrap_or(base);
    if let Some(pos) = canonical.rfind('\\') {
        let ns = &canonical[..pos];
        if !ns.is_empty() {
            return Some(ns.to_string());
        }
    }

    None
}

/// Check whether `type_str` is a `@template` parameter declared on
/// the method's own docblock or the owning class's docblock.  Method-level
/// templates take priority.  Returns a formatted info line like
/// `"**template** \`T\` of \`Model\`"`, or `None` when the type is
/// not a template param in either scope.
fn find_template_info_in_method_or_class(
    type_str: &str,
    method: &MethodInfo,
    owner: &ClassInfo,
) -> Option<String> {
    if let Some(line) = find_template_info_in_method(type_str, method) {
        return Some(line);
    }
    find_template_info_in_class(type_str, owner)
}

/// Check whether `type_str` is a `@template` parameter declared on
/// the method's own docblock.  Returns a formatted info line like
/// `"**template** \`T\` of \`Model\`"`, or `None` when the type is
/// not a method-level template param.
fn find_template_info_in_method(type_str: &str, method: &MethodInfo) -> Option<String> {
    let name = type_str.trim();
    if name.is_empty()
        || name.contains('\\')
        || name.contains('<')
        || name.contains('|')
        || name.contains('&')
    {
        return None;
    }

    // Method-level template_params stores just the names.
    if !method.template_params.iter().any(|p| p == name) {
        return None;
    }

    let bound_display = method
        .template_param_bounds
        .get(name)
        .map(|b| format!(" of `{}`", shorten_type_string(b)))
        .unwrap_or_default();

    // Method-level templates don't carry variance info (always invariant).
    Some(format!("**template** `{}`{}", name, bound_display))
}

/// Check whether `type_str` is a `@template` parameter declared on
/// `owner`'s class docblock.  Returns a formatted info line like
/// `"**template-covariant** \`TNode\` of \`AstNode\`"`, or `None`
/// when the type is not a template param on the class.
fn find_template_info_in_class(type_str: &str, owner: &ClassInfo) -> Option<String> {
    let name = type_str.trim();
    if name.is_empty()
        || name.contains('\\')
        || name.contains('<')
        || name.contains('|')
        || name.contains('&')
    {
        return None;
    }

    let docblock = owner.class_docblock.as_deref()?;
    let tpl = extract_template_params_full(docblock)
        .into_iter()
        .find(|(tpl_name, _, _)| tpl_name == name)?;

    let (tpl_name, bound, variance) = tpl;
    let bound_display = bound
        .map(|b| format!(" of `{}`", shorten_type_string(&b)))
        .unwrap_or_default();

    Some(format!(
        "**{}** `{}`{}",
        variance.tag_name(),
        tpl_name,
        bound_display
    ))
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
