//! Laravel validation-rule extraction.
//!
//! The keys of a validation rules array are a static contract: they name
//! exactly the input fields a validated request may carry.  This module
//! recovers that contract from source so that request-input completion
//! and go-to-definition can use it.
//!
//! Two sources are supported:
//!
//! - The `rules()` method of a `FormRequest` subclass, walking the parent
//!   chain and through `array_merge()` so `parent::rules()` composition
//!   still yields the locally declared keys.
//! - An inline `$request->validate([…])`, `$this->validate($request, […])`,
//!   `$request->validateWithBag('bag', […])`, or `Validator::make($data, […])`
//!   call earlier in the same function body.
//!
//! Only literal string keys are recovered.  A computed key contributes
//! nothing, which degrades to "fewer suggestions" rather than to wrong
//! ones.

use std::sync::Arc;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::Backend;
use crate::atom::bytes_to_str;
use crate::types::{ClassInfo, MAX_INHERITANCE_DEPTH};
use crate::util::short_name;

use super::helpers::extract_string_literal;

/// The FQN of Laravel's base form-request class.
pub(crate) const FORM_REQUEST_FQN: &str = "Illuminate\\Foundation\\Http\\FormRequest";

/// The FQN of the object `Request::safe()` returns.
pub(crate) const VALIDATED_INPUT_FQN: &str = "Illuminate\\Support\\ValidatedInput";

/// One field entry of a validation rules array.
#[derive(Debug, Clone)]
pub(crate) struct ValidationRule {
    /// The field name exactly as written, e.g. `"name"` or `"items.*.id"`.
    pub key: String,
    /// The rule specification rendered for display, e.g.
    /// `"required|string|max:255"`.  Empty when the value is not literal.
    pub rules: String,
    /// Byte offset of the key literal's content (just inside the quotes),
    /// in the file named by the owning [`RulesSource`].
    pub key_start: usize,
}

/// Which file a resolved rules array was parsed from.
#[derive(Debug)]
pub(crate) enum RulesSource {
    /// The rules live in the file the cursor is in — the caller already
    /// holds its content, so nothing is cloned.
    CurrentFile,
    /// The rules live in another file (a `FormRequest` class).
    OtherFile {
        /// URI of the file holding the rules array.
        uri: String,
        /// Content of that file; [`ValidationRule::key_start`] indexes it.
        content: String,
    },
}

/// A validation rules array located for a cursor position.
#[derive(Debug)]
pub(crate) struct ResolvedRules {
    /// Where the rules were parsed from.
    pub source: RulesSource,
    /// The field entries, in declaration order.
    pub rules: Vec<ValidationRule>,
}

// ─── Array parsing ──────────────────────────────────────────────────────────

/// Collect the top-level entries of a validation rules array literal.
///
/// Unlike config files, rules arrays are flat: nesting is expressed in the
/// key itself (`items.*.id`), so only the outermost level is walked.
/// `array_merge(parent::rules(), [...])` is followed into each argument.
fn collect_rules_from_expr(expr: &Expression<'_>, content: &str, out: &mut Vec<ValidationRule>) {
    match expr {
        Expression::Array(arr) => collect_rules_from_elements(arr.elements.iter(), content, out),
        Expression::LegacyArray(arr) => {
            collect_rules_from_elements(arr.elements.iter(), content, out)
        }
        Expression::Parenthesized(p) => collect_rules_from_expr(p.expression, content, out),
        Expression::Call(Call::Function(fc)) => {
            if let Expression::Identifier(ident) = fc.function
                && ident.value().eq_ignore_ascii_case(b"array_merge")
            {
                for arg in fc.argument_list.arguments.iter() {
                    collect_rules_from_expr(arg.value(), content, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_rules_from_elements<'a>(
    elements: impl Iterator<Item = &'a ArrayElement<'a>>,
    content: &str,
    out: &mut Vec<ValidationRule>,
) {
    for element in elements {
        let ArrayElement::KeyValue(kv) = element else {
            continue;
        };
        let Some((key, key_start, _)) = extract_string_literal(kv.key, content) else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        out.push(ValidationRule {
            key: key.to_string(),
            rules: render_rule_value(kv.value, content),
            key_start,
        });
    }
}

/// Render a rule value as the pipe-separated string Laravel documents,
/// regardless of whether it was written as a string or an array.
///
/// Non-literal entries (`new Enum(Role::class)`, `Rule::unique(...)`) are
/// rendered from their source text so hover/completion detail still says
/// something useful.
fn render_rule_value(expr: &Expression<'_>, content: &str) -> String {
    match expr {
        Expression::Literal(literal::Literal::String(_)) => extract_string_literal(expr, content)
            .map_or_else(String::new, |(v, _, _)| v.to_string()),
        Expression::Array(arr) => render_rule_list(arr.elements.iter(), content),
        Expression::LegacyArray(arr) => render_rule_list(arr.elements.iter(), content),
        other => condense(source_text(other, content)),
    }
}

fn render_rule_list<'a>(
    elements: impl Iterator<Item = &'a ArrayElement<'a>>,
    content: &str,
) -> String {
    let parts: Vec<String> = elements
        .filter_map(|element| match element {
            ArrayElement::Value(v) => Some(render_rule_value(v.value, content)),
            _ => None,
        })
        .filter(|p| !p.is_empty())
        .collect();
    parts.join("|")
}

fn source_text<'c>(expr: &Expression<'_>, content: &'c str) -> &'c str {
    let span = expr.span();
    content
        .get(span.start.offset as usize..span.end.offset as usize)
        .unwrap_or_default()
}

/// Collapse runs of whitespace so a multi-line rule expression fits on the
/// single line completion detail and hover show it on.
fn condense(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.truncate(out.trim_end().len());
    out
}

// ─── Inline `validate()` / `Validator::make()` ──────────────────────────────

/// Rules from the last `validate()` / `Validator::make()` call that completes
/// before `offset` inside the same function body.
///
/// Returns `None` when the cursor is not inside a function body, or when no
/// such call precedes it.
pub(crate) fn inline_validate_rules(content: &str, offset: usize) -> Option<Vec<ValidationRule>> {
    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());

    let (body_start, body_end) = enclosing_body_range(program, offset as u32)?;

    let mut best: Option<(u32, Vec<ValidationRule>)> = None;
    collect_validate_calls(
        Node::Program(program),
        content,
        (body_start, body_end),
        offset as u32,
        &mut best,
    );
    best.map(|(_, rules)| rules).filter(|r| !r.is_empty())
}

/// The outermost function-like body span containing `offset`.
///
/// Outermost rather than innermost so that a `validate()` call in a
/// controller action still applies inside a closure nested in that action —
/// "earlier in the same method" covers everything the method wraps.  A
/// sibling method's body never contains the offset, so rules stay scoped to
/// the one being edited.
fn enclosing_body_range(program: &Program<'_>, offset: u32) -> Option<(u32, u32)> {
    let mut found: Option<(u32, u32)> = None;
    walk_bodies(Node::Program(program), offset, &mut found);
    found
}

fn walk_bodies(node: Node<'_, '_>, offset: u32, found: &mut Option<(u32, u32)>) {
    if found.is_some() {
        return;
    }
    let body_span = match node {
        Node::Method(m) => match &m.body {
            MethodBody::Concrete(block) => Some(block.span()),
            MethodBody::Abstract(_) => None,
        },
        Node::Function(f) => Some(f.body.span()),
        Node::Closure(c) => Some(c.body.span()),
        _ => None,
    };
    if let Some(span) = body_span {
        let (start, end) = (span.start.offset, span.end.offset);
        if offset >= start && offset <= end {
            *found = Some((start, end));
            return;
        }
    }
    node.visit_children(|child| walk_bodies(child, offset, found));
}

/// Walk for `validate`-style calls inside `body`, keeping the last one that
/// finishes before `cursor`.
fn collect_validate_calls(
    node: Node<'_, '_>,
    content: &str,
    body: (u32, u32),
    cursor: u32,
    best: &mut Option<(u32, Vec<ValidationRule>)>,
) {
    let rules_arg = match node {
        Node::MethodCall(mc) => method_rules_argument(&mc.method, &mc.argument_list),
        Node::NullSafeMethodCall(mc) => method_rules_argument(&mc.method, &mc.argument_list),
        Node::StaticMethodCall(smc) => static_rules_argument(smc),
        _ => None,
    };

    if let Some(arg) = rules_arg {
        let span = node.span();
        let (start, end) = (span.start.offset, span.end.offset);
        if start >= body.0
            && end <= body.1
            && end <= cursor
            && best.as_ref().is_none_or(|(be, _)| end >= *be)
        {
            let mut rules = Vec::new();
            collect_rules_from_expr(arg, content, &mut rules);
            if !rules.is_empty() {
                *best = Some((end, rules));
            }
        }
    }

    node.visit_children(|child| collect_validate_calls(child, content, body, cursor, best));
}

/// The rules argument of `->validate([...])`, `->validate($request, [...])`,
/// or `->validateWithBag('bag', [...])`.
fn method_rules_argument<'ast, 'arena>(
    selector: &ClassLikeMemberSelector<'arena>,
    arguments: &'ast ArgumentList<'arena>,
) -> Option<&'ast Expression<'arena>> {
    let ClassLikeMemberSelector::Identifier(ident) = selector else {
        return None;
    };
    let name = bytes_to_str(ident.value);
    if name.eq_ignore_ascii_case("validate") {
        // `$request->validate($rules)` and the `ValidatesRequests` trait's
        // `$this->validate($request, $rules)` differ only in arity.
        argument_at(arguments, 0)
            .filter(|e| is_array_literal(e))
            .or_else(|| argument_at(arguments, 1).filter(|e| is_array_literal(e)))
    } else if name.eq_ignore_ascii_case("validateWithBag") {
        argument_at(arguments, 1).filter(|e| is_array_literal(e))
    } else {
        None
    }
}

/// The rules argument of `Validator::make($data, [...])`.
fn static_rules_argument<'ast, 'arena>(
    call: &'ast StaticMethodCall<'arena>,
) -> Option<&'ast Expression<'arena>> {
    let ClassLikeMemberSelector::Identifier(ident) = &call.method else {
        return None;
    };
    if !bytes_to_str(ident.value).eq_ignore_ascii_case("make") {
        return None;
    }
    let Expression::Identifier(class) = call.class else {
        return None;
    };
    if !short_name(bytes_to_str(class.value())).eq_ignore_ascii_case("Validator") {
        return None;
    }
    argument_at(&call.argument_list, 1).filter(|e| is_array_literal(e))
}

fn argument_at<'ast, 'arena>(
    arguments: &'ast ArgumentList<'arena>,
    index: usize,
) -> Option<&'ast Expression<'arena>> {
    arguments.arguments.iter().nth(index).map(|a| a.value())
}

fn is_array_literal(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Array(_) | Expression::LegacyArray(_) => true,
        Expression::Parenthesized(p) => is_array_literal(p.expression),
        _ => false,
    }
}

// ─── `FormRequest::rules()` ─────────────────────────────────────────────────

fn is_form_request_fqn(name: &str) -> bool {
    name == FORM_REQUEST_FQN
}

fn is_request_fqn(name: &str) -> bool {
    name == super::REQUEST_FQN || name == VALIDATED_INPUT_FQN
}

/// Whether `class` is a `FormRequest` subclass (or `FormRequest` itself).
pub(crate) fn is_form_request(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    is_form_request_fqn(&class.fqn())
        || super::helpers::walks_parent_chain(class, class_loader, is_form_request_fqn)
}

/// Whether `class` holds request input — an `Illuminate\Http\Request`
/// (which `FormRequest` extends) or the `ValidatedInput` wrapper that
/// `Request::safe()` returns.
///
/// `walks_parent_chain` matches parent names, which post-processing has
/// already resolved to FQNs; the class's own `name` is the short name, so
/// it is checked separately against its `fqn()`.
pub(crate) fn is_request_like(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    is_request_fqn(&class.fqn())
        || super::helpers::walks_parent_chain(class, class_loader, is_request_fqn)
}

/// The rules declared by `class`'s own `rules()` method, or the nearest
/// ancestor that declares one.
///
/// Returns `None` when no ancestor declares a `rules()` method with a literal
/// array return, or when the declaring class's source cannot be located.
pub(crate) fn form_request_rules(
    backend: &Backend,
    class: &ClassInfo,
    current_uri: &str,
    current_content: &str,
) -> Option<ResolvedRules> {
    let mut fqn = class.fqn().to_string();
    let mut depth = 0u32;

    loop {
        let (uri, content) = backend.find_class_file_content(&fqn, current_uri, current_content)?;
        let rules = rules_from_class_source(&content, short_name(&fqn));
        if !rules.is_empty() {
            let source = if uri == current_uri {
                RulesSource::CurrentFile
            } else {
                RulesSource::OtherFile { uri, content }
            };
            return Some(ResolvedRules { source, rules });
        }

        depth += 1;
        if depth > MAX_INHERITANCE_DEPTH {
            return None;
        }
        let parent = backend
            .find_or_load_class(&fqn)
            .and_then(|c| c.parent_class)?;
        if parent.as_str() == FORM_REQUEST_FQN {
            return None;
        }
        fqn = parent.to_string();
    }
}

/// Parse the array returned by `class_name`'s `rules()` method.
pub(crate) fn rules_from_class_source(content: &str, class_name: &str) -> Vec<ValidationRule> {
    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());

    let mut out = Vec::new();
    collect_rules_method(Node::Program(program), class_name, content, &mut out);
    out
}

fn collect_rules_method(
    node: Node<'_, '_>,
    class_name: &str,
    content: &str,
    out: &mut Vec<ValidationRule>,
) {
    if let Node::Class(class) = node {
        if !bytes_to_str(class.name.value).eq_ignore_ascii_case(class_name) {
            return;
        }
        for member in class.members.iter() {
            if let ClassLikeMember::Method(method) = member
                && bytes_to_str(method.name.value).eq_ignore_ascii_case("rules")
                && let MethodBody::Concrete(block) = &method.body
            {
                collect_returned_rules(Node::Block(block), content, out);
            }
        }
        return;
    }
    node.visit_children(|child| collect_rules_method(child, class_name, content, out));
}

fn collect_returned_rules(node: Node<'_, '_>, content: &str, out: &mut Vec<ValidationRule>) {
    if let Node::Return(ret) = node {
        if let Some(value) = ret.value {
            collect_rules_from_expr(value, content, out);
        }
        return;
    }
    node.visit_children(|child| collect_returned_rules(child, content, out));
}

// ─── Candidate keys ─────────────────────────────────────────────────────────

/// A field name offered to the user, paired with the rule that produced it.
pub(crate) struct RuleField {
    /// The completable field name, e.g. `"name"`, `"items"`, `"address.city"`.
    pub name: String,
    /// The rule specification of the entry this name came from.
    pub rules: String,
    /// Byte offset of the declaring key literal's content.
    pub key_start: usize,
}

/// Expand rule keys into the field names an input accessor accepts.
///
/// A plain dotted key (`address.city`) is offered whole, since Laravel's dot
/// notation reaches it directly.  A wildcard key (`items.*.id`) names array
/// members that no accessor addresses literally, so only its root segment is
/// offered.  Roots come after the declared keys so the exact rules — and
/// their rule text — rank first.
pub(crate) fn rule_fields(rules: &[ValidationRule]) -> Vec<RuleField> {
    let mut out: Vec<RuleField> = Vec::with_capacity(rules.len());

    for rule in rules {
        if rule.key.split('.').any(|seg| seg == "*") {
            continue;
        }
        if !out.iter().any(|f| f.name == rule.key) {
            out.push(RuleField {
                name: rule.key.clone(),
                rules: rule.rules.clone(),
                key_start: rule.key_start,
            });
        }
    }

    for rule in rules {
        let Some(root) = rule.key.split('.').next() else {
            continue;
        };
        if root.is_empty() || root == "*" || root == rule.key {
            continue;
        }
        if !out.iter().any(|f| f.name == root) {
            out.push(RuleField {
                name: root.to_string(),
                rules: String::new(),
                key_start: rule.key_start,
            });
        }
    }

    out
}

#[cfg(test)]
#[path = "validation_rules_tests.rs"]
mod tests;
