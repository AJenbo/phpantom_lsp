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
fn is_inside_docblock(content: &str, position: Position) -> bool {
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
fn position_to_byte_offset(content: &str, position: Position) -> usize {
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_inside_docblock ──────────────────────────────────────────

    #[test]
    fn inside_open_docblock() {
        let content = "<?php\n/**\n * @\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert!(is_inside_docblock(content, pos));
    }

    #[test]
    fn inside_closed_docblock() {
        let content = "<?php\n/**\n * @param string $x\n */\nfunction foo() {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert!(is_inside_docblock(content, pos));
    }

    #[test]
    fn outside_docblock_after_close() {
        let content = "<?php\n/**\n * @param string $x\n */\nfunction foo() {}\n";
        let pos = Position {
            line: 4,
            character: 5,
        };
        assert!(!is_inside_docblock(content, pos));
    }

    #[test]
    fn outside_docblock_before_open() {
        let content = "<?php\n\n/**\n * @param string $x\n */\n";
        let pos = Position {
            line: 1,
            character: 0,
        };
        assert!(!is_inside_docblock(content, pos));
    }

    #[test]
    fn not_inside_regular_comment() {
        let content = "<?php\n/* regular comment @param */\n";
        let pos = Position {
            line: 1,
            character: 22,
        };
        assert!(!is_inside_docblock(content, pos));
    }

    #[test]
    fn inside_multiline_docblock() {
        let content = "<?php\n/**\n * Some description.\n *\n * @\n */\n";
        let pos = Position {
            line: 4,
            character: 4,
        };
        assert!(is_inside_docblock(content, pos));
    }

    // ── extract_phpdoc_prefix ───────────────────────────────────────

    #[test]
    fn prefix_bare_at() {
        let content = "<?php\n/**\n * @\n */\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(extract_phpdoc_prefix(content, pos), Some("@".to_string()));
    }

    #[test]
    fn prefix_partial_tag() {
        let content = "<?php\n/**\n * @par\n */\n";
        let pos = Position {
            line: 2,
            character: 7,
        };
        assert_eq!(
            extract_phpdoc_prefix(content, pos),
            Some("@par".to_string())
        );
    }

    #[test]
    fn prefix_phpstan_tag() {
        let content = "<?php\n/**\n * @phpstan-a\n */\n";
        let pos = Position {
            line: 2,
            character: 14,
        };
        assert_eq!(
            extract_phpdoc_prefix(content, pos),
            Some("@phpstan-a".to_string())
        );
    }

    #[test]
    fn prefix_full_tag() {
        let content = "<?php\n/**\n * @return\n */\n";
        let pos = Position {
            line: 2,
            character: 10,
        };
        assert_eq!(
            extract_phpdoc_prefix(content, pos),
            Some("@return".to_string())
        );
    }

    #[test]
    fn no_prefix_outside_docblock() {
        let content = "<?php\n$email = 'user@example.com';\n";
        let pos = Position {
            line: 1,
            character: 25,
        };
        assert_eq!(extract_phpdoc_prefix(content, pos), None);
    }

    #[test]
    fn no_prefix_no_at_sign() {
        let content = "<?php\n/**\n * Just a description\n */\n";
        let pos = Position {
            line: 2,
            character: 20,
        };
        assert_eq!(extract_phpdoc_prefix(content, pos), None);
    }

    #[test]
    fn no_prefix_in_email_inside_docblock() {
        let content = "<?php\n/**\n * Contact user@example.com\n */\n";
        let pos = Position {
            line: 2,
            character: 25,
        };
        assert_eq!(extract_phpdoc_prefix(content, pos), None);
    }

    // ── detect_context ──────────────────────────────────────────────

    #[test]
    fn context_function() {
        let content = "<?php\n/**\n * @\n */\nfunction hello(): void {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(
            detect_context(content, pos),
            DocblockContext::FunctionOrMethod
        );
    }

    #[test]
    fn context_method() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public function bar(): void {}\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(
            detect_context(content, pos),
            DocblockContext::FunctionOrMethod
        );
    }

    #[test]
    fn context_static_method() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public static function bar(): void {}\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(
            detect_context(content, pos),
            DocblockContext::FunctionOrMethod
        );
    }

    #[test]
    fn context_class() {
        let content = "<?php\n/**\n * @\n */\nclass MyClass {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
    }

    #[test]
    fn context_abstract_class() {
        let content = "<?php\n/**\n * @\n */\nabstract class MyClass {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
    }

    #[test]
    fn context_final_class() {
        let content = "<?php\n/**\n * @\n */\nfinal class MyClass {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
    }

    #[test]
    fn context_interface() {
        let content = "<?php\n/**\n * @\n */\ninterface MyInterface {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
    }

    #[test]
    fn context_trait() {
        let content = "<?php\n/**\n * @\n */\ntrait MyTrait {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
    }

    #[test]
    fn context_enum() {
        let content = "<?php\n/**\n * @\n */\nenum Status {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
    }

    #[test]
    fn context_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public string $name;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Property);
    }

    #[test]
    fn context_typed_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    protected ?int $count = 0;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Property);
    }

    #[test]
    fn context_readonly_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public readonly string $name;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Property);
    }

    #[test]
    fn context_static_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    private static array $cache = [];\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Property);
    }

    #[test]
    fn context_constant() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    const MAX_SIZE = 100;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Constant);
    }

    #[test]
    fn context_visibility_constant() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public const VERSION = '1.0';\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Constant);
    }

    #[test]
    fn context_unknown_file_level() {
        let content = "<?php\n/**\n * @\n */\n\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        assert_eq!(detect_context(content, pos), DocblockContext::Unknown);
    }

    // ── extract_symbol_info ─────────────────────────────────────────

    #[test]
    fn symbol_info_function_params() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function greet(string $name, int $age): string {\n",
            "    return '';\n",
            "}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let info = extract_symbol_info(content, pos);

        assert_eq!(info.params.len(), 2);
        assert_eq!(
            info.params[0],
            (Some("string".to_string()), "$name".to_string())
        );
        assert_eq!(
            info.params[1],
            (Some("int".to_string()), "$age".to_string())
        );
        assert_eq!(info.return_type, Some("string".to_string()));
    }

    #[test]
    fn symbol_info_method_no_type_hints() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public function bar($x, $y) {}\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let info = extract_symbol_info(content, pos);

        assert_eq!(info.params.len(), 2);
        assert_eq!(info.params[0], (None, "$x".to_string()));
        assert_eq!(info.params[1], (None, "$y".to_string()));
        assert_eq!(info.return_type, None);
    }

    #[test]
    fn symbol_info_nullable_return() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function find(int $id): ?User {\n",
            "    return null;\n",
            "}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let info = extract_symbol_info(content, pos);
        assert_eq!(info.return_type, Some("?User".to_string()));
    }

    #[test]
    fn symbol_info_property_type() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public string $name;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let info = extract_symbol_info(content, pos);
        assert_eq!(info.type_hint, Some("string".to_string()));
    }

    #[test]
    fn symbol_info_nullable_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    protected ?int $count = 0;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let info = extract_symbol_info(content, pos);
        assert_eq!(info.type_hint, Some("?int".to_string()));
    }

    #[test]
    fn symbol_info_readonly_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public readonly string $name;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let info = extract_symbol_info(content, pos);
        assert_eq!(info.type_hint, Some("string".to_string()));
    }

    #[test]
    fn symbol_info_variadic_param() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function merge(array ...$arrays): array {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let info = extract_symbol_info(content, pos);
        assert_eq!(info.params.len(), 1);
        assert_eq!(
            info.params[0],
            (Some("array".to_string()), "$arrays".to_string())
        );
    }

    #[test]
    fn symbol_info_reference_param() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function swap(int &$a, int &$b): void {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let info = extract_symbol_info(content, pos);
        assert_eq!(info.params.len(), 2);
        assert_eq!(info.params[0], (Some("int".to_string()), "$a".to_string()));
        assert_eq!(info.params[1], (Some("int".to_string()), "$b".to_string()));
    }

    #[test]
    fn symbol_info_no_params() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function now(): DateTimeImmutable {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let info = extract_symbol_info(content, pos);
        assert!(info.params.is_empty());
        assert_eq!(info.return_type, Some("DateTimeImmutable".to_string()));
    }

    // ── find_existing_param_tags ─────────────────────────────────────

    #[test]
    fn finds_existing_param_tags() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @param string $name\n",
            " * @param int $age\n",
            " * @\n",
            " */\n",
            "function greet(string $name, int $age, bool $formal): string {}\n",
        );
        let pos = Position {
            line: 4,
            character: 4,
        };
        let existing = find_existing_param_tags(content, pos);
        assert_eq!(existing, vec!["$name", "$age"]);
    }

    #[test]
    fn no_existing_param_tags() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function greet(string $name): string {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let existing = find_existing_param_tags(content, pos);
        assert!(existing.is_empty());
    }

    // ── build_phpdoc_completions ────────────────────────────────────

    #[test]
    fn completions_bare_at_function() {
        let content = "<?php\n/**\n * @\n */\nfunction foo(): void {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

        // Should suggest function tags (some may have pre-filled info)
        assert!(
            labels
                .iter()
                .any(|l| l.starts_with("@param") || l == &"@param Type $name"),
            "Should suggest @param. Got: {:?}",
            labels
        );
        assert!(
            labels.iter().any(|l| l.starts_with("@return")),
            "Should suggest @return. Got: {:?}",
            labels
        );
        assert!(
            labels.iter().any(|l| l.starts_with("@throws")),
            "Should suggest @throws. Got: {:?}",
            labels
        );
        assert!(
            labels.iter().any(|l| l == &"@deprecated"),
            "Should suggest @deprecated"
        );
        assert!(
            labels.iter().any(|l| l.starts_with("@phpstan-assert")),
            "Should suggest @phpstan-assert"
        );

        // Should NOT suggest class-only tags
        assert!(
            !labels.iter().any(|l| l.starts_with("@property")),
            "Should NOT suggest @property in function context"
        );
        assert!(
            !labels.iter().any(|l| l.starts_with("@method")),
            "Should NOT suggest @method in function context"
        );
        assert!(
            !labels.iter().any(|l| l.starts_with("@mixin")),
            "Should NOT suggest @mixin in function context"
        );
    }

    #[test]
    fn completions_bare_at_class() {
        let content = "<?php\n/**\n * @\n */\nclass Foo {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::ClassLike, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(
            filter_texts.contains(&"@property"),
            "Should suggest @property"
        );
        assert!(filter_texts.contains(&"@method"), "Should suggest @method");
        assert!(filter_texts.contains(&"@mixin"), "Should suggest @mixin");
        assert!(
            filter_texts.contains(&"@template"),
            "Should suggest @template"
        );
        assert!(
            filter_texts.contains(&"@deprecated"),
            "Should suggest @deprecated"
        );

        // Should NOT suggest function-only tags
        assert!(
            !filter_texts.contains(&"@param"),
            "Should NOT suggest @param in class context"
        );
        assert!(
            !filter_texts.contains(&"@return"),
            "Should NOT suggest @return in class context"
        );
        assert!(
            !filter_texts.contains(&"@throws"),
            "Should NOT suggest @throws in class context"
        );
    }

    #[test]
    fn completions_bare_at_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public string $name;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::Property, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(filter_texts.contains(&"@var"), "Should suggest @var");
        assert!(
            filter_texts.contains(&"@deprecated"),
            "Should suggest @deprecated"
        );

        assert!(
            !filter_texts.contains(&"@param"),
            "Should NOT suggest @param in property context"
        );
        assert!(
            !filter_texts.contains(&"@return"),
            "Should NOT suggest @return in property context"
        );
        assert!(
            !filter_texts.contains(&"@method"),
            "Should NOT suggest @method in property context"
        );
    }

    #[test]
    fn completions_bare_at_constant() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    const X = 1;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::Constant, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(filter_texts.contains(&"@var"), "Should suggest @var");
        assert!(
            filter_texts.contains(&"@deprecated"),
            "Should suggest @deprecated"
        );

        assert!(
            !filter_texts.contains(&"@param"),
            "Should NOT suggest @param in constant context"
        );
    }

    #[test]
    fn completions_unknown_includes_all() {
        let content = "<?php\n/**\n * @\n */\n\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::Unknown, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(filter_texts.contains(&"@param"), "Should suggest @param");
        assert!(filter_texts.contains(&"@return"), "Should suggest @return");
        assert!(
            filter_texts.contains(&"@property"),
            "Should suggest @property"
        );
        assert!(filter_texts.contains(&"@method"), "Should suggest @method");
        assert!(filter_texts.contains(&"@var"), "Should suggest @var");
        assert!(
            filter_texts.contains(&"@deprecated"),
            "Should suggest @deprecated"
        );
    }

    #[test]
    fn completions_filtered_by_prefix() {
        let content = "<?php\n/**\n * @par\n */\nfunction foo(): void {}\n";
        let pos = Position {
            line: 2,
            character: 7,
        };
        let items =
            build_phpdoc_completions(content, "@par", DocblockContext::FunctionOrMethod, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(filter_texts.contains(&"@param"), "Should suggest @param");
        assert!(
            !filter_texts.contains(&"@return"),
            "Should NOT suggest @return for prefix @par"
        );
    }

    #[test]
    fn completions_phpstan_prefix() {
        let content = "<?php\n/**\n * @phpstan-a\n */\nfunction foo(): void {}\n";
        let pos = Position {
            line: 2,
            character: 14,
        };
        let items = build_phpdoc_completions(
            content,
            "@phpstan-a",
            DocblockContext::FunctionOrMethod,
            pos,
        );
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(
            filter_texts.contains(&"@phpstan-assert"),
            "Should suggest @phpstan-assert"
        );
        assert!(
            filter_texts.contains(&"@phpstan-assert-if-true"),
            "Should suggest @phpstan-assert-if-true"
        );
        assert!(
            filter_texts.contains(&"@phpstan-assert-if-false"),
            "Should suggest @phpstan-assert-if-false"
        );
        assert!(
            !filter_texts.contains(&"@phpstan-self-out"),
            "Should NOT suggest @phpstan-self-out for prefix @phpstan-a"
        );
    }

    #[test]
    fn completions_case_insensitive() {
        let content = "<?php\n/**\n * @PAR\n */\nfunction foo(): void {}\n";
        let pos = Position {
            line: 2,
            character: 7,
        };
        let items =
            build_phpdoc_completions(content, "@PAR", DocblockContext::FunctionOrMethod, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();

        assert!(
            filter_texts.contains(&"@param"),
            "Should match case-insensitively"
        );
    }

    #[test]
    fn completions_have_keyword_kind() {
        let content = "<?php\n/**\n * @\n */\nfunction foo(): void {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);
        for item in &items {
            assert_eq!(
                item.kind,
                Some(CompletionItemKind::KEYWORD),
                "PHPDoc tags should use KEYWORD kind"
            );
        }
    }

    #[test]
    fn completions_no_duplicates() {
        let content = "<?php\n/**\n * @\n */\n\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::Unknown, pos);
        let filter_texts: Vec<&str> = items
            .iter()
            .filter_map(|i| i.filter_text.as_deref())
            .collect();
        let unique: std::collections::HashSet<&&str> = filter_texts.iter().collect();
        assert_eq!(
            filter_texts.len(),
            unique.len(),
            "Should not have duplicate tags. Got: {:?}",
            filter_texts
        );
    }

    // ── Smart pre-fill tests ────────────────────────────────────────

    #[test]
    fn smart_param_completions_per_parameter() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function greet(string $name, int $age): string {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let param_items: Vec<_> = items
            .iter()
            .filter(|i| i.filter_text.as_deref() == Some("@param"))
            .collect();

        // Should have one item per parameter
        assert_eq!(
            param_items.len(),
            2,
            "Should have one @param per parameter. Got: {:?}",
            param_items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );

        assert_eq!(param_items[0].label, "@param string $name");
        assert_eq!(
            param_items[0].insert_text.as_deref(),
            Some("param string $name")
        );
        assert_eq!(param_items[1].label, "@param int $age");
        assert_eq!(
            param_items[1].insert_text.as_deref(),
            Some("param int $age")
        );
    }

    #[test]
    fn smart_param_skips_already_documented() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @param string $name\n",
            " * @\n",
            " */\n",
            "function greet(string $name, int $age): string {}\n",
        );
        let pos = Position {
            line: 3,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let param_items: Vec<_> = items
            .iter()
            .filter(|i| i.filter_text.as_deref() == Some("@param"))
            .collect();

        // $name is already documented, only $age should appear
        assert_eq!(
            param_items.len(),
            1,
            "Should only suggest undocumented params. Got: {:?}",
            param_items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );
        assert_eq!(param_items[0].label, "@param int $age");
    }

    #[test]
    fn smart_return_prefilled() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function getName(): string {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let return_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@return"));
        assert!(
            return_item.is_some(),
            "Should have @return item. Got: {:?}",
            items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );
        let r = return_item.unwrap();
        assert_eq!(r.label, "@return string");
        assert_eq!(r.insert_text.as_deref(), Some("return string"));
    }

    #[test]
    fn smart_return_void_uses_generic_label() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function doStuff(): void {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let return_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@return"));
        assert!(return_item.is_some(), "Should have @return item");
        let r = return_item.unwrap();
        // void return → generic label (no point pre-filling @return void)
        assert_eq!(r.label, "@return Type");
        assert_eq!(r.insert_text.as_deref(), Some("return"));
    }

    #[test]
    fn smart_return_skipped_when_already_documented() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @return string\n",
            " * @\n",
            " */\n",
            "function getName(): string {}\n",
        );
        let pos = Position {
            line: 3,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let return_items: Vec<_> = items
            .iter()
            .filter(|i| i.filter_text.as_deref() == Some("@return"))
            .collect();

        assert!(
            return_items.is_empty(),
            "Should NOT suggest @return when already documented. Got: {:?}",
            return_items.iter().map(|i| &i.label).collect::<Vec<_>>()
        );
    }

    #[test]
    fn smart_var_prefilled_for_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public string $name;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::Property, pos);

        let var_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@var"));
        assert!(var_item.is_some(), "Should have @var item");
        let v = var_item.unwrap();
        assert_eq!(v.label, "@var string");
        assert_eq!(v.insert_text.as_deref(), Some("var string"));
    }

    #[test]
    fn smart_var_nullable_property() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    protected ?int $count = 0;\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 8,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::Property, pos);

        let var_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@var"));
        assert!(var_item.is_some(), "Should have @var item");
        assert_eq!(var_item.unwrap().label, "@var ?int");
    }

    #[test]
    fn display_labels_for_generic_tags() {
        let content = "<?php\n/**\n * @\n */\nclass Foo {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::ClassLike, pos);

        let method_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@method"));
        assert!(method_item.is_some(), "Should have @method item");
        assert_eq!(
            method_item.unwrap().label,
            "@method ReturnType name()",
            "@method should show usage pattern as label"
        );

        let template_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@template"));
        assert!(template_item.is_some(), "Should have @template item");
        assert_eq!(
            template_item.unwrap().label,
            "@template T",
            "@template should show usage pattern as label"
        );
    }

    #[test]
    fn display_labels_for_general_tags() {
        let content = "<?php\n/**\n * @\n */\nfunction foo(): void {}\n";
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let throws_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@throws"));
        assert!(throws_item.is_some(), "Should have @throws item");
        assert_eq!(throws_item.unwrap().label, "@throws ExceptionType");

        // Tags with no special format should use tag as label
        let deprecated_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@deprecated"));
        assert!(deprecated_item.is_some(), "Should have @deprecated item");
        assert_eq!(deprecated_item.unwrap().label, "@deprecated");
    }

    #[test]
    fn smart_param_untyped_params() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function process($data, $options) {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let param_items: Vec<_> = items
            .iter()
            .filter(|i| i.filter_text.as_deref() == Some("@param"))
            .collect();

        assert_eq!(param_items.len(), 2);
        assert_eq!(param_items[0].label, "@param $data");
        assert_eq!(param_items[0].insert_text.as_deref(), Some("param $data"));
        assert_eq!(param_items[1].label, "@param $options");
    }

    #[test]
    fn smart_return_nullable() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @\n",
            " */\n",
            "function find(): ?User {}\n",
        );
        let pos = Position {
            line: 2,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let return_item = items
            .iter()
            .find(|i| i.filter_text.as_deref() == Some("@return"));
        assert!(return_item.is_some());
        assert_eq!(return_item.unwrap().label, "@return ?User");
    }

    #[test]
    fn all_params_documented_falls_back_to_generic() {
        let content = concat!(
            "<?php\n",
            "/**\n",
            " * @param string $name\n",
            " * @\n",
            " */\n",
            "function greet(string $name): string {}\n",
        );
        let pos = Position {
            line: 3,
            character: 4,
        };
        let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

        let param_items: Vec<_> = items
            .iter()
            .filter(|i| i.filter_text.as_deref() == Some("@param"))
            .collect();

        // All params documented → falls back to generic @param
        assert_eq!(param_items.len(), 1);
        assert_eq!(param_items[0].label, "@param Type $name");
        assert_eq!(param_items[0].insert_text.as_deref(), Some("param"));
    }
}
