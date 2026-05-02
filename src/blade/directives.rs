pub fn match_directive(s: &str) -> Option<&'static str> {
    let directives = [
        "if",
        "elseif",
        "else",
        "endif",
        "foreach",
        "endforeach",
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
        "extends",
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
        "endcomponent",
        "slot",
        "endslot",
        "props",
        "aware",
        "stop",
        "show",
        "append",
        "overwrite",
    ];

    for d in directives {
        if let Some(stripped) = s.strip_prefix(d) {
            let next_char = stripped.chars().next();
            if next_char.is_none() || !next_char.unwrap().is_alphanumeric() {
                return Some(d);
            }
        }
    }
    None
}

pub fn translate_directive(directive: &str) -> String {
    match directive {
        "php" | "endphp" => "".to_string(),
        "if" | "elseif" | "foreach" | "for" | "while" | "switch" | "case" => directive.to_string(),
        "unless" => "if(!".to_string(),
        "else" => "else:".to_string(),
        "endif" | "endforeach" | "endfor" | "endwhile" | "endunless" | "endisset" | "endempty"
        | "endswitch" => {
            let mapped = match directive {
                "endunless" | "endisset" | "endempty" => "endif",
                other => other,
            };
            format!("{mapped};")
        }
        "isset" => "if(isset".to_string(),
        "empty" => "if(empty".to_string(),
        "use" => "use ".to_string(),
        "break" => "break;".to_string(),
        "default" => "default:".to_string(),
        "inject" => "$".to_string(),
        "extends" | "section" | "yield" | "include" | "includeIf" | "includeWhen"
        | "includeUnless" | "includeFirst" | "push" | "prepend" | "component" | "slot"
        | "props" | "aware" => "blade_directive".to_string(),
        "endsection" | "endpush" | "endprepend" | "endcomponent" | "endslot" | "stop" | "show"
        | "append" | "overwrite" => "".to_string(),
        _ => format!("/* @{directive} */"),
    }
}
