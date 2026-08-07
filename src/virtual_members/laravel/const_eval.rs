//! A small constant evaluator for the statically-known values a route file
//! builds its route names and URIs from.
//!
//! Route files legitimately register one route per entry of a literal array
//! and name each of them by interpolation, so a name is not always a plain
//! string literal:
//!
//! ```php
//! foreach (['black-friday', 'valentines'] as $event) {
//!     Route::get("/{$event}", [EventsController::class, 'landing'])
//!         ->name("events.{$event}.landing");
//! }
//! ```
//!
//! Everything here folds only what PHP would fold to the same value without
//! running any code.  Anything else — a function call, an unbound variable,
//! an object — is [`ConstValue::Unknown`], which yields no name at all
//! rather than a partial one.

use mago_syntax::cst::*;

use crate::atom::bytes_to_str;

/// The value a PHP expression folds to, as far as it folds without executing
/// anything.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ConstValue {
    /// Not statically known.
    Unknown,
    /// A string or integer, as the text PHP would interpolate it as.
    Scalar(String),
    /// A literal array, as `(key, value)` pairs in source order.
    Array(Vec<(ConstValue, ConstValue)>),
}

impl ConstValue {
    /// The value as the text PHP would interpolate, or `None` when it is not
    /// a known scalar.
    fn scalar(&self) -> Option<&str> {
        match self {
            ConstValue::Scalar(value) => Some(value),
            _ => None,
        }
    }
}

/// The variables in scope while a route file is scanned, innermost last.
///
/// A `foreach` binds its key/value variables for the duration of the body and
/// truncates back afterwards, so a nested loop sees the enclosing loop's
/// variables while a later sibling loop does not.
#[derive(Default)]
pub(crate) struct Scope(Vec<(String, ConstValue)>);

impl Scope {
    fn bind(&mut self, name: &str, value: ConstValue) {
        self.0.push((name.to_string(), value));
    }

    /// The innermost binding of `name`, so a rebound variable shadows the
    /// outer one rather than resurrecting it.
    fn get(&self, name: &str) -> Option<&ConstValue> {
        self.0
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, value)| value)
    }
}

/// Fold an expression to the string it evaluates to, or `None` when it is not
/// statically known.
pub(crate) fn const_string(expr: &Expression<'_>, content: &str, scope: &Scope) -> Option<String> {
    match const_value(expr, content, scope) {
        ConstValue::Scalar(value) => Some(value),
        _ => None,
    }
}

/// Record `$name = <expression>` in `scope`.
///
/// A right-hand side that is not statically known still binds, as
/// [`ConstValue::Unknown`], so it shadows an earlier value of the same name
/// instead of leaving the stale one in force.
pub(crate) fn bind_assignment(assignment: &Assignment<'_>, content: &str, scope: &mut Scope) {
    if !matches!(assignment.operator, AssignmentOperator::Assign(_)) {
        return;
    }
    let Some(name) = direct_variable_name(assignment.lhs) else {
        return;
    };
    let value = const_value(assignment.rhs, content, scope);
    scope.bind(name, value);
}

/// Run `body` once per iteration of `foreach`, with the loop variables bound
/// to that iteration's key and value.
///
/// A subject that folds to a literal array gives one run per element.
/// Anything else gives a single run with the loop variables bound to
/// [`ConstValue::Unknown`] — the body is still walked, and an enclosing
/// variable of the same name cannot leak into it.
pub(crate) fn for_each_iteration(
    foreach: &Foreach<'_>,
    content: &str,
    scope: &mut Scope,
    body: &mut dyn FnMut(&mut Scope),
) {
    let key_name = foreach.target.key().and_then(direct_variable_name);
    let value_name = direct_variable_name(foreach.target.value());

    let entries = match const_value(foreach.expression, content, scope) {
        ConstValue::Array(entries) => entries,
        _ => vec![(ConstValue::Unknown, ConstValue::Unknown)],
    };

    for (key, value) in entries {
        let depth = scope.0.len();
        if let Some(name) = key_name {
            scope.bind(name, key);
        }
        if let Some(name) = value_name {
            scope.bind(name, value);
        }
        body(scope);
        scope.0.truncate(depth);
    }
}

fn const_value(expr: &Expression<'_>, content: &str, scope: &Scope) -> ConstValue {
    match expr {
        Expression::Parenthesized(inner) => const_value(inner.expression, content, scope),
        Expression::Literal(Literal::String(literal)) => {
            match string_literal_text(literal, content) {
                Some(text) => ConstValue::Scalar(text.to_string()),
                None => ConstValue::Unknown,
            }
        }
        Expression::Literal(Literal::Integer(literal)) => match literal.value {
            Some(value) => ConstValue::Scalar(value.to_string()),
            None => ConstValue::Unknown,
        },
        Expression::CompositeString(string) => interpolated_value(string, content, scope),
        Expression::Binary(binary) if binary.operator.is_concatenation() => {
            let left = const_value(binary.lhs, content, scope);
            let right = const_value(binary.rhs, content, scope);
            match (left.scalar(), right.scalar()) {
                (Some(left), Some(right)) => ConstValue::Scalar(format!("{left}{right}")),
                _ => ConstValue::Unknown,
            }
        }
        Expression::Variable(Variable::Direct(variable)) => scope
            .get(variable_name(variable))
            .cloned()
            .unwrap_or(ConstValue::Unknown),
        Expression::Array(array) => const_array(array.elements.as_slice(), content, scope),
        Expression::LegacyArray(array) => const_array(array.elements.as_slice(), content, scope),
        Expression::ArrayAccess(access) => {
            let ConstValue::Array(entries) = const_value(access.array, content, scope) else {
                return ConstValue::Unknown;
            };
            let Some(index) = const_string(access.index, content, scope) else {
                return ConstValue::Unknown;
            };
            // Searched from the end because a repeated key keeps its last
            // value, exactly as PHP builds the array.
            entries
                .into_iter()
                .rev()
                .find(|(key, _)| key.scalar() == Some(index.as_str()))
                .map(|(_, value)| value)
                .unwrap_or(ConstValue::Unknown)
        }
        _ => ConstValue::Unknown,
    }
}

/// Fold a literal array, numbering the elements that were written without a
/// key the way PHP's next-free-integer rule does.
fn const_array(elements: &[ArrayElement<'_>], content: &str, scope: &Scope) -> ConstValue {
    let mut entries = Vec::with_capacity(elements.len());
    let mut next_index: u64 = 0;
    for element in elements {
        let (key, value) = match element {
            ArrayElement::KeyValue(pair) => {
                let key = const_value(pair.key, content, scope);
                if let Some(index) = key.scalar().and_then(|text| text.parse::<u64>().ok()) {
                    next_index = index + 1;
                }
                (key, pair.value)
            }
            ArrayElement::Value(element) => {
                let key = ConstValue::Scalar(next_index.to_string());
                next_index += 1;
                (key, element.value)
            }
            // A spread merges in a shape we would have to know the keys of,
            // and a hole is a syntax error the parser kept; neither leaves the
            // array's element order recoverable.
            ArrayElement::Variadic(_) | ArrayElement::Missing(_) => return ConstValue::Unknown,
        };
        entries.push((key, const_value(value, content, scope)));
    }
    ConstValue::Array(entries)
}

/// Fold an interpolated string or heredoc by folding each of its parts.
///
/// A backtick string runs a shell command, so it is never a constant.
fn interpolated_value(string: &CompositeString<'_>, content: &str, scope: &Scope) -> ConstValue {
    if matches!(string, CompositeString::ShellExecute(_)) {
        return ConstValue::Unknown;
    }
    let mut text = String::new();
    for part in string.parts().iter() {
        let expression = match part {
            // The raw source text is used rather than the unescaped value so
            // that the result matches what a plain literal yields.
            StringPart::Literal(literal) => {
                text.push_str(bytes_to_str(literal.raw));
                continue;
            }
            StringPart::Expression(expression) => expression,
            StringPart::BracedExpression(braced) => braced.expression,
        };
        match const_value(expression, content, scope).scalar() {
            Some(value) => text.push_str(value),
            None => return ConstValue::Unknown,
        }
    }
    ConstValue::Scalar(text)
}

/// The source text between a string literal's quotes, including the empty
/// string that `''` is.
fn string_literal_text<'c>(literal: &LiteralString<'_>, content: &'c str) -> Option<&'c str> {
    let start = literal.span.start.offset as usize + 1;
    let end = literal.span.end.offset as usize - 1;
    if start > end || end > content.len() {
        return None;
    }
    Some(&content[start..end])
}

fn direct_variable_name<'a>(expr: &Expression<'a>) -> Option<&'a str> {
    match expr {
        Expression::Variable(Variable::Direct(variable)) => Some(variable_name(variable)),
        _ => None,
    }
}

fn variable_name<'a>(variable: &DirectVariable<'a>) -> &'a str {
    bytes_to_str(variable.name).trim_start_matches('$')
}
