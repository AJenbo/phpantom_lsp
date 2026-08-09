use super::TemplateKind;
use super::directives::{match_directive, translate_directive};
use super::source_map::BladeSourceMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Html,
    Php,
    /// A raw `<?php` / `<?=` / `<?` tag embedded directly in the template
    /// (i.e. not via `@php`/`@endphp`). Content is passed through verbatim
    /// with no directive/echo scanning, and the mode ends at `?>`. The
    /// `bool` tracks whether the opening tag was a short-echo tag (`<?=`),
    /// which needs a trailing `;` injected before the closing `?>`.
    RawPhp(bool),
    DirectiveArgs(&'static str),
    SkipArgs(&'static str),
    Verbatim,
    /// The body of a `{{-- ... --}}` comment, emitted as a PHP `/* ... */`
    /// block. Comment text is neither PHP nor Blade, so nothing in it but the
    /// `--}}` terminator carries meaning: an apostrophe must not start a
    /// string literal (the scanner would hunt for a matching closing quote), a
    /// commented-out `}}`/`!!}` or an `@endphp` in prose must not end the
    /// comment, and a literal `*/` in the text must not close the emitted
    /// block. Any of those desyncs the rest of the file.
    Comment,
    /// The expression of a Blade component bound attribute
    /// (`:name="$expr"` or the `:$var` shorthand). The expression is
    /// emitted verbatim as a real PHP argument to `blade_directive(...)`
    /// so the forward walker sees the variables it uses; the surrounding
    /// tag markup stays masked. `Some(quote)` is the delimiting quote of
    /// a `:name="..."` value; `None` is the shorthand `:$var`, which ends
    /// at the first character that cannot be part of the variable name.
    BoundAttr(Option<char>),
    /// The parenthesised argument list of an `@use(...)` or `@inject(...)`
    /// directive. Unlike `DirectiveArgs`, the argument text is captured and
    /// transformed (rather than emitted verbatim) so the correct real PHP
    /// construct can be produced when the list closes.
    CaptureArgs(CapturedDirective),
}

/// Which directive is having its argument list captured by
/// [`Mode::CaptureArgs`]. Each has a different real-PHP translation:
/// `@use` becomes a top-level `use` import (hoisted out of the wrapper
/// function) and `@inject` becomes an inline `$var = app(service);`
/// assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturedDirective {
    Use,
    Inject,
}

pub fn preprocess(content: &str) -> (String, BladeSourceMap) {
    preprocess_with_vars(content, &[], TemplateKind::View, None)
}

/// The variables Blade puts in a component view's scope on top of the data
/// its caller passes: (name without `$`, docblock type, initialiser).
///
/// No caller passes these — Blade injects them when it renders the
/// component — so no signature or `@props` list can be expected to declare
/// them.
const COMPONENT_VARS: [(&str, &str, &str); 3] = [
    (
        "attributes",
        "\\Illuminate\\View\\ComponentAttributeBag",
        "new \\Illuminate\\View\\ComponentAttributeBag()",
    ),
    (
        "slot",
        "\\Illuminate\\View\\ComponentSlot",
        "new \\Illuminate\\View\\ComponentSlot()",
    ),
    ("componentName", "string", "''"),
];

/// A type string that is safe to place inside a one-line `/** @var … */`
/// docblock, or `mixed` when it is not.
///
/// Inferred types are rendered from expressions in caller files, so they
/// can carry arbitrary text: a literal-string type keeps its source form,
/// and PHP allows a real line break inside a quoted string. A line break
/// would add a prologue line the source map has to account for, and a
/// `*/` would close the docblock early and spill the rest into code.
/// Neither is worth reproducing faithfully, so such a type degrades to
/// `mixed` and the variable is still declared.
fn docblock_safe_type(type_string: &str) -> &str {
    let usable = !type_string.trim().is_empty()
        && !type_string.contains(['\n', '\r'])
        && !type_string.contains("*/");
    if usable { type_string } else { "mixed" }
}

/// Whether `name` (without the `$`) is something PHP can bind as a
/// variable.
///
/// A component tag's attributes become the template's variables, but an
/// attribute name is HTML, not PHP: `wire:model.live`, `@click` and
/// `x-on:keydown` are all legal there.  Blade hands the data to
/// `extract()`, which silently skips any key that is not a valid variable
/// name, so those attributes are reachable only through `$attributes`.
/// Declaring one anyway would emit `$wire:model.live = null;` into the
/// prologue and break the whole template with a syntax error.
fn is_php_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || !first.is_ascii())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || !ch.is_ascii())
}

/// Like [`preprocess`], but seeds the template's scope with externally
/// inferred variables (name without `$`, docblock type string).  Each
/// variable is declared in the top-level prologue with a `@var` docblock
/// and pulled into the wrapper function via `global`, the same mechanism
/// that makes `$errors`/`$__env` visible to every consumer (forward
/// walker, docblock backward scan, undefined-variable diagnostics).
///
/// Every variable the template does not assign itself is declared in the
/// prologue, following the priority chain in [`super::signature`]: the
/// template's own signature docblock wins, then `@props`/`@aware`, then the
/// variables Blade injects into a component body, then the externally
/// resolved variables the caller passes in (a backing class's members and
/// the layouts the template extends ahead of call-site inference, in the
/// order given).  A name declared by a higher source is not re-declared by
/// a lower one.
///
/// A signature-declared name is deliberately left out: its docblock stays
/// in the template body, where the forward walker reads it and carries the
/// type over the rest of the file.  Re-declaring it here would put a second
/// (and, for a `@props` default, a *wrong*) type in front of the author's.
///
/// `this_class` is the fully qualified name of the class a template renders
/// with bound to `$this` (Livewire hands its view the component instance).
/// `$this` cannot arrive through the declaration channel above, since PHP
/// allows neither `$this = …` nor `global $this`, so the body is wrapped in
/// a method of a synthesized subclass of that class instead of in a plain
/// function.
pub fn preprocess_with_vars(
    content: &str,
    injected_vars: &[(String, String)],
    kind: TemplateKind,
    this_class: Option<&str>,
) -> (String, BladeSourceMap) {
    let mut virtual_php = String::with_capacity(content.len() + 512);
    let mut source_map = BladeSourceMap::default();

    let signature = super::signature::extract(content);
    // (name without `$`, the PHP that declares it), in priority order.
    let mut declared: Vec<(String, String)> = Vec::new();
    let mut declare = |name: &str, decl: String| {
        if !is_php_variable_name(name)
            || signature.declares(name)
            || declared.iter().any(|(existing, _)| existing == name)
        {
            return;
        }
        declared.push((name.to_string(), decl));
    };

    // `@props`/`@aware` entries. A default value types its prop directly
    // (the expression is emitted verbatim, so anything the type engine can
    // resolve works); an entry without one is a *required* prop, whose
    // value the caller supplies, so it is declared `mixed` rather than
    // being invented as `null`.
    let entries = super::signature::extract_props(content)
        .into_iter()
        .chain(super::signature::extract_aware(content))
        .flatten();
    for entry in entries {
        let decl = match &entry.default {
            Some(default) => format!("${} = {};\n", entry.name, default),
            None => format!(
                "/** @var mixed ${name} */\n${name} = null;\n",
                name = entry.name
            ),
        };
        declare(&entry.name, decl);
    }

    if kind == TemplateKind::Component {
        for &(name, type_name, init) in &COMPONENT_VARS {
            declare(
                name,
                format!("/** @var {type_name} ${name} */\n${name} = {init};\n"),
            );
        }
    }

    for (name, type_string) in injected_vars {
        let type_string = docblock_safe_type(type_string);
        declare(
            name,
            format!("/** @var {type_string} ${name} */\n${name} = null;\n"),
        );
    }

    // ── Prologue ──
    virtual_php.push_str("<?php if (!function_exists('blade_directive')) { function blade_directive(...$args) {} function blade_view_directive(...$args) {} function blade_each_directive(...$args) {} }\n");
    // Where hoisted `@use` imports are spliced in once the whole
    // template has been scanned: still in the prologue, so they precede
    // every name they import (name resolution runs in source order and
    // an import written after a use of the name does not apply to it).
    let uses_insert_at = virtual_php.len();
    virtual_php.push_str("/** @var \\Illuminate\\Support\\ViewErrorBag $errors */\n");
    virtual_php.push_str("$errors = new \\Illuminate\\Support\\ViewErrorBag();\n");
    virtual_php.push_str("/** @var \\Illuminate\\View\\Factory $__env */\n");
    virtual_php.push_str("$__env = new \\Illuminate\\View\\Factory();\n");
    for (_, decl) in &declared {
        virtual_php.push_str(decl);
    }

    // Wrap the template body in a function so that diagnostic
    // collectors (which only analyse function/method bodies) treat
    // the Blade content as analysable code.  The closing brace is
    // appended after the main loop.  `$errors`/`$__env` (and every
    // declared variable) are assigned in the outer scope above, so
    // pull them in with `global` — otherwise every use of them inside
    // the wrapped function is a false-positive "undefined variable".
    //
    // A template that renders with a component instance bound gets a
    // method of a subclass of that component instead, so `$this` resolves
    // off the component the way it does in any other method body.  The
    // subclass is abstract: it exists only to carry the body, and a
    // concrete one would be reported for every method its parent leaves
    // abstract.
    if let Some(fqn) = this_class {
        virtual_php.push_str("abstract class ");
        virtual_php.push_str(&super::scope_class_name(fqn));
        virtual_php.push_str(" extends \\");
        virtual_php.push_str(fqn.trim_matches('\\'));
        virtual_php.push_str(" { public ");
    }
    virtual_php.push_str("function ");
    virtual_php.push_str(super::WRAPPER_FUNCTION);
    virtual_php.push_str("() { global $errors, $__env");
    for (name, _) in &declared {
        virtual_php.push_str(", $");
        virtual_php.push_str(name);
    }
    virtual_php.push_str(";\n");
    // Derive the prologue height from what was actually emitted rather
    // than assuming a line count per injected variable.  Every Blade
    // position is offset by this number, so a type string that carried
    // an unexpected line break would shift the whole file.
    source_map.prologue_lines = virtual_php.matches('\n').count() as u32;

    // `@use` imports cannot be emitted inline: the template body is wrapped
    // in `function __blade_template()`, and PHP `use` imports are only valid
    // at the top level. They are collected here and spliced into the
    // prologue as real top-level `use` statements once the scan is done.
    let mut hoisted_uses: Vec<String> = Vec::new();

    let mut in_php_directive_block = false;
    let mut mode = Mode::Html;
    let mut paren_depth = 0;
    let mut in_string: Option<char> = None;
    let mut is_escaped = false;
    // Whether the HTML scanner is currently between the `<` and `>` of a
    // tag, and (when inside a tag) whether it is inside a quoted attribute
    // value. Both persist across lines so multi-line tags are tracked
    // correctly. They gate recognition of `:name="$expr"` bound
    // attributes, which are only valid at attribute position inside a tag.
    let mut in_html_tag = false;
    let mut html_attr_string: Option<char> = None;
    // Text captured by `Mode::CaptureArgs` from lines before the current
    // one. A captured argument list (e.g. a multi-line `@props([...])`
    // array) can span several lines, but the per-line `buffer` below is
    // reset every iteration of the outer loop, so each line's contribution
    // is appended here (instead of being flushed into `processed`) until
    // the closing paren is reached and the whole span is transformed as
    // one unit.
    let mut capture_buffer = String::new();
    // Whether the bound attribute currently open in `Mode::BoundAttr` has
    // its closing quote on a later line, so the expression must stay open
    // at end of line instead of being closed off. Set when the attribute
    // opens; see `bound_attr_spans_lines`.
    let mut bound_attr_multiline = false;

    let lines: Vec<&str> = content.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        let mut processed = String::new();
        let mut adjustments = vec![(0, 0)]; // (blade_utf16_col, php_utf16_col)

        let mut current_utf16_col = 0;
        let line_chars: Vec<char> = line.chars().collect();
        let mut buffer = String::new();

        if mode == Mode::Html && in_php_directive_block {
            mode = Mode::Php;
        }

        let mut char_idx = 0;
        while char_idx < line_chars.len() {
            let ch = line_chars[char_idx];

            // Close a bound-attribute expression when its terminator is
            // reached. This must run before the generic string tracking
            // below, otherwise the closing `"` of a `:name="..."` value
            // would be mistaken for the start of a PHP string literal.
            if let Mode::BoundAttr(term) = mode {
                let at_end = match term {
                    Some(delim) => in_string.is_none() && ch == delim,
                    None => {
                        in_string.is_none()
                            && !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
                    }
                };
                if at_end {
                    flush_buffer(
                        &mut processed,
                        &mut buffer,
                        mode,
                        current_utf16_col,
                        &mut adjustments,
                    );
                    let start_suffix = utf16_count(&processed) as u32;
                    processed.push_str(");");
                    let end_suffix = utf16_count(&processed) as u32;
                    adjustments.push((current_utf16_col, start_suffix));
                    adjustments.push((current_utf16_col, end_suffix));
                    if term.is_some() {
                        // Consume the closing quote (masked tag markup).
                        char_idx += 1;
                        current_utf16_col += ch.len_utf16() as u32;
                        adjustments.push((current_utf16_col, end_suffix));
                    }
                    // The shorthand terminator (whitespace, `>`, `/`, …) is
                    // left for the HTML scanner to reprocess.
                    mode = Mode::Html;
                    continue;
                }
            }

            if mode != Mode::Html && mode != Mode::Comment {
                if let Some(quote) = in_string {
                    if is_escaped {
                        is_escaped = false;
                    } else if ch == '\\' {
                        is_escaped = true;
                    } else if ch == quote {
                        in_string = None;
                    }
                    buffer.push(ch);
                    char_idx += 1;
                    current_utf16_col += ch.len_utf16() as u32;
                    continue;
                } else if ch == '\'' || ch == '"' {
                    in_string = Some(ch);
                    buffer.push(ch);
                    char_idx += 1;
                    current_utf16_col += ch.len_utf16() as u32;
                    continue;
                }
            }

            // In Verbatim mode, skip all content until @endverbatim
            if mode == Mode::Verbatim {
                let remaining = &line_chars[char_idx..];
                let rest_str: String = remaining.iter().collect();
                if rest_str.starts_with("@endverbatim") {
                    let directive_len = "@endverbatim".len();
                    char_idx += directive_len;
                    current_utf16_col += directive_len as u32;
                    mode = Mode::Html;
                } else {
                    char_idx += 1;
                    current_utf16_col += ch.len_utf16() as u32;
                }
                continue;
            }

            let remaining = &line_chars[char_idx..];

            let mut match_len = 0;
            let mut replacement = String::new();
            let mut next_mode = mode;

            if mode == Mode::Html {
                if remaining.starts_with(&['{', '{']) {
                    let is_comment = remaining.starts_with(&['{', '{', '-', '-']);
                    let is_raw = remaining.starts_with(&['{', '{', '!', '!']);
                    replacement = if is_comment {
                        " /* ".to_string()
                    } else if is_raw {
                        " echo (".to_string()
                    } else {
                        " echo e(".to_string()
                    };
                    match_len = if is_comment || is_raw { 4 } else { 2 };
                    next_mode = if is_comment { Mode::Comment } else { Mode::Php };
                } else if remaining.starts_with(&['<', '?', 'p', 'h', 'p']) {
                    // Raw <?php tag embedded directly in the template (not via @php).
                    match_len = 5;
                    next_mode = Mode::RawPhp(false);
                } else if remaining.starts_with(&['<', '?', '=']) {
                    match_len = 3;
                    replacement = " echo ".to_string();
                    next_mode = Mode::RawPhp(true);
                } else if remaining.starts_with(&['<', '?', 'x', 'm', 'l']) {
                    // `<?xml ... ?>` is never a PHP open tag, regardless of
                    // `short_open_tag` — PHP special-cases it so XML
                    // declarations in templates aren't misparsed. Leave it
                    // as plain HTML.
                } else if remaining.starts_with(&['<', '?']) {
                    match_len = 2;
                    next_mode = Mode::RawPhp(false);
                } else if remaining.starts_with(&['@']) {
                    let rest_str: String = remaining[1..].iter().collect();
                    if let Some(directive) = match_directive(&rest_str) {
                        match_len = 1 + directive.len();
                        if directive == "php" {
                            let after_php = rest_str[3..].trim_start();
                            if !after_php.starts_with('(') {
                                in_php_directive_block = true;
                                next_mode = Mode::Php;
                                replacement = "".to_string();
                            } else {
                                replacement = format!(" {} ", translate_directive(directive));
                                next_mode = Mode::DirectiveArgs(";");
                                paren_depth = 0;
                            }
                        } else if directive == "endphp" {
                            replacement = "".to_string();
                            next_mode = Mode::Html;
                        } else if directive == "verbatim" {
                            replacement = "".to_string();
                            next_mode = Mode::Verbatim;
                        } else if directive == "empty" {
                            // @empty with parens = if(empty(...)):, without parens = forelse separator
                            let after_dir: String = rest_str[directive.len()..].chars().collect();
                            let after_trimmed = after_dir.trim_start();
                            if after_trimmed.starts_with('(') {
                                // `translate_directive("empty")` opens an
                                // extra unmatched `(` (`if(empty`), so the
                                // directive's own closing paren needs a
                                // second `)` before the `:`.
                                replacement = format!(" {} ", translate_directive(directive));
                                next_mode = Mode::DirectiveArgs("):");
                                paren_depth = 0;
                            } else {
                                replacement = " endforeach; if (false): ".to_string();
                                next_mode = Mode::Html;
                            }
                        } else if matches!(directive, "session" | "context") {
                            replacement = " if (true) ".to_string();
                            next_mode = Mode::SkipArgs(": $value = '';");
                            paren_depth = 0;
                        } else if directive == "error" {
                            replacement = " if (true) ".to_string();
                            next_mode = Mode::SkipArgs(": $message = '';");
                            paren_depth = 0;
                        } else if matches!(
                            directive,
                            "auth" | "guest" | "production" | "env" | "once"
                        ) {
                            // These are conditional blocks: if args present, skip them;
                            // if no args, emit directly.
                            let after_dir: String = rest_str[directive.len()..].chars().collect();
                            let after_trimmed = after_dir.trim_start();
                            if after_trimmed.starts_with('(') {
                                replacement = " if (true) ".to_string();
                                next_mode = Mode::SkipArgs(":");
                                paren_depth = 0;
                            } else {
                                replacement = " if (true): ".to_string();
                                next_mode = Mode::Html;
                            }
                        } else if matches!(directive, "foreach" | "forelse") {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::DirectiveArgs(
                                ": /** @var object{index: int, iteration: int, remaining: int, count: int, first: bool, last: bool, even: bool, odd: bool, depth: int, parent: ?object} $loop */ $loop = (object)[];",
                            );
                            paren_depth = 0;
                        } else if matches!(
                            directive,
                            "if" | "elseif" | "for" | "while" | "switch" | "case"
                        ) {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::DirectiveArgs(":");
                            paren_depth = 0;
                        } else if matches!(
                            directive,
                            "unless"
                                | "isset"
                                | "can"
                                | "cannot"
                                | "canany"
                                | "elsecan"
                                | "elsecannot"
                                | "elsecanany"
                                | "hasStack"
                                | "hasSection"
                                | "sectionMissing"
                        ) {
                            // `translate_directive` opens an extra unmatched
                            // `(` for all of these (`if(!` / `if(isset` /
                            // `if (blade_directive` / `elseif (blade_directive`),
                            // so the directive's own closing paren needs a
                            // second `)` before the `:`.
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::DirectiveArgs("):");
                            paren_depth = 0;
                        } else if matches!(
                            directive,
                            "extends"
                                | "extendsFirst"
                                | "section"
                                | "yield"
                                | "include"
                                | "includeIf"
                                | "includeWhen"
                                | "includeUnless"
                                | "includeFirst"
                                | "push"
                                | "prepend"
                                | "component"
                                | "componentFirst"
                                | "slot"
                                | "props"
                                | "aware"
                                | "fragment"
                                | "includeIsolated"
                                | "each"
                                | "pushIf"
                                | "pushOnce"
                                | "prependOnce"
                                | "method"
                                | "class"
                                | "style"
                                | "checked"
                                | "selected"
                                | "disabled"
                                | "readonly"
                                | "required"
                                | "stack"
                                | "json"
                                | "dump"
                                | "unset"
                                | "choice"
                                | "js"
                                | "dd"
                        ) {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::DirectiveArgs(";");
                            paren_depth = 0;
                        } else if directive == "lang" {
                            // `@lang` is either a bare block opener paired
                            // with `@endlang` (translation buffering that
                            // always runs, so it has nothing to type-check)
                            // or `@lang('key')` / `@lang(['key' => ...])`,
                            // a one-shot call whose argument is a real
                            // expression.
                            let after_dir: String = rest_str[directive.len()..].chars().collect();
                            if after_dir.trim_start().starts_with('(') {
                                replacement = format!(" {} ", translate_directive(directive));
                                next_mode = Mode::DirectiveArgs(";");
                                paren_depth = 0;
                            } else {
                                replacement = "".to_string();
                                next_mode = Mode::Html;
                            }
                        } else if matches!(directive, "vite" | "fonts") {
                            // Both take an optional argument list (Laravel
                            // defaults it to `()` when omitted), so a bare
                            // `@vite` / `@fonts` must not enter
                            // `DirectiveArgs`, which would otherwise consume
                            // the rest of the template hunting for a closing
                            // paren that was never opened.
                            let after_dir: String = rest_str[directive.len()..].chars().collect();
                            if after_dir.trim_start().starts_with('(') {
                                replacement = format!(" {} ", translate_directive(directive));
                                next_mode = Mode::DirectiveArgs(";");
                                paren_depth = 0;
                            } else {
                                replacement = "".to_string();
                                next_mode = Mode::Html;
                            }
                        } else if matches!(
                            directive,
                            "endif"
                                | "endforeach"
                                | "endfor"
                                | "endwhile"
                                | "endunless"
                                | "endisset"
                                | "endempty"
                                | "endswitch"
                                | "endforelse"
                                | "endsection"
                                | "endpush"
                                | "endprepend"
                                | "endcomponent"
                                | "endcomponentFirst"
                                | "endslot"
                                | "stop"
                                | "show"
                                | "append"
                                | "overwrite"
                                | "else"
                                | "default"
                                | "break"
                                | "endauth"
                                | "endguest"
                                | "endproduction"
                                | "endenv"
                                | "endsession"
                                | "endcontext"
                                | "enderror"
                                | "endonce"
                                | "endfragment"
                                | "endPushIf"
                                | "endPushOnce"
                                | "endPrependOnce"
                                | "csrf"
                                | "parent"
                                | "continue"
                                | "endcan"
                                | "endcannot"
                                | "endcanany"
                                | "endlang"
                                | "viteReactRefresh"
                        ) {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::Html; // These don't take args and return to HTML mode immediately
                        } else if matches!(directive, "use" | "inject") {
                            // `@use(...)` / `@inject(...)` need their
                            // argument(s) parsed into a real PHP construct, so
                            // the argument list is captured (not emitted
                            // verbatim) and transformed when it closes. Emit
                            // nothing inline until then.
                            let after_dir: String = rest_str[directive.len()..].chars().collect();
                            if after_dir.trim_start().starts_with('(') {
                                replacement = "".to_string();
                                next_mode = Mode::CaptureArgs(if directive == "use" {
                                    CapturedDirective::Use
                                } else {
                                    CapturedDirective::Inject
                                });
                                paren_depth = 0;
                            } else {
                                // Malformed (no argument list): mask and move on.
                                replacement = "".to_string();
                                next_mode = Mode::Html;
                            }
                        } else {
                            replacement = format!(" {}; ", translate_directive(directive));
                            next_mode = Mode::Php;
                        }
                    }
                } else if remaining.starts_with(&[':'])
                    && in_html_tag
                    && html_attr_string.is_none()
                    && (char_idx == 0 || line_chars[char_idx - 1].is_ascii_whitespace())
                    && remaining.get(1) != Some(&':')
                {
                    // A Blade component bound attribute at attribute
                    // position: `:name="$expr"`, `:name='$expr'`, or the
                    // `:$var` shorthand. The expression becomes a real PHP
                    // argument so its variables are seen; the rest of the
                    // tag stays masked. A leading `::` is an escaped literal
                    // colon and is left alone.
                    if remaining.get(1) == Some(&'$')
                        && remaining
                            .get(2)
                            .is_some_and(|c| c.is_ascii_alphabetic() || *c == '_')
                    {
                        match_len = 1;
                        replacement = " blade_directive(".to_string();
                        next_mode = Mode::BoundAttr(None);
                        bound_attr_multiline = false;
                    } else if let Some(open_len) = bound_attr_open_len(remaining) {
                        let quote = remaining[open_len - 1];
                        match_len = open_len;
                        replacement = " blade_directive(".to_string();
                        next_mode = Mode::BoundAttr(Some(quote));
                        bound_attr_multiline = bound_attr_spans_lines(
                            quote,
                            &remaining[open_len..],
                            &lines[line_idx + 1..],
                        );
                    }
                }
            } else if mode == Mode::Comment {
                // Inside a comment the only meaningful token is the `--}}`
                // terminator, which Blade requires to be contiguous. Comment
                // text is neither PHP nor Blade, so a commented-out echo's
                // `}}`/`!!}` and an `@endphp` written in prose must not end
                // it — treating either as the terminator would leave the
                // emitted `/*` open and desync the rest of the file.
                if remaining.starts_with(&['}', '}'])
                    && char_idx >= 2
                    && line_chars[char_idx - 2..].starts_with(&['-', '-'])
                {
                    replacement = " */ ".to_string();
                    match_len = 2;
                    next_mode = Mode::Html;
                }
            } else if mode == Mode::Php {
                if remaining.starts_with(&['}', '}']) || remaining.starts_with(&['!', '!', '}']) {
                    replacement = "); ".to_string();
                    match_len = if remaining.starts_with(&['!', '!', '}']) {
                        3
                    } else {
                        2
                    };
                    next_mode = Mode::Html;
                } else if remaining.starts_with(&['@', 'e', 'n', 'd', 'p', 'h', 'p']) {
                    in_php_directive_block = false;
                    next_mode = Mode::Html;
                    match_len = 7;
                    replacement = "".to_string();
                }
            } else if let Mode::RawPhp(needs_semicolon) = mode {
                if remaining.starts_with(&['?', '>']) {
                    replacement = if needs_semicolon {
                        "; ".to_string()
                    } else {
                        "".to_string()
                    };
                    match_len = 2;
                    next_mode = Mode::Html;
                }
            } else if let Mode::DirectiveArgs(suffix) = mode {
                // In Directive Args, we wait for balanced parentheses
                if ch == '(' {
                    paren_depth += 1;
                } else if ch == ')' {
                    paren_depth -= 1;
                    if paren_depth <= 0 {
                        buffer.push(')');
                        char_idx += 1;
                        current_utf16_col += 1;
                        flush_buffer(
                            &mut processed,
                            &mut buffer,
                            mode,
                            current_utf16_col,
                            &mut adjustments,
                        );

                        let start_suffix = utf16_count(&processed) as u32;
                        processed.push_str(suffix);
                        let end_suffix = utf16_count(&processed) as u32;

                        adjustments.push((current_utf16_col, start_suffix));
                        adjustments.push((current_utf16_col, end_suffix));

                        mode = Mode::Html;
                        continue;
                    }
                }
            } else if let Mode::SkipArgs(suffix) = mode {
                // Consume balanced parens without outputting them
                if ch == '(' {
                    paren_depth += 1;
                } else if ch == ')' {
                    paren_depth -= 1;
                    if paren_depth <= 0 {
                        char_idx += 1;
                        current_utf16_col += 1;
                        buffer.clear();

                        let start_suffix = utf16_count(&processed) as u32;
                        processed.push_str(suffix);
                        let end_suffix = utf16_count(&processed) as u32;

                        adjustments.push((current_utf16_col, start_suffix));
                        adjustments.push((current_utf16_col, end_suffix));

                        mode = Mode::Html;
                        continue;
                    }
                }
                char_idx += 1;
                current_utf16_col += ch.len_utf16() as u32;
                continue;
            } else if let Mode::CaptureArgs(kind) = mode {
                // Capture the argument text (in `buffer`, via the fall-through
                // push below) until the parens balance, then transform it.
                if ch == '(' {
                    paren_depth += 1;
                } else if ch == ')' {
                    paren_depth -= 1;
                    if paren_depth <= 0 {
                        char_idx += 1;
                        current_utf16_col += 1;
                        // `capture_buffer` holds any prior lines of this
                        // argument list; `buffer` holds the current line's
                        // text from the opening `(` (or line start) up to
                        // (but not including) this closing `)`. Together
                        // they are the argument text from the opening `(`
                        // to the closing `)`.
                        let mut raw = std::mem::take(&mut capture_buffer);
                        raw.push_str(&buffer);
                        buffer.clear();
                        let emitted = match kind {
                            CapturedDirective::Use => {
                                if let Some(stmt) = build_use_statement(&raw) {
                                    hoisted_uses.push(stmt);
                                }
                                // The import is hoisted; nothing inline.
                                String::new()
                            }
                            CapturedDirective::Inject => build_inject_statement(&raw),
                        };

                        let start_suffix = utf16_count(&processed) as u32;
                        processed.push_str(&emitted);
                        let end_suffix = utf16_count(&processed) as u32;

                        adjustments.push((current_utf16_col, start_suffix));
                        adjustments.push((current_utf16_col, end_suffix));

                        mode = Mode::Html;
                        in_string = None;
                        continue;
                    }
                }
            }

            if match_len > 0 || mode != next_mode {
                flush_buffer(
                    &mut processed,
                    &mut buffer,
                    mode,
                    current_utf16_col,
                    &mut adjustments,
                );

                if !replacement.is_empty() {
                    let start_php_col = utf16_count(&processed) as u32;
                    processed.push_str(&replacement);
                    let end_php_col = utf16_count(&processed) as u32;

                    // Boilerplate replacement: everything in the replacement
                    // (e.g. " echo e(") maps back to the START of the Blade
                    // tag.  This ensures that any semantic tokens Mago
                    // produces for the boilerplate (like the 'echo' keyword)
                    // have start == end in Blade space and are discarded.
                    adjustments.push((current_utf16_col, start_php_col));
                    adjustments.push((current_utf16_col, end_php_col));

                    char_idx += match_len;
                    current_utf16_col += match_len as u32;

                    // Anchor at the END of the Blade tag for subsequent content.
                    adjustments.push((current_utf16_col, end_php_col));
                } else {
                    // Empty replacement (e.g. @php)
                    adjustments.push((current_utf16_col, utf16_count(&processed) as u32));
                    char_idx += match_len;
                    current_utf16_col += match_len as u32;
                    adjustments.push((current_utf16_col, utf16_count(&processed) as u32));
                }

                mode = next_mode;
                continue;
            }

            // Track HTML tag / attribute-value state so bound attributes
            // are only recognized at attribute position (inside a tag, not
            // inside a quoted value). Colons in attribute values (e.g.
            // `href="mailto:x"`, `style="color:red"`) or in text between
            // tags (`10:30`) never satisfy `in_html_tag && !html_attr_string`.
            if mode == Mode::Html {
                match html_attr_string {
                    Some(q) if ch == q => html_attr_string = None,
                    Some(_) => {}
                    None => {
                        if ch == '<' {
                            // Enter a tag only when `<` begins an element
                            // (next char names a tag or is `/`), not on a
                            // stray `<` in text or a `< ` comparison.
                            let next = line_chars.get(char_idx + 1);
                            if next.is_none()
                                || next.is_some_and(|c| c.is_ascii_alphabetic() || *c == '/')
                            {
                                in_html_tag = true;
                            }
                        } else if ch == '>' {
                            in_html_tag = false;
                        } else if in_html_tag && (ch == '"' || ch == '\'') {
                            html_attr_string = Some(ch);
                        }
                    }
                }
            }

            buffer.push(ch);
            char_idx += 1;
            current_utf16_col += ch.len_utf16() as u32;
        }

        // A bound-attribute expression whose closing quote is on a later
        // line (what a formatter produces for a long array or argument
        // list) stays open: this line's PHP is flushed as-is and the next
        // line continues the same `blade_directive(` call. Cutting it off
        // here would truncate the expression mid-syntax.
        //
        // When the closing quote never appears at all the attribute is
        // malformed, and the call is closed off so only the attribute
        // itself is lost rather than the rest of the template.
        if let Mode::BoundAttr(_) = mode {
            flush_buffer(
                &mut processed,
                &mut buffer,
                mode,
                current_utf16_col,
                &mut adjustments,
            );
            if !bound_attr_multiline {
                processed.push_str(");");
                adjustments.push((current_utf16_col, utf16_count(&processed) as u32));
                mode = Mode::Html;
                in_string = None;
            }
        }

        if let Mode::CaptureArgs(_) = mode {
            // The argument list is still open at end of line: defer this
            // line's text instead of flushing it into `processed`, which
            // would leak a raw fragment into the virtual PHP before the
            // closing paren transforms the whole span as one unit.
            capture_buffer.push_str(&buffer);
            capture_buffer.push('\n');
            buffer.clear();
        } else {
            flush_buffer(
                &mut processed,
                &mut buffer,
                mode,
                current_utf16_col,
                &mut adjustments,
            );
        }

        virtual_php.push_str(&processed);
        virtual_php.push('\n');
        adjustments.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        source_map.adjustments.push(adjustments);
    }

    // An unterminated `{{--` leaves the emitted `/*` open, which would
    // swallow the wrapper's closing brace and make the whole file
    // unparseable. Close it so only the comment itself is lost.
    if mode == Mode::Comment {
        virtual_php.push_str(" */\n");
    }

    // Likewise for a multi-line bound attribute whose closing quote turned
    // out to be unreachable: leaving `blade_directive(` open would swallow
    // the wrapper's closing brace.
    if let Mode::BoundAttr(_) = mode {
        virtual_php.push_str(");\n");
    }

    // Close the wrapper function, and the class holding it when the body
    // was wrapped in a method.
    virtual_php.push_str(if this_class.is_some() { "} }\n" } else { "}\n" });

    // Splice the collected `@use` imports into the prologue as real
    // top-level `use` statements, and grow the prologue height by the
    // lines they add so every Blade position still maps correctly.
    if !hoisted_uses.is_empty() {
        let mut block = String::new();
        for stmt in &hoisted_uses {
            block.push_str(stmt);
            block.push('\n');
        }
        source_map.prologue_lines += hoisted_uses.len() as u32;
        virtual_php.insert_str(uses_insert_at, &block);
    }

    (virtual_php, source_map)
}

fn flush_buffer(
    processed: &mut String,
    buffer: &mut String,
    mode: Mode,
    current_utf16_col: u32,
    adjustments: &mut Vec<(u32, u32)>,
) {
    if buffer.is_empty() {
        return;
    }
    let blade_start = current_utf16_col.saturating_sub(utf16_count(buffer) as u32);

    if mode == Mode::Html {
        // HTML outside PHP/Directives — mask with spaces to maintain 1:1 utf-16 mapping.
        adjustments.push((blade_start, utf16_count(processed) as u32));

        for c in buffer.chars() {
            let len = c.len_utf16();
            for _ in 0..len {
                processed.push(' ');
            }
        }

        adjustments.push((current_utf16_col, utf16_count(processed) as u32));
    } else {
        // PHP content — 1:1 mapping
        adjustments.push((blade_start, utf16_count(processed) as u32));
        if mode == Mode::Comment {
            push_comment_text(processed, buffer);
        } else {
            processed.push_str(buffer);
        }
        adjustments.push((current_utf16_col, utf16_count(processed) as u32));
    }

    buffer.clear();
}

/// Copy Blade comment text into the emitted `/* ... */` block, blanking the
/// `/` of any `*/` in it. A literal `*/` in the text (common, since
/// commenting out a block of PHP is the usual reason to write a Blade
/// comment) would close the block early and turn the remainder of the
/// comment into live PHP. Replacing one character with a space rather than
/// escaping the sequence keeps the utf-16 columns aligned with the Blade
/// source.
fn push_comment_text(processed: &mut String, buffer: &str) {
    let mut after_star = false;
    for c in buffer.chars() {
        if after_star && c == '/' {
            processed.push(' ');
            after_star = false;
            continue;
        }
        after_star = c == '*';
        processed.push(c);
    }
}

fn utf16_count(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Trim surrounding whitespace and quote characters, matching Blade's
/// compiler (`trim($x, " '\"")`).
fn trim_quotes_and_space(s: &str) -> &str {
    s.trim_matches(|c: char| c == ' ' || c == '\'' || c == '"')
}

/// Whether `s` is a valid PHP identifier (variable name without the `$`).
fn is_php_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Translate the captured argument text of an `@use(...)` directive into a
/// real top-level `use` statement, mirroring Blade's `compileUse`. `raw` is
/// everything from the opening `(` up to (not including) the closing `)`.
///
/// Handles the plain form (`'App\Models\Post'`), the inline alias
/// (`'App\Models\Post as Article'`), the two-argument alias
/// (`'App\Models\Post', 'Article'`), grouped imports
/// (`'App\Models\{Post, Comment}'`), and the `function`/`const` modifiers.
/// Returns `None` when no importable path can be parsed.
fn build_use_statement(raw: &str) -> Option<String> {
    // Blade strips all parens, then trims whitespace/quotes.
    let expression: String = raw.chars().filter(|c| *c != '(' && *c != ')').collect();
    let expression = trim_quotes_and_space(&expression);

    let (path_with_modifier, alias) = if expression.contains('{') {
        // Grouped import: the braces are the argument, no alias.
        (expression.to_string(), String::new())
    } else {
        let mut segments = expression.splitn(2, ',');
        let path = trim_quotes_and_space(segments.next().unwrap_or("")).to_string();
        let alias = match segments.next() {
            Some(a) => format!(" as {}", trim_quotes_and_space(a)),
            None => String::new(),
        };
        (path, alias)
    };

    // Split off a `function ` / `const ` modifier if present.
    let (modifier, path) = if let Some(rest) = path_with_modifier.strip_prefix("function ") {
        ("function ", rest)
    } else if let Some(rest) = path_with_modifier.strip_prefix("const ") {
        ("const ", rest)
    } else {
        ("", path_with_modifier.as_str())
    };
    let path = path.trim().trim_start_matches('\\');

    if path.is_empty() {
        return None;
    }

    Some(format!("use {modifier}{path}{alias};"))
}

/// Translate the captured argument text of an `@inject(...)` directive into
/// an inline `$var = app(service);` assignment, mirroring Blade's
/// `compileInject`. `raw` is everything from the opening `(` up to (not
/// including) the closing `)`. Returns an empty string when the argument
/// list has no valid variable name or service.
fn build_inject_statement(raw: &str) -> String {
    let stripped: String = raw.chars().filter(|c| *c != '(' && *c != ')').collect();
    let mut segments = stripped.splitn(2, ',');
    let variable = trim_quotes_and_space(segments.next().unwrap_or(""));
    // The service keeps its own quotes; only surrounding whitespace is trimmed.
    let service = segments.next().unwrap_or("").trim();

    if variable.is_empty() || !is_php_identifier(variable) || service.is_empty() {
        return String::new();
    }

    format!(" ${variable} = app({service}); ")
}

/// If `rem` (starting at a `:`) opens a `:name="` or `:name='` bound
/// attribute, return the length (in chars) of that opening span, up to and
/// including the opening quote. Returns `None` when the syntax does not
/// match, so the `:` is left as ordinary masked tag markup.
fn bound_attr_open_len(rem: &[char]) -> Option<usize> {
    // rem[0] is the ':'.
    let mut i = 1;
    let name_start = i;
    while i < rem.len() && (rem[i].is_ascii_alphanumeric() || matches!(rem[i], '_' | '-' | '.')) {
        i += 1;
    }
    if i == name_start {
        return None; // no attribute name after the colon
    }
    if rem.get(i) != Some(&'=') {
        return None;
    }
    i += 1;
    match rem.get(i) {
        Some('"') | Some('\'') => Some(i + 1),
        _ => None,
    }
}

/// Whether a bound attribute delimited by `quote` closes on a line after
/// the one it opens on. `rest` is the remainder of the opening line (after
/// the opening quote) and `following` the lines after it.
///
/// `false` covers both the single-line case and a malformed attribute whose
/// closing quote never appears, so the caller closes the expression at end
/// of line in either case. A malformed attribute can still pick up a quote
/// from further down the template, but that markup is already broken.
fn bound_attr_spans_lines(quote: char, rest: &[char], following: &[&str]) -> bool {
    let mut in_string = None;
    let mut is_escaped = false;
    if scan_to_bound_attr_end(quote, rest.iter().copied(), &mut in_string, &mut is_escaped) {
        return false;
    }
    following
        .iter()
        .any(|line| scan_to_bound_attr_end(quote, line.chars(), &mut in_string, &mut is_escaped))
}

/// Scan one line's worth of a bound-attribute expression, reporting whether
/// the closing `quote` was reached. `in_string` and `is_escaped` carry the
/// PHP string state into the next line and must mirror how the main scan
/// tracks it, or the two disagree about where the attribute ends.
fn scan_to_bound_attr_end(
    quote: char,
    chars: impl Iterator<Item = char>,
    in_string: &mut Option<char>,
    is_escaped: &mut bool,
) -> bool {
    for ch in chars {
        match *in_string {
            _ if ch == quote && in_string.is_none() => return true,
            Some(delim) => {
                if *is_escaped {
                    *is_escaped = false;
                } else if ch == '\\' {
                    *is_escaped = true;
                } else if ch == delim {
                    *in_string = None;
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    *in_string = Some(ch);
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `<?xml ... ?>` is never a PHP open tag regardless of
    /// `short_open_tag`; PHP special-cases it so XML declarations and
    /// feeds embedded in templates aren't misparsed as PHP.
    #[test]
    fn test_preprocess_xml_declaration_is_not_a_php_tag() {
        let content = "<?xml version=\"1.0\" ?>\n<users>\n    <user>{{ $user }}</user>\n</users>\n";
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("version"),
            "<?xml ...?> should be masked as HTML, not parsed as PHP: {}",
            php
        );
        assert!(
            php.contains("echo e( $user )"),
            "{{ $user }} after the XML declaration should still translate normally: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_directive_with_string_parens() {
        let content = "@if(str_contains($val, \")\"))\n    {{ $val }}\n@endif";
        let (php, _) = preprocess(content);
        // It should properly wait for the outer parenthesis to close
        assert!(
            php.contains(" if (str_contains($val, \")\")):"),
            "Failed to parse parens inside string: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_foreach_loop_variable() {
        let content = "@foreach($items as $item)\n{{ $loop->first }}\n@endforeach\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("$loop"),
            "should inject $loop variable: {}",
            php
        );
        assert!(
            php.contains("object{index: int"),
            "should have typed $loop: {}",
            php
        );
        // $loop should be declared before its usage
        let loop_decl = php.find("$loop = (object)[];").unwrap();
        let loop_use = php.rfind("$loop").unwrap();
        assert!(
            loop_use > loop_decl,
            "$loop usage after declaration: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_errors_bag_visible_inside_template_function() {
        let content = "{{ $errors->has('name') }}";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("function __blade_template() { global $errors, $__env;"),
            "$errors/$__env must be pulled into the wrapper function's scope: {}",
            php
        );
    }

    /// A template that renders with a component instance bound wraps its
    /// body in a method of a subclass of that component, which is the only
    /// way `$this` can carry a type: PHP allows neither `$this = …` nor
    /// `global $this`.
    #[test]
    fn test_a_bound_this_wraps_the_body_in_a_subclass_method() {
        let (php, map) = preprocess_with_vars(
            "{{ $this->count }}",
            &[],
            TemplateKind::View,
            Some("App\\Livewire\\Counter"),
        );
        assert!(
            php.contains(
                "abstract class __blade_scope_App_Livewire_Counter \
                 extends \\App\\Livewire\\Counter \
                 { public function __blade_template() { global $errors, $__env;"
            ),
            "the body must sit in a method of a subclass of the component: {}",
            php
        );
        assert!(
            php.trim_end().ends_with("} }"),
            "the method and the class holding it must both close: {}",
            php
        );
        // The wrapper still occupies exactly one prologue line, so Blade
        // positions map the same as they do without a bound `$this`.
        assert_eq!(map.prologue_lines, super::super::PROLOGUE_LINES);
    }

    #[test]
    fn test_component_prologue_declares_attributes_and_slot() {
        let (php, map) = preprocess_with_vars(
            "<img {{ $attributes->merge(['class' => 'x']) }} />{{ $slot }}",
            &[],
            TemplateKind::Component,
            None,
        );
        assert!(
            php.contains("/** @var \\Illuminate\\View\\ComponentAttributeBag $attributes */")
                && php.contains("/** @var \\Illuminate\\View\\ComponentSlot $slot */"),
            "component variables must be declared with their framework types: {}",
            php
        );
        assert!(
            php.contains("/** @var string $componentName */"),
            "a component also knows its own name: {}",
            php
        );
        assert!(
            php.contains(
                "function __blade_template() { global $errors, $__env, $attributes, $slot, $componentName;"
            ),
            "component variables must be pulled into the wrapper scope: {}",
            php
        );
        // Three declarations of two lines each on top of the base prologue.
        assert_eq!(map.prologue_lines, super::super::PROLOGUE_LINES + 6);
    }

    #[test]
    fn test_plain_view_prologue_has_no_component_variables() {
        let (php, _) = preprocess("{{ $slot }}");
        assert!(
            !php.contains("$attributes = new") && !php.contains("$slot = new"),
            "a plain view must not receive component variables: {}",
            php
        );
    }

    /// A caller cannot pass `$attributes`, so a call-site inference that
    /// produced one must not overwrite the framework's own declaration.
    #[test]
    fn test_component_variables_are_not_overwritten_by_inferred_vars() {
        let (php, map) = preprocess_with_vars(
            "{{ $attributes }}",
            &[("attributes".to_string(), "string".to_string())],
            TemplateKind::Component,
            None,
        );
        assert!(
            !php.contains("$attributes = null;"),
            "the inferred declaration must be dropped: {}",
            php
        );
        assert_eq!(map.prologue_lines, super::super::PROLOGUE_LINES + 6);
    }

    #[test]
    fn test_preprocess_with_vars_injects_declarations() {
        let content = "{{ $user->name }}";
        let (php, map) = preprocess_with_vars(
            content,
            &[
                ("results".to_string(), "array<int, string>".to_string()),
                ("user".to_string(), "\\App\\Models\\User".to_string()),
            ],
            TemplateKind::View,
            None,
        );
        assert!(
            php.contains("/** @var array<int, string> $results */"),
            "injected @var declaration missing: {}",
            php
        );
        assert!(
            php.contains("/** @var \\App\\Models\\User $user */"),
            "injected @var declaration missing: {}",
            php
        );
        assert!(
            php.contains("function __blade_template() { global $errors, $__env, $results, $user;"),
            "injected variables must be pulled into the wrapper scope: {}",
            php
        );
        // Each injected variable adds a @var line and an assignment line.
        assert_eq!(map.prologue_lines, super::super::PROLOGUE_LINES + 4);

        // Round trip: blade (0,0) → php and back lands on the same line.
        let php_pos = map.blade_to_php(tower_lsp::lsp_types::Position {
            line: 0,
            character: 3,
        });
        assert_eq!(php_pos.line, map.prologue_lines);
        let back = map.php_to_blade(php_pos);
        assert_eq!(back.line, 0);
    }

    #[test]
    fn test_preprocess_without_vars_keeps_default_prologue() {
        let (_, map) = preprocess("{{ $x }}");
        assert_eq!(map.prologue_lines, super::super::PROLOGUE_LINES);
    }

    /// A literal-string type keeps its source form, and PHP allows a real
    /// line break inside a quoted string — so an inferred type can arrive
    /// with a newline in it. It must not add a prologue line (that would
    /// shift every position in the template) nor leave the `@var` docblock
    /// straddling two lines.
    #[test]
    fn test_preprocess_with_vars_multiline_type_does_not_shift_positions() {
        let (php, map) = preprocess_with_vars(
            "{{ $body }}",
            &[("body".to_string(), "'line1\nline2'".to_string())],
            TemplateKind::View,
            None,
        );
        assert_eq!(map.prologue_lines, super::super::PROLOGUE_LINES + 2);
        assert!(
            php.contains("/** @var mixed $body */"),
            "a multi-line type must degrade to mixed: {}",
            php
        );
        // The template body still starts exactly at the prologue height.
        let php_lines: Vec<&str> = php.lines().collect();
        assert!(
            php_lines[map.prologue_lines as usize].contains("$body"),
            "template line 0 must sit at prologue_lines: {}",
            php
        );
    }

    /// A `*/` inside an inferred type would close the docblock early and
    /// spill the remainder into code.
    #[test]
    fn test_preprocess_with_vars_type_cannot_close_the_docblock() {
        let (php, _) = preprocess_with_vars(
            "{{ $x }}",
            &[("x".to_string(), "'*/ evil()'".to_string())],
            TemplateKind::View,
            None,
        );
        assert!(
            php.contains("/** @var mixed $x */") && !php.contains("evil()"),
            "a type containing */ must degrade to mixed: {}",
            php
        );
    }

    /// A component tag's attribute names are HTML, so a caller writing
    /// `wire:model.live="…"` or `@click="…"` offers a name PHP cannot bind.
    /// Blade's `extract()` skips those keys, and so must the prologue:
    /// emitting `$wire:model.live = null;` would be a syntax error that
    /// takes the whole template down with it.
    #[test]
    fn test_preprocess_with_vars_skips_names_php_cannot_bind() {
        let (php, _) = preprocess_with_vars(
            "{{ $ok }}",
            &[
                ("wire:model.live".to_string(), "string".to_string()),
                ("@click".to_string(), "string".to_string()),
                ("ok".to_string(), "string".to_string()),
            ],
            TemplateKind::Component,
            None,
        );
        assert!(
            !php.contains("wire:model.live") && !php.contains("@click"),
            "an attribute name that is not a PHP variable must not be declared: {}",
            php
        );
        assert!(
            php.contains("$ok = null;") && php.contains(", $ok;"),
            "a valid name alongside it must still be declared: {}",
            php
        );
    }

    /// Inline attribute directives (`@class`, `@style`, `@checked`,
    /// `@selected`, `@disabled`, `@readonly`, `@required`) must consume
    /// their own argument list and return to HTML mode, not fall into the
    /// generic directive branch (which leaves everything after them
    /// parsed as PHP for the rest of the template).
    #[test]
    fn test_preprocess_attribute_directives_return_to_html() {
        let content = r#"<div @class(['a', 'b' => $cond]) id="x"></div>"#;
        let (php, _) = preprocess(content);
        // HTML content is masked with spaces (it is not meant to be parsed
        // as PHP), so the literal `id="x"` markup must NOT survive as raw
        // PHP source after the directive — that was the bug: the
        // generic-directive fallback left the parser in PHP mode for the
        // rest of the template, so `id="x"></div>` leaked through
        // unmasked and caused cascading syntax errors.
        assert!(
            !php.contains(r#"id="x""#),
            "content after @class(...) should be masked as HTML, not left as raw PHP: {}",
            php
        );
        assert!(
            php.contains("blade_directive (['a', 'b' => $cond]);"),
            "unexpected @class(...) translation: {}",
            php
        );
    }

    /// `@stack('name')` (render a named stack) must consume its own
    /// argument list and return to HTML mode, like `@yield`/`@section`,
    /// instead of falling into the generic directive branch.
    #[test]
    fn test_preprocess_stack_directive_returns_to_html() {
        let content = r#"<div>@stack('scripts')</div><p>after</p>"#;
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("after"),
            "content after @stack(...) should be masked as HTML, not left as raw PHP: {}",
            php
        );
        assert!(
            php.contains("blade_directive ('scripts');"),
            "unexpected @stack(...) translation: {}",
            php
        );
    }

    /// `@json($var)` must consume its argument as a real expression so a
    /// variable used only inside it is not silently invisible to the
    /// forward walker (it previously fell outside `match_directive`
    /// entirely, so `$var` in `@json($var)` was never emitted as PHP and
    /// the variable was reported as unused).
    #[test]
    fn test_preprocess_json_directive_consumes_argument() {
        let content = r#"<script>window.foo = @json($value);</script><p>after</p>"#;
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("after"),
            "content after @json(...) should be masked as HTML, not left as raw PHP: {}",
            php
        );
        assert!(
            php.contains("blade_directive ($value);"),
            "unexpected @json(...) translation: {}",
            php
        );
    }

    /// `@dump($var)` must likewise consume its argument as a real
    /// expression, for the same reason as `@json` above.
    #[test]
    fn test_preprocess_dump_directive_consumes_argument() {
        let content = r#"<div>@dump($value)</div><p>after</p>"#;
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("after"),
            "content after @dump(...) should be masked as HTML, not left as raw PHP: {}",
            php
        );
        assert!(
            php.contains("blade_directive ($value);"),
            "unexpected @dump(...) translation: {}",
            php
        );
    }

    /// `@can`/`@cannot`/`@canany` (and their `@elsecan*` counterparts) open a
    /// real `if`/`elseif` so the always-literal `@endif` that closes them
    /// stays balanced, while their arguments are still type-checked.
    #[test]
    fn test_preprocess_can_directive_opens_a_real_if() {
        let content = "@can('update', $post)\n<p>can</p>\n@elsecan('view', $post)\n<p>view</p>\n@endcan\n<p>after</p>";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (blade_directive ('update', $post)):"),
            "@can should open a balanced if with its arguments type-checked: {}",
            php
        );
        assert!(
            php.contains("elseif (blade_directive ('view', $post)):"),
            "@elsecan should open a balanced elseif with its arguments type-checked: {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endcan should close the if opened by @can: {}",
            php
        );
        assert!(
            !php.contains("after"),
            "content after @endcan should be masked as HTML, not left as raw PHP: {}",
            php
        );
    }

    /// `@hasStack`/`@hasSection`/`@sectionMissing` are always closed by a
    /// literal `@endif`, not a dedicated end-directive, so they must open a
    /// real `if` too (previously they degraded to a bare comment, leaving
    /// `@endif` dangling with no matching `if` and breaking the rest of the
    /// virtual PHP file).
    #[test]
    fn test_preprocess_has_stack_and_has_section_open_a_real_if() {
        let (php, _) = preprocess("@hasStack('scripts')\nx\n@endif\n<p>after</p>");
        assert!(
            php.contains("if (blade_directive ('scripts')):"),
            "@hasStack should open a balanced if with its argument type-checked: {}",
            php
        );
        assert!(
            !php.contains("after"),
            "content after @endif should be masked as HTML, not left as raw PHP: {}",
            php
        );

        let (php, _) = preprocess("@hasSection('content')\nx\n@endif\n<p>after</p>");
        assert!(
            php.contains("if (blade_directive ('content')):"),
            "@hasSection should open a balanced if with its argument type-checked: {}",
            php
        );

        let (php, _) = preprocess("@sectionMissing('content')\nx\n@endif\n<p>after</p>");
        assert!(
            php.contains("if (blade_directive ('content')):"),
            "@sectionMissing should open a balanced if with its argument type-checked: {}",
            php
        );
    }

    /// `@pushIf`/`@pushOnce`/`@prependOnce`/`@hasStack` used to fall through
    /// to `translate_directive`'s default `/* @directive */` comment, which
    /// left their arguments completely untyped.
    #[test]
    fn test_preprocess_push_if_and_push_once_consume_arguments() {
        let (php, _) = preprocess("@pushIf($condition, 'scripts')\nx\n@endPushIf\n<p>after</p>");
        assert!(
            php.contains("blade_directive ($condition, 'scripts');"),
            "@pushIf should type-check its arguments: {}",
            php
        );

        let (php, _) = preprocess("@pushOnce('scripts')\nx\n@endPushOnce\n<p>after</p>");
        assert!(
            php.contains("blade_directive ('scripts');"),
            "@pushOnce should type-check its argument: {}",
            php
        );

        let (php, _) = preprocess("@prependOnce('scripts')\nx\n@endPrependOnce\n<p>after</p>");
        assert!(
            php.contains("blade_directive ('scripts');"),
            "@prependOnce should type-check its argument: {}",
            php
        );
    }

    /// `@lang` is optional-argument: bare it opens a translation-buffering
    /// block paired with `@endlang` (nothing to type-check), and with an
    /// argument it is a one-shot call whose expression should be checked.
    #[test]
    fn test_preprocess_lang_directive_optional_argument() {
        let (php, _) = preprocess("@lang\n<p>x</p>\n@endlang\n<p>after</p>");
        assert!(
            !php.contains("after"),
            "bare @lang/@endlang should not swallow the rest of the template into raw PHP: {}",
            php
        );

        let (php, _) = preprocess("@lang($key)\n<p>after</p>");
        assert!(
            php.contains("blade_directive ($key);"),
            "@lang(...) should type-check its argument: {}",
            php
        );
    }

    /// `@vite`/`@fonts` take an optional argument list; a bare `@vite` must
    /// not send the scanner hunting for a closing paren that was never
    /// opened, which would swallow the rest of the template.
    #[test]
    fn test_preprocess_vite_and_fonts_optional_argument() {
        let (php, _) = preprocess("@vite\n<p>after</p>");
        assert!(
            !php.contains("after"),
            "bare @vite should not swallow the rest of the template into raw PHP: {}",
            php
        );

        let (php, _) = preprocess("@vite(['resources/js/app.js'])\n<p>after</p>");
        assert!(
            php.contains("blade_directive (['resources/js/app.js']);"),
            "@vite(...) should type-check its argument: {}",
            php
        );

        let (php, _) = preprocess("@fonts\n<p>after</p>");
        assert!(
            !php.contains("after"),
            "bare @fonts should not swallow the rest of the template into raw PHP: {}",
            php
        );
    }

    /// `@unset($var)` must compile to a real `unset(...)` statement, not a
    /// `blade_directive(...)` call — `unset` is a language construct and
    /// cannot be used as a function-call argument.
    #[test]
    fn test_preprocess_unset_directive() {
        let (php, _) = preprocess("@unset($value)\n<p>after</p>");
        assert!(
            php.contains("unset ($value);"),
            "@unset should compile to a real unset() statement: {}",
            php
        );
    }

    /// `@choice`/`@js`/`@dd`, previously unrecognised entirely (masked as
    /// inert HTML), must type-check their arguments like other expression
    /// directives.
    #[test]
    fn test_preprocess_choice_js_dd_directives_consume_arguments() {
        let (php, _) = preprocess("@choice('apples', $count)\n<p>after</p>");
        assert!(
            php.contains("blade_directive ('apples', $count);"),
            "@choice should type-check its arguments: {}",
            php
        );

        let (php, _) = preprocess("@js($data)\n<p>after</p>");
        assert!(
            php.contains("blade_directive ($data);"),
            "@js should type-check its argument: {}",
            php
        );

        let (php, _) = preprocess("@dd($value)\n<p>after</p>");
        assert!(
            php.contains("blade_directive ($value);"),
            "@dd should type-check its argument: {}",
            php
        );
    }

    /// A bound attribute on a component tag (`:src="$image"`) must emit
    /// its expression as real PHP so the variable is seen by the forward
    /// walker (otherwise a variable used only there is a false-positive
    /// "unused variable"). The surrounding tag markup stays masked.
    #[test]
    fn test_preprocess_bound_attribute_emits_expression() {
        let content = r#"<x-img.size :src="$image" alt="x" />"#;
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive($image);"),
            "bound attribute expression should be emitted as PHP: {}",
            php
        );
        // The tag name and other attribute markup must not leak as raw PHP.
        assert!(
            !php.contains("x-img.size"),
            "tag markup should stay masked: {}",
            php
        );
        assert!(
            !php.contains(r#"alt="x""#),
            "unbound attribute markup should stay masked: {}",
            php
        );
    }

    /// Package tag namespaces (`<livewire:...>`) and method-call
    /// expressions inside the binding must work the same way.
    #[test]
    fn test_preprocess_bound_attribute_livewire_and_method_call() {
        let content = r#"<livewire:edit-channel :key="$item->id" />"#;
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive($item->id);"),
            "method-call expression in a bound attribute should be emitted: {}",
            php
        );
        // The `:` inside the `livewire:edit-channel` tag name is part of
        // the name, not an attribute, so it must not open a directive call.
        assert!(
            !php.contains("blade_directive(edit-channel"),
            "namespace colon in the tag name must not be treated as a binding: {}",
            php
        );
    }

    /// The `:$var` shorthand expands to a bound `var` attribute whose
    /// expression is `$var`.
    #[test]
    fn test_preprocess_bound_attribute_shorthand() {
        let content = r#"<x-alert :$message />"#;
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive($message);"),
            "`:$var` shorthand should emit the variable as PHP: {}",
            php
        );
    }

    /// A bound attribute whose value contains a PHP string literal (with
    /// the opposite quote) must be captured whole, not truncated at the
    /// inner quote.
    #[test]
    fn test_preprocess_bound_attribute_with_inner_string() {
        let content = r#"<x-btn :class="$active ? 'on' : 'off'" />"#;
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive($active ? 'on' : 'off');"),
            "inner string literals should be preserved in the expression: {}",
            php
        );
    }

    /// Colons that are not at attribute position must never be treated as
    /// bindings: inside an attribute value (`mailto:`), in text between
    /// tags (`10:30`), or as an escaped literal colon (`::class`).
    #[test]
    fn test_preprocess_bound_attribute_does_not_misfire_on_value_colons() {
        let content =
            "<a href=\"mailto:x@example.com\">10:30</a>\n<x-c ::class=\"literal\" :real=\"$v\" />";
        let (php, _) = preprocess(content);
        // The only binding here is `:real="$v"`.
        assert!(
            php.contains("blade_directive($v);"),
            "the real binding should still be emitted: {}",
            php
        );
        // The prologue declares `function blade_directive(...)` once, so a
        // single binding yields two occurrences of `blade_directive(`.
        assert_eq!(
            php.matches("blade_directive(").count(),
            2,
            "no spurious bindings from value/text/escaped colons: {}",
            php
        );
        // `mailto:` and the escaped `::class` literal must stay masked.
        assert!(
            !php.contains("mailto"),
            "attr value must stay masked: {}",
            php
        );
        assert!(
            !php.contains("literal"),
            "escaped `::` attribute must stay masked: {}",
            php
        );
    }

    /// A `:name="..."` written outside any tag (in text) must not be
    /// treated as a binding.
    #[test]
    fn test_preprocess_bound_attribute_ignored_outside_tag() {
        let content = r#"<p>ratio :w="16" here</p>"#;
        let (php, _) = preprocess(content);
        // Only the prologue's `function blade_directive(...)` declaration
        // should remain; no binding call is emitted for a colon in text.
        assert_eq!(
            php.matches("blade_directive(").count(),
            1,
            "a colon in text (outside a tag span) is not a binding: {}",
            php
        );
    }

    /// A bound attribute split across lines from its tag opener must still
    /// be recognized (tags span multiple lines in real templates).
    #[test]
    fn test_preprocess_bound_attribute_multiline_tag() {
        let content = "<x-img.size\n    :src=\"$image\"\n/>";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive($image);"),
            "binding on a continuation line should be recognized: {}",
            php
        );
    }

    /// A bound attribute whose expression is wrapped over several lines
    /// (what a formatter does to a long array) must be emitted whole, not
    /// truncated at the first line break.
    #[test]
    fn test_preprocess_bound_attribute_multiline_expression() {
        let content = "<x-file.upload name=\"image\"\n    :rules=\"[\n        'Dimensions must match: 2420 x 1614',\n        'Max file size: 2 mb',\n    ]\" />\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive([") && php.contains("]);"),
            "the wrapped array must be emitted whole: {}",
            php
        );
        assert!(
            php.contains("'Dimensions must match: 2420 x 1614',"),
            "continuation lines must survive: {}",
            php
        );
        assert!(
            !php.contains("[);"),
            "the expression must not be closed off at the line break: {}",
            php
        );
        assert!(
            !php.contains("name=\"image\""),
            "the surrounding tag markup must stay masked: {}",
            php
        );
    }

    /// A multi-line bound attribute holding a call must keep every argument,
    /// otherwise the truncated call reports a bogus argument-count mismatch.
    #[test]
    fn test_preprocess_bound_attribute_multiline_call() {
        let content = "<x-alert\n    :message=\"__('a.b',\n        ['count' => 2])\"\n/>\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("__('a.b',") && php.contains("['count' => 2]));"),
            "both call arguments must survive the wrap: {}",
            php
        );
    }

    /// A bound attribute whose closing quote never appears is malformed;
    /// the call is closed at end of line so only that attribute is lost.
    #[test]
    fn test_preprocess_bound_attribute_unterminated() {
        let content = "<x-alert :message=\"$msg\n<p>{{ $after }}</p>\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive($msg);"),
            "an unterminated attribute must be closed at end of line: {}",
            php
        );
        assert!(
            php.contains("echo e( $after );"),
            "the rest of the template must still be processed: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_forelse_loop_variable() {
        let content = "@forelse($items as $item)\n{{ $loop->index }}\n@empty\n@endforelse\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("$loop = (object)[];"),
            "forelse should also inject $loop: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_echo_with_string_braces() {
        let content = "{{ \"}} \" }}";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("echo e( \"}} \" );"),
            "Failed to parse braces inside string: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_foreach() {
        let content = r#"@php
/**
 * @var \App\Models\AuthorCollection $users
 */
@endphp

@foreach($users->active()->byName() as $user)
    <p>{{ $user->name }}</p>
@endforeach
"#;
        let (php, _) = preprocess(content);
        for (i, line) in php.lines().enumerate() {
            eprintln!("{:2}: {}", i, line);
        }
        assert!(php.contains("$user->name"));
    }

    #[test]
    fn test_preprocess_forelse() {
        let content = r#"@forelse($users as $user)
    <p>{{ $user->name }}</p>
@empty
    <p>No users</p>
@endforelse
"#;
        let (php, _) = preprocess(content);
        for (i, line) in php.lines().enumerate() {
            eprintln!("{:2}: {}", i, line);
        }
        assert!(php.contains("foreach"), "should contain foreach: {}", php);
        assert!(
            php.contains("endforeach"),
            "should contain endforeach: {}",
            php
        );
        assert!(
            php.contains("if (false):"),
            "should contain if (false): {}",
            php
        );
        assert!(php.contains("endif;"), "should contain endif: {}", php);
    }

    #[test]
    fn test_preprocess_session_directive() {
        let content = "@session('key')\n    <p>{{ $value }}</p>\n@endsession\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true)"),
            "should contain if (true): {}",
            php
        );
        assert!(
            php.contains("$value = '';"),
            "should inject $value: {}",
            php
        );
        assert!(php.contains("endif;"), "should contain endif: {}", php);
    }

    #[test]
    fn test_preprocess_verbatim() {
        let content =
            "@verbatim\n    {{ $name }}\n    @if(true)\n@endverbatim\n<p>{{ $real }}</p>\n";
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("$name"),
            "verbatim content should be skipped: {}",
            php
        );
        assert!(
            php.contains("$real"),
            "content after endverbatim should work: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_verbatim_with_comment_syntax() {
        // Verbatim blocks may contain */ which would break PHP block comments
        let content =
            "@verbatim\n    {{ /* js comment */ value }}\n@endverbatim\n<p>{{ $after }}</p>\n";
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("js comment"),
            "verbatim content should be skipped: {}",
            php
        );
        assert!(
            php.contains("$after"),
            "content after endverbatim should work: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_error_directive() {
        let content = "@error('email')\n    <p>{{ $message }}</p>\n@enderror\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true)"),
            "should contain if (true): {}",
            php
        );
        assert!(
            php.contains("$message = '';"),
            "should inject $message: {}",
            php
        );
        assert!(php.contains("endif;"), "should contain endif: {}", php);
    }

    #[test]
    fn test_preprocess_context_directive() {
        let content = "@context('key')\n    <p>{{ $value }}</p>\n@endcontext\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true)"),
            "should contain if (true): {}",
            php
        );
        assert!(
            php.contains("$value = '';"),
            "should inject $value: {}",
            php
        );
        assert!(php.contains("endif;"), "should contain endif: {}", php);
    }

    #[test]
    fn test_preprocess_prologue_declares_view_directive() {
        let (php, _) = preprocess("<p>hello</p>");
        assert!(
            php.contains("function blade_view_directive"),
            "prologue should declare blade_view_directive: {}",
            php
        );
        assert!(
            php.contains("function blade_each_directive"),
            "prologue should declare blade_each_directive: {}",
            php
        );
    }

    /// `@each` gets a marker of its own: the arguments after its view name
    /// are a collection and an item name, not a data array.
    #[test]
    fn test_preprocess_each_uses_its_own_marker() {
        let (php, _) = preprocess("@each('partials.row', $rows, 'row')\n");
        assert!(
            php.contains("blade_each_directive ('partials.row', $rows, 'row');"),
            "@each should compile to a blade_each_directive call: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_multiline_directive() {
        let content = "@include('vendor.fbRemarket', [\n    'facebook_pixel_id' => Config::get('services.facebook.pixel_id'),\n])\n\n@include('vendor.googleRemarket')";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_view_directive"),
            "@include should produce blade_view_directive call: {}",
            php
        );

        let content2 = "{{\n    $var\n}}";
        let (php2, _) = preprocess(content2);
        assert!(
            php2.contains("$var"),
            "Multiline echo should preserve variable: {}",
            php2
        );
    }

    #[test]
    fn test_preprocess_stub_directives() {
        // @csrf should produce a comment (no-args directive)
        let content = "@csrf\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("/* @csrf */"),
            "@csrf should become a comment: {}",
            php
        );

        // @auth without args should produce if (true):
        let content = "@auth\n<p>logged in</p>\n@endauth\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true):"),
            "@auth should produce if (true):: {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endauth should produce endif;: {}",
            php
        );

        // @auth with args should also produce if (true):
        let content = "@auth('admin')\n<p>admin</p>\n@endauth\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true)"),
            "@auth('admin') should produce if (true): {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endauth should produce endif;: {}",
            php
        );

        // @guest without args
        let content = "@guest\n<p>guest</p>\n@endguest\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true):"),
            "@guest should produce if (true):: {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endguest should produce endif;: {}",
            php
        );

        // @production (never takes args)
        let content = "@production\n<p>prod</p>\n@endproduction\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true):"),
            "@production should produce if (true):: {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endproduction should produce endif;: {}",
            php
        );

        // @env with args
        let content = "@env('local')\n<p>local</p>\n@endenv\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true)"),
            "@env should produce if (true): {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endenv should produce endif;: {}",
            php
        );

        // @once without args
        let content = "@once\n<script>app.js</script>\n@endonce\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("if (true):"),
            "@once should produce if (true):: {}",
            php
        );
        assert!(
            php.contains("endif;"),
            "@endonce should produce endif;: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_raw_php_tag_preserves_at_prefixed_string() {
        // A raw <?php ... ?> block (not @php/@endphp) containing a string
        // literal that starts with '@' (e.g. a JSON-LD '@context' key) must
        // not be misread as a Blade directive.
        let content = "@php\n@endphp\n<?php\n$schema = ['@context' => 'x'];\n?>\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("'@context' => 'x'"),
            "raw PHP tag content should pass through verbatim: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_raw_php_tag_short_echo() {
        let content = "<p><?= $value ?></p>";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("echo  $value ;"),
            "<?= ?> should translate to an echo statement: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_switch_case_with_class_constant() {
        let content = "@switch($x)\n    @case (App\\Enums\\E::A)\n        {{ 1 }}\n        @break\n@endswitch\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("case  (App\\Enums\\E::A):"),
            "@case should preserve its argument and emit a trailing colon: {}",
            php
        );
        assert!(php.contains("break;"), "@break should emit break;: {}", php);
    }

    #[test]
    fn test_preprocess_session_value_accessible() {
        // $value should be accessible inside @session block
        let content = "@session('status')\n{{ $value }}\n@endsession\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("$value = '';"),
            "should declare $value: {}",
            php
        );
        // The $value echo should appear after the declaration
        let val_decl = php.find("$value = '';").unwrap();
        // Find last occurrence of $value (the echo usage)
        let val_echo = php.rfind("$value").unwrap();
        assert!(
            val_echo > val_decl,
            "$value usage should come after declaration: {}",
            php
        );
    }

    #[test]
    fn test_preprocess_error_message_accessible() {
        // $message should be accessible inside @error block
        let content = "@error('email')\n{{ $message }}\n@enderror\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("$message = '';"),
            "should declare $message: {}",
            php
        );
        let msg_decl = php.find("$message = '';").unwrap();
        let msg_echo = php.rfind("$message").unwrap();
        assert!(
            msg_echo > msg_decl,
            "$message usage should come after declaration: {}",
            php
        );
    }

    /// `@unless`/`@isset`/`@empty(...)` translate to `if(!`/`if(isset`/
    /// `if(empty` respectively — an extra, unmatched opening paren on top
    /// of the directive's own argument parens — so the directive needs a
    /// second closing paren before the trailing `:`, or the next PHP
    /// parser sees `unexpected token ':', expected ')'` and the rest of
    /// the template is corrupted.
    #[test]
    fn test_preprocess_unless_isset_empty_close_extra_paren() {
        let (unless_php, _) = preprocess("@unless($cond)\nx\n@endunless\n<p>after</p>");
        assert!(
            unless_php.contains("if(! ($cond)):"),
            "@unless should close both the synthetic and the argument paren: {}",
            unless_php
        );

        let (isset_php, _) = preprocess("@isset($var)\nx\n@endisset\n<p>after</p>");
        assert!(
            isset_php.contains("if(isset ($var)):"),
            "@isset should close both the synthetic and the argument paren: {}",
            isset_php
        );

        let (empty_php, _) = preprocess("@empty($var)\nx\n@endempty\n<p>after</p>");
        assert!(
            empty_php.contains("if(empty ($var)):"),
            "@empty(...) should close both the synthetic and the argument paren: {}",
            empty_php
        );
    }

    /// `@use('App\Models\Post')` must become a real top-level `use` import
    /// (hoisted out of the wrapper function), and must not leave the parser
    /// in PHP mode corrupting the rest of the template.
    #[test]
    fn test_preprocess_use_directive_emits_import() {
        let content = "@use('App\\Models\\Post')\n<p>after</p>";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("use App\\Models\\Post;"),
            "@use should emit a real use import: {}",
            php
        );
        // The import is hoisted into the prologue: top-level (not inside
        // the wrapper function) and ahead of every name it imports, since
        // name resolution runs in source order.
        let wrapper = php.find("function __blade_template()").unwrap();
        let import = php.find("use App\\Models\\Post;").unwrap();
        assert!(
            import < wrapper,
            "the use import must be hoisted into the prologue: {}",
            php
        );
        // Content after @use must stay masked as HTML, not leak as raw PHP.
        assert!(
            !php.contains("after"),
            "content after @use(...) should be masked as HTML: {}",
            php
        );
    }

    /// The inline-alias form `@use('App\Models\Post as Article')` keeps the
    /// alias.
    #[test]
    fn test_preprocess_use_directive_inline_alias() {
        let content = "@use('App\\Models\\Post as Article')\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("use App\\Models\\Post as Article;"),
            "@use with an inline `as` should preserve the alias: {}",
            php
        );
    }

    /// The two-argument alias form `@use('App\Models\Post', 'Article')`
    /// produces the same aliased import.
    #[test]
    fn test_preprocess_use_directive_second_arg_alias() {
        let content = "@use('App\\Models\\Post', 'Article')\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("use App\\Models\\Post as Article;"),
            "@use with a second alias argument should preserve the alias: {}",
            php
        );
    }

    /// The `function`/`const` modifiers are carried through to the import.
    #[test]
    fn test_preprocess_use_directive_function_modifier() {
        let content = "@use('function App\\Support\\helper')\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("use function App\\Support\\helper;"),
            "@use with a function modifier should emit `use function`: {}",
            php
        );
    }

    /// `@inject('metrics', 'App\Services\Metrics')` becomes an inline
    /// `$metrics = app(...)` assignment so the injected variable is defined
    /// and typed, and does not corrupt the rest of the template.
    #[test]
    fn test_preprocess_inject_directive_emits_assignment() {
        let content = "@inject('metrics', 'App\\Services\\Metrics')\n<p>after</p>";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("$metrics = app('App\\Services\\Metrics');"),
            "@inject should emit an inline app() assignment: {}",
            php
        );
        // The assignment is inline (inside the wrapper function), so it must
        // come before the wrapper function's closing brace.
        let brace = php.rfind('}').unwrap();
        let assign = php.find("$metrics = app(").unwrap();
        assert!(
            assign < brace,
            "the inject assignment must stay inside the wrapper function: {}",
            php
        );
        assert!(
            !php.contains("after"),
            "content after @inject(...) should be masked as HTML: {}",
            php
        );
    }

    /// An apostrophe inside a `{{-- ... --}}` comment must not be mistaken
    /// for the start of a PHP string literal — that previously made the
    /// scanner hunt for a matching closing quote instead of the comment's
    /// `--}}` terminator, desyncing the rest of the file.
    #[test]
    fn test_preprocess_comment_with_apostrophe_does_not_desync() {
        let content = "{{-- user's note --}}\n<p>{{ $after }}</p>\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("/*  user's note"),
            "comment should translate to a block comment: {}",
            php
        );
        assert!(
            php.contains("echo e( $after )"),
            "content after the comment should still translate normally: {}",
            php
        );
    }

    /// A double quote inside a `{{-- ... --}}` comment must not be mistaken
    /// for the start of a PHP string literal either — same root cause as
    /// the apostrophe case above.
    #[test]
    fn test_preprocess_comment_with_double_quote_does_not_desync() {
        let content = "{{-- say \"hi\" --}}\n<p>{{ $after }}</p>\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("/*  say \"hi\""),
            "comment should translate to a block comment: {}",
            php
        );
        assert!(
            php.contains("echo e( $after )"),
            "content after the comment should still translate normally: {}",
            php
        );
    }

    /// The text of the first `/* ... */` block comment in the virtual PHP.
    /// Panics if there is no closed block comment, which is itself the bug
    /// the callers are guarding against.
    fn comment_body(php: &str) -> &str {
        let start = php.find("/* ").expect("a block comment should be emitted");
        let rest = &php[start + 3..];
        let end = rest.find("*/").expect("the comment should be closed");
        &rest[..end]
    }

    /// Commenting out an echo is the usual reason to write a Blade comment,
    /// so the `}}` / `!!}` of the commented-out echo must not be taken for
    /// the comment's terminator: only a contiguous `--}}` ends a comment.
    #[test]
    fn test_preprocess_comment_containing_echo_does_not_desync() {
        for content in [
            "{{-- {{ $old }} --}}\n<p>{{ $after }}</p>\n",
            "{{-- {!! $old !!} --}}\n<p>{{ $after }}</p>\n",
        ] {
            let (php, _) = preprocess(content);
            assert!(
                comment_body(&php).contains("$old"),
                "the commented-out echo should stay inside the block comment: {}",
                php
            );
            assert!(
                php.contains("echo e( $after )"),
                "content after the comment should still translate normally: {}",
                php
            );
        }
    }

    /// `@endphp` mentioned in comment prose is text, not the end of an
    /// `@php` block, so it must not terminate the comment either.
    #[test]
    fn test_preprocess_comment_mentioning_endphp_does_not_desync() {
        let content = "{{-- use @php/@endphp instead --}}\n<p>{{ $after }}</p>\n";
        let (php, _) = preprocess(content);
        assert!(
            comment_body(&php).contains("@endphp instead"),
            "the mentioned directive should stay inside the block comment: {}",
            php
        );
        assert!(
            php.contains("echo e( $after )"),
            "content after the comment should still translate normally: {}",
            php
        );
    }

    /// Commenting out a block of PHP is the usual reason to write a Blade
    /// comment, so a `*/` in the comment text must not close the emitted
    /// block comment early — everything after it would become live PHP.
    #[test]
    fn test_preprocess_comment_containing_block_comment_end_does_not_desync() {
        let content = "{{-- see /* legacy */ code --}}\n<p>{{ $after }}</p>\n";
        let (php, _) = preprocess(content);
        let body = comment_body(&php);
        assert!(
            body.contains("legacy") && body.contains("code"),
            "the whole comment text should stay inside the block comment: {}",
            php
        );
        assert!(
            php.contains("echo e( $after )"),
            "content after the comment should still translate normally: {}",
            php
        );
        let emitted = php
            .lines()
            .find(|l| l.contains("legacy"))
            .expect("the comment line");
        assert_eq!(
            emitted.encode_utf16().count(),
            content.lines().next().unwrap().encode_utf16().count() + 2,
            "blanking `*/` must keep the columns aligned; only the \
             two-character `--}}` terminator grows (to ` */ `): {}",
            php
        );
    }

    /// An unterminated `{{--` must still emit a closed `/* ... */`, or the
    /// open comment swallows the wrapper function's closing brace and makes
    /// the whole virtual file unparseable.
    #[test]
    fn test_preprocess_unterminated_comment_is_closed() {
        let content = "<p>{{ $before }}</p>\n{{-- forgot to close\nstill comment\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("echo e( $before )"),
            "content before the comment should translate normally: {}",
            php
        );
        let comment_start = php.find("/* ").expect("comment should be emitted");
        let comment_end = php[comment_start..]
            .find("*/")
            .expect("unterminated comment should still be closed");
        assert!(
            php[comment_start + comment_end..].contains('}'),
            "the wrapper function's closing brace must not be inside the comment: {}",
            php
        );
    }

    /// `@inject` with a `::class` service expression is preserved verbatim
    /// (Blade keeps the second argument unquoted-trimmed).
    #[test]
    fn test_preprocess_inject_directive_class_constant_service() {
        let content = "@inject('repo', App\\Repo::class)\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("$repo = app(App\\Repo::class);"),
            "@inject should preserve a ::class service expression: {}",
            php
        );
    }

    /// The prologue text before the wrapper function: where every declared
    /// variable lives.
    fn prologue_of(php: &str) -> &str {
        php.split_once("function __blade_template()").unwrap().0
    }

    /// `@props` declares each key in the prologue, typed from its default
    /// value, so the forward walker sees it as defined and typed without
    /// waiting on the caller's `<x-… />` attributes.
    #[test]
    fn test_preprocess_props_directive_declares_variables() {
        let content = "@props(['caption' => '', 'count' => 0])\n{{ $caption }}\n";
        let (php, _) = preprocess(content);
        let prologue = prologue_of(&php);
        assert!(
            prologue.contains("$caption = '';") && prologue.contains("$count = 0;"),
            "@props should declare each key with its default: {}",
            php
        );
        assert!(
            php.contains("global $errors, $__env, $caption, $count;"),
            "props must be pulled into the wrapper scope: {}",
            php
        );
    }

    /// The declaration belongs in the prologue, not the template body. An
    /// assignment in the body would overwrite whatever type the author
    /// declared for the same name, and read as a dead local assignment to
    /// the unused-variable check.
    #[test]
    fn test_preprocess_props_directive_does_not_assign_in_the_body() {
        let content = "@props(['caption' => ''])\n<span>{{ $caption }}</span>\n";
        let (php, _) = preprocess(content);
        let body = php.split_once("function __blade_template()").unwrap().1;
        assert!(
            !body.contains("$caption ="),
            "the body must not re-assign a prop: {}",
            php
        );
        // The default expression stays visible so it is still type-checked.
        assert!(
            body.contains("blade_directive"),
            "the directive's arguments should still be analysed: {}",
            php
        );
    }

    /// A `@props` key the template's own docblock already declares keeps the
    /// declared type: the signature is the contract, `@props` only supplies
    /// what the signature leaves out.
    #[test]
    fn test_declared_signature_wins_over_a_props_default() {
        let content = "@php\n/**\n * @var \\App\\Options $options\n */\n@endphp\n@props(['options' => []])\n{{ $options->first() }}\n";
        let (php, _) = preprocess(content);
        assert!(
            !php.contains("$options = [];"),
            "the props default must not shadow the declared type: {}",
            php
        );
    }

    /// The array literal in `@props(...)` commonly spans multiple lines;
    /// the whole argument list must be read, not just the closing line, or
    /// every prop declared before the last line is lost.
    #[test]
    fn test_preprocess_props_directive_spans_multiple_lines() {
        let content = "@props([\n    'caption' => '',\n])\n{{ $caption }}\n";
        let (php, _) = preprocess(content);
        assert!(
            prologue_of(&php).contains("$caption = '';"),
            "a multi-line @props array must still declare its keys: {}",
            php
        );
    }

    /// A prop with no default (`@props(['visible'])`) is *required*: its
    /// value comes from the caller, so it is declared `mixed` rather than
    /// being invented as `null`, which would make every use of it a type
    /// error against whatever the prop is really passed.
    #[test]
    fn test_preprocess_props_directive_shorthand_without_default() {
        let content = "@props(['visible'])\n{{ $visible }}\n";
        let (php, _) = preprocess(content);
        assert!(
            prologue_of(&php).contains("/** @var mixed $visible */"),
            "a defaultless prop should be declared mixed: {}",
            php
        );
    }

    /// `@aware` pulls a value from the parent component, falling back to the
    /// declared default, so it types the body exactly as `@props` does.
    #[test]
    fn test_preprocess_aware_directive_declares_variables() {
        let content = "@aware(['color' => 'gray'])\n{{ $color }}\n";
        let (php, _) = preprocess(content);
        assert!(
            prologue_of(&php).contains("$color = 'gray';"),
            "@aware should declare its keys: {}",
            php
        );
    }

    /// A dynamic props argument (not a plain array literal) cannot be read,
    /// so no variable is invented; the expression still reaches PHP as an
    /// inert call so its own variables are seen.
    #[test]
    fn test_preprocess_props_directive_dynamic_argument_falls_back() {
        let content = "@props($dynamicProps)\n";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive ($dynamicProps);"),
            "a non-literal @props argument should fall back to the inert call: {}",
            php
        );
        assert!(
            php.contains("global $errors, $__env;"),
            "a non-literal @props argument declares nothing: {}",
            php
        );
    }
}
