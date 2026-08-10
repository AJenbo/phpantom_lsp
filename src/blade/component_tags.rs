//! Raw-text scanning of `<x-…>` component tags in Blade source: the
//! attributes each call site passes, the lowest-priority variable source
//! in the declaration chain documented in [`super::signature`].
//!
//! Component tags are HTML syntax, not PHP, so they cannot be read from
//! the mago AST the way a `view()` call site can (see
//! [`super::call_site_inference`]). The virtual PHP the preprocessor
//! emits only carries a bound attribute's *expression* forward, as a
//! `blade_directive(...)` call; the tag name and any plain string
//! attribute never appear in it at all. This module scans the original
//! Blade source directly instead.

use std::path::PathBuf;

use crate::Backend;
use crate::php_type::PhpType;

use super::signature::mask_inert_regions;

/// One anonymous-component registration in effect: the tag prefix it is
/// addressed under, and the view directory (dot notation) its templates
/// live in.
///
/// `Blade::anonymousComponentNamespace('components', 'webshop')` is
/// `("webshop", "components")`.  An empty prefix is the prefix-less
/// `Blade::anonymousComponentPath()` registration, whose templates every
/// un-namespaced tag can address.
pub(crate) type AnonymousNamespace = (String, String);

/// One `<x-…>` tag occurrence whose tag name matched one of the requested
/// component names.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ComponentTagCall {
    /// Plain (non-bound) attributes, already typed from their literal
    /// text: `(camelCase name, type)`.
    pub(crate) literal: Vec<(String, PhpType)>,
    /// Bound attributes (`:name="expr"` / the `:$var` shorthand):
    /// `(camelCase name, index into the file's `blade_directive(...)`
    /// call sequence)`.
    ///
    /// The preprocessor emits exactly one `blade_directive` call per
    /// bound attribute, on every HTML tag in the file, in document
    /// order — the same order this scan counts them in — so the index
    /// correlates the two without needing to translate byte offsets
    /// between the Blade source and the virtual PHP.
    pub(crate) bound: Vec<(String, usize)>,
}

impl Backend {
    /// The anonymous-component registrations in effect, as
    /// [`AnonymousNamespace`] pairs — what
    /// `ComponentTagCompiler::guessAnonymousComponentUsingNamespaces` and
    /// its path-keyed twin read before falling back to `components.`.
    ///
    /// A path registration names a directory on disk rather than a view
    /// prefix, so it is rewritten as the view directory that directory sits
    /// in.  One outside every configured view root is dropped: no template
    /// under it has a view name to be matched against in the first place.
    pub(crate) fn anonymous_component_namespaces(&self) -> Vec<AnonymousNamespace> {
        let (mut namespaces, paths) = {
            let resources = self.laravel_provider_resources.read();
            (
                resources.anonymous_component_namespaces.clone(),
                resources.anonymous_component_paths.clone(),
            )
        };
        if paths.is_empty() {
            return namespaces;
        }
        let roots: Vec<PathBuf> = self
            .laravel_view_roots()
            .into_iter()
            .map(|root| root.canonicalize().unwrap_or(root))
            .collect();
        for (prefix, path) in paths {
            let path = path.canonicalize().unwrap_or(path);
            let directory = roots.iter().find_map(|root| {
                let rel = path.strip_prefix(root).ok()?;
                Some(rel.to_string_lossy().replace(['/', '\\'], "."))
            });
            if let Some(directory) = directory {
                namespaces.push((prefix, directory));
            }
        }
        namespaces
    }
}

/// The bare tag names (without the `x-` prefix) a Blade file's own view
/// names make it addressable by: `components.brand.boxes` becomes
/// `brand.boxes` (so `<x-brand.boxes>` matches it), and a namespaced name
/// drops the `components.` segment after the namespace the same way
/// Laravel's `ComponentTagCompiler::guessViewName` inserts it —
/// `webshop::components.brand.boxes` is what `<x-webshop::brand.boxes>`
/// compiles to.
///
/// `anonymous` adds the directories a project registered a tag prefix for,
/// under which a view is addressed without the `components.` convention at
/// all: with `('webshop', 'components')` registered,
/// `components.pages.boxes` is also what `<x-webshop::pages.boxes>` names.
///
/// A view name that no rule makes a tag of contributes nothing.
pub(crate) fn component_tag_names(
    view_names: &[String],
    anonymous: &[AnonymousNamespace],
) -> Vec<String> {
    let mut tags = Vec::new();
    for name in view_names {
        if let Some(tag) = component_tag_for_view_name(name) {
            push_tag(tag, &mut tags);
        }
        for (prefix, directory) in anonymous {
            let Some(rest) = strip_view_directory(name, directory) else {
                continue;
            };
            push_tag(
                if prefix.is_empty() {
                    rest.to_string()
                } else {
                    format!("{prefix}::{rest}")
                },
                &mut tags,
            );
        }
    }
    tags
}

/// Add a tag and, for the view of an index component, the shorter tag it
/// also answers to.
///
/// Laravel falls back to `{view}.index` and to `{view}.{last segment}` when
/// a component's own view name does not exist, so `components.card.index`
/// and `components.card.card` are both what `<x-card>` reaches.
fn push_tag(tag: String, tags: &mut Vec<String>) {
    if let Some(shorter) = index_component_tag(&tag)
        && !tags.contains(&shorter)
    {
        tags.push(shorter);
    }
    if !tags.contains(&tag) {
        tags.push(tag);
    }
}

/// The tag an index component's view name is *also* addressable by:
/// `card.index` and `card.card` both answer to `<x-card>`.
fn index_component_tag(tag: &str) -> Option<String> {
    let (head, last) = tag.rsplit_once('.')?;
    if head.is_empty() || head.ends_with("::") {
        return None;
    }
    let previous = head.rsplit_once('.').map_or(head, |(_, seg)| seg);
    let previous = previous.rsplit_once("::").map_or(previous, |(_, seg)| seg);
    (last == "index" || last == previous).then(|| head.to_string())
}

/// The component name a view under a registered directory is addressed by,
/// or `None` for a view that does not sit under it.
fn strip_view_directory<'a>(view_name: &'a str, directory: &str) -> Option<&'a str> {
    if directory.is_empty() {
        return Some(view_name);
    }
    view_name.strip_prefix(directory)?.strip_prefix('.')
}

/// The tag name a view makes a component addressable by, or `None` for a
/// view outside the `components.` namespace, which no `<x-…>` tag names.
///
/// A namespaced view keeps its namespace (`nightshade::calendar`), and
/// drops a `components.` segment a package puts its component views under,
/// since the class behind them sits directly in the registered namespace.
pub(crate) fn component_tag_for_view_name(view_name: &str) -> Option<String> {
    match view_name.split_once("::") {
        Some((namespace, rest)) => {
            let bare = rest.strip_prefix("components.").unwrap_or(rest);
            Some(format!("{namespace}::{bare}"))
        }
        None => view_name.strip_prefix("components.").map(str::to_string),
    }
}

/// The inverse of [`component_tag_names`]: the view names a tag written as
/// `<x-{tag}>` can resolve to, in the order Laravel's
/// `ComponentTagCompiler::componentClass` tries them — the `components.`
/// convention first (with the `components.` prefix going after the
/// namespace when the tag has one), then each registered anonymous
/// directory whose prefix the tag is written under.
///
/// Each of those is tried as itself, then as its `.index` and repeated-last
/// segment forms, which is how an index component is addressed by its
/// directory alone.
pub(crate) fn view_names_for_component_tag(
    tag: &str,
    anonymous: &[AnonymousNamespace],
) -> Vec<String> {
    let mut names = Vec::new();
    push_view_name(guess_view_name(tag, "components"), tag, &mut names);
    for (prefix, directory) in anonymous {
        let Some(rest) = strip_tag_prefix(tag, prefix) else {
            continue;
        };
        push_view_name(guess_view_name(rest, directory), rest, &mut names);
    }
    names
}

/// Laravel's `ComponentTagCompiler::guessViewName`: the directory becomes
/// the view's prefix, and goes after the namespace when the component name
/// carries one.
fn guess_view_name(component: &str, directory: &str) -> String {
    if directory.is_empty() {
        return component.to_string();
    }
    match component.split_once("::") {
        Some((namespace, rest)) => format!("{namespace}::{directory}.{rest}"),
        None => format!("{directory}.{component}"),
    }
}

/// Add a candidate view name and the two an index component also answers
/// to, skipping the ones already recorded.
fn push_view_name(view_name: String, component: &str, names: &mut Vec<String>) {
    let last = component.rsplit(['.', ':']).next().unwrap_or(component);
    let mut candidates = vec![format!("{view_name}.index")];
    if !last.is_empty() {
        candidates.push(format!("{view_name}.{last}"));
    }
    candidates.insert(0, view_name);
    for candidate in candidates {
        if !names.contains(&candidate) {
            names.push(candidate);
        }
    }
}

/// The component name a tag addresses under a registered prefix, or `None`
/// for a tag written under a different one.
///
/// A prefix-less registration is reached by every tag that names no
/// namespace of its own; one written under some other namespace belongs to
/// that namespace instead.
fn strip_tag_prefix<'a>(tag: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return (!tag.contains("::")).then_some(tag);
    }
    tag.strip_prefix(prefix)?.strip_prefix("::")
}

/// The prefixes a component tag is written under, longest first so
/// neither shadows the other.
const TAG_PREFIXES: [&str; 2] = ["<livewire:", "<x-"];

/// Every distinct component tag referenced by an occurrence in `content`,
/// with the prefix it was written under (`x-alert`, `livewire:counter`).
///
/// Closing tags are skipped: an opening tag is what names a component, and
/// a self-closing one has no closing tag to find it by.
pub(crate) fn referenced_tags(content: &str) -> Vec<String> {
    if !TAG_PREFIXES.iter().any(|p| content.contains(p)) {
        return Vec::new();
    }
    let masked = mask_inert_regions(content, true);
    let bytes = masked.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let Some(prefix) = TAG_PREFIXES
            .iter()
            .find(|prefix| bytes[i..].starts_with(prefix.as_bytes()))
        else {
            i += 1;
            continue;
        };
        let name_start = i + prefix.len();
        let mut j = name_start;
        while j < bytes.len() && is_tag_name_char(bytes[j]) {
            j += 1;
        }
        if j > name_start {
            let tag = format!("{}{}", &prefix[1..], &masked[name_start..j]);
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        i = j.max(name_start);
    }
    tags
}

/// Every distinct tag name referenced by an `<x-…>` occurrence in
/// `content`. Used in the reverse direction from [`scan_component_tag_calls`]:
/// given a file that was just edited, which components does it call?
pub(crate) fn referenced_component_tags(content: &str) -> Vec<String> {
    referenced_tags(content)
        .into_iter()
        .filter_map(|tag| tag.strip_prefix("x-").map(str::to_string))
        .collect()
}

/// The `<x-…>` openings that could name one of `tag_names`, for the
/// cheap rejection test [`may_contain_component_tag`] applies.
///
/// Built once per component rather than per candidate file: a bulk
/// refresh pass tests one component's tags against every Blade file in
/// the workspace, and [`scan_component_tag_calls`] masks the whole file
/// before it can answer, which is far more than a rejection needs.
pub(crate) fn component_tag_needles(tag_names: &[String]) -> Vec<String> {
    tag_names.iter().map(|name| format!("<x-{name}")).collect()
}

/// Whether `content` is worth handing to [`scan_component_tag_calls`].
///
/// Conservative in the direction that matters: masking only ever removes
/// tags (a `<x-…>` inside a comment or a `@php` block), and a needle hit
/// on a longer tag name (`<x-card` for the needle `<x-car`) is settled by
/// the real scan, so a `true` here can still scan to nothing while a
/// `false` cannot hide a call.
pub(crate) fn may_contain_component_tag(content: &str, needles: &[String]) -> bool {
    content.contains("<x-")
        && needles
            .iter()
            .any(|needle| content.contains(needle.as_str()))
}

/// Scan `content` for `<x-…>` occurrences whose tag name (after the `x-`
/// prefix) is one of `tag_names`, and collect the attributes each passes.
///
/// Every bound attribute on *any* tag in the file is counted — not just a
/// matching one — because the preprocessor's `blade_directive` call
/// sequence includes them all; skipping a non-matching tag's bound
/// attributes here would desynchronise this scan's count against that
/// sequence.
///
/// `arguments` is the same partition the preprocessor applied to this
/// file: a bound attribute naming a parameter of the call its tag makes
/// is that call's argument, not a `blade_directive` of its own, so it is
/// not in the sequence to be counted. Both sides read the tag's target
/// from one place (a template's [`crate::blade::call_site_inference::BladeScope`]),
/// so the two cannot disagree about which attributes are arguments.
pub(crate) fn scan_component_tag_calls(
    content: &str,
    tag_names: &[String],
    arguments: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Vec<ComponentTagCall> {
    if tag_names.is_empty() || !content.contains("<x-") {
        return Vec::new();
    }
    let masked = mask_inert_regions(content, true);
    let bytes = masked.as_bytes();
    let mut results = Vec::new();
    let mut bound_index = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        match bytes.get(i + 1) {
            Some(b'/') => {
                // A closing tag has no attributes to scan.
                i = find_byte(&masked, i, b'>').map_or(bytes.len(), |end| end + 1);
                continue;
            }
            Some(c) if c.is_ascii_alphabetic() => {}
            _ => {
                i += 1;
                continue;
            }
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < bytes.len() && is_tag_name_char(bytes[j]) {
            j += 1;
        }
        let tag_name = &masked[name_start..j];
        let is_match = tag_name
            .strip_prefix("x-")
            .is_some_and(|bare| tag_names.iter().any(|n| n == bare));
        let consumed = arguments(tag_name).unwrap_or_default();
        let (end, call) = scan_tag_attributes(&masked, j, &consumed, &mut bound_index);
        if is_match {
            results.push(call);
        }
        i = end;
    }
    results
}

fn find_byte(content: &str, from: usize, needle: u8) -> Option<usize> {
    content.as_bytes()[from..]
        .iter()
        .position(|&b| b == needle)
        .map(|pos| from + pos)
}

fn is_tag_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b':' | b'_')
}

fn is_attr_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b':' | b'_' | b'@')
}

/// Parse the attribute list of a tag starting right after its name, up to
/// (and past) the tag's closing `>` or self-closing `/>`. Returns the
/// offset just past the close, plus the literal and bound attributes
/// found; `bound_index` is threaded through and bumped for every bound
/// attribute encountered, matching or not, to stay in sync with the
/// file-wide `blade_directive` call count.
fn scan_tag_attributes(
    masked: &str,
    start: usize,
    consumed: &[String],
    bound_index: &mut usize,
) -> (usize, ComponentTagCall) {
    let bytes = masked.as_bytes();
    let mut i = start;
    let mut call = ComponentTagCall::default();
    let literal = &mut call.literal;
    let bound = &mut call.bound;
    // A parameter can only be filled once, so a name an earlier attribute
    // of this tag already claimed is back to being ordinary markup — the
    // same rule `OpenComponentCall::take` applies on the emitting side.
    let mut unclaimed: Vec<&str> = consumed.iter().map(String::as_str).collect();
    let mut is_argument = |name: &str| match unclaimed.iter().position(|param| *param == name) {
        Some(index) => {
            unclaimed.remove(index);
            true
        }
        None => false,
    };

    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => break,
            Some(b'>') => {
                i += 1;
                break;
            }
            Some(b'/') if bytes.get(i + 1) == Some(&b'>') => {
                i += 2;
                break;
            }
            Some(b'/') => {
                i += 1;
                continue;
            }
            _ => {}
        }

        // `:$var` shorthand: bound, named after the variable.
        if bytes[i] == b':' && bytes.get(i + 1) == Some(&b'$') {
            let name_start = i + 2;
            let mut j = name_start;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if j > name_start {
                let name = masked[name_start..j].to_string();
                if is_argument(&name) {
                    // The tag's own call carries this one, so it is not in
                    // the `blade_directive` sequence at all.
                    i = j;
                    continue;
                }
                bound.push((name, *bound_index));
            }
            *bound_index += 1;
            i = j;
            continue;
        }

        // A single leading `:` marks a bound attribute; `::` is an
        // escaped literal colon (e.g. `::class`), and the attribute name
        // drops just the escape, not the real colon it protects.
        let is_bound = bytes[i] == b':' && bytes.get(i + 1) != Some(&b':');
        let name_start = if bytes[i] == b':' { i + 1 } else { i };
        let mut j = name_start;
        while j < bytes.len() && is_attr_name_char(bytes[j]) {
            j += 1;
        }
        if j == name_start {
            // Not an attribute token (e.g. a stray `<`); skip one byte so
            // a malformed tag cannot spin this loop forever.
            i += 1;
            continue;
        }
        let name = camel_case_attr_name(&masked[name_start..j]);
        i = j;

        if bytes.get(i) != Some(&b'=') {
            // A bare attribute (`disabled`) is `true`. A bare *bound*
            // attribute (`:disabled`, no `=`) never reaches the
            // preprocessor's `blade_directive` emission (it requires a
            // quoted value), so there is nothing to correlate for it.
            if !is_bound {
                literal.push((name, PhpType::bool()));
            }
            continue;
        }
        i += 1;

        let value_start = i;
        let quoted = matches!(bytes.get(i), Some(b'"') | Some(b'\''));
        let value_end = if quoted {
            let q = bytes[i];
            let mut k = i + 1;
            while k < bytes.len() && bytes[k] != q {
                k += 1;
            }
            k
        } else {
            let mut k = i;
            while k < bytes.len() && !bytes[k].is_ascii_whitespace() && bytes[k] != b'>' {
                k += 1;
            }
            k
        };

        if is_bound {
            // An unquoted bound value is never recognised by the
            // preprocessor either; only a quoted one produced a
            // `blade_directive` call to correlate against.
            if quoted && !is_argument(&name) {
                bound.push((name, *bound_index));
                *bound_index += 1;
            }
        } else {
            let raw = &masked[value_start + usize::from(quoted)..value_end];
            let ty = if raw.contains("{{") || raw.contains("{!!") {
                // A literal attribute embedding a Blade echo is not a
                // constant string; fall back to a generic type rather
                // than reporting the raw `{{ $expr }}` text as the value.
                PhpType::string()
            } else {
                PhpType::literal_string_value(raw)
            };
            literal.push((name, ty));
        }

        i = if quoted { value_end + 1 } else { value_end };
    }

    let end = i;
    (end, call)
}

/// Convert a kebab-case attribute name to the camelCase variable name
/// Blade exposes it as (`Illuminate\Support\Str::camel`). A PHP variable
/// name cannot contain a hyphen, so only the camelCase form of a
/// hyphenated attribute is ever accessible inside the template.
pub(crate) fn camel_case_attr_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '-' || ch == '_' {
            upper_next = true;
            continue;
        }
        if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(vars: &[(String, PhpType)]) -> Vec<&str> {
        vars.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// A project whose tags name no component the preprocessor could
    /// build, so no attribute of theirs is an argument.
    fn no_arguments(_tag: &str) -> Option<Vec<String>> {
        None
    }

    /// The registrations a project with no `anonymousComponent…` call has.
    const NONE: &[AnonymousNamespace] = &[];

    fn registered(prefix: &str, directory: &str) -> Vec<AnonymousNamespace> {
        vec![(prefix.to_string(), directory.to_string())]
    }

    #[test]
    fn component_tag_names_strips_the_components_prefix() {
        assert_eq!(
            component_tag_names(&["components.brand.boxes".to_string()], NONE),
            vec!["brand.boxes"]
        );
    }

    #[test]
    fn component_tag_names_strips_components_after_a_namespace() {
        assert_eq!(
            component_tag_names(&["webshop::components.brand.boxes".to_string()], NONE),
            vec!["webshop::brand.boxes"]
        );
        assert_eq!(
            component_tag_names(&["mail::message".to_string()], NONE),
            vec!["mail::message"]
        );
    }

    #[test]
    fn component_tag_names_skips_a_bare_non_component_view() {
        assert!(component_tag_names(&["emails.welcome".to_string()], NONE).is_empty());
    }

    /// `Blade::anonymousComponentNamespace('components', 'webshop')` makes
    /// the same template addressable under the registered prefix as well as
    /// by the un-registered `components.` convention.
    #[test]
    fn a_registered_prefix_adds_a_tag_for_the_directory_it_names() {
        assert_eq!(
            component_tag_names(
                &["components.pages.boxes".to_string()],
                &registered("webshop", "components"),
            ),
            vec!["pages.boxes", "webshop::pages.boxes"]
        );
    }

    /// A registration whose directory the view does not sit under names
    /// nothing about it.
    #[test]
    fn a_registration_for_another_directory_adds_no_tag() {
        assert_eq!(
            component_tag_names(
                &["components.pages.boxes".to_string()],
                &registered("webshop", "theme.components"),
            ),
            vec!["pages.boxes"]
        );
    }

    /// A prefix-less `anonymousComponentPath()` registration puts its whole
    /// directory behind bare tag names.
    #[test]
    fn a_prefix_less_registration_addresses_its_directory_bare() {
        assert_eq!(
            component_tag_names(&["ui.alert".to_string()], &registered("", "ui")),
            vec!["alert"]
        );
    }

    /// Laravel falls back to `{view}.index` and to the repeated-directory
    /// form, so both are what the directory's own tag reaches.
    #[test]
    fn an_index_component_answers_to_its_directory_alone() {
        assert_eq!(
            component_tag_names(&["components.card.index".to_string()], NONE),
            vec!["card", "card.index"]
        );
        assert_eq!(
            component_tag_names(&["components.card.card".to_string()], NONE),
            vec!["card", "card.card"]
        );
        assert_eq!(
            component_tag_names(&["components.index".to_string()], NONE),
            vec!["index"],
            "a component named `index` is not the index of anything"
        );
    }

    #[test]
    fn view_names_for_component_tag_round_trips() {
        assert_eq!(
            view_names_for_component_tag("brand.boxes", NONE),
            vec![
                "components.brand.boxes",
                "components.brand.boxes.index",
                "components.brand.boxes.boxes"
            ]
        );
        assert_eq!(
            view_names_for_component_tag("webshop::brand.boxes", NONE),
            vec![
                "webshop::components.brand.boxes",
                "webshop::components.brand.boxes.index",
                "webshop::components.brand.boxes.boxes"
            ]
        );
    }

    /// A tag written under a registered prefix names the view in the
    /// registered directory on top of the un-registered fallback, which is
    /// the one Laravel tries first.
    #[test]
    fn a_registered_prefix_adds_the_view_it_names() {
        assert_eq!(
            view_names_for_component_tag(
                "webshop::pages.boxes",
                &registered("webshop", "components")
            ),
            vec![
                "webshop::components.pages.boxes",
                "webshop::components.pages.boxes.index",
                "webshop::components.pages.boxes.boxes",
                "components.pages.boxes",
                "components.pages.boxes.index",
                "components.pages.boxes.boxes",
            ]
        );
    }

    /// A namespaced tag belongs to its own namespace, so a prefix-less path
    /// registration does not claim it.
    #[test]
    fn a_prefix_less_registration_ignores_a_namespaced_tag() {
        assert_eq!(
            view_names_for_component_tag("mail::message", &registered("", "ui")),
            vec![
                "mail::components.message",
                "mail::components.message.index",
                "mail::components.message.message"
            ]
        );
    }

    #[test]
    fn referenced_component_tags_collects_distinct_names() {
        let content = r#"<x-brand.boxes /><x-brand.boxes /><x-alert type="danger" />"#;
        let mut tags = referenced_component_tags(content);
        tags.sort();
        assert_eq!(tags, vec!["alert", "brand.boxes"]);
    }

    #[test]
    fn scans_a_literal_string_attribute() {
        let calls = scan_component_tag_calls(
            r#"<x-alert type="danger" />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(names(&calls[0].literal), vec!["type"]);
        assert_eq!(
            calls[0].literal[0].1,
            PhpType::literal_string_value("danger")
        );
    }

    #[test]
    fn scans_a_bare_boolean_attribute() {
        let calls = scan_component_tag_calls(
            r#"<x-alert disabled />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert_eq!(
            calls[0].literal,
            vec![("disabled".to_string(), PhpType::bool())]
        );
    }

    #[test]
    fn camel_cases_a_hyphenated_attribute_name() {
        let calls = scan_component_tag_calls(
            r#"<x-alert hair-analysis="x" />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert_eq!(names(&calls[0].literal), vec!["hairAnalysis"]);
    }

    #[test]
    fn a_bound_attribute_is_indexed_in_document_order() {
        let calls = scan_component_tag_calls(
            r#"<x-alert :hairAnalysis="$model->hairAnalysis" />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert_eq!(calls[0].bound, vec![("hairAnalysis".to_string(), 0)]);
    }

    #[test]
    fn a_non_matching_tags_bound_attribute_still_advances_the_index() {
        let calls = scan_component_tag_calls(
            r#"<div :class="$active"></div><x-alert :message="$msg" />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        // The `<div>` binding is index 0; `alert`'s own binding must
        // therefore be index 1, or the caller correlates it against the
        // wrong `blade_directive` call.
        assert_eq!(calls[0].bound, vec![("message".to_string(), 1)]);
    }

    #[test]
    fn a_shorthand_bound_attribute_is_named_after_its_variable() {
        let calls = scan_component_tag_calls(
            r#"<x-alert :$message />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert_eq!(calls[0].bound, vec![("message".to_string(), 0)]);
    }

    #[test]
    fn a_non_matching_tag_contributes_nothing() {
        let calls = scan_component_tag_calls(
            r#"<x-widget foo="bar" />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert!(calls.is_empty());
    }

    #[test]
    fn an_echo_interpolated_literal_falls_back_to_a_generic_string() {
        let calls = scan_component_tag_calls(
            r#"<x-alert title="Hello {{ $name }}" />"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert_eq!(calls[0].literal[0].1, PhpType::string());
    }

    /// A utility class carrying an arbitrary value (`max-h-[80vh]`) puts
    /// brackets inside a quoted attribute value. The scan tracks quotes, so
    /// neither the attribute nor the rest of the tag is cut short by them.
    #[test]
    fn a_bracket_in_a_quoted_attribute_value_does_not_truncate_the_tag() {
        let calls = scan_component_tag_calls(
            r#"<x-modal class="max-h-[80vh]" title="Save" /><x-alert type="danger" />"#,
            &["modal".to_string(), "alert".to_string()],
            &no_arguments,
        );
        assert_eq!(calls.len(), 2, "both tags must be seen");
        assert_eq!(names(&calls[0].literal), vec!["class", "title"]);
        assert_eq!(
            calls[0].literal[0].1,
            PhpType::literal_string_value("max-h-[80vh]")
        );
        assert_eq!(names(&calls[1].literal), vec!["type"]);
    }

    #[test]
    fn a_component_tag_inside_a_comment_is_ignored() {
        let calls = scan_component_tag_calls(
            r#"{{-- <x-alert type="danger" /> --}}"#,
            &["alert".to_string()],
            &no_arguments,
        );
        assert!(calls.is_empty());
    }
}
