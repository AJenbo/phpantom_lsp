use super::directives::{match_directive, translate_directive};
use super::source_map::BladeSourceMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Html,
    Php,
    DirectiveArgs(&'static str),
}

pub fn preprocess(content: &str) -> (String, BladeSourceMap) {
    let mut virtual_php = String::with_capacity(content.len() + 512);
    let mut source_map = BladeSourceMap::default();

    // ── Prologue (5 lines) ──
    virtual_php.push_str("<?php if (!function_exists('blade_directive')) { function blade_directive(...$args) {} }\n");
    virtual_php.push_str("/** @var \\Illuminate\\Support\\ViewErrorBag $errors */\n");
    virtual_php.push_str("$errors = new \\Illuminate\\Support\\ViewErrorBag();\n");
    virtual_php.push_str("/** @var \\Illuminate\\View\\Factory $__env */\n");
    virtual_php.push_str("$__env = new \\Illuminate\\View\\Factory();\n");

    let mut in_php_directive_block = false;
    let mut mode = Mode::Html;
    let mut paren_depth = 0;
    let mut in_string: Option<char> = None;
    let mut is_escaped = false;

    for line in content.lines() {
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

            if mode != Mode::Html {
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
                    next_mode = Mode::Php;
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
                                next_mode = Mode::DirectiveArgs(";"); // Directive Args for @php(...)
                                paren_depth = 0;
                            }
                        } else if directive == "endphp" {
                            replacement = "".to_string();
                            next_mode = Mode::Html;
                        } else if matches!(
                            directive,
                            "if" | "elseif"
                                | "foreach"
                                | "for"
                                | "while"
                                | "switch"
                                | "unless"
                                | "isset"
                                | "empty"
                        ) {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::DirectiveArgs(":"); // Directive Args
                            paren_depth = 0;
                        } else if matches!(
                            directive,
                            "extends"
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
                                | "slot"
                                | "props"
                                | "aware"
                                | "auth"
                                | "guest"
                                | "production"
                                | "env"
                                | "session"
                                | "context"
                                | "error"
                                | "once"
                                | "verbatim"
                                | "fragment"
                                | "hasSection"
                                | "sectionMissing"
                                | "includeIsolated"
                                | "each"
                                | "pushIf"
                                | "pushOnce"
                                | "prependOnce"
                                | "hasstack"
                                | "method"
                        ) {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::DirectiveArgs(";"); // Directive Args for layout tags
                            paren_depth = 0;
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
                                | "endsection"
                                | "endpush"
                                | "endprepend"
                                | "endcomponent"
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
                                | "endverbatim"
                                | "endfragment"
                                | "endPushIf"
                                | "endPushOnce"
                                | "csrf"
                                | "parent"
                                | "continue"
                        ) {
                            replacement = format!(" {} ", translate_directive(directive));
                            next_mode = Mode::Html; // These don't take args and return to HTML mode immediately
                        } else {
                            replacement = format!(" {}; ", translate_directive(directive));
                            next_mode = Mode::Php;
                        }
                    }
                }
            } else if mode == Mode::Php {
                if remaining.starts_with(&['}', '}']) || remaining.starts_with(&['!', '!', '}']) {
                    let is_comment_end =
                        char_idx >= 2 && line_chars[char_idx - 2..].starts_with(&['-', '-']);
                    replacement = if is_comment_end {
                        " */ ".to_string()
                    } else {
                        "); ".to_string()
                    };
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

            buffer.push(ch);
            char_idx += 1;
            current_utf16_col += ch.len_utf16() as u32;
        }

        flush_buffer(
            &mut processed,
            &mut buffer,
            mode,
            current_utf16_col,
            &mut adjustments,
        );

        virtual_php.push_str(&processed);
        virtual_php.push('\n');
        adjustments.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        source_map.adjustments.push(adjustments);
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
        processed.push_str(buffer);
        adjustments.push((current_utf16_col, utf16_count(processed) as u32));
    }

    buffer.clear();
}

fn utf16_count(s: &str) -> usize {
    s.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_preprocess_multiline_directive() {
        let content = "@include('vendor.fbRemarket', [\n    'facebook_pixel_id' => Config::get('services.facebook.pixel_id'),\n])\n\n@include('vendor.googleRemarket')";
        let (php, _) = preprocess(content);
        assert!(
            php.contains("blade_directive"),
            "@include should produce blade_directive call: {}",
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
}
