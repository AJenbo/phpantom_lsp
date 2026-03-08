//! Signature help (`textDocument/signatureHelp`).
//!
//! When the cursor is inside the parentheses of a function or method call,
//! this module resolves the callable and returns its signature (parameter
//! names, types, and return type) along with the index of the parameter
//! currently being typed.
//!
//! The primary detection path uses precomputed [`CallSite`] data from the
//! [`SymbolMap`] (AST-based, handles chains and nesting correctly).  When
//! the symbol map has no matching call site (e.g. the parser couldn't
//! recover an unclosed paren), we fall back to text-based backward
//! scanning so that signature help still works on incomplete code.

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::named_args::{
    extract_call_expression, find_enclosing_open_paren, position_to_char_offset,
    split_args_top_level,
};
use crate::symbol_map::SymbolMap;
use crate::types::*;
use crate::util::position_to_offset;

/// Information about a signature help call site, extracted from the source
/// text around the cursor.
struct CallSiteContext {
    /// The call expression in a format suitable for resolution (same
    /// format as [`NamedArgContext::call_expression`]).
    call_expression: String,
    /// Zero-based index of the parameter the cursor is currently on,
    /// determined by counting top-level commas before the cursor.
    active_parameter: u32,
}

// ─── AST-based detection ────────────────────────────────────────────────────

/// Detect the call site using precomputed [`CallSite`] data from the
/// symbol map.
///
/// Converts the LSP `Position` to a byte offset, finds the innermost
/// `CallSite` whose argument list contains the cursor, and computes the
/// active parameter index from the precomputed comma offsets.
fn detect_call_site_from_map(
    symbol_map: &SymbolMap,
    content: &str,
    position: Position,
) -> Option<CallSiteContext> {
    let cursor_byte_offset = position_to_offset(content, position);
    let cs = symbol_map.find_enclosing_call_site(cursor_byte_offset)?;
    // Active parameter = number of commas before the cursor.
    let active = cs
        .comma_offsets
        .iter()
        .filter(|&&comma| comma < cursor_byte_offset)
        .count() as u32;
    Some(CallSiteContext {
        call_expression: cs.call_expression.clone(),
        active_parameter: active,
    })
}

// ─── Text-based detection (fallback) ────────────────────────────────────────

/// Detect whether the cursor is inside a function/method call using
/// text-based backward scanning.
///
/// This is the **fallback** path used when the AST-based detection
/// (via `detect_call_site_from_map`) has no hit — typically because the
/// parser couldn't recover the call node from incomplete code (e.g. an
/// unclosed `(`).
///
/// Returns `None` if the cursor is not inside call parentheses.
fn detect_call_site_text_fallback(content: &str, position: Position) -> Option<CallSiteContext> {
    let chars: Vec<char> = content.chars().collect();
    let cursor = position_to_char_offset(&chars, position)?;

    // Find the enclosing open paren.  We search backward from the cursor
    // position itself (not from a word-start like named-arg detection does)
    // because signature help should fire even when the cursor is right
    // after a comma or the open paren with no identifier typed yet.
    let open_paren = find_enclosing_open_paren(&chars, cursor)?;

    // Extract the call expression before `(`.
    let call_expr = extract_call_expression(&chars, open_paren)?;
    if call_expr.is_empty() {
        return None;
    }

    // Count the active parameter by splitting the text between `(` and the
    // cursor into top-level comma-separated segments.
    let args_text: String = chars[open_paren + 1..cursor].iter().collect();
    let segments = split_args_top_level(&args_text);
    // `split_args_top_level` returns one segment per completed comma-separated
    // argument (it omits a trailing empty segment).  The number of commas
    // equals the number of segments (each segment ended with a comma, except
    // possibly the last one which is the argument currently being typed).
    //
    // If the text ends with a comma (i.e. the cursor is right after `,`),
    // the split will have consumed it and the cursor is on the *next*
    // parameter.  Otherwise, the cursor is still on the segment after the
    // last comma.
    let trimmed = args_text.trim_end();
    let active = if trimmed.is_empty() {
        0
    } else if trimmed.ends_with(',') {
        segments.len() as u32
    } else {
        // The user is in the middle of typing an argument.  The number of
        // completed args equals segments.len() - 1 (last segment is the
        // current one) + 1 for the current, but we want a 0-based index
        // so it's segments.len() - 1.  However split_args_top_level may
        // or may not include the trailing segment.  Counting commas
        // directly is more reliable.
        count_top_level_commas(&chars, open_paren + 1, cursor)
    };

    Some(CallSiteContext {
        call_expression: call_expr,
        active_parameter: active,
    })
}

/// Count commas at nesting depth 0 between `start` (inclusive) and `end`
/// (exclusive) in a char slice, skipping nested parens/brackets and
/// string literals.
fn count_top_level_commas(chars: &[char], start: usize, end: usize) -> u32 {
    let mut count = 0u32;
    let mut depth = 0i32;
    let mut i = start;

    while i < end {
        match chars[i] {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => count += 1,
            '\'' | '"' => {
                let q = chars[i];
                i += 1;
                while i < end {
                    if chars[i] == q {
                        let mut backslashes = 0u32;
                        let mut k = i;
                        while k > start && chars[k - 1] == '\\' {
                            backslashes += 1;
                            k -= 1;
                        }
                        if backslashes.is_multiple_of(2) {
                            break;
                        }
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    count
}

// ─── Signature building ─────────────────────────────────────────────────────

/// Format a single parameter for the signature label using the **native**
/// PHP type hint (not the docblock-enriched effective type).
///
/// Optional (non-variadic) parameters with a known default value include
/// ` = <value>` so the user can see the default at a glance.
fn format_param_label(param: &ParameterInfo) -> String {
    let mut parts = Vec::new();
    if let Some(ref th) = param.native_type_hint {
        parts.push(th.clone());
    }
    if param.is_variadic {
        parts.push(format!("...{}", param.name));
    } else if param.is_reference {
        parts.push(format!("&{}", param.name));
    } else {
        parts.push(param.name.clone());
    }
    let base = parts.join(" ");
    if !param.is_required
        && !param.is_variadic
        && let Some(ref dv) = param.default_value
    {
        return format!("{} = {}", base, dv);
    }
    base
}

/// Shorten a type string by stripping namespace prefixes from every
/// fully-qualified name, including names nested inside generic
/// parameters.
///
/// For example `\App\Models\User` → `User`, `string` → `string`,
/// `\App\User|\App\Admin` → `User|Admin`,
/// `list<\App\User>` → `list<User>`,
/// `array<string, \Ns\Cls>` → `array<string, Cls>`.
fn shorten_type(ty: &str) -> String {
    // Split on characters that delimit type names in PHP type strings
    // (|, <, >, comma, space) while preserving the delimiters. Replace
    // each segment that looks like a FQN with its short name.
    let mut result = String::with_capacity(ty.len());
    let mut segment_start = 0;
    for (i, ch) in ty.char_indices() {
        if matches!(ch, '|' | '<' | '>' | ',' | ' ') {
            if i > segment_start {
                let seg = &ty[segment_start..i];
                result.push_str(crate::util::short_name(seg));
            }
            result.push(ch);
            segment_start = i + ch.len_utf8();
        }
    }
    // Trailing segment after the last delimiter (or the whole string).
    if segment_start < ty.len() {
        result.push_str(crate::util::short_name(&ty[segment_start..]));
    }
    result
}

/// Build per-parameter documentation markdown.
///
/// When the effective type (`type_hint`) differs from the native PHP type
/// (`native_type_hint`), the effective type is shown (shortened to base
/// names) in an inline code span so the user sees the richer docblock
/// type.  If a `@param` description is also present it is appended after
/// the type.
///
/// Returns `None` when there is nothing to show (no description and the
/// types are identical or absent).
fn build_param_documentation(param: &ParameterInfo) -> Option<Documentation> {
    let effective = param.type_hint.as_deref();
    let native = param.native_type_hint.as_deref();
    let desc = param.description.as_deref();

    let show_effective = match (effective, native) {
        (Some(e), Some(n)) => !crate::hover::types_equivalent(e, n),
        (Some(_), None) => true,
        _ => false,
    };

    let shortened = effective.map(shorten_type);
    let value = match (show_effective, desc) {
        (true, Some(d)) => format!("`{}` {}", shortened.as_deref().unwrap_or(""), d),
        (true, None) => format!("`{}`", shortened.as_deref().unwrap_or("")),
        (false, Some(d)) => d.to_string(),
        (false, None) => return None,
    };

    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value,
    }))
}

/// Build a `SignatureInformation` from a callable's metadata.
///
/// The label uses **native** PHP types for parameters and a shortened
/// (base-name) effective return type.  Per-parameter documentation shows
/// the `@param` description, optionally prefixed with the effective type
/// when it differs from the native type.
fn build_signature(params: &[ParameterInfo], return_type: Option<&str>) -> SignatureInformation {
    // Build the label: `(param1, param2, ...): ReturnType`
    // The callable name is omitted — the user already knows what they
    // are calling and the editor shows it in the surrounding code.
    let param_labels: Vec<String> = params.iter().map(format_param_label).collect();
    let params_str = param_labels.join(", ");
    let ret = format!(
        ": {}",
        return_type.map_or("mixed".to_string(), shorten_type)
    );
    let label = format!("({}){}", params_str, ret);

    // Build ParameterInformation using label offsets.  The offsets are
    // byte offsets into the label string (UTF-16 code unit offsets are
    // also accepted, but since PHP identifiers are ASCII the byte
    // offsets match).
    let mut param_infos = Vec::with_capacity(params.len());
    // The parameters start right after the `(`.
    let mut offset = 1; // skip the leading `(`

    for (idx, (pl, param)) in param_labels.iter().zip(params.iter()).enumerate() {
        let start = offset as u32;
        let end = (offset + pl.len()) as u32;
        param_infos.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([start, end]),
            documentation: build_param_documentation(param),
        });
        // Move past this parameter label and the separator `, `.
        offset += pl.len();
        if idx < param_labels.len() - 1 {
            offset += 2; // ", "
        }
    }

    SignatureInformation {
        label,
        documentation: None,
        parameters: Some(param_infos),
        active_parameter: None,
    }
}

// ─── Resolution ─────────────────────────────────────────────────────────────

/// Resolved callable information ready to be turned into a
/// `SignatureHelp` response.
///
/// This is a thin projection of [`ResolvedCallableTarget`] kept for
/// local use within this module.  Only the fields actually consumed by
/// `resolve_signature` are retained.
struct ResolvedCallable {
    /// The parameters of the callable.
    parameters: Vec<ParameterInfo>,
    /// Optional return type string (effective / docblock-enriched).
    return_type: Option<String>,
}

impl From<crate::types::ResolvedCallableTarget> for ResolvedCallable {
    fn from(t: crate::types::ResolvedCallableTarget) -> Self {
        Self {
            parameters: t.parameters,
            return_type: t.return_type,
        }
    }
}

impl Backend {
    /// Handle a `textDocument/signatureHelp` request.
    ///
    /// Returns `Some(SignatureHelp)` when the cursor is inside a
    /// function or method call and the callable can be resolved, or
    /// `None` otherwise.
    ///
    /// Detection strategy:
    /// 1. **AST-based** — look up the precomputed `CallSite` in the
    ///    symbol map.  This handles chains, nesting, and strings correctly.
    /// 2. **Text fallback** — when the symbol map has no hit (e.g. the
    ///    parser couldn't recover the call node from incomplete code),
    ///    fall back to the text-based backward scanner.
    pub(crate) fn handle_signature_help(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<SignatureHelp> {
        let ctx = self.file_context(uri);

        // ── Primary path: AST-based detection via symbol map ────────
        let symbol_map = self.symbol_maps.read().get(uri).cloned();

        if let Some(ref sm) = symbol_map
            && let Some(site) = detect_call_site_from_map(sm, content, position)
            && let Some(result) = self.resolve_signature(&site, content, position, &ctx)
        {
            return Some(result);
        }

        // ── Fallback: text-based detection ──────────────────────────
        // The parser may not have produced a call node (e.g. unclosed
        // paren while typing).  The text scanner handles this because
        // it only needs an unmatched `(`.
        if let Some(site) = detect_call_site_text_fallback(content, position) {
            // Try with current AST first.
            if let Some(result) = self.resolve_signature(&site, content, position, &ctx) {
                return Some(result);
            }

            // Patch content (insert `);` at cursor) and retry with
            // a re-parsed AST so resolution can find class context.
            let patched = Self::patch_content_for_signature(content, position);
            if patched != content {
                let patched_classes = self.parse_php(&patched);
                if !patched_classes.is_empty() {
                    let patched_ctx = FileContext {
                        classes: patched_classes,
                        use_map: ctx.use_map.clone(),
                        namespace: ctx.namespace.clone(),
                    };
                    if let Some(result) =
                        self.resolve_signature(&site, &patched, position, &patched_ctx)
                    {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Resolve the call expression to a `SignatureHelp` using the given
    /// file context and content.
    fn resolve_signature(
        &self,
        site: &CallSiteContext,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<SignatureHelp> {
        let resolved = self.resolve_callable(&site.call_expression, content, position, ctx)?;

        let sig = build_signature(&resolved.parameters, resolved.return_type.as_deref());
        Some(SignatureHelp {
            signatures: vec![sig],
            active_signature: Some(0),
            active_parameter: Some(clamp_active_param(
                site.active_parameter,
                &resolved.parameters,
            )),
        })
    }

    /// Resolve a call expression string to the callable's metadata.
    ///
    /// Delegates to the shared [`Backend::resolve_callable_target`] and
    /// converts the result into the local [`ResolvedCallable`] type.
    fn resolve_callable(
        &self,
        expr: &str,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<ResolvedCallable> {
        self.resolve_callable_target(expr, content, position, ctx)
            .map(ResolvedCallable::from)
    }

    /// Scan backward from `cursor_offset` for an assignment like
    /// `$fn = someTarget(...)` and return the callable target string
    /// (e.g. `"makePen"`, `"$obj->method"`, `"ClassName::method"`).
    ///
    /// This enables signature help for first-class callable invocations:
    /// `$fn = makePen(...); $fn()` shows `makePen`'s parameters.
    pub(crate) fn extract_callable_target_from_variable(
        var_name: &str,
        content: &str,
        cursor_offset: u32,
    ) -> Option<String> {
        let search_area = content.get(..cursor_offset as usize)?;
        let assign_prefix = format!("{} = ", var_name);
        let assign_pos = search_area.rfind(&assign_prefix)?;
        let rhs_start = assign_pos + assign_prefix.len();

        // Find the terminating `;`.
        let remaining = &content[rhs_start..];
        let semi_pos = remaining.find(';')?;
        let rhs_text = remaining[..semi_pos].trim();

        // Must end with `(...)` — the first-class callable syntax marker.
        let callable_text = rhs_text.strip_suffix("(...)")?.trim_end();
        if callable_text.is_empty() {
            return None;
        }

        // Return the target in the format `resolve_callable` expects:
        //   - `$this->method` or `$obj->method` → instance method
        //   - `ClassName::method` → static method
        //   - `functionName` → standalone function
        Some(callable_text.to_string())
    }

    /// Insert `);` at the cursor position so that an unclosed call
    /// expression becomes syntactically valid.
    ///
    /// This is the same patching strategy used by named-argument
    /// completion (see `handler::patch_content_at_cursor`).
    fn patch_content_for_signature(content: &str, position: Position) -> String {
        let line_idx = position.line as usize;
        let col = position.character as usize;
        let mut result = String::with_capacity(content.len() + 2);

        for (i, line) in content.lines().enumerate() {
            if i == line_idx {
                let byte_col = line
                    .char_indices()
                    .nth(col)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
                result.push_str(&line[..byte_col]);
                result.push_str(");");
                result.push_str(&line[byte_col..]);
            } else {
                result.push_str(line);
            }
            result.push('\n');
        }

        // Remove the trailing newline we may have added if the original
        // content did not end with one.
        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }

        result
    }
}

/// Clamp the active parameter index so it doesn't exceed the parameter
/// count.  For variadic parameters, the index stays on the last parameter
/// even when the user types additional arguments.
fn clamp_active_param(active: u32, params: &[ParameterInfo]) -> u32 {
    if params.is_empty() {
        return 0;
    }
    let last = (params.len() - 1) as u32;
    active.min(last)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "signature_help_tests.rs"]
mod tests;
