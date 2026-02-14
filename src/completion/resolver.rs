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
        // ── Keywords that always mean "current class" ──
        if subject == "$this" || subject == "self" || subject == "static" {
            return current_class.cloned();
        }

        // ── `parent::` — resolve to the current class's parent ──
        if subject == "parent" {
            if let Some(cc) = current_class
                && let Some(ref parent_name) = cc.parent_class
            {
                // Try local lookup first
                let lookup = parent_name.rsplit('\\').next().unwrap_or(parent_name);
                if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                    return Some(cls.clone());
                }
                // Fall back to cross-file / PSR-4
                return class_loader(parent_name);
            }
            return None;
        }

        // ── Bare class name (for `::`) ──
        if access_kind == AccessKind::DoubleColon && !subject.starts_with('$') {
            let lookup = subject.rsplit('\\').next().unwrap_or(subject);
            if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
                return Some(cls.clone());
            }
            // Try cross-file / PSR-4 with the full subject
            return class_loader(subject);
        }

        // ── Call expression: subject ends with "()" ──
        // Handles function calls (`app()`), method calls (`$this->getService()`),
        // and static method calls (`ClassName::make()`).
        if let Some(call_body) = subject.strip_suffix("()") {
            return Self::resolve_call_return_type(
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
            if let Some(cc) = current_class
                && let Some(resolved) =
                    Self::resolve_property_type(prop_name, cc, all_classes, class_loader)
            {
                return Some(resolved);
            }
            return None;
        }

        // ── Variable like `$var` — resolve via assignments / parameter hints ──
        if subject.starts_with('$') {
            if let Some(cc) = current_class
                && let Some(resolved) = Self::resolve_variable_type(
                    subject,
                    cc,
                    all_classes,
                    content,
                    cursor_offset,
                    class_loader,
                    function_loader,
                )
            {
                return Some(resolved);
            }
            return None;
        }

        None
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
    fn resolve_call_return_type(
        call_body: &str,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<ClassInfo> {
        // ── Instance method call: $this->method / $var->method ──
        if let Some(pos) = call_body.rfind("->") {
            let lhs = &call_body[..pos];
            let method_name = &call_body[pos + 2..];

            // Resolve the left-hand side to a class (recursively handles
            // $this, $var, property chains, nested calls, etc.)
            let lhs_class = if lhs == "$this" || lhs == "self" || lhs == "static" {
                current_class.cloned()
            } else if let Some(prop) = lhs
                .strip_prefix("$this->")
                .or_else(|| lhs.strip_prefix("$this?->"))
            {
                current_class
                    .and_then(|cc| Self::resolve_property_type(prop, cc, all_classes, class_loader))
            } else {
                // Could be a variable — for now, skip complex chains
                None
            };

            if let Some(ref owner) = lhs_class {
                return Self::resolve_method_return_type(
                    owner,
                    method_name,
                    all_classes,
                    class_loader,
                );
            }
            return None;
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
                return Self::resolve_method_return_type(
                    owner,
                    method_name,
                    all_classes,
                    class_loader,
                );
            }
            return None;
        }

        // ── Standalone function call: app / myHelper ──
        if let Some(fl) = function_loader
            && let Some(func_info) = fl(call_body)
            && let Some(ref ret) = func_info.return_type
        {
            return Self::type_hint_to_class(ret, "", all_classes, class_loader);
        }

        None
    }

    /// Look up a method's return type in a class (including inherited methods)
    /// and resolve it to a `ClassInfo`.
    pub(crate) fn resolve_method_return_type(
        class_info: &ClassInfo,
        method_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        // First check the class itself
        if let Some(method) = class_info.methods.iter().find(|m| m.name == method_name) {
            if let Some(ref ret) = method.return_type {
                return Self::type_hint_to_class(ret, &class_info.name, all_classes, class_loader);
            }
            return None;
        }

        // Walk up the inheritance chain
        let merged = Self::resolve_class_with_inheritance(class_info, class_loader);
        if let Some(method) = merged.methods.iter().find(|m| m.name == method_name)
            && let Some(ref ret) = method.return_type
        {
            return Self::type_hint_to_class(ret, &class_info.name, all_classes, class_loader);
        }

        None
    }

    /// Look up a property's type hint in `class_info` and find the
    /// corresponding class in `all_classes` (or via `class_loader`).
    pub(crate) fn resolve_property_type(
        prop_name: &str,
        class_info: &ClassInfo,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        let prop = class_info.properties.iter().find(|p| p.name == prop_name)?;
        let type_hint = prop.type_hint.as_deref()?;
        Self::type_hint_to_class(type_hint, &class_info.name, all_classes, class_loader)
    }

    /// Map a type-hint string to a `ClassInfo`, treating `self` / `static`
    /// as the owning class.  Strips a leading `?` for nullable types.
    ///
    /// First searches `all_classes` (the current file), then falls back to
    /// the `class_loader` callback for cross-file resolution.
    pub(crate) fn type_hint_to_class(
        type_hint: &str,
        owning_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);

        // self / static always refer to the owning class
        if hint == "self" || hint == "static" {
            return all_classes
                .iter()
                .find(|c| c.name == owning_class_name)
                .cloned()
                .or_else(|| class_loader(owning_class_name));
        }

        // Try local (current-file) lookup by last segment
        let lookup = hint.rsplit('\\').next().unwrap_or(hint);
        if let Some(cls) = all_classes.iter().find(|c| c.name == lookup) {
            return Some(cls.clone());
        }

        // Fallback: cross-file / PSR-4 with the full hint string
        class_loader(hint)
    }

    /// Resolve the type of `$variable` by re-parsing the file and walking
    /// the method body that contains `cursor_offset`.
    ///
    /// Looks at:
    ///   1. Assignments: `$var = new ClassName(…)` / `new self` / `new static`
    ///   2. Assignments from function calls: `$var = app()` → look up return type
    ///   3. Method parameter type hints
    fn resolve_variable_type(
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<ClassInfo> {
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
    ) -> Option<ClassInfo> {
        for stmt in statements {
            match stmt {
                Statement::Class(class) => {
                    let start = class.left_brace.start.offset;
                    let end = class.right_brace.end.offset;
                    if cursor_offset < start || cursor_offset > end {
                        continue;
                    }
                    if let Some(cls) = Self::resolve_variable_in_members(
                        class.members.iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                    ) {
                        return Some(cls);
                    }
                }
                Statement::Interface(iface) => {
                    let start = iface.left_brace.start.offset;
                    let end = iface.right_brace.end.offset;
                    if cursor_offset < start || cursor_offset > end {
                        continue;
                    }
                    if let Some(cls) = Self::resolve_variable_in_members(
                        iface.members.iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                    ) {
                        return Some(cls);
                    }
                }
                Statement::Namespace(ns) => {
                    if let Some(cls) = Self::resolve_variable_in_statements(
                        ns.statements().iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                    ) {
                        return Some(cls);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Resolve a variable's type by scanning class-like members for parameter
    /// type hints and assignment expressions.
    ///
    /// Shared between `Statement::Class` and `Statement::Interface`.
    fn resolve_variable_in_members<'b>(
        members: impl Iterator<Item = &'b ClassLikeMember<'b>>,
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<ClassInfo> {
        for member in members {
            if let ClassLikeMember::Method(method) = member {
                // Check parameter type hints first
                for param in method.parameter_list.parameters.iter() {
                    let pname = param.variable.name.to_string();
                    if pname == var_name
                        && let Some(hint) = &param.hint
                    {
                        let type_str = Self::extract_hint_string(hint);
                        return Self::type_hint_to_class(
                            &type_str,
                            &current_class.name,
                            all_classes,
                            class_loader,
                        );
                    }
                }
                if let MethodBody::Concrete(block) = &method.body {
                    let blk_start = block.left_brace.start.offset;
                    let blk_end = block.right_brace.end.offset;
                    if cursor_offset >= blk_start
                        && cursor_offset <= blk_end
                        && let Some(cls) = Self::find_assignment_type_in_block(
                            block,
                            var_name,
                            &current_class.name,
                            all_classes,
                            cursor_offset,
                            class_loader,
                            function_loader,
                        )
                    {
                        return Some(cls);
                    }
                }
            }
        }
        None
    }

    /// Walk a block's statements looking for the *last* assignment to
    /// `$var_name` that occurs *before* the cursor.
    fn find_assignment_type_in_block<'b>(
        block: &'b Block<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Option<ClassInfo> {
        let mut result: Option<ClassInfo> = None;
        Self::walk_statements_for_assignments(
            block.statements.iter(),
            var_name,
            current_class_name,
            all_classes,
            cursor_offset,
            class_loader,
            function_loader,
            &mut result,
        );
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_statements_for_assignments<'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
        result: &mut Option<ClassInfo>,
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
                        result,
                    );
                }
                // Recurse into blocks, if/else, loops, try, etc.
                Statement::Block(block) => {
                    Self::walk_statements_for_assignments(
                        block.statements.iter(),
                        var_name,
                        current_class_name,
                        all_classes,
                        cursor_offset,
                        class_loader,
                        function_loader,
                        result,
                    );
                }
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
                            result,
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
                                result,
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
                                result,
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
                            result,
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
                                result,
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
                                result,
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
                            result,
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
                            result,
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
                            result,
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
                            result,
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
                            result,
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
                            result,
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
                        result,
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
                        result,
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
                            result,
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
                            result,
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
        result: &mut Option<ClassInfo>,
    ) {
        Self::walk_statements_for_assignments(
            std::iter::once(stmt),
            var_name,
            current_class_name,
            all_classes,
            cursor_offset,
            class_loader,
            function_loader,
            result,
        );
    }

    /// If `expr` is an assignment whose LHS matches `$var_name` and whose
    /// RHS is a `new …` instantiation or a function/method call with a
    /// known return type, resolve the class.
    fn check_expression_for_assignment<'b>(
        expr: &'b Expression<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
        result: &mut Option<ClassInfo>,
    ) {
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
                if let Some(name) = class_name
                    && let Some(cls) = Self::type_hint_to_class(
                        name,
                        current_class_name,
                        all_classes,
                        class_loader,
                    )
                {
                    *result = Some(cls);
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
                            && let Some(cls) = Self::type_hint_to_class(
                                ret,
                                current_class_name,
                                all_classes,
                                class_loader,
                            )
                        {
                            *result = Some(cls);
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
                                && let Some(cls) = Self::resolve_method_return_type(
                                    owner,
                                    &method_name,
                                    all_classes,
                                    class_loader,
                                )
                            {
                                *result = Some(cls);
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
                            if let Some(ref owner) = owner
                                && let Some(cls) = Self::resolve_method_return_type(
                                    owner,
                                    &method_name,
                                    all_classes,
                                    class_loader,
                                )
                            {
                                *result = Some(cls);
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
