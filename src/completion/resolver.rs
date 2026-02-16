/// Type resolution for completion subjects.
///
/// This module contains the logic for resolving a completion subject (e.g.
/// `$this`, `self`, `static`, `$var`, `$this->prop`, `ClassName`) to a concrete
/// `ClassInfo` so that the correct completion items can be offered.
///
/// Resolution strategies include:
///   - Keywords: `$this`, `self`, `static` → current class
///   - Bare class names for `::` access → look up in parsed classes
///   - Property chains: `$this->prop` → follow property type hints
///   - Variable assignments: `$var = new ClassName(…)` → resolve class
///   - Parameter type hints: `function f(Foo $x)` → resolve from hint
///   - Function call return types: `app()` → look up return type in global_functions
///   - Method call return types: `$this->getService()` → look up method return type
///   - Static method call return types: `Class::make()` → look up method return type
///   - **PHPStan conditional return types**: `app(A::class)` where `app`
///     has `@return ($abstract is class-string<TClass> ? TClass : …)` →
///     resolve to class `A`.
///   - **Ambiguous variables**: when a variable is assigned different types
///     in conditional branches (if/else, try/catch, loops), all possible
///     types are collected so that member resolution can try each one.
///   - **Union types**: `A|B` in return types, property types, and parameter
///     type hints are split into individual candidates so each one can be
///     tried when resolving members.
///
/// When a class cannot be found in the local `all_classes` slice, the
/// `class_loader` callback is invoked. This allows the caller (typically
/// the completion handler) to provide cross-file resolution — searching
/// the full `ast_map` and, if necessary, loading files from disk via
/// PSR-4 autoload mappings.
use bumpalo::Bump;
use mago_span::HasSpan;
use mago_syntax::ast::sequence::TokenSeparatedSequence;
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

use crate::Backend;
use crate::docblock;
use crate::types::Visibility;
use crate::types::*;

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

/// Bundles the common parameters threaded through variable-type resolution.
///
/// Introducing this struct avoids passing 7–10 individual arguments to
/// every helper in the resolution chain, which keeps clippy happy and
/// makes call-sites much easier to read.
struct VarResolutionCtx<'a> {
    var_name: &'a str,
    current_class: &'a ClassInfo,
    all_classes: &'a [ClassInfo],
    content: &'a str,
    cursor_offset: u32,
    class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    function_loader: FunctionLoaderFn<'a>,
}

/// Bundles the common parameters threaded through call-expression
/// return-type resolution.
///
/// This keeps the argument count of [`resolve_call_return_types`] under
/// clippy's `too_many_arguments` threshold.
struct CallResolutionCtx<'a> {
    current_class: Option<&'a ClassInfo>,
    all_classes: &'a [ClassInfo],
    content: &'a str,
    cursor_offset: u32,
    class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    function_loader: FunctionLoaderFn<'a>,
}

impl Backend {
    /// Determine which class (if any) the completion subject refers to.
    ///
    /// `current_class` is the class the cursor is inside (if any).
    /// `all_classes` is every class we know about in the current file.
    /// `content` + `cursor_offset` are used for variable-type resolution.
    /// `class_loader` is a fallback that can search across files / load
    /// classes on demand (e.g. via PSR-4).
    ///
    /// Returns an owned `ClassInfo` if the type could be resolved.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_target_class(
        subject: &str,
        access_kind: AccessKind,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<ClassInfo> {
        Self::resolve_target_classes(
            subject,
            access_kind,
            current_class,
            all_classes,
            content,
            cursor_offset,
            class_loader,
            function_loader,
        )
        .into_iter()
        .next()
    }

    /// Like `resolve_target_class`, but returns **all** candidate types.
    ///
    /// When a variable is assigned different types in conditional branches
    /// (e.g. an `if` block reassigns `$thing`), this returns every possible
    /// type so the caller can try each one when looking up members.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_target_classes(
        subject: &str,
        _access_kind: AccessKind,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
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
                let lookup = parent_name.rsplit('\\').next().unwrap_or(parent_name);
                if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                    return vec![cls.clone()];
                }
                // Fall back to cross-file / PSR-4
                return class_loader(parent_name).into_iter().collect();
            }
            return vec![];
        }

        // ── Enum case / static member access: `ClassName::CaseName` ──
        // When an enum case or static member is used with `->`, resolve to
        // the class/enum itself (e.g. `Status::Active->label()` → `Status`).
        if !subject.starts_with('$')
            && subject.contains("::")
            && !subject.ends_with(')')
            && let Some((class_part, _case_part)) = subject.split_once("::")
        {
            let lookup = class_part.rsplit('\\').next().unwrap_or(class_part);
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
            let lookup = subject.rsplit('\\').next().unwrap_or(subject);
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
            let ctx = CallResolutionCtx {
                current_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
            };
            return Self::resolve_call_return_types(call_body, args_text, &ctx);
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
                    dummy_class = ClassInfo {
                        name: String::new(),
                        methods: vec![],
                        properties: vec![],
                        constants: vec![],
                        start_offset: 0,
                        end_offset: 0,
                        parent_class: None,
                        used_traits: vec![],
                        mixins: vec![],
                        is_final: false,
                    };
                    &dummy_class
                }
            };
            return Self::resolve_variable_types(
                subject,
                effective_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
            );
        }

        vec![]
    }

    /// Resolve a call expression to the class of its return type.
    ///
    /// `call_body` is the subject without the trailing `()`, for example:
    ///   - `"app"` for a standalone function call
    ///   - `"$this->getService"` for an instance method call
    ///   - `"ClassName::make"` for a static method call
    ///
    /// The return type string is extracted from the function / method
    /// definition and then resolved to a `ClassInfo` via `class_loader`.
    ///
    /// Returns all candidate classes when the return type is a union
    /// (e.g. `A|B`).
    fn resolve_call_return_types(
        call_body: &str,
        text_args: &str,
        ctx: &CallResolutionCtx<'_>,
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
                Self::resolve_target_classes(
                    lhs,
                    AccessKind::Arrow,
                    ctx.current_class,
                    ctx.all_classes,
                    ctx.content,
                    ctx.cursor_offset,
                    ctx.class_loader,
                    ctx.function_loader,
                )
            } else {
                // Unknown LHS form — skip
                vec![]
            };

            let mut results = Vec::new();
            for owner in &lhs_classes {
                results.extend(Self::resolve_method_return_types_with_args(
                    owner,
                    method_name,
                    text_args,
                    all_classes,
                    class_loader,
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
                let lookup = class_part.rsplit('\\').next().unwrap_or(class_part);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(class_part))
            };

            if let Some(ref owner) = owner_class {
                return Self::resolve_method_return_types_with_args(
                    owner,
                    method_name,
                    text_args,
                    all_classes,
                    class_loader,
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

        vec![]
    }

    /// Look up a method's return type in a class (including inherited methods)
    /// and resolve all candidate classes.
    ///
    /// When the return type is a union (e.g. `A|B`), every resolvable part
    /// is returned as a separate candidate.
    pub(crate) fn resolve_method_return_types(
        class_info: &ClassInfo,
        method_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        Self::resolve_method_return_types_with_args(
            class_info,
            method_name,
            "",
            all_classes,
            class_loader,
        )
    }

    /// Resolve a method call's return type, taking into account PHPStan
    /// conditional return types when `text_args` is provided.
    ///
    /// This is the workhorse behind both `resolve_method_return_types`
    /// (which passes `""`) and the inline call-chain path (which passes
    /// the raw argument text from the source, e.g. `"CurrentCart::class"`).
    fn resolve_method_return_types_with_args(
        class_info: &ClassInfo,
        method_name: &str,
        text_args: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        // Helper: try to resolve a method's conditional return type, falling
        // back to the plain return type.
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
        let prop = match class_info.properties.iter().find(|p| p.name == prop_name) {
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
    ///   - `self` / `static` → owning class
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
        let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);

        // Strip surrounding parentheses that appear in DNF types like `(A&B)|C`.
        let hint = hint
            .strip_prefix('(')
            .and_then(|h| h.strip_suffix(')'))
            .unwrap_or(hint);

        // ── Union type: split on `|` and resolve each part ──
        if hint.contains('|') {
            let mut results = Vec::new();
            for part in hint.split('|') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                // Recursively resolve each part (handles self/static, scalars,
                // intersection components, etc.)
                let resolved =
                    Self::type_hint_to_classes(part, owning_class_name, all_classes, class_loader);
                for cls in resolved {
                    if !results.iter().any(|c: &ClassInfo| c.name == cls.name) {
                        results.push(cls);
                    }
                }
            }
            return results;
        }

        // ── Intersection type: split on `&` and resolve each part ──
        // `User&JsonSerializable` means the value satisfies *all* listed
        // types, so completions should include members from every part.
        if hint.contains('&') {
            let mut results = Vec::new();
            for part in hint.split('&') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let resolved =
                    Self::type_hint_to_classes(part, owning_class_name, all_classes, class_loader);
                for cls in resolved {
                    if !results.iter().any(|c: &ClassInfo| c.name == cls.name) {
                        results.push(cls);
                    }
                }
            }
            return results;
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

        // Try local (current-file) lookup by last segment
        let lookup = hint.rsplit('\\').next().unwrap_or(hint);
        if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
            return vec![cls.clone()];
        }

        // Fallback: cross-file / PSR-4 with the full hint string
        class_loader(hint).into_iter().collect()
    }

    /// Resolve the type of `$variable` by re-parsing the file and walking
    /// the method body that contains `cursor_offset`.
    ///
    /// Looks at:
    ///   1. Assignments: `$var = new ClassName(…)` / `new self` / `new static`
    ///   2. Assignments from function calls: `$var = app()` → look up return type
    ///   3. Method parameter type hints
    ///
    /// Returns all possible types when the variable is assigned different
    /// types in conditional branches.
    fn resolve_variable_types(
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let ctx = VarResolutionCtx {
            var_name,
            current_class,
            all_classes,
            content,
            cursor_offset,
            class_loader,
            function_loader,
        };

        // Walk top-level (and namespace-nested) statements to find the
        // class + method containing the cursor.
        Self::resolve_variable_in_statements(program.statements.iter(), &ctx)
    }

    fn resolve_variable_in_statements<'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        // Collect so we can iterate twice: once to check class bodies,
        // once (if needed) to walk top-level statements.
        let stmts: Vec<&Statement> = statements.collect();

        for &stmt in &stmts {
            match stmt {
                Statement::Class(class) => {
                    let start = class.left_brace.start.offset;
                    let end = class.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(class.members.iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Interface(iface) => {
                    let start = iface.left_brace.start.offset;
                    let end = iface.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(iface.members.iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Enum(enum_def) => {
                    let start = enum_def.left_brace.start.offset;
                    let end = enum_def.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(enum_def.members.iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Namespace(ns) => {
                    let results = Self::resolve_variable_in_statements(ns.statements().iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                // ── Top-level function declarations ──
                // If the cursor is inside a `function foo(Type $p) { … }`
                // at the top level, resolve the variable from its params
                // and walk its body.
                Statement::Function(func) => {
                    let body_start = func.body.left_brace.start.offset;
                    let body_end = func.body.right_brace.end.offset;
                    if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                        let mut results: Vec<ClassInfo> = Vec::new();
                        Self::resolve_closure_params(&func.parameter_list, ctx, &mut results);
                        Self::walk_statements_for_assignments(
                            func.body.statements.iter(),
                            ctx,
                            &mut results,
                            false,
                        );
                        if !results.is_empty() {
                            return results;
                        }
                    }
                }
                _ => {}
            }
        }

        // The cursor is not inside any class/interface/enum body — it must
        // be in top-level code.  Walk all top-level statements to find
        // variable assignments (e.g. `$user = new User(…);`).
        let mut results: Vec<ClassInfo> = Vec::new();
        Self::walk_statements_for_assignments(stmts.into_iter(), ctx, &mut results, false);
        results
    }

    /// Resolve a variable's type by scanning class-like members for parameter
    /// type hints and assignment expressions.
    ///
    /// Shared between `Statement::Class` and `Statement::Interface`.
    ///
    /// Returns all possible types when the variable is assigned different
    /// types in conditional branches.
    fn resolve_variable_in_members<'b>(
        members: impl Iterator<Item = &'b ClassLikeMember<'b>>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        for member in members {
            if let ClassLikeMember::Method(method) = member {
                // Collect parameter type hint as initial candidate set.
                // We no longer return early here so that the method body
                // can be scanned for instanceof narrowing / reassignments.
                let mut param_results: Vec<ClassInfo> = Vec::new();
                for param in method.parameter_list.parameters.iter() {
                    let pname = param.variable.name.to_string();
                    if pname == ctx.var_name
                        && let Some(hint) = &param.hint
                    {
                        let type_str = Self::extract_hint_string(hint);
                        let resolved = Self::type_hint_to_classes(
                            &type_str,
                            &ctx.current_class.name,
                            ctx.all_classes,
                            ctx.class_loader,
                        );
                        if !resolved.is_empty() {
                            param_results = resolved;
                            break;
                        }
                    }
                }

                if let MethodBody::Concrete(block) = &method.body {
                    let blk_start = block.left_brace.start.offset;
                    let blk_end = block.right_brace.end.offset;
                    if ctx.cursor_offset >= blk_start && ctx.cursor_offset <= blk_end {
                        // Seed the result set with the parameter type hint
                        // (if any) so that instanceof narrowing and
                        // unconditional reassignments can refine it.
                        let mut results = param_results.clone();
                        Self::walk_statements_for_assignments(
                            block.statements.iter(),
                            ctx,
                            &mut results,
                            false,
                        );
                        if !results.is_empty() {
                            return results;
                        }
                    }
                }

                // Cursor is outside the method body — return the
                // parameter type hint as-is (no body to narrow in).
                if !param_results.is_empty() {
                    return param_results;
                }
            }
        }
        vec![]
    }

    /// Walk statements collecting variable assignment types.
    ///
    /// The `conditional` flag indicates whether we are inside a conditional
    /// block (if/else, try/catch, loop).  When `conditional` is `false`,
    /// a new assignment **replaces** all previous candidates (the variable
    /// is being unconditionally reassigned).  When `conditional` is `true`,
    /// a new assignment **adds** to the list (the variable *might* be this
    /// type).
    fn walk_statements_for_assignments<'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        for stmt in statements {
            // ── Closure / arrow-function scope ──
            // If the cursor falls *inside* this statement, check whether
            // it is (or contains) a closure / arrow function whose body
            // encloses the cursor.  Closures introduce a new variable
            // scope, so we resolve entirely within that scope and stop.
            let stmt_span = stmt.span();
            if ctx.cursor_offset >= stmt_span.start.offset
                && ctx.cursor_offset <= stmt_span.end.offset
                && Self::try_resolve_in_closure_stmt(stmt, ctx, results)
            {
                return;
            }

            // Only consider statements whose start is before the cursor
            if stmt.span().start.offset >= ctx.cursor_offset {
                continue;
            }

            match stmt {
                Statement::Expression(expr_stmt) => {
                    // Try inline `/** @var Type */` override first.
                    // If the docblock resolves successfully (and passes
                    // the same override check we apply to @return), use
                    // it and skip normal resolution for this statement.
                    if !Self::try_inline_var_override(
                        expr_stmt.expression,
                        stmt.span().start.offset as usize,
                        ctx,
                        results,
                        conditional,
                    ) {
                        Self::check_expression_for_assignment(
                            expr_stmt.expression,
                            ctx,
                            results,
                            conditional,
                        );
                    }

                    // ── assert($var instanceof ClassName) narrowing ──
                    // When `assert($var instanceof Foo)` appears before
                    // the cursor, narrow the variable to `Foo` for the
                    // remainder of the current scope.
                    Self::try_apply_assert_instanceof_narrowing(expr_stmt.expression, ctx, results);

                    // ── @phpstan-assert / @psalm-assert narrowing ──
                    // When a function with `@phpstan-assert Type $param`
                    // is called as a standalone statement, narrow the
                    // corresponding argument variable unconditionally.
                    Self::try_apply_custom_assert_narrowing(expr_stmt.expression, ctx, results);

                    // ── match(true) { $var instanceof Foo => … } narrowing ──
                    Self::try_apply_match_true_narrowing(expr_stmt.expression, ctx, results);
                }
                // Recurse into blocks — these are just `{ … }` groupings,
                // not conditional, so preserve the current `conditional` flag.
                Statement::Block(block) => {
                    Self::walk_statements_for_assignments(
                        block.statements.iter(),
                        ctx,
                        results,
                        conditional,
                    );
                }
                // ── Conditional blocks: recurse with conditional = true ──
                //
                // When the condition is `$var instanceof ClassName` and the
                // cursor falls inside the corresponding branch body, we
                // *narrow* the variable to only that class — replacing all
                // previous candidates.
                Statement::If(if_stmt) => match &if_stmt.body {
                    IfBody::Statement(body) => {
                        // ── instanceof narrowing for then-body ──
                        Self::try_apply_instanceof_narrowing(
                            if_stmt.condition,
                            body.statement.span(),
                            ctx,
                            results,
                        );
                        // ── @phpstan-assert-if-true/false narrowing for then-body ──
                        Self::try_apply_assert_condition_narrowing(
                            if_stmt.condition,
                            body.statement.span(),
                            ctx,
                            results,
                            false, // not inverted — this is the then-body
                        );
                        Self::check_statement_for_assignments(body.statement, ctx, results, true);

                        for else_if in body.else_if_clauses.iter() {
                            // ── instanceof narrowing for elseif-body ──
                            Self::try_apply_instanceof_narrowing(
                                else_if.condition,
                                else_if.statement.span(),
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                else_if.condition,
                                else_if.statement.span(),
                                ctx,
                                results,
                                false,
                            );
                            Self::check_statement_for_assignments(
                                else_if.statement,
                                ctx,
                                results,
                                true,
                            );
                        }
                        if let Some(else_clause) = &body.else_clause {
                            // ── inverse instanceof narrowing for else-body ──
                            // `if ($v instanceof Foo) { … } else { ← here }`
                            // means $v is NOT Foo in the else branch.
                            Self::try_apply_instanceof_narrowing_inverse(
                                if_stmt.condition,
                                else_clause.statement.span(),
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                if_stmt.condition,
                                else_clause.statement.span(),
                                ctx,
                                results,
                                true, // inverted — this is the else-body
                            );
                            Self::check_statement_for_assignments(
                                else_clause.statement,
                                ctx,
                                results,
                                true,
                            );
                        }
                    }
                    IfBody::ColonDelimited(body) => {
                        // Determine the then-body span: from the colon to
                        // the first elseif / else / endif keyword.
                        let then_end = if !body.else_if_clauses.is_empty() {
                            body.else_if_clauses
                                .first()
                                .unwrap()
                                .elseif
                                .span()
                                .start
                                .offset
                        } else if let Some(ref ec) = body.else_clause {
                            ec.r#else.span().start.offset
                        } else {
                            body.endif.span().start.offset
                        };
                        let then_span = mago_span::Span::new(
                            body.colon.file_id,
                            body.colon.start,
                            mago_span::Position::new(then_end),
                        );
                        Self::try_apply_instanceof_narrowing(
                            if_stmt.condition,
                            then_span,
                            ctx,
                            results,
                        );
                        Self::try_apply_assert_condition_narrowing(
                            if_stmt.condition,
                            then_span,
                            ctx,
                            results,
                            false,
                        );
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                        for else_if in body.else_if_clauses.iter() {
                            let ei_span = mago_span::Span::new(
                                else_if.colon.file_id,
                                else_if.colon.start,
                                mago_span::Position::new(
                                    else_if
                                        .statements
                                        .span(else_if.colon.file_id, else_if.colon.end)
                                        .end
                                        .offset,
                                ),
                            );
                            Self::try_apply_instanceof_narrowing(
                                else_if.condition,
                                ei_span,
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                else_if.condition,
                                ei_span,
                                ctx,
                                results,
                                false,
                            );
                            Self::walk_statements_for_assignments(
                                else_if.statements.iter(),
                                ctx,
                                results,
                                true,
                            );
                        }
                        if let Some(else_clause) = &body.else_clause {
                            // ── inverse instanceof narrowing for else-body ──
                            let else_span = mago_span::Span::new(
                                else_clause.colon.file_id,
                                else_clause.colon.start,
                                mago_span::Position::new(
                                    else_clause
                                        .statements
                                        .span(else_clause.colon.file_id, else_clause.colon.end)
                                        .end
                                        .offset,
                                ),
                            );
                            Self::try_apply_instanceof_narrowing_inverse(
                                if_stmt.condition,
                                else_span,
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                if_stmt.condition,
                                else_span,
                                ctx,
                                results,
                                true, // inverted — else-body
                            );
                            Self::walk_statements_for_assignments(
                                else_clause.statements.iter(),
                                ctx,
                                results,
                                true,
                            );
                        }
                    }
                },
                Statement::Foreach(foreach) => match &foreach.body {
                    ForeachBody::Statement(inner) => {
                        Self::check_statement_for_assignments(inner, ctx, results, true);
                    }
                    ForeachBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                },
                Statement::While(while_stmt) => match &while_stmt.body {
                    WhileBody::Statement(inner) => {
                        // ── instanceof narrowing for while-body ──
                        Self::try_apply_instanceof_narrowing(
                            while_stmt.condition,
                            inner.span(),
                            ctx,
                            results,
                        );
                        Self::try_apply_assert_condition_narrowing(
                            while_stmt.condition,
                            inner.span(),
                            ctx,
                            results,
                            false,
                        );
                        Self::check_statement_for_assignments(inner, ctx, results, true);
                    }
                    WhileBody::ColonDelimited(body) => {
                        let body_span = mago_span::Span::new(
                            body.colon.file_id,
                            body.colon.start,
                            mago_span::Position::new(body.end_while.span().start.offset),
                        );
                        Self::try_apply_instanceof_narrowing(
                            while_stmt.condition,
                            body_span,
                            ctx,
                            results,
                        );
                        Self::try_apply_assert_condition_narrowing(
                            while_stmt.condition,
                            body_span,
                            ctx,
                            results,
                            false,
                        );
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                },
                Statement::For(for_stmt) => match &for_stmt.body {
                    ForBody::Statement(inner) => {
                        Self::check_statement_for_assignments(inner, ctx, results, true);
                    }
                    ForBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                },
                Statement::DoWhile(dw) => {
                    Self::check_statement_for_assignments(dw.statement, ctx, results, true);
                }
                Statement::Try(try_stmt) => {
                    Self::walk_statements_for_assignments(
                        try_stmt.block.statements.iter(),
                        ctx,
                        results,
                        true,
                    );
                    for catch in try_stmt.catch_clauses.iter() {
                        Self::walk_statements_for_assignments(
                            catch.block.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                    if let Some(finally) = &try_stmt.finally_clause {
                        Self::walk_statements_for_assignments(
                            finally.block.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Check if `condition` is `$var instanceof ClassName` (possibly
    /// parenthesised or negated) where the variable matches `ctx.var_name`.
    ///
    /// If the cursor falls inside `body_span`:
    ///   - positive match → narrow `results` to only the instanceof class
    ///   - negated match (`!($var instanceof ClassName)`) → *exclude* the
    ///     class from the current candidates
    fn try_apply_instanceof_narrowing(
        condition: &Expression<'_>,
        body_span: mago_span::Span,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
            return;
        }
        if let Some((cls_name, negated)) =
            Self::try_extract_instanceof_with_negation(condition, ctx.var_name)
        {
            if negated {
                Self::apply_instanceof_exclusion(&cls_name, ctx, results);
            } else {
                Self::apply_instanceof_inclusion(&cls_name, ctx, results);
            }
        }
    }

    /// Inverse of `try_apply_instanceof_narrowing` — used for the `else`
    /// branch of an `if ($var instanceof ClassName)` check.
    ///
    /// A positive instanceof in the condition means the variable is NOT
    /// that class inside the else body (→ exclude), and vice-versa for a
    /// negated condition (→ include only that class).
    fn try_apply_instanceof_narrowing_inverse(
        condition: &Expression<'_>,
        body_span: mago_span::Span,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
            return;
        }
        if let Some((cls_name, negated)) =
            Self::try_extract_instanceof_with_negation(condition, ctx.var_name)
        {
            // Flip the polarity: positive condition → exclude in else,
            // negated condition → include in else.
            if negated {
                Self::apply_instanceof_inclusion(&cls_name, ctx, results);
            } else {
                Self::apply_instanceof_exclusion(&cls_name, ctx, results);
            }
        }
    }

    /// Replace `results` with only the resolved classes for `cls_name`.
    fn apply_instanceof_inclusion(
        cls_name: &str,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        let narrowed = Self::type_hint_to_classes(
            cls_name,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if !narrowed.is_empty() {
            results.clear();
            for cls in narrowed {
                if !results.iter().any(|c| c.name == cls.name) {
                    results.push(cls);
                }
            }
        }
    }

    /// Remove the resolved classes for `cls_name` from `results`.
    fn apply_instanceof_exclusion(
        cls_name: &str,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        let excluded = Self::type_hint_to_classes(
            cls_name,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if !excluded.is_empty() {
            results.retain(|r| !excluded.iter().any(|e| e.name == r.name));
        }
    }

    /// If `expr` is `$var instanceof ClassName` and the variable name
    /// matches `var_name`, return the class name.
    ///
    /// Handles parenthesised expressions recursively so that
    /// `($var instanceof Foo)` also works.
    fn try_extract_instanceof<'b>(expr: &'b Expression<'b>, var_name: &str) -> Option<String> {
        match expr {
            Expression::Parenthesized(inner) => {
                Self::try_extract_instanceof(inner.expression, var_name)
            }
            Expression::Binary(bin) if bin.operator.is_instanceof() => {
                // LHS must be our variable
                let lhs_name = match bin.lhs {
                    Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
                    _ => return None,
                };
                if lhs_name != var_name {
                    return None;
                }
                // RHS is the class name
                match bin.rhs {
                    Expression::Identifier(ident) => Some(ident.value().to_string()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Like `try_extract_instanceof` but also detects negation.
    ///
    /// Returns `Some((class_name, negated))` where `negated` is `true`
    /// when the expression is `!($var instanceof ClassName)` or
    /// `!$var instanceof ClassName` (PHP precedence: `instanceof` binds
    /// tighter than `!`, so both forms are equivalent).
    ///
    /// Also handles:
    ///   - `is_a($var, ClassName::class)` — treated as equivalent to instanceof
    ///   - `get_class($var) === ClassName::class` or `==` — exact class match
    ///   - `$var::class === ClassName::class` or `==` — exact class match
    ///
    /// Handles arbitrary parenthesisation.
    fn try_extract_instanceof_with_negation<'b>(
        expr: &'b Expression<'b>,
        var_name: &str,
    ) -> Option<(String, bool)> {
        match expr {
            Expression::Parenthesized(inner) => {
                Self::try_extract_instanceof_with_negation(inner.expression, var_name)
            }
            Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
                // `!expr` — the inner expr should be a (possibly parenthesised) instanceof
                Self::try_extract_instanceof(prefix.operand, var_name)
                    .map(|cls| (cls, true))
                    .or_else(|| {
                        // Also support `!is_a($var, ClassName::class)`
                        Self::try_extract_is_a(prefix.operand, var_name).map(|cls| (cls, true))
                    })
            }
            _ => {
                Self::try_extract_instanceof(expr, var_name)
                    .map(|cls| (cls, false))
                    .or_else(|| {
                        // `is_a($var, ClassName::class)` — equivalent to instanceof
                        Self::try_extract_is_a(expr, var_name).map(|cls| (cls, false))
                    })
                    .or_else(|| {
                        // `get_class($var) === ClassName::class` or
                        // `$var::class === ClassName::class` — exact class match
                        Self::try_extract_class_identity_check(expr, var_name)
                    })
            }
        }
    }

    /// Detect `is_a($var, ClassName::class)` — semantically equivalent to
    /// `$var instanceof ClassName`.
    ///
    /// Returns the class name if the pattern matches.
    fn try_extract_is_a<'b>(expr: &'b Expression<'b>, var_name: &str) -> Option<String> {
        let expr = match expr {
            Expression::Parenthesized(inner) => inner.expression,
            other => other,
        };
        if let Expression::Call(Call::Function(func_call)) = expr {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value(),
                _ => return None,
            };
            if func_name != "is_a" {
                return None;
            }
            let args: Vec<_> = func_call.argument_list.arguments.iter().collect();
            if args.len() < 2 {
                return None;
            }
            // First argument must be our variable
            let first_expr = match &args[0] {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            let first_var = match first_expr {
                Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
                _ => return None,
            };
            if first_var != var_name {
                return None;
            }
            // Second argument should be ClassName::class
            let second_expr = match &args[1] {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            extract_class_string_from_expr(second_expr)
        } else {
            None
        }
    }

    /// Detect `get_class($var) === ClassName::class` (or `==`) and
    /// `$var::class === ClassName::class` (or `==`).
    ///
    /// Returns `Some((class_name, negated))` where `negated` is `true`
    /// for `!==` and `!=` operators.
    fn try_extract_class_identity_check<'b>(
        expr: &'b Expression<'b>,
        var_name: &str,
    ) -> Option<(String, bool)> {
        let expr = match expr {
            Expression::Parenthesized(inner) => inner.expression,
            other => other,
        };
        if let Expression::Binary(bin) = expr {
            let negated = match &bin.operator {
                BinaryOperator::Identical(_) | BinaryOperator::Equal(_) => false,
                BinaryOperator::NotIdentical(_) | BinaryOperator::NotEqual(_) => true,
                _ => return None,
            };
            // Try both orders: class-check == ClassName::class and
            // ClassName::class == class-check
            if let Some(cls) = Self::match_class_identity_pair(bin.lhs, bin.rhs, var_name) {
                return Some((cls, negated));
            }
            if let Some(cls) = Self::match_class_identity_pair(bin.rhs, bin.lhs, var_name) {
                return Some((cls, negated));
            }
        }
        None
    }

    /// Helper for `try_extract_class_identity_check`.
    ///
    /// Checks if `lhs` is a class-identity expression for `var_name`
    /// (`get_class($var)` or `$var::class`) and `rhs` is a
    /// `ClassName::class` constant.
    fn match_class_identity_pair<'b>(
        lhs: &'b Expression<'b>,
        rhs: &'b Expression<'b>,
        var_name: &str,
    ) -> Option<String> {
        let is_class_of_var =
            Self::is_get_class_of_var(lhs, var_name) || Self::is_var_class_constant(lhs, var_name);
        if !is_class_of_var {
            return None;
        }
        extract_class_string_from_expr(rhs)
    }

    /// Check if `expr` is `get_class($var)` where the variable matches.
    fn is_get_class_of_var(expr: &Expression<'_>, var_name: &str) -> bool {
        let expr = match expr {
            Expression::Parenthesized(inner) => inner.expression,
            other => other,
        };
        if let Expression::Call(Call::Function(func_call)) = expr {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value(),
                _ => return false,
            };
            if func_name != "get_class" {
                return false;
            }
            if let Some(first_arg) = func_call.argument_list.arguments.iter().next() {
                let arg_expr = match first_arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
                    return dv.name == var_name;
                }
            }
        }
        false
    }

    /// Check if `expr` is `$var::class` where the variable matches.
    fn is_var_class_constant(expr: &Expression<'_>, var_name: &str) -> bool {
        if let Expression::Access(Access::ClassConstant(cca)) = expr {
            // The class part must be our variable
            if let Expression::Variable(Variable::Direct(dv)) = cca.class {
                if dv.name != var_name {
                    return false;
                }
                // The constant selector must be `class`
                if let ClassLikeConstantSelector::Identifier(ident) = &cca.constant {
                    return ident.value == "class";
                }
            }
        }
        false
    }

    /// Apply narrowing from `@phpstan-assert` / `@psalm-assert` annotations
    /// on a function called as a standalone expression statement.
    ///
    /// Only `AssertionKind::Always` assertions are applied here — the
    /// `IfTrue` / `IfFalse` variants are handled by
    /// `try_apply_assert_condition_narrowing`.
    fn try_apply_custom_assert_narrowing(
        expr: &Expression<'_>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        let expr = match expr {
            Expression::Parenthesized(inner) => inner.expression,
            other => other,
        };
        if let Expression::Call(Call::Function(func_call)) = expr {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return,
            };
            let func_info = match ctx.function_loader {
                Some(fl) => match fl(&func_name) {
                    Some(fi) => fi,
                    None => return,
                },
                None => return,
            };
            for assertion in &func_info.type_assertions {
                if assertion.kind != AssertionKind::Always {
                    continue;
                }
                // Find the parameter index for this assertion
                if let Some(arg_var) = Self::find_assertion_arg_variable(
                    &func_call.argument_list,
                    &assertion.param_name,
                    &func_info.parameters,
                ) && arg_var == ctx.var_name
                {
                    if assertion.negated {
                        Self::apply_instanceof_exclusion(&assertion.asserted_type, ctx, results);
                    } else {
                        Self::apply_instanceof_inclusion(&assertion.asserted_type, ctx, results);
                    }
                }
            }
        }
    }

    /// Apply narrowing from `@phpstan-assert-if-true` / `-if-false`
    /// annotations on a function call used as an `if` / `while` condition.
    ///
    /// * `inverted == false` → we're in the then-body (or while-body):
    ///   apply `IfTrue` assertions (and `IfFalse` if the condition is negated).
    /// * `inverted == true` → we're in the else-body:
    ///   apply `IfFalse` assertions (and `IfTrue` if the condition is negated).
    fn try_apply_assert_condition_narrowing(
        condition: &Expression<'_>,
        body_span: mago_span::Span,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        inverted: bool,
    ) {
        if ctx.cursor_offset < body_span.start.offset || ctx.cursor_offset > body_span.end.offset {
            return;
        }

        // Unwrap parentheses and detect negation (`!func($var)`)
        let (func_call_expr, condition_negated) = Self::unwrap_condition_negation(condition);

        let func_call = match func_call_expr {
            Expression::Call(Call::Function(fc)) => fc,
            _ => return,
        };
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value().to_string(),
            _ => return,
        };
        let func_info = match ctx.function_loader {
            Some(fl) => match fl(&func_name) {
                Some(fi) => fi,
                None => return,
            },
            None => return,
        };

        // Determine whether the function returned true in this branch.
        //
        // - then-body (inverted=false), no negation  → function returned true
        // - then-body (inverted=false), negated       → function returned false
        // - else-body (inverted=true),  no negation  → function returned false
        // - else-body (inverted=true),  negated       → function returned true
        let function_returned_true = !(inverted ^ condition_negated);

        for assertion in &func_info.type_assertions {
            // Determine if this assertion's condition is satisfied in this
            // branch.  IfTrue assertions apply positively when the function
            // returned true; IfFalse assertions apply positively when the
            // function returned false.  In the opposite branch, we apply
            // the *inverse* (exclude instead of include, and vice-versa).
            let applies_positively = match assertion.kind {
                AssertionKind::IfTrue => function_returned_true,
                AssertionKind::IfFalse => !function_returned_true,
                AssertionKind::Always => continue, // handled elsewhere
            };

            if let Some(arg_var) = Self::find_assertion_arg_variable(
                &func_call.argument_list,
                &assertion.param_name,
                &func_info.parameters,
            ) && arg_var == ctx.var_name
            {
                // XOR the assertion's own negation with whether we're in the
                // opposite branch: positive + non-negated → include,
                // positive + negated → exclude, opposite + non-negated → exclude,
                // opposite + negated → include.
                let should_exclude = assertion.negated ^ !applies_positively;
                if should_exclude {
                    Self::apply_instanceof_exclusion(&assertion.asserted_type, ctx, results);
                } else {
                    Self::apply_instanceof_inclusion(&assertion.asserted_type, ctx, results);
                }
            }
        }
    }

    /// Unwrap parentheses and a single `!` prefix from a condition,
    /// returning `(inner_expr, negated)`.
    fn unwrap_condition_negation<'b>(expr: &'b Expression<'b>) -> (&'b Expression<'b>, bool) {
        match expr {
            Expression::Parenthesized(inner) => Self::unwrap_condition_negation(inner.expression),
            Expression::UnaryPrefix(prefix) if prefix.operator.is_not() => {
                let (inner, already_negated) = Self::unwrap_condition_negation(prefix.operand);
                (inner, !already_negated)
            }
            _ => (expr, false),
        }
    }

    /// Given a function's argument list and a parameter name (with `$`
    /// prefix), find the variable name passed at that parameter's position.
    ///
    /// Returns `Some("$varName")` if the argument at the matching position
    /// is a simple direct variable.
    fn find_assertion_arg_variable(
        argument_list: &ArgumentList<'_>,
        param_name: &str,
        parameters: &[ParameterInfo],
    ) -> Option<String> {
        // Find the parameter index
        let param_idx = parameters.iter().position(|p| p.name == param_name)?;

        // Get the argument at that position
        let arg = argument_list.arguments.iter().nth(param_idx)?;
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };

        // The argument must be a simple variable
        match arg_expr {
            Expression::Variable(Variable::Direct(dv)) => Some(dv.name.to_string()),
            _ => None,
        }
    }

    /// If `expr` is `assert($var instanceof ClassName)` (or the negated
    /// form `assert(!$var instanceof ClassName)`), narrow or exclude
    /// `results` accordingly.
    ///
    /// Unlike `if`-based narrowing which is scoped to the block body,
    /// `assert()` narrows unconditionally for all subsequent code in the
    /// same scope — the statement being before the cursor is already
    /// guaranteed by the caller.
    fn try_apply_assert_instanceof_narrowing(
        expr: &Expression<'_>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        if let Some((cls_name, negated)) = Self::try_extract_assert_instanceof(expr, ctx.var_name) {
            if negated {
                Self::apply_instanceof_exclusion(&cls_name, ctx, results);
            } else {
                Self::apply_instanceof_inclusion(&cls_name, ctx, results);
            }
        }
    }

    /// If `expr` is `assert($var instanceof ClassName)` (or the negated
    /// form), return `Some((class_name, negated))`.
    ///
    /// Supports parenthesised inner expressions and the function name
    /// `assert`.
    fn try_extract_assert_instanceof<'b>(
        expr: &'b Expression<'b>,
        var_name: &str,
    ) -> Option<(String, bool)> {
        // Unwrap parenthesised wrapper on the whole expression
        let expr = match expr {
            Expression::Parenthesized(inner) => inner.expression,
            other => other,
        };
        if let Expression::Call(Call::Function(func_call)) = expr {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return None,
            };
            if func_name != "assert" {
                return None;
            }
            // The first argument should be the instanceof expression
            // (possibly negated), or is_a / class-identity check
            if let Some(first_arg) = func_call.argument_list.arguments.iter().next() {
                let arg_expr = match first_arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                return Self::try_extract_instanceof_with_negation(arg_expr, var_name);
            }
        }
        None
    }

    /// Check if the cursor is inside a `match (true)` arm whose
    /// condition is `$var instanceof ClassName` and, if so, narrow
    /// the results for the arm body.
    ///
    /// Supports patterns like:
    /// ```php
    /// match (true) {
    ///     $value instanceof AdminUser => $value->doAdmin(),
    ///     //                             ^cursor here
    /// };
    /// ```
    fn try_apply_match_true_narrowing(
        expr: &Expression<'_>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        // Unwrap parenthesised wrapper
        let expr = match expr {
            Expression::Parenthesized(inner) => inner.expression,
            other => other,
        };
        let match_expr = match expr {
            Expression::Match(m) => m,
            // Also handle `$var = match(true) { … }`
            Expression::Assignment(a) => {
                if let Expression::Match(m) = a.rhs {
                    m
                } else {
                    return;
                }
            }
            _ => return,
        };
        // The subject must be `true` for instanceof conditions to make sense
        if !match_expr.expression.is_true() {
            return;
        }
        for arm in match_expr.arms.iter() {
            if let MatchArm::Expression(expr_arm) = arm {
                let body_span = expr_arm.expression.span();
                if ctx.cursor_offset < body_span.start.offset
                    || ctx.cursor_offset > body_span.end.offset
                {
                    continue;
                }
                // Check each condition in this arm (comma-separated)
                for condition in expr_arm.conditions.iter() {
                    if let Some((cls_name, negated)) =
                        Self::try_extract_instanceof_with_negation(condition, ctx.var_name)
                    {
                        if negated {
                            Self::apply_instanceof_exclusion(&cls_name, ctx, results);
                        } else {
                            Self::apply_instanceof_inclusion(&cls_name, ctx, results);
                        }
                    }
                }
            }
        }
    }

    /// Helper: treat a single statement as an iterator of one and recurse.
    fn check_statement_for_assignments<'b>(
        stmt: &'b Statement<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        Self::walk_statements_for_assignments(std::iter::once(stmt), ctx, results, conditional);
    }

    // ────────────────────────────────────────────────────────────────────
    // Closure / arrow-function parameter resolution
    // ────────────────────────────────────────────────────────────────────

    /// Check whether `stmt` contains a closure or arrow function whose
    /// body encloses the cursor.  If so, resolve the variable from the
    /// closure's parameter list and walk its body, then return `true`.
    fn try_resolve_in_closure_stmt<'b>(
        stmt: &'b Statement<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        match stmt {
            Statement::Expression(expr_stmt) => {
                Self::try_resolve_in_closure_expr(expr_stmt.expression, ctx, results)
            }
            Statement::Return(ret) => {
                if let Some(val) = &ret.value {
                    Self::try_resolve_in_closure_expr(val, ctx, results)
                } else {
                    false
                }
            }
            Statement::Block(block) => {
                for inner in block.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                false
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    Self::try_resolve_in_closure_stmt(body.statement, ctx, results)
                }
                IfBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::Foreach(foreach) => match &foreach.body {
                ForeachBody::Statement(inner) => {
                    Self::try_resolve_in_closure_stmt(inner, ctx, results)
                }
                ForeachBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(inner) => {
                    Self::try_resolve_in_closure_stmt(inner, ctx, results)
                }
                WhileBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(inner) => Self::try_resolve_in_closure_stmt(inner, ctx, results),
                ForBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::DoWhile(dw) => Self::try_resolve_in_closure_stmt(dw.statement, ctx, results),
            Statement::Try(try_stmt) => {
                for inner in try_stmt.block.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                for catch in try_stmt.catch_clauses.iter() {
                    for inner in catch.block.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for inner in finally.block.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Recursively search an expression tree for a `Closure` or
    /// `ArrowFunction` whose body contains the cursor.  When found,
    /// resolve the target variable from the closure's parameters and
    /// walk its body statements, returning `true`.
    fn try_resolve_in_closure_expr<'b>(
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        // Quick span-based prune: if the cursor is not within this
        // expression at all, skip the entire sub-tree.
        let sp = expr.span();
        if ctx.cursor_offset < sp.start.offset || ctx.cursor_offset > sp.end.offset {
            return false;
        }

        match expr {
            // ── Closure: `function (Type $param) { … }` ──
            Expression::Closure(closure) => {
                let body_start = closure.body.left_brace.start.offset;
                let body_end = closure.body.right_brace.end.offset;
                if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                    Self::resolve_closure_params(&closure.parameter_list, ctx, results);
                    Self::walk_statements_for_assignments(
                        closure.body.statements.iter(),
                        ctx,
                        results,
                        false,
                    );
                    return true;
                }
                false
            }
            // ── Arrow function: `fn(Type $param) => expr` ──
            Expression::ArrowFunction(arrow) => {
                let arrow_body_span = arrow.expression.span();
                if ctx.cursor_offset >= arrow.arrow.start.offset
                    && ctx.cursor_offset <= arrow_body_span.end.offset
                {
                    Self::resolve_closure_params(&arrow.parameter_list, ctx, results);
                    // Arrow functions have a single expression body — no
                    // statements to walk, but the params are resolved.
                    return true;
                }
                false
            }
            // ── Recurse into sub-expressions that might contain closures ──
            Expression::Parenthesized(p) => {
                Self::try_resolve_in_closure_expr(p.expression, ctx, results)
            }
            Expression::Assignment(a) => {
                Self::try_resolve_in_closure_expr(a.lhs, ctx, results)
                    || Self::try_resolve_in_closure_expr(a.rhs, ctx, results)
            }
            Expression::Binary(bin) => {
                Self::try_resolve_in_closure_expr(bin.lhs, ctx, results)
                    || Self::try_resolve_in_closure_expr(bin.rhs, ctx, results)
            }
            Expression::Conditional(cond) => {
                Self::try_resolve_in_closure_expr(cond.condition, ctx, results)
                    || cond
                        .then
                        .is_some_and(|e| Self::try_resolve_in_closure_expr(e, ctx, results))
                    || Self::try_resolve_in_closure_expr(cond.r#else, ctx, results)
            }
            Expression::Call(call) => Self::try_resolve_in_closure_call(call, ctx, results),
            Expression::Array(arr) => {
                for elem in arr.elements.iter() {
                    let found = match elem {
                        ArrayElement::KeyValue(kv) => {
                            Self::try_resolve_in_closure_expr(kv.key, ctx, results)
                                || Self::try_resolve_in_closure_expr(kv.value, ctx, results)
                        }
                        ArrayElement::Value(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Variadic(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Missing(_) => false,
                    };
                    if found {
                        return true;
                    }
                }
                false
            }
            Expression::LegacyArray(arr) => {
                for elem in arr.elements.iter() {
                    let found = match elem {
                        ArrayElement::KeyValue(kv) => {
                            Self::try_resolve_in_closure_expr(kv.key, ctx, results)
                                || Self::try_resolve_in_closure_expr(kv.value, ctx, results)
                        }
                        ArrayElement::Value(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Variadic(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Missing(_) => false,
                    };
                    if found {
                        return true;
                    }
                }
                false
            }
            Expression::Match(m) => {
                if Self::try_resolve_in_closure_expr(m.expression, ctx, results) {
                    return true;
                }
                for arm in m.arms.iter() {
                    if Self::try_resolve_in_closure_expr(arm.expression(), ctx, results) {
                        return true;
                    }
                }
                false
            }
            Expression::Access(access) => match access {
                Access::Property(pa) => Self::try_resolve_in_closure_expr(pa.object, ctx, results),
                Access::NullSafeProperty(pa) => {
                    Self::try_resolve_in_closure_expr(pa.object, ctx, results)
                }
                Access::StaticProperty(pa) => {
                    Self::try_resolve_in_closure_expr(pa.class, ctx, results)
                }
                Access::ClassConstant(pa) => {
                    Self::try_resolve_in_closure_expr(pa.class, ctx, results)
                }
            },
            Expression::Instantiation(inst) => {
                if let Some(ref args) = inst.argument_list {
                    Self::try_resolve_in_closure_args(&args.arguments, ctx, results)
                } else {
                    false
                }
            }
            Expression::UnaryPrefix(u) => {
                Self::try_resolve_in_closure_expr(u.operand, ctx, results)
            }
            Expression::UnaryPostfix(u) => {
                Self::try_resolve_in_closure_expr(u.operand, ctx, results)
            }
            Expression::Yield(y) => match y {
                Yield::Value(yv) => {
                    if let Some(val) = &yv.value {
                        Self::try_resolve_in_closure_expr(val, ctx, results)
                    } else {
                        false
                    }
                }
                Yield::Pair(yp) => {
                    Self::try_resolve_in_closure_expr(yp.key, ctx, results)
                        || Self::try_resolve_in_closure_expr(yp.value, ctx, results)
                }
                Yield::From(yf) => Self::try_resolve_in_closure_expr(yf.iterator, ctx, results),
            },
            Expression::Throw(t) => Self::try_resolve_in_closure_expr(t.exception, ctx, results),
            Expression::Clone(c) => Self::try_resolve_in_closure_expr(c.object, ctx, results),
            Expression::Pipe(p) => {
                Self::try_resolve_in_closure_expr(p.input, ctx, results)
                    || Self::try_resolve_in_closure_expr(p.callable, ctx, results)
            }
            _ => false,
        }
    }

    /// Check call-expression arguments for closures containing the cursor.
    fn try_resolve_in_closure_call<'b>(
        call: &'b Call<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        match call {
            Call::Function(fc) => {
                Self::try_resolve_in_closure_args(&fc.argument_list.arguments, ctx, results)
            }
            Call::Method(mc) => {
                Self::try_resolve_in_closure_expr(mc.object, ctx, results)
                    || Self::try_resolve_in_closure_args(&mc.argument_list.arguments, ctx, results)
            }
            Call::NullSafeMethod(mc) => {
                Self::try_resolve_in_closure_expr(mc.object, ctx, results)
                    || Self::try_resolve_in_closure_args(&mc.argument_list.arguments, ctx, results)
            }
            Call::StaticMethod(sc) => {
                Self::try_resolve_in_closure_expr(sc.class, ctx, results)
                    || Self::try_resolve_in_closure_args(&sc.argument_list.arguments, ctx, results)
            }
        }
    }

    /// Check a list of arguments for closures containing the cursor.
    fn try_resolve_in_closure_args<'b>(
        arguments: &'b TokenSeparatedSequence<'b, Argument<'b>>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        for arg in arguments.iter() {
            let arg_expr = match arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            if Self::try_resolve_in_closure_expr(arg_expr, ctx, results) {
                return true;
            }
        }
        false
    }

    /// Resolve a variable's type from a closure / arrow-function
    /// parameter list.  If the variable matches a typed parameter,
    /// the resolved classes replace whatever is currently in `results`.
    fn resolve_closure_params(
        parameter_list: &FunctionLikeParameterList<'_>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        for param in parameter_list.parameters.iter() {
            let pname = param.variable.name.to_string();
            if pname == ctx.var_name {
                if let Some(hint) = &param.hint {
                    let type_str = Self::extract_hint_string(hint);
                    let resolved = Self::type_hint_to_classes(
                        &type_str,
                        &ctx.current_class.name,
                        ctx.all_classes,
                        ctx.class_loader,
                    );
                    if !resolved.is_empty() {
                        *results = resolved;
                    }
                }
                break;
            }
        }
    }

    /// Try to resolve a variable's type from an inline `/** @var … */`
    /// docblock that immediately precedes the assignment statement.
    ///
    /// Supports both formats:
    ///   - `/** @var TheType */`
    ///   - `/** @var TheType $var */`
    ///
    /// When a variable name is present in the annotation, it must match
    /// the variable being resolved.
    ///
    /// The same override check used for `@return` is applied: the docblock
    /// type only wins when `resolve_effective_type(native, docblock)` picks
    /// the docblock.  If the native (RHS) type is a concrete scalar and the
    /// docblock type is a class name, the override is rejected and the
    /// method returns `false` so the caller falls through to normal
    /// resolution.
    ///
    /// Returns `true` when the override was applied (results updated) and
    /// `false` when there is no applicable `@var` annotation.
    fn try_inline_var_override<'b>(
        expr: &'b Expression<'b>,
        stmt_start: usize,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) -> bool {
        // Must be an assignment to our target variable.
        let assignment = match expr {
            Expression::Assignment(a) if a.operator.is_assign() => a,
            _ => return false,
        };
        let lhs_name = match assignment.lhs {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => return false,
        };
        if lhs_name != ctx.var_name {
            return false;
        }

        // Look for a `/** @var … */` docblock right before this statement.
        let (var_type, var_name) = match docblock::find_inline_var_docblock(ctx.content, stmt_start)
        {
            Some(pair) => pair,
            None => return false,
        };

        // If the annotation includes a variable name, it must match.
        if let Some(ref vn) = var_name
            && vn != ctx.var_name
        {
            return false;
        }

        // Determine the "native" return-type string from the RHS so we can
        // apply the same override check used for `@return` annotations.
        let native_type = Self::extract_native_type_from_rhs(assignment.rhs, ctx);
        let effective = docblock::resolve_effective_type(native_type.as_deref(), Some(&var_type));

        let eff_type = match effective {
            Some(t) => t,
            None => return false,
        };

        let resolved = Self::type_hint_to_classes(
            &eff_type,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );

        if resolved.is_empty() {
            return false;
        }

        // Apply the resolved type(s) with the same conditional semantics
        // used by `check_expression_for_assignment`.
        if !conditional {
            results.clear();
        }
        for cls in resolved {
            if !results.iter().any(|c| c.name == cls.name) {
                results.push(cls);
            }
        }
        true
    }

    /// Extract the "native" return-type string from the RHS of an assignment
    /// expression, without resolving it to `ClassInfo`.
    ///
    /// This is used by [`try_inline_var_override`] to feed
    /// [`docblock::resolve_effective_type`] with the same kind of type
    /// string that `@return` override checking uses.
    ///
    /// Returns `None` when the native type cannot be determined (the
    /// caller should treat this as "unknown", which lets the docblock type
    /// win unconditionally).
    fn extract_native_type_from_rhs<'b>(
        rhs: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Option<String> {
        match rhs {
            // `new ClassName(…)` → the class name.
            Expression::Instantiation(inst) => match inst.class {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                Expression::Self_(_) => Some(ctx.current_class.name.clone()),
                Expression::Static(_) => Some(ctx.current_class.name.clone()),
                _ => None,
            },
            // Function / method calls → look up the return type.
            Expression::Call(call) => match call {
                Call::Function(func_call) => {
                    let func_name = match func_call.function {
                        Expression::Identifier(ident) => Some(ident.value().to_string()),
                        _ => None,
                    };
                    func_name.and_then(|name| {
                        ctx.function_loader
                            .and_then(|fl| fl(&name))
                            .and_then(|fi| fi.return_type)
                    })
                }
                Call::Method(method_call) => {
                    if let Expression::Variable(Variable::Direct(dv)) = method_call.object
                        && dv.name == "$this"
                        && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
                    {
                        let method_name = ident.value.to_string();
                        ctx.all_classes
                            .iter()
                            .find(|c| c.name == ctx.current_class.name)
                            .and_then(|cls| {
                                cls.methods
                                    .iter()
                                    .find(|m| m.name == method_name)
                                    .and_then(|m| m.return_type.clone())
                            })
                    } else {
                        None
                    }
                }
                Call::StaticMethod(static_call) => {
                    let class_name = match static_call.class {
                        Expression::Self_(_) | Expression::Static(_) => {
                            Some(ctx.current_class.name.clone())
                        }
                        Expression::Identifier(ident) => Some(ident.value().to_string()),
                        _ => None,
                    };
                    if let Some(cls_name) = class_name
                        && let ClassLikeMemberSelector::Identifier(ident) = &static_call.method
                    {
                        let method_name = ident.value.to_string();
                        let owner = ctx
                            .all_classes
                            .iter()
                            .find(|c| c.name == cls_name)
                            .cloned()
                            .or_else(|| (ctx.class_loader)(&cls_name));
                        owner.and_then(|o| {
                            o.methods
                                .iter()
                                .find(|m| m.name == method_name)
                                .and_then(|m| m.return_type.clone())
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// If `expr` is an assignment whose LHS matches `$var_name` and whose
    /// RHS is a `new …` instantiation or a function/method call with a
    /// known return type, resolve the class and add it to `results`.
    ///
    /// When `conditional` is `false` (unconditional assignment), previous
    /// candidates are cleared before adding the new type.  When `true`,
    /// the new type is appended (the variable *might* be this type).
    ///
    /// Duplicate class names are suppressed automatically.
    fn check_expression_for_assignment<'b>(
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        let var_name = ctx.var_name;
        let current_class_name: &str = &ctx.current_class.name;
        let all_classes = ctx.all_classes;
        let content = ctx.content;
        let class_loader = ctx.class_loader;
        let function_loader = ctx.function_loader;
        /// Push one or more resolved classes into `results`.
        ///
        /// * `conditional == false` → unconditional assignment: **clear**
        ///   previous candidates first, then add all new ones (handles
        ///   union return types like `A|B` from a single assignment).
        /// * `conditional == true` → conditional branch: **append**
        ///   without clearing (the variable *might* be these types).
        ///
        /// Duplicates (same class name) are always suppressed.
        fn push_results(
            results: &mut Vec<ClassInfo>,
            new_classes: Vec<ClassInfo>,
            conditional: bool,
        ) {
            if new_classes.is_empty() {
                return;
            }
            if !conditional {
                results.clear();
            }
            for cls in new_classes {
                if !results.iter().any(|c| c.name == cls.name) {
                    results.push(cls);
                }
            }
        }

        if let Expression::Assignment(assignment) = expr {
            if !assignment.operator.is_assign() {
                return;
            }
            // Check LHS is our variable
            let lhs_name = match assignment.lhs {
                Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
                _ => return,
            };
            if lhs_name != var_name {
                return;
            }
            // Check RHS is a `new …`
            if let Expression::Instantiation(inst) = assignment.rhs {
                let class_name = match inst.class {
                    Expression::Self_(_) => Some("self"),
                    Expression::Static(_) => Some("static"),
                    Expression::Identifier(ident) => Some(ident.value()),
                    _ => None,
                };
                if let Some(name) = class_name {
                    let resolved = Self::type_hint_to_classes(
                        name,
                        current_class_name,
                        all_classes,
                        class_loader,
                    );
                    push_results(results, resolved, conditional);
                }
                return;
            }
            // Check RHS is a function call: `$var = someFunction(…)`
            // Look up the function's return type and resolve to a class.
            if let Expression::Call(call) = assignment.rhs {
                match call {
                    Call::Function(func_call) => {
                        // Extract the function name from the call target
                        let func_name = match func_call.function {
                            Expression::Identifier(ident) => Some(ident.value().to_string()),
                            _ => None,
                        };
                        if let Some(name) = func_name
                            && let Some(fl) = function_loader
                            && let Some(func_info) = fl(&name)
                        {
                            // Try conditional return type first (PHPStan syntax)
                            let mut handled = false;
                            if let Some(ref cond) = func_info.conditional_return {
                                let resolved_type = resolve_conditional_with_args(
                                    cond,
                                    &func_info.parameters,
                                    &func_call.argument_list,
                                );
                                if let Some(ref ty) = resolved_type {
                                    let resolved = Self::type_hint_to_classes(
                                        ty,
                                        current_class_name,
                                        all_classes,
                                        class_loader,
                                    );
                                    if !resolved.is_empty() {
                                        push_results(results, resolved, conditional);
                                        handled = true;
                                    }
                                }
                            }
                            if !handled && let Some(ref ret) = func_info.return_type {
                                let resolved = Self::type_hint_to_classes(
                                    ret,
                                    current_class_name,
                                    all_classes,
                                    class_loader,
                                );
                                push_results(results, resolved, conditional);
                            }
                        }
                    }
                    Call::Method(method_call) => {
                        // `$var = $obj->method()` — resolve the object, find the
                        // method, and use its return type.
                        //
                        // We handle the common `$this->method()` case via the
                        // AST, and for everything else (chained calls like
                        // `app()->make(...)`, `$factory->create()`, etc.) we
                        // fall back to text-based resolution which already
                        // handles arbitrary nesting.
                        if let Expression::Variable(Variable::Direct(dv)) = method_call.object
                            && dv.name == "$this"
                            && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
                        {
                            let method_name = ident.value.to_string();
                            if let Some(owner) =
                                all_classes.iter().find(|c| c.name == current_class_name)
                            {
                                let resolved = Self::resolve_method_return_types(
                                    owner,
                                    &method_name,
                                    all_classes,
                                    class_loader,
                                );
                                push_results(results, resolved, conditional);
                            }
                        } else {
                            // General case: extract the RHS call expression text
                            // from the source and delegate to the text-based
                            // resolver that already handles chains like
                            // `app()->make(Foo::class)`, `$obj->method()`, etc.
                            let rhs_span = assignment.rhs.span();
                            let start = rhs_span.start.offset as usize;
                            let end = rhs_span.end.offset as usize;
                            if end <= content.len() {
                                let rhs_text = content[start..end].trim();
                                if rhs_text.ends_with(')')
                                    && let Some((call_body, args_text)) =
                                        split_call_subject(rhs_text)
                                {
                                    let current_class =
                                        all_classes.iter().find(|c| c.name == current_class_name);
                                    let call_ctx = CallResolutionCtx {
                                        current_class,
                                        all_classes,
                                        content,
                                        cursor_offset: ctx.cursor_offset,
                                        class_loader,
                                        function_loader,
                                    };
                                    let resolved = Self::resolve_call_return_types(
                                        call_body, args_text, &call_ctx,
                                    );
                                    push_results(results, resolved, conditional);
                                }
                            }
                        }
                    }
                    Call::StaticMethod(static_call) => {
                        // `$var = ClassName::method()` — resolve the class, find the
                        // method, and use its return type.
                        let class_name = match static_call.class {
                            Expression::Self_(_) => Some(current_class_name.to_string()),
                            Expression::Static(_) => Some(current_class_name.to_string()),
                            Expression::Identifier(ident) => Some(ident.value().to_string()),
                            _ => None,
                        };
                        if let Some(cls_name) = class_name
                            && let ClassLikeMemberSelector::Identifier(ident) = &static_call.method
                        {
                            let method_name = ident.value.to_string();
                            let owner = all_classes
                                .iter()
                                .find(|c| c.name == cls_name)
                                .cloned()
                                .or_else(|| class_loader(&cls_name));
                            if let Some(ref owner) = owner {
                                let resolved = Self::resolve_method_return_types(
                                    owner,
                                    &method_name,
                                    all_classes,
                                    class_loader,
                                );
                                push_results(results, resolved, conditional);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Resolve a class together with all inherited members from its parent
    /// chain.
    ///
    /// Walks up the `extends` chain via `class_loader`, collecting public and
    /// protected methods, properties, and constants from each ancestor.
    /// If a child already defines a member with the same name as a parent
    /// member, the child's version wins (even if the signatures differ).
    ///
    /// Private members are never inherited.
    ///
    /// A depth limit of 20 prevents infinite loops from circular inheritance.
    pub(crate) fn resolve_class_with_inheritance(
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> ClassInfo {
        let mut merged = class.clone();

        // 1. Merge traits used by this class.
        //    PHP precedence: class methods > trait methods > inherited methods.
        //    Since `merged` already contains the class's own members, we only
        //    add trait members that don't collide with existing ones.
        Self::merge_traits_into(&mut merged, &class.used_traits, class_loader, 0);

        // 2. Walk up the `extends` chain and merge parent members.
        let mut current = class.clone();
        let mut depth = 0;
        const MAX_DEPTH: u32 = 20;

        while let Some(ref parent_name) = current.parent_class {
            depth += 1;
            if depth > MAX_DEPTH {
                break;
            }

            let parent = if let Some(p) = class_loader(parent_name) {
                p
            } else {
                break;
            };

            // Merge traits used by the parent class as well, so that
            // grandparent-level trait members are visible.
            Self::merge_traits_into(&mut merged, &parent.used_traits, class_loader, 0);

            // Merge parent methods — skip private, skip if child already has one with same name
            for method in &parent.methods {
                if method.visibility == Visibility::Private {
                    continue;
                }
                if merged.methods.iter().any(|m| m.name == method.name) {
                    continue;
                }
                merged.methods.push(method.clone());
            }

            // Merge parent properties
            for property in &parent.properties {
                if property.visibility == Visibility::Private {
                    continue;
                }
                if merged.properties.iter().any(|p| p.name == property.name) {
                    continue;
                }
                merged.properties.push(property.clone());
            }

            // Merge parent constants
            for constant in &parent.constants {
                if constant.visibility == Visibility::Private {
                    continue;
                }
                if merged.constants.iter().any(|c| c.name == constant.name) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }

            current = parent;
        }

        // 3. Merge members from @mixin classes.
        //    Mixin members have the lowest precedence — they only fill in
        //    members that are not already provided by the class itself,
        //    its traits, or its parent chain.  This models the PHP pattern
        //    where `@mixin` documents that magic methods (__call, __get,
        //    etc.) proxy to another class.
        //
        //    Mixins are inherited: if `User extends Model` and `Model`
        //    has `@mixin Builder`, then `User` also gains Builder's
        //    members.  We merge the class's own mixins first, then walk
        //    up the parent chain again to collect ancestor mixins.
        Self::merge_mixins_into(&mut merged, &class.mixins, class_loader);

        // Also merge mixins declared on ancestor classes.
        let mut ancestor = class.clone();
        let mut mixin_depth = 0u32;
        while let Some(ref parent_name) = ancestor.parent_class {
            mixin_depth += 1;
            if mixin_depth > MAX_DEPTH {
                break;
            }
            let parent = if let Some(p) = class_loader(parent_name) {
                p
            } else {
                break;
            };
            if !parent.mixins.is_empty() {
                Self::merge_mixins_into(&mut merged, &parent.mixins, class_loader);
            }
            ancestor = parent;
        }

        merged
    }

    /// Recursively merge members from the given traits into `merged`.
    ///
    /// Traits can themselves `use` other traits (composition), so this
    /// method recurses up to `MAX_TRAIT_DEPTH` levels.  Members that
    /// already exist in `merged` (by name) are skipped — this naturally
    /// implements the PHP precedence rule where the current class's own
    /// members win over trait members, and earlier-listed traits win
    /// over later ones.
    ///
    /// Private trait members *are* merged (unlike parent class private
    /// members), because PHP copies trait members into the using class
    /// regardless of visibility.
    /// Merge public members from `@mixin` classes into `merged`.
    ///
    /// Mixins are resolved with full inheritance (the mixin class itself
    /// may extend another class, use traits, etc.), and only **public**
    /// members that don't already exist in `merged` are added.  This
    /// gives mixins the lowest precedence in the resolution chain:
    ///
    ///   class own > traits > parent chain > mixins
    ///
    /// Mixin classes can themselves declare `@mixin`, so this recurses
    /// up to a depth limit to handle mixin chains.
    fn merge_mixins_into(
        merged: &mut ClassInfo,
        mixin_names: &[String],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) {
        const MAX_MIXIN_DEPTH: u32 = 10;
        Self::merge_mixins_into_recursive(merged, mixin_names, class_loader, 0, MAX_MIXIN_DEPTH);
    }

    fn merge_mixins_into_recursive(
        merged: &mut ClassInfo,
        mixin_names: &[String],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: u32,
        max_depth: u32,
    ) {
        if depth > max_depth {
            return;
        }

        for mixin_name in mixin_names {
            let mixin_class = if let Some(c) = class_loader(mixin_name) {
                c
            } else {
                continue;
            };

            // Resolve the mixin class with its own inheritance so we see
            // all of its inherited/trait members too.
            let resolved_mixin = Self::resolve_class_with_inheritance(&mixin_class, class_loader);

            // Only merge public members — mixins proxy via magic methods
            // which only expose public API.
            for method in &resolved_mixin.methods {
                if method.visibility != Visibility::Public {
                    continue;
                }
                if merged.methods.iter().any(|m| m.name == method.name) {
                    continue;
                }
                let mut method = method.clone();
                // `@return $this` in the mixin class refers to the mixin
                // instance, NOT the consuming class.  Rewrite the return
                // type to the concrete mixin class name so that resolution
                // produces the mixin class rather than the consumer.
                if matches!(
                    method.return_type.as_deref(),
                    Some("$this" | "self" | "static")
                ) {
                    method.return_type = Some(mixin_class.name.clone());
                }
                merged.methods.push(method);
            }

            for property in &resolved_mixin.properties {
                if property.visibility != Visibility::Public {
                    continue;
                }
                if merged.properties.iter().any(|p| p.name == property.name) {
                    continue;
                }
                merged.properties.push(property.clone());
            }

            for constant in &resolved_mixin.constants {
                if constant.visibility != Visibility::Public {
                    continue;
                }
                if merged.constants.iter().any(|c| c.name == constant.name) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }

            // Recurse into mixins declared by the mixin class itself.
            if !mixin_class.mixins.is_empty() {
                Self::merge_mixins_into_recursive(
                    merged,
                    &mixin_class.mixins,
                    class_loader,
                    depth + 1,
                    max_depth,
                );
            }
        }
    }

    fn merge_traits_into(
        merged: &mut ClassInfo,
        trait_names: &[String],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: u32,
    ) {
        const MAX_TRAIT_DEPTH: u32 = 20;
        if depth > MAX_TRAIT_DEPTH {
            return;
        }

        for trait_name in trait_names {
            let trait_info = if let Some(t) = class_loader(trait_name) {
                t
            } else {
                continue;
            };

            // Recursively merge traits used by this trait (trait composition).
            if !trait_info.used_traits.is_empty() {
                Self::merge_traits_into(merged, &trait_info.used_traits, class_loader, depth + 1);
            }

            // Walk the `parent_class` (extends) chain so that interface
            // inheritance is resolved.  For example, `BackedEnum extends
            // UnitEnum` — loading `BackedEnum` alone would miss `UnitEnum`'s
            // members (`cases()`, `$name`) unless we follow the chain here.
            // The same depth counter is shared to prevent infinite loops.
            let mut current = trait_info.clone();
            let mut parent_depth = depth;
            while let Some(ref parent_name) = current.parent_class {
                parent_depth += 1;
                if parent_depth > MAX_TRAIT_DEPTH {
                    break;
                }
                let parent = if let Some(p) = class_loader(parent_name) {
                    p
                } else {
                    break;
                };

                // Also follow the parent's own used_traits.
                if !parent.used_traits.is_empty() {
                    Self::merge_traits_into(
                        merged,
                        &parent.used_traits,
                        class_loader,
                        parent_depth + 1,
                    );
                }

                // Merge parent methods (skip private, skip duplicates)
                for method in &parent.methods {
                    if method.visibility == Visibility::Private {
                        continue;
                    }
                    if merged.methods.iter().any(|m| m.name == method.name) {
                        continue;
                    }
                    merged.methods.push(method.clone());
                }

                // Merge parent properties
                for property in &parent.properties {
                    if property.visibility == Visibility::Private {
                        continue;
                    }
                    if merged.properties.iter().any(|p| p.name == property.name) {
                        continue;
                    }
                    merged.properties.push(property.clone());
                }

                // Merge parent constants
                for constant in &parent.constants {
                    if constant.visibility == Visibility::Private {
                        continue;
                    }
                    if merged.constants.iter().any(|c| c.name == constant.name) {
                        continue;
                    }
                    merged.constants.push(constant.clone());
                }

                current = parent;
            }

            // Merge trait methods — skip if already present
            for method in &trait_info.methods {
                if merged.methods.iter().any(|m| m.name == method.name) {
                    continue;
                }
                merged.methods.push(method.clone());
            }

            // Merge trait properties
            for property in &trait_info.properties {
                if merged.properties.iter().any(|p| p.name == property.name) {
                    continue;
                }
                merged.properties.push(property.clone());
            }

            // Merge trait constants
            for constant in &trait_info.constants {
                if merged.constants.iter().any(|c| c.name == constant.name) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }
        }
    }
}

// ─── PHPStan Conditional Return Type Resolution ─────────────────────────────

/// Resolve a PHPStan conditional return type given AST-level call-site
/// arguments.
///
/// Walks the conditional tree and matches argument expressions against
/// the conditions:
///   - `class-string<T>`: checks if the positional argument is `X::class`
///     and returns `"X"`.
///   - `is null`: satisfied when no argument is provided (parameter has
///     a null default).
///   - `is SomeType`: not statically resolvable from AST; falls through
///     to the else branch.
///
/// Split a call-expression subject into the call body and any textual
/// arguments.  Handles both `"app()"` → `("app", "")` and
/// `"app(A::class)"` → `("app", "A::class")`.
///
/// For method / static-method calls the arguments are currently not
/// preserved by the extractors, so they always arrive as `""`.
fn split_call_subject(subject: &str) -> Option<(&str, &str)> {
    // Subject must end with ')'.
    let inner = subject.strip_suffix(')')?;
    // Find the matching '(' for the stripped ')' by scanning backwards
    // and tracking balanced parentheses.  This correctly handles nested
    // calls inside the argument list (e.g. `Environment::get(self::country())`).
    let bytes = inner.as_bytes();
    let mut depth: u32 = 0;
    let mut open = None;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    open = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let open = open?;
    let call_body = &inner[..open];
    let args_text = inner[open + 1..].trim();
    if call_body.is_empty() {
        return None;
    }
    Some((call_body, args_text))
}

/// Resolve a conditional return type using **textual** arguments extracted
/// from the source code (e.g. `"SessionManager::class"`).
///
/// This is used when the call is made inline (not assigned to a variable)
/// and we therefore don't have an AST `ArgumentList` — only the raw text
/// between the parentheses.
fn resolve_conditional_with_text_args(
    conditional: &ConditionalReturnType,
    params: &[ParameterInfo],
    text_args: &str,
) -> Option<String> {
    match conditional {
        ConditionalReturnType::Concrete(ty) => {
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty.clone())
        }
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            // Find which parameter index corresponds to $param_name
            let target = format!("${}", param_name);
            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);

            // Split the textual arguments by comma (at depth 0) and pick
            // the one at `param_idx`.
            let args = split_text_args(text_args);
            let arg_text = args.get(param_idx).map(|s| s.trim());

            match condition {
                ParamCondition::ClassString => {
                    // Check if the argument text matches `X::class`
                    if let Some(arg) = arg_text
                        && let Some(class_name) = extract_class_name_from_text(arg)
                    {
                        return Some(class_name);
                    }
                    // Argument isn't a ::class literal → try else branch
                    resolve_conditional_with_text_args(else_type, params, text_args)
                }
                ParamCondition::IsNull => {
                    if arg_text.is_none() || arg_text == Some("") || arg_text == Some("null") {
                        // No argument provided or explicitly null → null branch
                        resolve_conditional_with_text_args(then_type, params, text_args)
                    } else {
                        // Argument was provided → not null
                        resolve_conditional_with_text_args(else_type, params, text_args)
                    }
                }
                ParamCondition::IsType(_) => {
                    // Can't statically determine; fall through to else.
                    resolve_conditional_with_text_args(else_type, params, text_args)
                }
            }
        }
    }
}

/// Split a textual argument list by commas, respecting nested parentheses
/// so that `"foo(a, b), c"` splits into `["foo(a, b)", "c"]`.
fn split_text_args(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                result.push(&text[start..i]);
                start = i + 1; // skip the comma
            }
            _ => {}
        }
    }
    // Push the last segment (or the only one if there were no commas).
    if start <= text.len() {
        let last = &text[start..];
        if !last.trim().is_empty() {
            result.push(last);
        }
    }
    result
}

/// Extract a class name from textual `X::class` syntax.
///
/// Matches strings like `"SessionManager::class"`, `"\\App\\Foo::class"`,
/// returning the class name portion (`"SessionManager"`, `"\\App\\Foo"`).
fn extract_class_name_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let name = trimmed.strip_suffix("::class")?;
    if name.is_empty() {
        return None;
    }
    // Validate that it looks like a class name (identifiers and backslashes).
    if name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '\\')
    {
        Some(name.strip_prefix('\\').unwrap_or(name).to_string())
    } else {
        None
    }
}

fn resolve_conditional_with_args<'b>(
    conditional: &ConditionalReturnType,
    params: &[ParameterInfo],
    argument_list: &ArgumentList<'b>,
) -> Option<String> {
    match conditional {
        ConditionalReturnType::Concrete(ty) => {
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty.clone())
        }
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            // Find which parameter index corresponds to $param_name
            let target = format!("${}", param_name);
            let param_idx = params.iter().position(|p| p.name == target).unwrap_or(0);

            // Get the actual argument expression (if provided)
            let arg_expr: Option<&Expression<'b>> = argument_list
                .arguments
                .iter()
                .nth(param_idx)
                .and_then(|arg| match arg {
                    Argument::Positional(pos) => Some(pos.value),
                    Argument::Named(named) => {
                        // Also match named arguments by param name
                        if named.name.value == param_name.as_str() {
                            Some(named.value)
                        } else {
                            None
                        }
                    }
                });

            match condition {
                ParamCondition::ClassString => {
                    // Check if the argument is `X::class`
                    if let Some(class_name) = arg_expr.and_then(extract_class_string_from_expr) {
                        return Some(class_name);
                    }
                    // Argument isn't a ::class literal → try else branch
                    resolve_conditional_with_args(else_type, params, argument_list)
                }
                ParamCondition::IsNull => {
                    if arg_expr.is_none() {
                        // No argument provided → param uses default (null)
                        resolve_conditional_with_args(then_type, params, argument_list)
                    } else {
                        // Argument was provided → not null
                        resolve_conditional_with_args(else_type, params, argument_list)
                    }
                }
                ParamCondition::IsType(_) => {
                    // We can't statically determine the type of an
                    // arbitrary expression; fall through to else.
                    resolve_conditional_with_args(else_type, params, argument_list)
                }
            }
        }
    }
}

/// Resolve a conditional return type **without** call-site arguments
/// (text-based path).  Walks the tree taking the "no argument / null
/// default" branch at each level.
fn resolve_conditional_without_args(
    conditional: &ConditionalReturnType,
    params: &[ParameterInfo],
) -> Option<String> {
    match conditional {
        ConditionalReturnType::Concrete(ty) => {
            if ty == "mixed" || ty == "void" || ty == "never" {
                return None;
            }
            Some(ty.clone())
        }
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            // Without arguments we check whether the parameter has a
            // null default — if so, the `is null` branch is taken.
            let target = format!("${}", param_name);
            let param = params.iter().find(|p| p.name == target);
            let has_null_default = param.is_some_and(|p| !p.is_required);

            match condition {
                ParamCondition::IsNull if has_null_default => {
                    resolve_conditional_without_args(then_type, params)
                }
                _ => {
                    // Try else branch
                    resolve_conditional_without_args(else_type, params)
                }
            }
        }
    }
}

/// Extract the class name from an `X::class` expression.
///
/// Matches `Expression::Access(Access::ClassConstant(cca))` where the
/// constant selector is the identifier `class`.
fn extract_class_string_from_expr(expr: &Expression<'_>) -> Option<String> {
    if let Expression::Access(Access::ClassConstant(cca)) = expr
        && let ClassLikeConstantSelector::Identifier(ident) = &cca.constant
        && ident.value == "class"
    {
        // Extract the class name from the LHS
        return match cca.class {
            Expression::Identifier(class_ident) => Some(class_ident.value().to_string()),
            Expression::Self_(_) => Some("self".to_string()),
            Expression::Static(_) => Some("static".to_string()),
            Expression::Parent(_) => Some("parent".to_string()),
            _ => None,
        };
    }
    None
}
