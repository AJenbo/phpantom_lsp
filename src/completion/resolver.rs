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
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

use crate::Backend;
use crate::types::Visibility;
use crate::types::*;

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

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
            return Self::resolve_call_return_types(
                call_body,
                args_text,
                current_class,
                all_classes,
                class_loader,
                function_loader,
            );
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
            if let Some(cc) = current_class {
                return Self::resolve_variable_types(
                    subject,
                    cc,
                    all_classes,
                    content,
                    cursor_offset,
                    class_loader,
                    function_loader,
                );
            }
            return vec![];
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
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        // ── Instance method call: $this->method / $var->method ──
        if let Some(pos) = call_body.rfind("->") {
            let lhs = &call_body[..pos];
            let method_name = &call_body[pos + 2..];

            // Resolve the left-hand side to a class (recursively handles
            // $this, $var, property chains, nested calls, etc.)
            let lhs_classes: Vec<ClassInfo> = if lhs == "$this" || lhs == "self" || lhs == "static"
            {
                current_class.cloned().into_iter().collect()
            } else if let Some(prop) = lhs
                .strip_prefix("$this->")
                .or_else(|| lhs.strip_prefix("$this?->"))
            {
                current_class
                    .map(|cc| Self::resolve_property_types(prop, cc, all_classes, class_loader))
                    .unwrap_or_default()
            } else if lhs.ends_with(')') {
                // LHS is itself a call expression (e.g. `app()` in
                // `app()->make(…)`).  Recursively resolve it.
                if let Some((inner_body, inner_args)) = split_call_subject(lhs) {
                    Self::resolve_call_return_types(
                        inner_body,
                        inner_args,
                        current_class,
                        all_classes,
                        class_loader,
                        function_loader,
                    )
                } else {
                    vec![]
                }
            } else {
                // Could be a variable — for now, skip complex chains
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

        // ── Union type: split on `|` and resolve each part ──
        if hint.contains('|') {
            let mut results = Vec::new();
            for part in hint.split('|') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                // Recursively resolve each part (handles self/static, scalars, etc.)
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

        // self / static always refer to the owning class
        if hint == "self" || hint == "static" {
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

        // Walk top-level (and namespace-nested) statements to find the
        // class + method containing the cursor.
        Self::resolve_variable_in_statements(
            program.statements.iter(),
            var_name,
            current_class,
            all_classes,
            cursor_offset,
            class_loader,
            function_loader,
        )
    }

    fn resolve_variable_in_statements<'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        for stmt in statements {
            match stmt {
                Statement::Class(class) => {
                    let start = class.left_brace.start.offset;
                    let end = class.right_brace.end.offset;
                    if cursor_offset < start || cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(
                        class.members.iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                    );
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Interface(iface) => {
                    let start = iface.left_brace.start.offset;
                    let end = iface.right_brace.end.offset;
                    if cursor_offset < start || cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(
                        iface.members.iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                    );
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Namespace(ns) => {
                    let results = Self::resolve_variable_in_statements(
                        ns.statements().iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                    );
                    if !results.is_empty() {
                        return results;
                    }
                }
                _ => {}
            }
        }
        vec![]
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
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        for member in members {
            if let ClassLikeMember::Method(method) = member {
                // Check parameter type hints first
                for param in method.parameter_list.parameters.iter() {
                    let pname = param.variable.name.to_string();
                    if pname == var_name
                        && let Some(hint) = &param.hint
                    {
                        let type_str = Self::extract_hint_string(hint);
                        let resolved = Self::type_hint_to_classes(
                            &type_str,
                            &current_class.name,
                            all_classes,
                            class_loader,
                        );
                        if !resolved.is_empty() {
                            return resolved;
                        }
                    }
                }
                if let MethodBody::Concrete(block) = &method.body {
                    let blk_start = block.left_brace.start.offset;
                    let blk_end = block.right_brace.end.offset;
                    if cursor_offset >= blk_start && cursor_offset <= blk_end {
                        let results = Self::find_assignment_types_in_block(
                            block,
                            var_name,
                            &current_class.name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                        );
                        if !results.is_empty() {
                            return results;
                        }
                    }
                }
            }
        }
        vec![]
    }

    /// Walk a block's statements looking for assignments to `$var_name`
    /// that occur *before* the cursor.
    ///
    /// Returns all possible types: unconditional assignments replace
    /// previous candidates, while conditional assignments (inside
    /// if/else/try/catch/loops) add to the list.
    fn find_assignment_types_in_block<'b>(
        block: &'b Block<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        let mut results: Vec<ClassInfo> = Vec::new();
        Self::walk_statements_for_assignments(
            block.statements.iter(),
            var_name,
            current_class_name,
            all_classes,
            cursor_offset,
            class_loader,
            function_loader,
            &mut results,
            false,
        );
        results
    }

    /// Walk statements collecting variable assignment types.
    ///
    /// The `conditional` flag indicates whether we are inside a conditional
    /// block (if/else, try/catch, loop).  When `conditional` is `false`,
    /// a new assignment **replaces** all previous candidates (the variable
    /// is being unconditionally reassigned).  When `conditional` is `true`,
    /// a new assignment **adds** to the list (the variable *might* be this
    /// type).
    #[allow(clippy::too_many_arguments)]
    fn walk_statements_for_assignments<'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        for stmt in statements {
            // Only consider statements whose start is before the cursor
            if stmt.span().start.offset >= cursor_offset {
                continue;
            }

            match stmt {
                Statement::Expression(expr_stmt) => {
                    Self::check_expression_for_assignment(
                        expr_stmt.expression,
                        var_name,
                        current_class_name,
                        all_classes,
                        class_loader,
                        function_loader,
                        results,
                        conditional,
                    );
                }
                // Recurse into blocks — these are just `{ … }` groupings,
                // not conditional, so preserve the current `conditional` flag.
                Statement::Block(block) => {
                    Self::walk_statements_for_assignments(
                        block.statements.iter(),
                        var_name,
                        current_class_name,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                        results,
                        conditional,
                    );
                }
                // ── Conditional blocks: recurse with conditional = true ──
                Statement::If(if_stmt) => match &if_stmt.body {
                    IfBody::Statement(body) => {
                        Self::check_statement_for_assignments(
                            body.statement,
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                        for else_if in body.else_if_clauses.iter() {
                            Self::check_statement_for_assignments(
                                else_if.statement,
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
                                class_loader,
                                function_loader,
                                results,
                                true,
                            );
                        }
                        if let Some(else_clause) = &body.else_clause {
                            Self::check_statement_for_assignments(
                                else_clause.statement,
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
                                class_loader,
                                function_loader,
                                results,
                                true,
                            );
                        }
                    }
                    IfBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                        for else_if in body.else_if_clauses.iter() {
                            Self::walk_statements_for_assignments(
                                else_if.statements.iter(),
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
                                class_loader,
                                function_loader,
                                results,
                                true,
                            );
                        }
                        if let Some(else_clause) = &body.else_clause {
                            Self::walk_statements_for_assignments(
                                else_clause.statements.iter(),
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
                                class_loader,
                                function_loader,
                                results,
                                true,
                            );
                        }
                    }
                },
                Statement::Foreach(foreach) => match &foreach.body {
                    ForeachBody::Statement(inner) => {
                        Self::check_statement_for_assignments(
                            inner,
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                    ForeachBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                },
                Statement::While(while_stmt) => match &while_stmt.body {
                    WhileBody::Statement(inner) => {
                        Self::check_statement_for_assignments(
                            inner,
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                    WhileBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                },
                Statement::For(for_stmt) => match &for_stmt.body {
                    ForBody::Statement(inner) => {
                        Self::check_statement_for_assignments(
                            inner,
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                    ForBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                },
                Statement::DoWhile(dw) => {
                    Self::check_statement_for_assignments(
                        dw.statement,
                        var_name,
                        current_class_name,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                        results,
                        true,
                    );
                }
                Statement::Try(try_stmt) => {
                    Self::walk_statements_for_assignments(
                        try_stmt.block.statements.iter(),
                        var_name,
                        current_class_name,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                        results,
                        true,
                    );
                    for catch in try_stmt.catch_clauses.iter() {
                        Self::walk_statements_for_assignments(
                            catch.block.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                    if let Some(finally) = &try_stmt.finally_clause {
                        Self::walk_statements_for_assignments(
                            finally.block.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                            results,
                            true,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Helper: treat a single statement as an iterator of one and recurse.
    #[allow(clippy::too_many_arguments)]
    fn check_statement_for_assignments<'b>(
        stmt: &'b Statement<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        Self::walk_statements_for_assignments(
            std::iter::once(stmt),
            var_name,
            current_class_name,
            all_classes,
            cursor_offset,
            class_loader,
            function_loader,
            results,
            conditional,
        );
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
    #[allow(clippy::too_many_arguments)]
    fn check_expression_for_assignment<'b>(
        expr: &'b Expression<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
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
                        // For now, handle the common case of `$this->method()`.
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
    // Find the matching '(' — for simple subjects (no nested parens in
    // the call body) this is the first '(' that belongs to the call.
    // The call body part (before the open-paren) never contains '('
    // (it's things like `app`, `$this->method`, `ClassName::make`),
    // so a simple `rfind` is correct.
    let open = inner.rfind('(')?;
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
