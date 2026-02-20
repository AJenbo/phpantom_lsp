/// Member-access definition resolution.
///
/// This module handles go-to-definition for member references — methods,
/// properties, and constants accessed via `->`, `?->`, or `::` operators.
///
/// Supported patterns:
///   - `$this->method()`, `$this->property`
///   - `$var->method()`, `$var->property`
///   - `self::method()`, `self::CONST`, `self::$staticProp`
///   - `static::method()`, `parent::method()`
///   - `ClassName::method()`, `ClassName::CONST`, `ClassName::$staticProp`
///   - Chained access: `$this->prop->method()`, `app()->method()`
///
/// Resolution walks the class hierarchy (parent classes, traits, mixins)
/// to find the declaring class and locates the member position in its
/// source file.
use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::subject_extraction::{extract_arrow_subject, extract_double_colon_subject};
use crate::types::*;

/// The kind of class member being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemberKind {
    Method,
    Property,
    Constant,
}

/// Hint about whether the member access looks like a method call or a property
/// access.  Used to disambiguate when a class has both a method and a property
/// with the same name (e.g. `id()` method vs `$id` property).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberAccessHint {
    /// Followed by `(` — looks like a method call.
    MethodCall,
    /// No `(` after the name — looks like a property / constant access.
    PropertyAccess,
    /// Cannot determine (fallback to original order).
    Unknown,
}

impl Backend {
    // ─── Member Definition Resolution ───────────────────────────────────────

    /// Try to resolve a member access pattern and jump to the member's
    /// declaration.
    ///
    /// Detects `::`, `->`, and `?->` before the word under the cursor,
    /// resolves the owning class, and finds the member position in the
    /// class's source file.
    pub(super) fn resolve_member_definition(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        member_name: &str,
    ) -> Option<Location> {
        // 1. Detect the access operator and extract the subject (left side).
        let (subject, access_kind) = Self::extract_member_access_context(content, position)?;

        // 2. Gather context needed for class resolution.
        let cursor_offset = Self::position_to_offset(content, position)?;

        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let current_class = Self::find_class_at_offset(&classes, cursor_offset).cloned();

        let file_use_map: HashMap<String, String> = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let file_namespace: Option<String> = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        // Build a class_loader closure (same pattern as the completion handler).
        let class_loader = |name: &str| -> Option<ClassInfo> {
            self.resolve_class_name(name, &classes, &file_use_map, &file_namespace)
        };

        // Build a function_loader closure for resolving standalone function
        // return types (needed for call-expression subjects like `app()->`).
        let function_loader = |name: &str| -> Option<FunctionInfo> {
            self.resolve_function_name(name, &file_use_map, &file_namespace)
        };

        // 3. Resolve the subject to all candidate classes.
        //    When a variable is assigned different types in conditional
        //    branches (e.g. if/else), multiple candidates are returned.
        let candidates = Self::resolve_target_classes(
            &subject,
            access_kind,
            current_class.as_ref(),
            &classes,
            content,
            cursor_offset,
            &class_loader,
            Some(&function_loader),
        );

        if candidates.is_empty() {
            return None;
        }

        // Determine whether this looks like a method call or property access.
        let access_hint = Self::detect_member_access_hint(content, position, member_name);

        // 4. Try each candidate class and pick the first one where the
        //    member actually exists (directly or via inheritance).
        for target_class in &candidates {
            // Check if the member name is a trait `as` alias on this class.
            // If so, resolve to the original method name and (optionally) the
            // source trait so we jump to the actual method definition rather
            // than failing to find an alias that only exists after inheritance
            // resolution.
            let (effective_name, alias_trait) =
                Self::resolve_trait_alias(target_class, member_name);

            // If we know the exact source trait from the alias, go directly
            // to that trait's method definition.
            if let Some(ref trait_name) = alias_trait
                && let Some(trait_info) = class_loader(trait_name)
                && Self::classify_member(&trait_info, &effective_name, access_hint).is_some()
                && let Some((class_uri, class_content)) =
                    self.find_class_file_content(&trait_info.name, uri, content)
                && let Some(member_position) =
                    Self::find_member_position(&class_content, &effective_name, MemberKind::Method)
                && let Ok(parsed_uri) = Url::parse(&class_uri)
            {
                return Some(Location {
                    uri: parsed_uri,
                    range: Range {
                        start: member_position,
                        end: member_position,
                    },
                });
            }

            let declaring_class =
                Self::find_declaring_class(target_class, &effective_name, &class_loader)
                    .unwrap_or_else(|| target_class.clone());

            // Check that the member is actually present on the declaring class.
            let member_kind =
                match Self::classify_member(&declaring_class, &effective_name, access_hint) {
                    Some(k) => k,
                    None => continue, // member not on this candidate, try next
                };

            // Locate the file that contains the declaring class.
            if let Some((class_uri, class_content)) =
                self.find_class_file_content(&declaring_class.name, uri, content)
                && let Some(member_position) =
                    Self::find_member_position(&class_content, &effective_name, member_kind)
                && let Ok(parsed_uri) = Url::parse(&class_uri)
            {
                return Some(Location {
                    uri: parsed_uri,
                    range: Range {
                        start: member_position,
                        end: member_position,
                    },
                });
            }
        }

        // No candidate had the member — fall back to the first candidate
        // and try the original (non-iterating) logic so we at least get
        // partial results when possible.
        let target_class = &candidates[0];

        let (effective_name, alias_trait) = Self::resolve_trait_alias(target_class, member_name);

        // Direct trait lookup for aliased members in the fallback path.
        if let Some(ref trait_name) = alias_trait
            && let Some(trait_info) = class_loader(trait_name)
            && let Some((class_uri, class_content)) =
                self.find_class_file_content(&trait_info.name, uri, content)
            && let Some(member_position) =
                Self::find_member_position(&class_content, &effective_name, MemberKind::Method)
            && let Ok(parsed_uri) = Url::parse(&class_uri)
        {
            return Some(Location {
                uri: parsed_uri,
                range: Range {
                    start: member_position,
                    end: member_position,
                },
            });
        }

        let declaring_class =
            Self::find_declaring_class(target_class, &effective_name, &class_loader)
                .unwrap_or_else(|| target_class.clone());

        let member_kind = Self::classify_member(&declaring_class, &effective_name, access_hint)?;

        let (class_uri, class_content) =
            self.find_class_file_content(&declaring_class.name, uri, content)?;

        let member_position =
            Self::find_member_position(&class_content, &effective_name, member_kind)?;

        let parsed_uri = Url::parse(&class_uri).ok()?;
        Some(Location {
            uri: parsed_uri,
            range: Range {
                start: member_position,
                end: member_position,
            },
        })
    }

    // ─── Member Access Context Extraction ───────────────────────────────────

    /// Check whether the cursor is on the right-hand side of a member
    /// access operator (`->`, `?->`, or `::`).
    ///
    /// Returns `true` when the word under the cursor is preceded by one
    /// of these operators — meaning the word is a member name, NOT a
    /// standalone function / class / constant.  This is used by
    /// [`resolve_definition`](super::resolve) to prevent falling through
    /// to standalone symbol resolution when member resolution fails
    /// (e.g. because the owning class couldn't be determined).
    pub(crate) fn is_member_access_context(content: &str, position: Position) -> bool {
        Self::extract_member_access_context(content, position).is_some()
    }

    /// Detect the access operator (`::`, `->`, `?->`) immediately before the
    /// word under the cursor and extract the subject to its left.
    ///
    /// Returns `(subject, AccessKind)` or `None` if no operator is found.
    ///
    /// This works by:
    ///   1. Finding the start of the identifier under the cursor.
    ///   2. Skipping a `$` prefix if present (for `::$staticProp`).
    ///   3. Checking for `::`, `->`, or `?->` immediately before.
    ///   4. Extracting the subject expression to the left of the operator.
    pub(crate) fn extract_member_access_context(
        content: &str,
        position: Position,
    ) -> Option<(String, AccessKind)> {
        let lines: Vec<&str> = content.lines().collect();
        let line = lines.get(position.line as usize)?;
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        if chars.is_empty() {
            return None;
        }

        // Find the start of the identifier under the cursor.
        let mut i = col;

        // If the cursor is on or past the end of a word, adjust.
        if i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
            // on a word char — walk left
        } else if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        } else {
            return None;
        }

        // Walk left past identifier characters.
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        let mut operator_end = i;

        // Skip `$` prefix (for `Class::$staticProp`).
        if operator_end > 0 && chars[operator_end - 1] == '$' {
            operator_end -= 1;
        }

        // Detect `::`.
        if operator_end >= 2 && chars[operator_end - 2] == ':' && chars[operator_end - 1] == ':' {
            let subject = extract_double_colon_subject(&chars, operator_end - 2);
            if !subject.is_empty() {
                return Some((subject, AccessKind::DoubleColon));
            }
        }

        // Detect `->`.
        if operator_end >= 2 && chars[operator_end - 2] == '-' && chars[operator_end - 1] == '>' {
            let subject = extract_arrow_subject(&chars, operator_end - 2);
            if !subject.is_empty() {
                return Some((subject, AccessKind::Arrow));
            }
        }

        // Detect `?->` (null-safe operator).
        if operator_end >= 3
            && chars[operator_end - 3] == '?'
            && chars[operator_end - 2] == '-'
            && chars[operator_end - 1] == '>'
        {
            let subject = extract_arrow_subject(&chars, operator_end - 3);
            if !subject.is_empty() {
                return Some((subject, AccessKind::Arrow));
            }
        }

        None
    }

    // ─── Member Classification ──────────────────────────────────────────────

    /// Determine the kind of member (method, property, or constant) by
    /// checking the class's parsed information.
    ///
    /// Returns `None` if the member is not found in the class.
    fn classify_member(
        class: &ClassInfo,
        member_name: &str,
        hint: MemberAccessHint,
    ) -> Option<MemberKind> {
        let has_method = class.methods.iter().any(|m| m.name == member_name);
        let has_property = class.properties.iter().any(|p| p.name == member_name);
        let has_constant = class.constants.iter().any(|c| c.name == member_name);

        match hint {
            MemberAccessHint::PropertyAccess => {
                // Prefer property/constant over method when there's no `()`.
                if has_property {
                    return Some(MemberKind::Property);
                }
                if has_constant {
                    return Some(MemberKind::Constant);
                }
                if has_method {
                    return Some(MemberKind::Method);
                }
            }
            MemberAccessHint::MethodCall => {
                // Prefer method when followed by `()`.
                if has_method {
                    return Some(MemberKind::Method);
                }
                if has_property {
                    return Some(MemberKind::Property);
                }
                if has_constant {
                    return Some(MemberKind::Constant);
                }
            }
            MemberAccessHint::Unknown => {
                // Default order: method, property, constant.
                if has_method {
                    return Some(MemberKind::Method);
                }
                if has_property {
                    return Some(MemberKind::Property);
                }
                if has_constant {
                    return Some(MemberKind::Constant);
                }
            }
        }
        None
    }

    /// Determine whether the member name at the given position is followed by
    /// `(` (indicating a method call) or not (indicating property / constant
    /// access).
    fn detect_member_access_hint(
        content: &str,
        position: Position,
        member_name: &str,
    ) -> MemberAccessHint {
        let lines: Vec<&str> = content.lines().collect();
        let line = match lines.get(position.line as usize) {
            Some(l) => *l,
            None => return MemberAccessHint::Unknown,
        };
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Find the end of the member name by walking right from the cursor.
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

        let mut end = col;
        // If cursor is on a word char, walk right to end of word.
        if end < chars.len() && is_word_char(chars[end]) {
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }
        } else if end > 0 && is_word_char(chars[end - 1]) {
            // Cursor is just past the word; `end` is already correct.
        } else {
            // Try to find the member name by searching forward from col.
            if let Some(idx) = line[col..].find(member_name) {
                end = col + idx + member_name.len();
            } else {
                return MemberAccessHint::Unknown;
            }
        }

        // Skip whitespace after the word.
        let mut i = end;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        if i < chars.len() && chars[i] == '(' {
            MemberAccessHint::MethodCall
        } else {
            MemberAccessHint::PropertyAccess
        }
    }

    // ─── Inheritance Chain Walking ──────────────────────────────────────────

    /// Walk up the inheritance chain to find the class that actually declares
    /// the given member.
    ///
    /// Returns `Some(ClassInfo)` of the declaring class, or `None` if the
    /// member cannot be found in any ancestor.
    /// Resolve a trait `as` alias on a class.
    ///
    /// If `member_name` matches a trait alias declared on the class, returns
    /// the original method name and (optionally) the source trait name.
    /// Otherwise returns `member_name` unchanged with no trait hint.
    fn resolve_trait_alias(class: &ClassInfo, member_name: &str) -> (String, Option<String>) {
        for alias in &class.trait_aliases {
            if alias.alias.as_deref() == Some(member_name) {
                return (alias.method_name.clone(), alias.trait_name.clone());
            }
        }
        (member_name.to_string(), None)
    }

    fn find_declaring_class(
        class: &ClassInfo,
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<ClassInfo> {
        const MAX_DEPTH: usize = 20;

        // Check if this class directly declares the member.
        if Self::classify_member(class, member_name, MemberAccessHint::Unknown).is_some() {
            return Some(class.clone());
        }

        // Check traits used by this class.
        if let Some(found) =
            Self::find_declaring_in_traits(&class.used_traits, member_name, class_loader, 0)
        {
            return Some(found);
        }

        // Walk up the parent chain.
        let mut current = class.clone();
        for _ in 0..MAX_DEPTH {
            let parent_name = match current.parent_class.as_ref() {
                Some(name) => name.clone(),
                None => break,
            };
            let parent = match class_loader(&parent_name) {
                Some(p) => p,
                None => break,
            };
            if Self::classify_member(&parent, member_name, MemberAccessHint::Unknown).is_some() {
                return Some(parent);
            }
            // Check traits used by the parent class.
            if let Some(found) =
                Self::find_declaring_in_traits(&parent.used_traits, member_name, class_loader, 0)
            {
                return Some(found);
            }
            current = parent;
        }

        // Check @mixin classes — these have the lowest precedence.
        if let Some(found) =
            Self::find_declaring_in_mixins(&class.mixins, member_name, class_loader, 0)
        {
            return Some(found);
        }

        // Also check @mixin classes declared on ancestor classes.
        // e.g. `User extends Model` where `Model` has `@mixin Builder`.
        let mut ancestor = class.clone();
        for _ in 0..MAX_DEPTH {
            let parent_name = match ancestor.parent_class.as_ref() {
                Some(name) => name.clone(),
                None => break,
            };
            let parent = match class_loader(&parent_name) {
                Some(p) => p,
                None => break,
            };
            if !parent.mixins.is_empty()
                && let Some(found) =
                    Self::find_declaring_in_mixins(&parent.mixins, member_name, class_loader, 0)
            {
                return Some(found);
            }
            ancestor = parent;
        }

        None
    }

    /// Search through a list of trait names for one that declares `member_name`.
    ///
    /// Traits can themselves `use` other traits, so this recurses up to a
    /// depth limit to handle trait composition.
    fn find_declaring_in_traits(
        trait_names: &[String],
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: usize,
    ) -> Option<ClassInfo> {
        const MAX_TRAIT_DEPTH: usize = 20;
        if depth > MAX_TRAIT_DEPTH {
            return None;
        }

        for trait_name in trait_names {
            let trait_info = if let Some(t) = class_loader(trait_name) {
                t
            } else {
                continue;
            };
            if Self::classify_member(&trait_info, member_name, MemberAccessHint::Unknown).is_some()
            {
                return Some(trait_info);
            }
            // Recurse into traits used by this trait.
            if let Some(found) = Self::find_declaring_in_traits(
                &trait_info.used_traits,
                member_name,
                class_loader,
                depth + 1,
            ) {
                return Some(found);
            }
            // Walk the parent_class (extends) chain so that interface
            // inheritance is resolved.  For example, BackedEnum extends
            // UnitEnum — looking up `cases` on BackedEnum should find
            // the declaring UnitEnum interface.
            let mut current = trait_info;
            let mut parent_depth = depth;
            while let Some(ref parent_name) = current.parent_class {
                parent_depth += 1;
                if parent_depth > MAX_TRAIT_DEPTH {
                    break;
                }
                let parent = if let Some(p) = class_loader(parent_name) {
                    p
                } else {
                    break;
                };
                if Self::classify_member(&parent, member_name, MemberAccessHint::Unknown).is_some()
                {
                    return Some(parent);
                }
                if let Some(found) = Self::find_declaring_in_traits(
                    &parent.used_traits,
                    member_name,
                    class_loader,
                    parent_depth + 1,
                ) {
                    return Some(found);
                }
                current = parent;
            }
        }

        None
    }

    /// Search through `@mixin` class names for one that declares `member_name`.
    ///
    /// Mixin classes are resolved with their full inheritance chain (parent
    /// classes, traits) so that inherited members are found.  Only public
    /// members are considered since mixins proxy via magic methods.
    /// Mixin classes can themselves declare `@mixin`, so this recurses up
    /// to a depth limit.
    fn find_declaring_in_mixins(
        mixin_names: &[String],
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: usize,
    ) -> Option<ClassInfo> {
        const MAX_MIXIN_DEPTH: usize = 10;
        if depth > MAX_MIXIN_DEPTH {
            return None;
        }

        for mixin_name in mixin_names {
            let mixin_class = if let Some(c) = class_loader(mixin_name) {
                c
            } else {
                continue;
            };

            // Try to find the declaring class within the mixin's own
            // hierarchy (itself, its traits, its parents).
            if let Some(found) = Self::find_declaring_class(&mixin_class, member_name, class_loader)
            {
                return Some(found);
            }

            // Recurse into mixins declared by this mixin class.
            if !mixin_class.mixins.is_empty()
                && let Some(found) = Self::find_declaring_in_mixins(
                    &mixin_class.mixins,
                    member_name,
                    class_loader,
                    depth + 1,
                )
            {
                return Some(found);
            }
        }

        None
    }

    // ─── File & Position Lookup ─────────────────────────────────────────────

    /// Find the file URI and content for the file that contains a given class.
    ///
    /// Searches the `ast_map` (which includes files loaded via PSR-4 by
    /// `find_or_load_class`) and returns `(uri, content)`.
    pub(crate) fn find_class_file_content(
        &self,
        class_name: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<(String, String)> {
        // Search the ast_map for the file containing this class.
        let uri = {
            let map = self.ast_map.lock().ok()?;

            // Check the current file first (common case: $this->method).
            if let Some(classes) = map.get(current_uri) {
                if classes.iter().any(|c| c.name == class_name) {
                    Some(current_uri.to_string())
                } else {
                    // Search other files.
                    map.iter()
                        .find(|(_, classes)| classes.iter().any(|c| c.name == class_name))
                        .map(|(u, _)| u.clone())
                }
            } else {
                map.iter()
                    .find(|(_, classes)| classes.iter().any(|c| c.name == class_name))
                    .map(|(u, _)| u.clone())
            }
        }?;

        // Get the file content.
        let file_content = if uri == current_uri {
            current_content.to_string()
        } else if uri.starts_with("phpantom-stub://") {
            // Embedded stubs are stored under synthetic URIs and have no
            // on-disk file.  Retrieve the raw stub source from the
            // stub_index instead.
            self.stub_index.get(class_name).map(|s| s.to_string())?
        } else {
            // Try open files first, then read from disk.
            let from_open = self
                .open_files
                .lock()
                .ok()
                .and_then(|files| files.get(&uri).cloned());

            if let Some(c) = from_open {
                c
            } else {
                // Parse the URI to a file path and read from disk.
                let path = Url::parse(&uri).ok()?.to_file_path().ok()?;
                std::fs::read_to_string(path).ok()?
            }
        };

        Some((uri, file_content))
    }

    /// Find the position of a member declaration (method, property, or constant)
    /// inside a PHP file.
    ///
    /// Searches line by line for the declaration pattern corresponding to the
    /// member kind, with word-boundary checks to avoid partial matches.
    pub(crate) fn find_member_position(
        content: &str,
        member_name: &str,
        kind: MemberKind,
    ) -> Option<Position> {
        let is_word_boundary = |c: u8| {
            let ch = c as char;
            !ch.is_alphanumeric() && ch != '_'
        };

        for (line_idx, line) in content.lines().enumerate() {
            match kind {
                MemberKind::Method => {
                    // Look for `function methodName` with word boundaries.
                    let pattern = format!("function {}", member_name);
                    if let Some(col) = line.find(&pattern) {
                        let after_pos = col + pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                        if after_ok {
                            return Some(Position {
                                line: line_idx as u32,
                                character: col as u32,
                            });
                        }
                    }
                }
                MemberKind::Constant => {
                    // Look for `const CONSTANT_NAME` with word boundaries.
                    let pattern = format!("const {}", member_name);
                    if let Some(col) = line.find(&pattern) {
                        // Make sure `const` is preceded by a word boundary
                        // (not part of another identifier).
                        let before_ok = col == 0 || is_word_boundary(line.as_bytes()[col - 1]);
                        let after_pos = col + pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                        if before_ok && after_ok {
                            return Some(Position {
                                line: line_idx as u32,
                                character: col as u32,
                            });
                        }
                    }
                    // Also look for `case CASE_NAME` (enum cases are stored
                    // as constants but declared with `case` keyword).
                    let case_pattern = format!("case {}", member_name);
                    if let Some(col) = line.find(&case_pattern) {
                        let before_ok = col == 0 || is_word_boundary(line.as_bytes()[col - 1]);
                        let after_pos = col + case_pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                        if before_ok && after_ok {
                            return Some(Position {
                                line: line_idx as u32,
                                character: col as u32,
                            });
                        }
                    }
                }
                MemberKind::Property => {
                    // Look for `$propertyName` on a line that looks like a
                    // property declaration (has a visibility keyword, `var`,
                    // or `readonly`).
                    let var_pattern = format!("${}", member_name);
                    if let Some(col) = line.find(&var_pattern) {
                        let after_pos = col + var_pattern.len();
                        let after_ok =
                            after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);

                        if after_ok {
                            let trimmed = line.trim_start();
                            // A line starting with a visibility keyword is a
                            // property declaration UNLESS it also contains
                            // `function` before the `$` — in that case it is
                            // a method whose parameter happens to share the
                            // property name (e.g.
                            // `public static function from(int|string $value)`
                            // vs `public readonly int|string $value;`).
                            let prefix = &line[..col];
                            let is_method_param = prefix.contains("function");
                            let is_declaration = !is_method_param
                                && (trimmed.starts_with("public")
                                    || trimmed.starts_with("protected")
                                    || trimmed.starts_with("private")
                                    || trimmed.starts_with("var ")
                                    || trimmed.starts_with("readonly")
                                    || trimmed.starts_with("static"));

                            // Also detect promoted constructor properties:
                            // `public function __construct(private Type $prop)`
                            // In this case the visibility keyword appears
                            // inside the parameter list on the same line.
                            // Only applies to `__construct` — regular method
                            // parameters like `function from(int|string $value)`
                            // must not be mistaken for property declarations.
                            let is_promoted = !is_declaration
                                && !is_method_param
                                && prefix.contains("__construct")
                                && {
                                    // Check if visibility keyword appears before
                                    // the `$prop` on the same line (inside parens).
                                    prefix.contains("public")
                                        || prefix.contains("protected")
                                        || prefix.contains("private")
                                        || prefix.contains("readonly")
                                };

                            if is_declaration || is_promoted {
                                // Place the cursor on the first letter after
                                // `$` so that a second go-to-definition
                                // triggers type-hint resolution.
                                return Some(Position {
                                    line: line_idx as u32,
                                    character: (col + 1) as u32,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Fallback: for properties, check if this is a magic property
        // declared via a `@property` tag in the class docblock.
        // Lines look like: ` * @property Type $propertyName`
        if kind == MemberKind::Property {
            let var_pattern = format!("${}", member_name);
            for (line_idx, line) in content.lines().enumerate() {
                if let Some(col) = line.find(&var_pattern) {
                    let after_pos = col + var_pattern.len();
                    let after_ok =
                        after_pos >= line.len() || is_word_boundary(line.as_bytes()[after_pos]);
                    if !after_ok {
                        continue;
                    }

                    let trimmed = line.trim().trim_start_matches('*').trim();
                    if trimmed.starts_with("@property-read")
                        || trimmed.starts_with("@property-write")
                        || trimmed.starts_with("@property")
                    {
                        return Some(Position {
                            line: line_idx as u32,
                            character: (col + 1) as u32,
                        });
                    }
                }
            }
        }

        // Fallback: for methods, check if this is a magic method
        // declared via a `@method` tag in the class docblock.
        // Lines look like: ` * @method ReturnType methodName(params...)`
        if kind == MemberKind::Method {
            // The method name is followed by `(` in a @method tag.
            let method_pattern = member_name;
            for (line_idx, line) in content.lines().enumerate() {
                // Search for ALL occurrences of the pattern within the line,
                // not just the first one.  This is important when the method
                // name collides with a type keyword (e.g. `string`) that also
                // appears as the return type on the same line.
                let mut search_start = 0;
                while let Some(offset) = line[search_start..].find(method_pattern) {
                    let col = search_start + offset;
                    search_start = col + method_pattern.len();

                    // Verify the character after the name is `(` (method call syntax).
                    let after_pos = col + method_pattern.len();
                    if after_pos >= line.len() {
                        continue;
                    }
                    let after_char = line.as_bytes()[after_pos];
                    if after_char != b'(' {
                        continue;
                    }

                    // Verify the character before is a word boundary (whitespace)
                    // to avoid matching partial names.
                    if col > 0 && !is_word_boundary(line.as_bytes()[col - 1]) {
                        continue;
                    }

                    let trimmed = line.trim().trim_start_matches('*').trim();
                    if trimmed.starts_with("@method") {
                        return Some(Position {
                            line: line_idx as u32,
                            character: col as u32,
                        });
                    }
                }
            }
        }

        None
    }
}
