//! Hover formatting helpers.
//!
//! Pure functions that take data types and return formatted strings or
//! `Hover` values.  These have no dependency on `Backend` and are used
//! by the dispatch/resolution code in `hover/mod.rs`.

use tower_lsp::lsp_types::*;

use crate::symbol_map::SymbolSpan;
use crate::types::*;
use crate::util::offset_to_position;

/// Convert a `SymbolSpan`'s byte offsets to an LSP `Range`.
pub(super) fn symbol_span_to_range(content: &str, symbol: &SymbolSpan) -> Range {
    Range {
        start: offset_to_position(content, symbol.start as usize),
        end: offset_to_position(content, symbol.end as usize),
    }
}

/// Create a `Hover` with Markdown content.
pub(super) fn make_hover(contents: String) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: contents,
        }),
        range: None,
    }
}

/// Format a deprecation message as a Markdown line for hover output.
///
/// Returns `"🪦 **deprecated**"` when the message is empty, or
/// `"🪦 **deprecated** Use foo() instead."` when a message is present.
pub(super) fn format_deprecation_line(msg: &str) -> String {
    if msg.is_empty() {
        "🪦 **deprecated**".to_string()
    } else {
        format!("🪦 **deprecated** {}", msg)
    }
}

/// Format a visibility keyword.
pub(super) fn format_visibility(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "public ",
        Visibility::Protected => "protected ",
        Visibility::Private => "private ",
    }
}

/// Format a parameter list using native PHP type hints only.
///
/// Used inside `<?php` code blocks so the displayed declaration matches
/// what the actual PHP source code looks like.
pub(super) fn format_native_params(params: &[ParameterInfo]) -> String {
    format_params_inner(params, true)
}

/// Shared implementation for parameter formatting.
///
/// When `use_native` is true, uses `native_type_hint` (falling back to
/// `type_hint` when no native hint is stored — e.g. for virtual members
/// synthesised from docblocks).  Otherwise uses `type_hint` (effective).
fn format_params_inner(params: &[ParameterInfo], use_native: bool) -> String {
    params
        .iter()
        .map(|p| {
            let mut parts = Vec::new();
            let hint = if use_native {
                p.native_type_hint.as_ref()
            } else {
                p.type_hint.as_ref()
            };
            if let Some(th) = hint {
                parts.push(th.clone());
            }
            if p.is_variadic {
                parts.push(format!("...{}", p.name));
            } else if p.is_reference {
                parts.push(format!("&{}", p.name));
            } else {
                parts.push(p.name.clone());
            }
            let param_str = parts.join(" ");
            if !p.is_required && !p.is_variadic {
                if let Some(ref dv) = p.default_value {
                    format!("{} = {}", param_str, dv)
                } else {
                    format!("{} = ...", param_str)
                }
            } else {
                param_str
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build a `namespace Foo;\n` line for use inside PHP code blocks.
/// Returns an empty string when the namespace is global (None).
pub(super) fn namespace_line(namespace: &Option<String>) -> String {
    if let Some(ns) = namespace
        && !ns.is_empty()
        && !ns.starts_with("___")
    {
        format!("namespace {};\n", ns)
    } else {
        String::new()
    }
}

/// Build a `@var` docblock annotation when the effective type differs from
/// the native type.  Returns `None` when they are identical or when there
/// is no effective type.
pub(super) fn build_var_annotation(
    effective: Option<&str>,
    native: Option<&str>,
) -> Option<String> {
    let eff = effective?;
    // When there is no native type hint, `mixed` is the implicit type
    // in PHP — showing `@var mixed` would be noise.
    if native.is_none() && eff == "mixed" {
        return None;
    }
    if native.is_some_and(|n| types_equivalent(n, eff)) {
        return None;
    }
    Some(format!("@var {}", shorten_type_string(eff)))
}

/// Build a readable markdown section showing parameter and return type
/// information.
///
/// Produces output like:
///
/// ```text
/// **$callback** `(callable(TItem): TReturn)|null`
///     Callback function to run for each element.
/// **$array** `array<string|int, TItem>`
///     An array to run through the callback function.
/// **$arrays** `array<string|int, TItem>` ...
/// **return** `array<string|int, TReturn>`
///     an array containing all the elements of arr1 ...
/// ```
///
/// A parameter entry is emitted when:
///   - the effective type differs from the native type, OR
///   - the parameter has a description.
///
/// When types are the same, the type is shown alongside the description.
/// When types differ but there is no description, only the type is shown.
///
/// A return entry is emitted when:
///   - the effective return type differs from the native return type, OR
///   - there is a return description.
///
/// Returns `None` when there is nothing to show.
pub(super) fn build_param_return_section(
    params: &[ParameterInfo],
    effective_return: Option<&str>,
    native_return: Option<&str>,
    return_description: Option<&str>,
) -> Option<String> {
    let mut entries = Vec::new();

    for p in params {
        let type_differs = match (p.type_hint.as_deref(), p.native_type_hint.as_deref()) {
            (Some(eff), Some(nat)) => !types_equivalent(eff, nat),
            (Some(eff), None) => eff != "mixed",
            _ => false,
        };
        let has_desc = p.description.as_ref().is_some_and(|d| !d.is_empty());

        if !type_differs && !has_desc {
            continue;
        }

        let mut entry = format!("**{}**", p.name);
        if type_differs {
            if let Some(ref eff) = p.type_hint {
                entry.push_str(&format!(" `{}`", shorten_type_string(eff)));
            }
            if p.is_variadic {
                entry.push_str(" ...");
            }
            if has_desc {
                entry.push_str("  \n\u{00a0}\u{00a0}\u{00a0}\u{00a0}");
                entry.push_str(p.description.as_deref().unwrap());
            }
        } else if has_desc {
            // Types match — show description directly after the name.
            entry.push(' ');
            entry.push_str(p.description.as_deref().unwrap());
        }
        entries.push(entry);
    }

    // return entry
    let ret_type_differs = match (effective_return, native_return) {
        (Some(eff), Some(nat)) => !types_equivalent(eff, nat),
        (Some(eff), None) => eff != "mixed",
        _ => false,
    };
    let has_ret_desc = return_description.is_some_and(|d| !d.is_empty());

    if ret_type_differs || has_ret_desc {
        let mut entry = String::from("**return**");
        if ret_type_differs {
            if let Some(eff) = effective_return {
                entry.push_str(&format!(" `{}`", shorten_type_string(eff)));
            }
            if has_ret_desc {
                entry.push_str("  \n\u{00a0}\u{00a0}\u{00a0}\u{00a0}");
                entry.push_str(return_description.unwrap());
            }
        } else if has_ret_desc {
            entry.push(' ');
            entry.push_str(return_description.unwrap());
        }
        entries.push(entry);
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries.join("\n\n"))
    }
}

/// Build a PHP code block wrapping a member inside its owning class.
///
/// Produces a fenced `php` block containing:
///
///   - `<?php`
///   - `namespace Foo;` (omitted when global)
///   - `class ShortName {`
///   - `    public string $name;`
///   - `}`
pub(super) fn build_class_member_block(
    owner_name: &str,
    owner_namespace: &Option<String>,
    kind_keyword: &str,
    name_suffix: &str,
    member_line: &str,
) -> String {
    let mut body = String::new();
    let ns_line = namespace_line(owner_namespace);
    body.push_str("```php\n<?php\n");
    body.push_str(&ns_line);
    body.push_str(kind_keyword);
    body.push(' ');
    body.push_str(owner_name);
    body.push_str(name_suffix);
    body.push_str(" {\n    ");
    body.push_str(member_line);
    body.push_str("\n}\n```");
    body
}

/// Return the PHP keyword for a class-like owner.
///
/// Produces `"class"`, `"interface"`, `"trait"`, or `"enum"`.
pub(super) fn owner_kind_keyword(owner: &ClassInfo) -> &'static str {
    match owner.kind {
        ClassLikeKind::Interface => "interface",
        ClassLikeKind::Trait => "trait",
        ClassLikeKind::Enum => "enum",
        _ => "class",
    }
}

/// Return the suffix after the owner name for backed enums (e.g. `": string"`).
///
/// Returns an empty string for non-enums and unit enums.
pub(super) fn owner_name_suffix(owner: &ClassInfo) -> String {
    if let Some(ref bt) = owner.backed_type {
        format!(": {}", bt)
    } else {
        String::new()
    }
}

/// Build a PHP code block wrapping a member inside its owning class,
/// with an optional single-line `/** @var ... */` annotation above it.
///
/// Used for properties where the effective (docblock) type differs from
/// the native PHP type hint.
pub(super) fn build_class_member_block_with_var(
    owner_name: &str,
    owner_namespace: &Option<String>,
    kind_keyword: &str,
    name_suffix: &str,
    var_annotation: &Option<String>,
    member_line: &str,
) -> String {
    let mut body = String::new();
    let ns_line = namespace_line(owner_namespace);
    body.push_str("```php\n<?php\n");
    body.push_str(&ns_line);
    body.push_str(kind_keyword);
    body.push(' ');
    body.push_str(owner_name);
    body.push_str(name_suffix);
    body.push_str(" {\n");
    if let Some(annotation) = var_annotation {
        body.push_str("    /** ");
        body.push_str(annotation);
        body.push_str(" */\n");
    }
    body.push_str("    ");
    body.push_str(member_line);
    body.push_str("\n}\n```");
    body
}

/// Build hover content for a standalone function.
pub(crate) fn hover_for_function(
    func: &FunctionInfo,
    resolved_see: Option<&[ResolvedSeeRef]>,
) -> Hover {
    let native_params = format_native_params(&func.parameters);

    // Use native return type in the code block.
    let native_ret = func
        .native_return_type
        .as_ref()
        .map(|r| format!(": {}", r))
        .unwrap_or_default();

    let signature = format!("function {}({}){}", func.name, native_params, native_ret);
    let ns_line = namespace_line(&func.namespace);

    let mut lines = Vec::new();

    if let Some(ref desc) = func.description {
        lines.push(desc.clone());
    }

    if let Some(ref msg) = func.deprecation_message {
        lines.push(format_deprecation_line(msg));
    }

    for url in &func.links {
        lines.push(format!("[{}]({})", url, url));
    }

    if let Some(refs) = resolved_see {
        format_see_refs(refs, &func.links, &mut lines);
    } else {
        // Fallback: render raw @see refs without location links.
        let unresolved: Vec<ResolvedSeeRef> = func
            .see_refs
            .iter()
            .map(|raw| ResolvedSeeRef {
                raw: raw.clone(),
                location_uri: None,
            })
            .collect();
        format_see_refs(&unresolved, &func.links, &mut lines);
    }

    // Build the readable param/return section as markdown.
    if let Some(section) = build_param_return_section(
        &func.parameters,
        func.return_type.as_deref(),
        func.native_return_type.as_deref(),
        func.return_description.as_deref(),
    ) {
        lines.push(section);
    }

    // Build a clean code block with just the signature.
    let code = format!("```php\n<?php\n{}{};\n```", ns_line, signature);
    lines.push(code);

    make_hover(lines.join("\n\n"))
}

/// A `@see` reference that has been resolved to an optional file location.
///
/// When `location_uri` is `Some`, the symbol name is rendered as a
/// clickable link that opens the target file at the definition site.
pub(crate) struct ResolvedSeeRef {
    /// The raw text after `@see` (e.g. `"UnsetDemo"`,
    /// `"MyClass::method() Use this instead"`,
    /// `"https://example.com/docs"`).
    pub raw: String,
    /// File URI with line fragment (e.g. `"file:///path/to/file.php#L42"`)
    /// for symbol references that could be resolved to a definition site.
    /// `None` for URLs or unresolvable symbols.
    pub location_uri: Option<String>,
}

/// Format `@see` references as hover lines.
///
/// URL references are rendered as clickable markdown links.
/// Symbol references with a resolved location are rendered as clickable
/// file links that jump to the definition site.  Unresolved symbols are
/// rendered as inline code.
/// Each entry becomes a separate line in the hover popup.
pub(super) fn format_see_refs(
    see_refs: &[ResolvedSeeRef],
    existing_links: &[String],
    lines: &mut Vec<String>,
) {
    for entry in see_refs {
        // Split into the first token (symbol or URL) and optional description.
        let (target, description) = match entry.raw.split_once(|c: char| c.is_whitespace()) {
            Some((t, d)) => (t.trim(), Some(d.trim())),
            None => (entry.raw.as_str(), None),
        };

        let desc_suffix = description.map(|d| format!(" {}", d)).unwrap_or_default();

        if target.starts_with("http://") || target.starts_with("https://") {
            // Skip URL references that already appear as @link entries.
            if existing_links.iter().any(|l| l == target) {
                continue;
            }
            // URL reference — render as a clickable markdown link,
            // same style as @link.
            lines.push(format!("[{}]({}){}", target, target, desc_suffix));
        } else if let Some(ref uri) = entry.location_uri {
            // Symbol reference with resolved location — render as a
            // clickable link that opens the file at the definition line.
            lines.push(format!("[`{}`]({}){}", target, uri, desc_suffix));
        } else {
            // Symbol reference without a known location — inline code.
            lines.push(format!("`{}`{}", target, desc_suffix));
        }
    }
}

/// Extract the trailing description from a `@var` tag line.
///
/// Handles formats like:
///   - `/** @var list<Pen> The batches */`       → `Some("The batches")`
///   - `/** @var list<Pen> $batch The batches */` → `Some("The batches")`
///   - `/** @var list<Pen> */`                    → `None`
///
/// Only looks at the `@var` line itself.  For multi-line docblocks where
/// the description precedes the `@var` tag, use `extract_docblock_description`.
pub(crate) fn extract_var_description(docblock: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@var") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                return None;
            }
            // Skip past the type token (respecting `<…>` nesting).
            let after_type = skip_type_token(rest);
            let after_type = after_type.trim_start();
            if after_type.is_empty() {
                return None;
            }
            // Skip an optional `$variable` name.
            let after_var = if after_type.starts_with('$') {
                after_type
                    .split_once(|c: char| c.is_whitespace())
                    .map(|(_, rest)| rest.trim_start())
                    .unwrap_or("")
            } else {
                after_type
            };
            if after_var.is_empty() {
                return None;
            }
            return Some(after_var.to_string());
        }
    }
    None
}

/// Skip past a type token in a docblock string, respecting `<…>` nesting.
///
/// Returns the remainder of the string after the type token.
fn skip_type_token(s: &str) -> &str {
    let mut depth = 0i32;
    let mut end = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth -= 1,
            _ if c.is_whitespace() && depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
        end = i + c.len_utf8();
    }
    &s[end..]
}

/// Extract the human-readable description text from a raw docblock string.
///
/// Strips the `/**` and `*/` delimiters, leading `*` characters, and all
/// `@tag` lines. Returns `None` if no description text remains.
pub(crate) fn extract_docblock_description(docblock: Option<&str>) -> Option<String> {
    let raw = docblock?;
    let inner = raw
        .trim()
        .strip_prefix("/**")
        .unwrap_or(raw)
        .strip_suffix("*/")
        .unwrap_or(raw);

    let mut lines = Vec::new();
    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        // Skip empty lines at the very start
        if lines.is_empty() && trimmed.is_empty() {
            continue;
        }

        // Stop at the first @tag
        if trimmed.starts_with('@') {
            break;
        }

        lines.push(trimmed.to_string());
    }

    // Trim trailing empty lines
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        None
    } else {
        let result = lines.join("\n");
        let result = result
            .replace("<b>", "**")
            .replace("</b>", "**")
            .replace("<i>", "*")
            .replace("</i>", "*")
            .replace("<code>", "`")
            .replace("</code>", "`")
            .replace("<br />", "\n")
            .replace("<br/>", "\n")
            .replace("<br>", "\n")
            .replace("<p>", "\n\n")
            .replace("</p>", "");
        Some(result)
    }
}

/// Shorten all namespace-qualified class names in a type string to their
/// short (unqualified) form.
///
/// Handles union (`|`), intersection (`&`), nullable (`?`), and generic
/// (`<`, `,`) type syntax.  For example:
///
///   - `"App\\Models\\User"` → `"User"`
///   - `"list<App\\Models\\User>"` → `"list<User>"`
///   - `"App\\Foo|App\\Bar|null"` → `"Foo|Bar|null"`
pub(crate) fn shorten_type_string(ty: &str) -> String {
    let mut result = String::with_capacity(ty.len());
    let mut segment_start = 0;
    let bytes = ty.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if matches!(
            b,
            b'|' | b'&' | b'<' | b'>' | b',' | b' ' | b'?' | b'{' | b'}' | b':' | b'(' | b')'
        ) {
            if i > segment_start {
                result.push_str(short_name(&ty[segment_start..i]));
            }
            result.push(b as char);
            segment_start = i + 1;
        }
    }
    // Flush trailing segment.
    if segment_start < ty.len() {
        result.push_str(short_name(&ty[segment_start..]));
    }
    result
}

/// Check whether two type strings refer to the same type, ignoring
/// namespace qualification differences.
///
/// Returns `true` when the only difference between `a` and `b` is that
/// one uses a fully-qualified class name (e.g. `App\Models\User`) while
/// the other uses the short name (`User`).  Handles nullable (`?`),
/// union (`|`), and intersection (`&`) types by comparing each component
/// after stripping namespace prefixes and a leading `\`.
pub(crate) fn types_equivalent(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }

    // Strip nullable `?` prefix from both sides.
    let a = a.strip_prefix('?').unwrap_or(a);
    let b = b.strip_prefix('?').unwrap_or(b);

    // Split on `|` and `&` to handle union and intersection types.
    // We compare component counts first, then each pair after
    // normalising namespace prefixes.
    let parts_a: Vec<&str> = a.split('|').flat_map(|part| part.split('&')).collect();
    let parts_b: Vec<&str> = b.split('|').flat_map(|part| part.split('&')).collect();

    if parts_a.len() != parts_b.len() {
        return false;
    }

    // Sort both sides so that `Foo|null` matches `null|Foo`.
    let mut sorted_a: Vec<&str> = parts_a.iter().map(|s| short_name(s)).collect();
    let mut sorted_b: Vec<&str> = parts_b.iter().map(|s| short_name(s)).collect();
    sorted_a.sort_unstable();
    sorted_b.sort_unstable();

    sorted_a == sorted_b
}

/// Return the short (unqualified) class name from a potentially
/// namespace-qualified type string.  Returns only the part after the
/// last `\`.  Non-class types (scalars, `array`, etc.) pass through
/// unchanged.
pub(super) fn short_name(ty: &str) -> &str {
    let t = ty.trim();
    t.rsplit('\\').next().unwrap_or(t)
}
