//! Environment-variable names: `env('APP_NAME')` and `Env::get('APP_NAME')`.
//!
//! The symbol map records both spellings as
//! [`crate::symbol_map::LaravelStringKind::Env`] spans, so find-references
//! and hover come from the same index every other Laravel string key uses.
//! What a name resolves *to* is the line that declares it in the project's
//! `.env`, which is what this module supplies.

use tower_lsp::lsp_types::{Location, Position, Url};

use crate::Backend;

/// The dotenv files a project declares its variables in, in the order a
/// reader should prefer them.
///
/// `.env` is the one the framework actually loads; `.env.example` is the
/// committed inventory of what a project expects, and is all a fresh clone
/// has before anyone copies it.
const ENV_FILES: [&str; 2] = [".env", ".env.example"];

/// Resolve an environment-variable name to the line declaring it in each
/// dotenv file that does.
///
/// A name neither file declares still resolves to the top of the first file
/// that exists: an environment a process runs with is not on disk, so a name
/// missing from `.env` is usually one that belongs there, and opening the
/// file says more than refusing to navigate.
pub(crate) fn resolve_env_definitions(backend: &Backend, key: &str) -> Vec<Location> {
    let Some(root) = backend.workspace.workspace_root.read().clone() else {
        return Vec::new();
    };
    let mut fallback = None;
    let mut declarations = Vec::new();
    for name in ENV_FILES {
        let path = root.join(name);
        let (Ok(content), Ok(uri)) = (std::fs::read_to_string(&path), Url::from_file_path(&path))
        else {
            continue;
        };
        match declaring_line(&content, key) {
            Some(line) => declarations.push(crate::definition::point_location(
                uri,
                Position::new(line, 0),
            )),
            None if fallback.is_none() => {
                fallback = Some(crate::definition::point_location(uri, Position::new(0, 0)));
            }
            None => {}
        }
    }
    if declarations.is_empty() {
        return fallback.into_iter().collect();
    }
    declarations
}

/// The dotenv file that declares `key`, as a workspace-relative name.
///
/// Separate from [`resolve_env_definitions`], which points at a file even
/// when nothing in it declares the name: hover has to tell the two apart.
pub(crate) fn env_declaration_file(backend: &Backend, key: &str) -> Option<&'static str> {
    let root = backend.workspace.workspace_root.read().clone()?;
    ENV_FILES.into_iter().find(|name| {
        std::fs::read_to_string(root.join(name))
            .is_ok_and(|content| declaring_line(&content, key).is_some())
    })
}

/// Every variable name the project's dotenv files declare, sorted and
/// deduplicated.
///
/// Read from disk on demand rather than cached: there are at most two files,
/// and an edit to `.env` has to show up in the next completion.
pub(crate) fn enumerate_env_keys(backend: &Backend) -> Vec<String> {
    let Some(root) = backend.workspace.workspace_root.read().clone() else {
        return Vec::new();
    };
    let mut keys: Vec<String> = ENV_FILES
        .iter()
        .filter_map(|name| std::fs::read_to_string(root.join(name)).ok())
        .flat_map(|content| {
            content
                .lines()
                .filter_map(declared_key)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// The variable one dotenv line declares, or `None` for a comment, a blank
/// line, or anything else that is not an assignment.
fn declared_key(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    // Laravel's dotenv reader accepts an `export ` prefix.
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let key = line.split_once('=')?.0.trim_end();
    (!key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.'))
    .then_some(key)
}

/// The zero-based line `key` is declared on, or `None` when this file does
/// not declare it.
fn declaring_line(env_content: &str, key: &str) -> Option<u32> {
    env_content
        .lines()
        .position(|line| declared_key(line) == Some(key))
        .map(|index| index as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_env_key_line() {
        let env = "APP_NAME=Laravel\nDB_HOST=127.0.0.1\n";
        assert_eq!(declaring_line(env, "DB_HOST"), Some(1));
    }

    #[test]
    fn an_undeclared_key_has_no_line() {
        let env = "APP_NAME=Laravel\n";
        assert_eq!(declaring_line(env, "MISSING"), None);
    }

    /// A commented-out variable is documentation of what could be set, not a
    /// declaration, and a key mentioned inside a value is not one either.
    #[test]
    fn only_assignments_declare_a_variable() {
        let env = "# DB_HOST=127.0.0.1\nAPP_NAME=Laravel\nexport MAIL_MAILER=log\nnonsense\n";
        let declared: Vec<&str> = env.lines().filter_map(declared_key).collect();
        assert_eq!(declared, vec!["APP_NAME", "MAIL_MAILER"]);
    }

    /// A name that shares a prefix with a declared one is a different
    /// variable.
    #[test]
    fn a_prefix_of_a_declared_name_is_not_declared() {
        let env = "DB_HOSTNAME=example.test\n";
        assert_eq!(declaring_line(env, "DB_HOST"), None);
        assert_eq!(declaring_line(env, "DB_HOSTNAME"), Some(0));
    }
}
