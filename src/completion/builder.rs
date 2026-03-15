use std::collections::HashMap;
/// Member completion item building.
///
/// This module contains the logic for constructing LSP `CompletionItem`s from
/// resolved `ClassInfo`, filtered by the `AccessKind` (arrow, double-colon,
/// or parent double-colon).
///
/// The union-merge pipeline ([`build_union_completion_items`] and
/// [`merge_union_completion_items`]) handles the case where a variable has
/// multiple candidate types (e.g. `User|AdminUser`).  It deduplicates
/// completion items across candidates, partitions them into intersection
/// members (present on all types) and branch-only members, and assigns
/// sort tiers so intersection members appear first.
///
/// Use-statement insertion helpers live in the sibling [`super::use_edit`]
/// module and are re-exported here for backward compatibility.
use std::sync::Arc;

use tower_lsp::lsp_types::*;

use crate::types::Visibility;
use crate::types::*;

/// Return a user-friendly class name for display in completion item details.
///
/// Anonymous classes have synthetic names like `__anonymous@156` which are
/// meaningless to the user. This replaces them with `"anonymous class"`.
fn display_class_name(name: &str) -> &str {
    if name.starts_with("__anonymous@") {
        "anonymous class"
    } else {
        name
    }
}

/// Build an LSP snippet string for a callable (function, method, or constructor).
///
/// Required parameters are included as numbered tab stops with their
/// PHP variable name as placeholder text.  Optional and variadic
/// parameters are omitted — they can be filled in via signature help.
///
/// The returned string uses LSP snippet syntax and **must** be paired
/// with `InsertTextFormat::SNIPPET` on the `CompletionItem`.
///
/// # Examples
///
/// | call                                       | result                              |
/// |--------------------------------------------|-------------------------------------|
/// | `("reset", &[])`                           | `"reset()$0"`                       |
/// | `("makeText", &[req($text), opt($long)])`  | `"makeText(${1:\\$text})$0"`        |
/// | `("add", &[req($a), req($b)])`             | `"add(${1:\\$a}, ${2:\\$b})$0"`     |
pub(crate) fn build_callable_snippet(name: &str, params: &[ParameterInfo]) -> String {
    let required: Vec<&ParameterInfo> = params.iter().filter(|p| p.is_required).collect();

    if required.is_empty() {
        format!("{name}()$0")
    } else {
        let placeholders: Vec<String> = required
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // Escape `$` in parameter names so it is treated as a
                // literal character rather than a snippet tab-stop /
                // variable reference.
                let escaped_name = p.name.replace('$', "\\$");
                format!("${{{}:{}}}", i + 1, escaped_name)
            })
            .collect();
        format!("{name}({})$0", placeholders.join(", "))
    }
}

// Re-export use-statement helpers so existing `use crate::completion::builder::{…}`
// imports continue to work.
pub(crate) use super::use_edit::{analyze_use_block, build_use_edit, use_import_conflicts};

/// PHP magic methods that should not appear in completion results.
/// These are invoked implicitly by the language runtime rather than
/// called directly by user code.
const MAGIC_METHODS: &[&str] = &[
    "__construct",
    "__destruct",
    "__clone",
    "__get",
    "__set",
    "__isset",
    "__unset",
    "__call",
    "__callStatic",
    "__invoke",
    "__toString",
    "__sleep",
    "__wakeup",
    "__serialize",
    "__unserialize",
    "__set_state",
    "__debugInfo",
];

/// Check whether a method name is a PHP magic method that should be
/// excluded from completion results.
fn is_magic_method(name: &str) -> bool {
    MAGIC_METHODS.iter().any(|&m| m.eq_ignore_ascii_case(name))
}

/// Build the label showing the full method signature.
///
/// Example: `regularCode(string $text, $frogs = false): string`
pub(crate) fn build_method_label(method: &MethodInfo) -> String {
    let params: Vec<String> = method
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

    let ret = method
        .return_type
        .as_ref()
        .map(|r| format!(": {}", r))
        .unwrap_or_default();

    format!("{}({}){}", method.name, params.join(", "), ret)
}

/// Build completion items for a resolved class, filtered by access kind
/// and visibility scope.
///
/// - `Arrow` access: returns only non-static methods and properties.
/// - `DoubleColon` access: returns only static methods, static properties, and constants.
/// - `ParentDoubleColon` access: returns both static and non-static methods,
///   static properties, and constants — but excludes private members.
/// - `Other` access: returns all members.
///
/// Visibility filtering based on `current_class_name` and `is_self_or_ancestor`:
/// - `None` (top-level code): only **public** members are shown.
/// - `Some(name)` where `name == target_class.name`: all members are shown
///   (same-class access, e.g. `$this->`).
/// - `is_self_or_ancestor == true`: **public** and **protected** members
///   are shown (the cursor is inside the target class or a subclass).
/// - Otherwise: only **public** members are shown.
///
/// `is_self_or_ancestor` should be `true` when the cursor is inside the
/// target class itself or inside a class that (transitively) extends the
/// target.  When `true`, `__construct` is offered for `::` access
/// (e.g. `self::__construct()`, `static::__construct()`,
/// `parent::__construct()`, `ClassName::__construct()` from within a
/// subclass).  When `false`, magic methods are suppressed entirely.
pub(crate) fn build_completion_items(
    target_class: &ClassInfo,
    access_kind: AccessKind,
    current_class_name: Option<&str>,
    is_self_or_ancestor: bool,
) -> Vec<CompletionItem> {
    // Determine whether we are inside the same class as the target.
    let same_class = current_class_name.is_some_and(|name| name == target_class.name);
    let mut items: Vec<CompletionItem> = Vec::new();

    // Methods — filtered by static / instance, excluding magic methods
    for method in &target_class.methods {
        // `__construct` is meaningful to call explicitly via `::` when
        // inside the same class or a subclass (e.g.
        // `parent::__construct(...)`, `self::__construct()`).
        // Outside of that relationship, magic methods are suppressed.
        let is_constructor = method.name.eq_ignore_ascii_case("__construct");
        if is_magic_method(&method.name) {
            let allow = is_constructor
                && is_self_or_ancestor
                && matches!(
                    access_kind,
                    AccessKind::DoubleColon | AccessKind::ParentDoubleColon
                );
            if !allow {
                continue;
            }
        }

        // Visibility filtering:
        // - private: only visible from within the same class
        // - protected: visible from the same class or a subclass
        //   (we approximate by allowing when inside any class)
        if method.visibility == Visibility::Private && !same_class {
            continue;
        }
        if method.visibility == Visibility::Protected && !same_class && !is_self_or_ancestor {
            continue;
        }

        let include = match access_kind {
            AccessKind::Arrow => !method.is_static,
            // External `ClassName::` shows only static methods, but
            // `__construct` is an exception — it's an instance method
            // that is routinely called via `ClassName::__construct()`
            // from within a subclass.
            AccessKind::DoubleColon => method.is_static || is_constructor,
            // `self::`, `static::`, and `parent::` show both static and
            // non-static methods (PHP allows calling instance methods
            // via `::` from within the class hierarchy).
            AccessKind::ParentDoubleColon => true,
            AccessKind::Other => true,
        };
        if !include {
            continue;
        }

        let label = build_method_label(method);
        items.push(CompletionItem {
            label,
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(format!("Class: {}", display_class_name(&target_class.name))),
            insert_text: Some(build_callable_snippet(&method.name, &method.parameters)),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            filter_text: Some(method.name.clone()),
            deprecated: if method.deprecation_message.is_some() {
                Some(true)
            } else {
                None
            },
            ..CompletionItem::default()
        });
    }

    // Properties — filtered by static / instance
    for property in &target_class.properties {
        if property.visibility == Visibility::Private && !same_class {
            continue;
        }
        if property.visibility == Visibility::Protected && !same_class && !is_self_or_ancestor {
            continue;
        }

        let include = match access_kind {
            AccessKind::Arrow => !property.is_static,
            AccessKind::DoubleColon | AccessKind::ParentDoubleColon => property.is_static,
            AccessKind::Other => true,
        };
        if !include {
            continue;
        }

        // Static properties accessed via `::` need the `$` prefix
        // (e.g. `self::$path`, `ClassName::$path`), while instance
        // properties via `->` use the bare name (e.g. `$this->path`).
        let display_name = if access_kind == AccessKind::DoubleColon
            || access_kind == AccessKind::ParentDoubleColon
        {
            format!("${}", property.name)
        } else {
            property.name.clone()
        };

        let display = display_class_name(&target_class.name);
        let detail = if let Some(ref th) = property.type_hint {
            format!("Class: {} — {}", display, th)
        } else {
            format!("Class: {}", display)
        };

        items.push(CompletionItem {
            label: display_name.clone(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(detail),
            insert_text: Some(display_name.clone()),
            filter_text: Some(display_name),
            deprecated: if property.deprecation_message.is_some() {
                Some(true)
            } else {
                None
            },
            ..CompletionItem::default()
        });
    }

    // Constants — only for `::`, `parent::`, or unqualified access
    if access_kind == AccessKind::DoubleColon
        || access_kind == AccessKind::ParentDoubleColon
        || access_kind == AccessKind::Other
    {
        for constant in &target_class.constants {
            if constant.visibility == Visibility::Private && !same_class {
                continue;
            }
            if constant.visibility == Visibility::Protected && !same_class && !is_self_or_ancestor {
                continue;
            }

            let display = display_class_name(&target_class.name);
            let detail = if let Some(ref th) = constant.type_hint {
                format!("Class: {} — {}", display, th)
            } else {
                format!("Class: {}", display)
            };

            items.push(CompletionItem {
                label: constant.name.clone(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some(detail),
                insert_text: Some(constant.name.clone()),
                filter_text: Some(constant.name.clone()),
                deprecated: if constant.deprecation_message.is_some() {
                    Some(true)
                } else {
                    None
                },
                ..CompletionItem::default()
            });
        }
    }

    // `::class` keyword — returns the fully qualified class name as a string.
    // Available on any class, interface, or enum via `::` access.
    if access_kind == AccessKind::DoubleColon || access_kind == AccessKind::ParentDoubleColon {
        items.push(CompletionItem {
            label: "class".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("class-string".to_string()),
            insert_text: Some("class".to_string()),
            filter_text: Some("class".to_string()),
            ..CompletionItem::default()
        });
    }

    // Sort all items alphabetically (case-insensitive) and assign
    // sort_text so the editor preserves this ordering.
    items.sort_by(|a, b| {
        a.filter_text
            .as_deref()
            .unwrap_or(&a.label)
            .to_lowercase()
            .cmp(&b.filter_text.as_deref().unwrap_or(&b.label).to_lowercase())
    });

    for (i, item) in items.iter_mut().enumerate() {
        item.sort_text = Some(format!("{:05}", i));
    }

    items
}

// ─── Union-merge pipeline ───────────────────────────────────────────────────

/// Check whether `target_class` is the same class as, or an ancestor of,
/// the class the cursor is inside.
///
/// Returns `true` when:
/// - `current_class.name == target_class.name` (same class), or
/// - walking the parent chain of `current_class` reaches `target_class`.
///
/// This controls visibility filtering: when `true`, `__construct` is
/// offered via `::` access and protected members are visible.
pub(crate) fn is_ancestor_of(
    current_class: Option<&ClassInfo>,
    target_class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    let Some(cc) = current_class else {
        return false;
    };
    if cc.name == target_class.name {
        return true;
    }
    // Walk the parent chain of the current class to see if the target
    // is an ancestor.
    let mut ancestor_name = cc.parent_class.clone();
    let mut depth = 0u32;
    while let Some(ref name) = ancestor_name {
        depth += 1;
        if depth > 20 {
            break;
        }
        // ClassInfo.name stores the short name (e.g. "BaseService")
        // while parent_class stores the FQN (e.g. "App\\BaseService").
        // Compare against both the full name and the short (last segment)
        // so that cross-file inheritance is detected correctly.
        let short = name.rsplit('\\').next().unwrap_or(name);
        if name == &target_class.name || short == target_class.name {
            return true;
        }
        ancestor_name = class_loader(name).and_then(|ci| ci.parent_class.clone());
    }
    false
}

/// Build completion items from multiple candidate classes (union types),
/// resolving each through full inheritance and deduplicating across them.
///
/// This is the high-level entry point that combines per-candidate item
/// building with union-aware merging.  For each candidate:
/// 1. Resolves the class fully (own + traits + parents + virtual members).
/// 2. Determines whether the cursor is inside the target class or a
///    subclass (for visibility and `__construct` filtering).
/// 3. Builds raw completion items via [`build_completion_items`].
///
/// The collected items are then passed to [`merge_union_completion_items`]
/// for deduplication and sort-tier assignment.
pub(crate) fn build_union_completion_items(
    candidates: &[ClassInfo],
    effective_access: AccessKind,
    current_class: Option<&ClassInfo>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: &crate::virtual_members::ResolvedClassCache,
) -> Vec<CompletionItem> {
    let current_class_name = current_class.map(|cc| cc.name.as_str());
    let num_candidates = candidates.len();

    // Track how many candidate classes contributed each label so we can
    // distinguish intersection vs branch-only members.
    let mut all_items: Vec<CompletionItem> = Vec::new();
    let mut occurrence_count: HashMap<String, usize> = HashMap::new();

    for target_class in candidates {
        let merged =
            crate::virtual_members::resolve_class_fully_cached(target_class, class_loader, cache);

        let self_or_ancestor = is_ancestor_of(current_class, target_class, class_loader);

        let items = build_completion_items(
            &merged,
            effective_access,
            current_class_name,
            self_or_ancestor,
        );

        for item in items {
            if let Some(existing) = all_items
                .iter_mut()
                .find(|existing| existing.label == item.label)
            {
                *occurrence_count.entry(existing.label.clone()).or_insert(1) += 1;
                // Merge class names into the detail so the user sees
                // which types provide this member (e.g.
                // "User|AdminUser" for shared members vs "AdminUser"
                // for branch-only members).
                merge_detail_class_names(existing, &item);
            } else {
                occurrence_count.insert(item.label.clone(), 1);
                all_items.push(item);
            }
        }
    }

    merge_union_completion_items(all_items, occurrence_count, num_candidates)
}

/// Merge class names from a new item's `detail` into an existing item's
/// `detail`.
///
/// Both items are expected to have a `detail` of the form
/// `"Class: Foo"` or `"Class: Foo \u{2014} type"`.  The class name from
/// `new_item` is appended with `|` if not already present.
fn merge_detail_class_names(existing: &mut CompletionItem, new_item: &CompletionItem) {
    let em_dash = " \u{2014} ";
    let get_cls = |d: &str| -> Option<String> {
        d.strip_prefix("Class: ")
            .map(|r| r.split(em_dash).next().unwrap_or(r).to_string())
    };

    if let (Some(existing_detail), Some(new_detail)) = (&mut existing.detail, &new_item.detail)
        && let (Some(ec), Some(nc)) = (get_cls(existing_detail), get_cls(new_detail))
        && !ec.split('|').any(|p| p == nc)
    {
        let merged = format!("{ec}|{nc}");
        if let Some(pos) = existing_detail.find(em_dash) {
            let suffix = existing_detail[pos..].to_string();
            *existing_detail = format!("Class: {merged}{suffix}");
        } else {
            *existing_detail = format!("Class: {merged}");
        }
    }
}

/// Partition and sort completion items by union membership.
///
/// When a variable has a union type (`num_candidates > 1`), members
/// present on **all** candidate types (intersection members) are more
/// likely to be type-safe.  This function:
///
/// 1. Partitions items into intersection and branch-only based on
///    `occurrence_count` vs `num_candidates`.
/// 2. Sorts each partition alphabetically by `filter_text` / `label`.
/// 3. Assigns `sort_text` prefixes (`"0_"` for intersection, `"1_"` for
///    branch-only) so intersection members appear first in the popup.
/// 4. Adds `label_details` to branch-only items showing which class(es)
///    provide them.
///
/// When `num_candidates <= 1`, returns `items` unchanged (the items
/// already have correct `sort_text` from [`build_completion_items`]).
pub(crate) fn merge_union_completion_items(
    items: Vec<CompletionItem>,
    occurrence_count: HashMap<String, usize>,
    num_candidates: usize,
) -> Vec<CompletionItem> {
    if num_candidates <= 1 {
        return items;
    }

    let sort_key = |item: &CompletionItem| -> String {
        item.filter_text
            .as_deref()
            .unwrap_or(&item.label)
            .to_lowercase()
    };

    let mut intersection: Vec<CompletionItem> = Vec::new();
    let mut branch_only: Vec<CompletionItem> = Vec::new();

    for item in items {
        let count = occurrence_count.get(&item.label).copied().unwrap_or(1);
        if count >= num_candidates {
            intersection.push(item);
        } else {
            branch_only.push(item);
        }
    }

    intersection.sort_by_key(|item| sort_key(item));
    branch_only.sort_by_key(|item| sort_key(item));

    // Assign sort_text: "0_NNNNN" for intersection, "1_NNNNN" for
    // branch-only.
    let mut result = Vec::with_capacity(intersection.len() + branch_only.len());

    for (i, mut item) in intersection.into_iter().enumerate() {
        item.sort_text = Some(format!("0_{:05}", i));
        result.push(item);
    }

    let em_dash = " \u{2014} ";
    for (i, mut item) in branch_only.into_iter().enumerate() {
        item.sort_text = Some(format!("1_{:05}", i));
        // Add label_details showing the originating class(es) so the
        // user can tell at a glance which branch provides this member.
        if let Some(ref detail) = item.detail {
            let class_names = detail
                .strip_prefix("Class: ")
                .map(|r| r.split(em_dash).next().unwrap_or(r))
                .unwrap_or("");
            if !class_names.is_empty() {
                item.label_details = Some(CompletionItemLabelDetails {
                    detail: None,
                    description: Some(class_names.to_string()),
                });
            }
        }
        result.push(item);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClassInfo;

    /// Helper to build a minimal `CompletionItem` with a label, detail, and
    /// filter_text — the fields that the merge logic inspects.
    fn item(label: &str, detail: &str) -> CompletionItem {
        CompletionItem {
            label: label.to_string(),
            detail: Some(detail.to_string()),
            filter_text: Some(label.to_string()),
            ..CompletionItem::default()
        }
    }

    // ── merge_detail_class_names ────────────────────────────────────────

    #[test]
    fn merge_detail_appends_new_class_name() {
        let mut existing = item("foo", "Class: User");
        let new = item("foo", "Class: AdminUser");
        merge_detail_class_names(&mut existing, &new);
        assert_eq!(existing.detail.as_deref(), Some("Class: User|AdminUser"));
    }

    #[test]
    fn merge_detail_preserves_type_suffix() {
        let mut existing = item("name", "Class: User \u{2014} string");
        let new = item("name", "Class: AdminUser \u{2014} string");
        merge_detail_class_names(&mut existing, &new);
        assert_eq!(
            existing.detail.as_deref(),
            Some("Class: User|AdminUser \u{2014} string")
        );
    }

    #[test]
    fn merge_detail_does_not_duplicate_same_class() {
        let mut existing = item("foo", "Class: User");
        let new = item("foo", "Class: User");
        merge_detail_class_names(&mut existing, &new);
        assert_eq!(existing.detail.as_deref(), Some("Class: User"));
    }

    #[test]
    fn merge_detail_handles_three_classes() {
        let mut existing = item("foo", "Class: A");
        merge_detail_class_names(&mut existing, &item("foo", "Class: B"));
        merge_detail_class_names(&mut existing, &item("foo", "Class: C"));
        assert_eq!(existing.detail.as_deref(), Some("Class: A|B|C"));
    }

    #[test]
    fn merge_detail_no_op_when_details_missing() {
        let mut existing = CompletionItem {
            label: "foo".to_string(),
            detail: None,
            ..CompletionItem::default()
        };
        let new = item("foo", "Class: User");
        merge_detail_class_names(&mut existing, &new);
        assert_eq!(existing.detail, None);
    }

    // ── merge_union_completion_items ─────────────────────────────────────

    #[test]
    fn single_candidate_returns_items_unchanged() {
        let items = vec![item("foo", "Class: A"), item("bar", "Class: A")];
        let mut counts = std::collections::HashMap::new();
        counts.insert("foo".to_string(), 1);
        counts.insert("bar".to_string(), 1);

        let result = merge_union_completion_items(items.clone(), counts, 1);
        // With a single candidate, items pass through unchanged.
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].label, items[0].label);
        assert_eq!(result[1].label, items[1].label);
    }

    #[test]
    fn intersection_members_sorted_before_branch_only() {
        // Two candidates: both have "shared", only one has "unique_a",
        // only one has "unique_b".
        let items = vec![
            item("shared", "Class: A|B"),
            item("unique_a", "Class: A"),
            item("unique_b", "Class: B"),
        ];
        let mut counts = std::collections::HashMap::new();
        counts.insert("shared".to_string(), 2);
        counts.insert("unique_a".to_string(), 1);
        counts.insert("unique_b".to_string(), 1);

        let result = merge_union_completion_items(items, counts, 2);
        assert_eq!(result.len(), 3);

        // Intersection member first (sort_text starts with "0_").
        assert_eq!(result[0].label, "shared");
        assert!(result[0].sort_text.as_deref().unwrap().starts_with("0_"));

        // Branch-only members after (sort_text starts with "1_").
        assert!(result[1].sort_text.as_deref().unwrap().starts_with("1_"));
        assert!(result[2].sort_text.as_deref().unwrap().starts_with("1_"));
    }

    #[test]
    fn branch_only_items_get_label_details() {
        let items = vec![item("only_a", "Class: A")];
        let mut counts = std::collections::HashMap::new();
        counts.insert("only_a".to_string(), 1);

        let result = merge_union_completion_items(items, counts, 2);
        assert_eq!(result.len(), 1);
        let ld = result[0]
            .label_details
            .as_ref()
            .expect("should have label_details");
        assert_eq!(ld.description.as_deref(), Some("A"));
    }

    #[test]
    fn intersection_items_do_not_get_label_details() {
        let items = vec![item("shared", "Class: A|B")];
        let mut counts = std::collections::HashMap::new();
        counts.insert("shared".to_string(), 2);

        let result = merge_union_completion_items(items, counts, 2);
        assert_eq!(result.len(), 1);
        assert!(result[0].label_details.is_none());
    }

    #[test]
    fn branch_only_items_sorted_alphabetically() {
        let items = vec![
            item("zebra", "Class: A"),
            item("alpha", "Class: A"),
            item("middle", "Class: A"),
        ];
        let mut counts = std::collections::HashMap::new();
        counts.insert("zebra".to_string(), 1);
        counts.insert("alpha".to_string(), 1);
        counts.insert("middle".to_string(), 1);

        let result = merge_union_completion_items(items, counts, 2);
        assert_eq!(result[0].label, "alpha");
        assert_eq!(result[1].label, "middle");
        assert_eq!(result[2].label, "zebra");
    }

    // ── is_ancestor_of ──────────────────────────────────────────────────

    #[test]
    fn same_class_is_ancestor() {
        let cls = ClassInfo {
            name: "Foo".to_string(),
            ..ClassInfo::default()
        };
        let loader = |_: &str| -> Option<Arc<ClassInfo>> { None };
        assert!(is_ancestor_of(Some(&cls), &cls, &loader));
    }

    #[test]
    fn no_current_class_is_not_ancestor() {
        let target = ClassInfo {
            name: "Foo".to_string(),
            ..ClassInfo::default()
        };
        let loader = |_: &str| -> Option<Arc<ClassInfo>> { None };
        assert!(!is_ancestor_of(None, &target, &loader));
    }

    #[test]
    fn direct_parent_is_ancestor() {
        let parent = ClassInfo {
            name: "Parent".to_string(),
            ..ClassInfo::default()
        };
        let child = ClassInfo {
            name: "Child".to_string(),
            parent_class: Some("Parent".to_string()),
            ..ClassInfo::default()
        };
        let loader = |_: &str| -> Option<Arc<ClassInfo>> { None };
        assert!(is_ancestor_of(Some(&child), &parent, &loader));
    }

    #[test]
    fn grandparent_is_ancestor_via_loader() {
        let grandparent = ClassInfo {
            name: "GrandParent".to_string(),
            ..ClassInfo::default()
        };
        let child = ClassInfo {
            name: "Child".to_string(),
            parent_class: Some("Parent".to_string()),
            ..ClassInfo::default()
        };
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "Parent" {
                Some(Arc::new(ClassInfo {
                    name: "Parent".to_string(),
                    parent_class: Some("GrandParent".to_string()),
                    ..ClassInfo::default()
                }))
            } else {
                None
            }
        };
        assert!(is_ancestor_of(Some(&child), &grandparent, &loader));
    }

    #[test]
    fn unrelated_class_is_not_ancestor() {
        let current = ClassInfo {
            name: "Foo".to_string(),
            parent_class: Some("Bar".to_string()),
            ..ClassInfo::default()
        };
        let target = ClassInfo {
            name: "Baz".to_string(),
            ..ClassInfo::default()
        };
        let loader = |_: &str| -> Option<Arc<ClassInfo>> { None };
        assert!(!is_ancestor_of(Some(&current), &target, &loader));
    }

    #[test]
    fn fqn_parent_matches_short_name_target() {
        let parent_target = ClassInfo {
            name: "BaseService".to_string(),
            ..ClassInfo::default()
        };
        let child = ClassInfo {
            name: "MyService".to_string(),
            parent_class: Some("App\\BaseService".to_string()),
            ..ClassInfo::default()
        };
        let loader = |_: &str| -> Option<Arc<ClassInfo>> { None };
        assert!(is_ancestor_of(Some(&child), &parent_target, &loader));
    }
}
