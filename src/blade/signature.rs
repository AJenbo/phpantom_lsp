//! A Blade template's declared variable contract, and the other
//! declaration sources that sit below it.
//!
//! A template declares what it expects the way a function declares
//! parameters. The sources form a strict priority chain, matching the
//! model Bladestan (the PHPStan extension for Blade) enforces in CI, so
//! one set of annotations drives both the editor and the analyser:
//!
//! 1. an explicit `@bladestan-signature` docblock — the canonical contract;
//! 2. the first docblock before any template code — an implicit signature,
//!    so a codebase that already carries `@var` blocks needs no edits;
//! 3. `@props` / `@aware`, which type a component *body* from each entry's
//!    default value;
//! 4. the variables Blade itself injects into a component body
//!    (`$attributes`, `$slot`, `$componentName`);
//! 5. the members of the class backing a component view, which Blade
//!    merges into the view's data (see [`super::backing_class`]);
//! 6. the variables a service provider shares into every template or a view
//!    composer adds to the ones it targets (see [`super::shared_vars`]);
//! 7. types inferred from `view()` call sites (see
//!    [`super::call_site_inference`]), the lowest-priority fallback.
//!
//! Everything here works on the raw Blade source with byte-offset-stable
//! scans, so it can run before the preprocessor turns the template into
//! virtual PHP.

use std::borrow::Cow;

use mago_span::HasSpan;
use mago_syntax::cst::*;

use super::call_site_inference::string_literal_contents;

/// A template's declared type contract: the variable names (without `$`)
/// its signature docblock declares, in declaration order.
///
/// Only the names are recorded. The *types* stay where the author wrote
/// them: the docblock is left in the template body, where the forward
/// walker's standalone-`@var` handling reads it and carries the types
/// forward over the whole body. Recording them twice would just create a
/// second source to keep in sync.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TemplateSignature {
    pub(crate) vars: Vec<String>,
    /// Whether the contract came from an explicit `@bladestan-signature`
    /// docblock rather than being inferred from the first docblock.
    pub(crate) explicit: bool,
}

impl TemplateSignature {
    /// Whether the template declares no contract at all. An explicit but
    /// empty `@bladestan-signature` is still a declaration ("this template
    /// takes nothing"), so it does not count as absent.
    pub(crate) fn is_absent(&self) -> bool {
        !self.explicit && self.vars.is_empty()
    }

    pub(crate) fn declares(&self, name: &str) -> bool {
        self.vars.iter().any(|n| n == name)
    }
}

/// One entry of a `@props` or `@aware` declaration: the prop name and the
/// source text of its default expression, or `None` when the entry is a
/// bare name (a *required* prop, whose value the caller supplies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PropDecl {
    pub(crate) name: String,
    pub(crate) default: Option<String>,
}

/// The template's declared signature.
///
/// An explicit `@bladestan-signature` wins over the first-docblock
/// fallback; a docblock that carries no `@var` tag is not a signature.
pub(crate) fn extract(content: &str) -> TemplateSignature {
    // Scan with comments and `@verbatim` blanked so a signature parked in
    // either is invisible, exactly as it is to Blade. `@php` blocks stay
    // visible: that is where a signature docblock lives.
    let masked = mask_inert_regions(content, false);

    if let Some(range) = find_explicit_signature_docblock(&masked) {
        return TemplateSignature {
            vars: parse_var_tags(&content[range]),
            explicit: true,
        };
    }

    if let Some(range) = find_leading_docblock(&masked) {
        let vars = parse_var_tags(&content[range]);
        if !vars.is_empty() {
            return TemplateSignature {
                vars,
                explicit: false,
            };
        }
    }

    TemplateSignature::default()
}

/// Whether the template declares its own contract, and so manages its own
/// variable types rather than taking them from its call sites.
pub(crate) fn has_declared_signature(content: &str) -> bool {
    !extract(content).is_absent()
}

/// The entries of the template's `@props` directive, or `None` when it
/// declares none (or the argument is not a plain array literal, e.g. a
/// variable holding a dynamically built props array).
pub(crate) fn extract_props(content: &str) -> Option<Vec<PropDecl>> {
    extract_declared_entries(content, "props")
}

/// The entries of the template's `@aware` directive. `@aware` pulls a
/// value from the parent component's data, falling back to the declared
/// default, so its entries type the body exactly as `@props` do.
pub(crate) fn extract_aware(content: &str) -> Option<Vec<PropDecl>> {
    extract_declared_entries(content, "aware")
}

/// Whether the template uses a directive only a component can use, and so
/// receives Blade's component scope.
///
/// True even when the directive's argument is not a readable array literal
/// (`@props($dynamic)`): what makes the template a component is the
/// directive, not whether its list can be read.
pub(crate) fn declares_component_directive(content: &str) -> bool {
    let masked = mask_inert_regions(content, true);
    find_directive_args(&masked, "props").is_some()
        || find_directive_args(&masked, "aware").is_some()
}

fn extract_declared_entries(content: &str, directive: &str) -> Option<Vec<PropDecl>> {
    // A directive inside a comment, `@verbatim`, or `@php` block is inert
    // to Blade, so none of those declare props. `@php` is masked here (and
    // not for the signature scan) so a `@props` written inside a PHP string
    // literal cannot become the component's contract.
    let masked = mask_inert_regions(content, true);
    let args = find_directive_args(&masked, directive)?;
    parse_entries(&content[args])
}

/// Parse the argument text of a `@props`/`@aware` directive (everything
/// between its parentheses) into declared entries.
///
/// Returns `None` when the argument is not an array literal, so the caller
/// can leave the directive alone instead of inventing variables.
fn parse_entries(args: &str) -> Option<Vec<PropDecl>> {
    const PARSE_PREFIX: &str = "<?php ";
    let synthetic = format!("{PARSE_PREFIX}{args};");

    crate::parser::with_parsed_program(&synthetic, "blade_props_directive", |program, _content| {
        // `program.statements` includes the leading `<?php` opening tag as
        // its own statement, so the array literal is the first *expression*
        // statement, not necessarily `.first()`.
        let Some(Statement::Expression(expr_stmt)) = program
            .statements
            .iter()
            .find(|stmt| matches!(stmt, Statement::Expression(_)))
        else {
            return None;
        };
        let elements = match expr_stmt.expression {
            Expression::Array(array) => &array.elements,
            Expression::LegacyArray(array) => &array.elements,
            _ => return None,
        };

        let mut entries = Vec::new();
        for element in elements.iter() {
            match element {
                ArrayElement::KeyValue(kv) => {
                    let Expression::Literal(Literal::String(s)) = kv.key else {
                        continue;
                    };
                    let Some(name) = string_literal_contents(s) else {
                        continue;
                    };
                    let span = kv.value.span();
                    let start = (span.start.offset as usize).saturating_sub(PARSE_PREFIX.len());
                    let end = (span.end.offset as usize).saturating_sub(PARSE_PREFIX.len());
                    let Some(default) = args.get(start..end) else {
                        continue;
                    };
                    entries.push(PropDecl {
                        name,
                        default: Some(default.to_string()),
                    });
                }
                ArrayElement::Value(v) => {
                    // A bare entry (`@props(['visible'])`) is a *required*
                    // prop: the caller supplies its value, so it has no
                    // default to type it from.
                    let Expression::Literal(Literal::String(s)) = v.value else {
                        continue;
                    };
                    let Some(name) = string_literal_contents(s) else {
                        continue;
                    };
                    entries.push(PropDecl {
                        name,
                        default: None,
                    });
                }
                _ => continue,
            }
        }
        Some(entries)
    })
}

/// Blank out the regions Blade excludes from directive processing:
/// `{{-- … --}}` comments, `@verbatim … @endverbatim` blocks, and (on
/// request) raw `@php … @endphp` blocks.
///
/// Every masked byte becomes a space, newlines are kept, so the result has
/// the same length and line structure as the input and an offset found in
/// the masked text points at the same byte of the original.
///
/// `@php` masking is opt-in because the signature scan deliberately parks
/// its docblock inside a `@php` block and must keep seeing it; only the
/// directive scans (`@props`, `@aware`) want PHP blocks blanked.
pub(crate) fn mask_inert_regions(content: &str, mask_php_blocks: bool) -> Cow<'_, str> {
    let needs_scan = content.contains("{{--")
        || content.contains("@verbatim")
        || (mask_php_blocks && content.contains("@php"));
    if !needs_scan {
        return Cow::Borrowed(content);
    }

    let bytes = content.as_bytes();
    let mut out = content.as_bytes().to_vec();
    let mut i = 0;
    while i < bytes.len() {
        // `@@verbatim` / `@@php` are Blade's escapes for the literal text,
        // so they open no block.
        let escaped = i > 0 && bytes[i - 1] == b'@';
        let region = if bytes[i..].starts_with(b"{{--") {
            Some(end_of_region(bytes, i + 4, b"--}}"))
        } else if !escaped && bytes[i..].starts_with(b"@verbatim") {
            Some(end_of_region(bytes, i + 9, b"@endverbatim"))
        } else if mask_php_blocks && !escaped && bytes[i..].starts_with(b"@php") {
            Some(end_of_region(bytes, i + 4, b"@endphp"))
        } else {
            None
        };

        match region {
            Some(end) => {
                for byte in &mut out[i..end] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }
                i = end;
            }
            None => i += 1,
        }
    }

    // Only whole inert regions were overwritten with ASCII spaces, so the
    // result is still valid UTF-8.
    Cow::Owned(String::from_utf8(out).unwrap_or_else(|_| content.to_string()))
}

/// The end offset (exclusive) of a region that opened at `from`, i.e. just
/// past `terminator`. An unterminated region runs to end of input, matching
/// Blade's non-greedy patterns failing to match and the compiler leaving
/// the rest of the file alone.
fn end_of_region(bytes: &[u8], from: usize, terminator: &[u8]) -> usize {
    let mut i = from;
    while i + terminator.len() <= bytes.len() {
        if bytes[i..].starts_with(terminator) {
            return i + terminator.len();
        }
        i += 1;
    }
    bytes.len()
}

/// The byte range of the first docblock carrying a `@bladestan-signature`
/// marker. The marker may sit anywhere in the block, so a leading
/// description line is fine.
fn find_explicit_signature_docblock(masked: &str) -> Option<std::ops::Range<usize>> {
    let mut from = 0;
    while let Some(range) = next_docblock(masked, from) {
        if masked[range.clone()].contains("@bladestan-signature") {
            return Some(range);
        }
        from = range.end;
    }
    None
}

/// The byte range of the first docblock in the file, provided nothing but
/// whitespace, block delimiters, and imports precedes it.
///
/// A docblock that appears *after* template code is a local annotation
/// about one statement, not the template's contract, so it does not become
/// an implicit signature. Imports are allowed to precede it because a
/// signature written with short class names needs them.
fn find_leading_docblock(masked: &str) -> Option<std::ops::Range<usize>> {
    let mut rest = masked;
    let mut offset = 0;
    loop {
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        rest = trimmed;

        if rest.starts_with("/**") {
            let range = next_docblock(masked, offset)?;
            return Some(range);
        }

        // Skip the delimiters a signature block is wrapped in, plus any
        // `use` imports its type names rely on.
        let skipped = ["@php", "@endphp", "<?php", "?>"]
            .iter()
            .find_map(|token| rest.strip_prefix(token).map(|_| token.len()))
            .or_else(|| {
                rest.strip_prefix("use ")
                    .and_then(|after| after.find(';').map(|end| 4 + end + 1))
            })?;
        offset += skipped;
        rest = &rest[skipped..];
    }
}

/// The byte range of the first `/** … */` docblock at or after `from`.
fn next_docblock(content: &str, from: usize) -> Option<std::ops::Range<usize>> {
    let start = from + content[from..].find("/**")?;
    let end = content[start + 3..]
        .find("*/")
        .map(|i| start + 3 + i + 2)
        .unwrap_or(content.len());
    Some(start..end)
}

/// The name of every `@var Type $name` tag in a docblock (without `$`), in
/// source order and deduplicated.
fn parse_var_tags(docblock: &str) -> Vec<String> {
    let mut vars: Vec<String> = Vec::new();
    for (name, _) in crate::type_engine::variable::forward_walk::parse_var_docblock_pairs(docblock)
    {
        let name = name.trim_start_matches('$').to_string();
        if name.is_empty() || vars.contains(&name) {
            continue;
        }
        vars.push(name);
    }
    vars
}

/// The byte range of the argument text of `@<directive>( … )` — everything
/// between the outer parentheses, exclusive.
///
/// The scan balances parentheses and brackets and skips string literals, so
/// a default value containing either (`@props(['a' => foo(1), 'b' => ']'])`)
/// is spanned whole rather than truncated at its first closing paren.
fn find_directive_args(masked: &str, directive: &str) -> Option<std::ops::Range<usize>> {
    let bytes = masked.as_bytes();
    let needle = format!("@{directive}");
    let mut from = 0;
    while let Some(found) = masked[from..].find(&needle) {
        let at = from + found;
        from = at + needle.len();
        // `@@props` is an escaped literal, and `@propsFoo` is a different
        // directive.
        if at > 0 && bytes[at - 1] == b'@' {
            continue;
        }
        let mut i = from;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'(') {
            continue;
        }
        if let Some(end) = matching_paren(bytes, i) {
            return Some(i + 1..end);
        }
    }
    None
}

/// The offset of the `)` matching the `(` at `open`, or `None` when the
/// argument list is unterminated.
fn matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut i = open;
    while i < bytes.len() {
        let byte = bytes[i];
        match quote {
            Some(q) => {
                if byte == b'\\' {
                    i += 2;
                    continue;
                }
                if byte == q {
                    quote = None;
                }
            }
            None => match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' | b'[' => depth += 1,
                b')' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(content: &str) -> Vec<(String, Option<String>)> {
        extract_props(content)
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.name, p.default))
            .collect()
    }

    #[test]
    fn masks_comment_content_while_keeping_length() {
        let comment = "{{-- @extends('old') --}}";
        let blade = format!("<h1>Hi</h1>{comment}<p>Bye</p>");
        let masked = mask_inert_regions(&blade, false);
        assert_eq!(
            masked,
            format!("<h1>Hi</h1>{}<p>Bye</p>", " ".repeat(comment.len()))
        );
        assert_eq!(masked.len(), blade.len());
    }

    #[test]
    fn masks_verbatim_content() {
        let blade = "@verbatim\n@props(['a'])\n@endverbatim";
        let masked = mask_inert_regions(blade, false);
        assert!(!masked.contains("@props"));
        assert_eq!(masked.len(), blade.len());
    }

    #[test]
    fn masking_preserves_newlines_and_offsets() {
        let blade = "line1\n{{-- a\nb\nc --}}\nline5";
        let masked = mask_inert_regions(blade, false);
        assert_eq!(
            masked.matches('\n').count(),
            blade.matches('\n').count(),
            "line count must survive masking: {masked:?}"
        );
        assert!(masked.starts_with("line1\n") && masked.ends_with("\nline5"));
    }

    #[test]
    fn masking_leaves_real_directives_untouched() {
        let blade = "@extends('layout')\n@props(['a'])\n{{ $name }}";
        assert_eq!(mask_inert_regions(blade, true), blade);
    }

    #[test]
    fn escaped_verbatim_opens_no_block() {
        let blade = "@@verbatim @extends('layout') @@endverbatim";
        assert_eq!(mask_inert_regions(blade, false), blade);
    }

    #[test]
    fn masking_stops_at_the_first_closing_comment_marker() {
        let blade = "{{-- one --}}@extends('layout'){{-- two --}}";
        let masked = mask_inert_regions(blade, false);
        assert!(masked.contains("@extends('layout')"));
        assert!(!masked.contains("one") && !masked.contains("two"));
    }

    #[test]
    fn php_blocks_are_kept_unless_masking_is_requested() {
        // The signature scan relies on seeing its docblock inside a @php
        // block, so @php content is left untouched by default.
        let blade = "@php $x = \"@extends('layout')\"; @endphp";
        assert_eq!(mask_inert_regions(blade, false), blade);

        let masked = mask_inert_regions(blade, true);
        assert!(!masked.contains("@extends"));
        assert_eq!(masked.len(), blade.len());
    }

    #[test]
    fn escaped_php_opens_no_block() {
        let blade = "@@php @extends('layout') @@endphp";
        assert_eq!(mask_inert_regions(blade, true), blade);
    }

    #[test]
    fn masking_a_multibyte_region_keeps_the_rest_intact() {
        let blade = "{{-- æøå — dash --}}\n{{ $name }}";
        let masked = mask_inert_regions(blade, false);
        assert!(masked.ends_with("{{ $name }}"));
        assert_eq!(masked.len(), blade.len());
    }

    #[test]
    fn extracts_an_explicit_signature() {
        let blade = "@php\n/**\n * @bladestan-signature\n * @var string $name\n * @var \\App\\Models\\User $user\n */\n@endphp\n\n<h1>Hello {{ $name }}</h1>\n";
        let signature = extract(blade);
        assert!(signature.explicit);
        assert_eq!(signature.vars, vec!["name", "user"]);
    }

    #[test]
    fn extracts_an_implicit_signature_from_the_first_docblock() {
        let blade = "@php\n/** @var string $name */\n@endphp\n\n<h1>{{ $name }}</h1>\n";
        let signature = extract(blade);
        assert!(!signature.explicit);
        assert_eq!(signature.vars, vec!["name"]);
    }

    #[test]
    fn a_closure_signatures_own_param_name_does_not_shadow_the_declared_var() {
        // The closure type's own `$user` parameter name must not be
        // mistaken for the variable the `@var` tag declares.
        let blade = "@php\n/** @var \\Closure(\\App\\Models\\User $user): string $callback */\n@endphp\n{{ $callback() }}\n";
        let signature = extract(blade);
        assert_eq!(signature.vars, vec!["callback"]);
    }

    #[test]
    fn an_implicit_signature_may_follow_its_imports() {
        let blade =
            "@php\nuse App\\Models\\User;\n/** @var User $user */\n@endphp\n{{ $user->name }}\n";
        assert!(extract(blade).declares("user"));
    }

    #[test]
    fn a_docblock_after_template_code_is_not_a_signature() {
        let blade = "<h1>Hello</h1>\n@php\n/** @var string $name */\n@endphp\n";
        assert!(extract(blade).is_absent());
    }

    #[test]
    fn a_docblock_with_no_var_tag_is_not_a_signature() {
        let blade = "@php\n/** This is just a comment. */\n@endphp\n<h1>Hello</h1>\n";
        assert!(extract(blade).is_absent());
    }

    #[test]
    fn an_explicit_signature_wins_over_an_implicit_one() {
        let blade = "@php\n/**\n * @bladestan-signature\n * @var string $name\n */\n@endphp\n\n@php\n/** @var int $age */\n@endphp\n";
        let signature = extract(blade);
        assert!(signature.explicit);
        assert!(signature.declares("name") && !signature.declares("age"));
    }

    /// An explicit but empty signature still declares a contract ("this
    /// template takes nothing"), so call-site inference must stay out.
    #[test]
    fn an_empty_explicit_signature_is_still_a_declaration() {
        let blade = "@php\n/**\n * @bladestan-signature\n */\n@endphp\n@props(['caption' => ''])\n";
        let signature = extract(blade);
        assert!(signature.explicit && signature.vars.is_empty());
        assert!(!signature.is_absent());
        assert!(has_declared_signature(blade));
    }

    #[test]
    fn a_signature_inside_a_comment_is_ignored() {
        let blade = "{{--\n@php\n/**\n * @bladestan-signature\n * @var string $name\n */\n@endphp\n--}}\n\n<h1>Hello</h1>\n";
        assert!(extract(blade).is_absent());
    }

    #[test]
    fn a_signature_inside_verbatim_is_ignored() {
        let blade = "@verbatim\n@php\n/**\n * @bladestan-signature\n * @var string $name\n */\n@endphp\n@endverbatim\n";
        assert!(extract(blade).is_absent());
    }

    #[test]
    fn the_real_signature_wins_over_a_commented_one() {
        let blade = "{{--\n@php\n/**\n * @bladestan-signature\n * @var int $stale\n */\n@endphp\n--}}\n@php\n/**\n * @bladestan-signature\n * @var string $name\n */\n@endphp\n";
        let signature = extract(blade);
        assert!(signature.declares("name") && !signature.declares("stale"));
    }

    #[test]
    fn the_marker_may_follow_a_description_line() {
        let blade = "@php\n/**\n * The user profile card.\n *\n * @bladestan-signature\n * @var string $name\n */\n@endphp\n";
        let signature = extract(blade);
        assert!(signature.explicit);
        assert!(signature.declares("name"));
    }

    /// A declared name must be recognised whatever shape its type takes —
    /// generics, an intersection, a closure signature whose parameters carry
    /// their own `$` names, or a trailing prose description.
    #[test]
    fn recognises_names_declared_with_complex_types() {
        for (tag, name) in [
            ("@var array<string, int> $items", "items"),
            (
                "@var \\Illuminate\\Support\\Collection<int, \\App\\Models\\User> $users",
                "users",
            ),
            ("@var \\Countable&\\Iterator $iter", "iter"),
            ("@var string $title The page title", "title"),
        ] {
            let blade = format!("@php\n/**\n * @bladestan-signature\n * {tag}\n */\n@endphp\n");
            assert!(extract(&blade).declares(name), "tag: {tag}");
        }
    }

    #[test]
    fn props_entries_carry_their_default_expression() {
        assert_eq!(
            props("@props(['type' => 'info', 'title', 'count' => 0])"),
            vec![
                ("type".to_string(), Some("'info'".to_string())),
                ("title".to_string(), None),
                ("count".to_string(), Some("0".to_string())),
            ]
        );
    }

    #[test]
    fn props_span_a_call_default_and_a_nested_array() {
        // The first ")" belongs to foo(1) and the "]" to the nested array;
        // the scan must not stop at either, or every later prop is lost.
        assert_eq!(
            props("@props(['a' => foo(1), 'nested' => ['x' => 1], 'label' => 'hi'])")
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "nested", "label"]
        );
    }

    #[test]
    fn props_span_a_bracket_inside_a_string_default() {
        assert_eq!(
            props("@props(['a' => ']', 'b' => 2])")
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn props_span_multiple_lines() {
        assert_eq!(
            props("@props([\n    'caption' => '',\n])\n"),
            vec![("caption".to_string(), Some("''".to_string()))]
        );
    }

    #[test]
    fn no_props_directive_declares_nothing() {
        assert!(extract_props("<div>{{ $slot }}</div>").is_none());
    }

    #[test]
    fn a_non_literal_props_argument_declares_nothing() {
        assert!(extract_props("@props($dynamicProps)\n").is_none());
    }

    #[test]
    fn props_inside_a_comment_or_verbatim_are_ignored() {
        assert!(extract_props("{{-- @props(['type' => 'info']) --}}\n<div></div>").is_none());
        assert!(extract_props("@verbatim\n@props(['type' => 'info'])\n@endverbatim").is_none());
    }

    #[test]
    fn props_inside_a_php_block_are_ignored() {
        assert!(extract_props("@php\n$label = \"@props(['x'])\";\n@endphp\n").is_none());
    }

    #[test]
    fn real_props_win_over_a_commented_declaration() {
        assert_eq!(
            props("{{-- @props(['stale' => 1]) --}}\n@props(['type' => 'info'])"),
            vec![("type".to_string(), Some("'info'".to_string()))]
        );
    }

    #[test]
    fn aware_entries_are_read_like_props() {
        let entries = extract_aware("@aware(['color' => 'gray'])\n").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "color");
        assert_eq!(entries[0].default.as_deref(), Some("'gray'"));
    }
}
