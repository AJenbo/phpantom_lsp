/// Class name completions.
///
/// This module handles building completion items for class, interface,
/// trait, and enum names when no member-access operator (`->` or `::`)
/// is present.
///
/// Also provides a Throwable-filtered variant for catch clause fallback
/// and `throw new` completion, which only suggests exception classes
/// from already-parsed sources and includes everything else (classmap,
/// stubs) unfiltered.
///
/// Constant, function, and namespace completions live in their own
/// sibling modules (`constant_completion`, `function_completion`,
/// `namespace_completion`).
use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::named_args::position_to_char_offset;
use crate::completion::use_edit;
use crate::types::*;
use crate::util::short_name;

use crate::completion::builder::{
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
pub(in crate::completion) fn is_anonymous_class(name: &str) -> bool {
    name.starts_with("__anonymous@")
}

/// Check whether a class matches the typed prefix.
///
/// In FQN-prefix mode (`is_fqn` is `true`) both the short name and the
/// fully-qualified name are checked so that `App\Models\U` can surface
/// `App\Models\User`.  In non-FQN mode only the short name is checked
/// to avoid flooding the response with every class under a broad
/// namespace prefix.
pub(in crate::completion) fn matches_class_prefix(
    short_name: &str,
    fqn: &str,
    prefix_lower: &str,
    is_fqn: bool,
) -> bool {
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
pub(in crate::completion) fn class_completion_texts(
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

/// Shared context for building class completion items.
///
/// Bundles the parameters that are constant across all class sources
/// within a single completion request, so that `apply_import_fixups`
/// and `build_item` don't need seven-plus arguments each.
pub(in crate::completion) struct ClassItemCtx<'a> {
    pub(in crate::completion) is_fqn_prefix: bool,
    pub(in crate::completion) is_new: bool,
    pub(in crate::completion) fqn_replace_range: Option<Range>,
    pub(in crate::completion) file_use_map: &'a HashMap<String, String>,
    pub(in crate::completion) use_block: use_edit::UseBlockInfo,
    pub(in crate::completion) file_namespace: &'a Option<String>,
}

/// Per-item text fields produced by `class_completion_texts` and
/// post-processed by `apply_import_fixups`.
pub(in crate::completion) struct ClassItemTexts {
    pub(in crate::completion) label: String,
    pub(in crate::completion) base_name: String,
    pub(in crate::completion) filter: String,
    pub(in crate::completion) use_import: Option<String>,
}

impl ClassItemCtx<'_> {
    /// Fix up `base_name` and `use_import` after `class_completion_texts`
    /// to handle import conflicts and FQN alias collisions.
    ///
    /// This logic was repeated across every class source (class_index,
    /// classmap, stubs) in both `build_class_name_completions` and
    /// `build_catch_class_name_completions`.
    ///
    /// - If the short name conflicts with an existing import, falls back
    ///   to a fully-qualified reference (prepends `\`).
    /// - In FQN mode, if the first namespace segment matches an existing
    ///   alias (and the name wasn't intentionally shortened through that
    ///   alias), prepends `\` so PHP resolves from the global namespace.
    pub(in crate::completion) fn apply_import_fixups(
        &self,
        base_name: &mut String,
        use_import: &mut Option<String>,
        was_shortened: bool,
    ) {
        if let Some(ref import_fqn) = *use_import
            && use_import_conflicts(import_fqn, self.file_use_map)
        {
            *base_name = format!("\\{}", import_fqn);
            *use_import = None;
        }
        if self.is_fqn_prefix
            && !was_shortened
            && !base_name.starts_with('\\')
            && let Some(first_seg) = base_name.split('\\').next()
            && self
                .file_use_map
                .keys()
                .any(|a| a.eq_ignore_ascii_case(first_seg))
        {
            *base_name = format!("\\{}", base_name);
        }
    }

    /// Build a `CompletionItem` for a class name completion.
    ///
    /// This is the shared construction logic used by both
    /// `build_class_name_completions` and
    /// `build_catch_class_name_completions` for class_index, classmap,
    /// and stub sources.
    pub(in crate::completion) fn build_item(
        &self,
        texts: ClassItemTexts,
        fqn: &str,
        sort_text: String,
        new_insert_fn: impl FnOnce(&str) -> (String, Option<InsertTextFormat>),
        is_deprecated: bool,
    ) -> CompletionItem {
        let (insert_text, insert_text_format) = if self.is_new {
            new_insert_fn(&texts.base_name)
        } else {
            (texts.base_name, None)
        };
        CompletionItem {
            label: texts.label,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(fqn.to_string()),
            insert_text: Some(insert_text.clone()),
            insert_text_format,
            filter_text: Some(texts.filter),
            sort_text: Some(sort_text),
            deprecated: if is_deprecated { Some(true) } else { None },
            text_edit: self.fqn_replace_range.map(|range| {
                CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: insert_text.clone(),
                })
            }),
            additional_text_edits: texts.use_import.as_ref().and_then(|import_fqn| {
                build_use_edit(import_fqn, &self.use_block, self.file_namespace)
            }),
            ..CompletionItem::default()
        }
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
    pub(in crate::completion) fn build_new_insert(
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
    pub(in crate::completion) const MAX_CLASS_COMPLETIONS: usize = 100;

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
        let ctx = ClassItemCtx {
            is_fqn_prefix,
            is_new,
            fqn_replace_range,
            file_use_map,
            use_block: analyze_use_block(content),
            file_namespace: effective_namespace,
        };

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
        if let Some(ns) = file_namespace {
            let nmap = self.namespace_map.read();
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

            {
                let amap = self.ast_map.read();
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
                                deprecated: if cls.deprecation_message.is_some() {
                                    Some(true)
                                } else {
                                    None
                                },
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
        {
            let idx = self.class_index.read();
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
                let mut texts = ClassItemTexts {
                    label,
                    base_name,
                    filter,
                    use_import,
                };
                ctx.apply_import_fixups(&mut texts.base_name, &mut texts.use_import, was_shortened);
                // Demote names that heuristically mismatch the context
                // so better-looking candidates appear first.
                let sort_prefix = if context.likely_mismatch(sn) {
                    "7"
                } else {
                    "2"
                };
                items.push(ctx.build_item(
                    texts,
                    fqn,
                    format!("{}_{}", sort_prefix, sn.to_lowercase()),
                    |name| (format!("{name}()$0"), Some(InsertTextFormat::SNIPPET)),
                    false,
                ));
            }
        }

        // ── 4. Composer classmap (all autoloaded classes) ───────────
        {
            let cmap = self.classmap.read();
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
                let mut texts = ClassItemTexts {
                    label,
                    base_name,
                    filter,
                    use_import,
                };
                ctx.apply_import_fixups(&mut texts.base_name, &mut texts.use_import, was_shortened);
                // Demote names that heuristically mismatch the context
                // so better-looking candidates appear first.
                let sort_prefix = if context.likely_mismatch(sn) {
                    "8"
                } else {
                    "3"
                };
                items.push(ctx.build_item(
                    texts,
                    fqn,
                    format!("{}_{}", sort_prefix, sn.to_lowercase()),
                    |name| Self::build_new_insert(name, None),
                    false,
                ));
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
            let mut texts = ClassItemTexts {
                label,
                base_name,
                filter,
                use_import,
            };
            ctx.apply_import_fixups(&mut texts.base_name, &mut texts.use_import, was_shortened);
            // Demote names that heuristically mismatch the context
            // so better-looking candidates appear first.
            let sort_prefix = if context.likely_mismatch(sn) {
                "9"
            } else {
                "4"
            };
            items.push(ctx.build_item(
                texts,
                name,
                format!("{}_{}", sort_prefix, sn.to_lowercase()),
                |name| (format!("{name}()$0"), Some(InsertTextFormat::SNIPPET)),
                false,
            ));
        }

        // ── Namespace segment items (FQN mode only) ─────────────────
        // When the user is typing a namespace-qualified reference (e.g.
        // `App\`, `\Illuminate\Database\`), inject the distinct
        // next-level namespace segments as MODULE-kind items so the
        // user can drill into the namespace tree incrementally instead
        // of being overwhelmed by hundreds of deeply-nested classes.
        if is_fqn_prefix {
            // Everything up to and including the last `\` in the
            // normalized (no leading `\`) prefix.  For `App\Models\U`
            // this is `App\Models\`; for `App\` it is `App\`.
            let ns_prefix_end = normalized.rfind('\\').map(|p| p + 1).unwrap_or(0);

            // Only inject segments when the prefix actually contains a
            // backslash.  A bare name like `User` in UseImport context
            // has `is_fqn_prefix` true but no namespace to browse.
            if ns_prefix_end > 0 {
                let ns_prefix_lower = normalized[..ns_prefix_end].to_lowercase();
                // Partial text after the last `\` that the user is
                // still typing (e.g. `U` from `App\Models\U`).  Used
                // to filter segments whose short name doesn't match.
                let after_ns_lower = normalized[ns_prefix_end..].to_lowercase();

                let mut seen_segments: HashSet<String> = HashSet::new();

                for fqn in &seen_fqns {
                    let fqn_lower = fqn.to_lowercase();
                    if !fqn_lower.starts_with(&ns_prefix_lower) {
                        continue;
                    }
                    // Portion of the FQN after the namespace prefix.
                    // PHP namespaces are ASCII so byte offsets match.
                    let rest = &fqn[ns_prefix_end..];
                    if let Some(next_bs) = rest.find('\\') {
                        let segment_short = &rest[..next_bs];
                        // Filter: the segment's short name must start
                        // with whatever the user typed after the last `\`.
                        if !after_ns_lower.is_empty()
                            && !segment_short.to_lowercase().starts_with(&after_ns_lower)
                        {
                            continue;
                        }
                        let segment = fqn[..ns_prefix_end + next_bs].to_string();
                        seen_segments.insert(segment);
                    }
                }

                for segment in &seen_segments {
                    let short = segment.rsplit('\\').next().unwrap_or(segment);

                    // Compute insert text and label the same way
                    // class_completion_texts does for FQN mode.
                    let (label, insert_ns) = if let Some(ns) = effective_namespace {
                        let ns_with_slash = format!("{}\\", ns);
                        if let Some(relative) = segment.strip_prefix(&ns_with_slash) {
                            (relative.to_string(), relative.to_string())
                        } else if has_leading_backslash {
                            (segment.clone(), format!("\\{}", segment))
                        } else {
                            (segment.clone(), segment.clone())
                        }
                    } else if has_leading_backslash {
                        (segment.clone(), format!("\\{}", segment))
                    } else {
                        (segment.clone(), segment.clone())
                    };

                    let filter = if has_leading_backslash {
                        format!("\\{}", segment)
                    } else {
                        segment.clone()
                    };

                    items.push(CompletionItem {
                        label,
                        kind: Some(CompletionItemKind::MODULE),
                        detail: Some(format!("namespace {}", segment)),
                        insert_text: Some(insert_ns.clone()),
                        filter_text: Some(filter),
                        sort_text: Some(format!("0!_{}", short.to_lowercase())),
                        text_edit: fqn_replace_range.map(|range| {
                            CompletionTextEdit::Edit(TextEdit {
                                range,
                                new_text: insert_ns,
                            })
                        }),
                        ..CompletionItem::default()
                    });
                }
            }
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
        if self.class_index.read().contains_key(fqn) {
            return false;
        }
        if self.classmap.read().contains_key(fqn) {
            return false;
        }
        if self.stub_index.contains_key(fqn) {
            return false;
        }

        // Not a known class. Check for positive namespace evidence.

        // 1. Some open file declares this FQN as its namespace.
        {
            let nmap = self.namespace_map.read();
            for ns in nmap.values().flatten() {
                if ns == fqn {
                    return true;
                }
            }
        }

        // 2. Known classes exist under this FQN as a namespace prefix.
        let prefix = format!("{}\\", fqn);
        if self
            .class_index
            .read()
            .keys()
            .any(|k| k.starts_with(&prefix))
        {
            return true;
        }
        if self.classmap.read().keys().any(|k| k.starts_with(&prefix)) {
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
        if self.class_index.read().contains_key(class_name) {
            return true;
        }
        if self.classmap.read().contains_key(class_name) {
            return true;
        }
        false
    }
}

#[cfg(test)]
#[path = "class_completion_tests.rs"]
mod tests;
