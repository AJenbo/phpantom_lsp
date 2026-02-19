/// Array shape key completion.
///
/// This module detects when the cursor is inside an array access expression
/// with a string key (e.g. `$config['`) and offers completion items for
/// the known keys of the array shape type annotation.
///
/// It also provides helpers for resolving the raw type annotation of a
/// variable so that array shape entries can be extracted.
///
/// # Supported annotation sources
///
/// - `/** @var array{name: string, age: int} $var */` — inline `@var`
/// - `@param array{name: string, age: int} $param` — method/function parameter
/// - `@return array{name: string}` — return type of a function/method call
///   assigned to the variable
/// - Property type annotations (`@var` on class properties)
/// - `$_SERVER` superglobal — hardcoded key completions for all 40 well-known keys
///
/// # Auto-close handling
///
/// Completion items use `text_edit` with a range that covers any trailing
/// auto-inserted characters (closing quote + `]`) placed by the IDE.
/// This prevents duplicates like `$config['host']]` or `$config['host']']`.
///
/// # Nested array shapes
///
/// Chained array accesses like `$response['meta']['` are supported.
/// The detector collects prefix keys (`["meta"]`) and the resolver walks
/// through each level of the shape to offer keys from the inner type.
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::docblock;
use crate::types::ClassInfo;

/// Well-known keys for the `$_SERVER` superglobal.
///
/// Each entry is `(key, detail)` where `detail` is a short description
/// shown next to the completion item.
const SERVER_KEYS: &[(&str, &str)] = &[
    ("PHP_SELF", "string — Current script filename"),
    ("argv", "array — Arguments passed to the script"),
    ("argc", "int — Number of arguments passed to the script"),
    ("GATEWAY_INTERFACE", "string — CGI specification revision"),
    ("SERVER_ADDR", "string — Server IP address"),
    ("SERVER_NAME", "string — Server hostname"),
    ("SERVER_SOFTWARE", "string — Server identification string"),
    (
        "SERVER_PROTOCOL",
        "string — Name and revision of the protocol",
    ),
    ("REQUEST_METHOD", "string — Request method (GET, POST, …)"),
    ("REQUEST_TIME", "int — Timestamp of the request start"),
    ("REQUEST_TIME_FLOAT", "float — Timestamp with microseconds"),
    ("QUERY_STRING", "string — The query string"),
    ("DOCUMENT_ROOT", "string — Document root directory"),
    ("HTTP_ACCEPT", "string — Accept header contents"),
    ("HTTP_ACCEPT_CHARSET", "string — Accept-Charset header"),
    ("HTTP_ACCEPT_ENCODING", "string — Accept-Encoding header"),
    ("HTTP_ACCEPT_LANGUAGE", "string — Accept-Language header"),
    ("HTTP_CONNECTION", "string — Connection header"),
    ("HTTP_HOST", "string — Host header"),
    ("HTTP_REFERER", "string — Referring page URL"),
    ("HTTP_USER_AGENT", "string — User agent string"),
    ("HTTPS", "string — Set to 'on' if HTTPS is used"),
    ("REMOTE_ADDR", "string — Client IP address"),
    ("REMOTE_HOST", "string — Client hostname"),
    ("REMOTE_PORT", "string — Client port"),
    ("REMOTE_USER", "string — Authenticated user"),
    (
        "REDIRECT_REMOTE_USER",
        "string — Authenticated user (redirect)",
    ),
    ("SCRIPT_FILENAME", "string — Absolute path of the script"),
    ("SERVER_ADMIN", "string — SERVER_ADMIN directive value"),
    ("SERVER_PORT", "string — Server port"),
    ("SERVER_SIGNATURE", "string — Server signature string"),
    ("PATH_TRANSLATED", "string — Filesystem path of the script"),
    ("SCRIPT_NAME", "string — Current script path"),
    ("REQUEST_URI", "string — URI used to access the page"),
    ("PHP_AUTH_DIGEST", "string — Digest HTTP auth header"),
    ("PHP_AUTH_USER", "string — HTTP auth username"),
    ("PHP_AUTH_PW", "string — HTTP auth password"),
    ("AUTH_TYPE", "string — Authentication type"),
    ("PATH_INFO", "string — Client-provided path info"),
    ("ORIG_PATH_INFO", "string — Original PATH_INFO"),
];

/// The result of detecting an array key completion context.
///
/// Returned by [`detect_array_key_context`] when the cursor is positioned
/// inside an array access with a string key (or right after `[`).
#[derive(Debug, Clone)]
pub(crate) struct ArrayKeyContext {
    /// The variable name including the `$` prefix (e.g. `"$config"`).
    pub var_name: String,
    /// The partial key the user has typed so far (may be empty).
    /// Does **not** include the opening quote character.
    pub partial_key: String,
    /// The quote character used (`'` or `"`), or `None` if the user
    /// typed `[` without a quote yet.
    pub quote_char: Option<char>,
    /// The column (0-based) where the key text begins on the cursor line.
    /// This is right after the opening quote (if any) or right after `[`.
    pub key_start_col: u32,
    /// Keys from preceding chained array accesses, innermost first.
    ///
    /// For `$response['meta']['page'][`, this would be `["meta", "page"]`
    /// so the resolver can walk through nested array shapes.
    pub prefix_keys: Vec<String>,
}

/// Detect whether the cursor is in an array key completion context.
///
/// Recognises patterns like:
///   - `$var['`                    — empty partial, single-quote
///   - `$var['na`                  — partial "na", single-quote
///   - `$var["`                    — empty partial, double-quote
///   - `$var["na`                  — partial "na", double-quote
///   - `$var[`                     — no quote yet
///   - `$var['key1']['key2'][`     — chained access (nested shapes)
///   - `$var['key1']['key2']['`    — chained access with quote
///
/// Returns `None` if the cursor is not in such a context.
pub(crate) fn detect_array_key_context(
    content: &str,
    position: Position,
) -> Option<ArrayKeyContext> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let chars: Vec<char> = line.chars().collect();
    let col = (position.character as usize).min(chars.len());

    if col == 0 {
        return None;
    }

    // Walk backward from the cursor to find the pattern.
    let mut i = col;

    // 1. Collect partial key text (identifier characters the user has typed).
    let partial_end = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }
    let partial_start = i;

    // 2. Check for a quote character.
    let quote_char = if i > 0 && (chars[i - 1] == '\'' || chars[i - 1] == '"') {
        let q = chars[i - 1];
        i -= 1;
        Some(q)
    } else {
        None
    };

    // 3. Must have `[` immediately before the quote (or the partial if no quote).
    if i == 0 || chars[i - 1] != '[' {
        return None;
    }
    i -= 1; // skip `[`

    let key_start_col = partial_start as u32;

    // 4. Try to collect chained `['key']` access segments before the
    //    current `[`.  Walk backward through zero or more `]['key']`
    //    or `]["key"]` patterns, collecting the keys.
    let mut prefix_keys: Vec<String> = Vec::new();
    loop {
        // We're now right before the `[` we just consumed.
        // Check if there is a preceding `]` — that would indicate a
        // chained access like `$var['k1']['k2'][`.
        if i == 0 || chars[i - 1] != ']' {
            break;
        }
        // Try to parse the preceding `['key']` segment.
        let saved_i = i;
        i -= 1; // skip `]`

        // Expect a closing quote.
        if i == 0 || (chars[i - 1] != '\'' && chars[i - 1] != '"') {
            i = saved_i;
            break;
        }
        let prev_quote = chars[i - 1];
        i -= 1; // skip closing quote

        // Collect the key text (walk backward to the matching opening quote).
        let key_end = i;
        while i > 0 && chars[i - 1] != prev_quote {
            i -= 1;
        }
        if i == 0 {
            i = saved_i;
            break;
        }
        let key_text: String = chars[i..key_end].iter().collect();
        i -= 1; // skip opening quote

        // Expect `[` before the opening quote.
        if i == 0 || chars[i - 1] != '[' {
            i = saved_i;
            break;
        }
        i -= 1; // skip `[`

        prefix_keys.push(key_text);
    }

    // Reverse so prefix_keys[0] is the outermost key.
    prefix_keys.reverse();

    // 5. Extract the variable name before the first `[`.
    //    Walk backward through identifier chars, then expect `$`.
    let bracket_pos = i;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
        i -= 1;
    }
    if i == 0 || chars[i - 1] != '$' {
        return None;
    }
    i -= 1; // include `$`

    let var_name: String = chars[i..bracket_pos].iter().collect();
    if var_name.len() < 2 {
        // Must be at least `$x`
        return None;
    }

    let partial_key: String = chars[partial_start..partial_end].iter().collect();

    Some(ArrayKeyContext {
        var_name,
        partial_key,
        quote_char,
        key_start_col,
        prefix_keys,
    })
}

impl Backend {
    /// Build completion items for array shape keys.
    ///
    /// Given an `ArrayKeyContext`, resolves the variable's type annotation
    /// and, if it is an array shape, returns completion items for each key.
    ///
    /// Uses `text_edit` with a range that covers any auto-inserted trailing
    /// characters (closing quote + `]`) so that accepting a completion does
    /// not produce duplicate brackets.
    pub(crate) fn build_array_key_completions(
        &self,
        ctx: &ArrayKeyContext,
        content: &str,
        position: Position,
        classes: &[ClassInfo],
        file_use_map: &std::collections::HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Vec<CompletionItem> {
        // ── $_SERVER superglobal — hardcoded keys ────────────────────
        if ctx.var_name == "$_SERVER" && ctx.prefix_keys.is_empty() {
            return self.build_server_key_completions(ctx, content, position);
        }

        let cursor_offset = Self::position_to_offset(content, position).unwrap_or(0);

        // Try to find the raw type annotation for this variable.
        // We also track which set of classes was used for resolution so
        // that type alias expansion can consult the same set (important
        // when the original parse fails and patched classes are used).
        let raw_type = self.resolve_variable_raw_type(
            &ctx.var_name,
            content,
            cursor_offset as usize,
            classes,
            file_use_map,
            file_namespace,
        );

        // If initial resolution failed, the content likely has a syntax
        // error (e.g. unclosed `$var['`) that prevented the parser from
        // recovering the class structure.  Patch the cursor line to close
        // the array access, re-parse, and retry.
        let patched_classes_storage;
        let (raw_type, effective_classes) = match raw_type {
            Some(t) => (t, classes),
            None => {
                let patched = patch_array_access_at_cursor(content, position);
                if patched == content {
                    return vec![];
                }
                patched_classes_storage = self.parse_php(&patched);
                let patched_offset = Self::position_to_offset(&patched, position).unwrap_or(0);
                match self.resolve_variable_raw_type(
                    &ctx.var_name,
                    &patched,
                    patched_offset as usize,
                    &patched_classes_storage,
                    file_use_map,
                    file_namespace,
                ) {
                    Some(t) => (t, patched_classes_storage.as_slice()),
                    None => return vec![],
                }
            }
        };

        // If there are prefix keys (chained access), resolve through each
        // level of the shape to get the inner type.
        let effective_type = self.resolve_through_prefix_keys(&raw_type, &ctx.prefix_keys);
        let effective_type = match effective_type {
            Some(t) => t,
            None => return vec![],
        };

        // Expand type aliases before parsing as an array shape.
        // The raw type might be an alias name like `UserData` that
        // resolves to `array{name: string, email: string}`.
        // Uses `effective_classes` which may be the patched classes when
        // the original parse failed due to syntax errors.
        let class_loader = |name: &str| -> Option<ClassInfo> {
            self.resolve_class_name(name, effective_classes, file_use_map, file_namespace)
        };
        let effective_type =
            Self::expand_type_alias(&effective_type, effective_classes, &class_loader);

        // Parse the array shape entries.
        let entries = match docblock::parse_array_shape(&effective_type) {
            Some(e) => e,
            None => return vec![],
        };

        // Compute the text edit range that covers the partial key and any
        // trailing auto-inserted characters after the cursor.
        let (range, _) = self.compute_edit_range(ctx, content, position);

        // Build completion items, filtering by partial key.
        let quote = ctx.quote_char.unwrap_or('\'');
        let mut items = Vec::new();

        for (sort_idx, entry) in entries.iter().enumerate() {
            // Filter by partial key prefix.
            if !ctx.partial_key.is_empty()
                && !entry
                    .key
                    .to_lowercase()
                    .starts_with(&ctx.partial_key.to_lowercase())
            {
                continue;
            }

            let optional_marker = if entry.optional { "?" } else { "" };
            let detail = format!("{}{}: {}", entry.key, optional_marker, entry.value_type);

            // The new_text always produces the complete `key']` or `'key']`
            // fragment. The text_edit range is set to cover any existing
            // partial key text *and* any trailing auto-closed chars, so
            // accepting the completion replaces everything cleanly.
            let new_text = if ctx.quote_char.is_some() {
                format!("{}{}]", entry.key, quote)
            } else {
                format!("{}{}{}]", quote, entry.key, quote)
            };

            items.push(CompletionItem {
                label: entry.key.clone(),
                kind: Some(CompletionItemKind::FIELD),
                detail: Some(detail),
                filter_text: Some(entry.key.clone()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text })),
                sort_text: Some(format!("{:04}", sort_idx)),
                ..CompletionItem::default()
            });
        }

        items
    }

    /// Build completion items for `$_SERVER` superglobal keys.
    fn build_server_key_completions(
        &self,
        ctx: &ArrayKeyContext,
        content: &str,
        position: Position,
    ) -> Vec<CompletionItem> {
        let (range, _) = self.compute_edit_range(ctx, content, position);
        let quote = ctx.quote_char.unwrap_or('\'');
        let mut items = Vec::new();

        for (sort_idx, &(key, detail)) in SERVER_KEYS.iter().enumerate() {
            // Filter by partial key prefix.
            if !ctx.partial_key.is_empty()
                && !key
                    .to_lowercase()
                    .starts_with(&ctx.partial_key.to_lowercase())
            {
                continue;
            }

            let new_text = if ctx.quote_char.is_some() {
                format!("{}{}]", key, quote)
            } else {
                format!("{}{}{}]", quote, key, quote)
            };

            items.push(CompletionItem {
                label: key.to_string(),
                kind: Some(CompletionItemKind::FIELD),
                detail: Some(detail.to_string()),
                filter_text: Some(key.to_string()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text })),
                sort_text: Some(format!("{:04}", sort_idx)),
                ..CompletionItem::default()
            });
        }

        items
    }

    /// Compute the `TextEdit` range for an array key completion.
    ///
    /// The range starts at `key_start_col` (right after the opening quote
    /// or `[`) and extends past any trailing auto-inserted characters
    /// (closing quote + `]`) that the IDE may have inserted.
    ///
    /// Returns `(range, trailing_count)` where `trailing_count` is the
    /// number of characters consumed after the cursor.
    fn compute_edit_range(
        &self,
        ctx: &ArrayKeyContext,
        content: &str,
        position: Position,
    ) -> (Range, usize) {
        let lines: Vec<&str> = content.lines().collect();
        let line_idx = position.line as usize;
        let trailing_count = if line_idx < lines.len() {
            let line = lines[line_idx];
            let chars: Vec<char> = line.chars().collect();
            let cursor_col = position.character as usize;
            count_trailing_close_chars(&chars, cursor_col, ctx.quote_char)
        } else {
            0
        };

        let range = Range {
            start: Position {
                line: position.line,
                character: ctx.key_start_col,
            },
            end: Position {
                line: position.line,
                character: position.character + trailing_count as u32,
            },
        };

        (range, trailing_count)
    }

    /// Walk through `prefix_keys` to resolve the inner type of a nested
    /// array shape.
    ///
    /// Given a raw type like `"array{meta: array{page: int, total: int}}"` and
    /// prefix keys `["meta"]`, returns `Some("array{page: int, total: int}")`.
    fn resolve_through_prefix_keys(
        &self,
        raw_type: &str,
        prefix_keys: &[String],
    ) -> Option<String> {
        if prefix_keys.is_empty() {
            return Some(raw_type.to_string());
        }

        let mut current_type = raw_type.to_string();
        for key in prefix_keys {
            current_type = docblock::extract_array_shape_value_type(&current_type, key)?;
        }
        Some(current_type)
    }

    /// Resolve the raw (uncleaned) type annotation for a variable.
    ///
    /// Searches for `@var` and `@param` annotations, and also follows
    /// simple assignments from function/method calls to extract their
    /// `@return` type.
    ///
    /// Returns the raw type string (e.g. `"array{name: string, user: User}"`)
    /// or `None` if no type annotation is found.
    pub(crate) fn resolve_variable_raw_type(
        &self,
        var_name: &str,
        content: &str,
        cursor_offset: usize,
        classes: &[ClassInfo],
        file_use_map: &std::collections::HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Option<String> {
        // Try with the class that contains the cursor first, then fall
        // back to trying all classes so that top-level code still works.
        // 1. Direct @var / @param annotation on the variable.
        if let Some(raw) =
            docblock::find_iterable_raw_type_in_source(content, cursor_offset, var_name)
        {
            return Some(raw);
        }

        // 2. Try to find an assignment and resolve through it.
        //    Look for patterns like `$var = someFunction();` or
        //    `$var = $this->method();` and extract the return type.
        //
        //    First try with only the class that contains the cursor so
        //    that `$this->` resolves to the correct class even when
        //    there are multiple classes/interfaces in the file.
        if let Some(current) = Self::find_class_at_offset(classes, cursor_offset as u32) {
            let single = [current.clone()];
            if let Some(t) = self.resolve_raw_type_from_assignment(
                var_name,
                content,
                cursor_offset,
                &single,
                file_use_map,
                file_namespace,
            ) {
                return Some(t);
            }
        }

        // Fall back to all classes (handles top-level code and cases
        // where offset-based lookup doesn't match).
        self.resolve_raw_type_from_assignment(
            var_name,
            content,
            cursor_offset,
            classes,
            file_use_map,
            file_namespace,
        )
    }

    /// Follow a variable assignment to extract the raw return type of the
    /// RHS expression.
    ///
    /// Handles:
    /// - `$var = functionName(…);` → `@return` type of `functionName`
    /// - `$var = $this->methodName(…);` → return type of `methodName` on current class
    /// - `$var = ClassName::methodName(…);` → return type of static method
    /// - `$var = $this->propertyName;` → `@var` type of property
    /// - `$var = ['key' => value, …];` → synthesised `array{key: type, …}`
    /// - Incremental `$var['key'] = expr;` assignments after the initial
    ///   assignment are merged into the shape.
    /// - `$var[] = expr;` push-style assignments → synthesised `list<Type>`
    fn resolve_raw_type_from_assignment(
        &self,
        var_name: &str,
        content: &str,
        cursor_offset: usize,
        classes: &[ClassInfo],
        file_use_map: &std::collections::HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Option<String> {
        // Simple text-based scan: search backward for `$var = …;`
        let search_area = content.get(..cursor_offset)?;

        // Look for the most recent assignment to this variable.
        let assign_pattern = format!("{} = ", var_name);
        let assign_pos = search_area.rfind(&assign_pattern)?;
        let rhs_start = assign_pos + assign_pattern.len();

        // Extract the RHS up to the next `;`
        let remaining = &content[rhs_start..];
        let semi_pos = find_balanced_semicolon(remaining)?;
        let rhs_text = remaining[..semi_pos].trim();

        // Case 1: RHS is an array literal — `[…]` or `array(…)`.
        // Check this BEFORE the function-call case because `array(…)`
        // ends with `)` and would otherwise be mistaken for a call.
        // Also scan for incremental `$var['key'] = expr;` assignments
        // and push-style `$var[] = expr;` assignments.
        let base_entries = parse_array_literal_entries(rhs_text);

        // Scan for incremental `$var['key'] = expr;` assignments.
        let after_assign = assign_pos + assign_pattern.len() + semi_pos + 1; // past the `;`
        let incremental =
            collect_incremental_key_assignments(var_name, content, after_assign, cursor_offset);

        // Scan for push-style `$var[] = expr;` assignments.
        let push_types = collect_push_assignments(var_name, content, after_assign, cursor_offset);

        if base_entries.is_some() || !incremental.is_empty() || !push_types.is_empty() {
            let mut entries: Vec<(String, String)> = base_entries.unwrap_or_default();
            // Merge incremental assignments — later assignments for the
            // same key override earlier ones.
            for (k, v) in incremental {
                if let Some(existing) = entries.iter_mut().find(|(ek, _)| *ek == k) {
                    existing.1 = v;
                } else {
                    entries.push((k, v));
                }
            }
            // If there are string-keyed entries, prefer the array shape.
            if !entries.is_empty() {
                let shape_parts: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                return Some(format!("array{{{}}}", shape_parts.join(", ")));
            }
            // No string-keyed entries — try push-style list inference.
            if let Some(list_type) = build_list_type_from_push_types(&push_types) {
                return Some(list_type);
            }
        }

        // Case 2: RHS is a function call — `functionName(…)`
        // Case 3: RHS is a method call — `$this->methodName(…)` or `$obj->method(…)`
        // Case 4: RHS is a static call — `ClassName::methodName(…)`
        if rhs_text.ends_with(')') {
            return self.resolve_call_raw_return_type(
                rhs_text,
                content,
                cursor_offset,
                classes,
                file_use_map,
                file_namespace,
            );
        }

        // Case 5: RHS is a property access — `$this->propertyName`
        if let Some(prop) = rhs_text.strip_prefix("$this->")
            && prop.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            return self.resolve_property_raw_type(prop, classes, content, cursor_offset);
        }

        None
    }

    /// Resolve the raw return type of a function/method call expression.
    fn resolve_call_raw_return_type(
        &self,
        call_text: &str,
        content: &str,
        cursor_offset: usize,
        classes: &[ClassInfo],
        file_use_map: &std::collections::HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Option<String> {
        // Find the opening `(` at depth 0 to split name from args.
        let paren_pos = find_top_level_paren(call_text)?;
        let callee = &call_text[..paren_pos];

        // Method call: `$this->methodName`
        if let Some(method_name) = callee.strip_prefix("$this->") {
            // Find the current class that contains the cursor.
            let current_class =
                self.find_current_class_from_content(content, classes, cursor_offset)?;
            return self.get_method_raw_return_type(&current_class, method_name, classes);
        }

        // Static call: `ClassName::methodName`
        if let Some((class_part, method_part)) = callee.rsplit_once("::") {
            let class_info =
                self.resolve_class_name(class_part, classes, file_use_map, file_namespace)?;
            return self.get_method_raw_return_type(&class_info, method_part, classes);
        }

        // Standalone function call.
        let func_info = self.resolve_function_name(callee, file_use_map, file_namespace)?;
        func_info.return_type
    }

    /// Get the raw return type of a method, checking docblock `@return` first.
    fn get_method_raw_return_type(
        &self,
        class: &ClassInfo,
        method_name: &str,
        all_classes: &[ClassInfo],
    ) -> Option<String> {
        let merged =
            Self::resolve_class_with_inheritance(class, &|name: &str| -> Option<ClassInfo> {
                self.resolve_class_name(name, all_classes, &Default::default(), &None)
            });
        merged
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .and_then(|m| m.return_type.clone())
    }

    /// Get the raw type of a property from the class info.
    fn resolve_property_raw_type(
        &self,
        prop_name: &str,
        classes: &[ClassInfo],
        content: &str,
        cursor_offset: usize,
    ) -> Option<String> {
        let current_class =
            self.find_current_class_from_content(content, classes, cursor_offset)?;
        let merged = Self::resolve_class_with_inheritance(&current_class, &|name: &str| -> Option<
            ClassInfo,
        > {
            self.resolve_class_name(name, classes, &Default::default(), &None)
        });
        merged
            .properties
            .iter()
            .find(|p| p.name == prop_name)
            .and_then(|p| p.type_hint.clone())
    }

    /// Find the ClassInfo that contains the cursor offset based on the
    /// class list.  Uses byte-offset span matching so that when there
    /// are multiple classes/interfaces in the file, `$this->` resolves
    /// to the correct one.
    fn find_current_class_from_content(
        &self,
        content: &str,
        classes: &[ClassInfo],
        cursor_offset: usize,
    ) -> Option<ClassInfo> {
        // Prefer offset-based lookup so that the correct class is found
        // even when there are multiple classes/interfaces in the file.
        if let Some(c) = Self::find_class_at_offset(classes, cursor_offset as u32) {
            return Some(c.clone());
        }
        // If the cursor is inside a method call expression that was
        // extracted from a different offset (e.g. the RHS of an
        // assignment), try scanning all classes for one that contains
        // the method.  Use the content length as a rough heuristic —
        // fall back to the last class whose span starts before the
        // cursor offset.
        classes
            .iter()
            .rfind(|c| (c.start_offset as usize) < content.len())
            .cloned()
    }
}

/// Patch the content at the cursor line to close an unclosed array key
/// access so that the PHP parser can recover the surrounding class/function
/// structure.
///
/// Replaces patterns like `$var['` or `$var[` at the cursor line with
/// `$var[''];` (or `$var[];`) so the rest of the file parses correctly.
impl Backend {
    /// Expand a type string if it is a type alias defined on any class in
    /// `classes`.
    ///
    /// This is used by the array-key completion path which works with raw
    /// type strings rather than going through `type_hint_to_classes`.  When
    /// the type is not an alias, the original string is returned unchanged.
    ///
    /// Follows up to 10 levels of alias indirection to handle aliases that
    /// reference other aliases.  For imported type aliases (`from:Class:Name`),
    /// the `class_loader` is used to load the source class and resolve the
    /// original alias definition.
    fn expand_type_alias(
        type_str: &str,
        classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> String {
        let mut current = type_str.to_string();
        for _ in 0..10 {
            // Only bare identifiers can be aliases.
            if current.contains('<')
                || current.contains('{')
                || current.contains('|')
                || current.contains('&')
                || current.contains('\\')
                || current.contains('$')
            {
                break;
            }
            let mut found = false;
            for cls in classes {
                if let Some(def) = cls.type_aliases.get(current.as_str()) {
                    if let Some(import_ref) = def.strip_prefix("from:") {
                        // Imported alias: resolve from the source class.
                        if let Some(resolved) =
                            Self::expand_imported_type_alias(import_ref, classes, class_loader)
                        {
                            current = resolved;
                            found = true;
                        }
                        break;
                    }
                    current = def.clone();
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }
        current
    }

    /// Resolve an imported type alias reference (`ClassName:OriginalName`)
    /// by loading the source class and looking up the original alias.
    fn expand_imported_type_alias(
        import_ref: &str,
        classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let (source_class_name, original_name) = import_ref.split_once(':')?;
        let lookup = source_class_name
            .rsplit('\\')
            .next()
            .unwrap_or(source_class_name);
        let source_class = classes
            .iter()
            .find(|c| c.name == lookup)
            .cloned()
            .or_else(|| class_loader(source_class_name));
        let source_class = source_class?;
        let def = source_class.type_aliases.get(original_name)?;
        if def.starts_with("from:") {
            return None; // Don't follow nested imports.
        }
        Some(def.clone())
    }
}

fn patch_array_access_at_cursor(content: &str, position: Position) -> String {
    let line_idx = position.line as usize;
    let mut result = String::with_capacity(content.len() + 4);

    for (i, line) in content.lines().enumerate() {
        if i == line_idx {
            let trimmed = line.trim_end();
            // Detect the unclosed pattern and close it.
            // Order matters: check longer/more-specific patterns first so
            // that e.g. `['']` is not partially matched by `['`.
            if trimmed.ends_with("['']") || trimmed.ends_with("[\"\"]") {
                // Fully auto-closed quotes + bracket — just add semicolon.
                result.push_str(trimmed);
                result.push(';');
            } else if trimmed.ends_with("[']") || trimmed.ends_with("[\"]") {
                // Quote + auto-closed bracket without closing quote:
                //   `$data[']` → `$data[''];`
                //   `$data["]` → `$data[""];`
                let q = if trimmed.ends_with("[']") { '\'' } else { '"' };
                // Insert the closing quote before the `]`.
                let before_bracket = &trimmed[..trimmed.len() - 1];
                result.push_str(before_bracket);
                result.push(q);
                result.push_str("];");
            } else if trimmed.ends_with("['") || trimmed.ends_with("[\"") {
                result.push_str(trimmed);
                // Close the quote + bracket + semicolon
                let q = if trimmed.ends_with("['") { '\'' } else { '"' };
                result.push(q);
                result.push_str("];");
            } else if trimmed.ends_with("[]") {
                result.push_str(trimmed);
                result.push(';');
            } else if trimmed.ends_with('[') {
                result.push_str(trimmed);
                result.push_str("];");
            } else {
                // Nothing to patch
                result.push_str(line);
            }
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    // Remove trailing newline if the original didn't end with one.
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Count the number of trailing auto-inserted characters after the cursor.
///
/// When the IDE auto-closes brackets, the line may contain:
///   - `']` or `"]` after the cursor (2 chars) — when a quote was typed
///   - `]` after the cursor (1 char) — when only `[` was typed
///
/// This function looks at the characters starting at `cursor_col` and
/// returns how many should be consumed by the text edit range.
fn count_trailing_close_chars(
    chars: &[char],
    cursor_col: usize,
    quote_char: Option<char>,
) -> usize {
    if cursor_col >= chars.len() {
        return 0;
    }

    let remaining = &chars[cursor_col..];

    match quote_char {
        Some(q) => {
            // Expect closing quote + `]`
            if remaining.len() >= 2 && remaining[0] == q && remaining[1] == ']' {
                2
            } else if !remaining.is_empty() && remaining[0] == ']' {
                // Just a `]` even though we had a quote — still consume it
                1
            } else {
                0
            }
        }
        None => {
            // Expect just `]`
            if !remaining.is_empty() && remaining[0] == ']' {
                1
            } else {
                0
            }
        }
    }
}

/// Find the position of `;` in `s`, respecting `(…)`, `[…]`, `{…}`, and
/// string literal nesting.
fn find_balanced_semicolon(s: &str) -> Option<usize> {
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';

    for (i, ch) in s.char_indices() {
        // Handle string literals — skip everything inside quotes.
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
/// Parse an array literal expression into a list of `(key, value_type)` pairs.
///
/// Handles both `[…]` and `array(…)` syntax.  Only entries with explicit
/// string keys (`'key' => value` or `"key" => value`) are included;
/// positional entries are skipped since they don't produce useful key
/// completions.
///
/// The value type is inferred from the literal expression using simple
/// pattern matching (see [`infer_literal_type`]).
///
/// Returns `None` if the expression is not an array literal.
pub(super) fn parse_array_literal_entries(rhs: &str) -> Option<Vec<(String, String)>> {
    let inner = if rhs.starts_with('[') && rhs.ends_with(']') {
        &rhs[1..rhs.len() - 1]
    } else {
        // Handle `array(…)` syntax.
        let lower = rhs.to_ascii_lowercase();
        if lower.starts_with("array(") && rhs.ends_with(')') {
            &rhs[6..rhs.len() - 1]
        } else {
            return None;
        }
    };

    let inner = inner.trim();
    if inner.is_empty() {
        return Some(vec![]);
    }

    let parts = split_array_literal_elements(inner);
    let mut entries = Vec::new();

    for part in &parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Look for `=>` at depth 0 to split key from value.
        if let Some((key_part, value_part)) = split_on_fat_arrow(part) {
            let key_trimmed = key_part.trim();
            let value_trimmed = value_part.trim();

            // Only string-keyed entries produce useful completions.
            if let Some(key) = extract_string_literal(key_trimmed) {
                let value_type = infer_literal_type(value_trimmed);
                entries.push((key, value_type));
            }
        }
        // Positional entries (no `=>`) are skipped — numeric keys
        // are rarely useful for key completion.
    }

    Some(entries)
}

/// Split array literal elements on commas at depth 0, respecting
/// `(…)`, `[…]`, `{…}`, `<…>` nesting and quoted strings.
fn split_array_literal_elements(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';
    let mut start = 0;

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
            '(' | '[' => {
                if ch == '(' {
                    depth_paren += 1;
                } else {
                    depth_bracket += 1;
                }
            }
            ')' => depth_paren -= 1,
            ']' => depth_bracket -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        prev_char = ch;
    }
    let last = &s[start..];
    if !last.trim().is_empty() {
        parts.push(last);
    }
    parts
}

/// Split a single array element on `=>` at depth 0, respecting nesting
/// and quoted strings.
fn split_on_fat_arrow(s: &str) -> Option<(&str, &str)> {
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';
    let bytes = s.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;

        if in_single_quote {
            if ch == '\'' && prev_char != '\\' {
                in_single_quote = false;
            }
            prev_char = ch;
            i += 1;
            continue;
        }
        if in_double_quote {
            if ch == '"' && prev_char != '\\' {
                in_double_quote = false;
            }
            prev_char = ch;
            i += 1;
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
            '=' if depth_paren == 0
                && depth_bracket == 0
                && depth_brace == 0
                && i + 1 < bytes.len()
                && bytes[i + 1] == b'>' =>
            {
                return Some((&s[..i], &s[i + 2..]));
            }
            _ => {}
        }
        prev_char = ch;
        i += 1;
    }
    None
}

/// Extract the string content from a quoted literal (`'foo'` → `foo`,
/// `"bar"` → `bar`).  Returns `None` if the expression is not a simple
/// string literal.
fn extract_string_literal(s: &str) -> Option<String> {
    if ((s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')))
        && s.len() >= 2
    {
        return Some(s[1..s.len() - 1].to_string());
    }
    None
}

/// Infer a PHPStan-style type string from a literal PHP expression.
///
/// Uses simple pattern matching:
/// - `'…'` / `"…"` → `string`
/// - Integer literals → `int`
/// - Float literals → `float`
/// - `true` / `false` → `bool`
/// - `null` → `null`
/// - `new ClassName(…)` → `ClassName`
/// - `[…]` → `array`
/// - Anything else → `mixed`
fn infer_literal_type(expr: &str) -> String {
    let trimmed = expr.trim();

    if trimmed.is_empty() {
        return "mixed".to_string();
    }

    // String literals
    if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
    {
        return "string".to_string();
    }

    // Boolean
    let lower = trimmed.to_ascii_lowercase();
    if lower == "true" || lower == "false" {
        return "bool".to_string();
    }

    // Null
    if lower == "null" {
        return "null".to_string();
    }

    // `new ClassName(…)` — extract the class name.
    if let Some(rest) = trimmed.strip_prefix("new ") {
        let rest = rest.trim_start();
        // Class name ends at `(` or whitespace.
        let end = rest
            .find(|c: char| c == '(' || c.is_whitespace())
            .unwrap_or(rest.len());
        let class_name = &rest[..end];
        if !class_name.is_empty() {
            return class_name.trim_start_matches('\\').to_string();
        }
    }

    // Array literal
    if (trimmed.starts_with('[') && trimmed.ends_with(']'))
        || (lower.starts_with("array(") && trimmed.ends_with(')'))
    {
        return "array".to_string();
    }

    // Integer literal (possibly negative)
    let num_str = trimmed.strip_prefix('-').unwrap_or(trimmed);
    if !num_str.is_empty() && num_str.chars().all(|c| c.is_ascii_digit()) {
        return "int".to_string();
    }

    // Float literal (e.g. `1.5`, `-3.14`)
    if !num_str.is_empty() {
        let dot_parts: Vec<&str> = num_str.splitn(2, '.').collect();
        if dot_parts.len() == 2
            && !dot_parts[0].is_empty()
            && !dot_parts[1].is_empty()
            && dot_parts[0].chars().all(|c| c.is_ascii_digit())
            && dot_parts[1].chars().all(|c| c.is_ascii_digit())
        {
            return "float".to_string();
        }
    }

    "mixed".to_string()
}

/// Scan for incremental `$var['key'] = expr;` assignments in the content
/// between `start_offset` and `end_offset`.
///
/// Returns a list of `(key, inferred_type)` pairs.  Only string-keyed
/// assignments are collected; `$var[] = expr;` push-style assignments
/// are handled separately by [`collect_push_assignments`].
pub(super) fn collect_incremental_key_assignments(
    var_name: &str,
    content: &str,
    start_offset: usize,
    end_offset: usize,
) -> Vec<(String, String)> {
    let search_area = match content.get(start_offset..end_offset) {
        Some(s) => s,
        None => return vec![],
    };

    let mut entries = Vec::new();
    // Pattern: `$var['key'] = ` or `$var["key"] = `
    let prefix = format!("{}[", var_name);

    let mut pos = 0;
    while let Some(found) = search_area[pos..].find(&prefix) {
        let abs = pos + found;
        let after_bracket = abs + prefix.len();

        // Expect a quote character next.
        let rest = &search_area[after_bracket..];
        let quote_char = rest.chars().next();
        if !matches!(quote_char, Some('\'' | '"')) {
            pos = after_bracket;
            continue;
        }
        let q = quote_char.unwrap();

        // Find the closing quote.
        let after_quote = 1; // skip the opening quote
        let key_rest = &rest[after_quote..];
        let close_quote_pos = match key_rest.find(q) {
            Some(p) => p,
            None => {
                pos = after_bracket;
                continue;
            }
        };
        let key = &key_rest[..close_quote_pos];

        // After the closing quote, expect `] = `.
        let after_key = after_quote + close_quote_pos + 1; // past closing quote
        let remainder = &rest[after_key..];
        let remainder_trimmed = remainder.trim_start();
        if !remainder_trimmed.starts_with("] =") && !remainder_trimmed.starts_with("]=") {
            pos = after_bracket;
            continue;
        }

        // Find the `=` and extract the RHS up to `;`.
        let eq_offset_in_remainder = match remainder_trimmed.find('=') {
            Some(p) => p,
            None => {
                pos = after_bracket;
                continue;
            }
        };
        let rhs_start_in_remainder = eq_offset_in_remainder + 1;
        let rhs_and_rest = &remainder_trimmed[rhs_start_in_remainder..];

        // Find `;` respecting nesting.
        if let Some(semi) = find_balanced_semicolon(rhs_and_rest) {
            let value_expr = rhs_and_rest[..semi].trim();
            let inferred = infer_literal_type(value_expr);
            entries.push((key.to_string(), inferred));
        }

        pos = after_bracket;
    }

    entries
}

/// Scan for push-style `$var[] = expr;` assignments in the content
/// between `start_offset` and `end_offset`.
///
/// Returns a list of inferred type strings (one per push assignment).
/// Duplicate types are preserved so callers can deduplicate as needed.
///
/// # Example
///
/// ```php
/// $arr = [];
/// $arr[] = new User();       // → "User"
/// $arr[] = new AdminUser();  // → "AdminUser"
/// ```
///
/// The caller can combine these into `list<User|AdminUser>`.
pub(super) fn collect_push_assignments(
    var_name: &str,
    content: &str,
    start_offset: usize,
    end_offset: usize,
) -> Vec<String> {
    let search_area = match content.get(start_offset..end_offset) {
        Some(s) => s,
        None => return vec![],
    };

    let mut types = Vec::new();
    // Pattern: `$var[] = `
    let prefix = format!("{}[]", var_name);

    let mut pos = 0;
    while let Some(found) = search_area[pos..].find(&prefix) {
        let abs = pos + found;
        let after_brackets = abs + prefix.len();

        let rest = match search_area.get(after_brackets..) {
            Some(r) => r,
            None => break,
        };

        // After `$var[]`, expect ` = ` (with optional whitespace).
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('=') {
            pos = after_brackets;
            continue;
        }

        // Make sure it's `=` and not `==` or `===`.
        let after_eq = &trimmed[1..];
        if after_eq.starts_with('=') {
            pos = after_brackets;
            continue;
        }

        let rhs_and_rest = after_eq;

        // Find `;` respecting nesting.
        if let Some(semi) = find_balanced_semicolon(rhs_and_rest) {
            let value_expr = rhs_and_rest[..semi].trim();
            let inferred = infer_literal_type(value_expr);
            types.push(inferred);
        }

        pos = after_brackets;
    }

    types
}

/// Build a `list<Type>` type string from a collection of push-assignment
/// value types.
///
/// Deduplicates types and joins them with `|` inside the generic parameter.
/// Returns `None` if the input is empty or all types are `mixed`.
pub(super) fn build_list_type_from_push_types(types: &[String]) -> Option<String> {
    if types.is_empty() {
        return None;
    }

    // Deduplicate while preserving first-seen order.
    let mut seen = Vec::new();
    for t in types {
        if !seen.contains(t) {
            seen.push(t.clone());
        }
    }

    // If all types are `mixed`, don't synthesize a list type — it's not
    // useful for completion.
    if seen.iter().all(|t| t == "mixed") {
        return None;
    }

    let inner = seen.join("|");
    Some(format!("list<{}>", inner))
}

fn find_top_level_paren(s: &str) -> Option<usize> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_single_quote_empty() {
        // $config['
        let content = "<?php\n$config['";
        let pos = Position {
            line: 1,
            character: 9,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$config");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, Some('\''));
        assert_eq!(ctx.key_start_col, 9);
        assert!(ctx.prefix_keys.is_empty());
    }

    #[test]
    fn test_detect_single_quote_partial() {
        // $config['na
        let content = "<?php\n$config['na";
        let pos = Position {
            line: 1,
            character: 11,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$config");
        assert_eq!(ctx.partial_key, "na");
        assert_eq!(ctx.quote_char, Some('\''));
        assert_eq!(ctx.key_start_col, 9);
        assert!(ctx.prefix_keys.is_empty());
    }

    #[test]
    fn test_detect_double_quote_empty() {
        let content = "<?php\n$config[\"";
        let pos = Position {
            line: 1,
            character: 9,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$config");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, Some('"'));
        assert_eq!(ctx.key_start_col, 9);
        assert!(ctx.prefix_keys.is_empty());
    }

    #[test]
    fn test_detect_bracket_only() {
        // $config[
        let content = "<?php\n$config[";
        let pos = Position {
            line: 1,
            character: 8,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$config");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, None);
        assert_eq!(ctx.key_start_col, 8);
        assert!(ctx.prefix_keys.is_empty());
    }

    #[test]
    fn test_no_context_without_bracket() {
        let content = "<?php\n$config";
        let pos = Position {
            line: 1,
            character: 7,
        };
        assert!(detect_array_key_context(content, pos).is_none());
    }

    #[test]
    fn test_no_context_without_variable() {
        let content = "<?php\nfoo['";
        let pos = Position {
            line: 1,
            character: 5,
        };
        assert!(detect_array_key_context(content, pos).is_none());
    }

    #[test]
    fn test_detect_chained_single_key() {
        // $response['meta'][
        let content = "<?php\n$response['meta'][";
        let pos = Position {
            line: 1,
            character: 18,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$response");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, None);
        assert_eq!(ctx.prefix_keys, vec!["meta"]);
    }

    #[test]
    fn test_detect_chained_single_key_with_quote() {
        // $response['meta']['
        let content = "<?php\n$response['meta']['";
        let pos = Position {
            line: 1,
            character: 19,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$response");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, Some('\''));
        assert_eq!(ctx.prefix_keys, vec!["meta"]);
    }

    #[test]
    fn test_detect_chained_two_keys() {
        // $data['a']['b'][
        let content = "<?php\n$data['a']['b'][";
        let pos = Position {
            line: 1,
            character: 16,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$data");
        assert_eq!(ctx.prefix_keys, vec!["a", "b"]);
    }

    #[test]
    fn test_detect_autoclosed_bracket() {
        // $config[] — cursor between [ and ]
        let content = "<?php\n$config[]";
        let pos = Position {
            line: 1,
            character: 8,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$config");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, None);
        assert_eq!(ctx.key_start_col, 8);
    }

    #[test]
    fn test_detect_autoclosed_quote_bracket() {
        // $config[''] — cursor between the two quotes
        let content = "<?php\n$config['']";
        let pos = Position {
            line: 1,
            character: 9,
        };
        let ctx = detect_array_key_context(content, pos).unwrap();
        assert_eq!(ctx.var_name, "$config");
        assert_eq!(ctx.partial_key, "");
        assert_eq!(ctx.quote_char, Some('\''));
        assert_eq!(ctx.key_start_col, 9);
    }

    // ── parse_array_literal_entries ──────────────────────────────────

    #[test]
    fn test_parse_literal_simple() {
        let entries = parse_array_literal_entries("['name' => 'Alice', 'age' => 42]").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("name".to_string(), "string".to_string()));
        assert_eq!(entries[1], ("age".to_string(), "int".to_string()));
    }

    #[test]
    fn test_parse_literal_double_quoted_keys() {
        let entries =
            parse_array_literal_entries(r#"["host" => 'localhost', "port" => 8080]"#).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "host");
        assert_eq!(entries[1].0, "port");
    }

    #[test]
    fn test_parse_literal_array_syntax() {
        let entries =
            parse_array_literal_entries("array('driver' => 'mysql', 'port' => 3306)").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("driver".to_string(), "string".to_string()));
        assert_eq!(entries[1], ("port".to_string(), "int".to_string()));
    }

    #[test]
    fn test_parse_literal_empty_array() {
        let entries = parse_array_literal_entries("[]").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_literal_positional_entries_skipped() {
        // Positional entries (no =>) should be skipped
        let entries = parse_array_literal_entries("['key' => 1, 'value', 42]").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "key");
    }

    #[test]
    fn test_parse_literal_not_an_array() {
        assert!(parse_array_literal_entries("$this->getConfig()").is_none());
        assert!(parse_array_literal_entries("someFunction()").is_none());
        assert!(parse_array_literal_entries("'hello'").is_none());
    }

    #[test]
    fn test_parse_literal_nested_arrays() {
        let entries =
            parse_array_literal_entries("['db' => ['host' => 'x'], 'debug' => true]").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("db".to_string(), "array".to_string()));
        assert_eq!(entries[1], ("debug".to_string(), "bool".to_string()));
    }

    #[test]
    fn test_parse_literal_new_object_value() {
        let entries =
            parse_array_literal_entries("['user' => new User(), 'addr' => new Address()]").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("user".to_string(), "User".to_string()));
        assert_eq!(entries[1], ("addr".to_string(), "Address".to_string()));
    }

    #[test]
    fn test_parse_literal_value_with_commas_in_strings() {
        // Comma inside a string value should not split entries
        let entries =
            parse_array_literal_entries("['msg' => 'hello, world', 'code' => 200]").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "msg");
        assert_eq!(entries[0].1, "string");
        assert_eq!(entries[1].0, "code");
    }

    // ── infer_literal_type ──────────────────────────────────────────

    #[test]
    fn test_infer_string() {
        assert_eq!(infer_literal_type("'hello'"), "string");
        assert_eq!(infer_literal_type("\"world\""), "string");
    }

    #[test]
    fn test_infer_int() {
        assert_eq!(infer_literal_type("42"), "int");
        assert_eq!(infer_literal_type("0"), "int");
        assert_eq!(infer_literal_type("-1"), "int");
    }

    #[test]
    fn test_infer_float() {
        assert_eq!(infer_literal_type("3.14"), "float");
        assert_eq!(infer_literal_type("-0.5"), "float");
    }

    #[test]
    fn test_infer_bool() {
        assert_eq!(infer_literal_type("true"), "bool");
        assert_eq!(infer_literal_type("false"), "bool");
        assert_eq!(infer_literal_type("TRUE"), "bool");
    }

    #[test]
    fn test_infer_null() {
        assert_eq!(infer_literal_type("null"), "null");
        assert_eq!(infer_literal_type("NULL"), "null");
    }

    #[test]
    fn test_infer_new_object() {
        assert_eq!(infer_literal_type("new User()"), "User");
        assert_eq!(infer_literal_type("new \\App\\User()"), "App\\User");
        assert_eq!(infer_literal_type("new User"), "User");
    }

    #[test]
    fn test_infer_array() {
        assert_eq!(infer_literal_type("[]"), "array");
        assert_eq!(infer_literal_type("['a', 'b']"), "array");
        assert_eq!(infer_literal_type("array()"), "array");
    }

    #[test]
    fn test_infer_mixed() {
        assert_eq!(infer_literal_type("someFunction()"), "mixed");
        assert_eq!(infer_literal_type("$other"), "mixed");
        assert_eq!(infer_literal_type("self::VALUE"), "mixed");
    }

    // ── collect_incremental_key_assignments ──────────────────────────

    #[test]
    fn test_collect_incremental_basic() {
        let content = "$var = [];\n$var['name'] = 'Alice';\n$var['age'] = 30;\n$var['";
        // start after the first `;` (offset 10), end before cursor
        let entries = collect_incremental_key_assignments("$var", content, 10, content.len());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("name".to_string(), "string".to_string()));
        assert_eq!(entries[1], ("age".to_string(), "int".to_string()));
    }

    #[test]
    fn test_collect_incremental_double_quoted_keys() {
        let content = "$x = [];\n$x[\"host\"] = 'localhost';\n$x[\"port\"] = 80;\n";
        let entries = collect_incremental_key_assignments("$x", content, 8, content.len());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "host");
        assert_eq!(entries[1].0, "port");
    }

    #[test]
    fn test_collect_incremental_override() {
        let content = "$v = [];\n$v['k'] = 'hello';\n$v['k'] = 42;\n";
        let entries = collect_incremental_key_assignments("$v", content, 8, content.len());
        // Both assignments are collected; merging happens in the caller.
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("k".to_string(), "string".to_string()));
        assert_eq!(entries[1], ("k".to_string(), "int".to_string()));
    }

    #[test]
    fn test_collect_incremental_push_ignored() {
        // $var[] = expr should be ignored by string-key collector
        let content = "$v = [];\n$v[] = new User();\n$v['name'] = 'x';\n";
        let entries = collect_incremental_key_assignments("$v", content, 8, content.len());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "name");
    }

    #[test]
    fn test_collect_push_basic() {
        let content = "$v = [];\n$v[] = new User();\n$v[] = new AdminUser();\n";
        let types = collect_push_assignments("$v", content, 8, content.len());
        assert_eq!(types.len(), 2);
        assert_eq!(types[0], "User");
        assert_eq!(types[1], "AdminUser");
    }

    #[test]
    fn test_collect_push_string_literals() {
        let content = "$v = [];\n$v[] = 'hello';\n$v[] = 'world';\n";
        let types = collect_push_assignments("$v", content, 8, content.len());
        assert_eq!(types.len(), 2);
        assert_eq!(types[0], "string");
        assert_eq!(types[1], "string");
    }

    #[test]
    fn test_collect_push_skips_keyed() {
        // $var['key'] = expr should NOT be collected by push scanner
        let content = "$v = [];\n$v['name'] = 'x';\n$v[] = new User();\n";
        let types = collect_push_assignments("$v", content, 8, content.len());
        assert_eq!(types.len(), 1);
        assert_eq!(types[0], "User");
    }

    #[test]
    fn test_collect_push_empty_range() {
        let types = collect_push_assignments("$v", "", 0, 0);
        assert!(types.is_empty());
    }

    #[test]
    fn test_collect_push_no_double_equals() {
        // $var[] == expr should NOT be collected (comparison, not assignment)
        let content = "$v = [];\n$v[] == new User();\n";
        let types = collect_push_assignments("$v", content, 8, content.len());
        assert!(types.is_empty());
    }

    #[test]
    fn test_build_list_type_single() {
        let types = vec!["User".to_string()];
        assert_eq!(
            build_list_type_from_push_types(&types),
            Some("list<User>".to_string())
        );
    }

    #[test]
    fn test_build_list_type_union() {
        let types = vec!["User".to_string(), "AdminUser".to_string()];
        assert_eq!(
            build_list_type_from_push_types(&types),
            Some("list<User|AdminUser>".to_string())
        );
    }

    #[test]
    fn test_build_list_type_deduplicates() {
        let types = vec![
            "User".to_string(),
            "User".to_string(),
            "AdminUser".to_string(),
        ];
        assert_eq!(
            build_list_type_from_push_types(&types),
            Some("list<User|AdminUser>".to_string())
        );
    }

    #[test]
    fn test_build_list_type_empty() {
        let types: Vec<String> = vec![];
        assert_eq!(build_list_type_from_push_types(&types), None);
    }

    #[test]
    fn test_build_list_type_all_mixed() {
        let types = vec!["mixed".to_string(), "mixed".to_string()];
        assert_eq!(build_list_type_from_push_types(&types), None);
    }

    #[test]
    fn test_collect_incremental_new_objects() {
        let content = "$d = [];\n$d['user'] = new User();\n$d['addr'] = new Address();\n";
        let entries = collect_incremental_key_assignments("$d", content, 8, content.len());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("user".to_string(), "User".to_string()));
        assert_eq!(entries[1], ("addr".to_string(), "Address".to_string()));
    }

    #[test]
    fn test_collect_incremental_empty_range() {
        let content = "$v = [];";
        let entries = collect_incremental_key_assignments("$v", content, 8, 8);
        assert!(entries.is_empty());
    }
}
