/// Type resolution for completion subjects.
///
/// This module contains the core entry points for resolving a completion
/// subject (e.g. `$this`, `self`, `static`, `$var`, `$this->prop`,
/// `ClassName`) to a concrete `ClassInfo` so that the correct completion
/// items can be offered.
///
/// The resolution logic is split across several sibling modules:
///
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
use crate::Backend;
use crate::docblock;
use crate::docblock::types::{parse_generic_args, split_union_depth0, strip_generics};
use crate::inheritance::apply_generic_args;
use crate::types::*;

use super::conditional_resolution::{
    resolve_conditional_with_text_args, resolve_conditional_without_args, split_call_subject,
};

/// A single bracket segment in a chained array access subject.
///
/// Used by [`parse_bracket_segments`] to decompose subjects like
/// `$response['items'][]` into structured parts.
#[derive(Debug, Clone)]
enum BracketSegment {
    /// A string-key access, e.g. `['items']` → `StringKey("items")`.
    StringKey(String),
    /// A numeric / variable index access, e.g. `[0]` or `[$i]` → `ElementAccess`.
    ElementAccess,
}

/// Result of parsing a chained array access subject.
#[derive(Debug)]
struct BracketSubject {
    /// The base variable (e.g. `"$response"`).
    base_var: String,
    /// The bracket segments in left-to-right order.
    segments: Vec<BracketSegment>,
}

/// Parse a subject like `$var['key'][]` into its base variable and
/// bracket segments.
///
/// Returns `None` if the subject doesn't start with `$` or has no `[`.
fn parse_bracket_segments(subject: &str) -> Option<BracketSubject> {
    if !subject.starts_with('$') || !subject.contains('[') {
        return None;
    }

    let first_bracket = subject.find('[')?;
    let base_var = subject[..first_bracket].to_string();
    if base_var.len() < 2 {
        return None;
    }

    let mut segments = Vec::new();
    let mut rest = &subject[first_bracket..];

    while rest.starts_with('[') {
        // Find the matching `]`.
        let close = rest.find(']')?;
        let inner = rest[1..close].trim();

        if let Some(key) = inner
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .or_else(|| inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
        {
            segments.push(BracketSegment::StringKey(key.to_string()));
        } else {
            segments.push(BracketSegment::ElementAccess);
        }

        rest = &rest[close + 1..];
    }

    if segments.is_empty() {
        return None;
    }

    Some(BracketSubject { base_var, segments })
}

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

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

/// Bundles the common parameters threaded through call-expression
/// return-type resolution.
///
/// This keeps the argument count of [`resolve_call_return_types`] under
/// clippy's `too_many_arguments` threshold.
pub(super) struct CallResolutionCtx<'a> {
    pub current_class: Option<&'a ClassInfo>,
    pub all_classes: &'a [ClassInfo],
    pub content: &'a str,
    pub cursor_offset: u32,
    pub class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    pub function_loader: FunctionLoaderFn<'a>,
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
            let base_classes = Self::resolve_target_classes(
                base,
                _access_kind,
                current_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                function_loader,
            );
            let mut results = Vec::new();
            for cls in &base_classes {
                let resolved =
                    Self::resolve_property_types(prop_name, cls, all_classes, class_loader);
                for r in resolved {
                    if !results.iter().any(|c: &ClassInfo| c.name == r.name) {
                        results.push(r);
                    }
                }
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
                    content,
                    cursor_offset,
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
                        is_deprecated: false,
                        template_params: vec![],
                        extends_generics: vec![],
                        implements_generics: vec![],
                        use_generics: vec![],
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

    /// Text-based assignment following for raw type extraction.
    ///
    /// Scans backward from `cursor_offset` for `$var = expr;`, then
    /// extracts the raw return type from the RHS expression.  This is
    /// used as a fallback when no `@var` / `@param` annotation is found.
    fn extract_raw_type_from_assignment_text(
        base_var: &str,
        content: &str,
        cursor_offset: usize,
        current_class: Option<&ClassInfo>,
        _all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let search_area = content.get(..cursor_offset)?;

        // Find the most recent assignment to this variable.
        let assign_pattern = format!("{} = ", base_var);
        let assign_pos = search_area.rfind(&assign_pattern)?;
        let rhs_start = assign_pos + assign_pattern.len();

        // Extract the RHS up to the next `;`
        let remaining = &content[rhs_start..];
        let semi_pos = Self::find_semicolon_balanced(remaining)?;
        let rhs_text = remaining[..semi_pos].trim();

        // RHS is a call expression — extract the return type.
        if rhs_text.ends_with(')') {
            let paren_pos = Self::find_top_level_open_paren(rhs_text)?;
            let callee = &rhs_text[..paren_pos];

            // Method call: `$this->methodName(…)`
            if let Some(method_name) = callee.strip_prefix("$this->") {
                let owner = current_class?;
                let merged = Self::resolve_class_with_inheritance(owner, class_loader);
                return merged
                    .methods
                    .iter()
                    .find(|m| m.name == method_name)
                    .and_then(|m| m.return_type.clone());
            }

            // Static call: `ClassName::methodName(…)`
            if let Some((class_part, method_part)) = callee.rsplit_once("::") {
                let resolved_class = if class_part == "self" || class_part == "static" {
                    current_class.cloned()
                } else {
                    class_loader(class_part)
                };
                if let Some(cls) = resolved_class {
                    let merged = Self::resolve_class_with_inheritance(&cls, class_loader);
                    return merged
                        .methods
                        .iter()
                        .find(|m| m.name == method_part)
                        .and_then(|m| m.return_type.clone());
                }
            }

            // Standalone function call — search all classes for a matching
            // global function.  Since we don't have `function_loader` here,
            // search backward in the source for a `@return` in the
            // function's docblock.
            return Self::extract_function_return_from_source(callee, content);
        }

        // RHS is a property access: `$this->propName`
        if let Some(prop_name) = rhs_text.strip_prefix("$this->")
            && prop_name.chars().all(|c| c.is_alphanumeric() || c == '_')
            && let Some(owner) = current_class
        {
            let merged = Self::resolve_class_with_inheritance(owner, class_loader);
            return merged
                .properties
                .iter()
                .find(|p| p.name == prop_name)
                .and_then(|p| p.type_hint.clone());
        }

        None
    }

    /// Find `;` in `s`, respecting `()`, `[]`, `{}`, and string nesting.
    fn find_semicolon_balanced(s: &str) -> Option<usize> {
        let mut depth_paren = 0i32;
        let mut depth_bracket = 0i32;
        let mut depth_brace = 0i32;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut prev_char = '\0';

        for (i, ch) in s.char_indices() {
            if in_single_quote {
                if ch == '\'' && prev_char != '\\' {
                    in_single_quote = false;
                }
                prev_char = ch;
                continue;
            }
            if in_double_quote {
                if ch == '"' && prev_char != '\\' {
                    in_double_quote = false;
                }
                prev_char = ch;
                continue;
            }
            match ch {
                '\'' => in_single_quote = true,
                '"' => in_double_quote = true,
                '(' => depth_paren += 1,
                ')' => depth_paren -= 1,
                '[' => depth_bracket += 1,
                ']' => depth_bracket -= 1,
                '{' => depth_brace += 1,
                '}' => depth_brace -= 1,
                ';' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                    return Some(i);
                }
                _ => {}
            }
            prev_char = ch;
        }
        None
    }

    /// Find the position of the first `(` at nesting depth 0.
    ///
    /// Respects `<…>` nesting for generic types but is careful not to
    /// treat `>` in `->` (arrow operator) as a closing angle bracket.
    fn find_top_level_open_paren(s: &str) -> Option<usize> {
        let mut depth_angle = 0i32;
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'<' => depth_angle += 1,
                b'>' if depth_angle > 0 => depth_angle -= 1,
                b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                    // Skip `->` entirely — it's an arrow operator, not
                    // an angle bracket.
                    i += 2;
                    continue;
                }
                b'(' if depth_angle == 0 => return Some(i),
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Search backward in `content` for a function definition matching
    /// `func_name` and extract its `@return` type from the docblock.
    fn extract_function_return_from_source(func_name: &str, content: &str) -> Option<String> {
        // Look for `function funcName(` in the source.
        let pattern = format!("function {}(", func_name);
        let func_pos = content.find(&pattern)?;

        // Search backward from the function definition for a docblock.
        let before = content.get(..func_pos)?;
        let trimmed = before.trim_end();
        if !trimmed.ends_with("*/") {
            return None;
        }
        let open_pos = trimmed.rfind("/**")?;
        let docblock = &trimmed[open_pos..];

        docblock::extract_return_type(docblock)
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
    /// Resolve the element type of an array/list variable accessed with `[]`.
    ///
    /// Given a base variable name like `$admins`, searches backward from
    /// Resolve a chained array access subject like `$var['key'][]`.
    ///
    /// Walks through each bracket segment in order:
    /// - `BracketSegment::StringKey(k)` → extract the value type for key
    ///   `k` from an array shape annotation.
    /// - `BracketSegment::ElementAccess` → extract the generic element
    ///   type (e.g. `list<User>` → `User`).
    ///
    /// Returns the resolved `ClassInfo` for the final type.
    fn resolve_chained_array_access(
        base_var: &str,
        segments: &[BracketSegment],
        content: &str,
        cursor_offset: u32,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Vec<ClassInfo> {
        let current_class_name = current_class.map(|c| c.name.as_str()).unwrap_or("");

        // 1. Resolve the raw type annotation for the base variable.
        let raw_type =
            docblock::find_iterable_raw_type_in_source(content, cursor_offset as usize, base_var)
                .or_else(|| {
                    Self::extract_raw_type_from_assignment_text(
                        base_var,
                        content,
                        cursor_offset as usize,
                        current_class,
                        all_classes,
                        class_loader,
                    )
                });

        let mut current_type = match raw_type {
            Some(t) => t,
            None => return vec![],
        };

        // 2. Walk through each bracket segment to refine the type.
        for seg in segments {
            match seg {
                BracketSegment::StringKey(key) => {
                    // Array shape key lookup: array{key: Type} → Type
                    current_type =
                        match docblock::extract_array_shape_value_type(&current_type, key) {
                            Some(t) => t,
                            None => return vec![],
                        };
                }
                BracketSegment::ElementAccess => {
                    // Generic element extraction: list<User> → User
                    current_type = match docblock::types::extract_generic_value_type(&current_type)
                    {
                        Some(t) => t,
                        None => return vec![],
                    };
                }
            }
        }

        // 3. Resolve the final type string to ClassInfo.
        let cleaned = docblock::clean_type(&current_type);
        let base_name = docblock::types::strip_generics(&cleaned);
        if base_name.is_empty() || docblock::types::is_scalar(&base_name) {
            return vec![];
        }

        Self::type_hint_to_classes(&cleaned, current_class_name, all_classes, class_loader)
    }

    pub(super) fn resolve_call_return_types(
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
        let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);

        // Strip surrounding parentheses that appear in DNF types like `(A&B)|C`.
        let hint = hint
            .strip_prefix('(')
            .and_then(|h| h.strip_suffix(')'))
            .unwrap_or(hint);

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

        // ── Parse generic arguments (if any) ──
        // `Collection<int, User>` → base_hint = `Collection`, generic_args = ["int", "User"]
        // `Foo`                   → base_hint = `Foo`,        generic_args = []
        let (base_hint, generic_args) = parse_generic_args(hint);

        // For class lookup, strip any remaining generics from the base
        // (should already be clean, but defensive) and use the short name.
        let base_clean = strip_generics(base_hint.strip_prefix('\\').unwrap_or(base_hint));
        let lookup = base_clean.rsplit('\\').next().unwrap_or(&base_clean);

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
}
