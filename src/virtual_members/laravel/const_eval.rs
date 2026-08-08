//! A small constant evaluator for the statically-known strings a project
//! builds its route names, URIs, and container binding keys from.
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
//! Service providers do the same with the key they bind under, keeping it in
//! a constant or a static property rather than repeating the string
//! (`$this->app->singleton(static::$abstract, …)`), which a [`ClassContext`]
//! on the [`Scope`] resolves.
//!
//! Everything here folds only what PHP would fold to the same value without
//! running any code.  A call to one of a fixed list of pure string functions
//! (`trim`, `str_replace`, `preg_replace`, …) folds when every argument is
//! already known; anything else — an unbound variable, an object, a call
//! outside the list — is [`ConstValue::Unknown`], which yields no name at
//! all rather than a partial one.

use std::collections::HashMap;

use mago_syntax::cst::*;
use regex::Regex;

use crate::atom::bytes_to_str;
use crate::types::{ClassInfo, PropertySource};

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

/// The class that `self::` and `static::` name in the file being scanned,
/// together with the values its constants and static-property defaults hold.
///
/// The members are inheritance-merged by the caller, so a service provider
/// binding under `static::$abstract` folds even when the property is declared
/// on the base provider it extends.
#[derive(Default)]
pub(crate) struct ClassContext {
    /// The class's short name, which an explicit `Provider::ABSTRACT`
    /// qualifier is matched against.
    short_name: String,
    /// Constant name (`ABSTRACT`) or static property name including its `$`
    /// (`$abstract`) → the string the member holds.
    members: HashMap<String, String>,
}

impl ClassContext {
    /// Collect the members of `class` whose declared value is a plain
    /// literal.  Anything computed (a concatenation, another constant, a
    /// call) is left out rather than folded from its source text.
    pub(crate) fn from_class(class: &ClassInfo) -> Self {
        let mut members = HashMap::new();

        for constant in class.constants.iter() {
            if constant.is_enum_case {
                continue;
            }
            if let Some(value) = constant.value.as_deref().and_then(literal_text) {
                members.insert(constant.name.to_string(), value);
            }
        }

        for property in class.properties.iter() {
            let Some(PropertySource::DeclaredDefault { value }) = property.source.as_ref() else {
                continue;
            };
            if property.is_static
                && let Some(value) = literal_text(value)
            {
                members.insert(format!("${}", property.name), value);
            }
        }

        Self {
            short_name: class.name.to_string(),
            members,
        }
    }
}

/// The string a declared constant or property default holds, given the source
/// text of its initializer (`"'sentry'"`, `"42"`).
///
/// Only a plain quoted literal or an integer folds.  An escape sequence or an
/// interpolation would mean re-implementing PHP's string syntax over text the
/// parser already threw away, and no container key is written that way.
fn literal_text(source: &str) -> Option<String> {
    let source = source.trim();
    let quote = source.chars().next()?;
    if quote != '\'' && quote != '"' {
        return source.parse::<i64>().ok().map(|number| number.to_string());
    }
    let inner = source.strip_prefix(quote)?.strip_suffix(quote)?;
    // A second quote of the same kind means the text is not one literal but a
    // concatenation of several, and a backslash or `$` means PHP would read
    // the contents rather than take them verbatim.
    if inner.contains([quote, '\\']) || (quote == '"' && inner.contains(['$', '{'])) {
        return None;
    }
    Some(inner.to_string())
}

/// The variables in scope while a file is scanned, innermost last, plus the
/// class its `self::` / `static::` references resolve against.
///
/// A `foreach` binds its key/value variables for the duration of the body and
/// truncates back afterwards, so a nested loop sees the enclosing loop's
/// variables while a later sibling loop does not.
#[derive(Default)]
pub(crate) struct Scope {
    variables: Vec<(String, ConstValue)>,
    class: Option<ClassContext>,
}

impl Scope {
    /// A scope for a file that declares `class`, so its own constants and
    /// static properties fold.
    pub(crate) fn for_class(class: ClassContext) -> Self {
        Self {
            variables: Vec::new(),
            class: Some(class),
        }
    }

    fn bind(&mut self, name: &str, value: ConstValue) {
        self.variables.push((name.to_string(), value));
    }

    /// The innermost binding of `name`, so a rebound variable shadows the
    /// outer one rather than resurrecting it.
    fn get(&self, name: &str) -> Option<&ConstValue> {
        self.variables
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
        let depth = scope.variables.len();
        if let Some(name) = key_name {
            scope.bind(name, key);
        }
        if let Some(name) = value_name {
            scope.bind(name, value);
        }
        body(scope);
        scope.variables.truncate(depth);
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
        Expression::Call(Call::Function(call)) => const_function_call(call, content, scope),
        Expression::Access(Access::StaticProperty(access)) => match &access.property {
            Variable::Direct(property) => {
                class_member_value(access.class, bytes_to_str(property.name), scope)
            }
            _ => ConstValue::Unknown,
        },
        Expression::Access(Access::ClassConstant(access)) => match &access.constant {
            // `Foo::class` is the class's own name rather than a declared
            // constant, and a binding keyed by one already resolves as written.
            ClassLikeConstantSelector::Identifier(constant)
                if !constant.value.eq_ignore_ascii_case(b"class") =>
            {
                class_member_value(access.class, bytes_to_str(constant.value), scope)
            }
            _ => ConstValue::Unknown,
        },
        _ => ConstValue::Unknown,
    }
}

/// The value of a class member named through `self::`, `static::`, or the
/// scanned class's own name.
///
/// Every other qualifier names a class this scan never read, so it stays
/// unknown rather than borrowing the scanned class's member of that name.
/// `parent::` is unknown too: the members are merged over the parent chain,
/// which cannot tell an inherited value from one the child overrode.
fn class_member_value(class: &Expression<'_>, member: &str, scope: &Scope) -> ConstValue {
    let Some(context) = scope.class.as_ref() else {
        return ConstValue::Unknown;
    };
    let names_context = match class {
        Expression::Self_(_) | Expression::Static(_) => true,
        Expression::Identifier(identifier) => {
            crate::util::short_name(bytes_to_str(identifier.value()))
                .eq_ignore_ascii_case(&context.short_name)
        }
        _ => false,
    };
    if !names_context {
        return ConstValue::Unknown;
    }
    match context.members.get(member) {
        Some(value) => ConstValue::Scalar(value.clone()),
        None => ConstValue::Unknown,
    }
}

/// Fold a call to a fixed list of pure string functions whose result is
/// fully determined by constant arguments, e.g. `preg_replace('#^/xmas/#',
/// '', $slug)` normalizing a loop variable before it is used in a route
/// name.
///
/// Only folds when every argument is a plain positional argument and every
/// value it resolves to is already known; a named or spread argument, or an
/// unknown value, leaves the whole call [`ConstValue::Unknown`] rather than
/// risk folding the wrong position or inventing a partial result.
fn const_function_call(call: &FunctionCall<'_>, content: &str, scope: &Scope) -> ConstValue {
    let Expression::Identifier(identifier) = call.function else {
        return ConstValue::Unknown;
    };
    let name = bytes_to_str(identifier.value())
        .trim_start_matches('\\')
        .to_ascii_lowercase();

    if call
        .argument_list
        .arguments
        .iter()
        .any(|argument| !argument.is_positional() || argument.is_unpacked())
    {
        return ConstValue::Unknown;
    }

    let args: Vec<ConstValue> = call
        .argument_list
        .arguments
        .iter()
        .map(|argument| const_value(argument.value(), content, scope))
        .collect();

    match name.as_str() {
        "trim" => fold_trim(&args, |text, chars| {
            text.trim_matches(|c| chars.contains(&c)).to_string()
        }),
        "ltrim" => fold_trim(&args, |text, chars| {
            text.trim_start_matches(|c| chars.contains(&c)).to_string()
        }),
        "rtrim" => fold_trim(&args, |text, chars| {
            text.trim_end_matches(|c| chars.contains(&c)).to_string()
        }),
        "strtolower" => fold_case(&args, |c| c.to_ascii_lowercase()),
        "strtoupper" => fold_case(&args, |c| c.to_ascii_uppercase()),
        "ucfirst" => fold_ucfirst(&args),
        "str_replace" => fold_str_replace(&args),
        "preg_replace" => fold_preg_replace(&args),
        "implode" => fold_implode(&args),
        "sprintf" => fold_sprintf(&args),
        _ => ConstValue::Unknown,
    }
}

/// The characters `trim`/`ltrim`/`rtrim` strip without an explicit charlist:
/// space, tab, newline, carriage return, NUL, and vertical tab.
const DEFAULT_TRIM_CHARS: [char; 6] = [' ', '\t', '\n', '\r', '\0', '\u{0B}'];

fn fold_trim(args: &[ConstValue], trim: impl Fn(&str, &[char]) -> String) -> ConstValue {
    let (subject, charlist) = match args {
        [subject] => (subject, None),
        [subject, charlist] => (subject, Some(charlist)),
        _ => return ConstValue::Unknown,
    };
    let Some(subject) = subject.scalar() else {
        return ConstValue::Unknown;
    };
    let chars: Vec<char> = match charlist {
        None => DEFAULT_TRIM_CHARS.to_vec(),
        Some(charlist) => {
            let Some(charlist) = charlist.scalar() else {
                return ConstValue::Unknown;
            };
            // `"a..z"` is a character range, not a two-character set; folding
            // it as a literal set would strip the wrong characters.
            if charlist.contains("..") {
                return ConstValue::Unknown;
            }
            charlist.chars().collect()
        }
    };
    ConstValue::Scalar(trim(subject, &chars))
}

/// Fold `strtolower`/`strtoupper`, which PHP defines as ASCII-only: bytes
/// outside `A-Z`/`a-z` (including multibyte UTF-8 sequences) pass through
/// unchanged.
fn fold_case(args: &[ConstValue], map: impl Fn(char) -> char) -> ConstValue {
    let [subject] = args else {
        return ConstValue::Unknown;
    };
    let Some(text) = subject.scalar() else {
        return ConstValue::Unknown;
    };
    ConstValue::Scalar(
        text.chars()
            .map(|c| if c.is_ascii() { map(c) } else { c })
            .collect(),
    )
}

fn fold_ucfirst(args: &[ConstValue]) -> ConstValue {
    let [subject] = args else {
        return ConstValue::Unknown;
    };
    let Some(text) = subject.scalar() else {
        return ConstValue::Unknown;
    };
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => {
            let first = if first.is_ascii() {
                first.to_ascii_uppercase()
            } else {
                first
            };
            ConstValue::Scalar(format!("{first}{}", chars.as_str()))
        }
        None => ConstValue::Scalar(String::new()),
    }
}

/// The scalars a `str_replace`/`implode` argument reduces to: one for a
/// scalar value, one per element for a literal array, or `None` the moment
/// any element is not itself a known scalar.
fn scalar_list(value: &ConstValue) -> Option<Vec<&str>> {
    match value {
        ConstValue::Scalar(value) => Some(vec![value.as_str()]),
        ConstValue::Array(entries) => entries.iter().map(|(_, value)| value.scalar()).collect(),
        ConstValue::Unknown => None,
    }
}

/// Fold `str_replace($search, $replace, $subject)`.  `$search`/`$replace`
/// may each be a scalar or a literal array; PHP pairs array entries
/// positionally (padding a short `$replace` with `""`), applies each pair in
/// order — so a later search can act on an earlier replacement's output —
/// and, when `$replace` is a scalar but `$search` an array, broadcasts the
/// one replacement to every search term. The 4-argument form (`&$count`) is
/// not folded, since a call site passing it wants the by-reference count.
fn fold_str_replace(args: &[ConstValue]) -> ConstValue {
    let [search, replace, subject] = args else {
        return ConstValue::Unknown;
    };
    let Some(subject) = subject.scalar() else {
        return ConstValue::Unknown;
    };
    // A scalar `$search` paired with an array `$replace` has no PHP meaning
    // that reduces to a single replacement text.
    if matches!(search, ConstValue::Scalar(_)) && matches!(replace, ConstValue::Array(_)) {
        return ConstValue::Unknown;
    }
    let Some(searches) = scalar_list(search) else {
        return ConstValue::Unknown;
    };
    let mut replacements = if matches!(replace, ConstValue::Scalar(_)) {
        let Some(replacement) = scalar_list(replace) else {
            return ConstValue::Unknown;
        };
        vec![replacement[0]; searches.len()]
    } else {
        let Some(replacements) = scalar_list(replace) else {
            return ConstValue::Unknown;
        };
        replacements
    };
    while replacements.len() < searches.len() {
        replacements.push("");
    }

    let mut result = subject.to_string();
    for (needle, replacement) in searches.iter().zip(replacements.iter()) {
        // PHP leaves the subject untouched for an empty search term rather
        // than inserting the replacement at every position.
        if !needle.is_empty() {
            result = result.replace(needle, replacement);
        }
    }
    ConstValue::Scalar(result)
}

/// Fold `implode()`, accepting both `implode($array)` and either argument
/// order of `implode($glue, $array)` (PHP allows both, though the reversed
/// form is deprecated).
fn fold_implode(args: &[ConstValue]) -> ConstValue {
    let (glue, pieces) = match args {
        [ConstValue::Array(pieces)] => ("", pieces),
        [glue, ConstValue::Array(pieces)] => {
            let Some(glue) = glue.scalar() else {
                return ConstValue::Unknown;
            };
            (glue, pieces)
        }
        [ConstValue::Array(pieces), glue] => {
            let Some(glue) = glue.scalar() else {
                return ConstValue::Unknown;
            };
            (glue, pieces)
        }
        _ => return ConstValue::Unknown,
    };

    let mut joined = String::new();
    for (index, (_, value)) in pieces.iter().enumerate() {
        let Some(value) = value.scalar() else {
            return ConstValue::Unknown;
        };
        if index > 0 {
            joined.push_str(glue);
        }
        joined.push_str(value);
    }
    ConstValue::Scalar(joined)
}

/// Fold `preg_replace($pattern, $replacement, $subject)`.  Only the 3-argument,
/// all-scalar form folds: an array `$pattern`/`$subject`, or a `$limit`
/// argument, is left unknown rather than approximated.
///
/// A `$` or `\` in `$replacement` could be a PHP backreference (`$1`, `\1`),
/// whose syntax differs from the `regex` crate's own replacement syntax, so
/// that case is left unknown rather than substituted literally.
fn fold_preg_replace(args: &[ConstValue]) -> ConstValue {
    let [pattern, replacement, subject] = args else {
        return ConstValue::Unknown;
    };
    let Some(pattern) = pattern.scalar() else {
        return ConstValue::Unknown;
    };
    let Some(replacement) = replacement.scalar() else {
        return ConstValue::Unknown;
    };
    let Some(subject) = subject.scalar() else {
        return ConstValue::Unknown;
    };
    if replacement.contains(['$', '\\']) {
        return ConstValue::Unknown;
    }
    let Some(regex) = compile_pcre_pattern(pattern) else {
        return ConstValue::Unknown;
    };
    ConstValue::Scalar(regex.replace_all(subject, replacement).into_owned())
}

/// Compile a PHP-delimited pattern (e.g. `#^/xmas/#i`) with the `regex`
/// crate, or return `None` when the delimiter, a modifier, or the pattern
/// syntax itself is something the two engines might not agree on.
///
/// Only self-closing delimiters are supported (the common case: `/`, `#`,
/// `~`, …); a bracket delimiter (`(`, `{`, `[`, `<`) pairs with a different
/// closing character and can nest, which is not worth the complexity here.
fn compile_pcre_pattern(pattern: &str) -> Option<Regex> {
    let mut chars = pattern.chars();
    let delimiter = chars.next()?;
    if delimiter.is_alphanumeric()
        || delimiter == '\\'
        || delimiter.is_whitespace()
        || "([{<".contains(delimiter)
    {
        return None;
    }
    let rest = chars.as_str();
    let close = rest.rfind(delimiter)?;
    let body = &rest[..close];
    let modifiers = &rest[close + delimiter.len_utf8()..];

    let mut flags = String::new();
    for modifier in modifiers.chars() {
        match modifier {
            'i' | 'm' | 's' => flags.push(modifier),
            // PCRE's UTF-8 mode; the `regex` crate is Unicode-aware by
            // default, so there is nothing to translate.
            'u' => {}
            _ => return None,
        }
    }

    let full_pattern = if flags.is_empty() {
        body.to_string()
    } else {
        format!("(?{flags}){body}")
    };
    Regex::new(&full_pattern).ok()
}

/// Fold the `sprintf` specifiers this evaluator understands: `%%`, `%s`, and
/// `%d`, with the `-` (left-justify) and `0` (zero-pad) flags and a width.
/// Anything else — precision, a custom pad character, positional (`%1$s`)
/// arguments, other specifiers — is left unknown rather than guessed at.
fn fold_sprintf(args: &[ConstValue]) -> ConstValue {
    let [format, values @ ..] = args else {
        return ConstValue::Unknown;
    };
    let Some(format) = format.scalar() else {
        return ConstValue::Unknown;
    };

    let mut result = String::new();
    let mut chars = format.chars().peekable();
    let mut values = values.iter();
    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            result.push('%');
            continue;
        }

        let mut left_justify = false;
        let mut zero_pad = false;
        while let Some(&flag) = chars.peek() {
            match flag {
                '-' => left_justify = true,
                '0' => zero_pad = true,
                _ => break,
            }
            chars.next();
        }

        let mut width_digits = String::new();
        while let Some(&digit) = chars.peek() {
            if !digit.is_ascii_digit() {
                break;
            }
            width_digits.push(digit);
            chars.next();
        }
        let width: usize = width_digits.parse().unwrap_or(0);

        let Some(specifier) = chars.next() else {
            return ConstValue::Unknown;
        };
        let Some(value) = values.next().and_then(ConstValue::scalar) else {
            return ConstValue::Unknown;
        };
        let formatted = match specifier {
            's' => value.to_string(),
            'd' => match value.parse::<i64>() {
                Ok(number) => number.to_string(),
                Err(_) => return ConstValue::Unknown,
            },
            _ => return ConstValue::Unknown,
        };

        if formatted.len() >= width {
            result.push_str(&formatted);
            continue;
        }
        let pad_count = width - formatted.len();
        if left_justify {
            result.push_str(&formatted);
            result.extend(std::iter::repeat_n(' ', pad_count));
        } else if zero_pad {
            // The sign, if any, stays ahead of the padding zeros rather than
            // being pushed to the front of the whole field.
            let (sign, digits) = formatted.split_at(if formatted.starts_with('-') { 1 } else { 0 });
            result.push_str(sign);
            result.extend(std::iter::repeat_n('0', pad_count));
            result.push_str(digits);
        } else {
            result.extend(std::iter::repeat_n(' ', pad_count));
            result.push_str(&formatted);
        }
    }
    ConstValue::Scalar(result)
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
