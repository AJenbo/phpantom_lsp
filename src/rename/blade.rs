//! The part of a template a rename cannot reach through its symbol map.
//!
//! Everything else a Blade template names is rewritten from the virtual
//! PHP the preprocessor lowers it to, and translated back through the
//! source map.  The `@use` directive is the exception: it is hoisted to
//! the top of the virtual file as a real `use` statement, because a PHP
//! import is only valid at the top level and the template body is wrapped
//! in a function.  The prologue it lands in has no template text behind
//! it, so the reference recorded there translates back to no position at
//! all.  The directive is scanned in the template's own text instead.

use tower_lsp::lsp_types::{Range, TextEdit};

use crate::text_position::offset_to_position;

/// Collect the edits that rewrite the names a template's `@use(...)`
/// directives import.
///
/// `moved` answers what a name the rename carries becomes, or `None` for
/// a name it leaves alone.  It is asked about the imported name as
/// written, without any leading `\`: a `use` names its target absolutely,
/// so the root marker is decoration the rewrite leaves where it is.
pub(super) fn collect_use_directive_edits(
    content: &str,
    moved: &dyn Fn(&str) -> Option<String>,
    edits: &mut Vec<TextEdit>,
) {
    for (arguments_at, arguments) in use_directive_arguments(content) {
        let Some((literal_at, literal)) = first_string_literal(arguments) else {
            continue;
        };
        let literal_at = arguments_at + literal_at;

        let Some((name_at, name)) = imported_name(literal) else {
            continue;
        };
        let bare = name.trim_start_matches('\\');
        let name_at = literal_at + name_at + (name.len() - bare.len());
        let renamed = moved(bare);

        if let Some(to) = &renamed {
            push_edit(content, name_at, bare, to, edits);
        }

        // A group import (`'App\Models\{Post, Comment}'`) names the shared
        // prefix where a plain one names the class, so the prefix is what
        // the edit above moved and each member has to be checked on its
        // own.  A member that leaves the prefix behind is one the group
        // cannot express; it is left alone rather than rewritten into a
        // name that would compose back into a different class.
        let Some((group_at, members)) = group_members(literal) else {
            continue;
        };
        let prefix = renamed.as_deref().unwrap_or(bare);
        for (member_at, member) in members {
            let Some(tail) = moved(&format!("{bare}\\{member}"))
                .and_then(|to| to.strip_prefix(&format!("{prefix}\\")).map(str::to_string))
            else {
                continue;
            };
            push_edit(
                content,
                literal_at + group_at + member_at,
                member,
                &tail,
                edits,
            );
        }
    }
}

/// Every `@use(...)` directive in `content`, as the byte offset of its
/// argument list and the text of it (the parentheses excluded).
fn use_directive_arguments(content: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut searched = 0;
    std::iter::from_fn(move || {
        loop {
            let at = searched + content[searched..].find("@use")?;
            searched = at + "@use".len();
            // `@@use` is an escaped directive Blade prints verbatim, and
            // `@used` is a different word entirely.
            if content[..at].ends_with('@') {
                continue;
            }
            let rest = &content[searched..];
            let trimmed = rest.trim_start();
            if !trimmed.starts_with('(') {
                continue;
            }
            let open = searched + (rest.len() - trimmed.len());
            let Some(close) = content[open..].find(')') else {
                continue;
            };
            searched = open + close;
            return Some((open + 1, &content[open + 1..open + close]));
        }
    })
}

/// The first quoted string in an argument list, as its byte offset within
/// the list and its text with the quotes stripped.
///
/// `@use` takes the imported name first and an optional alias second, so
/// the first string is the only one that names a class.
fn first_string_literal(arguments: &str) -> Option<(usize, &str)> {
    let open = arguments.find(['\'', '"'])?;
    let quote = arguments.as_bytes()[open];
    let close = open + 1 + arguments[open + 1..].find(quote as char)?;
    Some((open + 1, &arguments[open + 1..close]))
}

/// The name a `@use` literal imports, as its byte offset within the
/// literal and its text.
///
/// Strips the `function` / `const` modifier and an inline `as` alias, and
/// for a group import answers the shared prefix rather than the braces.
fn imported_name(literal: &str) -> Option<(usize, &str)> {
    let mut at = literal.len() - literal.trim_start().len();
    let mut rest = &literal[at..];

    for modifier in ["function ", "const "] {
        if let Some(stripped) = rest.strip_prefix(modifier) {
            let trimmed = stripped.trim_start();
            at += modifier.len() + (stripped.len() - trimmed.len());
            rest = trimmed;
            break;
        }
    }

    let end = rest
        .find(" as ")
        .or_else(|| rest.find('{'))
        .unwrap_or(rest.len());
    let name = rest[..end].trim_end().trim_end_matches('\\');
    (!name.is_empty()).then_some((at, name))
}

/// The members of a group import (`'App\Models\{Post, Comment}'`), as the
/// byte offset of the braced list within the literal and each member's
/// offset within that list.
///
/// `None` when the literal is not a group import.
fn group_members(literal: &str) -> Option<(usize, Vec<(usize, &str)>)> {
    let open = literal.find('{')?;
    let close = open + literal[open..].find('}')?;
    let list = &literal[open + 1..close];

    let mut members = Vec::new();
    let mut at = 0;
    for member in list.split(',') {
        let name = member.trim();
        if !name.is_empty() {
            members.push((at + (member.len() - member.trim_start().len()), name));
        }
        at += member.len() + 1;
    }
    Some((open + 1, members))
}

fn push_edit(content: &str, at: usize, from: &str, to: &str, edits: &mut Vec<TextEdit>) {
    if from == to {
        return;
    }
    edits.push(TextEdit {
        range: Range {
            start: offset_to_position(content, at),
            end: offset_to_position(content, at + from.len()),
        },
        new_text: to.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rewrite every name under `App\Old` to the same name under
    /// `App\New`, which is what a namespace move asks for.
    fn moved_prefix(name: &str) -> Option<String> {
        name.strip_prefix("App\\Old")
            .filter(|rest| rest.is_empty() || rest.starts_with('\\'))
            .map(|rest| format!("App\\New{rest}"))
    }

    fn rewrite(template: &str, moved: &dyn Fn(&str) -> Option<String>) -> String {
        let mut edits = Vec::new();
        collect_use_directive_edits(template, moved, &mut edits);
        let mut result = template.to_string();
        let lines: Vec<&str> = template.lines().collect();
        // Applied last-first so an earlier edit's offsets stay valid.
        edits.sort_by_key(|edit| {
            std::cmp::Reverse((edit.range.start.line, edit.range.start.character))
        });
        for edit in &edits {
            let offset = |position: tower_lsp::lsp_types::Position| {
                lines[..position.line as usize]
                    .iter()
                    .map(|line| line.len() + 1)
                    .sum::<usize>()
                    + position.character as usize
            };
            result.replace_range(
                offset(edit.range.start)..offset(edit.range.end),
                &edit.new_text,
            );
        }
        result
    }

    #[test]
    fn a_plain_import_follows_the_move() {
        assert_eq!(
            rewrite("@use('App\\Old\\Widget')\n", &moved_prefix),
            "@use('App\\New\\Widget')\n"
        );
    }

    /// Double quotes and the two-argument alias form are the same import
    /// written differently, and the alias is not a class name.
    #[test]
    fn the_alias_forms_rewrite_only_the_imported_name() {
        assert_eq!(
            rewrite("@use(\"App\\Old\\Widget\", 'Gadget')\n", &moved_prefix),
            "@use(\"App\\New\\Widget\", 'Gadget')\n"
        );
        assert_eq!(
            rewrite("@use('App\\Old\\Widget as Gadget')\n", &moved_prefix),
            "@use('App\\New\\Widget as Gadget')\n"
        );
    }

    /// `use function` and `use const` import from the same namespaces
    /// classes do, so the modifier only moves the name it precedes.
    #[test]
    fn a_modifier_leaves_the_name_behind_it_reachable() {
        assert_eq!(
            rewrite("@use('function App\\Old\\helper')\n", &moved_prefix),
            "@use('function App\\New\\helper')\n"
        );
        assert_eq!(
            rewrite("@use('const App\\Old\\LIMIT')\n", &moved_prefix),
            "@use('const App\\New\\LIMIT')\n"
        );
    }

    /// A group import names the shared prefix, which is what a namespace
    /// move carries; the members it lists stay where they are.
    #[test]
    fn a_group_imports_prefix_follows_the_move() {
        assert_eq!(
            rewrite("@use('App\\Old\\{Widget, Gadget}')\n", &moved_prefix),
            "@use('App\\New\\{Widget, Gadget}')\n"
        );
    }

    /// A class renamed inside a group is rewritten member by member, and
    /// only where the new name still sits under the group's prefix — a
    /// member that leaves it cannot be spelled inside the braces at all.
    #[test]
    fn a_group_member_follows_a_rename_but_not_a_move_out_of_the_group() {
        let renamed = |name: &str| match name {
            "App\\Old\\Widget" => Some("App\\Old\\Gizmo".to_string()),
            _ => None,
        };
        assert_eq!(
            rewrite("@use('App\\Old\\{Widget, Gadget}')\n", &renamed),
            "@use('App\\Old\\{Gizmo, Gadget}')\n"
        );

        let moved_out = |name: &str| match name {
            "App\\Old\\Widget" => Some("App\\Elsewhere\\Widget".to_string()),
            _ => None,
        };
        assert_eq!(
            rewrite("@use('App\\Old\\{Widget, Gadget}')\n", &moved_out),
            "@use('App\\Old\\{Widget, Gadget}')\n"
        );
    }

    /// A rooted spelling names the same class an unrooted one does, since
    /// an import is always absolute, so the root marker survives the
    /// rewrite untouched.
    #[test]
    fn a_rooted_name_keeps_its_root() {
        assert_eq!(
            rewrite("@use('\\App\\Old\\Widget')\n", &moved_prefix),
            "@use('\\App\\New\\Widget')\n"
        );
    }

    /// `@@use` is an escaped directive Blade prints as text, and `@used`
    /// is a different word; neither imports anything.
    #[test]
    fn neither_an_escaped_directive_nor_a_longer_word_is_an_import() {
        for template in [
            "@@use('App\\Old\\Widget')\n",
            "@used('App\\Old\\Widget')\n",
            "@use\n",
        ] {
            assert_eq!(rewrite(template, &moved_prefix), template);
        }
    }
}
