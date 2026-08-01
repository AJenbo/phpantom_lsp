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

use std::collections::HashMap;
use std::sync::Arc;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::{HasSpan, Span};
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
    /// The enum class named by an enum rule (`new Enum(Role::class)`,
    /// `Rule::enum(Role::class)`), or `None` when the entry has no such rule.
    ///
    /// Stored exactly as written until [`resolve_enum_class_names`] resolves
    /// it against the imports of the file that declares the rules array.
    pub enum_class: Option<String>,
}

/// The entries of one validation rules array, plus whether its key set is
/// known to be complete.
#[derive(Debug, Clone)]
pub(crate) struct RulesArray {
    /// The field entries, in declaration order.
    pub entries: Vec<ValidationRule>,
    /// `false` when a key was not a string literal (`$field => 'required'`),
    /// which means entries are missing from [`Self::entries`].
    ///
    /// Callers that turn the rules into an array shape must bail on this: a
    /// shape that omits keys the request really accepts would report valid
    /// input as unknown.
    pub keys_complete: bool,
}

impl RulesArray {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            keys_complete: true,
        }
    }

    /// Whether no entries were recovered at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
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
    /// The recovered entries.
    pub rules: RulesArray,
}

// ─── Array parsing ──────────────────────────────────────────────────────────

/// Collect the top-level entries of a validation rules array literal.
///
/// Unlike config files, rules arrays are flat: nesting is expressed in the
/// key itself (`items.*.id`), so only the outermost level is walked.
/// `array_merge(parent::rules(), [...])` is followed into each argument.
fn collect_rules_from_expr(expr: &Expression<'_>, content: &str, out: &mut RulesArray) {
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
            } else {
                // `rules()` returning some other call's result — the keys it
                // produces are invisible here.
                out.keys_complete = false;
            }
        }
        // A spread, a variable, a match — whatever keys it contributes are
        // not recoverable, so the set is no longer known to be complete.
        _ => out.keys_complete = false,
    }
}

fn collect_rules_from_elements<'a>(
    elements: impl Iterator<Item = &'a ArrayElement<'a>>,
    content: &str,
    out: &mut RulesArray,
) {
    for element in elements {
        let ArrayElement::KeyValue(kv) = element else {
            // A spread (`...$base`) or a positional entry adds keys we
            // cannot name.
            out.keys_complete = false;
            continue;
        };
        let Some((key, key_start, _)) = extract_string_literal(kv.key, content) else {
            out.keys_complete = false;
            continue;
        };
        if key.is_empty() {
            out.keys_complete = false;
            continue;
        }
        out.entries.push(ValidationRule {
            key: key.to_string(),
            rules: render_rule_value(kv.value, content),
            key_start,
            enum_class: enum_rule_class(kv.value),
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

// ─── Enum rules ─────────────────────────────────────────────────────────────

/// The enum class an enum rule names, exactly as written in the source.
///
/// Laravel writes an enum rule as an object — `new Enum(Role::class)` or its
/// `Rule::enum(Role::class)` shorthand — so the class is only reachable
/// through the expression, not through the rule string.  The rule may sit
/// alone or as one element of an array-form rule list, and the shorthand may
/// carry a fluent chain (`Rule::enum(Role::class)->only([…])`), whose head is
/// still the call that names the enum.
///
/// Returns `None` for every other rule value, and for a class expression
/// that names no class of its own (`self::class`, a variable).
fn enum_rule_class(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::Array(arr) => arr.elements.iter().find_map(element_enum_class),
        Expression::LegacyArray(arr) => arr.elements.iter().find_map(element_enum_class),
        Expression::Parenthesized(p) => enum_rule_class(p.expression),
        Expression::Instantiation(inst) => {
            let Expression::Identifier(class) = inst.class else {
                return None;
            };
            if !short_name(bytes_to_str(class.value())).eq_ignore_ascii_case("Enum") {
                return None;
            }
            class_const_name(argument_at(inst.argument_list.as_ref()?, 0)?)
        }
        Expression::Call(Call::StaticMethod(call)) => {
            let ClassLikeMemberSelector::Identifier(method) = &call.method else {
                return None;
            };
            if !bytes_to_str(method.value).eq_ignore_ascii_case("enum") {
                return None;
            }
            let Expression::Identifier(class) = call.class else {
                return None;
            };
            if !short_name(bytes_to_str(class.value())).eq_ignore_ascii_case("Rule") {
                return None;
            }
            class_const_name(argument_at(&call.argument_list, 0)?)
        }
        Expression::Call(Call::Method(mc)) => enum_rule_class(mc.object),
        Expression::Call(Call::NullSafeMethod(mc)) => enum_rule_class(mc.object),
        _ => None,
    }
}

fn element_enum_class(element: &ArrayElement<'_>) -> Option<String> {
    match element {
        ArrayElement::Value(v) => enum_rule_class(v.value),
        _ => None,
    }
}

/// The class name written in a `Something::class` expression.
///
/// `self::class` and friends are relative to a declaration site the rules
/// array does not carry, so they name nothing here.
fn class_const_name(expr: &Expression<'_>) -> Option<String> {
    let Expression::Access(Access::ClassConstant(access)) = expr else {
        return None;
    };
    let ClassLikeConstantSelector::Identifier(constant) = &access.constant else {
        return None;
    };
    if !bytes_to_str(constant.value).eq_ignore_ascii_case("class") {
        return None;
    }
    let Expression::Identifier(class) = access.class else {
        return None;
    };
    let name = bytes_to_str(class.value());
    if matches!(
        name.to_ascii_lowercase().as_str(),
        "self" | "static" | "parent"
    ) {
        return None;
    }
    Some(name.to_string())
}

/// Resolve the enum class names in `rules` against the imports of the file
/// `program` was parsed from.
///
/// `use App\Enums\Role; … new Enum(Role::class)` names `App\Enums\Role`, and
/// only the declaring file's `use` statements and namespace say so — the
/// class index cannot, since a bare short name matches nothing there.
///
/// Building the import table walks the file's top-level statements, so it is
/// paid only when an enum rule was actually found.
fn resolve_enum_names_in_program(rules: &mut RulesArray, program: &Program<'_>) {
    if !rules.entries.iter().any(|e| e.enum_class.is_some()) {
        return;
    }

    let mut use_map = HashMap::new();
    Backend::extract_use_statements_from_statements(program.statements.iter(), &mut use_map);
    let namespace = Backend::extract_namespace_from_statements(program.statements.iter());

    for entry in &mut rules.entries {
        if let Some(name) = &entry.enum_class {
            entry.enum_class = Some(crate::util::resolve_to_fqn(name, &use_map, &namespace));
        }
    }
}

/// Resolve the enum class names in `rules` against the imports of `content`.
///
/// Rules recovered from a file are resolved during the parse that recovered
/// them; this is for the one array that is parsed on its own — the argument
/// text of a `validate([…])` call, whose imports are the calling file's.
/// Nothing is parsed unless an enum rule was found, and the parse then goes
/// through the shared cache, which the calling file is normally already in.
pub(crate) fn resolve_enum_class_names(rules: &mut RulesArray, content: &str) {
    if !rules.entries.iter().any(|e| e.enum_class.is_some()) {
        return;
    }
    crate::parser::with_parsed_program(content, "resolve_enum_class_names", |program, _| {
        resolve_enum_names_in_program(rules, program);
    });
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

// ─── Scope walking ──────────────────────────────────────────────────────────

/// Whether `span` covers `offset`, inclusive at both ends.
fn covers(span: Span, offset: u32) -> bool {
    offset >= span.start.offset && offset <= span.end.offset
}

/// The outermost function-like body containing `offset`.
///
/// Outermost rather than innermost so that a `validate()` call in a
/// controller action still applies inside a closure nested in that action —
/// "earlier in the same method" covers everything the method wraps.  A
/// sibling method's body never contains the offset, so rules stay scoped to
/// the one being edited.
///
/// Spans nest, so only the cursor's own ancestors are descended into: a
/// subtree that does not cover the offset cannot hold the body that does.
/// That keeps the search proportional to nesting depth rather than to file
/// size.
fn enclosing_body<'ast, 'arena>(
    node: Node<'ast, 'arena>,
    offset: u32,
) -> Option<Node<'ast, 'arena>> {
    let body = match node {
        Node::Method(m) => match &m.body {
            MethodBody::Concrete(block) => Some(Node::Block(block)),
            MethodBody::Abstract(_) => None,
        },
        Node::Function(f) => Some(Node::Block(&f.body)),
        Node::Closure(c) => Some(Node::Block(&c.body)),
        _ => None,
    };
    if let Some(body) = body
        && covers(body.span(), offset)
    {
        return Some(body);
    }

    let mut found = None;
    node.visit_children(|child| {
        if found.is_none() && covers(child.span(), offset) {
            found = enclosing_body(child, offset);
        }
    });
    found
}

/// Hand every node of `node`'s subtree that starts before `cursor` to
/// `visit`.
///
/// Callers are looking for a construct that *completes* before the cursor,
/// and one cannot end before the cursor without starting before it, so
/// subtrees that begin at or after the cursor are skipped rather than walked.
fn walk_before_cursor<'ast, 'arena>(
    node: Node<'ast, 'arena>,
    cursor: u32,
    visit: &mut impl FnMut(Node<'ast, 'arena>),
) {
    visit(node);
    node.visit_children(|child| {
        if child.span().start.offset < cursor {
            walk_before_cursor(child, cursor, visit);
        }
    });
}

/// Whether a construct ending at `end` is a better candidate than the one
/// already in `best`: it has to finish before the cursor, and later beats
/// earlier so the nearest preceding construct wins.
fn beats_best<T>(best: &Option<(u32, T)>, end: u32, cursor: u32) -> bool {
    end <= cursor && best.as_ref().is_none_or(|(seen, _)| end >= *seen)
}

// ─── Inline `validate()` / `Validator::make()` ──────────────────────────────

/// Cheap pre-filter for the shapes [`inline_validate_rules`] recognises.
///
/// `validate()`, `validateWithBag()` and `Validator::make()` all contain
/// "validat" as written, so a file without it is not worth parsing.  PHP
/// method names are case-insensitive, so an unconventional `VALIDATE(` is
/// missed here; that costs suggestions rather than producing wrong ones.
fn mentions_validation(content: &str) -> bool {
    // Matching on the case-invariant tail covers `validate`, `Validator` and
    // `validateWithBag` in one pass.
    memchr::memmem::find(content.as_bytes(), b"alidat").is_some()
}

/// Rules from the last `validate()` / `Validator::make()` call that completes
/// before `offset` inside the same function body.
///
/// Returns `None` when the cursor is not inside a function body, or when no
/// such call precedes it.
pub(crate) fn inline_validate_rules(content: &str, offset: usize) -> Option<RulesArray> {
    if !mentions_validation(content) {
        return None;
    }

    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());

    let body = enclosing_body(Node::Program(program), offset as u32)?;
    let cursor = offset as u32;

    let mut best: Option<(u32, RulesArray)> = None;
    walk_before_cursor(body, cursor, &mut |node| {
        let rules_arg = match node {
            Node::MethodCall(mc) => method_rules_argument(&mc.method, &mc.argument_list),
            Node::NullSafeMethodCall(mc) => method_rules_argument(&mc.method, &mc.argument_list),
            Node::StaticMethodCall(smc) => static_rules_argument(smc),
            _ => None,
        };
        let Some(arg) = rules_arg else {
            return;
        };
        let end = node.span().end.offset;
        if !beats_best(&best, end, cursor) {
            return;
        }
        let mut rules = RulesArray::new();
        collect_rules_from_expr(arg, content, &mut rules);
        // An array whose keys could not all be read is still the nearest
        // rules array, so it is recorded even when nothing was recovered
        // from it.  Dropping it would hand the cursor an earlier, unrelated
        // `validate()` call and describe the request with the wrong keys.
        if !rules.is_empty() || !rules.keys_complete {
            best = Some((end, rules));
        }
    });
    let mut rules = best.map(|(_, rules)| rules)?;
    // Only the winning array is resolved, and only if it holds an enum rule.
    resolve_enum_names_in_program(&mut rules, program);
    Some(rules)
}

/// Parse a rules array written directly at a call site, e.g. the argument
/// text of `$request->validate([…])`.
///
/// The text is parsed standalone, so [`ValidationRule::key_start`] offsets
/// index `array_text` rather than any file; callers that need navigable
/// offsets must use [`inline_validate_rules`] instead.  For the same reason
/// there are no imports to read here, so an enum rule keeps its name as
/// written and the caller must pass the rules through
/// [`resolve_enum_class_names`] with the surrounding file's content.
pub(crate) fn rules_from_array_text(array_text: &str) -> Option<RulesArray> {
    let source = format!("<?php return {};", array_text.trim());
    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, source.as_bytes());

    let mut out = RulesArray::new();
    collect_returned_rules(Node::Program(program), &source, &mut out);
    (!out.is_empty()).then_some(out)
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

// ─── `safe()` provenance ────────────────────────────────────────────────────

/// The request variable a `ValidatedInput` was narrowed from, e.g.
/// `"$request"` for `$safe = $request->safe();`.
///
/// `safe()` hands back the same rules array under a different type, so a
/// validated-input variable completes against the request that produced it.
/// Only the last assignment to `variable` before `offset` in the enclosing
/// body counts, so a reassignment wins.
///
/// Returns `None` when no such assignment precedes the cursor, or when the
/// object `safe()` was called on is not itself a plain variable.
pub(crate) fn safe_source_variable(content: &str, offset: usize, variable: &str) -> Option<String> {
    // Cheap pre-filter: without a `safe(` anywhere there is nothing to trace,
    // so the file is not worth parsing.
    memchr::memmem::find(content.as_bytes(), b"safe(")?;

    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());

    let body = enclosing_body(Node::Program(program), offset as u32)?;
    let cursor = offset as u32;

    let mut best: Option<(u32, String)> = None;
    walk_before_cursor(body, cursor, &mut |node| {
        let Node::Assignment(assignment) = node else {
            return;
        };
        let Expression::Variable(Variable::Direct(target)) = assignment.lhs else {
            return;
        };
        if bytes_to_str(target.name) != variable {
            return;
        }
        let end = node.span().end.offset;
        if !beats_best(&best, end, cursor) {
            return;
        }
        if let Some(source) = safe_call_receiver_variable(assignment.rhs) {
            best = Some((end, source));
        }
    });
    best.map(|(_, source)| source)
}

/// The receiver of a no-argument `->safe()` call, when it is a plain
/// variable.
pub(crate) fn safe_call_receiver_variable(expr: &Expression<'_>) -> Option<String> {
    let (object, method) = match expr {
        Expression::Call(Call::Method(mc)) => (mc.object, &mc.method),
        Expression::Call(Call::NullSafeMethod(mc)) => (mc.object, &mc.method),
        _ => return None,
    };
    let ClassLikeMemberSelector::Identifier(ident) = method else {
        return None;
    };
    if !bytes_to_str(ident.value).eq_ignore_ascii_case("safe") {
        return None;
    }
    let Expression::Variable(Variable::Direct(var)) = object else {
        return None;
    };
    Some(bytes_to_str(var.name).to_string())
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

/// Whether `class` is the `ValidatedInput` wrapper `Request::safe()` returns,
/// which carries the request's rules but not its `rules()` method.
pub(crate) fn is_validated_input(class: &ClassInfo) -> bool {
    class.fqn() == VALIDATED_INPUT_FQN
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

/// The rules that describe `class` at `offset`.
///
/// A `FormRequest` carries its own `rules()`; every other request-like
/// receiver — a plain `Request`, a `Validator`, a `ValidatedInput` — is
/// described by the nearest `validate()` / `Validator::make()` call preceding
/// the cursor in the same function body.
///
/// This is the single definition of that precedence; both request-input
/// completion and `validated()` shape inference go through it.
pub(crate) fn rules_in_scope(
    backend: &Backend,
    class: &ClassInfo,
    current_uri: &str,
    content: &str,
    offset: usize,
) -> Option<ResolvedRules> {
    let loader = |name: &str| backend.find_or_load_class(name);
    if is_form_request(class, &loader)
        && let Some(resolved) = form_request_rules(backend, class, current_uri, content)
    {
        return Some(resolved);
    }
    inline_validate_rules(content, offset).map(|rules| ResolvedRules {
        source: RulesSource::CurrentFile,
        rules,
    })
}

/// Parse the array returned by `class_name`'s `rules()` method.
pub(crate) fn rules_from_class_source(content: &str, class_name: &str) -> RulesArray {
    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());

    let mut out = RulesArray::new();
    collect_rules_method(Node::Program(program), class_name, content, &mut out);
    // An enum rule names its class in this file's terms, and this file is
    // already parsed — resolving here saves the caller a second pass over it.
    resolve_enum_names_in_program(&mut out, program);
    out
}

fn collect_rules_method(node: Node<'_, '_>, class_name: &str, content: &str, out: &mut RulesArray) {
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

fn collect_returned_rules(node: Node<'_, '_>, content: &str, out: &mut RulesArray) {
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
