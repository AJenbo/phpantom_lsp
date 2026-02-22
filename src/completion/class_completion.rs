/// Class name, constant, and function completions.
///
/// This module handles building completion items for bare identifiers
/// (class names, global constants, and standalone functions) when no
/// member-access operator (`->` or `::`) is present.
///
/// Also provides a Throwable-filtered variant for catch clause fallback
/// and `throw new` completion, which only suggests exception classes
/// from already-parsed sources and includes everything else (classmap,
/// stubs) unfiltered.
use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::named_args::position_to_char_offset;
use crate::types::*;
use crate::util::short_name;

use super::builder::{
    analyze_use_block, build_callable_snippet, build_use_edit, use_import_conflicts,
};

/// The syntactic context in which a class name is being completed.
///
/// Different PHP positions accept only certain kinds of class-like
/// declarations. For example, `extends` in a class declaration only
/// accepts non-final classes, while `implements` only accepts interfaces.
/// This enum lets `build_class_name_completions` filter out invalid
/// suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassNameContext {
    /// No special context. Offer all class-like types.
    Any,
    /// After `new`. Only concrete (non-abstract) classes.
    New,
    /// After `extends` in a class declaration. Only non-final classes
    /// (abstract classes are valid targets for extension).
    ExtendsClass,
    /// After `extends` in an interface declaration. Only interfaces.
    ExtendsInterface,
    /// After `implements`. Only interfaces.
    Implements,
    /// `use` inside a class body. Only traits.
    TraitUse,
    /// After `instanceof`. Classes, interfaces, and enums (not traits).
    Instanceof,
    /// Top-level `use` import (no `function`/`const` keyword).
    /// Classes, interfaces, traits, and enums only.
    UseImport,
    /// `use function`. Functions only.
    UseFunction,
    /// `use const`. Constants only.
    UseConst,
    /// After the `namespace` keyword at the top level.
    /// Only namespace names should be suggested (no class names).
    NamespaceDeclaration,
}

impl ClassNameContext {
    /// Check whether a loaded `ClassInfo` should be included in
    /// completion results for this context.
    pub(crate) fn matches(self, cls: &ClassInfo) -> bool {
        self.matches_kind_flags(cls.kind, cls.is_abstract, cls.is_final)
    }

    /// Check whether a class-like declaration with the given kind and
    /// modifier flags should be included in completion results for this
    /// context.
    ///
    /// This is the shared implementation behind both `matches` (which
    /// takes a full `ClassInfo`) and the lightweight stub scanner (which
    /// only extracts kind/abstract/final from raw PHP source).
    pub(crate) fn matches_kind_flags(
        self,
        kind: ClassLikeKind,
        is_abstract: bool,
        is_final: bool,
    ) -> bool {
        match self {
            Self::Any | Self::UseImport => true,
            Self::New => kind == ClassLikeKind::Class && !is_abstract,
            Self::ExtendsClass => kind == ClassLikeKind::Class && !is_final,
            Self::ExtendsInterface => kind == ClassLikeKind::Interface,
            Self::Implements => kind == ClassLikeKind::Interface,
            Self::TraitUse => kind == ClassLikeKind::Trait,
            Self::Instanceof => kind != ClassLikeKind::Trait,
            // UseFunction, UseConst, and NamespaceDeclaration are handled
            // specially by the handler — they never reach class-kind filtering.
            Self::UseFunction | Self::UseConst | Self::NamespaceDeclaration => false,
        }
    }

    /// Whether this context restricts completions to class-like names
    /// only (constants and functions should be suppressed).
    pub(crate) fn is_class_only(self) -> bool {
        !matches!(
            self,
            Self::Any | Self::UseFunction | Self::UseConst | Self::NamespaceDeclaration
        )
    }

    /// Whether this context should use constructor snippet insertion
    /// (only applicable after `new`).
    pub(crate) fn is_new(self) -> bool {
        matches!(self, Self::New)
    }

    /// Whether this context expects a very specific class-like kind
    /// (trait, interface) where unverifiable use-map entries should be
    /// rejected rather than shown with benefit of the doubt.
    pub(crate) fn is_narrow_kind(self) -> bool {
        matches!(
            self,
            Self::TraitUse | Self::Implements | Self::ExtendsInterface
        )
    }

    /// Heuristic check: does `short_name` look like a poor match for
    /// this context based on naming conventions alone?
    ///
    /// Used to demote (not remove) unloaded classes whose kind is
    /// unknown. For example, `LoggerInterface` is demoted in
    /// `ExtendsClass` context because it is almost certainly an
    /// interface, not an extendable class.
    pub(crate) fn likely_mismatch(self, short_name: &str) -> bool {
        match self {
            Self::New => likely_non_instantiable(short_name),
            Self::ExtendsClass => likely_interface_name(short_name),
            Self::ExtendsInterface | Self::Implements => likely_non_interface_name(short_name),
            Self::TraitUse => likely_non_instantiable(short_name),
            Self::Instanceof
            | Self::Any
            | Self::UseImport
            | Self::UseFunction
            | Self::UseConst
            | Self::NamespaceDeclaration => false,
        }
    }
}

/// Check whether the keyword `kw` ends exactly at position `end` in `chars`,
/// with a word boundary before it (i.e. the character at `end - kw.len() - 1`
/// is not alphanumeric or underscore).
fn keyword_ends_at(chars: &[char], end: usize, kw: &str) -> bool {
    let kw_len = kw.len();
    if end < kw_len {
        return false;
    }
    let start = end - kw_len;
    for (i, kc) in kw.chars().enumerate() {
        if chars[start + i] != kc {
            return false;
        }
    }
    // Word boundary: character before keyword must not be alphanumeric / underscore.
    if start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        return false;
    }
    true
}

/// Determine whether `extends` is in a class or interface declaration
/// by walking backward from the keyword through the class/interface name
/// to find the declaration keyword.
fn determine_extends_context(chars: &[char], extends_start: usize) -> ClassNameContext {
    let mut j = extends_start;

    // Skip whitespace before `extends`.
    while j > 0 && chars[j - 1].is_ascii_whitespace() {
        j -= 1;
    }

    // Skip the class/interface name (identifiers + backslash for FQN).
    while j > 0 && (chars[j - 1].is_alphanumeric() || chars[j - 1] == '_' || chars[j - 1] == '\\') {
        j -= 1;
    }

    // Skip whitespace before the name.
    while j > 0 && chars[j - 1].is_ascii_whitespace() {
        j -= 1;
    }

    if keyword_ends_at(chars, j, "interface") {
        ClassNameContext::ExtendsInterface
    } else {
        // `class`, `abstract class`, `final class`, `enum` — all
        // resolve to ExtendsClass (enums can't use extends in PHP,
        // but if a user writes it, offering classes is reasonable).
        ClassNameContext::ExtendsClass
    }
}

/// Compute the brace depth at a given character offset by counting
/// unmatched `{` and `}` from the start of the content.
///
/// This is a simple heuristic that does not account for braces inside
/// strings or comments, but is sufficient for detecting whether the
/// cursor is inside a class body.
fn brace_depth_at(chars: &[char], offset: usize) -> i32 {
    let mut depth: i32 = 0;
    for &ch in &chars[..offset] {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// Detect the syntactic context for class name completion at the given
/// cursor position.
///
/// Walks backward from the cursor through the partial identifier,
/// whitespace, and comma-separated lists to find the governing keyword
/// (`extends`, `implements`, `use`, `instanceof`, `new`).
///
/// Returns `ClassNameContext::Any` when no special context is detected.
pub(crate) fn detect_class_name_context(content: &str, position: Position) -> ClassNameContext {
    let chars: Vec<char> = content.chars().collect();
    let Some(offset) = position_to_char_offset(&chars, position) else {
        return ClassNameContext::Any;
    };

    // Walk back past the partial identifier (alphanumeric, _, \).
    let mut i = offset;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\') {
        i -= 1;
    }

    // Skip whitespace (including newlines for multi-line declarations).
    while i > 0 && chars[i - 1].is_ascii_whitespace() {
        i -= 1;
    }

    // Handle comma-separated lists (e.g. `implements Foo, Bar, Baz`).
    // Walk past `Identifier,` sequences.
    while i > 0 && chars[i - 1] == ',' {
        i -= 1; // skip comma
        // Skip whitespace.
        while i > 0 && chars[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
        // Skip identifier (including backslashes for FQNs).
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }
        // Skip whitespace.
        while i > 0 && chars[i - 1].is_ascii_whitespace() {
            i -= 1;
        }
    }

    // Now `i` points just past the keyword (if any). Check which keyword
    // precedes us.
    if keyword_ends_at(&chars, i, "instanceof") {
        return ClassNameContext::Instanceof;
    }
    if keyword_ends_at(&chars, i, "new") {
        return ClassNameContext::New;
    }
    if keyword_ends_at(&chars, i, "implements") {
        return ClassNameContext::Implements;
    }
    if keyword_ends_at(&chars, i, "extends") {
        let extends_start = i - "extends".len();
        return determine_extends_context(&chars, extends_start);
    }

    // `use function` and `use const` (two-word keywords).
    // Check for `function` / `const` first, then walk back to `use`.
    if keyword_ends_at(&chars, i, "function") {
        let kw_start = i - "function".len();
        let mut j = kw_start;
        while j > 0 && chars[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        if keyword_ends_at(&chars, j, "use") && brace_depth_at(&chars, j) < 1 {
            return ClassNameContext::UseFunction;
        }
    }
    if keyword_ends_at(&chars, i, "const") {
        let kw_start = i - "const".len();
        let mut j = kw_start;
        while j > 0 && chars[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        if keyword_ends_at(&chars, j, "use") && brace_depth_at(&chars, j) < 1 {
            return ClassNameContext::UseConst;
        }
    }

    if keyword_ends_at(&chars, i, "use") {
        // Distinguish trait `use` (inside class body, brace depth >= 1)
        // from namespace `use` (top level, brace depth 0).
        if brace_depth_at(&chars, i) >= 1 {
            return ClassNameContext::TraitUse;
        }
        return ClassNameContext::UseImport;
    }

    if keyword_ends_at(&chars, i, "namespace") && brace_depth_at(&chars, i) < 1 {
        return ClassNameContext::NamespaceDeclaration;
    }

    ClassNameContext::Any
}

/// Quickly scan raw PHP source to determine the `ClassLikeKind` (and
/// `is_abstract` / `is_final` flags) of the declaration matching
/// `name`, without performing a full parse.
///
/// Searches for `{keyword} {short_name}` where keyword is one of
/// `interface`, `trait`, `enum`, `abstract class`, `final class`, or
/// `class`.  The short name must be followed by a non-identifier
/// character to avoid partial matches (e.g. matching `Foo` inside
/// `FooBar`).
///
/// Returns `None` if the declaration cannot be found (e.g. the stub
/// file doesn't contain this particular class).
pub(crate) fn detect_stub_class_kind(
    name: &str,
    source: &str,
) -> Option<(ClassLikeKind, bool, bool)> {
    let short = short_name(name);
    let bytes = source.as_bytes();

    let mut search_from = 0;
    while let Some(rel_pos) = source[search_from..].find(short) {
        let abs_pos = search_from + rel_pos;

        // The name must be followed by a non-identifier character (or EOF).
        let after = abs_pos + short.len();
        if after < bytes.len() {
            let next = bytes[after];
            if next.is_ascii_alphanumeric() || next == b'_' {
                search_from = abs_pos + 1;
                continue;
            }
        }

        // The name must be preceded by a space (the keyword separator).
        if abs_pos == 0 || bytes[abs_pos - 1] != b' ' {
            search_from = abs_pos + 1;
            continue;
        }

        // Look at the text before the name to find the declaration keyword.
        let before = source[..abs_pos].trim_end();

        if before.ends_with("interface") {
            return Some((ClassLikeKind::Interface, false, false));
        }
        if before.ends_with("trait") {
            return Some((ClassLikeKind::Trait, false, false));
        }
        if before.ends_with("enum") {
            return Some((ClassLikeKind::Enum, false, false));
        }
        if let Some(rest) = before.strip_suffix("class") {
            let mut pre_class = rest.trim_end();
            // PHP 8.2 allows `readonly` between abstract/final and class
            // (e.g. `final readonly class Foo`).  Strip it so the
            // abstract/final check sees the right trailing keyword.
            if let Some(before_readonly) = pre_class.strip_suffix("readonly") {
                pre_class = before_readonly.trim_end();
            }
            let is_abstract = pre_class.ends_with("abstract");
            let is_final = pre_class.ends_with("final");
            return Some((ClassLikeKind::Class, is_abstract, is_final));
        }

        search_from = abs_pos + 1;
    }
    None
}

/// Heuristic: does the name look like an interface?
///
/// Matches `*Interface` suffix and `I[A-Z]` prefix (C#-style).
fn likely_interface_name(short_name: &str) -> bool {
    if short_name.to_ascii_lowercase().ends_with("interface") {
        return true;
    }
    // I[A-Z] prefix — C#-style interface naming (ILogger, IRepository).
    if short_name.starts_with('I') && short_name.len() >= 2 {
        let second = short_name.as_bytes()[1];
        if second.is_ascii_uppercase() {
            return true;
        }
    }
    false
}

/// Heuristic: does the name look like an abstract / base class rather
/// than an interface?
///
/// Matches `Abstract*`, `*Abstract`, and `Base[A-Z]*`.
fn likely_non_interface_name(short_name: &str) -> bool {
    let lower = short_name.to_ascii_lowercase();
    if lower.ends_with("abstract") || lower.starts_with("abstract") {
        return true;
    }
    if short_name.starts_with("Base") && short_name.len() >= 5 {
        let fifth = short_name.as_bytes()[4];
        if fifth.is_ascii_uppercase() {
            return true;
        }
    }
    false
}

/// Heuristic check for class names that are unlikely to be instantiable.
///
/// Returns `true` when the short name matches common naming conventions
/// for abstract classes and interfaces:
///
/// - **Abstract:** case-insensitive `"abstract"` as prefix or suffix
///   (e.g. `AbstractController`, `HandlerAbstract`)
/// - **Interface:** case-insensitive `"interface"` as suffix
///   (e.g. `LoggerInterface`)
/// - **I-prefix:** `I` followed by an uppercase letter
///   (e.g. `ILogger`, `IRepository` — C#-style interface naming)
/// - **Base-prefix:** `Base` followed by an uppercase letter
///   (e.g. `BaseController`, `BaseModel`)
fn likely_non_instantiable(short_name: &str) -> bool {
    likely_interface_name(short_name) || likely_non_interface_name(short_name)
}

/// Check whether a class name is a synthetic anonymous class name
/// (e.g. `__anonymous@27775`).  These are internal bookkeeping entries
/// that should never appear in completion results.
fn is_anonymous_class(name: &str) -> bool {
    name.starts_with("__anonymous@")
}

/// Check whether a class matches the typed prefix.
///
/// In FQN-prefix mode (`is_fqn` is `true`) both the short name and the
/// fully-qualified name are checked so that `App\Models\U` can surface
/// `App\Models\User`.  In non-FQN mode only the short name is checked
/// to avoid flooding the response with every class under a broad
/// namespace prefix.
fn matches_class_prefix(short_name: &str, fqn: &str, prefix_lower: &str, is_fqn: bool) -> bool {
    if is_fqn {
        short_name.to_lowercase().contains(prefix_lower)
            || fqn.to_lowercase().contains(prefix_lower)
    } else {
        short_name.to_lowercase().contains(prefix_lower)
    }
}

/// Try to shorten a FQN using the file's use-map.
///
/// Checks whether any existing `use` import provides a prefix (or exact
/// match) for the given FQN.  Returns the shortest reference that is
/// valid given the imports, or `None` if no shortening is possible.
///
/// Examples (given `use Cassandra\Exception;`):
///   - `Cassandra\Exception\AlreadyExistsException` → `Exception\AlreadyExistsException`
///   - `Cassandra\Exception` → `Exception`
fn shorten_fqn_via_use_map(fqn: &str, use_map: &HashMap<String, String>) -> Option<String> {
    let mut best: Option<String> = None;
    for (alias, import_fqn) in use_map {
        let shortened = if fqn == import_fqn {
            // Exact match: the full FQN is directly imported.
            Some(alias.clone())
        } else {
            // Prefix match: a parent namespace is imported.
            fqn.strip_prefix(&format!("{}\\", import_fqn))
                .map(|suffix| format!("{}\\{}", alias, suffix))
        };
        if let Some(ref s) = shortened
            && best.as_ref().is_none_or(|b| s.len() < b.len())
        {
            best = shortened;
        }
    }
    best
}

/// Compute the label, insert-text base, filter-text, and optional
/// use-import FQN for a class completion item.
///
/// In FQN-prefix mode the namespace path is shown and inserted.  When
/// the FQN belongs to the current namespace the reference is simplified
/// to a relative name (e.g. typing `\Demo\` in namespace `Demo` for
/// class `Demo\Box` produces just `Box`).
///
/// In non-FQN mode the short name is used with a full `use` import.
///
/// Returns `(label, insert_base, filter_text, use_import_fqn)`.
/// `use_import_fqn` is `None` when no `use` statement is needed (FQN
/// mode or same-namespace class).
fn class_completion_texts(
    short_name: &str,
    fqn: &str,
    is_fqn: bool,
    has_leading_backslash: bool,
    file_namespace: &Option<String>,
    _prefix_lower: &str,
) -> (String, String, String, Option<String>) {
    if is_fqn {
        // When the FQN belongs to the current namespace, simplify to a
        // relative reference so that `\Demo\` + `Demo\Box` → `Box`.
        if let Some(ns) = file_namespace {
            let ns_prefix = format!("{}\\", ns);
            if let Some(relative) = fqn.strip_prefix(&ns_prefix) {
                // Filter text keeps the full typed form so the editor's
                // fuzzy matcher still finds the item.
                let filter = if has_leading_backslash {
                    format!("\\{}", fqn)
                } else {
                    fqn.to_string()
                };
                return (relative.to_string(), relative.to_string(), filter, None);
            }
        }

        let insert = if has_leading_backslash {
            format!("\\{}", fqn)
        } else {
            fqn.to_string()
        };
        (fqn.to_string(), insert.clone(), insert, None)
    } else {
        // Non-FQN mode: insert the short name and import the full FQN.
        let filter = fqn.to_string();
        (
            short_name.to_string(),
            short_name.to_string(),
            filter,
            Some(fqn.to_string()),
        )
    }
}

impl Backend {
    /// Extract the partial identifier (class name fragment) that the user
    /// is currently typing at the given cursor position.
    ///
    /// Walks backward from the cursor through alphanumeric characters,
    /// underscores, and backslashes (namespace separators).  Returns
    /// `None` if the resulting text starts with `$` (variable context)
    /// or is empty.
    pub fn extract_partial_class_name(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards through identifier characters (including `\`)
        let mut i = col;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        if i == col {
            // Nothing typed — no partial identifier
            return None;
        }

        // If preceded by `$`, this is a variable, not a class name
        if i > 0 && chars[i - 1] == '$' {
            return None;
        }

        // If preceded by `->` or `::`, member completion handles this
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            return None;
        }
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            return None;
        }

        let partial: String = chars[i..col].iter().collect();
        if partial.is_empty() {
            return None;
        }

        Some(partial)
    }

    /// Detect whether the cursor is in a `throw new ClassName` context.
    ///
    /// Returns `true` when the text immediately before the partial
    /// identifier (at the cursor) is `throw new` (with optional
    /// whitespace).  This tells the handler to restrict completion to
    /// Throwable descendants only and skip constants / functions.
    pub(crate) fn is_throw_new_context(content: &str, position: Position) -> bool {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return false;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backward past the partial identifier (same logic as
        // extract_partial_class_name) to find where it starts.
        let mut i = col;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        // Now skip whitespace before the identifier
        let mut j = i;
        while j > 0 && chars[j - 1] == ' ' {
            j -= 1;
        }

        // Check for `new` keyword
        if j >= 3
            && chars[j - 3] == 'n'
            && chars[j - 2] == 'e'
            && chars[j - 1] == 'w'
            && (j < 4 || !chars[j - 4].is_alphanumeric())
        {
            // Skip whitespace before `new`
            let mut k = j - 3;
            while k > 0 && chars[k - 1] == ' ' {
                k -= 1;
            }

            // Check for `throw` keyword
            if k >= 5
                && chars[k - 5] == 't'
                && chars[k - 4] == 'h'
                && chars[k - 3] == 'r'
                && chars[k - 2] == 'o'
                && chars[k - 1] == 'w'
                && (k < 6 || !chars[k - 6].is_alphanumeric())
            {
                return true;
            }
        }

        false
    }

    /// Build `(insert_text, insert_text_format)` for a class in `new` context.
    ///
    /// When `ctor_params` is `Some`, those constructor parameters are used
    /// to build a snippet with tab-stops for each required argument.
    /// When `None`, a plain `Name()$0` snippet is returned so the user
    /// still gets parentheses inserted automatically.
    fn build_new_insert(
        short_name: &str,
        ctor_params: Option<&[ParameterInfo]>,
    ) -> (String, Option<InsertTextFormat>) {
        let snippet = if let Some(p) = ctor_params {
            build_callable_snippet(short_name, p)
        } else {
            // No constructor info available — insert empty parens.
            format!("{short_name}()$0")
        };

        (snippet, Some(InsertTextFormat::SNIPPET))
    }

    /// Build completion items for class names from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Classes imported via `use` statements in the current file
    ///   2. Classes in the same namespace (from the ast_map)
    ///   3. Classes from the class_index (discovered during parsing)
    ///   4. Classes from the Composer classmap (`autoload_classmap.php`)
    ///   5. Built-in PHP classes from embedded stubs
    ///
    /// Each item uses the short class name as `label` and the
    /// fully-qualified name as `detail`.  Items are deduplicated by FQN.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching classes exceeds [`MAX_CLASS_COMPLETIONS`], the result is
    /// truncated and `is_incomplete` is `true`, signalling the client to
    /// re-request as the user types more characters.
    const MAX_CLASS_COMPLETIONS: usize = 100;

    /// Build completion items for class, interface, trait, and enum names
    /// matching `prefix`.
    ///
    /// The `context` parameter controls which kinds of class-like
    /// declarations are included. For example, `ClassNameContext::Implements`
    /// filters results to interfaces only, while `ClassNameContext::Any`
    /// offers everything.
    pub(crate) fn build_class_name_completions(
        &self,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        prefix: &str,
        content: &str,
        context: ClassNameContext,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let is_new = context.is_new();
        let is_use_import = matches!(context, ClassNameContext::UseImport);
        // In FQN mode (except UseImport), try to shorten references
        // using the file's existing `use` imports.  E.g. if the user
        // has `use Cassandra\Exception;`, typing `Exception\Al` should
        // insert `Exception\AlreadyExistsException` rather than the
        // full FQN.
        let should_shorten_via_imports = !is_use_import;
        let has_leading_backslash = prefix.starts_with('\\');
        let normalized = prefix.strip_prefix('\\').unwrap_or(prefix);
        let prefix_lower = normalized.to_lowercase();
        // In UseImport context, always treat the prefix as FQN so that
        // the full qualified name is inserted (not the short name) and
        // no redundant `use` text-edit is generated.
        let is_fqn_prefix = has_leading_backslash || normalized.contains('\\') || is_use_import;

        // In UseImport context, suppress namespace-relative
        // simplification — `use User;` is wrong even when the cursor
        // file lives in the same namespace as `User`.  Passing `None`
        // makes `class_completion_texts` emit the full FQN.
        let no_namespace: Option<String> = None;
        let effective_namespace = if is_use_import {
            &no_namespace
        } else {
            file_namespace
        };

        // When the user is typing a namespace-qualified reference (e.g.
        // `http\En`, `\App\Models\U`, or `\Demo`), the editor may treat
        // `\` as a word boundary and only replace the text after the
        // last `\`.  Provide an explicit replacement range covering the
        // entire typed prefix so the editor replaces it in full.
        let fqn_replace_range = if is_fqn_prefix {
            Some(Range {
                start: Position {
                    line: position.line,
                    character: position
                        .character
                        .saturating_sub(prefix.chars().count() as u32),
                },
                end: position,
            })
        } else {
            None
        };
        let mut seen_fqns: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // Pre-compute the use-block info for alphabetical `use` insertion.
        // Only items from sources 3–5 (not already imported, not same
        // namespace) will carry an `additional_text_edits` entry.
        let use_block = analyze_use_block(content);

        // ── 1. Use-imported classes (highest priority) ──────────────
        for (short_name, fqn) in file_use_map {
            if !matches_class_prefix(short_name, fqn, &prefix_lower, is_fqn_prefix) {
                continue;
            }
            // Skip use-map entries that are namespace aliases rather
            // than actual class imports (e.g. `use Foo\Bar as FB;`
            // where `Foo\Bar` is a namespace, not a class).
            if self.is_likely_namespace_not_class(fqn) {
                continue;
            }
            if !seen_fqns.insert(fqn.clone()) {
                continue;
            }
            // Apply context-aware filtering for loaded classes.
            if context.is_class_only() && !self.matches_context_or_unloaded(fqn, context) {
                continue;
            }
            // In narrow contexts (TraitUse, Implements, ExtendsInterface)
            // the expected class-like kind is very specific.  Reject
            // use-map entries we cannot verify as actual class-likes —
            // they are likely namespace aliases or non-existent imports.
            if context.is_narrow_kind() && !self.is_known_class_like(fqn) {
                continue;
            }
            let (mut label, mut base_name, filter, _use_import) = class_completion_texts(
                short_name,
                fqn,
                is_fqn_prefix,
                has_leading_backslash,
                effective_namespace,
                &prefix_lower,
            );
            if should_shorten_via_imports
                && let Some(shortened) = shorten_fqn_via_use_map(fqn, file_use_map)
            {
                label = shortened.clone();
                base_name = shortened;
            }
            let (insert_text, insert_text_format) = if is_new {
                Self::build_new_insert(&base_name, None)
            } else {
                (base_name, None)
            };
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(fqn.clone()),
                insert_text: Some(insert_text.clone()),
                insert_text_format,
                filter_text: Some(filter),
                sort_text: Some(format!("0_{}", short_name.to_lowercase())),
                text_edit: fqn_replace_range.map(|range| {
                    CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: insert_text,
                    })
                }),
                ..CompletionItem::default()
            });
        }

        // ── 2. Same-namespace classes (from ast_map) ────────────────
        if let Some(ns) = file_namespace
            && let Ok(nmap) = self.namespace_map.lock()
        {
            // Find all URIs that share the same namespace
            let same_ns_uris: Vec<String> = nmap
                .iter()
                .filter_map(|(uri, opt_ns)| {
                    if opt_ns.as_deref() == Some(ns.as_str()) {
                        Some(uri.clone())
                    } else {
                        None
                    }
                })
                .collect();
            drop(nmap);

            if let Ok(amap) = self.ast_map.lock() {
                for uri in &same_ns_uris {
                    if let Some(classes) = amap.get(uri) {
                        for cls in classes {
                            if is_anonymous_class(&cls.name) {
                                continue;
                            }
                            let cls_fqn = format!("{}\\{}", ns, cls.name);
                            if !matches_class_prefix(
                                &cls.name,
                                &cls_fqn,
                                &prefix_lower,
                                is_fqn_prefix,
                            ) {
                                continue;
                            }
                            // Apply context-aware filtering.
                            if context.is_class_only() && !context.matches(cls) {
                                continue;
                            }
                            if !seen_fqns.insert(cls_fqn.clone()) {
                                continue;
                            }
                            let (mut label, mut base_name, filter, _use_import) =
                                class_completion_texts(
                                    &cls.name,
                                    &cls_fqn,
                                    is_fqn_prefix,
                                    has_leading_backslash,
                                    effective_namespace,
                                    &prefix_lower,
                                );
                            if should_shorten_via_imports
                                && let Some(shortened) =
                                    shorten_fqn_via_use_map(&cls_fqn, file_use_map)
                            {
                                label = shortened.clone();
                                base_name = shortened;
                            }
                            let (insert_text, insert_text_format) = if is_new {
                                // We already have the ClassInfo — check
                                // for __construct directly.
                                let ctor_params: Option<Vec<ParameterInfo>> = cls
                                    .methods
                                    .iter()
                                    .find(|m| m.name.eq_ignore_ascii_case("__construct"))
                                    .map(|m| m.parameters.clone());
                                Self::build_new_insert(&base_name, ctor_params.as_deref())
                            } else {
                                (base_name, None)
                            };
                            items.push(CompletionItem {
                                label,
                                kind: Some(CompletionItemKind::CLASS),
                                detail: Some(cls_fqn),
                                insert_text: Some(insert_text.clone()),
                                insert_text_format,
                                filter_text: Some(filter),
                                sort_text: Some(format!("1_{}", cls.name.to_lowercase())),
                                deprecated: if cls.is_deprecated { Some(true) } else { None },
                                text_edit: fqn_replace_range.map(|range| {
                                    CompletionTextEdit::Edit(TextEdit {
                                        range,
                                        new_text: insert_text,
                                    })
                                }),
                                ..CompletionItem::default()
                            });
                        }
                    }
                }
            }
        }

        // ── 3. class_index (discovered / interacted-with classes) ───
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                let sn = short_name(fqn);
                if !matches_class_prefix(sn, fqn, &prefix_lower, is_fqn_prefix) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                // Apply context-aware filtering for loaded classes.
                if context.is_class_only() && !self.matches_context_or_unloaded(fqn, context) {
                    continue;
                }
                let (mut label, mut base_name, filter, mut use_import) = class_completion_texts(
                    sn,
                    fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    effective_namespace,
                    &prefix_lower,
                );
                let mut was_shortened = false;
                if should_shorten_via_imports
                    && let Some(shortened) = shorten_fqn_via_use_map(fqn, file_use_map)
                {
                    label = shortened.clone();
                    base_name = shortened;
                    use_import = None;
                    was_shortened = true;
                }
                // When the short name conflicts with an existing import,
                // fall back to a fully-qualified reference at the usage
                // site instead of inserting a duplicate `use` statement.
                if let Some(ref import_fqn) = use_import
                    && use_import_conflicts(import_fqn, file_use_map)
                {
                    base_name = format!("\\{}", import_fqn);
                    use_import = None;
                }
                // In FQN mode, if the first namespace segment of the
                // insert text matches an existing alias (and we didn't
                // intentionally shorten through that alias), prepend `\`
                // so PHP resolves the name from the global namespace.
                if is_fqn_prefix
                    && !was_shortened
                    && !base_name.starts_with('\\')
                    && let Some(first_seg) = base_name.split('\\').next()
                    && file_use_map
                        .keys()
                        .any(|a| a.eq_ignore_ascii_case(first_seg))
                {
                    base_name = format!("\\{}", base_name);
                }
                let (insert_text, insert_text_format) = if is_new {
                    // class_index is a FQN → URI map; the class may or
                    // may not be fully loaded — just insert empty parens.
                    (format!("{base_name}()$0"), Some(InsertTextFormat::SNIPPET))
                } else {
                    (base_name, None)
                };
                // Demote names that heuristically mismatch the context
                // so better-looking candidates appear first.
                let sort_prefix = if context.likely_mismatch(sn) {
                    "7"
                } else {
                    "2"
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format,
                    filter_text: Some(filter),
                    sort_text: Some(format!("{}_{}", sort_prefix, sn.to_lowercase())),
                    text_edit: fqn_replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: insert_text,
                        })
                    }),
                    additional_text_edits: use_import.as_ref().and_then(|import_fqn| {
                        build_use_edit(import_fqn, &use_block, file_namespace)
                    }),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 4. Composer classmap (all autoloaded classes) ───────────
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                let sn = short_name(fqn);
                if !matches_class_prefix(sn, fqn, &prefix_lower, is_fqn_prefix) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                // Apply context-aware filtering for loaded classes.
                if context.is_class_only() && !self.matches_context_or_unloaded(fqn, context) {
                    continue;
                }
                let (mut label, mut base_name, filter, mut use_import) = class_completion_texts(
                    sn,
                    fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    effective_namespace,
                    &prefix_lower,
                );
                let mut was_shortened = false;
                if should_shorten_via_imports
                    && let Some(shortened) = shorten_fqn_via_use_map(fqn, file_use_map)
                {
                    label = shortened.clone();
                    base_name = shortened;
                    use_import = None;
                    was_shortened = true;
                }
                // When the short name conflicts with an existing import,
                // fall back to a fully-qualified reference at the usage
                // site instead of inserting a duplicate `use` statement.
                if let Some(ref import_fqn) = use_import
                    && use_import_conflicts(import_fqn, file_use_map)
                {
                    base_name = format!("\\{}", import_fqn);
                    use_import = None;
                }
                // In FQN mode, if the first namespace segment of the
                // insert text matches an existing alias (and we didn't
                // intentionally shorten through that alias), prepend `\`
                // so PHP resolves the name from the global namespace.
                if is_fqn_prefix
                    && !was_shortened
                    && !base_name.starts_with('\\')
                    && let Some(first_seg) = base_name.split('\\').next()
                    && file_use_map
                        .keys()
                        .any(|a| a.eq_ignore_ascii_case(first_seg))
                {
                    base_name = format!("\\{}", base_name);
                }
                let (insert_text, insert_text_format) = if is_new {
                    Self::build_new_insert(&base_name, None)
                } else {
                    (base_name, None)
                };
                // Demote names that heuristically mismatch the context
                // so better-looking candidates appear first.
                let sort_prefix = if context.likely_mismatch(sn) {
                    "8"
                } else {
                    "3"
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format,
                    filter_text: Some(filter),
                    sort_text: Some(format!("{}_{}", sort_prefix, sn.to_lowercase())),
                    text_edit: fqn_replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: insert_text,
                        })
                    }),
                    additional_text_edits: use_import.as_ref().and_then(|import_fqn| {
                        build_use_edit(import_fqn, &use_block, file_namespace)
                    }),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 5. Built-in PHP classes from stubs (lowest priority) ────
        for &name in self.stub_index.keys() {
            let sn = short_name(name);
            if !matches_class_prefix(sn, name, &prefix_lower, is_fqn_prefix) {
                continue;
            }
            if !seen_fqns.insert(name.to_string()) {
                continue;
            }
            // Apply context-aware filtering.  Unlike classmap entries
            // (where we only have a file path), stub source is already
            // in memory so we can scan it to determine the kind even
            // when the stub hasn't been fully parsed into ast_map yet.
            if context.is_class_only() {
                // Fast path: already loaded in ast_map.
                if let Some(cls) = self.find_class_in_ast_map(name) {
                    if !context.matches(&cls) {
                        continue;
                    }
                } else if let Some(source) = self.stub_index.get(name) {
                    // Slow path: scan the raw PHP source for the
                    // declaration keyword.
                    if let Some((kind, is_abstract, is_final)) =
                        detect_stub_class_kind(name, source)
                        && !context.matches_kind_flags(kind, is_abstract, is_final)
                    {
                        continue;
                    }
                    // If the scan fails, allow through.
                }
            }
            let (mut label, mut base_name, filter, mut use_import) = class_completion_texts(
                sn,
                name,
                is_fqn_prefix,
                has_leading_backslash,
                effective_namespace,
                &prefix_lower,
            );
            let mut was_shortened = false;
            if should_shorten_via_imports
                && let Some(shortened) = shorten_fqn_via_use_map(name, file_use_map)
            {
                label = shortened.clone();
                base_name = shortened;
                use_import = None;
                was_shortened = true;
            }
            // When the short name conflicts with an existing import,
            // fall back to a fully-qualified reference at the usage
            // site instead of inserting a duplicate `use` statement.
            if let Some(ref import_fqn) = use_import
                && use_import_conflicts(import_fqn, file_use_map)
            {
                base_name = format!("\\{}", import_fqn);
                use_import = None;
            }
            // In FQN mode, if the first namespace segment of the
            // insert text matches an existing alias (and we didn't
            // intentionally shorten through that alias), prepend `\`
            // so PHP resolves the name from the global namespace.
            if is_fqn_prefix
                && !was_shortened
                && !base_name.starts_with('\\')
                && let Some(first_seg) = base_name.split('\\').next()
                && file_use_map
                    .keys()
                    .any(|a| a.eq_ignore_ascii_case(first_seg))
            {
                base_name = format!("\\{}", base_name);
            }
            let (insert_text, insert_text_format) = if is_new {
                // Stub classes are not parsed yet — just insert empty
                // parens without attempting a lookup.
                (format!("{base_name}()$0"), Some(InsertTextFormat::SNIPPET))
            } else {
                (base_name, None)
            };
            // Demote names that heuristically mismatch the context
            // so better-looking candidates appear first.
            let sort_prefix = if context.likely_mismatch(sn) {
                "9"
            } else {
                "4"
            };
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(name.to_string()),
                insert_text: Some(insert_text.clone()),
                insert_text_format,
                filter_text: Some(filter),
                sort_text: Some(format!("{}_{}", sort_prefix, sn.to_lowercase())),
                text_edit: fqn_replace_range.map(|range| {
                    CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: insert_text,
                    })
                }),
                additional_text_edits: use_import
                    .as_ref()
                    .and_then(|import_fqn| build_use_edit(import_fqn, &use_block, file_namespace)),
                ..CompletionItem::default()
            });
        }

        // Cap the result set so the client isn't overwhelmed.
        // Sort by sort_text first so that higher-priority items
        // (use-imports, same-namespace, user project classes) survive
        // the truncation ahead of lower-priority SPL stubs.
        let is_incomplete = items.len() > Self::MAX_CLASS_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CLASS_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Catch clause / throw-new fallback completion ───────────────

    /// Check whether a class is a confirmed `\Throwable` descendant using
    /// only already-loaded data from the `ast_map`.
    ///
    /// Returns `true` only when the full parent chain can be walked to
    /// one of the three Throwable root types (`Throwable`, `Exception`,
    /// `Error`).  Returns `false` if the chain is broken (parent not
    /// loaded) or terminates at a non-Throwable class.
    ///
    /// This is a strict check: the caller should only include the class
    /// when this returns `true`.
    ///
    /// This never triggers disk I/O; it only consults `ast_map`.
    fn is_throwable_descendant(&self, class_name: &str, depth: u32) -> bool {
        if depth > 20 {
            return false; // prevent infinite loops
        }

        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let short = short_name(normalized);

        // These three types form the root of PHP's exception hierarchy.
        if matches!(short, "Throwable" | "Exception" | "Error") {
            return true;
        }

        // Look up ClassInfo from ast_map (no disk I/O).
        match self.find_class_in_ast_map(class_name) {
            Some(ci) => match &ci.parent_class {
                Some(parent) => self.is_throwable_descendant(parent, depth + 1),
                None => false, // no parent, not a Throwable type
            },
            None => false, // class not loaded — can't confirm
        }
    }

    /// Check whether a FQN is likely a namespace (not a class).
    ///
    /// Returns `true` only when we can *confirm* the FQN is a
    /// namespace — i.e. it is NOT a known class, AND we have positive
    /// evidence that it is a namespace (it appears as a namespace in
    /// `namespace_map`, or known classes exist under it as a prefix).
    ///
    /// When we have no information either way, returns `false` (benefit
    /// of the doubt — treat it as a potential class so undiscovered
    /// imports still appear in completions).
    fn is_likely_namespace_not_class(&self, fqn: &str) -> bool {
        // If the FQN is a known class, it's definitely not just a
        // namespace — even if classes also exist under it.
        if self.find_class_in_ast_map(fqn).is_some() {
            return false;
        }
        if let Ok(idx) = self.class_index.lock()
            && idx.contains_key(fqn)
        {
            return false;
        }
        if let Ok(cmap) = self.classmap.lock()
            && cmap.contains_key(fqn)
        {
            return false;
        }
        if self.stub_index.contains_key(fqn) {
            return false;
        }

        // Not a known class. Check for positive namespace evidence.

        // 1. Some open file declares this FQN as its namespace.
        if let Ok(nmap) = self.namespace_map.lock() {
            for ns in nmap.values().flatten() {
                if ns == fqn {
                    return true;
                }
            }
        }

        // 2. Known classes exist under this FQN as a namespace prefix.
        let prefix = format!("{}\\", fqn);
        if let Ok(idx) = self.class_index.lock()
            && idx.keys().any(|k| k.starts_with(&prefix))
        {
            return true;
        }
        if let Ok(cmap) = self.classmap.lock()
            && cmap.keys().any(|k| k.starts_with(&prefix))
        {
            return true;
        }
        if self.stub_index.keys().any(|k| k.starts_with(&prefix)) {
            return true;
        }

        // No evidence either way — benefit of the doubt.
        false
    }

    /// Check whether a class matches the given `ClassNameContext`, or
    /// allow it through if not loaded.
    ///
    /// Returns `true` when the class is found and satisfies
    /// `context.matches()`, or when the class is not in the `ast_map`
    /// but its stub source can be scanned and satisfies the context.
    /// Only returns `true` for truly unknown classes (not in ast_map
    /// and not in stub_index) as a last resort.
    fn matches_context_or_unloaded(&self, class_name: &str, context: ClassNameContext) -> bool {
        match self.find_class_in_ast_map(class_name) {
            Some(c) => context.matches(&c),
            None => {
                // Fall back to scanning the raw stub source to determine
                // the class kind without fully parsing it.
                if let Some(source) = self.stub_index.get(class_name)
                    && let Some((kind, is_abstract, is_final)) =
                        detect_stub_class_kind(class_name, source)
                {
                    return context.matches_kind_flags(kind, is_abstract, is_final);
                }
                // Truly unknown — allow through.
                true
            }
        }
    }

    /// Check whether `class_name` exists in any class source (ast_map,
    /// class_index, classmap, or stub_index).
    ///
    /// Used to reject use-map entries in narrow contexts (e.g.
    /// `TraitUse`, `Implements`) where showing an unverifiable FQN is
    /// worse than hiding it.
    fn is_known_class_like(&self, class_name: &str) -> bool {
        if self.find_class_in_ast_map(class_name).is_some() {
            return true;
        }
        if self.stub_index.contains_key(class_name) {
            return true;
        }
        if let Ok(idx) = self.class_index.lock()
            && idx.contains_key(class_name)
        {
            return true;
        }
        if let Ok(cmap) = self.classmap.lock()
            && cmap.contains_key(class_name)
        {
            return true;
        }
        false
    }

    /// Check whether the class identified by `class_name` is a concrete,
    /// non-abstract `class` (i.e. `ClassLikeKind::Class` and not
    /// `is_abstract`) in the `ast_map`.
    ///
    /// Returns `false` for interfaces, traits, enums, abstract classes,
    /// and classes that are not currently loaded.  This never triggers
    /// disk I/O.
    fn is_concrete_class_in_ast_map(&self, class_name: &str) -> bool {
        self.find_class_in_ast_map(class_name)
            .is_some_and(|c| c.kind == ClassLikeKind::Class && !c.is_abstract)
    }

    /// Collect the FQN of every class that is currently loaded in the
    /// `ast_map`.  Used by `build_catch_class_name_completions` so that
    /// classmap / stub sources can skip classes we already evaluated.
    fn collect_loaded_fqns(&self) -> HashSet<String> {
        let mut loaded = HashSet::new();
        let Ok(amap) = self.ast_map.lock() else {
            return loaded;
        };
        let nmap = self.namespace_map.lock().ok();
        for (uri, classes) in amap.iter() {
            let file_ns = nmap
                .as_ref()
                .and_then(|nm| nm.get(uri))
                .and_then(|opt| opt.as_deref());
            for cls in classes {
                let fqn = if let Some(ns) = file_ns {
                    format!("{}\\{}", ns, cls.name)
                } else {
                    cls.name.clone()
                };
                loaded.insert(fqn);
            }
        }
        loaded
    }

    /// Build completion items for class names, filtered for Throwable
    /// descendants.  Used as the catch clause fallback when no specific
    /// `@throws` types were discovered in the try block, and for
    /// `throw new` completion.
    ///
    /// The logic follows this priority:
    ///
    /// 1. **Loaded concrete classes** (use-imports, same-namespace,
    ///    class_index): only classes (not interfaces/traits/enums) whose
    ///    parent chain is fully walkable to `\Throwable` / `\Exception`
    ///    / `\Error`.
    /// 2. **Classmap** entries (not yet parsed) whose short name ends
    ///    with `Exception` — filtered to exclude already-loaded FQNs.
    /// 3. **Stub** entries whose short name ends with `Exception` —
    ///    filtered to exclude already-loaded FQNs.
    /// 4. **Classmap** entries that do *not* end with `Exception`.
    /// 5. **Stub** entries that do *not* end with `Exception`.
    pub(crate) fn build_catch_class_name_completions(
        &self,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        prefix: &str,
        content: &str,
        is_new: bool,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let has_leading_backslash = prefix.starts_with('\\');
        let normalized = prefix.strip_prefix('\\').unwrap_or(prefix);
        let prefix_lower = normalized.to_lowercase();
        let is_fqn_prefix = has_leading_backslash || normalized.contains('\\');

        // When the user is typing a namespace-qualified reference,
        // provide an explicit replacement range so the editor replaces
        // the entire typed prefix (including namespace separators).
        let fqn_replace_range = if is_fqn_prefix {
            Some(Range {
                start: Position {
                    line: position.line,
                    character: position
                        .character
                        .saturating_sub(prefix.chars().count() as u32),
                },
                end: position,
            })
        } else {
            None
        };
        let mut seen_fqns: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        let use_block = analyze_use_block(content);

        // Build the set of every FQN currently in the ast_map so that
        // classmap / stub sources can exclude already-evaluated classes.
        let loaded_fqns = self.collect_loaded_fqns();

        // ── 1a. Use-imported classes (must be concrete + Throwable) ─
        for (short_name, fqn) in file_use_map {
            if !matches_class_prefix(short_name, fqn, &prefix_lower, is_fqn_prefix) {
                continue;
            }
            if !seen_fqns.insert(fqn.clone()) {
                continue;
            }
            // Only concrete classes (not interfaces/traits/enums)
            if !self.is_concrete_class_in_ast_map(fqn) {
                continue;
            }
            // Strict check: only include if confirmed Throwable descendant
            if !self.is_throwable_descendant(fqn, 0) {
                continue;
            }
            let (label, base_name, filter, _use_import) = class_completion_texts(
                short_name,
                fqn,
                is_fqn_prefix,
                has_leading_backslash,
                file_namespace,
                &prefix_lower,
            );
            let (insert_text, insert_text_format) = if is_new {
                Self::build_new_insert(&base_name, None)
            } else {
                (base_name, None)
            };
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(fqn.clone()),
                insert_text: Some(insert_text.clone()),
                insert_text_format,
                filter_text: Some(filter),
                sort_text: Some(format!("0_{}", short_name.to_lowercase())),
                text_edit: fqn_replace_range.map(|range| {
                    CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: insert_text.clone(),
                    })
                }),
                ..CompletionItem::default()
            });
        }

        // ── 1b. Same-namespace classes (must be concrete + Throwable)
        // Collect candidates while holding the lock, then drop the lock
        // before calling `is_throwable_descendant` (which re-locks
        // `ast_map` internally — Rust's Mutex is not re-entrant).
        if let Some(ns) = file_namespace
            && let Ok(nmap) = self.namespace_map.lock()
        {
            let same_ns_uris: Vec<String> = nmap
                .iter()
                .filter_map(|(uri, opt_ns)| {
                    if opt_ns.as_deref() == Some(ns.as_str()) {
                        Some(uri.clone())
                    } else {
                        None
                    }
                })
                .collect();
            drop(nmap);

            // Phase 1: collect candidate (name, fqn, is_deprecated)
            // tuples under the ast_map lock — only concrete classes.
            let mut candidates: Vec<(String, String, bool)> = Vec::new();
            if let Ok(amap) = self.ast_map.lock() {
                for uri in &same_ns_uris {
                    if let Some(classes) = amap.get(uri) {
                        for cls in classes {
                            if is_anonymous_class(&cls.name) {
                                continue;
                            }
                            if cls.kind != ClassLikeKind::Class || cls.is_abstract {
                                continue;
                            }
                            let cls_fqn = format!("{}\\{}", ns, cls.name);
                            if !matches_class_prefix(
                                &cls.name,
                                &cls_fqn,
                                &prefix_lower,
                                is_fqn_prefix,
                            ) {
                                continue;
                            }
                            let fqn = cls_fqn;
                            if !seen_fqns.insert(fqn.clone()) {
                                continue;
                            }
                            candidates.push((cls.name.clone(), fqn, cls.is_deprecated));
                        }
                    }
                }
            }
            // Phase 2: filter by Throwable ancestry without holding locks.
            for (name, fqn, is_deprecated) in candidates {
                if !self.is_throwable_descendant(&fqn, 0) {
                    continue;
                }
                let (label, base_name, filter, _use_import) = class_completion_texts(
                    &name,
                    &fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    file_namespace,
                    &prefix_lower,
                );
                let (insert_text, insert_text_format) = if is_new {
                    // Same-namespace classes are collected without
                    // ClassInfo, so we cannot check __construct here.
                    Self::build_new_insert(&base_name, None)
                } else {
                    (base_name, None)
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format,
                    filter_text: Some(filter),
                    sort_text: Some(format!("1_{}", name.to_lowercase())),
                    deprecated: if is_deprecated { Some(true) } else { None },
                    text_edit: fqn_replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: insert_text.clone(),
                        })
                    }),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 1c. class_index (must be concrete + Throwable) ──────────
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                let sn = short_name(fqn);
                if !matches_class_prefix(sn, fqn, &prefix_lower, is_fqn_prefix) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                if !self.is_concrete_class_in_ast_map(fqn) {
                    continue;
                }
                if !self.is_throwable_descendant(fqn, 0) {
                    continue;
                }
                let (label, mut base_name, filter, mut use_import) = class_completion_texts(
                    sn,
                    fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    file_namespace,
                    &prefix_lower,
                );
                // When the short name conflicts with an existing import,
                // fall back to a fully-qualified reference at the usage
                // site instead of inserting a duplicate `use` statement.
                if let Some(ref import_fqn) = use_import
                    && use_import_conflicts(import_fqn, file_use_map)
                {
                    base_name = format!("\\{}", import_fqn);
                    use_import = None;
                }
                // In FQN mode, if the first namespace segment of the
                // insert text matches an existing alias, prepend `\`
                // so PHP resolves the name from the global namespace.
                if is_fqn_prefix
                    && !base_name.starts_with('\\')
                    && let Some(first_seg) = base_name.split('\\').next()
                    && file_use_map
                        .keys()
                        .any(|a| a.eq_ignore_ascii_case(first_seg))
                {
                    base_name = format!("\\{}", base_name);
                }
                let (insert_text, insert_text_format) = if is_new {
                    (format!("{base_name}()$0"), Some(InsertTextFormat::SNIPPET))
                } else {
                    (base_name, None)
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format,
                    filter_text: Some(filter),
                    sort_text: Some(format!("2_{}", sn.to_lowercase())),
                    text_edit: fqn_replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: insert_text,
                        })
                    }),
                    additional_text_edits: use_import.as_ref().and_then(|import_fqn| {
                        build_use_edit(import_fqn, &use_block, file_namespace)
                    }),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Classmap — names ending with "Exception" ─────────────
        // ── 4. Classmap — names NOT ending with "Exception" ─────────
        // We collect both buckets in a single pass over the classmap and
        // assign different sort_text prefixes so "Exception" entries
        // appear first.
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                if loaded_fqns.contains(fqn) {
                    continue;
                }
                let sn = short_name(fqn);
                if !matches_class_prefix(sn, fqn, &prefix_lower, is_fqn_prefix) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                let prefix_num = if sn.ends_with("Exception") { "3" } else { "5" };
                let (label, mut base_name, filter, mut use_import) = class_completion_texts(
                    sn,
                    fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    file_namespace,
                    &prefix_lower,
                );
                // When the short name conflicts with an existing import,
                // fall back to a fully-qualified reference at the usage
                // site instead of inserting a duplicate `use` statement.
                if let Some(ref import_fqn) = use_import
                    && use_import_conflicts(import_fqn, file_use_map)
                {
                    base_name = format!("\\{}", import_fqn);
                    use_import = None;
                }
                // In FQN mode, if the first namespace segment of the
                // insert text matches an existing alias, prepend `\`
                // so PHP resolves the name from the global namespace.
                if is_fqn_prefix
                    && !base_name.starts_with('\\')
                    && let Some(first_seg) = base_name.split('\\').next()
                    && file_use_map
                        .keys()
                        .any(|a| a.eq_ignore_ascii_case(first_seg))
                {
                    base_name = format!("\\{}", base_name);
                }
                let (insert_text, insert_text_format) = if is_new {
                    (format!("{base_name}()$0"), Some(InsertTextFormat::SNIPPET))
                } else {
                    (base_name, None)
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format,
                    filter_text: Some(filter),
                    sort_text: Some(format!("{}_{}", prefix_num, sn.to_lowercase())),
                    text_edit: fqn_replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: insert_text,
                        })
                    }),
                    additional_text_edits: use_import.as_ref().and_then(|import_fqn| {
                        build_use_edit(import_fqn, &use_block, file_namespace)
                    }),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 3. Stubs — names ending with "Exception" ────────────────
        // ── 5. Stubs — names NOT ending with "Exception" ────────────
        for &name in self.stub_index.keys() {
            if loaded_fqns.contains(name) {
                continue;
            }
            let sn = short_name(name);
            if !matches_class_prefix(sn, name, &prefix_lower, is_fqn_prefix) {
                continue;
            }
            if !seen_fqns.insert(name.to_string()) {
                continue;
            }
            let prefix_num = if sn.ends_with("Exception") { "4" } else { "6" };
            let (label, mut base_name, filter, mut use_import) = class_completion_texts(
                sn,
                name,
                is_fqn_prefix,
                has_leading_backslash,
                file_namespace,
                &prefix_lower,
            );
            // When the short name conflicts with an existing import,
            // fall back to a fully-qualified reference at the usage
            // site instead of inserting a duplicate `use` statement.
            if let Some(ref import_fqn) = use_import
                && use_import_conflicts(import_fqn, file_use_map)
            {
                base_name = format!("\\{}", import_fqn);
                use_import = None;
            }
            // In FQN mode, if the first namespace segment of the
            // insert text matches an existing alias, prepend `\`
            // so PHP resolves the name from the global namespace.
            if is_fqn_prefix
                && !base_name.starts_with('\\')
                && let Some(first_seg) = base_name.split('\\').next()
                && file_use_map
                    .keys()
                    .any(|a| a.eq_ignore_ascii_case(first_seg))
            {
                base_name = format!("\\{}", base_name);
            }
            let (insert_text, insert_text_format) = if is_new {
                (format!("{base_name}()$0"), Some(InsertTextFormat::SNIPPET))
            } else {
                (base_name, None)
            };
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(name.to_string()),
                insert_text: Some(insert_text.clone()),
                insert_text_format,
                filter_text: Some(filter),
                sort_text: Some(format!("{}_{}", prefix_num, sn.to_lowercase())),
                text_edit: fqn_replace_range.map(|range| {
                    CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: insert_text,
                    })
                }),
                additional_text_edits: use_import
                    .as_ref()
                    .and_then(|import_fqn| build_use_edit(import_fqn, &use_block, file_namespace)),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_CLASS_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CLASS_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Constant name completion ───────────────────────────────────

    /// Build completion items for standalone constants (`define()` constants)
    /// from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Constants discovered from parsed files (`global_defines`)
    ///   2. Built-in PHP constants from embedded stubs (`stub_constant_index`)
    ///
    /// Each item uses the constant name as `label` and the source as `detail`.
    /// Items are deduplicated by name.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching constants exceeds [`MAX_CONSTANT_COMPLETIONS`], the result
    /// is truncated and `is_incomplete` is `true`.
    const MAX_CONSTANT_COMPLETIONS: usize = 100;

    /// Build completion items for global constants matching `prefix`.
    pub(crate) fn build_constant_completions(&self, prefix: &str) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined constants (from parsed files) ───────────
        if let Ok(dmap) = self.global_defines.lock() {
            for (name, _uri) in dmap.iter() {
                if !name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen.insert(name.clone()) {
                    continue;
                }
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some("define constant".to_string()),
                    insert_text: Some(name.clone()),
                    filter_text: Some(name.clone()),
                    sort_text: Some(format!("5_{}", name.to_lowercase())),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Built-in PHP constants from stubs ────────────────────
        for &name in self.stub_constant_index.keys() {
            if !name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some("PHP constant".to_string()),
                insert_text: Some(name.to_string()),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("6_{}", name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_CONSTANT_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CONSTANT_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Function name completion ───────────────────────────────────

    /// Build a label showing the full function signature.
    ///
    /// Example: `array_map(callable|null $callback, array $array, array ...$arrays): array`
    pub(crate) fn build_function_label(func: &FunctionInfo) -> String {
        let params: Vec<String> = func
            .parameters
            .iter()
            .map(|p| {
                let mut parts = Vec::new();
                if let Some(ref th) = p.type_hint {
                    parts.push(th.clone());
                }
                if p.is_reference {
                    parts.push(format!("&{}", p.name));
                } else if p.is_variadic {
                    parts.push(format!("...{}", p.name));
                } else {
                    parts.push(p.name.clone());
                }
                let param_str = parts.join(" ");
                if !p.is_required && !p.is_variadic {
                    format!("{} = ...", param_str)
                } else {
                    param_str
                }
            })
            .collect();

        let ret = func
            .return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        format!("{}({}){}", func.name, params.join(", "), ret)
    }

    /// Build completion items for standalone functions from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Functions discovered from parsed files (`global_functions`)
    ///   2. Built-in PHP functions from embedded stubs (`stub_function_index`)
    ///
    /// For user-defined functions (source 1), the full signature is shown in
    /// the label because we already have a parsed `FunctionInfo`.  For stub
    /// functions (source 2), only the function name is shown to avoid the
    /// cost of parsing every matching stub at completion time.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching functions exceeds [`MAX_FUNCTION_COMPLETIONS`], the result
    /// is truncated and `is_incomplete` is `true`.
    const MAX_FUNCTION_COMPLETIONS: usize = 100;

    /// Build completion items for global functions matching `prefix`.
    ///
    /// When `for_use_import` is `true` the items are tailored for a
    /// `use function` statement: no parentheses are appended and the
    /// insert text format is plain text instead of a snippet.
    pub(crate) fn build_function_completions(
        &self,
        prefix: &str,
        for_use_import: bool,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined functions (from parsed files) ───────────
        if let Ok(fmap) = self.global_functions.lock() {
            for (name, (_uri, info)) in fmap.iter() {
                if !name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                // Use the short name for deduplication — if a user-defined
                // function shadows a built-in, the user version wins.
                if !seen.insert(info.name.clone()) {
                    continue;
                }
                let label = Self::build_function_label(info);
                let (insert_text, insert_text_format) = if for_use_import {
                    (info.name.clone(), None)
                } else {
                    (
                        build_callable_snippet(&info.name, &info.parameters),
                        Some(InsertTextFormat::SNIPPET),
                    )
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("function".to_string()),
                    insert_text: Some(insert_text),
                    insert_text_format,
                    filter_text: Some(info.name.clone()),
                    sort_text: Some(format!("4_{}", info.name.to_lowercase())),
                    deprecated: if info.is_deprecated { Some(true) } else { None },
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Built-in PHP functions from stubs ────────────────────
        for &name in self.stub_function_index.keys() {
            if !name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            let (insert_text, insert_text_format) = if for_use_import {
                (name.to_string(), None)
            } else {
                (format!("{name}()$0"), Some(InsertTextFormat::SNIPPET))
            };
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("PHP function".to_string()),
                insert_text: Some(insert_text),
                insert_text_format,
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("5_{}", name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_FUNCTION_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_FUNCTION_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Namespace declaration completion ───────────────────────────

    /// Maximum number of namespace suggestions to return.
    const MAX_NAMESPACE_COMPLETIONS: usize = 100;

    /// Build completion items for a `namespace` declaration.
    ///
    /// Only namespaces that fall under a known PSR-4 prefix are
    /// suggested.  The sources are:
    ///   1. PSR-4 mapping prefixes themselves (exploded to every level)
    ///   2. Namespace portions of FQNs from `namespace_map`,
    ///      `class_index`, `classmap`, and `ast_map` — but only when
    ///      they start with a PSR-4 prefix.
    ///
    /// Every accepted namespace is exploded to each intermediate level
    /// (e.g. `A\B\C` also inserts `A\B` and `A`).
    ///
    /// Returns `(items, is_incomplete)`.
    pub(crate) fn build_namespace_completions(
        &self,
        prefix: &str,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut namespaces: HashSet<String> = HashSet::new();

        // Collect the project's own PSR-4 prefixes (without trailing
        // `\`) so we can gate which cache entries are eligible.  Vendor
        // packages are excluded — you would never declare a namespace
        // that lives inside a vendor package.
        let psr4_prefixes: Vec<String> = self
            .psr4_mappings
            .lock()
            .ok()
            .map(|mappings| {
                mappings
                    .iter()
                    .filter(|m| !m.is_vendor)
                    .map(|m| m.prefix.trim_end_matches('\\').to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Helper: insert a namespace and all its parent namespaces.
        fn insert_with_parents(ns: &str, set: &mut HashSet<String>) {
            if ns.is_empty() {
                return;
            }
            set.insert(ns.to_string());
            let mut parts: Vec<&str> = ns.split('\\').collect();
            while parts.len() > 1 {
                parts.pop();
                set.insert(parts.join("\\"));
            }
        }

        /// Check whether `ns` falls under one of the PSR-4 prefixes.
        fn under_psr4(ns: &str, prefixes: &[String]) -> bool {
            prefixes
                .iter()
                .any(|p| ns == p || ns.starts_with(&format!("{}\\", p)))
        }

        // Helper: insert ns (and parents) only if under a PSR-4 prefix.
        fn insert_if_under_psr4(ns: &str, set: &mut HashSet<String>, prefixes: &[String]) {
            if under_psr4(ns, prefixes) {
                insert_with_parents(ns, set);
            }
        }

        // ── 1. PSR-4 prefixes (always included, exploded) ───────────
        for p in &psr4_prefixes {
            insert_with_parents(p, &mut namespaces);
        }

        // ── 2. namespace_map (already-opened files) ─────────────────
        if let Ok(nmap) = self.namespace_map.lock() {
            for ns in nmap.values().flatten() {
                insert_if_under_psr4(ns, &mut namespaces, &psr4_prefixes);
            }
        }

        // ── 3. ast_map namespace portions ───────────────────────────
        if let Ok(amap) = self.ast_map.lock() {
            let nmap = self.namespace_map.lock().ok();
            for (uri, classes) in amap.iter() {
                let file_ns = nmap
                    .as_ref()
                    .and_then(|nm| nm.get(uri))
                    .and_then(|opt| opt.as_deref());
                if let Some(ns) = file_ns {
                    for cls in classes {
                        let fqn = format!("{}\\{}", ns, cls.name);
                        if let Some(ns_end) = fqn.rfind('\\') {
                            insert_if_under_psr4(&fqn[..ns_end], &mut namespaces, &psr4_prefixes);
                        }
                    }
                }
            }
        }

        // ── 4. class_index + classmap namespace portions ────────────
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                if let Some(ns_end) = fqn.rfind('\\') {
                    insert_if_under_psr4(&fqn[..ns_end], &mut namespaces, &psr4_prefixes);
                }
            }
        }
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                if let Some(ns_end) = fqn.rfind('\\') {
                    insert_if_under_psr4(&fqn[..ns_end], &mut namespaces, &psr4_prefixes);
                }
            }
        }

        // When the typed prefix contains a backslash the editor may
        // only replace the segment after the last `\`.  Provide an
        // explicit replacement range covering the entire typed prefix
        // so that picking `Tests\Feature\Domain` after typing
        // `Tests\Feature\D` replaces the whole thing instead of
        // inserting a duplicate prefix.
        let replace_range = if prefix.contains('\\') {
            Some(Range {
                start: Position {
                    line: position.line,
                    character: position
                        .character
                        .saturating_sub(prefix.chars().count() as u32),
                },
                end: position,
            })
        } else {
            None
        };

        // ── Filter and build items ──────────────────────────────────
        let mut items: Vec<CompletionItem> = namespaces
            .into_iter()
            .filter(|ns| ns.to_lowercase().contains(&prefix_lower))
            .map(|ns| {
                let sn = ns.rsplit('\\').next().unwrap_or(&ns);
                CompletionItem {
                    label: ns.clone(),
                    kind: Some(CompletionItemKind::MODULE),
                    insert_text: Some(ns.clone()),
                    filter_text: Some(ns.clone()),
                    sort_text: Some(format!("0_{}", sn.to_lowercase())),
                    text_edit: replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: ns,
                        })
                    }),
                    ..CompletionItem::default()
                }
            })
            .collect();

        let is_incomplete = items.len() > Self::MAX_NAMESPACE_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_NAMESPACE_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClassLikeKind;

    // ── detect_stub_class_kind ──────────────────────────────────────

    #[test]
    fn test_detect_class_in_single_class_file() {
        let source = "<?php\nclass DateTime {\n}\n";
        let result = detect_stub_class_kind("DateTime", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Class, false, false)),
            "should detect a plain class"
        );
    }

    #[test]
    fn test_detect_interface_in_single_file() {
        let source = "<?php\ninterface JsonSerializable\n{\n}\n";
        let result = detect_stub_class_kind("JsonSerializable", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Interface, false, false)),
            "should detect an interface"
        );
    }

    #[test]
    fn test_detect_abstract_class() {
        let source = "<?php\nabstract class SplHeap implements Iterator, Countable\n{\n}\n";
        let result = detect_stub_class_kind("SplHeap", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Class, true, false)),
            "should detect an abstract class"
        );
    }

    #[test]
    fn test_detect_final_class() {
        let source = "<?php\nfinal class Closure {\n}\n";
        let result = detect_stub_class_kind("Closure", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Class, false, true)),
            "should detect a final class"
        );
    }

    #[test]
    fn test_detect_readonly_class() {
        let source = "<?php\nreadonly class Value {\n}\n";
        let result = detect_stub_class_kind("Value", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Class, false, false)),
            "readonly class is neither abstract nor final"
        );
    }

    #[test]
    fn test_detect_final_readonly_class() {
        let source = "<?php\nfinal readonly class Immutable {\n}\n";
        let result = detect_stub_class_kind("Immutable", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Class, false, true)),
            "should detect final through readonly"
        );
    }

    #[test]
    fn test_detect_abstract_readonly_class() {
        let source = "<?php\nabstract readonly class Base {\n}\n";
        let result = detect_stub_class_kind("Base", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Class, true, false)),
            "should detect abstract through readonly"
        );
    }

    #[test]
    fn test_detect_trait() {
        let source = "<?php\ntrait Stringable {\n}\n";
        let result = detect_stub_class_kind("Stringable", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Trait, false, false)),
            "should detect a trait"
        );
    }

    #[test]
    fn test_detect_enum() {
        let source = "<?php\nenum Suit {\n}\n";
        let result = detect_stub_class_kind("Suit", source);
        assert_eq!(
            result,
            Some((ClassLikeKind::Enum, false, false)),
            "should detect an enum"
        );
    }

    #[test]
    fn test_detect_class_in_multi_class_file() {
        // Simulates SPL_c1.php which has many classes and a few interfaces.
        let source = concat!(
            "<?php\n",
            "class SplFileInfo implements Stringable\n{\n}\n",
            "class DirectoryIterator extends SplFileInfo implements SeekableIterator\n{\n}\n",
            "class FilesystemIterator extends DirectoryIterator\n{\n}\n",
            "abstract class SplHeap implements Iterator, Countable\n{\n}\n",
            "interface SplObserver\n{\n}\n",
            "interface SplSubject\n{\n}\n",
            "class SplObjectStorage implements Countable\n{\n}\n",
        );

        assert_eq!(
            detect_stub_class_kind("DirectoryIterator", source),
            Some((ClassLikeKind::Class, false, false)),
            "should find DirectoryIterator as a class in a multi-class file"
        );
        assert_eq!(
            detect_stub_class_kind("SplHeap", source),
            Some((ClassLikeKind::Class, true, false)),
            "should find SplHeap as an abstract class"
        );
        assert_eq!(
            detect_stub_class_kind("SplObserver", source),
            Some((ClassLikeKind::Interface, false, false)),
            "should find SplObserver as an interface"
        );
        assert_eq!(
            detect_stub_class_kind("SplObjectStorage", source),
            Some((ClassLikeKind::Class, false, false)),
            "should find SplObjectStorage as a class"
        );
    }

    #[test]
    fn test_detect_does_not_match_substring() {
        // "Iterator" appears as a substring in "DirectoryIterator" and
        // "FilesystemIterator".  The word boundary check must prevent a
        // false match.
        let source = concat!(
            "<?php\n",
            "interface Iterator\n{\n}\n",
            "class DirectoryIterator extends SplFileInfo\n{\n}\n",
        );

        assert_eq!(
            detect_stub_class_kind("Iterator", source),
            Some((ClassLikeKind::Interface, false, false)),
            "should match the standalone 'Iterator' interface, not the substring in DirectoryIterator"
        );
    }

    #[test]
    fn test_detect_does_not_match_superstring() {
        // Searching for "Directory" should NOT match "DirectoryIterator".
        let source = "<?php\nclass DirectoryIterator extends SplFileInfo\n{\n}\n";
        assert_eq!(
            detect_stub_class_kind("Directory", source),
            None,
            "should not match 'Directory' inside 'DirectoryIterator'"
        );
    }

    #[test]
    fn test_detect_skips_name_in_comments() {
        // The class name appears in a docblock comment, not a declaration.
        let source = concat!(
            "<?php\n",
            "/**\n",
            " * @see DirectoryIterator\n",
            " */\n",
            "class DirectoryIterator extends SplFileInfo\n{\n}\n",
        );
        assert_eq!(
            detect_stub_class_kind("DirectoryIterator", source),
            Some((ClassLikeKind::Class, false, false)),
            "should skip the comment mention and find the actual class declaration"
        );
    }

    #[test]
    fn test_detect_skips_extends_mention() {
        // "SplFileInfo" appears after `extends`, not as a declaration keyword.
        let source = concat!(
            "<?php\n",
            "class DirectoryIterator extends SplFileInfo\n{\n}\n",
        );
        assert_eq!(
            detect_stub_class_kind("SplFileInfo", source),
            None,
            "should not match SplFileInfo in 'extends SplFileInfo' (no declaration keyword before it)"
        );
    }

    #[test]
    fn test_detect_with_fqn_key() {
        // The stub_index key might be a FQN like "Ds\\Set".
        // detect_stub_class_kind should extract the short name "Set".
        let source = concat!(
            "<?php\n",
            "namespace Ds;\n",
            "class Set implements Collection\n{\n}\n",
        );
        assert_eq!(
            detect_stub_class_kind("Ds\\Set", source),
            Some((ClassLikeKind::Class, false, false)),
            "should handle FQN keys by extracting the short name"
        );
    }

    #[test]
    fn test_detect_not_found() {
        let source = "<?php\nclass Foo {\n}\n";
        assert_eq!(
            detect_stub_class_kind("Bar", source),
            None,
            "should return None when the class is not in the source"
        );
    }

    #[test]
    fn test_detect_class_with_extends_and_implements() {
        let source = "<?php\nclass SplFixedArray implements Iterator, ArrayAccess, Countable, IteratorAggregate, JsonSerializable\n{\n}\n";
        assert_eq!(
            detect_stub_class_kind("SplFixedArray", source),
            Some((ClassLikeKind::Class, false, false)),
            "should detect a class with multiple implements"
        );
    }

    // ── ClassNameContext::matches_kind_flags ─────────────────────────

    #[test]
    fn test_extends_class_rejects_interface() {
        assert!(
            !ClassNameContext::ExtendsClass.matches_kind_flags(
                ClassLikeKind::Interface,
                false,
                false
            ),
            "ExtendsClass should reject interfaces"
        );
    }

    #[test]
    fn test_extends_class_rejects_final() {
        assert!(
            !ClassNameContext::ExtendsClass.matches_kind_flags(ClassLikeKind::Class, false, true),
            "ExtendsClass should reject final classes"
        );
    }

    #[test]
    fn test_extends_class_accepts_abstract() {
        assert!(
            ClassNameContext::ExtendsClass.matches_kind_flags(ClassLikeKind::Class, true, false),
            "ExtendsClass should accept abstract classes"
        );
    }

    #[test]
    fn test_implements_accepts_interface() {
        assert!(
            ClassNameContext::Implements.matches_kind_flags(ClassLikeKind::Interface, false, false),
            "Implements should accept interfaces"
        );
    }

    #[test]
    fn test_implements_rejects_class() {
        assert!(
            !ClassNameContext::Implements.matches_kind_flags(ClassLikeKind::Class, false, false),
            "Implements should reject classes"
        );
    }

    #[test]
    fn test_trait_use_accepts_trait() {
        assert!(
            ClassNameContext::TraitUse.matches_kind_flags(ClassLikeKind::Trait, false, false),
            "TraitUse should accept traits"
        );
    }

    #[test]
    fn test_trait_use_rejects_class() {
        assert!(
            !ClassNameContext::TraitUse.matches_kind_flags(ClassLikeKind::Class, false, false),
            "TraitUse should reject classes"
        );
    }

    #[test]
    fn test_instanceof_rejects_trait() {
        assert!(
            !ClassNameContext::Instanceof.matches_kind_flags(ClassLikeKind::Trait, false, false),
            "Instanceof should reject traits"
        );
    }

    #[test]
    fn test_instanceof_accepts_enum() {
        assert!(
            ClassNameContext::Instanceof.matches_kind_flags(ClassLikeKind::Enum, false, false),
            "Instanceof should accept enums"
        );
    }

    #[test]
    fn test_new_rejects_abstract() {
        assert!(
            !ClassNameContext::New.matches_kind_flags(ClassLikeKind::Class, true, false),
            "New should reject abstract classes"
        );
    }

    #[test]
    fn test_new_rejects_interface() {
        assert!(
            !ClassNameContext::New.matches_kind_flags(ClassLikeKind::Interface, false, false),
            "New should reject interfaces"
        );
    }

    // ── UseImport / UseFunction / UseConst detection ────────────────

    #[test]
    fn test_detect_use_import_context() {
        let content = "<?php\nuse App";
        let pos = Position {
            line: 1,
            character: 7,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::UseImport,
            "Top-level `use` should produce UseImport"
        );
    }

    #[test]
    fn test_detect_use_function_context() {
        let content = "<?php\nuse function array";
        let pos = Position {
            line: 1,
            character: 19,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::UseFunction,
            "`use function` should produce UseFunction"
        );
    }

    #[test]
    fn test_detect_use_const_context() {
        let content = "<?php\nuse const PHP";
        let pos = Position {
            line: 1,
            character: 14,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::UseConst,
            "`use const` should produce UseConst"
        );
    }

    #[test]
    fn test_detect_use_inside_class_body_is_trait_use() {
        let content = "<?php\nclass Foo {\n    use Some";
        let pos = Position {
            line: 2,
            character: 12,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::TraitUse,
            "`use` inside class body should remain TraitUse"
        );
    }

    #[test]
    fn test_use_import_is_class_only() {
        assert!(
            ClassNameContext::UseImport.is_class_only(),
            "UseImport should be class-only (no constants or functions)"
        );
    }

    #[test]
    fn test_use_function_is_not_class_only() {
        assert!(
            !ClassNameContext::UseFunction.is_class_only(),
            "UseFunction should NOT be class-only (handler shows functions)"
        );
    }

    #[test]
    fn test_use_const_is_not_class_only() {
        assert!(
            !ClassNameContext::UseConst.is_class_only(),
            "UseConst should NOT be class-only (handler shows constants)"
        );
    }

    #[test]
    fn test_use_import_accepts_all_kinds() {
        assert!(ClassNameContext::UseImport.matches_kind_flags(ClassLikeKind::Class, false, false));
        assert!(ClassNameContext::UseImport.matches_kind_flags(
            ClassLikeKind::Interface,
            false,
            false
        ));
        assert!(ClassNameContext::UseImport.matches_kind_flags(ClassLikeKind::Trait, false, false));
        assert!(ClassNameContext::UseImport.matches_kind_flags(ClassLikeKind::Enum, false, false));
    }

    #[test]
    fn test_detect_use_function_with_fqn_partial() {
        let content = "<?php\nuse function App\\Helpers\\format";
        let pos = Position {
            line: 1,
            character: 35,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::UseFunction,
            "`use function` with namespace-qualified partial should produce UseFunction"
        );
    }

    #[test]
    fn test_detect_use_const_with_fqn_partial() {
        let content = "<?php\nuse const App\\Config\\DB";
        let pos = Position {
            line: 1,
            character: 26,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::UseConst,
            "`use const` with namespace-qualified partial should produce UseConst"
        );
    }

    // ── NamespaceDeclaration detection ──────────────────────────────

    #[test]
    fn test_detect_namespace_declaration_context() {
        let content = "<?php\nnamespace App";
        let pos = Position {
            line: 1,
            character: 13,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::NamespaceDeclaration,
            "Top-level `namespace` should produce NamespaceDeclaration"
        );
    }

    #[test]
    fn test_detect_namespace_declaration_with_partial_fqn() {
        let content = "<?php\nnamespace App\\Models";
        let pos = Position {
            line: 1,
            character: 22,
        };
        assert_eq!(
            detect_class_name_context(content, pos),
            ClassNameContext::NamespaceDeclaration,
            "`namespace App\\Models` should produce NamespaceDeclaration"
        );
    }

    #[test]
    fn test_namespace_inside_class_body_is_not_declaration() {
        let content = "<?php\nclass Foo {\n    public function bar() {\n        namespace\n";
        let pos = Position {
            line: 3,
            character: 17,
        };
        assert_ne!(
            detect_class_name_context(content, pos),
            ClassNameContext::NamespaceDeclaration,
            "`namespace` inside class body (brace depth >= 1) should not be NamespaceDeclaration"
        );
    }

    // ── class_completion_texts edge cases ───────────────────────────

    #[test]
    fn test_class_completion_texts_fqn_same_namespace_simplifies() {
        let ns = Some("Demo".to_string());
        let (label, insert, _filter, use_import) =
            class_completion_texts("Box", "Demo\\Box", true, true, &ns, "demo\\");
        assert_eq!(label, "Box", "Label should be the relative name");
        assert_eq!(insert, "Box", "Insert text should be the relative name");
        assert!(
            use_import.is_none(),
            "No use import needed for same namespace"
        );
    }

    #[test]
    fn test_class_completion_texts_fqn_different_namespace_keeps_fqn() {
        let ns = Some("Demo".to_string());
        let (label, insert, _filter, use_import) =
            class_completion_texts("Foo", "Other\\Foo", true, true, &ns, "other\\");
        assert_eq!(label, "Other\\Foo", "Label should be the full FQN");
        assert_eq!(
            insert, "\\Other\\Foo",
            "Insert should have leading backslash"
        );
        assert!(use_import.is_none(), "FQN mode never produces a use import");
    }

    #[test]
    fn test_class_completion_texts_non_fqn_always_short_name() {
        let ns: Option<String> = None;
        let (label, insert, _filter, use_import) = class_completion_texts(
            "Dechunk",
            "http\\Encoding\\Dechunk",
            false,
            false,
            &ns,
            "dec",
        );
        assert_eq!(
            label, "Dechunk",
            "Non-FQN mode should always use the short name"
        );
        assert_eq!(insert, "Dechunk");
        assert_eq!(
            use_import.as_deref(),
            Some("http\\Encoding\\Dechunk"),
            "Non-FQN mode should import the full FQN"
        );
    }

    #[test]
    fn test_class_completion_texts_fqn_nested_same_namespace() {
        let ns = Some("Demo".to_string());
        let (label, insert, _filter, use_import) =
            class_completion_texts("Thing", "Demo\\Sub\\Thing", true, true, &ns, "demo\\");
        assert_eq!(
            label, "Sub\\Thing",
            "Nested same-namespace class should use relative path"
        );
        assert_eq!(insert, "Sub\\Thing");
        assert!(use_import.is_none(), "No use import for same namespace");
    }

    #[test]
    fn test_class_completion_texts_leading_backslash_single_segment_same_ns() {
        // Typing `\Demo` (no trailing backslash) in namespace `Demo`.
        // `is_fqn = true` because `has_leading_backslash` is true.
        // `prefix_lower = "demo"` (the normalised, lower-cased prefix).
        let ns = Some("Demo".to_string());
        let (label, insert, _filter, use_import) =
            class_completion_texts("Box", "Demo\\Box", true, true, &ns, "demo");
        assert_eq!(
            label, "Box",
            "Same-namespace class should simplify to short name"
        );
        assert_eq!(
            insert, "Box",
            "Insert text should be 'Box', not '\\Box' or '\\Demo\\Box'"
        );
        assert!(
            use_import.is_none(),
            "No use import needed for same namespace"
        );
    }

    #[test]
    fn test_class_completion_texts_leading_backslash_single_segment_diff_ns() {
        // Typing `\Other` in namespace `Demo` — different namespace.
        let ns = Some("Demo".to_string());
        let (label, insert, _filter, use_import) =
            class_completion_texts("Foo", "Other\\Foo", true, true, &ns, "other");
        assert_eq!(label, "Other\\Foo", "Label should be the full FQN");
        assert_eq!(
            insert, "\\Other\\Foo",
            "Insert should have leading backslash for different namespace"
        );
        assert!(use_import.is_none(), "FQN mode never produces a use import");
    }
}
