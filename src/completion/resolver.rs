/// Type resolution for completion subjects.
///
/// This module contains the core entry points for resolving a completion
/// subject (e.g. `$this`, `self`, `static`, `$var`, `$this->prop`,
/// `ClassName`) to a concrete `ClassInfo` so that the correct completion
/// items can be offered.
///
/// The resolution logic is split across several sibling modules:
///
/// - [`super::text_resolution`]: Text-based type resolution (scanning raw
///   source for `$var = …;` assignments, chained calls, array literals).
/// - [`super::variable_resolution`]: Variable type resolution via
///   assignment scanning and parameter type hints.
/// - [`super::type_narrowing`]: instanceof / assert / custom type guard
///   narrowing.
/// - [`super::closure_resolution`]: Closure and arrow-function parameter
///   resolution.
/// - [`crate::inheritance`]: Class inheritance merging (traits, mixins,
///   parent chain).
/// - [`super::conditional_resolution`]: PHPStan conditional return type
///   resolution at call sites.
use std::collections::HashMap;

use crate::Backend;
use crate::docblock;
use crate::docblock::types::{
    parse_generic_args, split_intersection_depth0, split_union_depth0, strip_generics,
};
use crate::inheritance::{apply_generic_args, apply_substitution};
use crate::types::*;
use crate::util::short_name;

use super::conditional_resolution::{
    resolve_conditional_with_text_args, resolve_conditional_without_args, split_call_subject,
    split_text_args,
};
use super::text_resolution::parse_bracket_segments;

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

/// Bundles the context needed by [`Backend::resolve_target_classes`] and
/// the functions it delegates to.
///
/// Introduced to replace the 8-parameter signature of
/// `resolve_target_classes` with a cleaner `(subject, access_kind, ctx)`
/// triple.  Also used directly by `resolve_call_return_types` and
/// `resolve_arg_text_to_type` (formerly `CallResolutionCtx`).
pub(crate) struct ResolutionCtx<'a> {
    /// The class the cursor is inside, if any.
    pub current_class: Option<&'a ClassInfo>,
    /// All classes known in the current file.
    pub all_classes: &'a [ClassInfo],
    /// The full source text of the current file.
    pub content: &'a str,
    /// Byte offset of the cursor in `content`.
    pub cursor_offset: u32,
    /// Cross-file class resolution callback.
    pub class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    /// Cross-file function resolution callback (optional).
    pub function_loader: FunctionLoaderFn<'a>,
}

/// Bundles the common parameters threaded through variable-type resolution.
///
/// Introducing this struct avoids passing 7–10 individual arguments to
/// every helper in the resolution chain, which keeps clippy happy and
/// makes call-sites much easier to read.
pub(super) struct VarResolutionCtx<'a> {
    pub var_name: &'a str,
    pub current_class: &'a ClassInfo,
    pub all_classes: &'a [ClassInfo],
    pub content: &'a str,
    pub cursor_offset: u32,
    pub class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    pub function_loader: FunctionLoaderFn<'a>,
}

impl<'a> VarResolutionCtx<'a> {
    /// Create a [`ResolutionCtx`] from this variable resolution context.
    ///
    /// The non-optional `current_class` is wrapped in `Some(…)`.
    pub fn as_resolution_ctx(&self) -> ResolutionCtx<'a> {
        ResolutionCtx {
            current_class: Some(self.current_class),
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset: self.cursor_offset,
            class_loader: self.class_loader,
            function_loader: self.function_loader,
        }
    }
}

/// Split a subject string at the **last** `->` or `?->` operator,
/// returning `(base, property_name)`.
///
/// Only splits at depth 0 (i.e. arrows inside balanced parentheses are
/// ignored).  Returns `None` if no arrow is found at depth 0.
///
/// # Examples
///
/// - `"$user->address"` → `Some(("$user", "address"))`
/// - `"$user->address->city"` → `Some(("$user->address", "city"))`
/// - `"$user?->address"` → `Some(("$user", "address"))`
fn split_last_arrow(subject: &str) -> Option<(&str, &str)> {
    let bytes = subject.as_bytes();
    let mut depth = 0i32;
    let mut last_arrow: Option<(usize, usize)> = None; // (start_of_arrow, start_of_prop)

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'-' if depth == 0 && i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                // Check for `?->`: the char before `-` might be `?`
                let arrow_start = if i > 0 && bytes[i - 1] == b'?' {
                    i - 1
                } else {
                    i
                };
                let prop_start = i + 2; // skip `->`
                last_arrow = Some((arrow_start, prop_start));
                i += 2; // skip past `->`
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    let (arrow_start, prop_start) = last_arrow?;
    if prop_start >= subject.len() {
        return None;
    }
    let base = &subject[..arrow_start];
    let prop = &subject[prop_start..];
    if base.is_empty() || prop.is_empty() {
        return None;
    }
    Some((base, prop))
}

impl Backend {
    /// Resolve a completion subject to all candidate class types.
    ///
    /// When a variable is assigned different types in conditional branches
    /// (e.g. an `if` block reassigns `$thing`), this returns every possible
    /// type so the caller can try each one when looking up members.
    pub(crate) fn resolve_target_classes(
        subject: &str,
        _access_kind: AccessKind,
        ctx: &ResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        let current_class = ctx.current_class;
        let all_classes = ctx.all_classes;
        let class_loader = ctx.class_loader;
        // ── Keywords that always mean "current class" ──
        if subject == "$this" || subject == "self" || subject == "static" {
            return current_class.cloned().into_iter().collect();
        }

        // ── `parent::` — resolve to the current class's parent ──
        if subject == "parent" {
            if let Some(cc) = current_class
                && let Some(ref parent_name) = cc.parent_class
            {
                // Try local lookup first
                let lookup = short_name(parent_name);
                if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                    return vec![cls.clone()];
                }
                // Fall back to cross-file / PSR-4
                return class_loader(parent_name).into_iter().collect();
            }
            return vec![];
        }

        let function_loader = ctx.function_loader;

        // ── Enum case / static member access: `ClassName::CaseName` ──
        // When an enum case or static member is used with `->`, resolve to
        // the class/enum itself (e.g. `Status::Active->label()` → `Status`).
        if !subject.starts_with('$')
            && subject.contains("::")
            && !subject.ends_with(')')
            && let Some((class_part, _case_part)) = subject.split_once("::")
        {
            let lookup = short_name(class_part);
            if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                return vec![cls.clone()];
            }
            return class_loader(class_part).into_iter().collect();
        }

        // ── Bare class name (for `::` or `->` from `new ClassName()`) ──
        if !subject.starts_with('$')
            && !subject.contains("->")
            && !subject.contains("::")
            && !subject.ends_with(')')
        {
            let lookup = short_name(subject);
            if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                return vec![cls.clone()];
            }
            // Try cross-file / PSR-4 with the full subject
            return class_loader(subject).into_iter().collect();
        }

        // ── Call expression: subject ends with ")" ──
        // Handles function calls (`app()`, `app(A::class)`),
        // method calls (`$this->getService()`),
        // and static method calls (`ClassName::make()`).
        if subject.ends_with(')')
            && let Some((call_body, args_text)) = split_call_subject(subject)
        {
            return Self::resolve_call_return_types(call_body, args_text, ctx);
        }

        // ── Property-chain: $this->prop  or  $this?->prop ──
        if let Some(prop_name) = subject
            .strip_prefix("$this->")
            .or_else(|| subject.strip_prefix("$this?->"))
        {
            if let Some(cc) = current_class {
                let resolved =
                    Self::resolve_property_types(prop_name, cc, all_classes, class_loader);
                if !resolved.is_empty() {
                    return resolved;
                }
            }
            return vec![];
        }

        // ── Property chain on non-`$this` variable: `$var->prop`, `$var->prop->sub` ──
        // When the subject starts with `$`, contains `->` (or `?->`), and
        // does not start with `$this->`, split at the last arrow to get
        // the base expression and the trailing property name, then
        // recursively resolve the base and look up the property type.
        if subject.starts_with('$')
            && !subject.starts_with("$this->")
            && !subject.starts_with("$this?->")
            && !subject.ends_with(')')
            && let Some((base, prop_name)) = split_last_arrow(subject)
        {
            let base_classes = Self::resolve_target_classes(base, _access_kind, ctx);
            let mut results = Vec::new();
            for cls in &base_classes {
                let resolved =
                    Self::resolve_property_types(prop_name, cls, all_classes, class_loader);
                ClassInfo::extend_unique(&mut results, resolved);
            }
            if !results.is_empty() {
                return results;
            }
            // If property lookup failed, don't fall through to the
            // bare `$var` branch — the subject is clearly a chain.
            return vec![];
        }

        // ── Chained array access: `$var['key'][]`, `$var['a']['b']` ──
        // When the subject has multiple bracket segments (e.g. from
        // `$response['items'][0]->`), walk through each segment to
        // resolve the final type.  This handles combinations of array
        // shape key lookups and generic element extraction.
        if subject.starts_with('$') && subject.contains('[') {
            let segments = parse_bracket_segments(subject);
            if let Some(ref segs) = segments {
                let resolved = Self::resolve_chained_array_access(
                    &segs.base_var,
                    &segs.segments,
                    ctx.content,
                    ctx.cursor_offset,
                    current_class,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }

        // ── Variable like `$var` — resolve via assignments / parameter hints ──
        if subject.starts_with('$') {
            // When the cursor is inside a class, use the enclosing class
            // for `self`/`static` resolution in type hints.  When in
            // top-level code (`current_class` is `None`), use a dummy
            // empty class so that assignment scanning still works.
            let dummy_class;
            let effective_class = match current_class {
                Some(cc) => cc,
                None => {
                    dummy_class = ClassInfo::default();
                    &dummy_class
                }
            };
            return Self::resolve_variable_types(
                subject,
                effective_class,
                all_classes,
                ctx.content,
                ctx.cursor_offset,
                class_loader,
                function_loader,
            );
        }

        vec![]
    }

    pub(super) fn resolve_call_return_types(
        call_body: &str,
        text_args: &str,
        ctx: &ResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        let current_class = ctx.current_class;
        let all_classes = ctx.all_classes;
        let class_loader = ctx.class_loader;
        let function_loader = ctx.function_loader;
        // ── Instance method call: $this->method / $var->method ──
        if let Some(pos) = call_body.rfind("->") {
            let lhs = &call_body[..pos];
            let method_name = &call_body[pos + 2..];

            // Resolve the left-hand side to a class (recursively handles
            // $this, $var, property chains, nested calls, etc.)
            //
            // IMPORTANT: the `ends_with(')')` check must come before the
            // `$this->` property-chain check, otherwise an LHS like
            // `$this->getFactory()` would be misinterpreted as a property
            // access on `getFactory()` instead of a method call.
            let lhs_classes: Vec<ClassInfo> = if lhs == "$this" || lhs == "self" || lhs == "static"
            {
                current_class.cloned().into_iter().collect()
            } else if let Some(class_name) = Self::extract_new_expression_class(lhs) {
                // Parenthesized (or bare) `new` expression:
                //   `(new Builder())`, `(new Builder)`, `new Builder()`
                // Resolve the class name to a ClassInfo.
                let lookup = short_name(&class_name);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(&class_name))
                    .into_iter()
                    .collect()
            } else if lhs.ends_with(')') {
                // LHS is itself a call expression (e.g. `app()` in
                // `app()->make(…)`, or `$this->getFactory()` in
                // `$this->getFactory()->create(…)`).
                // Recursively resolve it.
                if let Some((inner_body, inner_args)) = split_call_subject(lhs) {
                    Self::resolve_call_return_types(inner_body, inner_args, ctx)
                } else {
                    vec![]
                }
            } else if let Some(prop) = lhs
                .strip_prefix("$this->")
                .or_else(|| lhs.strip_prefix("$this?->"))
            {
                current_class
                    .map(|cc| Self::resolve_property_types(prop, cc, all_classes, class_loader))
                    .unwrap_or_default()
            } else if lhs.starts_with('$') {
                // Bare variable like `$profile` — resolve its type via
                // assignment scanning so that chains like
                // `$profile->getUser()->getEmail()` work in both
                // class-method and top-level contexts.
                Self::resolve_target_classes(lhs, AccessKind::Arrow, ctx)
            } else {
                // Unknown LHS form — skip
                vec![]
            };

            let mut results = Vec::new();
            for owner in &lhs_classes {
                // Build template substitution map when the method has
                // method-level @template params and we have arguments.
                let template_subs = if !text_args.is_empty() {
                    Self::build_method_template_subs(
                        owner,
                        method_name,
                        text_args,
                        ctx,
                        class_loader,
                    )
                } else {
                    HashMap::new()
                };
                results.extend(Self::resolve_method_return_types_with_args(
                    owner,
                    method_name,
                    text_args,
                    all_classes,
                    class_loader,
                    &template_subs,
                ));
            }
            return results;
        }

        // ── Static method call: ClassName::method / self::method ──
        if let Some(pos) = call_body.rfind("::") {
            let class_part = &call_body[..pos];
            let method_name = &call_body[pos + 2..];

            let owner_class = if class_part == "self" || class_part == "static" {
                current_class.cloned()
            } else if class_part == "parent" {
                current_class
                    .and_then(|cc| cc.parent_class.as_ref())
                    .and_then(|p| class_loader(p))
            } else {
                // Bare class name
                let lookup = short_name(class_part);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(class_part))
            };

            if let Some(ref owner) = owner_class {
                let template_subs = if !text_args.is_empty() {
                    Self::build_method_template_subs(
                        owner,
                        method_name,
                        text_args,
                        ctx,
                        class_loader,
                    )
                } else {
                    HashMap::new()
                };
                return Self::resolve_method_return_types_with_args(
                    owner,
                    method_name,
                    text_args,
                    all_classes,
                    class_loader,
                    &template_subs,
                );
            }
            return vec![];
        }

        // ── Standalone function call: app / myHelper ──
        if let Some(fl) = function_loader
            && let Some(func_info) = fl(call_body)
        {
            // If the function has a conditional return type, try to resolve
            // it using any textual arguments we preserved from the call site
            // (e.g. `app(SessionManager::class)` → text_args = "SessionManager::class").
            if let Some(ref cond) = func_info.conditional_return {
                let resolved_type = if !text_args.is_empty() {
                    resolve_conditional_with_text_args(cond, &func_info.parameters, text_args)
                } else {
                    resolve_conditional_without_args(cond, &func_info.parameters)
                };
                if let Some(ref ty) = resolved_type {
                    let classes = Self::type_hint_to_classes(ty, "", all_classes, class_loader);
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }
            if let Some(ref ret) = func_info.return_type {
                return Self::type_hint_to_classes(ret, "", all_classes, class_loader);
            }
        }

        // ── Variable invocation: $fn() ──────────────────────────────────
        // When the call body is a bare variable (e.g. `$fn`), the variable
        // holds a closure or callable.  Resolve the variable's type
        // annotation and extract the callable return type, or look for a
        // closure/arrow-function literal assignment and extract the native
        // return type hint from the source text.
        if call_body.starts_with('$') {
            let content = ctx.content;
            let cursor_offset = ctx.cursor_offset;

            // 1. Try docblock annotation: `@var Closure(): User $fn` or
            //    `@param callable(int): Response $fn`.
            if let Some(raw_type) = crate::docblock::find_iterable_raw_type_in_source(
                content,
                cursor_offset as usize,
                call_body,
            ) && let Some(ret) = crate::docblock::extract_callable_return_type(&raw_type)
            {
                let classes = Self::type_hint_to_classes(&ret, "", all_classes, class_loader);
                if !classes.is_empty() {
                    return classes;
                }
            }

            // 2. Scan backward for a closure/arrow-function literal
            //    assignment: `$fn = function(): User { … }` or
            //    `$fn = fn(): User => …`.  Extract the native return
            //    type hint from the source text.
            if let Some(ret) =
                Self::extract_closure_return_type_from_assignment(call_body, content, cursor_offset)
            {
                let classes = Self::type_hint_to_classes(&ret, "", all_classes, class_loader);
                if !classes.is_empty() {
                    return classes;
                }
            }
        }

        vec![]
    }

    /// Resolve a method call's return type, taking into account PHPStan
    /// conditional return types when `text_args` is provided, and
    /// method-level `@template` substitutions when `template_subs` is
    /// non-empty.
    ///
    /// This is the workhorse behind both `resolve_method_return_types`
    /// (which passes `""`) and the inline call-chain path (which passes
    /// the raw argument text from the source, e.g. `"CurrentCart::class"`).
    pub(super) fn resolve_method_return_types_with_args(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        template_subs: &HashMap<String, String>,
    ) -> Vec<ClassInfo> {
        // Helper: try to resolve a method's conditional return type, falling
        // back to template-substituted return type, then plain return type.
        let resolve_method = |method: &MethodInfo| -> Vec<ClassInfo> {
            // Try conditional return type first (PHPStan syntax)
            if let Some(ref cond) = method.conditional_return {
                let resolved_type = if !text_args.is_empty() {
                    resolve_conditional_with_text_args(cond, &method.parameters, text_args)
                } else {
                    resolve_conditional_without_args(cond, &method.parameters)
                };
                if let Some(ref ty) = resolved_type {
                    let classes =
                        Self::type_hint_to_classes(ty, &class_info.name, all_classes, class_loader);
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }

            // Try method-level @template substitution on the return type.
            // This handles the general case where the return type references
            // a template param (e.g. `@return Collection<T>`) and we have
            // resolved bindings from the call-site arguments.
            if !template_subs.is_empty()
                && let Some(ref ret) = method.return_type
            {
                let substituted = apply_substitution(ret, template_subs);
                if substituted != *ret {
                    let classes = Self::type_hint_to_classes(
                        &substituted,
                        &class_info.name,
                        all_classes,
                        class_loader,
                    );
                    if !classes.is_empty() {
                        return classes;
                    }
                }
            }

            // Fall back to plain return type
            if let Some(ref ret) = method.return_type {
                return Self::type_hint_to_classes(
                    ret,
                    &class_info.name,
                    all_classes,
                    class_loader,
                );
            }
            vec![]
        };

        // First check the class itself
        if let Some(method) = class_info.methods.iter().find(|m| m.name == method_name) {
            return resolve_method(method);
        }

        // Walk up the inheritance chain
        let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
        if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
            return resolve_method(method);
        }

        vec![]
    }

    /// Build a template substitution map for a method-level `@template` call.
    ///
    /// Finds the method on the class (or inherited), checks for template
    /// params and bindings, resolves argument types from `text_args` using
    /// the call resolution context, and returns a `HashMap` mapping template
    /// parameter names to their resolved concrete types.
    ///
    /// Returns an empty map if the method has no template params, no
    /// bindings, or if argument types cannot be resolved.
    pub(super) fn build_method_template_subs(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
        ctx: &ResolutionCtx<'_>,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> HashMap<String, String> {
        // Find the method — first on the class directly, then via inheritance.
        let method = class_info
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .cloned()
            .or_else(|| {
                let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
                merged.methods.into_iter().find(|m| m.name == method_name)
            });

        let method = match method {
            Some(m) if !m.template_params.is_empty() && !m.template_bindings.is_empty() => m,
            _ => return HashMap::new(),
        };

        let args = split_text_args(text_args);
        let mut subs = HashMap::new();

        for (tpl_name, param_name) in &method.template_bindings {
            // Find the parameter index for this binding.
            let param_idx = match method.parameters.iter().position(|p| p.name == *param_name) {
                Some(idx) => idx,
                None => continue,
            };

            // Get the corresponding argument text.
            let arg_text = match args.get(param_idx) {
                Some(text) => text.trim(),
                None => continue,
            };

            // Try to resolve the argument text to a type name.
            if let Some(type_name) = Self::resolve_arg_text_to_type(arg_text, ctx) {
                subs.insert(tpl_name.clone(), type_name);
            }
        }

        subs
    }

    /// Resolve an argument text string to a type name.
    ///
    /// Handles common patterns:
    /// - `ClassName::class` → `ClassName`
    /// - `new ClassName(…)` → `ClassName`
    /// - `$this` / `self` / `static` → current class name
    /// - `$this->prop` → property type
    /// - `$var` → variable type via assignment scanning
    fn resolve_arg_text_to_type(arg_text: &str, ctx: &ResolutionCtx<'_>) -> Option<String> {
        let trimmed = arg_text.trim();

        // ClassName::class → ClassName
        if let Some(name) = trimmed.strip_suffix("::class")
            && !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
        {
            return Some(name.strip_prefix('\\').unwrap_or(name).to_string());
        }

        // new ClassName(…) → ClassName
        if let Some(class_name) = Self::extract_new_expression_class(trimmed) {
            return Some(class_name);
        }

        // $this / self / static → current class
        if trimmed == "$this" || trimmed == "self" || trimmed == "static" {
            return ctx.current_class.map(|c| c.name.clone());
        }

        // $this->prop → property type
        if let Some(prop) = trimmed
            .strip_prefix("$this->")
            .or_else(|| trimmed.strip_prefix("$this?->"))
            && prop.chars().all(|c| c.is_alphanumeric() || c == '_')
            && let Some(owner) = ctx.current_class
        {
            let types =
                Self::resolve_property_types(prop, owner, ctx.all_classes, ctx.class_loader);
            if let Some(first) = types.first() {
                return Some(first.name.clone());
            }
        }

        // $var → resolve variable type
        if trimmed.starts_with('$') {
            let classes =
                Self::resolve_target_classes(trimmed, crate::types::AccessKind::Arrow, ctx);
            if let Some(first) = classes.first() {
                return Some(first.name.clone());
            }
        }

        None
    }

    /// Look up a property's type hint and resolve all candidate classes.
    ///
    /// When the type hint is a union (e.g. `A|B`), every resolvable part
    /// is returned.
    pub(crate) fn resolve_property_types(
        prop_name: &str,
        class_info: &ClassInfo,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        // Resolve inheritance so that inherited (and generic-substituted)
        // properties are visible.  For example, if `ConfigWrapper extends
        // Wrapper<Config>` and `Wrapper` has `/** @var T */ public $value`,
        // the merged class will have `$value` with type `Config`.
        let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
        let prop = match merged.properties.iter().find(|p| p.name == prop_name) {
            Some(p) => p,
            None => return vec![],
        };
        let type_hint = match prop.type_hint.as_deref() {
            Some(h) => h,
            None => return vec![],
        };
        Self::type_hint_to_classes(type_hint, &class_info.name, all_classes, class_loader)
    }

    /// Map a type-hint string to all matching `ClassInfo` values.
    ///
    /// Handles:
    ///   - Nullable types: `?Foo` → strips `?`, resolves `Foo`
    ///   - Union types: `A|B|C` → resolves each part independently
    ///     (respects `<…>` nesting so `Collection<int|string>` is not split)
    ///   - Intersection types: `A&B` → resolves each part independently
    ///   - Generic types: `Collection<int, User>` → resolves `Collection`,
    ///     then applies generic substitution (`TKey→int`, `TValue→User`)
    ///   - `self` / `static` / `$this` → owning class
    ///   - Scalar/built-in types (`int`, `string`, `bool`, `float`, `array`,
    ///     `void`, `null`, `mixed`, `never`, `object`, `callable`, `iterable`,
    ///     `false`, `true`) → skipped (not class types)
    ///
    /// Each resolvable class-like part is returned as a separate entry.
    pub(crate) fn type_hint_to_classes(
        type_hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        Self::type_hint_to_classes_depth(type_hint, owning_class_name, all_classes, class_loader, 0)
    }

    /// Inner implementation of [`type_hint_to_classes`] with a recursion
    /// depth guard to prevent infinite loops from circular type aliases.
    fn type_hint_to_classes_depth(
        type_hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: u8,
    ) -> Vec<ClassInfo> {
        // Guard against circular / deeply nested type alias resolution.
        const MAX_ALIAS_DEPTH: u8 = 10;
        if depth > MAX_ALIAS_DEPTH {
            return vec![];
        }

        let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);

        // Strip surrounding parentheses that appear in DNF types like `(A&B)|C`.
        let hint = hint
            .strip_prefix('(')
            .and_then(|h| h.strip_suffix(')'))
            .unwrap_or(hint);

        // ── Type alias resolution ──────────────────────────────────────
        // Check if `hint` is a type alias defined on the owning class
        // (via `@phpstan-type` / `@psalm-type` / `@phpstan-import-type`).
        // If so, expand the alias and resolve the underlying definition.
        //
        // This runs before union/intersection splitting because the alias
        // itself may expand to a union or intersection type.
        if let Some(alias_def) =
            Self::resolve_type_alias(hint, owning_class_name, all_classes, class_loader)
        {
            return Self::type_hint_to_classes_depth(
                &alias_def,
                owning_class_name,
                all_classes,
                class_loader,
                depth + 1,
            );
        }

        // ── Union type: split on `|` at depth 0, respecting `<…>` nesting ──
        let union_parts = split_union_depth0(hint);
        if union_parts.len() > 1 {
            let mut results = Vec::new();
            for part in union_parts {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                // Recursively resolve each part (handles self/static, scalars,
                // intersection components, etc.)
                let resolved = Self::type_hint_to_classes_depth(
                    part,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }
            return results;
        }

        // ── Intersection type: split on `&` at depth 0 and resolve each part ──
        // `User&JsonSerializable` means the value satisfies *all* listed
        // types, so completions should include members from every part.
        // Uses depth-aware splitting so that `&` inside `{…}` or `<…>`
        // (e.g. `object{foo: A&B}`) is not treated as a top-level split.
        let intersection_parts = split_intersection_depth0(hint);
        if intersection_parts.len() > 1 {
            let mut results = Vec::new();
            for part in intersection_parts {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let resolved = Self::type_hint_to_classes_depth(
                    part,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }
            return results;
        }

        // ── Object shape: `object{foo: int, bar: string}` ──────────────
        // Synthesise a ClassInfo with public properties from the shape
        // entries so that `$var->foo` resolves through normal property
        // resolution.  Object shape properties are read-only.
        if docblock::types::is_object_shape(hint)
            && let Some(entries) = docblock::parse_object_shape(hint)
        {
            let properties = entries
                .into_iter()
                .map(|e| PropertyInfo {
                    name: e.key,
                    type_hint: Some(e.value_type),
                    is_static: false,
                    visibility: Visibility::Public,
                    is_deprecated: false,
                })
                .collect();

            let synthetic = ClassInfo {
                name: "__object_shape".to_string(),
                properties,
                ..ClassInfo::default()
            };
            return vec![synthetic];
        }

        // self / static / $this always refer to the owning class.
        // In docblocks `@return $this` means "the instance the method is
        // called on" — identical to `static` for inheritance, but when the
        // method comes from a `@mixin` the return type is rewritten to the
        // mixin class name during merge (see `merge_mixins_into_recursive`).
        if hint == "self" || hint == "static" || hint == "$this" {
            return all_classes
                .iter()
                .find(|c| c.name == owning_class_name)
                .cloned()
                .or_else(|| class_loader(owning_class_name))
                .into_iter()
                .collect();
        }

        // ── Parse generic arguments (if any) ──
        // `Collection<int, User>` → base_hint = `Collection`, generic_args = ["int", "User"]
        // `Foo`                   → base_hint = `Foo`,        generic_args = []
        let (base_hint, generic_args) = parse_generic_args(hint);

        // For class lookup, strip any remaining generics from the base
        // (should already be clean, but defensive) and use the short name.
        let base_clean = strip_generics(base_hint.strip_prefix('\\').unwrap_or(base_hint));
        let lookup = short_name(&base_clean);

        // Try local (current-file) lookup by last segment
        let found = all_classes
            .iter()
            .find(|c| c.name == lookup)
            .cloned()
            .or_else(|| class_loader(base_hint));

        match found {
            Some(cls) => {
                // Apply generic substitution if the type hint carried
                // generic arguments and the class has template parameters.
                if !generic_args.is_empty() && !cls.template_params.is_empty() {
                    vec![apply_generic_args(&cls, &generic_args)]
                } else {
                    vec![cls]
                }
            }
            None => vec![],
        }
    }

    /// Look up a type alias by name and fully expand alias chains.
    ///
    /// Returns the fully expanded type definition string if `hint` is a
    /// known alias, or `None` if it is not. Follows up to 10 levels of
    /// alias indirection to handle aliases that reference other aliases.
    ///
    /// For imported aliases (`from:ClassName:OriginalName`), the source
    /// class is loaded and the original alias is resolved from its
    /// `type_aliases` map.
    ///
    /// Pass an empty `owning_class_name` to search all classes without
    /// priority (used by the array-key completion path).
    pub(crate) fn resolve_type_alias(
        hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let mut current = hint.to_string();
        let mut resolved_any = false;

        for _ in 0..10 {
            // Only bare identifiers (no `<`, `{`, `|`, `&`, `?`, `\`) can be
            // type aliases.  Skip anything that looks like a complex type
            // expression to avoid false matches.
            if current.contains('<')
                || current.contains('{')
                || current.contains('|')
                || current.contains('&')
                || current.contains('?')
                || current.contains('\\')
                || current.contains('$')
            {
                break;
            }

            let expanded = Self::resolve_type_alias_once(
                &current,
                owning_class_name,
                all_classes,
                class_loader,
            );

            match expanded {
                Some(def) => {
                    current = def;
                    resolved_any = true;
                }
                None => break,
            }
        }

        if resolved_any { Some(current) } else { None }
    }

    /// Single-level alias lookup (no chaining).
    fn resolve_type_alias_once(
        hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        // Find the owning class to check its type_aliases.
        let owning_class = all_classes.iter().find(|c| c.name == owning_class_name);

        if let Some(cls) = owning_class
            && let Some(def) = cls.type_aliases.get(hint)
        {
            // Handle imported type aliases: `from:ClassName:OriginalName`
            if let Some(import_ref) = def.strip_prefix("from:") {
                return Self::resolve_imported_type_alias(import_ref, all_classes, class_loader);
            }
            return Some(def.clone());
        }

        // Also check all classes in the file — the type alias might be
        // referenced from a method inside a different class that uses the
        // owning class's return type.  This is rare but handles the case
        // where the owning class name is empty (top-level code) or when
        // the type is used in a context where the owning class is not the
        // declaring class.
        for cls in all_classes {
            if cls.name == owning_class_name {
                continue; // Already checked above.
            }
            if let Some(def) = cls.type_aliases.get(hint) {
                if let Some(import_ref) = def.strip_prefix("from:") {
                    return Self::resolve_imported_type_alias(
                        import_ref,
                        all_classes,
                        class_loader,
                    );
                }
                return Some(def.clone());
            }
        }

        None
    }

    /// Resolve an imported type alias reference (`ClassName:OriginalName`).
    ///
    /// Loads the source class and looks up the original alias in its
    /// `type_aliases` map.
    pub(crate) fn resolve_imported_type_alias(
        import_ref: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let (source_class_name, original_name) = import_ref.split_once(':')?;

        // Try to find the source class.
        let lookup = source_class_name
            .rsplit('\\')
            .next()
            .unwrap_or(source_class_name);
        let source_class = all_classes
            .iter()
            .find(|c| c.name == lookup)
            .cloned()
            .or_else(|| class_loader(source_class_name));

        let source_class = source_class?;
        let def = source_class.type_aliases.get(original_name)?;

        // Don't follow nested imports — just return the definition.
        if def.starts_with("from:") {
            return None;
        }

        Some(def.clone())
    }
}
