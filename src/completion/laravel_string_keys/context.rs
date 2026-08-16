//! Syntactic detection for Laravel string-key completion contexts.

use tower_lsp::lsp_types::Position;

use crate::symbol_map::{LaravelConfigResource, LaravelResourceReceiverRule, LaravelStringKind};
use crate::text_position::position_to_offset;

// ─── Context ────────────────────────────────────────────────────────────────

/// A recognized Laravel string-key expression and the fragment to complete.
pub(super) struct LaravelStringKeyContext<'a> {
    /// The resource family addressed by the expression.
    pub(super) kind: LaravelStringKind,
    /// The fragment between the opening quote and cursor.
    pub(super) prefix: &'a str,
    /// Byte offset of the string content start (right after the opening quote).
    pub(super) content_start_offset: usize,
    /// A resource call/property whose Laravel meaning requires type
    /// confirmation. `kind` is replaced with the confirmed family before
    /// candidates are enumerated.
    pub(super) receiver_rule: Option<LaravelResourceReceiverRule>,
    /// Textual receiver of a type-dependent method call. Empty for a
    /// `$connection` property, whose enclosing class is the subject.
    pub(super) receiver_subject: Option<String>,
}

#[inline]
fn is_unescaped(bytes: &[u8], index: usize) -> bool {
    let mut before = index;
    while before > 0 && bytes[before - 1] == b'\\' {
        before -= 1;
    }
    (index - before).is_multiple_of(2)
}

/// Find the callable text before an array literal that is itself the first
/// argument of a call. `before_quote` ends immediately before the current
/// string literal, somewhere inside that array.
fn callable_before_array_argument(before_quote: &str) -> Option<(&str, Option<&str>)> {
    let bytes = before_quote.as_bytes();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut string_quote = None;
    let mut i = bytes.len();

    while i > 0 {
        i -= 1;
        let byte = bytes[i];

        if let Some(quote) = string_quote {
            if byte == quote && is_unescaped(bytes, i) {
                string_quote = None;
            }
            continue;
        }

        match byte {
            b'\'' | b'"' => string_quote = Some(byte),
            b']' if paren_depth == 0 && brace_depth == 0 => bracket_depth += 1,
            b'[' if paren_depth == 0 && brace_depth == 0 && bracket_depth == 0 => {
                let before_array = before_quote[..i].trim_end();
                return callable_before_scalar_argument(before_array);
            }
            b'[' if paren_depth == 0 && brace_depth == 0 => bracket_depth -= 1,
            b')' => paren_depth += 1,
            b'(' if paren_depth > 0 => paren_depth -= 1,
            b'(' if bracket_depth == 0 && brace_depth == 0 => {
                let before_open = before_quote[..i].trim_end();
                let mut token_start = before_open.len();
                let token_bytes = before_open.as_bytes();
                while token_start > 0
                    && (token_bytes[token_start - 1].is_ascii_alphanumeric()
                        || token_bytes[token_start - 1] == b'_')
                {
                    token_start -= 1;
                }
                if before_open[token_start..].eq_ignore_ascii_case("array") {
                    return callable_before_scalar_argument(before_open[..token_start].trim_end());
                }
                return None;
            }
            b'}' => brace_depth += 1,
            b'{' if brace_depth > 0 => brace_depth -= 1,
            b'{' if bracket_depth == 0 && paren_depth == 0 => return None,
            b';' if bracket_depth == 0 && paren_depth == 0 && brace_depth == 0 => return None,
            _ => {}
        }
    }

    None
}

/// Whether the current literal is the key side of an associative array
/// element. Resource-array triggers (for example `Log::stack`) name values,
/// never their bookkeeping keys.
pub(super) fn string_literal_is_array_key(content: &str, cursor: usize, quote: u8) -> bool {
    let bytes = content.as_bytes();
    let mut index = cursor;
    while index < bytes.len() {
        if bytes[index] == quote && is_unescaped(bytes, index) {
            return content[index + 1..].trim_start().starts_with("=>");
        }
        if bytes[index] == b'\n' {
            return false;
        }
        index += 1;
    }
    false
}

/// Callable text before a scalar first argument, including PHP's named-arg
/// spelling (`method(name: '…')`).
pub(super) fn callable_before_scalar_argument(before_value: &str) -> Option<(&str, Option<&str>)> {
    let before_value = before_value.trim_end();
    if let Some(callable) = before_value.strip_suffix('(') {
        return Some((callable.trim_end(), None));
    }

    let colon = before_value.rfind(':')?;
    let label = before_value[colon + 1..].trim();
    if !label.is_empty() {
        return None;
    }
    let before_label = before_value[..colon].trim_end();
    let label_start = before_label
        .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .map_or(0, |index| index + 1);
    if label_start == before_label.len() {
        return None;
    }
    let argument = &before_label[label_start..];
    let before_argument = before_label[..label_start].trim_end();
    let open_paren = enclosing_call_open_paren(before_argument)?;
    Some((before_argument[..open_paren].trim_end(), Some(argument)))
}

/// Find the unmatched call parenthesis immediately enclosing a named
/// argument. This accepts reordered arguments while ignoring delimiters in
/// earlier nested expressions and strings.
pub(super) fn enclosing_call_open_paren(content: &str) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut quote = None;
    let mut index = bytes.len();
    while index > 0 {
        index -= 1;
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == active_quote && is_unescaped(bytes, index) {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b')' => parens += 1,
            b'(' if parens > 0 => parens -= 1,
            b'(' if brackets == 0 && braces == 0 => return Some(index),
            b']' => brackets += 1,
            b'[' if brackets > 0 => brackets -= 1,
            b'}' => braces += 1,
            b'{' if braces > 0 => braces -= 1,
            b';' if parens == 0 && brackets == 0 && braces == 0 => return None,
            _ => {}
        }
    }
    None
}

/// Whether a literal is initializing an instance `$connection` property.
pub(super) fn is_connection_property_value(before_value: &str) -> bool {
    let Some(before_equals) = before_value.strip_suffix('=') else {
        return false;
    };
    let before_equals = before_equals.trim_end();
    // A promoted parameter owns only the text since the enclosing `(` or
    // preceding comma. Looking at the whole method declaration would mistake
    // the method's own visibility for parameter promotion.
    let statement_start = before_equals
        .rfind([';', '{', '}', '(', ')', ','])
        .map_or(0, |index| index + 1);
    let declaration = before_equals[statement_start..].trim_start();
    let Some(variable) = declaration.split_whitespace().next_back() else {
        return false;
    };
    if !variable.eq_ignore_ascii_case("$connection") {
        return false;
    }
    if declaration
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("static"))
    {
        return false;
    }
    declaration.split_whitespace().any(|token| {
        token.eq_ignore_ascii_case("public")
            || token.eq_ignore_ascii_case("protected")
            || token.eq_ignore_ascii_case("private")
            || token.eq_ignore_ascii_case("var")
    })
}

#[inline]
fn matches_ignore_ascii_case(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

#[inline]
fn ends_with_ignore_ascii_case(value: &str, suffix: &str) -> bool {
    let value = value.as_bytes();
    let suffix = suffix.as_bytes();
    value.len() >= suffix.len() && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// Interpret the payload of Laravel's parameterised middleware aliases.
/// Returns the kind, the fragment being replaced, and that fragment's byte
/// offset from the opening quote.
fn middleware_completion_context(prefix: &str) -> Option<(LaravelStringKind, &str, usize)> {
    let colon = prefix.find(':')?;
    let alias = &prefix[..=colon];
    let payload = &prefix[colon + 1..];

    if let Some(resource) = crate::symbol_map::laravel_resources::middleware_resource(alias) {
        let raw_current = payload
            .rsplit_once(',')
            .map_or(payload, |(_, current)| current);
        let current = raw_current.trim_start();
        let start = prefix.len().saturating_sub(raw_current.len());
        return Some((LaravelStringKind::ConfigResource(resource), current, start));
    }

    if alias.eq_ignore_ascii_case("throttle:") {
        // Everything after a comma is a decay/attempt parameter. A single
        // numeric token is ambiguous because Laravel checks registered names
        // before treating it as an inline limit; candidate filtering settles
        // it without offering unrelated limiter names.
        let current = payload.trim_start();
        if current.contains(',') {
            return None;
        }
        let start = prefix.len().saturating_sub(payload.len());
        return Some((LaravelStringKind::RateLimiter, current, start));
    }

    if alias.eq_ignore_ascii_case("can:") && !payload.contains(',') {
        let current = payload.trim_start();
        let start = prefix.len().saturating_sub(payload.len());
        return Some((LaravelStringKind::GateAbility, current, start));
    }

    None
}

fn instance_receiver_subject(content: &str, access_end: usize) -> Option<String> {
    let position = crate::text_position::offset_to_position(content, access_end);
    crate::completion::target::extract_completion_target(content, position)
        .map(|target| target.subject)
}

/// Whether `callable` is the class token of a `new` expression for one of
/// Laravel's queue rate-limiter middleware classes.
pub(super) fn rate_limited_constructor_class(callable: &str) -> Option<(usize, &str)> {
    let bytes = callable.as_bytes();
    let mut class_start = bytes.len();
    while class_start > 0
        && (bytes[class_start - 1].is_ascii_alphanumeric()
            || matches!(bytes[class_start - 1], b'_' | b'\\'))
    {
        class_start -= 1;
    }
    let class = callable[class_start..].trim_start_matches('\\');
    let before_class = callable[..class_start].trim_end();
    let before_new = before_class.strip_suffix("new")?;
    if before_new
        .chars()
        .next_back()
        .is_some_and(|ch| !ch.is_whitespace() && !matches!(ch, ';' | '{' | '}' | '(' | '=' | ','))
    {
        return None;
    }
    Some((class_start, class))
}

fn is_rate_limited_class(class: &str) -> bool {
    matches!(
        class.trim_start_matches('\\'),
        "RateLimited"
            | "RateLimitedWithRedis"
            | "Illuminate\\Queue\\Middleware\\RateLimited"
            | "Illuminate\\Queue\\Middleware\\RateLimitedWithRedis"
    )
}

#[inline]
fn semantic_class_name<'a>(
    written: &'a str,
    offset: usize,
    resolved_names: Option<&'a crate::names::OwnedResolvedNames>,
) -> &'a str {
    resolved_names
        .and_then(|names| names.get(offset as u32))
        .unwrap_or(written)
}

/// Whether a semantic class name identifies the requested Laravel facade.
#[inline]
pub(super) fn is_laravel_facade(class: &str, facade: &str) -> bool {
    let class = class.trim_start_matches('\\');
    class.rsplit_once('\\').map_or_else(
        || class.eq_ignore_ascii_case(facade),
        |(namespace, short)| {
            namespace.eq_ignore_ascii_case("Illuminate\\Support\\Facades")
                && short.eq_ignore_ascii_case(facade)
        },
    )
}

/// Whether the current method chain contains a static call rooted at one
/// Laravel facade. Production completion resolves each written class token,
/// so imported aliases work while namespace-local homonyms stay ordinary.
pub(super) fn chain_contains_laravel_facade(
    chain: &str,
    chain_offset: usize,
    resolved_names: Option<&crate::names::OwnedResolvedNames>,
    facade: &str,
) -> bool {
    let bytes = chain.as_bytes();
    let mut search_start = 0usize;
    while let Some(relative) = chain[search_start..].find("::") {
        let colons = search_start + relative;
        let mut class_end = colons;
        while class_end > 0 && bytes[class_end - 1].is_ascii_whitespace() {
            class_end -= 1;
        }
        let mut class_start = class_end;
        while class_start > 0
            && (bytes[class_start - 1].is_ascii_alphanumeric()
                || matches!(bytes[class_start - 1], b'_' | b'\\'))
        {
            class_start -= 1;
        }
        if class_start < class_end {
            let written = &chain[class_start..class_end];
            let semantic = semantic_class_name(written, chain_offset + class_start, resolved_names);
            if is_laravel_facade(semantic, facade) {
                return true;
            }
        }
        search_start = colons + 2;
    }
    false
}

#[inline]
fn is_laravel_container_attribute(class: &str) -> bool {
    let class = class.trim_start_matches('\\');
    class.rsplit_once('\\').is_some_and(|(namespace, _)| {
        namespace.eq_ignore_ascii_case("Illuminate\\Container\\Attributes")
    })
}

// ─── Detection ──────────────────────────────────────────────────────────────

/// Detect if the cursor is inside the first string argument of a Laravel
/// helper function.  Returns the key kind and the prefix typed so far.
#[cfg(test)]
pub(super) fn detect_laravel_string_key_context(
    content: &str,
    position: Position,
) -> Option<LaravelStringKeyContext<'_>> {
    detect_laravel_string_key_context_inner(content, position, None)
}

/// Detect a Laravel string-key context using resolved names when available.
pub(super) fn detect_laravel_string_key_context_inner<'a>(
    content: &'a str,
    position: Position,
    resolved_names: Option<&'a crate::names::OwnedResolvedNames>,
) -> Option<LaravelStringKeyContext<'a>> {
    let cursor_offset = position_to_offset(content, position) as usize;
    let bytes = content.as_bytes();

    if cursor_offset == 0 {
        return None;
    }

    // ── Find the opening quote before the cursor ────────────────────
    let mut quote_pos = None;
    let mut i = cursor_offset;
    while i > 0 {
        i -= 1;
        let ch = bytes[i];
        if (ch == b'\'' || ch == b'"') && is_unescaped(bytes, i) {
            quote_pos = Some(i);
            break;
        }
        if ch == b'\n' {
            return None;
        }
    }
    let quote_pos = quote_pos?;
    let prefix = &content[quote_pos + 1..cursor_offset];

    // ── Locate the call whose first argument owns this string ───────
    let before_quote = content[..quote_pos].trim_end();
    if is_connection_property_value(before_quote) {
        return Some(LaravelStringKeyContext {
            kind: LaravelStringKind::ConfigResource(LaravelConfigResource::DatabaseConnection),
            prefix,
            content_start_offset: quote_pos + 1,
            receiver_rule: Some(LaravelResourceReceiverRule::ConnectionProperty),
            receiver_subject: None,
        });
    }
    let (before_paren, named_argument, in_array_argument) =
        if let Some((before_paren, argument)) = callable_before_array_argument(before_quote) {
            (before_paren, argument, true)
        } else {
            let (before_paren, argument) = callable_before_scalar_argument(before_quote)?;
            (before_paren, argument, false)
        };
    if in_array_argument && string_literal_is_array_key(content, cursor_offset, bytes[quote_pos]) {
        return None;
    }

    // ── Extract the function/method name ────────────────────────────
    let bp_bytes = before_paren.as_bytes();
    let name_end = bp_bytes.len();
    let mut name_start = name_end;
    while name_start > 0
        && (bp_bytes[name_start - 1].is_ascii_alphanumeric() || bp_bytes[name_start - 1] == b'_')
    {
        name_start -= 1;
    }
    if name_start == name_end {
        return None;
    }
    let func_name = &before_paren[name_start..name_end];

    // ── Check for static method syntax (Config::get, etc.) ──────────
    let before_name = &before_paren[..name_start];
    let is_static = before_name.trim_end().ends_with("::");

    // Check for instance method call (->route() or ?->route())
    let trimmed_before = before_name.trim_end();
    let is_instance_method = trimmed_before.ends_with("->") || trimmed_before.ends_with("?->");

    // Check for PHP attribute syntax: #[Config('key')] or
    // #[\Illuminate\Container\Attributes\Config('key')].
    // Check only the callable token after the nearest `#[`: every byte there
    // must be part of one class name. An unrelated attribute farther up the
    // file therefore cannot turn an ordinary call into an attribute context.
    let is_attribute = before_paren.rfind("#[").is_some_and(|start| {
        let name = before_paren[start + 2..].trim_start_matches('\\');
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'\\')
    });

    let mut completion_prefix = prefix;
    let mut completion_start = quote_pos + 1;
    let mut receiver_rule = None;
    let mut receiver_subject = None;

    let kind = if is_attribute {
        const ATTR_NS: &str = "Illuminate\\Container\\Attributes\\";
        let start = before_paren.rfind("#[")? + 2;
        let attr_class = before_paren[start..].trim_start_matches('\\');
        let semantic_class = semantic_class_name(attr_class, start, resolved_names);
        let short = semantic_class.rsplit('\\').next().unwrap_or(semantic_class);
        let is_laravel_attribute = if resolved_names.is_some() {
            is_laravel_container_attribute(semantic_class)
        } else if attr_class.contains('\\') {
            attr_class
                .strip_prefix(ATTR_NS)
                .is_some_and(|rest| rest == short)
        } else {
            content.contains("use Illuminate\\Container\\Attributes\\")
        };
        if !is_laravel_attribute || in_array_argument {
            return None;
        }
        if short.eq_ignore_ascii_case("Config") {
            if named_argument.is_some_and(|argument| !argument.eq_ignore_ascii_case("key")) {
                return None;
            }
            Some(LaravelStringKind::Config)
        } else {
            crate::symbol_map::laravel_resources::attribute_trigger(short)
                .filter(|trigger| {
                    named_argument
                        .is_none_or(|argument| argument.eq_ignore_ascii_case(trigger.argument))
                })
                .map(|trigger| LaravelStringKind::ConfigResource(trigger.kind))
        }
    } else if is_static {
        let before_colons = &trimmed_before[..trimmed_before.len() - 2].trim_end();
        let bc_bytes = before_colons.as_bytes();
        let mut cls_start = bc_bytes.len();
        while cls_start > 0
            && (bc_bytes[cls_start - 1].is_ascii_alphanumeric()
                || bc_bytes[cls_start - 1] == b'_'
                || bc_bytes[cls_start - 1] == b'\\')
        {
            cls_start -= 1;
        }
        let class_name = &before_colons[cls_start..];
        let semantic_class = semantic_class_name(class_name, cls_start, resolved_names);
        let short = semantic_class.rsplit('\\').next().unwrap_or(semantic_class);
        if let Some(trigger) =
            crate::symbol_map::laravel_resources::static_method_trigger(semantic_class, func_name)
        {
            if named_argument
                .is_some_and(|argument| !argument.eq_ignore_ascii_case(trigger.argument))
                || (in_array_argument && !trigger.shape.accepts_array())
                || (!in_array_argument && !trigger.shape.accepts_scalar())
            {
                return None;
            }
            Some(LaravelStringKind::ConfigResource(trigger.kind))
        } else {
            if is_laravel_facade(semantic_class, "Route")
                && func_name.eq_ignore_ascii_case("middleware")
                && named_argument.is_none_or(|argument| argument.eq_ignore_ascii_case("middleware"))
            {
                let (middleware_kind, middleware_prefix, relative_start) =
                    middleware_completion_context(completion_prefix)?;
                completion_prefix = middleware_prefix;
                completion_start += relative_start;
                return Some(LaravelStringKeyContext {
                    kind: middleware_kind,
                    prefix: completion_prefix,
                    content_start_offset: completion_start,
                    receiver_rule: None,
                    receiver_subject: None,
                });
            }
            if in_array_argument {
                return None;
            }
            if is_laravel_facade(semantic_class, "Config")
                && matches_ignore_ascii_case(
                    func_name,
                    &[
                        "get",
                        "set",
                        "has",
                        "boolean",
                        "array",
                        "collection",
                        "prepend",
                        "push",
                    ],
                )
            {
                Some(LaravelStringKind::Config)
            } else if short.eq_ignore_ascii_case("View")
                && matches_ignore_ascii_case(func_name, &["make", "exists"])
            {
                Some(LaravelStringKind::View)
            } else if short.eq_ignore_ascii_case("Lang")
                && matches_ignore_ascii_case(func_name, &["get", "has", "choice"])
            {
                Some(LaravelStringKind::Trans)
            } else if (short.eq_ignore_ascii_case("Artisan")
                && matches_ignore_ascii_case(func_name, &["call", "queue"]))
                || (short.eq_ignore_ascii_case("Schedule")
                    && func_name.eq_ignore_ascii_case("command"))
            {
                Some(LaravelStringKind::Command)
            } else if (short.eq_ignore_ascii_case("Relation")
                && func_name.eq_ignore_ascii_case("getMorphedModel"))
                || (short.eq_ignore_ascii_case("Model")
                    && func_name.eq_ignore_ascii_case("getActualClassNameForMorph"))
            {
                Some(LaravelStringKind::MorphAlias)
            } else if short.eq_ignore_ascii_case("Gate")
                && matches_ignore_ascii_case(
                    func_name,
                    &[
                        "allows",
                        "denies",
                        "check",
                        "any",
                        "none",
                        "authorize",
                        "inspect",
                        "has",
                        "define",
                    ],
                )
            {
                Some(LaravelStringKind::GateAbility)
            } else if is_laravel_facade(semantic_class, "RateLimiter")
                && func_name.eq_ignore_ascii_case("for")
                && named_argument.is_none_or(|argument| argument.eq_ignore_ascii_case("name"))
            {
                Some(LaravelStringKind::RateLimiter)
            } else {
                None
            }
        }
    } else if is_instance_method {
        let is_middleware = func_name.eq_ignore_ascii_case("middleware");
        let is_call = matches_ignore_ascii_case(func_name, &["call", "callSilently"]);
        let is_authorize = func_name.eq_ignore_ascii_case("authorize");
        let is_can = matches_ignore_ascii_case(func_name, &["can", "cannot", "canAny"]);
        let is_gate_check = matches_ignore_ascii_case(
            func_name,
            &["allows", "denies", "check", "any", "none", "inspect", "has"],
        );
        let receiver = trimmed_before
            .trim_end_matches("?->")
            .trim_end_matches("->")
            .trim_end();
        // These generic method names acquire Laravel meaning on `$this` only
        // in their controller/command contexts.
        let receiver_is_this =
            (is_middleware || is_call || is_authorize) && receiver.ends_with("$this");
        // Whether the receiver plainly reads as the authenticated user,
        // which is what makes `->can('…')` an authorization check rather
        // than a same-named method on an unrelated object.  Mirrors the
        // symbol-map rule that decides which `can()` calls get a span.
        let receiver_is_user_like = is_can && {
            let tail = receiver
                .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("");
            ends_with_ignore_ascii_case(tail, "user")
                || ends_with_ignore_ascii_case(receiver, "user()")
        };
        // A chain that starts at the `Gate` facade
        // (`Gate::forUser($user)->allows('…')`) or at a route registration
        // (`Route::get(…)->can('…')`) is an authorization check whatever the
        // rest of the chain looks like.  Only the text back to the start of
        // the statement is searched — `trimmed_before` is the whole file
        // prefix, and an unrelated `Gate::` far above would false-positive.
        let needs_gate_chain = is_authorize || is_can || is_gate_check;
        let needs_route_chain = is_middleware || is_can;
        let (chain_starts_at_gate, chain_starts_at_route) = if needs_gate_chain || needs_route_chain
        {
            let chain_start = trimmed_before
                .rfind(['\n', ';', '{', '}'])
                .map_or(0, |idx| idx + 1);
            let chain_text = &trimmed_before[chain_start..];
            (
                needs_gate_chain
                    && chain_contains_laravel_facade(
                        chain_text,
                        chain_start,
                        resolved_names,
                        "Gate",
                    ),
                needs_route_chain
                    && chain_contains_laravel_facade(
                        chain_text,
                        chain_start,
                        resolved_names,
                        "Route",
                    ),
            )
        } else {
            (false, false)
        };
        if is_middleware && (receiver_is_this || chain_starts_at_route) {
            let (middleware_kind, middleware_prefix, relative_start) =
                middleware_completion_context(completion_prefix)?;
            completion_prefix = middleware_prefix;
            completion_start += relative_start;
            return Some(LaravelStringKeyContext {
                kind: middleware_kind,
                prefix: completion_prefix,
                content_start_offset: completion_start,
                receiver_rule: None,
                receiver_subject: None,
            });
        }
        if in_array_argument {
            return None;
        }

        if func_name.eq_ignore_ascii_case("connection")
            && named_argument.is_none_or(|argument| {
                argument.eq_ignore_ascii_case("name") || argument.eq_ignore_ascii_case("connection")
            })
        {
            receiver_rule = Some(LaravelResourceReceiverRule::ConnectionMethod);
            receiver_subject = instance_receiver_subject(content, trimmed_before.len());
            Some(LaravelStringKind::ConfigResource(
                LaravelConfigResource::DatabaseConnection,
            ))
        } else if let Some(trigger) = crate::symbol_map::laravel_resources::instance_method_trigger(
            func_name,
        )
        .filter(|trigger| {
            named_argument.is_none_or(|argument| argument.eq_ignore_ascii_case(trigger.argument))
        }) {
            receiver_rule = Some(LaravelResourceReceiverRule::QueueableConnection);
            receiver_subject = instance_receiver_subject(content, trimmed_before.len());
            Some(LaravelStringKind::ConfigResource(trigger.kind))
        } else if func_name.eq_ignore_ascii_case("onQueue")
            && named_argument.is_none_or(|argument| argument.eq_ignore_ascii_case("queue"))
        {
            receiver_rule = Some(LaravelResourceReceiverRule::QueueName);
            receiver_subject = instance_receiver_subject(content, trimmed_before.len());
            Some(LaravelStringKind::QueueName)
        } else if func_name.eq_ignore_ascii_case("route") {
            Some(LaravelStringKind::Route)
        } else if is_call && receiver_is_this {
            Some(LaravelStringKind::Command)
        } else if (is_authorize && (receiver_is_this || chain_starts_at_gate))
            || (is_can && (receiver_is_user_like || chain_starts_at_route || chain_starts_at_gate))
            || (is_gate_check && chain_starts_at_gate)
        {
            Some(LaravelStringKind::GateAbility)
        } else {
            None
        }
    } else {
        if in_array_argument {
            return None;
        }
        if rate_limited_constructor_class(before_paren).is_some_and(|(offset, class)| {
            is_rate_limited_class(semantic_class_name(class, offset, resolved_names))
        }) && named_argument.is_none_or(|argument| argument.eq_ignore_ascii_case("limiterName"))
        {
            Some(LaravelStringKind::RateLimiter)
        } else if let Some(trigger) =
            crate::symbol_map::laravel_resources::function_trigger(func_name)
        {
            (trigger.shape.accepts_scalar()
                && named_argument
                    .is_none_or(|argument| argument.eq_ignore_ascii_case(trigger.argument)))
            .then_some(LaravelStringKind::ConfigResource(trigger.kind))
        } else if matches_ignore_ascii_case(func_name, &["route", "to_route"]) {
            Some(LaravelStringKind::Route)
        } else if func_name.eq_ignore_ascii_case("config") {
            Some(LaravelStringKind::Config)
        } else if matches_ignore_ascii_case(
            func_name,
            &["view", "blade_view_directive", "blade_each_directive"],
        ) {
            Some(LaravelStringKind::View)
        } else if matches_ignore_ascii_case(func_name, &["__", "trans", "trans_choice"]) {
            Some(LaravelStringKind::Trans)
        } else if func_name.eq_ignore_ascii_case("blade_can_directive") {
            Some(LaravelStringKind::GateAbility)
        } else {
            None
        }
    };

    let kind = kind?;

    Some(LaravelStringKeyContext {
        kind,
        prefix: completion_prefix,
        content_start_offset: completion_start,
        receiver_rule,
        receiver_subject,
    })
}
