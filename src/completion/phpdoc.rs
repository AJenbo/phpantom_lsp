//! PHPDoc tag completion.
//!
//! When the user types `@` inside a `/** … */` docblock, this module
//! provides context-aware tag suggestions.  The tags offered depend on
//! what PHP symbol follows the docblock:
//!
//! - **Function / Method**: `@param`, `@return`, `@throws`, …
//! - **Class / Interface / Trait / Enum**: `@property`, `@method`, `@mixin`, `@template`, …
//! - **Property**: `@var`, `@deprecated`, …
//! - **Constant**: `@var`, `@deprecated`, …
//! - **Unknown / file-level**: all tags
//!
//! When the symbol following the docblock can be parsed, completions are
//! pre-filled with concrete type and parameter information extracted from
//! the declaration (e.g. `@param string $name` instead of bare `@param`).

use tower_lsp::lsp_types::*;

// ─── Context Detection ─────────────────────────────────────────────────────

/// The kind of PHP symbol that follows the docblock containing the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocblockContext {
    /// The docblock precedes a function or method declaration.
    FunctionOrMethod,
    /// The docblock precedes a class, interface, trait, or enum.
    ClassLike,
    /// The docblock precedes a property declaration.
    Property,
    /// The docblock precedes a constant declaration.
    Constant,
    /// Context could not be determined (file-level, or symbol not recognized).
    Unknown,
}

/// Information extracted from the PHP symbol following the docblock.
///
/// Used to pre-fill completion items with concrete types and names.
#[derive(Debug, Clone, Default)]
pub struct SymbolInfo {
    /// Parameters: `(optional_type_hint, $name)`.
    pub params: Vec<(Option<String>, String)>,
    /// Return type hint (e.g. `"string"`, `"void"`, `"?int"`).
    pub return_type: Option<String>,
    /// Property / constant type hint.
    pub type_hint: Option<String>,
}

/// Check whether the cursor at `position` is inside a `/** … */` docblock
/// comment, and if so, return the partial tag prefix the user is typing
/// (e.g. `"@par"`, `"@"`, `"@phpstan-a"`).
///
/// Returns `None` if the cursor is not inside a docblock or is not at a
/// tag position (i.e. no `@` on the current line before the cursor).
pub fn extract_phpdoc_prefix(content: &str, position: Position) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let chars: Vec<char> = line.chars().collect();
    let col = (position.character as usize).min(chars.len());

    // Walk backwards from cursor to find `@`
    let mut i = col;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '-' || chars[i - 1] == '_') {
        i -= 1;
    }

    // Must be preceded by `@`
    if i == 0 || chars[i - 1] != '@' {
        return None;
    }
    // Include the `@`
    i -= 1;

    // The character before `@` (if any) must be whitespace or `*`
    // (typical docblock line prefix).  This prevents matching email
    // addresses or annotations in regular strings.
    if i > 0 {
        let prev = chars[i - 1];
        if !prev.is_whitespace() && prev != '*' {
            return None;
        }
    }

    let prefix: String = chars[i..col].iter().collect();

    // Now verify that we are actually inside a `/** … */` block.
    if !is_inside_docblock(content, position) {
        return None;
    }

    Some(prefix)
}

/// Returns `true` if the given position is inside a `/** … */` docblock.
///
/// Scans backwards from the cursor to find the nearest `/**` that has not
/// been closed by a matching `*/` before the cursor position.
pub fn is_inside_docblock(content: &str, position: Position) -> bool {
    // Convert position to byte offset for easier scanning
    let byte_offset = position_to_byte_offset(content, position);

    let before_cursor = &content[..byte_offset.min(content.len())];

    // Find the last `/**` before the cursor
    let last_open = before_cursor.rfind("/**");
    if last_open.is_none() {
        return false;
    }
    let open_pos = last_open.unwrap();

    // Check if there is a `*/` between the `/**` and the cursor
    // (which would mean the docblock is closed)
    let after_open = &before_cursor[open_pos + 3..];
    !after_open.contains("*/")
}

/// Determine what PHP symbol follows the docblock at the cursor position.
///
/// This looks at the content after the docblock's closing `*/` (or after
/// the current line if the docblock is still open) to identify the next
/// meaningful PHP token.
pub fn detect_context(content: &str, position: Position) -> DocblockContext {
    let remaining = get_text_after_docblock(content, position);
    classify_from_tokens(&remaining)
}

/// Extract information about the PHP symbol following the docblock.
///
/// Parses the declaration line(s) after `*/` to pull out parameter names,
/// type hints, return types, etc.
pub fn extract_symbol_info(content: &str, position: Position) -> SymbolInfo {
    let remaining = get_text_after_docblock(content, position);
    parse_symbol_info(&remaining)
}

/// Collect the names of parameters already documented with `@param` tags
/// in the current docblock above the cursor.
pub fn find_existing_param_tags(content: &str, position: Position) -> Vec<String> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    // Find the opening `/**`
    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let docblock_so_far = &before_cursor[open_pos..];

    let mut existing = Vec::new();
    for line in docblock_so_far.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim();
            // @param may have: Type $name desc  or just $name
            for word in rest.split_whitespace() {
                if word.starts_with('$') {
                    existing.push(word.to_string());
                    break;
                }
            }
        }
    }

    existing
}

/// Check whether `@return` is already documented in the current docblock.
fn has_existing_return_tag(content: &str, position: Position) -> bool {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return false,
    };

    let docblock_so_far = &before_cursor[open_pos..];
    docblock_so_far.lines().any(|line| {
        let trimmed = line.trim().trim_start_matches('*').trim();
        trimmed.starts_with("@return")
    })
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert a `Position` (line, character) to a byte offset in `content`.
pub fn position_to_byte_offset(content: &str, position: Position) -> usize {
    let mut offset = 0usize;
    for (line_idx, line) in content.lines().enumerate() {
        if line_idx == position.line as usize {
            offset += (position.character as usize).min(line.len());
            return offset;
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}

/// Get the text after the current docblock's closing `*/`.
///
/// If the docblock isn't closed yet, returns the text after the cursor
/// position (skipping lines that look like docblock continuation).
fn get_text_after_docblock(content: &str, position: Position) -> String {
    let byte_offset = position_to_byte_offset(content, position);
    let after_cursor = &content[byte_offset.min(content.len())..];

    if let Some(close_pos) = after_cursor.find("*/") {
        after_cursor[close_pos + 2..].to_string()
    } else {
        // Docblock not closed — return whatever follows
        after_cursor.to_string()
    }
}

/// Classify the PHP symbol from the first meaningful tokens.
fn classify_from_tokens(text: &str) -> DocblockContext {
    let mut tokens = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.starts_with("/**") {
            continue;
        }
        for word in trimmed.split_whitespace() {
            tokens.push(word.to_lowercase());
            if tokens.len() >= 6 {
                break;
            }
        }
        if tokens.len() >= 6 {
            break;
        }
    }

    if tokens.is_empty() {
        return DocblockContext::Unknown;
    }

    for token in &tokens {
        let t = token.as_str();
        match t {
            "function" => return DocblockContext::FunctionOrMethod,
            "class" | "interface" | "trait" | "enum" => return DocblockContext::ClassLike,
            "const" => return DocblockContext::Constant,
            "public" | "protected" | "private" | "static" | "readonly" | "abstract" | "final" => {
                continue;
            }
            _ => {
                if t.starts_with('$') {
                    return DocblockContext::Property;
                }
                if t.starts_with('?')
                    || t.starts_with('\\')
                    || t.chars().next().is_some_and(|c| c.is_uppercase())
                    || is_type_keyword(t)
                {
                    continue;
                }
                return DocblockContext::Unknown;
            }
        }
    }

    DocblockContext::Unknown
}

/// Check if a token is a PHP type keyword (used in property declarations).
fn is_type_keyword(token: &str) -> bool {
    matches!(
        token,
        "int"
            | "float"
            | "string"
            | "bool"
            | "array"
            | "object"
            | "callable"
            | "iterable"
            | "mixed"
            | "void"
            | "never"
            | "null"
            | "false"
            | "true"
            | "self"
            | "parent"
    )
}

/// Parse symbol info (params, return type, property type) from the
/// declaration text following the docblock.
fn parse_symbol_info(text: &str) -> SymbolInfo {
    let mut info = SymbolInfo::default();

    // Collect the declaration — may span multiple lines until `{` or `;`
    let mut decl = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.starts_with("/**") {
            continue;
        }
        decl.push(' ');
        decl.push_str(trimmed);
        if trimmed.contains('{') || trimmed.contains(';') {
            break;
        }
    }

    let decl = decl.trim();
    if decl.is_empty() {
        return info;
    }

    // Check if it's a function/method
    if let Some(func_pos) = find_keyword_pos(decl, "function") {
        let after_func = &decl[func_pos + 8..]; // "function" is 8 chars

        // Find the parameter list between ( and )
        if let Some(open_paren) = after_func.find('(') {
            let after_open = &after_func[open_paren + 1..];
            if let Some(close_paren) = find_matching_paren(after_open) {
                let params_str = &after_open[..close_paren];
                info.params = parse_params(params_str);

                // Extract return type: look for `: Type` after the closing paren
                let after_close = &after_open[close_paren + 1..];
                info.return_type = extract_return_type_from_decl(after_close);
            }
        }
    } else {
        // Property or constant — extract type hint
        info.type_hint = extract_property_type(decl);
    }

    info
}

/// Find the position of a keyword in the declaration, making sure it's
/// a whole word (not part of another identifier).
fn find_keyword_pos(decl: &str, keyword: &str) -> Option<usize> {
    let lower = decl.to_lowercase();
    let mut start = 0;
    while let Some(pos) = lower[start..].find(keyword) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0 || !decl.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_pos = abs_pos + keyword.len();
        let after_ok =
            after_pos >= decl.len() || !decl.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return Some(abs_pos);
        }
        start = abs_pos + keyword.len();
    }
    None
}

/// Find the position of the matching `)` for the first `(`, handling nesting.
fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Parse a comma-separated parameter list into `(type_hint, $name)` pairs.
fn parse_params(params_str: &str) -> Vec<(Option<String>, String)> {
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();

    // Split on commas, respecting nested parens/angle brackets
    for param in split_params_str(params_str) {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        // Each param looks like: [Type] [$name] [= default]
        // or: [Type] &$name, [Type] ...$name
        let tokens: Vec<&str> = param.split_whitespace().collect();

        let mut type_hint: Option<String> = None;
        let mut name: Option<String> = None;

        for token in &tokens {
            let t = *token;
            // Skip default value part
            if t == "=" {
                break;
            }
            if t.starts_with('$') || t.starts_with("&$") || t.starts_with("...$") {
                // This is the variable name
                let clean = t.trim_start_matches("...").trim_start_matches('&');
                name = Some(clean.to_string());
                break;
            }
            // Otherwise it's (part of) the type hint
            if type_hint.is_some() {
                // Union/intersection types with spaces shouldn't happen,
                // but handle it gracefully
                let existing = type_hint.unwrap();
                type_hint = Some(format!("{}{}", existing, t));
            } else {
                type_hint = Some(t.to_string());
            }
        }

        if let Some(n) = name {
            result.push((type_hint, n));
        }
    }

    result
}

/// Split a parameter string on commas, respecting nested parens and angle brackets.
fn split_params_str(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for c in s.chars() {
        match c {
            '(' | '<' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | '>' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }

    parts
}

/// Extract the return type from the portion after `)` in a function declaration.
///
/// Looks for `: Type` pattern.
fn extract_return_type_from_decl(after_close_paren: &str) -> Option<String> {
    let trimmed = after_close_paren.trim();
    let rest = trimmed.strip_prefix(':')?;
    let rest = rest.trim();

    // The return type is everything up to `{`, `;`, or end
    let end = rest.find(['{', ';']).unwrap_or(rest.len());

    let ret_type = rest[..end].trim();
    if ret_type.is_empty() {
        None
    } else {
        Some(ret_type.to_string())
    }
}

/// Extract the type hint from a property declaration.
///
/// Handles: `public string $name`, `protected ?int $count = 0`,
/// `private static array $cache`, `readonly Foo $bar`, etc.
fn extract_property_type(decl: &str) -> Option<String> {
    let tokens: Vec<&str> = decl.split_whitespace().collect();

    // Walk tokens: skip modifiers, the token before `$var` is the type
    let mut last_non_modifier: Option<String> = None;
    for token in &tokens {
        let t = *token;
        let lower = t.to_lowercase();

        if t.starts_with('$') {
            // The previous non-modifier token is the type
            return last_non_modifier;
        }

        // Skip `=` and everything after it
        if t == "=" || t == ";" {
            break;
        }

        match lower.as_str() {
            "public" | "protected" | "private" | "static" | "readonly" | "const" => {
                continue;
            }
            _ => {
                last_non_modifier = Some(t.to_string());
            }
        }
    }

    None
}

// ─── Tag Definitions ────────────────────────────────────────────────────────

/// A PHPDoc / PHPStan tag definition with metadata for completion.
struct TagDef {
    /// The tag text including `@` (e.g. `"@param"`).
    tag: &'static str,
    /// Brief one-line description shown in the completion detail.
    detail: &'static str,
    /// Display label showing usage format (e.g. `"@param Type $name"`).
    /// `None` means use `tag` as the label.
    label: Option<&'static str>,
}

/// Strip the leading `@` from a tag string.
///
/// The user has already typed `@` (or `@par…`) in the buffer and the LSP
/// client only replaces the *word* portion after `@`.  If the insert text
/// still contains `@`, the result is a doubled `@@tag`.
fn strip_at(s: &str) -> &str {
    s.strip_prefix('@').unwrap_or(s)
}

// ── Function / Method tags ──────────────────────────────────────────────────

const FUNCTION_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@param",
        detail: "Document a function parameter",
        label: Some("@param Type $name"),
    },
    TagDef {
        tag: "@return",
        detail: "Document the return type",
        label: Some("@return Type"),
    },
    TagDef {
        tag: "@throws",
        detail: "Document a thrown exception",
        label: Some("@throws ExceptionType"),
    },
    TagDef {
        tag: "@var",
        detail: "Document a variable type",
        label: Some("@var Type"),
    },
    TagDef {
        tag: "@inheritdoc",
        detail: "Inherit documentation from parent",
        label: None,
    },
];

// ── Class-like tags ─────────────────────────────────────────────────────────

const CLASS_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@property",
        detail: "Declare a magic property",
        label: Some("@property Type $name"),
    },
    TagDef {
        tag: "@method",
        detail: "Declare a magic method",
        label: Some("@method ReturnType name()"),
    },
    TagDef {
        tag: "@mixin",
        detail: "Declare a mixin class",
        label: Some("@mixin ClassName"),
    },
    TagDef {
        tag: "@template",
        detail: "Declare a generic type parameter",
        label: Some("@template T"),
    },
    TagDef {
        tag: "@extends",
        detail: "Specify generic parent class type",
        label: Some("@extends ClassName<Type>"),
    },
    TagDef {
        tag: "@implements",
        detail: "Specify generic interface type",
        label: Some("@implements InterfaceName<Type>"),
    },
    TagDef {
        tag: "@use",
        detail: "Specify generic trait type",
        label: Some("@use TraitName<Type>"),
    },
];

// ── Property tags ───────────────────────────────────────────────────────────

const PROPERTY_TAGS: &[TagDef] = &[TagDef {
    tag: "@var",
    detail: "Document the property type",
    label: Some("@var Type"),
}];

// ── Constant tags ───────────────────────────────────────────────────────────

const CONSTANT_TAGS: &[TagDef] = &[TagDef {
    tag: "@var",
    detail: "Document the constant type",
    label: Some("@var Type"),
}];

// ── General tags (available everywhere) ─────────────────────────────────────

const GENERAL_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@deprecated",
        detail: "Mark as deprecated",
        label: None,
    },
    TagDef {
        tag: "@see",
        detail: "Reference to related element",
        label: Some("@see ClassName::method()"),
    },
    TagDef {
        tag: "@since",
        detail: "Version when this was introduced",
        label: Some("@since 1.0.0"),
    },
    TagDef {
        tag: "@example",
        detail: "Reference to an example file",
        label: None,
    },
    TagDef {
        tag: "@link",
        detail: "URL to external documentation",
        label: Some("@link https://"),
    },
    TagDef {
        tag: "@internal",
        detail: "Mark as internal / not part of the public API",
        label: None,
    },
    TagDef {
        tag: "@todo",
        detail: "Document a to-do item",
        label: None,
    },
];

// ── PHPStan tags ────────────────────────────────────────────────────────────

const PHPSTAN_FUNCTION_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@phpstan-assert",
        detail: "PHPStan: assert parameter type after call",
        label: Some("@phpstan-assert Type $var"),
    },
    TagDef {
        tag: "@phpstan-assert-if-true",
        detail: "PHPStan: assert type when method returns true",
        label: Some("@phpstan-assert-if-true Type $var"),
    },
    TagDef {
        tag: "@phpstan-assert-if-false",
        detail: "PHPStan: assert type when method returns false",
        label: Some("@phpstan-assert-if-false Type $var"),
    },
    TagDef {
        tag: "@phpstan-self-out",
        detail: "PHPStan: narrow the type of $this after call",
        label: Some("@phpstan-self-out Type"),
    },
    TagDef {
        tag: "@phpstan-this-out",
        detail: "PHPStan: narrow the type of $this after call",
        label: Some("@phpstan-this-out Type"),
    },
    TagDef {
        tag: "@phpstan-ignore-next-line",
        detail: "PHPStan: suppress errors on the next line",
        label: None,
    },
    TagDef {
        tag: "@phpstan-type",
        detail: "PHPStan: define a local type alias",
        label: Some("@phpstan-type TypeName = Type"),
    },
    TagDef {
        tag: "@phpstan-import-type",
        detail: "PHPStan: import a type alias from another class",
        label: Some("@phpstan-import-type TypeName from ClassName"),
    },
];

const PHPSTAN_CLASS_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@phpstan-type",
        detail: "PHPStan: define a local type alias",
        label: Some("@phpstan-type TypeName = Type"),
    },
    TagDef {
        tag: "@phpstan-import-type",
        detail: "PHPStan: import a type alias from another class",
        label: Some("@phpstan-import-type TypeName from ClassName"),
    },
    TagDef {
        tag: "@phpstan-require-extends",
        detail: "PHPStan: require extending a specific class",
        label: Some("@phpstan-require-extends ClassName"),
    },
    TagDef {
        tag: "@phpstan-require-implements",
        detail: "PHPStan: require implementing a specific interface",
        label: Some("@phpstan-require-implements InterfaceName"),
    },
];

const PHPSTAN_PROPERTY_TAGS: &[TagDef] = &[];

// ─── Completion Builder ─────────────────────────────────────────────────────

/// Build completion items for PHPDoc tags based on context.
///
/// `content` is the full file text (used to extract symbol info and
/// detect already-documented parameters).
/// `prefix` is the partial tag the user has typed (e.g. `"@par"`, `"@"`).
/// `context` indicates what PHP symbol follows the docblock.
/// `position` is the cursor position (used to scan the docblock and the
/// following declaration).
///
/// Returns the list of matching `CompletionItem`s.
pub fn build_phpdoc_completions(
    content: &str,
    prefix: &str,
    context: DocblockContext,
    position: Position,
) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    // Extract symbol info for smart pre-filling
    let sym = extract_symbol_info(content, position);

    // Collect all applicable tag lists based on context
    let tag_lists: Vec<&[TagDef]> = match context {
        DocblockContext::FunctionOrMethod => {
            vec![FUNCTION_TAGS, GENERAL_TAGS, PHPSTAN_FUNCTION_TAGS]
        }
        DocblockContext::ClassLike => vec![CLASS_TAGS, GENERAL_TAGS, PHPSTAN_CLASS_TAGS],
        DocblockContext::Property => vec![PROPERTY_TAGS, GENERAL_TAGS, PHPSTAN_PROPERTY_TAGS],
        DocblockContext::Constant => vec![CONSTANT_TAGS, GENERAL_TAGS],
        DocblockContext::Unknown => vec![
            FUNCTION_TAGS,
            CLASS_TAGS,
            PROPERTY_TAGS,
            GENERAL_TAGS,
            PHPSTAN_FUNCTION_TAGS,
            PHPSTAN_CLASS_TAGS,
            PHPSTAN_PROPERTY_TAGS,
        ],
    };

    for tags in tag_lists {
        for def in tags {
            if !def.tag.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(def.tag) {
                continue;
            }

            // ── Smart items for @param ──────────────────────────────
            if def.tag == "@param" && !sym.params.is_empty() {
                let existing = find_existing_param_tags(content, position);
                let mut param_idx = 0u32;

                for (type_hint, name) in &sym.params {
                    // Skip params already documented
                    if existing.iter().any(|e| e == name) {
                        continue;
                    }

                    let (insert, label) = if let Some(th) = type_hint {
                        (
                            format!("param {} {}", th, name),
                            format!("@param {} {}", th, name),
                        )
                    } else {
                        (format!("param {}", name), format!("@param {}", name))
                    };

                    items.push(CompletionItem {
                        label,
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some(def.detail.to_string()),
                        insert_text: Some(insert),
                        filter_text: Some(def.tag.to_string()),
                        sort_text: Some(format!("0_{}_{:03}", def.tag.to_lowercase(), param_idx)),
                        ..CompletionItem::default()
                    });
                    param_idx += 1;
                }

                // If we emitted at least one smart @param, don't add the
                // generic fallback.  If all are already documented, fall
                // through to the generic one.
                if param_idx > 0 {
                    continue;
                }
            }

            // ── Smart item for @return ──────────────────────────────
            if def.tag == "@return" {
                if has_existing_return_tag(content, position) {
                    continue;
                }
                if let Some(ref ret) = sym.return_type
                    && ret != "void"
                {
                    items.push(CompletionItem {
                        label: format!("@return {}", ret),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some(def.detail.to_string()),
                        insert_text: Some(format!("return {}", ret)),
                        filter_text: Some(def.tag.to_string()),
                        sort_text: Some(format!("0_{}", def.tag.to_lowercase())),
                        ..CompletionItem::default()
                    });
                    continue;
                }
            }

            // ── Smart item for @var on properties / constants ───────
            if def.tag == "@var"
                && matches!(
                    context,
                    DocblockContext::Property | DocblockContext::Constant
                )
                && let Some(ref th) = sym.type_hint
            {
                items.push(CompletionItem {
                    label: format!("@var {}", th),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(def.detail.to_string()),
                    insert_text: Some(format!("var {}", th)),
                    filter_text: Some(def.tag.to_string()),
                    sort_text: Some(format!("0_{}", def.tag.to_lowercase())),
                    ..CompletionItem::default()
                });
                continue;
            }

            // ── Generic fallback ────────────────────────────────────
            let display_label = def.label.unwrap_or(def.tag);

            items.push(CompletionItem {
                label: display_label.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(def.detail.to_string()),
                insert_text: Some(strip_at(def.tag).to_string()),
                filter_text: Some(def.tag.to_string()),
                sort_text: Some(format!("0_{}", def.tag.to_lowercase())),
                ..CompletionItem::default()
            });
        }
    }

    items
}
