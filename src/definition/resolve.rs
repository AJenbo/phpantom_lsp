/// Goto-definition resolution.
///
/// Given a cursor position in a PHP file this module:
///   1. Extracts the symbol (class / interface / trait / enum name) under the cursor.
///   2. Resolves it to a fully-qualified name using the file's `use` map and namespace.
///   3. Locates the file on disk via PSR-4 mappings.
///   4. Finds the exact line of the symbol's declaration inside that file.
///   5. Returns an LSP `Location` the editor can jump to.
///
/// Additionally, it resolves **member** references (methods, properties, constants):
///   - `MyClass::MY_CONST`, `MyClass::myMethod()`, `MyClass::$staticProp`
///   - `$this->method()`, `$this->property`
///   - `self::CONST`, `static::method()`, `parent::method()`
///   - `$var->method()` (with type inference from assignments / parameter hints)
///   - Chained access: `$this->prop->method()`
use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::types::*;
use crate::util::{
    check_new_keyword_before, extract_new_expression_inside_parens, skip_balanced_parens_back,
};

/// The kind of class member being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Method,
    Property,
    Constant,
}

/// Hint about whether the member access looks like a method call or a property
/// access.  Used to disambiguate when a class has both a method and a property
/// with the same name (e.g. `id()` method vs `$id` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberAccessHint {
    /// Followed by `(` — looks like a method call.
    MethodCall,
    /// No `(` after the name — looks like a property / constant access.
    PropertyAccess,
    /// Cannot determine (fallback to original order).
    Unknown,
}

impl Backend {
    /// Handle a "go to definition" request.
    ///
    /// Returns `Some(Location)` when the symbol under the cursor can be
    /// resolved to a file and a position inside that file, or `None` when
    /// resolution fails at any step.
    pub(crate) fn resolve_definition(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<Location> {
        // 1. Extract the symbol name under the cursor.
        let word = Self::extract_word_at_position(content, position)?;

        if word.is_empty() {
            return None;
        }

        // ── NEW: Try member access resolution (::, ->, ?->) ──
        // If the cursor is on a member name (right side of an operator),
        // resolve the owning class and jump to the member declaration.
        if let Some(location) = self.resolve_member_definition(uri, content, position, &word) {
            return Some(location);
        }

        // ── Handle `self`, `static`, `parent` keywords ──
        // When the cursor is on one of these keywords (e.g. `new self()`,
        // `new static()`, `new parent()`), resolve to the enclosing class
        // definition (or the parent class for `parent`).
        if (word == "self" || word == "static" || word == "parent")
            && let Some(location) = self.resolve_self_static_parent(uri, content, position, &word)
        {
            return Some(location);
        }

        // 2. Gather context from the current file (use map + namespace).
        let file_use_map = self
            .use_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())
            .flatten();

        // 3. Resolve to a fully-qualified name.
        let fqn = Self::resolve_to_fqn(&word, &file_use_map, &file_namespace);

        // Build a list of FQN candidates to try.  The resolved name is tried
        // first, but when the original word already contains `\` (e.g. from a
        // `use` statement where the name is already fully-qualified) we also
        // try the raw word so we don't fail just because namespace-prefixing
        // produced a wrong result.
        let mut candidates = vec![fqn];
        if word.contains('\\') && !candidates.contains(&word) {
            candidates.push(word.clone());
        }

        // 4. Try to find the class in the current file first (same-file jump).
        for fqn in &candidates {
            if let Some(location) = self.find_definition_in_ast_map(fqn, content, uri) {
                return Some(location);
            }
        }

        // 5. Resolve file path via PSR-4 (only when workspace root is available).
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(workspace_root) = workspace_root
            && let Ok(mappings) = self.psr4_mappings.lock()
        {
            for fqn in &candidates {
                if let Some(file_path) =
                    composer::resolve_class_path(&mappings, &workspace_root, fqn)
                {
                    // 6. Read the target file and find the definition line.
                    if let Ok(target_content) = std::fs::read_to_string(&file_path) {
                        let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);
                        if let Some(target_position) =
                            Self::find_definition_position(&target_content, short_name)
                            && let Ok(target_uri) = Url::from_file_path(&file_path)
                        {
                            return Some(Location {
                                uri: target_uri,
                                range: Range {
                                    start: target_position,
                                    end: target_position,
                                },
                            });
                        }
                    }
                }
            }
        }

        // 7. Try global function lookup as a last resort.
        //    Build candidates: the word itself, the FQN-resolved version, and
        //    (if inside a namespace) the namespace-qualified version.
        let mut func_candidates = candidates.clone();
        if !func_candidates.contains(&word) {
            func_candidates.push(word.clone());
        }

        if let Some(location) = self.resolve_function_definition(&func_candidates) {
            return Some(location);
        }

        None
    }

    // ─── Function Definition Resolution ─────────────────────────────────────

    /// Try to resolve a standalone function name to its definition.
    ///
    /// Searches the `global_functions` map (populated from autoload files
    /// and opened/changed files) for any of the given candidate names.
    /// When found, reads the source file and locates the `function name(`
    /// declaration line.
    fn resolve_function_definition(&self, candidates: &[String]) -> Option<Location> {
        let (file_uri, func_info) = {
            let fmap = self.global_functions.lock().ok()?;
            let mut found = None;
            for candidate in candidates {
                if let Some((uri, info)) = fmap.get(candidate) {
                    found = Some((uri.clone(), info.clone()));
                    break;
                }
            }
            found
        }?;

        // Read the file content (try open files first, then disk).
        let file_content = self
            .open_files
            .lock()
            .ok()
            .and_then(|files| files.get(&file_uri).cloned())
            .or_else(|| {
                let path = Url::parse(&file_uri).ok()?.to_file_path().ok()?;
                std::fs::read_to_string(path).ok()
            })?;

        let position = Self::find_function_position(&file_content, &func_info.name)?;
        let parsed_uri = Url::parse(&file_uri).ok()?;

        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }

    /// Find the position of a standalone `function name(` declaration in
    /// file content.
    ///
    /// This is distinct from `find_member_position` (which searches inside
    /// a class body) — here we look for top-level or namespace-level
    /// function declarations.
    fn find_function_position(content: &str, function_name: &str) -> Option<Position> {
        let pattern = format!("function {}", function_name);

        let is_word_boundary = |c: u8| {
            let ch = c as char;
            !ch.is_alphanumeric() && ch != '_'
        };

        for (line_idx, line) in content.lines().enumerate() {
            if let Some(col) = line.find(&pattern) {
                // Verify word boundary before `function` keyword.
                let before_ok = col == 0 || is_word_boundary(line.as_bytes()[col - 1]);

                // Verify word boundary after the function name.
                let after_pos = col + pattern.len();
                let after_ok =
                    after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);

                if before_ok && after_ok {
                    return Some(Position {
                        line: line_idx as u32,
                        character: col as u32,
                    });
                }
            }
        }

        None
    }

    // ─── Member Definition Resolution ───────────────────────────────────────

    /// Try to resolve a member access pattern and jump to the member's
    /// declaration.
    ///
    /// Detects `::`, `->`, and `?->` before the word under the cursor,
    /// resolves the owning class, and finds the member position in the
    /// class's source file.
    fn resolve_member_definition(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        member_name: &str,
    ) -> Option<Location> {
        // 1. Detect the access operator and extract the subject (left side).
        let (subject, access_kind) = Self::extract_member_access_context(content, position)?;

        // 2. Gather context needed for class resolution.
        let cursor_offset = Self::position_to_offset(content, position)?;

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let current_class = Self::find_class_at_offset(&classes, cursor_offset).cloned();

        let file_use_map: HashMap<String, String> = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace: Option<String> = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        // Build a class_loader closure (same pattern as the completion handler).
        let class_loader = |name: &str| -> Option<ClassInfo> {
            let resolved_name = if !name.contains('\\') {
                if let Some(fqn) = file_use_map.get(name) {
                    fqn.as_str()
                } else if let Some(ref ns) = file_namespace {
                    let ns_qualified = format!("{}\\{}", ns, name);
                    if let Some(cls) = self.find_or_load_class(&ns_qualified) {
                        return Some(cls);
                    }
                    name
                } else {
                    name
                }
            } else {
                name
            };
            self.find_or_load_class(resolved_name)
        };

        // Build a function_loader closure for resolving standalone function
        // return types (needed for call-expression subjects like `app()->`).
        let function_loader = |name: &str| -> Option<FunctionInfo> {
            let fmap = self.global_functions.lock().ok()?;
            if let Some((_, info)) = fmap.get(name) {
                return Some(info.clone());
            }
            // Try resolving via use map
            if let Some(fqn) = file_use_map.get(name)
                && let Some((_, info)) = fmap.get(fqn.as_str())
            {
                return Some(info.clone());
            }
            // Try namespace-qualified
            if let Some(ref ns) = file_namespace {
                let ns_qualified = format!("{}\\{}", ns, name);
                if let Some((_, info)) = fmap.get(&ns_qualified) {
                    return Some(info.clone());
                }
            }
            None
        };

        // 3. Resolve the subject to all candidate classes.
        //    When a variable is assigned different types in conditional
        //    branches (e.g. if/else), multiple candidates are returned.
        let candidates = Self::resolve_target_classes(
            &subject,
            access_kind,
            current_class.as_ref(),
            &classes,
            content,
            cursor_offset,
            &class_loader,
            Some(&function_loader),
        );

        if candidates.is_empty() {
            return None;
        }

        // Determine whether this looks like a method call or property access.
        let access_hint = Self::detect_member_access_hint(content, position, member_name);

        // 4. Try each candidate class and pick the first one where the
        //    member actually exists (directly or via inheritance).
        for target_class in &candidates {
            let declaring_class =
                Self::find_declaring_class(target_class, member_name, &class_loader)
                    .unwrap_or_else(|| target_class.clone());

            // Check that the member is actually present on the declaring class.
            let member_kind =
                match Self::classify_member(&declaring_class, member_name, access_hint) {
                    Some(k) => k,
                    None => continue, // member not on this candidate, try next
                };

            // Locate the file that contains the declaring class.
            if let Some((class_uri, class_content)) =
                self.find_class_file_content(&declaring_class.name, uri, content)
                && let Some(member_position) =
                    Self::find_member_position(&class_content, member_name, member_kind)
                && let Ok(parsed_uri) = Url::parse(&class_uri)
            {
                return Some(Location {
                    uri: parsed_uri,
                    range: Range {
                        start: member_position,
                        end: member_position,
                    },
                });
            }
        }

        // No candidate had the member — fall back to the first candidate
        // and try the original (non-iterating) logic so we at least get
        // partial results when possible.
        let target_class = &candidates[0];
        let declaring_class = Self::find_declaring_class(target_class, member_name, &class_loader)
            .unwrap_or_else(|| target_class.clone());

        let member_kind = Self::classify_member(&declaring_class, member_name, access_hint)?;

        let (class_uri, class_content) =
            self.find_class_file_content(&declaring_class.name, uri, content)?;

        let member_position = Self::find_member_position(&class_content, member_name, member_kind)?;

        let parsed_uri = Url::parse(&class_uri).ok()?;
        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: member_position,
                end: member_position,
            },
        })
    }

    /// Detect the access operator (`::`, `->`, `?->`) immediately before the
    /// word under the cursor and extract the subject to its left.
    ///
    /// Returns `(subject, AccessKind)` or `None` if no operator is found.
    ///
    /// This works by:
    ///   1. Finding the start of the identifier under the cursor.
    ///   2. Skipping a `$` prefix if present (for `::$staticProp`).
    ///   3. Checking for `::`, `->`, or `?->` immediately before.
    ///   4. Extracting the subject expression to the left of the operator.
    fn extract_member_access_context(
        content: &str,
        position: Position,
    ) -> Option<(String, AccessKind)> {
        let lines: Vec<&str> = content.lines().collect();
        let line = lines.get(position.line as usize)?;
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        if chars.is_empty() {
            return None;
        }

        // Find the start of the identifier under the cursor.
        let mut i = col;

        // If the cursor is on or past the end of a word, adjust.
        if i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
            // on a word char — walk left
        } else if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        } else {
            return None;
        }

        // Walk left past identifier characters.
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        let mut operator_end = i;

        // Skip `$` prefix (for `Class::$staticProp`).
        if operator_end > 0 && chars[operator_end - 1] == '$' {
            operator_end -= 1;
        }

        // Detect `::`.
        if operator_end >= 2 && chars[operator_end - 2] == ':' && chars[operator_end - 1] == ':' {
            let subject = Self::extract_subject_before(&chars, operator_end - 2);
            if !subject.is_empty() {
                return Some((subject, AccessKind::DoubleColon));
            }
        }

        // Detect `->`.
        if operator_end >= 2 && chars[operator_end - 2] == '-' && chars[operator_end - 1] == '>' {
            let subject = Self::extract_arrow_subject_for_definition(&chars, operator_end - 2);
            if !subject.is_empty() {
                return Some((subject, AccessKind::Arrow));
            }
        }

        // Detect `?->` (null-safe operator).
        if operator_end >= 3
            && chars[operator_end - 3] == '?'
            && chars[operator_end - 2] == '-'
            && chars[operator_end - 1] == '>'
        {
            let subject = Self::extract_arrow_subject_for_definition(&chars, operator_end - 3);
            if !subject.is_empty() {
                return Some((subject, AccessKind::Arrow));
            }
        }

        None
    }

    /// Extract the subject expression before an arrow operator for definition
    /// resolution.
    ///
    /// Handles:
    ///   - `$this->`, `$var->` (simple variable)
    ///   - `$this->prop->` (property chain)
    ///   - `app()->` (function call)
    ///   - `$this->getService()->` (method call chain)
    ///   - `ClassName::make()->` (static method call)
    fn extract_arrow_subject_for_definition(chars: &[char], arrow_pos: usize) -> String {
        let end = arrow_pos;

        // Skip whitespace before the operator.
        let mut i = end;
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }

        // ── Function / method call or `new` expression: detect `)` ──
        // e.g. `app()->`, `$this->getService()->`, `Class::make()->`,
        //      `new Foo()->`, `(new Foo())->`
        if i > 0
            && chars[i - 1] == ')'
            && let Some(call_subject) = Self::extract_call_subject_for_definition(chars, i)
        {
            return call_subject;
        }

        // Read an identifier (could be a property name if chained).
        let ident_end = i;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }
        let ident_start = i;

        // Check for chained `->` (e.g. `$this->prop->member`).
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_subject = Self::extract_simple_var_before(chars, i - 2);
            if !inner_subject.is_empty() {
                let prop: String = chars[ident_start..ident_end].iter().collect();
                return format!("{}->{}", inner_subject, prop);
            }
        }

        // Check for chained `?->` (e.g. `$this?->prop->member`).
        if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_subject = Self::extract_simple_var_before(chars, i - 3);
            if !inner_subject.is_empty() {
                let prop: String = chars[ident_start..ident_end].iter().collect();
                return format!("{}?->{}", inner_subject, prop);
            }
        }

        // Simple variable like `$this` or `$var`.
        Self::extract_simple_var_before(chars, end)
    }

    /// Extract the full call-expression subject when `)` appears before an
    /// operator (used for definition resolution).
    ///
    /// `paren_end` is the position one past the closing `)`.
    ///
    /// Returns subjects such as:
    ///   - `"app()"` for a standalone function call without arguments
    ///   - `"app(A::class)"` for a function call with arguments (preserved)
    ///   - `"$this->getService()"` for an instance method call
    ///   - `"ClassName::make()"` for a static method call
    ///   - `"ClassName"` for `new ClassName()` instantiation
    fn extract_call_subject_for_definition(chars: &[char], paren_end: usize) -> Option<String> {
        let open = skip_balanced_parens_back(chars, paren_end)?;
        if open == 0 {
            return None;
        }

        // Capture the argument text between the parentheses for later use
        // in conditional return-type resolution (e.g. `app(A::class)`).
        let args_text: String = chars[open + 1..paren_end - 1].iter().collect();
        let args_text = args_text.trim();

        // Read the function / method name before `(`
        let mut i = open;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }
        if i == open {
            // No identifier before `(` — check if the contents inside the
            // balanced parens form a `(new ClassName(...))` expression.
            return extract_new_expression_inside_parens(chars, open, paren_end);
        }
        let func_name: String = chars[i..open].iter().collect();

        // ── `new ClassName()` instantiation ──
        // Check if the `new` keyword immediately precedes the class name.
        if let Some(class_name) = check_new_keyword_before(chars, i, &func_name) {
            return Some(class_name);
        }

        // Build the right-hand side of the call expression, preserving
        // arguments for conditional return-type resolution.
        let rhs = if args_text.is_empty() {
            format!("{}()", func_name)
        } else {
            format!("{}({})", func_name, args_text)
        };

        // Instance method call: `$this->method()` / `$var->method()` /
        // `app()->method()` (chained call expression)
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            // First check if the LHS is itself a call expression ending
            // with `)` — e.g. `app()->make(...)` where we need to
            // recursively resolve `app()`.
            let arrow_pos = i - 2;
            let mut j = arrow_pos;
            while j > 0 && chars[j - 1] == ' ' {
                j -= 1;
            }
            if j > 0
                && chars[j - 1] == ')'
                && let Some(inner_call) = Self::extract_call_subject_for_definition(chars, j)
            {
                return Some(format!("{}->{}", inner_call, rhs));
            }
            let inner_subject = Self::extract_simple_var_before(chars, i - 2);
            if !inner_subject.is_empty() {
                return Some(format!("{}->{}", inner_subject, rhs));
            }
        }

        // Null-safe method call: `$var?->method()`
        if i >= 3 && chars[i - 3] == '?' && chars[i - 2] == '-' && chars[i - 1] == '>' {
            let inner_subject = Self::extract_simple_var_before(chars, i - 3);
            if !inner_subject.is_empty() {
                return Some(format!("{}?->{}", inner_subject, rhs));
            }
        }

        // Static method call: `ClassName::method()` / `self::method()`
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            let class_subject = Self::extract_subject_before(chars, i - 2);
            if !class_subject.is_empty() {
                return Some(format!("{}::{}", class_subject, rhs));
            }
        }

        // Standalone function call: preserve arguments for conditional
        // return-type resolution (e.g. `app(A::class)` instead of `app()`).
        Some(rhs)
    }

    /// Extract a `$variable` ending at position `end` (exclusive).
    fn extract_simple_var_before(chars: &[char], end: usize) -> String {
        let mut i = end;
        // Skip whitespace.
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        let var_end = i;
        // Walk back through identifier characters.
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }
        // Expect `$` prefix for a variable.
        if i > 0 && chars[i - 1] == '$' {
            i -= 1;
            chars[i..var_end].iter().collect()
        } else {
            // No `$` — return whatever we collected (might be empty).
            chars[i..var_end].iter().collect()
        }
    }

    /// Extract the identifier/keyword before `::`.
    ///
    /// Handles `self::`, `static::`, `parent::`, `ClassName::`,
    /// `Namespace\ClassName::`.
    fn extract_subject_before(chars: &[char], colon_pos: usize) -> String {
        let mut i = colon_pos;
        // Skip whitespace.
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        let end = i;
        // Walk back through identifier characters (including `\` for namespaces).
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }
        // Also accept `$` prefix for `$var::` edge case.
        if i > 0 && chars[i - 1] == '$' {
            i -= 1;
        }
        chars[i..end].iter().collect()
    }

    // ─── Member Classification ──────────────────────────────────────────────

    /// Determine the kind of member (method, property, or constant) by
    /// checking the class's parsed information.
    ///
    /// Returns `None` if the member is not found in the class.
    fn classify_member(
        class: &ClassInfo,
        member_name: &str,
        hint: MemberAccessHint,
    ) -> Option<MemberKind> {
        let has_method = class.methods.iter().any(|m| m.name == member_name);
        let has_property = class.properties.iter().any(|p| p.name == member_name);
        let has_constant = class.constants.iter().any(|c| c.name == member_name);

        match hint {
            MemberAccessHint::PropertyAccess => {
                // Prefer property/constant over method when there's no `()`.
                if has_property {
                    return Some(MemberKind::Property);
                }
                if has_constant {
                    return Some(MemberKind::Constant);
                }
                if has_method {
                    return Some(MemberKind::Method);
                }
            }
            MemberAccessHint::MethodCall => {
                // Prefer method when followed by `()`.
                if has_method {
                    return Some(MemberKind::Method);
                }
                if has_property {
                    return Some(MemberKind::Property);
                }
                if has_constant {
                    return Some(MemberKind::Constant);
                }
            }
            MemberAccessHint::Unknown => {
                // Default order: method, property, constant.
                if has_method {
                    return Some(MemberKind::Method);
                }
                if has_property {
                    return Some(MemberKind::Property);
                }
                if has_constant {
                    return Some(MemberKind::Constant);
                }
            }
        }
        None
    }

    /// Determine whether the member name at the given position is followed by
    /// `(` (indicating a method call) or not (indicating property / constant
    /// access).
    fn detect_member_access_hint(
        content: &str,
        position: Position,
        member_name: &str,
    ) -> MemberAccessHint {
        let lines: Vec<&str> = content.lines().collect();
        let line = match lines.get(position.line as usize) {
            Some(l) => *l,
            None => return MemberAccessHint::Unknown,
        };
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Find the end of the member name by walking right from the cursor.
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

        let mut end = col;
        // If cursor is on a word char, walk right to end of word.
        if end < chars.len() && is_word_char(chars[end]) {
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        } else if end > 0 && is_word_char(chars[end - 1]) {
            // Cursor is just past the word; `end` is already correct.
        } else {
            // Try to find the member name by searching forward from col.
            if let Some(idx) = line[col..].find(member_name) {
                end = col + idx + member_name.len();
            } else {
                return MemberAccessHint::Unknown;
            }
        }

        // Skip whitespace after the word.
        let mut i = end;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        if i < chars.len() && chars[i] == '(' {
            MemberAccessHint::MethodCall
        } else {
            MemberAccessHint::PropertyAccess
        }
    }

    /// Walk up the inheritance chain to find the class that actually declares
    /// the given member.
    ///
    /// Returns `Some(ClassInfo)` of the declaring class, or `None` if the
    /// member cannot be found in any ancestor.
    fn find_declaring_class(
        class: &ClassInfo,
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        const MAX_DEPTH: usize = 20;

        // Check if this class directly declares the member.
        if Self::classify_member(class, member_name, MemberAccessHint::Unknown).is_some() {
            return Some(class.clone());
        }

        // Check traits used by this class.
        if let Some(found) =
            Self::find_declaring_in_traits(&class.used_traits, member_name, class_loader, 0)
        {
            return Some(found);
        }

        // Walk up the parent chain.
        let mut current = class.clone();
        for _ in 0..MAX_DEPTH {
            let parent_name = match current.parent_class.as_ref() {
                Some(name) => name.clone(),
                None => break,
            };
            let parent = match class_loader(&parent_name) {
                Some(p) => p,
                None => break,
            };
            if Self::classify_member(&parent, member_name, MemberAccessHint::Unknown).is_some() {
                return Some(parent);
            }
            // Check traits used by the parent class.
            if let Some(found) =
                Self::find_declaring_in_traits(&parent.used_traits, member_name, class_loader, 0)
            {
                return Some(found);
            }
            current = parent;
        }

        // Check @mixin classes — these have the lowest precedence.
        if let Some(found) =
            Self::find_declaring_in_mixins(&class.mixins, member_name, class_loader, 0)
        {
            return Some(found);
        }

        None
    }

    /// Search through a list of trait names for one that declares `member_name`.
    ///
    /// Traits can themselves `use` other traits, so this recurses up to a
    /// depth limit to handle trait composition.
    fn find_declaring_in_traits(
        trait_names: &[String],
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: usize,
    ) -> Option<ClassInfo> {
        const MAX_TRAIT_DEPTH: usize = 20;
        if depth > MAX_TRAIT_DEPTH {
            return None;
        }

        for trait_name in trait_names {
            let trait_info = if let Some(t) = class_loader(trait_name) {
                t
            } else {
                continue;
            };
            if Self::classify_member(&trait_info, member_name, MemberAccessHint::Unknown).is_some()
            {
                return Some(trait_info);
            }
            // Recurse into traits used by this trait.
            if let Some(found) = Self::find_declaring_in_traits(
                &trait_info.used_traits,
                member_name,
                class_loader,
                depth + 1,
            ) {
                return Some(found);
            }
        }

        None
    }

    /// Search through `@mixin` class names for one that declares `member_name`.
    ///
    /// Mixin classes are resolved with their full inheritance chain (parent
    /// classes, traits) so that inherited members are found.  Only public
    /// members are considered since mixins proxy via magic methods.
    /// Mixin classes can themselves declare `@mixin`, so this recurses up
    /// to a depth limit.
    fn find_declaring_in_mixins(
        mixin_names: &[String],
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: usize,
    ) -> Option<ClassInfo> {
        const MAX_MIXIN_DEPTH: usize = 10;
        if depth > MAX_MIXIN_DEPTH {
            return None;
        }

        for mixin_name in mixin_names {
            let mixin_class = if let Some(c) = class_loader(mixin_name) {
                c
            } else {
                continue;
            };

            // Try to find the declaring class within the mixin's own
            // hierarchy (itself, its traits, its parents).
            if let Some(found) = Self::find_declaring_class(&mixin_class, member_name, class_loader)
            {
                return Some(found);
            }

            // Recurse into mixins declared by this mixin class.
            if !mixin_class.mixins.is_empty()
                && let Some(found) = Self::find_declaring_in_mixins(
                    &mixin_class.mixins,
                    member_name,
                    class_loader,
                    depth + 1,
                )
            {
                return Some(found);
            }
        }

        None
    }

    // ─── File & Position Lookup ─────────────────────────────────────────────

    /// Find the file URI and content for the file that contains a given class.
    ///
    /// Searches the `ast_map` (which includes files loaded via PSR-4 by
    /// `find_or_load_class`) and returns `(uri, content)`.
    fn find_class_file_content(
        &self,
        class_name: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<(String, String)> {
        // Search the ast_map for the file containing this class.
        let uri = {
            let map = self.ast_map.lock().ok()?;

            // Check the current file first (common case: $this->method).
            if let Some(classes) = map.get(current_uri) {
                if classes.iter().any(|c| c.name == class_name) {
                    Some(current_uri.to_string())
                } else {
                    // Search other files.
                    map.iter()
                        .find(|(_, classes)| classes.iter().any(|c| c.name == class_name))
                        .map(|(u, _)| u.clone())
                }
            } else {
                map.iter()
                    .find(|(_, classes)| classes.iter().any(|c| c.name == class_name))
                    .map(|(u, _)| u.clone())
            }
        }?;

        // Get the file content.
        let file_content = if uri == current_uri {
            current_content.to_string()
        } else {
            // Try open files first, then read from disk.
            let from_open = self
                .open_files
                .lock()
                .ok()
                .and_then(|files| files.get(&uri).cloned());

            if let Some(c) = from_open {
                c
            } else {
                // Parse the URI to a file path and read from disk.
                let path = Url::parse(&uri).ok()?.to_file_path().ok()?;
                std::fs::read_to_string(path).ok()?
            }
        };

        Some((uri, file_content))
    }

    /// Find the position of a member declaration (method, property, or constant)
    /// inside a PHP file.
    ///
    /// Searches line by line for the declaration pattern corresponding to the
    /// member kind, with word-boundary checks to avoid partial matches.
    fn find_member_position(
        content: &str,
        member_name: &str,
        kind: MemberKind,
    ) -> Option<Position> {
        let is_word_boundary = |c: u8| {
            let ch = c as char;
            !ch.is_alphanumeric() && ch != '_'
        };

        for (line_idx, line) in content.lines().enumerate() {
            match kind {
                MemberKind::Method => {
                    // Look for `function methodName` with word boundaries.
                    let pattern = format!("function {}", member_name);
                    if let Some(col) = line.find(&pattern) {
                        let after_pos = col + pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                        if after_ok {
                            return Some(Position {
                                line: line_idx as u32,
                                character: col as u32,
                            });
                        }
                    }
                }
                MemberKind::Constant => {
                    // Look for `const CONSTANT_NAME` with word boundaries.
                    let pattern = format!("const {}", member_name);
                    if let Some(col) = line.find(&pattern) {
                        // Make sure `const` is preceded by a word boundary
                        // (not part of another identifier).
                        let before_ok = col == 0 || is_word_boundary(line.as_bytes()[col - 1]);
                        let after_pos = col + pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                        if before_ok && after_ok {
                            return Some(Position {
                                line: line_idx as u32,
                                character: col as u32,
                            });
                        }
                    }
                }
                MemberKind::Property => {
                    // Look for `$propertyName` on a line that looks like a
                    // property declaration (has a visibility keyword, `var`,
                    // or `readonly`).
                    let var_pattern = format!("${}", member_name);
                    if let Some(col) = line.find(&var_pattern) {
                        let after_pos = col + var_pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);

                        if after_ok {
                            let trimmed = line.trim_start();
                            let is_declaration = trimmed.starts_with("public")
                                || trimmed.starts_with("protected")
                                || trimmed.starts_with("private")
                                || trimmed.starts_with("var ")
                                || trimmed.starts_with("readonly")
                                || trimmed.starts_with("static");

                            // Also detect promoted constructor properties:
                            // `public function __construct(private Type $prop)`
                            // In this case the visibility keyword appears
                            // inside the parameter list on the same line.
                            let is_promoted = !is_declaration && {
                                // Check if visibility keyword appears before
                                // the `$prop` on the same line (inside parens).
                                let before = &line[..col];
                                before.contains("public")
                                    || before.contains("protected")
                                    || before.contains("private")
                                    || before.contains("readonly")
                            };

                            if is_declaration || is_promoted {
                                return Some(Position {
                                    line: line_idx as u32,
                                    character: col as u32,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Fallback: for properties, check if this is a magic property
        // declared via a `@property` tag in the class docblock.
        // Lines look like: ` * @property Type $propertyName`
        if kind == MemberKind::Property {
            let var_pattern = format!("${}", member_name);
            for (line_idx, line) in content.lines().enumerate() {
                if let Some(col) = line.find(&var_pattern) {
                    let after_pos = col + var_pattern.len();
                    let after_ok =
                        after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                    if !after_ok {
                        continue;
                    }

                    let trimmed = line.trim().trim_start_matches('*').trim();
                    if trimmed.starts_with("@property-read")
                        || trimmed.starts_with("@property-write")
                        || trimmed.starts_with("@property")
                    {
                        return Some(Position {
                            line: line_idx as u32,
                            character: col as u32,
                        });
                    }
                }
            }
        }

        // Fallback: for methods, check if this is a magic method
        // declared via a `@method` tag in the class docblock.
        // Lines look like: ` * @method ReturnType methodName(params...)`
        if kind == MemberKind::Method {
            // The method name is followed by `(` in a @method tag.
            let method_pattern = member_name;
            for (line_idx, line) in content.lines().enumerate() {
                if let Some(col) = line.find(method_pattern) {
                    // Verify the character after the name is `(` (method call syntax).
                    let after_pos = col + method_pattern.len();
                    if after_pos >= line.len() {
                        continue;
                    }
                    let after_char = line.as_bytes()[after_pos];
                    if after_char != b'(' {
                        continue;
                    }

                    // Verify the character before is a word boundary (whitespace)
                    // to avoid matching partial names.
                    if col > 0 && !is_word_boundary(line.as_bytes()[col - 1]) {
                        continue;
                    }

                    let trimmed = line.trim().trim_start_matches('*').trim();
                    if trimmed.starts_with("@method") {
                        return Some(Position {
                            line: line_idx as u32,
                            character: col as u32,
                        });
                    }
                }
            }
        }

        None
    }

    // ─── Word Extraction & FQN Resolution (unchanged) ───────────────────────

    /// Extract the symbol name (class / interface / trait / enum / namespace)
    /// at the given cursor position.
    ///
    /// The word is defined as a contiguous run of alphanumeric characters,
    /// underscores, and backslashes (to capture fully-qualified names).
    pub fn extract_word_at_position(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        if line_idx >= lines.len() {
            return None;
        }

        let line = lines[line_idx];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Nothing to do on an empty line or if cursor is at position 0
        // with no word character.
        if chars.is_empty() {
            return None;
        }

        // If the cursor is right after a word (col points at a non-word char
        // or end-of-line), we still want to resolve the word to its left.
        // But if the cursor is in the middle of a word, expand in both
        // directions.

        let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '\\';

        // Find the start of the word: walk left from cursor.
        let mut start = col;

        // If cursor is between two chars and the right one is a word char,
        // start there.  Otherwise start from the char to the left.
        if start < chars.len() && is_word_char(chars[start]) {
            // cursor is on a word char — expand left
        } else if start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        } else {
            return None;
        }

        // Walk left to find start of word
        while start > 0 && is_word_char(chars[start - 1]) {
            start -= 1;
        }

        // Walk right to find end of word
        let mut end = col;
        if end < chars.len() && is_word_char(chars[end]) {
            // cursor is on a word char — also expand right
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        } else {
            // Cursor was past the word — expand right from start
            end = start;
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        }

        if start == end {
            return None;
        }

        let word: String = chars[start..end].iter().collect();

        // Strip a leading `\` (PHP fully-qualified prefix).
        let word = word.strip_prefix('\\').unwrap_or(&word).to_string();

        // Strip trailing `\` if any (partial namespace).
        let word = word.strip_suffix('\\').unwrap_or(&word).to_string();

        if word.is_empty() {
            return None;
        }

        Some(word)
    }

    /// Resolve a short or partially-qualified name to a fully-qualified name
    /// using the file's `use` map and namespace context.
    ///
    /// Rules:
    ///   - If the name contains `\` it is already (partially) qualified.
    ///     Check if the first segment is in the use_map; if so, expand it.
    ///     Otherwise prefix with the current namespace.
    ///   - If the name is unqualified (no `\`):
    ///     1. Check the use_map for a direct mapping.
    ///     2. Prefix with the current namespace.
    ///     3. Fall back to the bare name (global namespace).
    pub fn resolve_to_fqn(
        name: &str,
        use_map: &std::collections::HashMap<String, String>,
        namespace: &Option<String>,
    ) -> String {
        // Already fully-qualified (leading `\` was stripped earlier).
        // If name contains `\`, check if the first segment is aliased.
        if name.contains('\\') {
            let first_segment = name.split('\\').next().unwrap_or(name);
            if let Some(fqn_prefix) = use_map.get(first_segment) {
                // Replace the first segment with the FQN prefix.
                let rest = &name[first_segment.len()..];
                return format!("{}{}", fqn_prefix, rest);
            }
            // Not in use map — might already be fully-qualified, or
            // needs current namespace prepended.
            if let Some(ns) = namespace {
                return format!("{}\\{}", ns, name);
            }
            return name.to_string();
        }

        // Unqualified name — try use_map first.
        if let Some(fqn) = use_map.get(name) {
            return fqn.clone();
        }

        // Try current namespace.
        if let Some(ns) = namespace {
            return format!("{}\\{}", ns, name);
        }

        // Fall back to global / bare name.
        name.to_string()
    }

    /// Try to find the definition of a class in the current file by checking
    /// the ast_map.
    fn find_definition_in_ast_map(&self, fqn: &str, content: &str, uri: &str) -> Option<Location> {
        let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|map| map.get(uri).cloned())?;

        let _class_info = classes.iter().find(|c| c.name == short_name)?;

        // Convert start_offset to a position.  start_offset is the opening
        // brace — scan backwards to find the class/interface keyword line.
        let position = Self::find_definition_position(content, short_name)?;

        // Build a file URI from the current URI string.
        let parsed_uri = Url::parse(uri).ok()?;

        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }

    /// Find the position (line, character) of a class / interface / trait / enum
    /// declaration inside the given file content.
    ///
    /// Searches for patterns like:
    ///   `class ClassName`
    ///   `interface ClassName`
    ///   `trait ClassName`
    ///   `enum ClassName`
    ///   `abstract class ClassName`
    ///   `final class ClassName`
    ///   `readonly class ClassName`
    ///
    /// Returns the position of the keyword (`class`, `interface`, etc.) on
    /// the matching line.
    /// Resolve `self`, `static`, or `parent` keywords to a class definition.
    ///
    /// - `self` / `static` → jump to the enclosing class declaration.
    /// - `parent` → jump to the parent class declaration (from `extends`).
    fn resolve_self_static_parent(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        keyword: &str,
    ) -> Option<Location> {
        let cursor_offset = Self::position_to_offset(content, position)?;

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let current_class = Self::find_class_at_offset(&classes, cursor_offset)?;

        if keyword == "self" || keyword == "static" {
            // Jump to the enclosing class definition in the current file.
            let target_position = Self::find_definition_position(content, &current_class.name)?;
            let parsed_uri = Url::parse(uri).ok()?;
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: target_position,
                    end: target_position,
                },
            });
        }

        // keyword == "parent"
        let parent_name = current_class.parent_class.as_ref()?;

        // Try to find the parent class in the current file first.
        if let Some(pos) = Self::find_definition_position(content, parent_name) {
            let parsed_uri = Url::parse(uri).ok()?;
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: pos,
                    end: pos,
                },
            });
        }

        // Resolve the parent class name to a FQN using use-map / namespace.
        let file_use_map = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        let fqn = Self::resolve_to_fqn(parent_name, &file_use_map, &file_namespace);

        // Try class_index / ast_map lookup via find_class_file_content.
        let short_name = fqn.rsplit('\\').next().unwrap_or(&fqn);
        if let Some((class_uri, class_content)) =
            self.find_class_file_content(short_name, uri, content)
            && let Some(pos) = Self::find_definition_position(&class_content, short_name)
            && let Ok(parsed_uri) = Url::parse(&class_uri)
        {
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: pos,
                    end: pos,
                },
            });
        }

        // Try PSR-4 resolution as a last resort.
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(workspace_root) = workspace_root
            && let Ok(mappings) = self.psr4_mappings.lock()
        {
            let candidates = [fqn.as_str(), parent_name.as_str()];
            for candidate in &candidates {
                if let Some(file_path) =
                    composer::resolve_class_path(&mappings, &workspace_root, candidate)
                    && let Ok(target_content) = std::fs::read_to_string(&file_path)
                {
                    let name = candidate.rsplit('\\').next().unwrap_or(candidate);
                    if let Some(target_position) =
                        Self::find_definition_position(&target_content, name)
                        && let Ok(target_uri) = Url::from_file_path(&file_path)
                    {
                        return Some(Location {
                            uri: target_uri,
                            range: Range {
                                start: target_position,
                                end: target_position,
                            },
                        });
                    }
                }
            }
        }

        None
    }

    pub fn find_definition_position(content: &str, class_name: &str) -> Option<Position> {
        let keywords = ["class", "interface", "trait", "enum"];

        for (line_idx, line) in content.lines().enumerate() {
            for keyword in &keywords {
                // Search for `keyword ClassName` making sure ClassName is
                // followed by a word boundary (whitespace, `{`, `:`, end of
                // line) so we don't match partial names.
                let pattern = format!("{} {}", keyword, class_name);
                if let Some(col) = line.find(&pattern) {
                    // Verify word boundary before the keyword: either start
                    // of line or preceded by whitespace / non-alphanumeric.
                    let before_ok = col == 0 || {
                        let prev = line.as_bytes().get(col - 1).copied().unwrap_or(b' ');
                        !(prev as char).is_alphanumeric() && prev != b'_'
                    };

                    // Verify word boundary after the class name.
                    let after_pos = col + pattern.len();
                    let after_ok = after_pos >= line.len() || {
                        let next = line.as_bytes().get(after_pos).copied().unwrap_or(b' ');
                        !(next as char).is_alphanumeric() && next != b'_'
                    };

                    if before_ok && after_ok {
                        return Some(Position {
                            line: line_idx as u32,
                            character: col as u32,
                        });
                    }
                }
            }
        }

        None
    }
}
