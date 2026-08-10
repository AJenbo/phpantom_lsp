/// Every directive name `match_directive` recognises, in no particular
/// order beyond the loose grouping comments below. [`DIRECTIVE_COMPLETIONS`]
/// is checked against this list (`every_known_directive_has_a_completion`)
/// so the two cannot silently drift apart.
const KNOWN_DIRECTIVES: &[&str] = &[
    "if",
    "elseif",
    "else",
    "endif",
    "foreach",
    "endforeach",
    "forelse",
    "endforelse",
    "for",
    "endfor",
    "while",
    "endwhile",
    "unless",
    "endunless",
    "isset",
    "endisset",
    "empty",
    "endempty",
    "switch",
    "endswitch",
    "case",
    "default",
    "break",
    "php",
    "endphp",
    "use",
    "inject",
    "class",
    "style",
    "checked",
    "selected",
    "disabled",
    "readonly",
    "required",
    "json",
    "dump",
    "extends",
    "extendsFirst",
    "section",
    "endsection",
    "yield",
    "include",
    "includeIf",
    "includeWhen",
    "includeUnless",
    "includeFirst",
    "stack",
    "push",
    "endpush",
    "prepend",
    "endprepend",
    "component",
    "componentFirst",
    "endcomponent",
    "endcomponentFirst",
    "slot",
    "endslot",
    "props",
    "aware",
    "stop",
    "show",
    "append",
    "overwrite",
    // Auth/env directives
    "auth",
    "endauth",
    "guest",
    "endguest",
    "production",
    "endproduction",
    "env",
    "endenv",
    // Session/context directives
    "session",
    "endsession",
    "context",
    "endcontext",
    // Section helpers
    "hasSection",
    "sectionMissing",
    "parent",
    // Include variants
    "includeIsolated",
    "each",
    // Stack directives
    "pushIf",
    "endPushIf",
    "pushOnce",
    "endPushOnce",
    "prependOnce",
    "endPrependOnce",
    "hasStack",
    // Form directives
    "csrf",
    "method",
    "error",
    "enderror",
    // Continuation
    "continue",
    // Misc directives
    "once",
    "endonce",
    "verbatim",
    "endverbatim",
    "fragment",
    "endfragment",
    // Authorization directives
    "can",
    "cannot",
    "canany",
    "elsecan",
    "elsecannot",
    "elsecanany",
    "endcan",
    "endcannot",
    "endcanany",
    // Translation directives
    "lang",
    "endlang",
    "choice",
    // Raw PHP
    "unset",
    // JS/asset helpers
    "js",
    "vite",
    "viteReactRefresh",
    "fonts",
    "dd",
];

pub fn match_directive(s: &str) -> Option<&'static str> {
    for &d in KNOWN_DIRECTIVES {
        if let Some(stripped) = s.strip_prefix(d) {
            let next_char = stripped.chars().next();
            if next_char.is_none() || !next_char.unwrap().is_alphanumeric() {
                return Some(d);
            }
        }
    }
    None
}

/// A directive-name completion candidate offered when `@` is typed in an
/// HTML/directive position of a Blade template.
///
/// `insert_text` never includes the leading `@` — the trigger character is
/// already in the buffer by the time the completion request fires, so only
/// the rest is inserted, mirroring how PHPDoc tag completion inserts text
/// after an already-typed `@` (`src/completion/phpdoc/mod.rs`).
pub struct DirectiveCompletion {
    pub name: &'static str,
    pub insert_text: &'static str,
    pub is_snippet: bool,
}

/// One completion entry per name in [`match_directive`]'s list, verified
/// against Laravel's own compiler (`Illuminate\View\Compilers\Concerns\*`).
/// A directive that opens a block inserts the whole matching pair with a
/// `$0` exit tab stop, the way Blade authors write it by hand; a directive
/// with no arguments inserts just its own name.
///
/// Kept in sync with `match_directive` by
/// `every_known_directive_has_a_completion` below rather than sharing a
/// single array, so a plain `&[&str]` (used on every preprocessor
/// character) doesn't have to carry unused snippet payloads.
pub const DIRECTIVE_COMPLETIONS: &[DirectiveCompletion] = &[
    DirectiveCompletion {
        name: "if",
        insert_text: "if ($1)\n\t$0\n@endif",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "elseif",
        insert_text: "elseif ($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "else",
        insert_text: "else",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "endif",
        insert_text: "endif",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "foreach",
        insert_text: "foreach ($1 as $2)\n\t$0\n@endforeach",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endforeach",
        insert_text: "endforeach",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "forelse",
        insert_text: "forelse ($1 as $2)\n\t$0\n@empty\n\t\n@endforelse",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endforelse",
        insert_text: "endforelse",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "for",
        insert_text: "for ($1)\n\t$0\n@endfor",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endfor",
        insert_text: "endfor",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "while",
        insert_text: "while ($1)\n\t$0\n@endwhile",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endwhile",
        insert_text: "endwhile",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "unless",
        insert_text: "unless ($1)\n\t$0\n@endunless",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endunless",
        insert_text: "endunless",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "isset",
        insert_text: "isset ($1)\n\t$0\n@endisset",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endisset",
        insert_text: "endisset",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "empty",
        insert_text: "empty ($1)\n\t$0\n@endempty",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endempty",
        insert_text: "endempty",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "switch",
        insert_text: "switch ($1)\n\t@case($2)\n\t\t$0\n\t\t@break\n\n\t@default\n\t\t\n@endswitch",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endswitch",
        insert_text: "endswitch",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "case",
        insert_text: "case($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "default",
        insert_text: "default",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "break",
        insert_text: "break",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "php",
        insert_text: "php\n$0\n@endphp",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endphp",
        insert_text: "endphp",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "use",
        insert_text: "use('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "inject",
        insert_text: "inject('$1', '$2')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "class",
        insert_text: "class([$1])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "style",
        insert_text: "style([$1])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "checked",
        insert_text: "checked($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "selected",
        insert_text: "selected($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "disabled",
        insert_text: "disabled($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "readonly",
        insert_text: "readonly($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "required",
        insert_text: "required($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "json",
        insert_text: "json($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "dump",
        insert_text: "dump($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "extends",
        insert_text: "extends('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "extendsFirst",
        insert_text: "extendsFirst(['$1'])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "section",
        insert_text: "section('$1')\n\t$0\n@endsection",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endsection",
        insert_text: "endsection",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "yield",
        insert_text: "yield('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "include",
        insert_text: "include('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "includeIf",
        insert_text: "includeIf('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "includeWhen",
        insert_text: "includeWhen($1, '$2')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "includeUnless",
        insert_text: "includeUnless($1, '$2')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "includeFirst",
        insert_text: "includeFirst(['$1'])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "stack",
        insert_text: "stack('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "push",
        insert_text: "push('$1')\n\t$0\n@endpush",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endpush",
        insert_text: "endpush",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "prepend",
        insert_text: "prepend('$1')\n\t$0\n@endprepend",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endprepend",
        insert_text: "endprepend",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "component",
        insert_text: "component('$1')\n\t$0\n@endcomponent",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "componentFirst",
        insert_text: "componentFirst(['$1'])\n\t$0\n@endcomponentFirst",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endcomponent",
        insert_text: "endcomponent",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "endcomponentFirst",
        insert_text: "endcomponentFirst",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "slot",
        insert_text: "slot('$1')\n\t$0\n@endslot",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endslot",
        insert_text: "endslot",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "props",
        insert_text: "props([$1])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "aware",
        insert_text: "aware([$1])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "stop",
        insert_text: "stop",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "show",
        insert_text: "show",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "append",
        insert_text: "append",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "overwrite",
        insert_text: "overwrite",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "auth",
        insert_text: "auth\n\t$0\n@endauth",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endauth",
        insert_text: "endauth",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "guest",
        insert_text: "guest\n\t$0\n@endguest",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endguest",
        insert_text: "endguest",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "production",
        insert_text: "production\n\t$0\n@endproduction",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endproduction",
        insert_text: "endproduction",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "env",
        insert_text: "env('$1')\n\t$0\n@endenv",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endenv",
        insert_text: "endenv",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "session",
        insert_text: "session('$1')\n\t$0\n@endsession",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endsession",
        insert_text: "endsession",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "context",
        insert_text: "context('$1')\n\t$0\n@endcontext",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endcontext",
        insert_text: "endcontext",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "hasSection",
        insert_text: "hasSection('$1')\n\t$0\n@endif",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "sectionMissing",
        insert_text: "sectionMissing('$1')\n\t$0\n@endif",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "parent",
        insert_text: "parent",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "includeIsolated",
        insert_text: "includeIsolated('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "each",
        insert_text: "each('$1', $2, '$3')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "pushIf",
        insert_text: "pushIf($1, '$2')\n\t$0\n@endPushIf",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endPushIf",
        insert_text: "endPushIf",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "pushOnce",
        insert_text: "pushOnce('$1')\n\t$0\n@endPushOnce",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endPushOnce",
        insert_text: "endPushOnce",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "prependOnce",
        insert_text: "prependOnce('$1')\n\t$0\n@endPrependOnce",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endPrependOnce",
        insert_text: "endPrependOnce",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "hasStack",
        insert_text: "hasStack('$1')\n\t$0\n@endif",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "csrf",
        insert_text: "csrf",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "method",
        insert_text: "method('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "error",
        insert_text: "error('$1')\n\t$0\n@enderror",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "enderror",
        insert_text: "enderror",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "continue",
        insert_text: "continue",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "once",
        insert_text: "once\n\t$0\n@endonce",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endonce",
        insert_text: "endonce",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "verbatim",
        insert_text: "verbatim\n\t$0\n@endverbatim",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endverbatim",
        insert_text: "endverbatim",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "fragment",
        insert_text: "fragment('$1')\n\t$0\n@endfragment",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endfragment",
        insert_text: "endfragment",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "can",
        insert_text: "can('$1')\n\t$0\n@endcan",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "cannot",
        insert_text: "cannot('$1')\n\t$0\n@endcannot",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "canany",
        insert_text: "canany(['$1'])\n\t$0\n@endcanany",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "elsecan",
        insert_text: "elsecan('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "elsecannot",
        insert_text: "elsecannot('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "elsecanany",
        insert_text: "elsecanany(['$1'])",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endcan",
        insert_text: "endcan",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "endcannot",
        insert_text: "endcannot",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "endcanany",
        insert_text: "endcanany",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "lang",
        insert_text: "lang\n\t$0\n@endlang",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "endlang",
        insert_text: "endlang",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "choice",
        insert_text: "choice('$1', $2)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "unset",
        insert_text: "unset($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "js",
        insert_text: "js($1)",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "vite",
        insert_text: "vite('$1')",
        is_snippet: true,
    },
    DirectiveCompletion {
        name: "viteReactRefresh",
        insert_text: "viteReactRefresh",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "fonts",
        insert_text: "fonts",
        is_snippet: false,
    },
    DirectiveCompletion {
        name: "dd",
        insert_text: "dd($1)",
        is_snippet: true,
    },
];

pub fn translate_directive(directive: &str) -> String {
    // A section or stack name is a cross-file key, so these lower to
    // markers of their own for symbol extraction to recognise them by (see
    // `crate::blade::blocks`).  `@hasSection` and its cousins compile to a
    // condition, and Laravel always follows them with an `@endif`, so the
    // marker opens a real `if` for that `@endif` to close.
    if let Some(entry) = super::blocks::named_block_directive(directive) {
        let marker = entry.marker();
        return if entry.opens_condition() {
            format!("if ({marker}")
        } else {
            marker.to_string()
        };
    }
    match directive {
        "php" | "endphp" => "".to_string(),
        "if" | "elseif" | "foreach" | "for" | "while" | "switch" | "case" => directive.to_string(),
        "forelse" => "foreach".to_string(),
        "unless" => "if(!".to_string(),
        "else" => "else:".to_string(),
        "endif" | "endforeach" | "endfor" | "endwhile" | "endunless" | "endisset" | "endempty"
        | "endswitch" | "endforelse" | "endsession" | "endcontext" | "enderror" | "endauth"
        | "endguest" | "endproduction" | "endenv" | "endonce" | "endcan" | "endcannot"
        | "endcanany" => {
            let mapped = match directive {
                "endunless" | "endisset" | "endempty" | "endsession" | "endcontext"
                | "enderror" | "endauth" | "endguest" | "endproduction" | "endenv" | "endonce"
                | "endcan" | "endcannot" | "endcanany" => "endif",
                "endforelse" => "endif",
                other => other,
            };
            format!("{mapped};")
        }
        "isset" => "if(isset".to_string(),
        "empty" => "if(empty".to_string(),
        "break" => "break;".to_string(),
        "default" => "default:".to_string(),
        "extends" | "extendsFirst" | "include" | "includeIf" | "includeWhen" | "includeUnless"
        | "includeFirst" | "component" | "componentFirst" => "blade_view_directive".to_string(),
        // `@each` renders its partial once per entry of a collection, with
        // only the item and the key in scope.  The arguments after the view
        // name therefore mean something entirely different from every other
        // render directive's data array, so it gets a marker of its own.
        "each" => "blade_each_directive".to_string(),
        "slot" | "props" | "aware" | "class" | "style" | "checked" | "selected" | "disabled"
        | "readonly" | "required" | "json" | "dump" | "lang" | "choice" | "js" | "vite"
        | "fonts" | "dd" => "blade_directive".to_string(),
        // `unset(...)` is a language construct, not a function — it cannot
        // be passed as an argument to `blade_directive(...)`, so it keeps
        // its own real name instead of the generic marker.
        "unset" => "unset".to_string(),
        // `@can`/`@cannot`/`@canany` (and their `@hasStack`/`@hasSection`/
        // `@sectionMissing` cousins) are conditionals that Laravel compiles
        // to a real gate/environment method call inside `if (...):`. A
        // marker call stands in for it rather than a hard-coded
        // `\Illuminate\Contracts\Auth\Access\Gate` reference that may not
        // exist in every project's autoload map — the point is to keep the
        // arguments' expressions real PHP that gets type-checked, and to
        // open a genuine `if` so the `@endif` that always follows these
        // stays balanced.
        //
        // The authorization three get a marker of their own rather than
        // sharing `blade_directive` with everything else: their first
        // argument is an ability name, and symbol extraction recognises it
        // by the callee it is passed to.
        "can" | "cannot" | "canany" => "if (blade_can_directive".to_string(),
        "elsecan" | "elsecannot" | "elsecanany" => "elseif (blade_can_directive".to_string(),
        "endsection" | "endpush" | "endprepend" | "endcomponent" | "endcomponentFirst"
        | "endslot" | "stop" | "show" | "append" | "overwrite" => "".to_string(),
        _ => format!("/* @{directive} */"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`DIRECTIVE_COMPLETIONS`] is a separate table from [`KNOWN_DIRECTIVES`]
    /// (a plain `&[&str]` on the preprocessor's hot path doesn't need to carry
    /// unused snippet payloads), so nothing at compile time stops the two
    /// from drifting apart. This test is that guard: every directive
    /// `match_directive` recognises must have exactly one completion entry,
    /// and no completion entry may name a directive that doesn't exist.
    #[test]
    fn every_known_directive_has_exactly_one_completion() {
        let mut known: Vec<&str> = KNOWN_DIRECTIVES.to_vec();
        known.sort_unstable();
        known.dedup();
        assert_eq!(
            known.len(),
            KNOWN_DIRECTIVES.len(),
            "KNOWN_DIRECTIVES contains a duplicate name"
        );

        let mut completions: Vec<&str> = DIRECTIVE_COMPLETIONS.iter().map(|c| c.name).collect();
        completions.sort_unstable();
        completions.dedup();
        assert_eq!(
            completions.len(),
            DIRECTIVE_COMPLETIONS.len(),
            "DIRECTIVE_COMPLETIONS contains a duplicate name"
        );

        assert_eq!(
            known, completions,
            "DIRECTIVE_COMPLETIONS must have exactly one entry per known directive"
        );
    }

    /// Every directive `match_directive` recognises by itself (with nothing
    /// after it) must round-trip: recognising the directive by name and then
    /// re-matching that exact name must yield the same directive back. This
    /// guards against a prefix collision (a short name shadowing a longer
    /// one earlier in the list) silently truncating recognition.
    #[test]
    fn every_known_directive_matches_its_own_bare_name() {
        for &name in KNOWN_DIRECTIVES {
            assert_eq!(
                match_directive(name),
                Some(name),
                "directive {name:?} did not match its own bare name"
            );
        }
    }

    /// A directive that names a section or a stack has to lower to the
    /// marker call its table entry names, since symbol extraction reads
    /// the name off that callee and nothing else links the two tables.
    #[test]
    fn every_named_block_directive_lowers_to_its_own_marker() {
        for entry in crate::blade::blocks::NAMED_BLOCK_DIRECTIVES {
            let translated = translate_directive(entry.name);
            assert!(
                translated.contains(entry.marker()),
                "@{} lowers to {translated:?}, which does not call {}",
                entry.name,
                entry.marker()
            );
        }
    }

    /// Every completion's insert text must actually start with the
    /// directive's own name (allowing for the `$`-prefixed tab-stop syntax
    /// only after it), since a client replaces exactly the region right
    /// after the `@` the user already typed — the label promises the
    /// directive named, so the text must open with it.
    #[test]
    fn every_completion_insert_text_starts_with_its_own_name() {
        for completion in DIRECTIVE_COMPLETIONS {
            assert!(
                completion.insert_text.starts_with(completion.name),
                "completion for {:?} inserts {:?}, which doesn't start with the directive name",
                completion.name,
                completion.insert_text
            );
        }
    }
}
