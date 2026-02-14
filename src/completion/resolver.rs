/// Type resolution for completion subjects.
///
/// This module contains the logic for resolving a completion subject (e.g.
/// `$this`, `self`, `$var`, `$this->prop`, `ClassName`) to a concrete
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
            && !subject.ends_with("()")
        {
            let lookup = subject.rsplit('\\').next().unwrap_or(subject);
            if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                return vec![cls.clone()];
            }
            // Try cross-file / PSR-4 with the full subject
            return class_loader(subject).into_iter().collect();
        }

        // ── Call expression: subject ends with "()" ──
        // Handles function calls (`app()`), method calls (`$this->getService()`),
        // and static method calls (`ClassName::make()`).
        if let Some(call_body) = subject.strip_suffix("()") {
            return Self::resolve_call_return_types(
                call_body,
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
            } else {
                // Could be a variable — for now, skip complex chains
                vec![]
            };

            let mut results = Vec::new();
            for owner in &lhs_classes {
                results.extend(Self::resolve_method_return_types(
                    owner,
                    method_name,
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
                return Self::resolve_method_return_types(
                    owner,
                    method_name,
                    all_classes,
                    class_loader,
                );
            }
            return vec![];
        }

        // ── Standalone function call: app / myHelper ──
        if let Some(fl) = function_loader
            && let Some(func_info) = fl(call_body)
            && let Some(ref ret) = func_info.return_type
        {
            return Self::type_hint_to_classes(ret, "", all_classes, class_loader);
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
        // First check the class itself
        if let Some(method) = class_info.methods.iter().find(|m| m.name == method_name) {
            if let Some(ref ret) = method.return_type {
                return Self::type_hint_to_classes(
                    ret,
                    &class_info.name,
                    all_classes,
                    class_loader,
                );
            }
            return vec![];
        }

        // Walk up the inheritance chain
        let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
        if let Some(method) = merged.methods.iter().find(|m| m.name == method_name)
            && let Some(ref ret) = method.return_type
        {
            return Self::type_hint_to_classes(ret, &class_info.name, all_classes, class_loader);
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
                            && let Some(ref ret) = func_info.return_type
                        {
                            let resolved = Self::type_hint_to_classes(
                                ret,
                                current_class_name,
                                all_classes,
                                class_loader,
                            );
                            push_results(results, resolved, conditional);
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
}
