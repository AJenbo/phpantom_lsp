use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use bumpalo::Bump;
use mago_span::HasSpan;
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

/// Stores extracted parameter information from a parsed PHP method.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// The parameter name including the `$` prefix (e.g. "$text").
    pub name: String,
    /// Whether this parameter is required (no default value and not variadic).
    pub is_required: bool,
    /// Optional type hint string (e.g. "string", "int", "?Foo").
    pub type_hint: Option<String>,
    /// Whether this parameter is variadic (has `...`).
    pub is_variadic: bool,
    /// Whether this parameter is passed by reference (has `&`).
    pub is_reference: bool,
}

/// Stores extracted method information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// The method name (e.g. "updateText").
    pub name: String,
    /// The parameters of the method.
    pub parameters: Vec<ParameterInfo>,
    /// Optional return type hint string (e.g. "void", "string", "?int").
    pub return_type: Option<String>,
    /// Whether the method is static.
    pub is_static: bool,
}

/// Stores extracted property information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    /// The property name WITHOUT the `$` prefix (e.g. "name", "age").
    /// This matches PHP access syntax: `$this->name` not `$this->$name`.
    pub name: String,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
    /// Whether the property is static.
    pub is_static: bool,
}

/// Stores extracted constant information from a parsed PHP class.
#[derive(Debug, Clone)]
pub struct ConstantInfo {
    /// The constant name (e.g. "MAX_SIZE", "STATUS_ACTIVE").
    pub name: String,
    /// Optional type hint string (e.g. "string", "int").
    pub type_hint: Option<String>,
}

/// Describes the access operator that triggered completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// Completion triggered after `->` (instance access).
    Arrow,
    /// Completion triggered after `::` (static access).
    DoubleColon,
    /// No specific access operator detected (e.g. inside class body).
    Other,
}

/// The result of analysing what is to the left of `->` or `::`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionTarget {
    /// Whether `->` or `::` was used.
    pub access_kind: AccessKind,
    /// The textual subject before the operator, e.g. `"$this"`, `"self"`,
    /// `"$var"`, `"$this->prop"`, `"ClassName"`.
    pub subject: String,
}

/// Stores extracted class information from a parsed PHP file.
/// All data is owned so we don't depend on the parser's arena lifetime.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// The name of the class (e.g. "User").
    pub name: String,
    /// The methods defined directly in this class.
    pub methods: Vec<MethodInfo>,
    /// The properties defined directly in this class.
    pub properties: Vec<PropertyInfo>,
    /// The constants defined directly in this class.
    pub constants: Vec<ConstantInfo>,
    /// Byte offset where the class body starts (left brace).
    pub start_offset: u32,
    /// Byte offset where the class body ends (right brace).
    pub end_offset: u32,
}

pub struct Backend {
    name: String,
    version: String,
    open_files: Arc<Mutex<HashMap<String, String>>>,
    /// Maps a file URI to a list of ClassInfo extracted from that file.
    ast_map: Arc<Mutex<HashMap<String, Vec<ClassInfo>>>>,
    client: Option<Client>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: Some(client),
        }
    }

    pub fn new_test() -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: None,
        }
    }

    /// Extract a string representation of a type hint from the AST.
    fn extract_hint_string(hint: &Hint) -> String {
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
    fn extract_parameters(parameter_list: &FunctionLikeParameterList) -> Vec<ParameterInfo> {
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
    fn extract_property_info(property: &Property) -> Vec<PropertyInfo> {
        let is_static = property.modifiers().iter().any(|m| m.is_static());

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

        let mut classes = Vec::new();
        Self::extract_classes_from_statements(program.statements.iter(), &mut classes);
        classes
    }

    /// Recursively walk statements and extract class information.
    /// This handles classes at the top level as well as classes nested
    /// inside namespace declarations.
    fn extract_classes_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        classes: &mut Vec<ClassInfo>,
    ) {
        for statement in statements {
            match statement {
                Statement::Class(class) => {
                    let class_name = class.name.value.to_string();

                    let mut methods = Vec::new();
                    let mut properties = Vec::new();
                    let mut constants = Vec::new();

                    for member in class.members.iter() {
                        match member {
                            ClassLikeMember::Method(method) => {
                                let name = method.name.value.to_string();
                                let parameters = Self::extract_parameters(&method.parameter_list);
                                let return_type = method
                                    .return_type_hint
                                    .as_ref()
                                    .map(|rth| Self::extract_hint_string(&rth.hint));
                                let is_static = method.modifiers.iter().any(|m| m.is_static());

                                methods.push(MethodInfo {
                                    name,
                                    parameters,
                                    return_type,
                                    is_static,
                                });
                            }
                            ClassLikeMember::Property(property) => {
                                let mut prop_infos = Self::extract_property_info(property);
                                properties.append(&mut prop_infos);
                            }
                            ClassLikeMember::Constant(constant) => {
                                let type_hint =
                                    constant.hint.as_ref().map(|h| Self::extract_hint_string(h));
                                for item in constant.items.iter() {
                                    constants.push(ConstantInfo {
                                        name: item.name.value.to_string(),
                                        type_hint: type_hint.clone(),
                                    });
                                }
                            }
                            _ => {}
                        }
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
                    });
                }
                Statement::Namespace(namespace) => {
                    Self::extract_classes_from_statements(namespace.statements().iter(), classes);
                }
                _ => {}
            }
        }
    }

    /// Update the ast_map for a given file URI by parsing its content.
    fn update_ast(&self, uri: &str, content: &str) {
        let classes = self.parse_php(content);
        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri.to_string(), classes);
        }
    }

    /// Convert an LSP Position (line, character) to a byte offset in content.
    fn position_to_offset(content: &str, position: Position) -> Option<u32> {
        let mut offset: u32 = 0;
        for (i, line) in content.lines().enumerate() {
            if i == position.line as usize {
                let char_offset = position.character as usize;
                // Convert character offset (UTF-16 code units in LSP) to byte offset.
                // For simplicity, treat characters as single-byte (ASCII).
                // This is sufficient for most PHP code.
                let byte_col = line
                    .char_indices()
                    .nth(char_offset)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
                return Some(offset + byte_col as u32);
            }
            // +1 for the newline character
            offset += line.len() as u32 + 1;
        }
        // If the position is past the last line, return end of content
        Some(content.len() as u32)
    }

    /// Find which class the cursor (byte offset) is inside.
    fn find_class_at_offset(classes: &[ClassInfo], offset: u32) -> Option<&ClassInfo> {
        classes
            .iter()
            .find(|c| offset >= c.start_offset && offset <= c.end_offset)
    }

    // ─── Completion-target extraction ───────────────────────────────────

    /// Detect the access operator before the cursor position and extract
    /// both the `AccessKind` and the textual subject to its left.
    ///
    /// Returns `None` when no `->` or `::` is found (i.e. `AccessKind::Other`).
    pub fn extract_completion_target(content: &str, position: Position) -> Option<CompletionTarget> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards past any partial identifier the user may have typed
        let mut i = col;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        // Detect operator
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let subject = Self::extract_arrow_subject(&chars, i - 2);
            Some(CompletionTarget {
                access_kind: AccessKind::Arrow,
                subject,
            })
        } else if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            let subject = Self::extract_double_colon_subject(&chars, i - 2);
            Some(CompletionTarget {
                access_kind: AccessKind::DoubleColon,
                subject,
            })
        } else {
            None
        }
    }

    /// Kept for backward-compat with existing tests that call it directly.
    pub fn detect_access_kind(content: &str, position: Position) -> AccessKind {
        match Self::extract_completion_target(content, position) {
            Some(ct) => ct.access_kind,
            None => AccessKind::Other,
        }
    }

    /// Extract the subject expression before `->` (the arrow sits at
    /// `chars[arrow_pos]` = `-`, `chars[arrow_pos+1]` = `>`).
    ///
    /// Handles:
    ///   `$this->`, `$var->`, `$this->prop->` (one level of chaining).
    fn extract_arrow_subject(chars: &[char], arrow_pos: usize) -> String {
        // Position just before the `->`
        let end = arrow_pos;

        // Skip whitespace
        let mut i = end;
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }

        // Try to read an identifier (property name if chained)
        let ident_end = i;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }
        let ident_start = i;

        // Check whether this identifier is preceded by another `->` (chained access)
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            // We have something like  `expr->ident->` — extract inner subject
            let inner_arrow = i - 2;
            let inner_subject = Self::extract_simple_variable(chars, inner_arrow);
            if !inner_subject.is_empty() {
                let prop: String = chars[ident_start..ident_end].iter().collect();
                return format!("{}->{}", inner_subject, prop);
            }
        }

        // Check if preceded by `?->` (null-safe)
        if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_arrow = i - 3;
            let inner_subject = Self::extract_simple_variable(chars, inner_arrow);
            if !inner_subject.is_empty() {
                let prop: String = chars[ident_start..ident_end].iter().collect();
                return format!("{}?->{}", inner_subject, prop);
            }
        }

        // Otherwise treat the whole thing as a simple variable like `$this` or `$var`
        Self::extract_simple_variable(chars, end)
    }

    /// Extract a simple `$variable` ending at position `end` (exclusive).
    fn extract_simple_variable(chars: &[char], end: usize) -> String {
        let mut i = end;
        // skip whitespace
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        let var_end = i;
        // walk back through identifier chars
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }
        // expect `$` prefix
        if i > 0 && chars[i - 1] == '$' {
            i -= 1;
            chars[i..var_end].iter().collect()
        } else {
            // no `$` — return whatever we collected (may be empty)
            chars[i..var_end].iter().collect()
        }
    }

    /// Extract the identifier/keyword before `::`.
    /// Handles `self::`, `static::`, `parent::`, `ClassName::`, `Foo\Bar::`.
    fn extract_double_colon_subject(chars: &[char], colon_pos: usize) -> String {
        let mut i = colon_pos;
        // skip whitespace
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        let end = i;
        // walk back through identifier chars (including `\` for namespaces)
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\') {
            i -= 1;
        }
        // Also accept `$` prefix for `$var::` edge case (variable class name)
        if i > 0 && chars[i - 1] == '$' {
            i -= 1;
        }
        chars[i..end].iter().collect()
    }

    // ─── Subject-to-class resolution ────────────────────────────────────

    /// Determine which class (if any) the completion subject refers to.
    ///
    /// `current_class` is the class the cursor is inside (if any).
    /// `all_classes` is every class we know about in this file.
    /// `content` + `cursor_offset` are used for variable-type resolution.
    ///
    /// Returns the `ClassInfo` to use for completion, or `None` if we
    /// cannot determine the type.
    pub fn resolve_target_class<'a>(
        subject: &str,
        access_kind: AccessKind,
        current_class: Option<&'a ClassInfo>,
        all_classes: &'a [ClassInfo],
        content: &str,
        cursor_offset: u32,
    ) -> Option<&'a ClassInfo> {
        // ── Keywords that always mean "current class" ──
        if subject == "$this" || subject == "self" || subject == "static" {
            return current_class;
        }

        // ── Bare class name (for `::`) ──
        if access_kind == AccessKind::DoubleColon && !subject.starts_with('$') {
            let lookup = subject.rsplit('\\').next().unwrap_or(subject);
            return all_classes.iter().find(|c| c.name == lookup);
        }

        // ── Property-chain: $this->prop  or  $this?->prop ──
        if let Some(prop_name) = subject
            .strip_prefix("$this->")
            .or_else(|| subject.strip_prefix("$this?->"))
        {
            if let Some(cc) = current_class {
                if let Some(resolved) = Self::resolve_property_type(prop_name, cc, all_classes) {
                    return Some(resolved);
                }
            }
            return None;
        }

        // ── Variable like `$var` — resolve via assignments / parameter hints ──
        if subject.starts_with('$') {
            if let Some(cc) = current_class {
                if let Some(resolved) =
                    Self::resolve_variable_type(subject, cc, all_classes, content, cursor_offset)
                {
                    return Some(resolved);
                }
            }
            return None;
        }

        None
    }

    /// Look up a property's type hint in `class_info` and find the
    /// corresponding class in `all_classes`.
    fn resolve_property_type<'a>(
        prop_name: &str,
        class_info: &ClassInfo,
        all_classes: &'a [ClassInfo],
    ) -> Option<&'a ClassInfo> {
        let prop = class_info.properties.iter().find(|p| p.name == prop_name)?;
        let type_hint = prop.type_hint.as_deref()?;
        Self::type_hint_to_class(type_hint, &class_info.name, all_classes)
    }

    /// Map a type-hint string to a `ClassInfo`, treating `self` / `static`
    /// as the owning class.  Strips a leading `?` for nullable types.
    fn type_hint_to_class<'a>(
        type_hint: &str,
        owning_class_name: &str,
        all_classes: &'a [ClassInfo],
    ) -> Option<&'a ClassInfo> {
        let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);
        if hint == "self" || hint == "static" {
            return all_classes.iter().find(|c| c.name == owning_class_name);
        }
        let lookup = hint.rsplit('\\').next().unwrap_or(hint);
        all_classes.iter().find(|c| c.name == lookup)
    }

    /// Resolve the type of `$variable` by re-parsing the file and walking
    /// the method body that contains `cursor_offset`.
    ///
    /// Looks at:
    ///   1. Assignments: `$var = new ClassName(…)` / `new self` / `new static`
    ///   2. Method parameter type hints
    fn resolve_variable_type<'a>(
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &'a [ClassInfo],
        content: &str,
        cursor_offset: u32,
    ) -> Option<&'a ClassInfo> {
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
        )
    }

    fn resolve_variable_in_statements<'a, 'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &'a [ClassInfo],
        cursor_offset: u32,
    ) -> Option<&'a ClassInfo> {
        for stmt in statements {
            match stmt {
                Statement::Class(class) => {
                    let start = class.left_brace.start.offset;
                    let end = class.right_brace.end.offset;
                    if cursor_offset < start || cursor_offset > end {
                        continue;
                    }
                    for member in class.members.iter() {
                        if let ClassLikeMember::Method(method) = member {
                            // Check parameter type hints first
                            for param in method.parameter_list.parameters.iter() {
                                let pname = param.variable.name.to_string();
                                if pname == var_name {
                                    if let Some(hint) = &param.hint {
                                        let type_str = Self::extract_hint_string(hint);
                                        return Self::type_hint_to_class(
                                            &type_str,
                                            &current_class.name,
                                            all_classes,
                                        );
                                    }
                                }
                            }
                            if let MethodBody::Concrete(block) = &method.body {
                                let blk_start = block.left_brace.start.offset;
                                let blk_end = block.right_brace.end.offset;
                                if cursor_offset >= blk_start && cursor_offset <= blk_end {
                                    if let Some(cls) = Self::find_assignment_type_in_block(
                                        block,
                                        var_name,
                                        &current_class.name,
                                        all_classes,
                                        cursor_offset,
                                    ) {
                                        return Some(cls);
                                    }
                                }
                            }
                        }
                    }
                }
                Statement::Namespace(ns) => {
                    if let Some(cls) = Self::resolve_variable_in_statements(
                        ns.statements().iter(),
                        var_name,
                        current_class,
                        all_classes,
                        cursor_offset,
                    ) {
                        return Some(cls);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Walk a block's statements looking for the *last* assignment to
    /// `$var_name` that occurs *before* the cursor.
    fn find_assignment_type_in_block<'a, 'b>(
        block: &'b Block<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &'a [ClassInfo],
        cursor_offset: u32,
    ) -> Option<&'a ClassInfo> {
        let mut result: Option<&'a ClassInfo> = None;
        Self::walk_statements_for_assignments(
            block.statements.iter(),
            var_name,
            current_class_name,
            all_classes,
            cursor_offset,
            &mut result,
        );
        result
    }

    fn walk_statements_for_assignments<'a, 'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &'a [ClassInfo],
        cursor_offset: u32,
        result: &mut Option<&'a ClassInfo>,
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
                        result,
                    );
                }
                Statement::If(if_stmt) => {
                    match &if_stmt.body {
                        IfBody::Statement(body) => {
                            Self::check_statement_for_assignments(
                                &body.statement,
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
                                result,
                            );
                            for else_if in body.else_if_clauses.iter() {
                                Self::check_statement_for_assignments(
                                    &else_if.statement,
                                    var_name,
                                    current_class_name,
                                    all_classes,
                                    cursor_offset,
                                    result,
                                );
                            }
                            if let Some(else_clause) = &body.else_clause {
                                Self::check_statement_for_assignments(
                                    &else_clause.statement,
                                    var_name,
                                    current_class_name,
                                    all_classes,
                                    cursor_offset,
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
                                result,
                            );
                            for else_if in body.else_if_clauses.iter() {
                                Self::walk_statements_for_assignments(
                                    else_if.statements.iter(),
                                    var_name,
                                    current_class_name,
                                    all_classes,
                                    cursor_offset,
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
                                    result,
                                );
                            }
                        }
                    }
                }
                Statement::Foreach(foreach) => {
                    match &foreach.body {
                        ForeachBody::Statement(inner) => {
                            Self::check_statement_for_assignments(
                                inner,
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
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
                                result,
                            );
                        }
                    }
                }
                Statement::While(while_stmt) => {
                    match &while_stmt.body {
                        WhileBody::Statement(inner) => {
                            Self::check_statement_for_assignments(
                                inner,
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
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
                                result,
                            );
                        }
                    }
                }
                Statement::For(for_stmt) => {
                    match &for_stmt.body {
                        ForBody::Statement(inner) => {
                            Self::check_statement_for_assignments(
                                inner,
                                var_name,
                                current_class_name,
                                all_classes,
                                cursor_offset,
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
                                result,
                            );
                        }
                    }
                }
                Statement::DoWhile(dw) => {
                    Self::check_statement_for_assignments(
                        dw.statement,
                        var_name,
                        current_class_name,
                        all_classes,
                        cursor_offset,
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
                        result,
                    );
                    for catch in try_stmt.catch_clauses.iter() {
                        Self::walk_statements_for_assignments(
                            catch.block.statements.iter(),
                            var_name,
                            current_class_name,
                            all_classes,
                            cursor_offset,
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
                            result,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Helper: treat a single statement as an iterator of one and recurse.
    fn check_statement_for_assignments<'a, 'b>(
        stmt: &'b Statement<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &'a [ClassInfo],
        cursor_offset: u32,
        result: &mut Option<&'a ClassInfo>,
    ) {
        Self::walk_statements_for_assignments(
            std::iter::once(stmt),
            var_name,
            current_class_name,
            all_classes,
            cursor_offset,
            result,
        );
    }

    /// If `expr` is an assignment whose LHS matches `$var_name` and whose
    /// RHS is a `new …` instantiation, resolve the class.
    fn check_expression_for_assignment<'a, 'b>(
        expr: &'b Expression<'b>,
        var_name: &str,
        current_class_name: &str,
        all_classes: &'a [ClassInfo],
        result: &mut Option<&'a ClassInfo>,
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
                if let Some(name) = class_name {
                    if let Some(cls) = Self::type_hint_to_class(name, current_class_name, all_classes) {
                        *result = Some(cls);
                    }
                }
            }
        }
    }

    // ─── Label / display helpers ────────────────────────────────────────

    /// Build the label showing the full method signature.
    ///
    /// Example: `regularCode(string $text, $frogs = false): string`
    fn build_method_label(method: &MethodInfo) -> String {
        let params: Vec<String> = method
            .parameters
            .iter()
            .map(|p| {
                let mut parts = Vec::new();
                if let Some(ref th) = p.type_hint {
                    parts.push(th.clone());
                }
                if p.is_reference {
                    parts.push(format!("&{}", p.name));
                } else if p.is_variadic {
                    parts.push(format!("...{}", p.name));
                } else {
                    parts.push(p.name.clone());
                }
                let param_str = parts.join(" ");
                if !p.is_required && !p.is_variadic {
                    format!("{} = ...", param_str)
                } else {
                    param_str
                }
            })
            .collect();

        let ret = method
            .return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        format!("{}({}){}", method.name, params.join(", "), ret)
    }

    /// Public helper for tests: get the ast_map for a given URI.
    pub fn get_classes_for_uri(&self, uri: &str) -> Option<Vec<ClassInfo>> {
        if let Ok(map) = self.ast_map.lock() {
            map.get(uri).cloned()
        } else {
            None
        }
    }

    async fn log(&self, typ: MessageType, message: String) {
        if let Some(client) = &self.client {
            client.log_message(typ, message).await;
        }
    }

    // ─── Completion item builders ───────────────────────────────────────

    fn build_completion_items(
        target_class: &ClassInfo,
        access_kind: AccessKind,
    ) -> Vec<CompletionItem> {
        let mut items: Vec<CompletionItem> = Vec::new();

        // Methods — filtered by static / instance
        for method in &target_class.methods {
            let include = match access_kind {
                AccessKind::Arrow => !method.is_static,
                AccessKind::DoubleColon => method.is_static,
                AccessKind::Other => true,
            };
            if !include {
                continue;
            }

            let label = Self::build_method_label(method);
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(format!("Class: {}", target_class.name)),
                insert_text: Some(method.name.clone()),
                filter_text: Some(method.name.clone()),
                ..CompletionItem::default()
            });
        }

        // Properties — filtered by static / instance
        for property in &target_class.properties {
            let include = match access_kind {
                AccessKind::Arrow => !property.is_static,
                AccessKind::DoubleColon => property.is_static,
                AccessKind::Other => true,
            };
            if !include {
                continue;
            }

            let detail = if let Some(ref th) = property.type_hint {
                format!("Class: {} — {}", target_class.name, th)
            } else {
                format!("Class: {}", target_class.name)
            };

            items.push(CompletionItem {
                label: property.name.clone(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some(detail),
                insert_text: Some(property.name.clone()),
                ..CompletionItem::default()
            });
        }

        // Constants — only for `::` or unqualified access
        if access_kind == AccessKind::DoubleColon || access_kind == AccessKind::Other {
            for constant in &target_class.constants {
                let detail = if let Some(ref th) = constant.type_hint {
                    format!("Class: {} — {}", target_class.name, th)
                } else {
                    format!("Class: {}", target_class.name)
                };

                items.push(CompletionItem {
                    label: constant.name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some(detail),
                    insert_text: Some(constant.name.clone()),
                    filter_text: Some(constant.name.clone()),
                    ..CompletionItem::default()
                });
            }
        }

        items
    }
}

// ─── LSP trait impl ─────────────────────────────────────────────────────────

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "$".to_string(),
                        ">".to_string(),
                        ":".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: self.name.clone(),
                version: Some(self.version.clone()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.log(MessageType::INFO, "PHPantomLSP initialized!".to_string())
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let uri = doc.uri.to_string();
        let text = doc.text;

        // Store file content
        if let Ok(mut files) = self.open_files.lock() {
            files.insert(uri.clone(), text.clone());
        }

        // Parse and update AST map
        self.update_ast(&uri, &text);

        self.log(MessageType::INFO, format!("Opened file: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Some(change) = params.content_changes.first() {
            let text = &change.text;

            // Update stored content
            if let Ok(mut files) = self.open_files.lock() {
                files.insert(uri.clone(), text.clone());
            }

            // Re-parse and update AST map
            self.update_ast(&uri, text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Ok(mut files) = self.open_files.lock() {
            files.remove(&uri);
        }

        if let Ok(mut map) = self.ast_map.lock() {
            map.remove(&uri);
        }

        self.log(MessageType::INFO, format!("Closed file: {}", uri))
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        // Get file content for offset calculation
        let content = if let Ok(files) = self.open_files.lock() {
            files.get(&uri).cloned()
        } else {
            None
        };

        // Get classes from ast_map
        let classes = if let Ok(map) = self.ast_map.lock() {
            map.get(&uri).cloned()
        } else {
            None
        };

        if let (Some(content), Some(classes)) = (content, classes) {
            // Try to extract a completion target (requires `->` or `::`)
            if let Some(target) = Self::extract_completion_target(&content, position) {
                let cursor_offset = Self::position_to_offset(&content, position);
                let current_class = cursor_offset
                    .and_then(|off| Self::find_class_at_offset(&classes, off));

                let resolved = Self::resolve_target_class(
                    &target.subject,
                    target.access_kind,
                    current_class,
                    &classes,
                    &content,
                    cursor_offset.unwrap_or(0),
                );

                if let Some(target_class) = resolved {
                    let items = Self::build_completion_items(target_class, target.access_kind);
                    if !items.is_empty() {
                        return Ok(Some(CompletionResponse::Array(items)));
                    }
                }
            }
        }

        // Fallback: return the default PHPantomLSP completion item
        Ok(Some(CompletionResponse::Array(vec![CompletionItem {
            label: "PHPantomLSP".to_string(),
            kind: Some(CompletionItemKind::TEXT),
            detail: Some("PHPantomLSP completion".to_string()),
            insert_text: Some("PHPantomLSP".to_string()),
            ..CompletionItem::default()
        }])))
    }
}