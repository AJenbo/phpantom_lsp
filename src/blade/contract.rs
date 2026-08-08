//! A template's merged contract, and what a caller may hand it.
//!
//! [`super::signature`] reads what one template declares. This module
//! answers the two questions a caller has to be judged against:
//!
//! * **What must I pass?** A template and the layouts it `@extends` render
//!   from one data array, so the contract is the union of their signatures,
//!   nearest declaration winning (a child may narrow what a layout declared,
//!   which is the covariant merge Bladestan performs). Names some other
//!   source supplies (a component's backing class, a provider's shared or
//!   composed data, Blade's own component scope) are recorded separately:
//!   they are part of the template's scope but no caller has to pass them.
//! * **What may I pass?** Blade hands a template's whole data array on to
//!   every template it renders from the same data, so a name the template
//!   itself never mentions may still be destined for a partial it
//!   `@include`s. The accepted set is therefore the transitive closure of
//!   the templates reachable from it, and it is only *closed* when every
//!   view name on the way is a readable literal.
//!
//! Both are computed from raw Blade source, so a template that has never
//! been opened or preprocessed can still be judged.

use std::collections::HashSet;

use crate::Backend;
use crate::php_type::PhpType;

use super::signature;

/// The variables Blade and Laravel construct themselves, in every
/// template's scope, whatever a call site writes down for them.
///
/// A template's declaration for one of these describes a framework object,
/// and the framework's own is what reaches it: `@include('row', ['loop' =>
/// $loop])` hands over Blade's loop object, not the caller's data. So they
/// are neither the caller's to supply nor the caller's to be judged on.
const AMBIENT_VARS: [&str; 5] = ["errors", "app", "__env", "__data", "loop"];

/// The variables Blade adds to a component's scope on top of its props.
const COMPONENT_SCOPE_VARS: [&str; 4] = ["attributes", "slot", "componentName", "component"];

/// Whether Blade builds `name` itself, so no call site decides its type.
pub(crate) fn is_framework_var(name: &str) -> bool {
    AMBIENT_VARS.contains(&name)
}

/// The directives that render another view from the calling template's own
/// data, paired with the argument index the view name sits at.
///
/// `@extends` is included: Laravel compiles it into a render of the layout
/// with the child's data, so the layout's declarations are the child's to
/// satisfy.
const RENDER_DIRECTIVES: [(&str, usize); 9] = [
    ("extends", 0),
    ("include", 0),
    ("includeIf", 0),
    ("includeIsolated", 0),
    ("includeFirst", 0),
    ("includeWhen", 1),
    ("includeUnless", 1),
    ("component", 0),
    ("each", 0),
];

/// What a template promises its callers.
pub(crate) struct TemplateContract {
    /// The declared variables in priority order: the template's own
    /// signature first, then what each layout above it adds.
    pub(crate) vars: Vec<(String, PhpType)>,
    /// Names something other than the caller supplies, so their absence
    /// from a call site is not a missing argument.
    pub(crate) supplied: HashSet<String>,
}

impl TemplateContract {
    /// The declared type of `name`, when the contract declares it.
    pub(crate) fn declared(&self, name: &str) -> Option<&PhpType> {
        self.vars
            .iter()
            .find(|(declared, _)| declared == name)
            .map(|(_, ty)| ty)
    }
}

/// The variable names a template and everything it renders from the same
/// data can make use of.
pub(crate) struct AcceptedNames {
    pub(crate) names: HashSet<String>,
    /// Whether the walk saw every template involved. A view name that is
    /// not a plain literal, or one no view root holds, leaves a template
    /// out of the set, and a name missing from an incomplete set proves
    /// nothing.
    pub(crate) closed: bool,
}

impl Backend {
    /// The contract a view name declares, or `None` when the template
    /// cannot be read or nothing on its `@extends` chain declares anything.
    ///
    /// A template with no contract anywhere above it leaves its callers
    /// nothing to be judged against, which is what keeps call-site
    /// validation opt-in. A child that declares nothing of its own is still
    /// held to its layout's contract: Blade renders the layout from the
    /// child's own data, so a caller short of a layout variable is just as
    /// short as one missing the child's.
    pub(crate) fn blade_template_contract(&self, view_name: &str) -> Option<TemplateContract> {
        let source = self.blade_view_source(view_name)?;
        let declarations = signature::declarations(&source);
        // Reading the layout chain means reading files, so skip it for the
        // templates that can contribute nothing either way.
        if declarations.is_empty() && signature::extract_extends(&source).is_none() {
            return None;
        }

        let names = self.blade_addressable_names(view_name);
        let qualify = self.blade_type_qualifier(view_name);

        let mut vars: Vec<(String, PhpType)> = Vec::new();
        let mut push = |name: String, ty: PhpType| {
            if vars.iter().any(|(declared, _)| declared == &name) {
                return;
            }
            vars.push((name, ty));
        };
        for (name, ty) in declarations {
            push(name, qualify(&ty));
        }
        // The layouts above render from the same data, so what they declare
        // the caller has to supply too. The nearest declaration wins, which
        // lets a child narrow a layout's type but never widen it.
        for (_, layout_source) in self.blade_layout_chain(&source) {
            for (name, ty) in signature::declarations(&layout_source) {
                push(name, qualify(&ty));
            }
        }
        if vars.is_empty() {
            return None;
        }

        let mut supplied: HashSet<String> = AMBIENT_VARS.iter().map(|n| n.to_string()).collect();
        if signature::declares_component_directive(&source) {
            supplied.extend(COMPONENT_SCOPE_VARS.iter().map(|n| n.to_string()));
        }
        // A prop with a default value stands on its own; a bare one is the
        // caller's to supply, so it stays out of the supplied set.
        for entries in [
            signature::extract_props(&source),
            signature::extract_aware(&source),
        ]
        .into_iter()
        .flatten()
        {
            for entry in entries {
                if entry.default.is_some() {
                    supplied.insert(entry.name);
                }
            }
        }
        let (backing, this_class) = self.blade_backing_class_vars(&names);
        supplied.extend(backing.into_iter().map(|(name, _)| name));
        if this_class.is_some() {
            // A Livewire view renders with the component bound, so a
            // signature naming `$this` is describing that binding.
            supplied.insert("this".to_string());
        }
        supplied.extend(
            self.blade_provider_vars(&names)
                .into_iter()
                .map(|(name, _)| name),
        );

        Some(TemplateContract { vars, supplied })
    }

    /// The names in scope in the template at `uri` when it renders another
    /// view, or `None` when the template declares no contract of its own.
    ///
    /// Blade hands an `@include` its parent's whole scope on top of the
    /// array the directive writes, so the parent's scope is what decides
    /// whether the partial is short of anything. Without a signature the
    /// parent's inbound data is whatever its own callers happened to pass,
    /// which is not knowable, and no conclusion can be drawn.
    pub(crate) fn blade_rendering_scope(&self, uri: &str) -> Option<HashSet<String>> {
        let source = self.get_file_content(uri)?;
        if !signature::has_declared_signature(&source) {
            return None;
        }
        let mut names: HashSet<String> = AMBIENT_VARS
            .iter()
            .chain(COMPONENT_SCOPE_VARS.iter())
            .map(|n| n.to_string())
            .collect();
        // Anything the template writes down itself is in scope for the
        // include: a `@foreach` binding, a `@php` assignment, or a name its
        // own signature declares.
        collect_variable_names(&source, &mut names);
        for (name, _) in signature::declarations(&source) {
            names.insert(name);
        }
        for (_, layout_source) in self.blade_layout_chain(&source) {
            for (name, _) in signature::declarations(&layout_source) {
                names.insert(name);
            }
        }
        let view_names = self.view_names_for_blade_uri(uri);
        let (backing, _) = self.blade_backing_class_vars(&view_names);
        names.extend(backing.into_iter().map(|(name, _)| name));
        names.extend(
            self.blade_provider_vars(&view_names)
                .into_iter()
                .map(|(name, _)| name),
        );
        Some(names)
    }

    /// Every name the template behind `view_name` is addressable by, for
    /// the sources that key off a template's names rather than one call
    /// site's spelling of it.
    fn blade_addressable_names(&self, view_name: &str) -> Vec<String> {
        let uri = self
            .blade_view_path(view_name)
            .and_then(|path| tower_lsp::lsp_types::Url::from_file_path(path).ok());
        let names = uri
            .map(|uri| self.view_names_for_blade_uri(uri.as_str()))
            .unwrap_or_default();
        if names.is_empty() {
            vec![view_name.to_string()]
        } else {
            names
        }
    }

    /// A resolver that fully qualifies the class names a template's
    /// signature docblock writes.
    ///
    /// The docblock is read from raw source, so a short name means whatever
    /// the template's own imports say it does; a caller comparing against
    /// it has neither those imports nor the template's namespace.
    fn blade_type_qualifier(&self, view_name: &str) -> impl Fn(&PhpType) -> PhpType + '_ {
        let uri = self
            .blade_view_path(view_name)
            .and_then(|path| tower_lsp::lsp_types::Url::from_file_path(path).ok());
        // An unknown URI yields an empty import map, which leaves a short
        // name to resolve globally — the same fallback the rest of the
        // Blade pipeline uses for a template it has never parsed.
        let file_ctx = self.file_context(uri.as_ref().map_or("", |uri| uri.as_str()));
        move |ty: &PhpType| {
            let class_loader = self.class_loader(&file_ctx);
            ty.resolve_names(&|name: &str| match class_loader(name) {
                Some(class) => format!("\\{}", class.fqn()),
                None => name.to_string(),
            })
        }
    }

    /// The variable names the template behind `view_name`, and every
    /// template it renders from the same data, can make use of.
    ///
    /// Blade passes a template's whole data array down through `@include`
    /// and `@extends`, so a name the template itself never mentions is not
    /// automatically unwanted: the partial three levels down may be the one
    /// that reads it.
    pub(crate) fn blade_accepted_names(&self, view_name: &str) -> AcceptedNames {
        let mut names: HashSet<String> = AMBIENT_VARS
            .iter()
            .chain(COMPONENT_SCOPE_VARS.iter())
            .map(|n| n.to_string())
            .collect();
        let mut closed = true;
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue = vec![view_name.to_string()];

        while let Some(name) = queue.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let Some(source) = self.blade_view_source(&name) else {
                // A name no view root holds leaves a hole in the walk: what
                // that template reads is simply unknown.
                closed = false;
                continue;
            };
            collect_variable_names(&source, &mut names);
            for (name, _) in signature::declarations(&source) {
                names.insert(name);
            }
            for entries in [
                signature::extract_props(&source),
                signature::extract_aware(&source),
            ]
            .into_iter()
            .flatten()
            {
                names.extend(entries.into_iter().map(|entry| entry.name));
            }
            // A template that reads its scope wholesale can use any name at
            // all, so nothing it is handed is provably unwanted.
            if source.contains("get_defined_vars") {
                closed = false;
            }
            let rendered = rendered_view_names(&source);
            closed &= rendered.closed;
            queue.extend(rendered.names);
        }

        AcceptedNames { names, closed }
    }
}

/// The view names a template renders from its own data, and whether every
/// one of them was readable.
struct RenderedViews {
    names: Vec<String>,
    closed: bool,
}

/// Scan a template for the directives that pass its data on to another
/// view, collecting the view names they name.
fn rendered_view_names(content: &str) -> RenderedViews {
    let masked = signature::mask_inert_regions(content, true);
    let bytes = masked.as_bytes();
    let mut names = Vec::new();
    let mut closed = true;

    let mut i = 0;
    while let Some(found) = masked[i..].find('@') {
        let at = i + found;
        i = at + 1;
        // `@@include` is Blade's escape for the literal text.
        if at > 0 && bytes[at - 1] == b'@' {
            continue;
        }
        let rest = &masked[at + 1..];
        let Some(&(directive, index)) = RENDER_DIRECTIVES.iter().find(|(directive, _)| {
            rest.strip_prefix(directive)
                .is_some_and(|after| !after.starts_with(|ch: char| ch.is_alphanumeric()))
        }) else {
            continue;
        };
        let mut open = at + 1 + directive.len();
        while open < bytes.len() && bytes[open].is_ascii_whitespace() {
            open += 1;
        }
        if bytes.get(open) != Some(&b'(') {
            // `@each` and friends always take arguments; without them the
            // directive is not a render at all.
            continue;
        }
        let Some(end) = signature::matching_paren(bytes, open) else {
            continue;
        };
        i = end;
        let args = &content[open + 1..end];
        let Some(argument) = split_top_level_args(args).into_iter().nth(index) else {
            closed = false;
            continue;
        };
        // `@includeFirst` takes a list of candidates and renders the first
        // that exists, so every entry is reachable.
        let literals = if directive == "includeFirst" {
            array_string_literals(argument)
        } else {
            signature::leading_string_literal(argument).map(|name| vec![name])
        };
        match literals {
            Some(found) => names.extend(found),
            None => closed = false,
        }
    }

    RenderedViews { names, closed }
}

/// Split a directive's argument text at its top-level commas, ignoring the
/// ones inside nested calls, arrays, and string literals.
fn split_top_level_args(args: &str) -> Vec<&str> {
    let bytes = args.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    let mut start = 0;
    let mut i = 0;
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
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => {
                    parts.push(&args[start..i]);
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    parts.push(&args[start..]);
    parts
}

/// The string literals of an array-literal argument, or `None` when the
/// argument is not one.
fn array_string_literals(argument: &str) -> Option<Vec<String>> {
    let inner = argument
        .trim()
        .strip_prefix('[')?
        .strip_suffix(']')
        .unwrap_or_default();
    let mut names = Vec::new();
    for entry in split_top_level_args(inner) {
        if entry.trim().is_empty() {
            continue;
        }
        names.push(signature::leading_string_literal(entry)?);
    }
    Some(names)
}

/// Add every `$name` the source mentions to `names`.
///
/// Deliberately crude: it scans the raw template, so an identifier written
/// in an HTML attribute or a comment counts too. The set is only ever used
/// to *accept* a name, so over-collecting costs a missed report, never a
/// false one.
fn collect_variable_names(content: &str, names: &mut HashSet<String>) {
    let bytes = content.as_bytes();
    let mut i = 0;
    while let Some(found) = content[i..].find('$') {
        let start = i + found + 1;
        i = start;
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end == start || bytes[start].is_ascii_digit() {
            continue;
        }
        names.insert(content[start..end].to_string());
        i = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(content: &str) -> (Vec<String>, bool) {
        let result = rendered_view_names(content);
        (result.names, result.closed)
    }

    #[test]
    fn collects_the_views_a_template_renders_from_its_own_data() {
        let blade = "@extends('layouts.app')\n@include('partials.header')\n@each('rows.item', $rows, 'row')\n";
        let (mut names, closed) = rendered(blade);
        names.sort();
        assert_eq!(names, vec!["layouts.app", "partials.header", "rows.item"]);
        assert!(closed);
    }

    #[test]
    fn the_conditional_include_family_names_its_view_second() {
        let (names, closed) = rendered("@includeWhen($ok, 'partials.flash', ['a' => 1])\n");
        assert_eq!(names, vec!["partials.flash"]);
        assert!(closed);
    }

    #[test]
    fn include_first_names_every_candidate() {
        let (names, closed) = rendered("@includeFirst(['custom.header', 'partials.header'])\n");
        assert_eq!(names, vec!["custom.header", "partials.header"]);
        assert!(closed);
    }

    #[test]
    fn a_dynamic_view_name_leaves_the_walk_open() {
        let (names, closed) = rendered("@include($partial)\n");
        assert!(names.is_empty());
        assert!(!closed);
    }

    #[test]
    fn an_inert_render_directive_names_no_view() {
        let (names, closed) = rendered("{{-- @include('partials.old') --}}\n<p>hi</p>\n");
        assert!(names.is_empty(), "{names:?}");
        assert!(closed);
    }

    #[test]
    fn an_escaped_directive_names_no_view() {
        let (names, closed) = rendered("@@include('partials.header')\n");
        assert!(names.is_empty(), "{names:?}");
        assert!(closed);
    }

    #[test]
    fn a_nested_array_argument_does_not_truncate_the_scan() {
        let (names, closed) =
            rendered("@include('partials.a', ['x' => [1, 2]])\n@include('partials.b')\n");
        assert_eq!(names, vec!["partials.a", "partials.b"]);
        assert!(closed);
    }

    #[test]
    fn collects_the_variables_a_template_mentions() {
        let mut names = HashSet::new();
        collect_variable_names(
            "<h1>{{ $title }}</h1>@foreach ($rows as $row)@endforeach",
            &mut names,
        );
        assert!(names.contains("title") && names.contains("rows") && names.contains("row"));
    }
}
