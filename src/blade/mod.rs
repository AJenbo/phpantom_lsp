pub(crate) mod backing_class;
pub(crate) mod balance;
pub(crate) mod block_index;
pub(crate) mod blocks;
pub(crate) mod call_site_inference;
pub(crate) mod component_tags;
pub(crate) mod contract;
pub mod directive_completion;
pub mod directives;
pub(crate) mod discovery;
pub(crate) mod implicit_props;
pub(crate) mod layout;
pub(crate) mod outline;
pub mod preprocessor;
pub(crate) mod shared_vars;
pub(crate) mod signature;
pub mod source_map;
pub(crate) mod typed_receiver;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Number of lines the Blade preprocessor injects as a prologue
/// (<?php header, $errors declaration, $__env declaration, wrapper function, etc.).
pub const PROLOGUE_LINES: u32 = 6;

/// Name of the function the preprocessor wraps a template's body in, so
/// that collectors which only analyse function bodies see the template as
/// analysable code.
pub const WRAPPER_FUNCTION: &str = "__blade_template";

/// The marker functions the lowering calls to stand in for the directives
/// it cannot express as PHP, each with the return type its call sites
/// need: a directive that compiles into a condition needs a `bool`, the
/// rest are called as statements or as argument wrappers.
const MARKER_FUNCTIONS: &[(&str, Option<&str>)] = &[
    ("blade_directive", None),
    ("blade_bound_attr_directive", None),
    ("blade_view_directive", None),
    ("blade_each_directive", None),
    ("blade_can_directive", Some("bool")),
    ("blade_section_directive", Some("bool")),
    ("blade_stack_directive", Some("bool")),
    ("blade_push_if_directive", None),
    ("blade_custom_directive", Some("bool")),
];

/// One declaration of every [`MARKER_FUNCTIONS`] entry, as a stub the
/// whole project shares.
///
/// A template used to carry these declarations in its own prologue, which
/// made each of them a symbol as many times over as the project has
/// templates.  Registering the file once instead keeps the calls the
/// lowering emits resolvable without any template declaring anything.
static MARKER_STUB: LazyLock<String> = LazyLock::new(|| {
    use std::fmt::Write;

    let mut stub = String::from("<?php\n");
    for (name, return_type) in MARKER_FUNCTIONS {
        match return_type {
            Some(ty) => {
                let _ = writeln!(stub, "function {name}(...$args): {ty} {{ return true; }}");
            }
            None => {
                let _ = writeln!(stub, "function {name}(...$args) {{}}");
            }
        }
    }
    stub
});

/// Add the marker stub to a function-stub index under each name it
/// declares, so `find_or_load_function` resolves a marker call the same
/// way it resolves a call to a built-in.
pub(crate) fn with_marker_stubs(
    mut index: crate::ci_map::CiMap<&'static str>,
) -> crate::ci_map::CiMap<&'static str> {
    let stub: &'static str = &MARKER_STUB;
    for (name, _) in MARKER_FUNCTIONS {
        index.insert(*name, stub);
    }
    index
}

/// Whether `name` is a function the lowering declared for itself rather
/// than one the template wrote: the wrapper holding the template body, or
/// one of the marker functions its directives compile to.
///
/// They have to resolve, or every marker call the lowering emits reads as
/// a call to a function that does not exist, but they are boilerplate no
/// file wrote: nothing should offer them as a symbol of the project.
pub fn is_synthetic_function(name: &str) -> bool {
    name == WRAPPER_FUNCTION || MARKER_FUNCTIONS.iter().any(|(marker, _)| *marker == name)
}

/// The variable a component tag binds its instance to, matching the name
/// Blade's own compiled output uses.
///
/// No caller assigns it and a template that renders a component may never
/// read it, so it is exempt from the unused-variable diagnostic the way
/// `$loop` is.
pub const COMPONENT_VAR: &str = "component";

/// What Laravel instantiates for a component tag that names a template
/// with no class of its own.
pub const ANONYMOUS_COMPONENT: &str = "Illuminate\\View\\AnonymousComponent";

/// Prefix of the class the preprocessor wraps a template's body in when
/// the template renders with a component instance bound to `$this`.
const SCOPE_CLASS_PREFIX: &str = "__blade_scope_";

/// The name of the synthesized subclass whose method holds the body of a
/// template rendered with `fqn` bound to `$this`.
///
/// Deriving the name from the bound class keeps two templates backed by
/// different classes from colliding in the project-wide class index; two
/// templates backed by the *same* class do collide, but they synthesize
/// the identical class, so nothing is lost.
pub fn scope_class_name(fqn: &str) -> String {
    format!(
        "{SCOPE_CLASS_PREFIX}{}",
        fqn.trim_matches('\\').replace('\\', "_")
    )
}

/// Whether a class name is one [`scope_class_name`] produced.
pub fn is_scope_class(name: &str) -> bool {
    name.starts_with(SCOPE_CLASS_PREFIX)
}

/// Check whether a URI refers to a Blade template file.
pub fn is_blade_file(uri: &str) -> bool {
    uri.ends_with(".blade.php")
}

/// How Laravel renders a Blade template, which decides what it gets in
/// scope beyond the data its caller passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemplateKind {
    /// An ordinary view rendered through `view()` or `@include`.
    #[default]
    View,
    /// A component view, which additionally receives `$attributes` and
    /// `$slot`.
    Component,
}

/// Classify a Blade template from its path and source.
///
/// Either signal is conclusive on its own: the template sits in a
/// `components` directory (Laravel's anonymous-component convention, and
/// where a class-based component's default view lives), or it uses a
/// directive only a component can use.  The directive has to be a real one:
/// a `@props` inside a comment, `@verbatim`, or `@php` block is inert to
/// Blade and makes nothing a component.
pub fn template_kind(uri: &str, content: &str) -> TemplateKind {
    if uri.contains("/components/") || signature::declares_component_directive(content) {
        TemplateKind::Component
    } else {
        TemplateKind::View
    }
}

/// Discover Laravel Blade view directories from `config/view.php`.
///
/// Parses the `'paths'` array in the config file to extract directory
/// paths.  Falls back to `resources/views` if the config file is
/// missing or unparseable.  Returns only directories that exist.
pub fn discover_view_paths(workspace_root: &Path) -> Vec<PathBuf> {
    let config_path = workspace_root.join("config/view.php");
    let paths = if config_path.is_file() {
        parse_view_config_paths(&config_path, workspace_root)
    } else {
        Vec::new()
    };

    if paths.is_empty() {
        // Fallback: use the conventional Laravel view directory.
        let default = workspace_root.join("resources/views");
        if default.is_dir() {
            return vec![default];
        }
        return Vec::new();
    }

    paths
}

/// Parse `config/view.php` to extract the `'paths'` array entries.
///
/// Looks for string literals inside `'paths' => [...]` and resolves
/// `base_path('...')` calls relative to the workspace root.
fn parse_view_config_paths(config_path: &Path, workspace_root: &Path) -> Vec<PathBuf> {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    // Find the 'paths' => [...] section.
    let paths_idx = match content.find("'paths'") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let after = &content[paths_idx..];

    // Find the opening bracket.
    let bracket_start = match after.find('[') {
        Some(i) => i,
        None => return Vec::new(),
    };
    let bracket_end = match after[bracket_start..].find(']') {
        Some(i) => bracket_start + i,
        None => return Vec::new(),
    };
    let array_content = &after[bracket_start + 1..bracket_end];

    let mut result = Vec::new();

    // Match `base_path('...')`, `resource_path('...')`, `realpath(...)`
    // wrappers, and bare string literals.
    for segment in array_content.split(',') {
        let trimmed = segment.trim();
        if let Some(path) = extract_view_path_arg(trimmed) {
            let resolved = workspace_root.join(path);
            if resolved.is_dir() {
                result.push(resolved);
            }
        } else if let Some(path) = extract_string_literal(trimmed) {
            // Absolute or relative path literal.
            let resolved = if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                workspace_root.join(path)
            };
            if resolved.is_dir() {
                result.push(resolved);
            }
        }
    }

    result
}

/// Extract the workspace-relative directory from a `config/view.php`
/// path expression: `base_path('resources/views')`,
/// `resource_path('views')`, or either wrapped in `realpath(...)`.
///
/// `resource_path('X')` resolves to `resources/X` (and bare
/// `resource_path()` to `resources`), matching Laravel's helper.
fn extract_view_path_arg(s: &str) -> Option<String> {
    // Strip an optional `realpath(` wrapper.
    let inner = if let Some(rest) = s.strip_prefix("realpath(") {
        rest.strip_suffix(')')?.trim()
    } else {
        s
    };

    if let Some(rest) = inner.strip_prefix("base_path(") {
        let arg = rest.strip_suffix(')')?.trim();
        return extract_string_literal(arg).map(|p| p.to_string());
    }

    if let Some(rest) = inner.strip_prefix("resource_path(") {
        let arg = rest.strip_suffix(')')?.trim();
        if arg.is_empty() {
            return Some("resources".to_string());
        }
        return extract_string_literal(arg).map(|p| format!("resources/{p}"));
    }

    None
}

/// Extract content from a single- or double-quoted PHP string literal.
fn extract_string_literal(s: &str) -> Option<&str> {
    let s = s.trim();
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        Some(&s[1..s.len() - 1])
    } else {
        None
    }
}

use tower_lsp::lsp_types::{
    Hover, HoverContents, Location, MarkupContent, MarkupKind, Position, Range,
};

/// Column of the `{{`/`}}` escaped-echo delimiter the cursor is on, if any.
///
/// Shared by [`Backend::blade_echo_delimiter_hover`] and
/// [`Backend::blade_echo_delimiter_definition`] so the two features agree on
/// exactly which cursor positions count as "on the delimiter".
fn blade_echo_delimiter_col(line: &str, col: usize) -> Option<usize> {
    // Check if cursor is on `{{` (escaped echo open)
    if col < line.len()
        && line.get(col..col + 2) == Some("{{")
        && line.get(col..col + 3) != Some("{!!")
    {
        return Some(col);
    }
    // Also match if cursor is on the second `{` of `{{`
    if col > 0
        && line.get(col - 1..col + 1) == Some("{{")
        && (col < 2 || line.get(col - 1..col + 2) != Some("{!!"))
    {
        return Some(col - 1);
    }
    // `}}` closing delimiter
    if col < line.len()
        && line.get(col..col + 2) == Some("}}")
        && (col == 0 || line.as_bytes().get(col - 1) != Some(&b'!'))
    {
        return Some(col);
    }
    if col > 0
        && line.get(col - 1..col + 1) == Some("}}")
        && (col < 2 || line.as_bytes().get(col - 2) != Some(&b'!'))
    {
        return Some(col - 1);
    }

    None
}

impl crate::Backend {
    /// If the cursor is on a `{{` or `}}` Blade echo delimiter, return a
    /// hover describing the implicit `e()` call the delimiter compiles to.
    pub(crate) fn blade_echo_delimiter_hover(
        &self,
        uri: &str,
        position: Position,
    ) -> Option<Hover> {
        let content = self.get_file_content(uri)?;
        let line = content.lines().nth(position.line as usize)?;
        let start_col = blade_echo_delimiter_col(line, position.character as usize)?;
        Some(self.blade_e_hover(
            Position {
                line: position.line,
                character: start_col as u32,
            },
            2,
        ))
    }

    /// If the cursor is on a `{{` or `}}` Blade echo delimiter, return the
    /// go-to-definition target for the implicit `e()` call, so it agrees
    /// with [`Self::blade_echo_delimiter_hover`] on the same position
    /// instead of falling through to whatever PHP expression the
    /// blade-to-PHP offset mapping happens to land on.
    ///
    /// Returns `Some(None)` (suppressing go-to-definition, rather than
    /// disagreeing with the hover) when the cursor is on the delimiter but
    /// `e()` itself has no navigable declaration (e.g. it only resolved
    /// from an embedded stub). Returns `None` when the cursor is not on the
    /// delimiter at all, so the caller can fall through to ordinary
    /// go-to-definition.
    pub(crate) fn blade_echo_delimiter_definition(
        &self,
        uri: &str,
        position: Position,
    ) -> Option<Option<Location>> {
        let content = self.get_file_content(uri)?;
        let line = content.lines().nth(position.line as usize)?;
        blade_echo_delimiter_col(line, position.character as usize)?;
        Some(self.resolve_function_definition(&["e".to_string()]))
    }

    /// Build hover content for `{{ }}` (escaped echo via `e()`).
    fn blade_e_hover(&self, start: Position, len: u32) -> Hover {
        // Try to resolve the actual `e()` function from the project/stubs.
        let empty_use_map = std::collections::HashMap::new();
        let loader = self.function_loader_with(None, &empty_use_map, &None);
        let content = if let Some(func) = loader("e", 0) {
            crate::hover::hover_for_function(&func, None, None, false).contents
        } else {
            HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "Blade escaped echo. Output is passed through `e()` (`htmlspecialchars`).\n\n\
                    ```php\n<?php\nfunction e(mixed $value, bool $doubleEncode = true): string;\n```"
                    .to_string(),
            })
        };
        Hover {
            contents: content,
            range: Some(Range {
                start,
                end: Position {
                    line: start.line,
                    character: start.character + len,
                },
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_blade_file_by_extension() {
        assert!(is_blade_file("file:///app/views/welcome.blade.php"));
        assert!(!is_blade_file("file:///app/controllers/Home.php"));
    }

    #[test]
    fn test_is_blade_file_by_language_id() {
        let backend = crate::Backend::test_defaults();
        // Not blade by extension
        let uri = "file:///app/views/welcome.php";
        assert!(!backend.is_blade_file(uri));

        // Register via language_id
        backend.blade_uris.write().insert(uri.to_string());
        assert!(backend.is_blade_file(uri));
    }

    #[test]
    fn template_kind_reads_only_real_component_directives() {
        let view = "file:///resources/views/page.blade.php";
        assert_eq!(
            template_kind(view, "@props(['caption'])\n"),
            TemplateKind::Component
        );
        assert_eq!(
            template_kind(view, "@aware(['color'])\n"),
            TemplateKind::Component
        );
        // A dynamic list is still a component; the directive is the signal.
        assert_eq!(
            template_kind(view, "@props($dynamic)\n"),
            TemplateKind::Component
        );
        // Inert to Blade, so it makes nothing a component.
        assert_eq!(
            template_kind(view, "{{-- @props(['caption']) --}}\n"),
            TemplateKind::View
        );
        assert_eq!(
            template_kind(view, "@php\n$x = \"@props(['caption'])\";\n@endphp\n"),
            TemplateKind::View
        );
        // The path convention is conclusive on its own.
        assert_eq!(
            template_kind(
                "file:///resources/views/components/box.blade.php",
                "{{ $slot }}"
            ),
            TemplateKind::Component
        );
    }

    #[test]
    fn view_path_arg_variants() {
        assert_eq!(
            extract_view_path_arg("base_path('resources/views')").as_deref(),
            Some("resources/views")
        );
        assert_eq!(
            extract_view_path_arg("realpath(base_path('resources/backoffice/views'))").as_deref(),
            Some("resources/backoffice/views")
        );
        // resource_path('X') resolves relative to the resources dir.
        assert_eq!(
            extract_view_path_arg("resource_path('views')").as_deref(),
            Some("resources/views")
        );
        assert_eq!(
            extract_view_path_arg("resource_path('theme/views')").as_deref(),
            Some("resources/theme/views")
        );
        assert_eq!(
            extract_view_path_arg("resource_path()").as_deref(),
            Some("resources")
        );
        assert_eq!(extract_view_path_arg("some_other_call('x')"), None);
    }

    #[test]
    fn discover_view_paths_reads_custom_config() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir_all(root.join("resources/backoffice/views")).unwrap();
        std::fs::create_dir_all(root.join("resources/views")).unwrap();
        std::fs::write(
            root.join("config/view.php"),
            "<?php\nreturn [\n 'paths' => [\n  realpath(base_path('resources/backoffice/views')),\n  resource_path('views'),\n ],\n];\n",
        )
        .unwrap();

        let paths = discover_view_paths(root);
        assert!(paths.contains(&root.join("resources/backoffice/views")));
        assert!(paths.contains(&root.join("resources/views")));
    }

    #[test]
    fn discover_view_paths_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("resources/views")).unwrap();
        // No config/view.php present.
        let paths = discover_view_paths(root);
        assert_eq!(paths, vec![root.join("resources/views")]);
    }
}
