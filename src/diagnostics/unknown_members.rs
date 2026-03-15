//! Unknown member access diagnostics.
//!
//! Walk the precomputed [`SymbolMap`] for a file and flag every
//! `MemberAccess` span where the member does not exist on the resolved
//! class after full resolution (inheritance + virtual member providers).
//!
//! Diagnostics use `Severity::Warning` because the code may still run
//! (e.g. via `__call` / `__get` magic methods that we cannot see), but
//! the user benefits from knowing that PHPantom can't resolve the member.
//!
//! We suppress diagnostics when:
//!
//! - The subject type cannot be resolved (we can't know what members it has).
//! - Any resolved class in a union type has the member (the member is
//!   valid for at least one branch of the union).
//! - Any resolved class has `__call` / `__callStatic` (for method calls)
//!   or `__get` (for property access) magic methods — these accept
//!   arbitrary member names at runtime.
//! - Any resolved class is `stdClass` — it is a universal object
//!   container that accepts arbitrary properties at runtime.
//! - The member name is `class` (the magic `::class` constant).
//! - The subject is an enum and the member is a case name (enum cases
//!   are accessed via `::` but stored as constants).

use std::collections::HashMap;
use std::sync::Arc;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::resolver::{
    ResolutionCtx, resolve_target_classes, resolve_target_classes_expr,
};
use crate::completion::variable::raw_type_inference::resolve_variable_assignment_raw_type;
use crate::docblock::type_strings::is_scalar;
use crate::hover::variable_type::resolve_variable_type_string;
use crate::inheritance::resolve_property_type_hint;
use crate::subject_expr::SubjectExpr;
use crate::symbol_map::SymbolKind;
use crate::types::{AccessKind, ClassInfo};
use crate::virtual_members::{resolve_class_fully_cached, resolve_class_fully_maybe_cached};

use super::helpers::{find_innermost_enclosing_class, make_diagnostic};
use super::offset_range_to_lsp_range;

/// Diagnostic code used for unknown-member diagnostics so that code
/// actions can match on it.
pub(crate) const UNKNOWN_MEMBER_CODE: &str = "unknown_member";

/// Diagnostic code used when member access is attempted on a scalar
/// type (int, string, bool, float, null, void, never, array).  This
/// is always a runtime crash, so the severity is `Error`.
pub(crate) const SCALAR_MEMBER_ACCESS_CODE: &str = "scalar_member_access";

impl Backend {
    /// Collect unknown-member diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub fn collect_unknown_member_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Gather context under locks ──────────────────────────────────
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_use_map: HashMap<String, String> =
            self.use_map.read().get(uri).cloned().unwrap_or_default();

        let file_namespace: Option<String> = self.namespace_map.read().get(uri).cloned().flatten();

        let local_classes: Vec<Arc<ClassInfo>> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();

        let class_loader = self.class_loader_with(&local_classes, &file_use_map, &file_namespace);
        let function_loader = self.function_loader_with(&file_use_map, &file_namespace);
        let cache = &self.resolved_class_cache;

        // ── Walk every symbol span ──────────────────────────────────────
        for span in &symbol_map.spans {
            let (subject_text, member_name, is_static, is_method_call, is_docblock_ref) =
                match &span.kind {
                    SymbolKind::MemberAccess {
                        subject_text,
                        member_name,
                        is_static,
                        is_method_call,
                        is_docblock_reference,
                    } => (
                        subject_text,
                        member_name,
                        *is_static,
                        *is_method_call,
                        *is_docblock_reference,
                    ),
                    _ => continue,
                };

            // ── Skip the magic `::class` constant ───────────────────────
            if member_name == "class" && is_static {
                continue;
            }

            // ── Resolve the subject using the full completion pipeline ───
            // This handles bare variables, property chains, method call
            // return types, static access, and all other subject forms
            // identically to completion and go-to-definition.
            let access_kind = if is_static {
                AccessKind::DoubleColon
            } else {
                AccessKind::Arrow
            };

            let current_class = find_innermost_enclosing_class(&local_classes, span.start);

            let rctx = ResolutionCtx {
                current_class,
                all_classes: &local_classes,
                content,
                cursor_offset: span.start,
                class_loader: &class_loader,
                resolved_class_cache: Some(cache),
                function_loader: Some(&function_loader),
            };

            let base_classes: Vec<ClassInfo> =
                resolve_target_classes(subject_text, access_kind, &rctx);

            // ── Subject did not resolve to any class ────────────────────
            // Three possibilities:
            //
            //   a) The subject is a bare untyped variable with no type
            //      info at all (`$x` where we have no annotations or
            //      assignments to infer from).  Skip — the opt-in
            //      `unresolved-member-access` diagnostic covers this.
            //
            //   b) The subject is a chain (`$obj->prop`, `$obj->m()`)
            //      or a typed parameter/variable whose type resolved to
            //      a scalar or an unknown class.  The developer clearly
            //      expects a class type here.  Emit a diagnostic.
            //
            //   c) The subject has a scalar type we can name (bool, int,
            //      string, …).  Give a specific message.
            if base_classes.is_empty() {
                let expr = SubjectExpr::parse(subject_text);

                // Try to find a specific scalar type name.  This covers
                // bare variables (`$number = 1`), property chains
                // (`$user->age`), and call expressions (`getInt()`,
                // `$user->getAge()`).
                let scalar_type = resolve_scalar_subject_type(
                    &expr,
                    access_kind,
                    &rctx,
                    &class_loader,
                    &function_loader,
                    cache,
                );

                if let Some(ref scalar) = scalar_type {
                    // Scalar member access — always a runtime crash.
                    let range = match offset_range_to_lsp_range(
                        content,
                        span.start as usize,
                        span.end as usize,
                    ) {
                        Some(r) => r,
                        None => continue,
                    };
                    let kind_label = if is_method_call { "method" } else { "property" };
                    let message = format!(
                        "Cannot access {} '{}' on type '{}'",
                        kind_label, member_name, scalar,
                    );
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::ERROR,
                        SCALAR_MEMBER_ACCESS_CODE,
                        message,
                    ));
                    continue;
                }

                // Not scalar — check if the subject is a variable whose
                // raw type is a class name that can't be resolved.  This
                // covers `@param NoteReal $value` where NoteReal doesn't
                // exist, and `$bad = unknownReturnFn()` where the return
                // type names an unknown class.
                if let Some(unresolved_class) = resolve_unresolvable_class_subject(
                    &expr,
                    &rctx,
                    &class_loader,
                    &function_loader,
                ) {
                    let range = match offset_range_to_lsp_range(
                        content,
                        span.start as usize,
                        span.end as usize,
                    ) {
                        Some(r) => r,
                        None => continue,
                    };
                    let kind_label = if is_method_call { "method" } else { "property" };
                    let message = format!(
                        "Cannot verify {} '{}' — subject type '{}' could not be resolved",
                        kind_label, member_name, unresolved_class,
                    );
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::WARNING,
                        UNKNOWN_MEMBER_CODE,
                        message,
                    ));
                    continue;
                }

                // Not scalar, not unknown-class — check if the subject is
                // a chain or call expression where the type simply couldn't
                // be resolved.
                let is_chain = matches!(
                    expr,
                    SubjectExpr::PropertyChain { .. } | SubjectExpr::CallExpr { .. }
                );
                if is_chain {
                    let range = match offset_range_to_lsp_range(
                        content,
                        span.start as usize,
                        span.end as usize,
                    ) {
                        Some(r) => r,
                        None => continue,
                    };
                    let kind_label = if is_method_call { "method" } else { "property" };
                    let message = format!(
                        "Cannot verify {} '{}' — subject type could not be resolved",
                        kind_label, member_name,
                    );
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::WARNING,
                        UNKNOWN_MEMBER_CODE,
                        message,
                    ));
                }
                continue;
            }

            // ── Quick check on pre-resolved base classes ────────────────
            // `resolve_target_classes` already returns fully-resolved
            // classes in many code paths (e.g. `type_hint_to_classes`
            // calls `resolve_class_fully` and injects model-specific
            // scope methods onto Eloquent Builders).  Check the member
            // on these classes FIRST, before re-resolving through the
            // cache.  The cache is keyed by bare FQN and may hold a
            // stale entry that lacks context-specific virtual members
            // (e.g. Builder scope methods that depend on the concrete
            // model type).  Checking here avoids false positives when
            // the cache and the resolver disagree.
            if base_classes
                .iter()
                .any(|c| has_magic_method_for_access(c, is_static, is_method_call))
            {
                continue;
            }
            if base_classes.iter().any(|c| c.name == "stdClass") {
                continue;
            }
            if base_classes.iter().any(|c| {
                member_exists(c, member_name, is_static, is_method_call)
                    || (is_docblock_ref && member_exists_relaxed(c, member_name, is_method_call))
            }) {
                continue;
            }

            // ── Fully resolve each class (inheritance + virtual members) ─
            // Synthetic classes like `__object_shape` already carry all
            // their members and must NOT go through the cache (every
            // object shape shares the same name, so the cache would
            // return the wrong entry).
            let resolved_classes: Vec<ClassInfo> = base_classes
                .iter()
                .map(|c| {
                    if c.name == "__object_shape" {
                        c.clone()
                    } else {
                        resolve_class_fully_cached(c, &class_loader, cache)
                    }
                })
                .collect();

            // ── Check for magic methods on ANY branch ───────────────────
            if resolved_classes
                .iter()
                .any(|c| has_magic_method_for_access(c, is_static, is_method_call))
            {
                continue;
            }

            // ── Skip stdClass (universal object container) ──────────────
            if resolved_classes.iter().any(|c| c.name == "stdClass") {
                continue;
            }

            // ── Check whether the member exists on ANY branch ───────────
            if resolved_classes.iter().any(|c| {
                member_exists(c, member_name, is_static, is_method_call)
                    || (is_docblock_ref && member_exists_relaxed(c, member_name, is_method_call))
            }) {
                continue;
            }

            // ── Member is unresolved on ALL branches — emit diagnostic ──
            let range =
                match offset_range_to_lsp_range(content, span.start as usize, span.end as usize) {
                    Some(r) => r,
                    None => continue,
                };

            let kind_label = if is_method_call {
                "Method"
            } else if is_static {
                // Static non-method could be a property ($prop) or constant
                "Member"
            } else {
                "Property"
            };

            // Show the first resolved class name for context.  For union
            // types we could list all of them, but keeping it short is
            // more useful in the editor gutter.
            let class_display = display_class_name(&resolved_classes[0]);

            let message = if resolved_classes.len() > 1 {
                format!(
                    "{} '{}' not found on any of the {} possible types ({})",
                    kind_label,
                    member_name,
                    resolved_classes.len(),
                    resolved_classes
                        .iter()
                        .map(display_class_name)
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            } else {
                format!(
                    "{} '{}' not found on class '{}'",
                    kind_label, member_name, class_display,
                )
            };

            out.push(make_diagnostic(
                range,
                DiagnosticSeverity::WARNING,
                UNKNOWN_MEMBER_CODE,
                message,
            ));
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Check whether a member exists on the fully-resolved class.
///
/// For method calls, checks `methods`.  For non-method static access,
/// checks constants first then static properties.  For instance property
/// access, checks properties.
///
/// Method name matching is case-insensitive (PHP methods are
/// case-insensitive).  Property and constant matching is case-sensitive.
/// Relaxed member check for docblock references (`@see Class::member`).
///
/// PHPDoc `@see` uses `::` notation for all members (instance properties,
/// instance methods, static properties, constants), so we check every
/// member kind regardless of `is_static` or `is_method_call`.
fn member_exists_relaxed(class: &ClassInfo, member_name: &str, _is_method_call: bool) -> bool {
    // Check methods (case-insensitive, like PHP).
    let lower = member_name.to_ascii_lowercase();
    if class
        .methods
        .iter()
        .any(|m| m.name.to_ascii_lowercase() == lower)
    {
        return true;
    }
    // Check instance and static properties.
    if class.properties.iter().any(|p| p.name == member_name) {
        return true;
    }
    // Check constants.
    class.constants.iter().any(|c| c.name == member_name)
}

fn member_exists(
    class: &ClassInfo,
    member_name: &str,
    is_static: bool,
    is_method_call: bool,
) -> bool {
    if is_method_call {
        // PHP method names are case-insensitive
        let lower = member_name.to_ascii_lowercase();
        return class
            .methods
            .iter()
            .any(|m| m.name.to_ascii_lowercase() == lower);
    }

    if is_static {
        // Static access: could be a constant (Foo::BAR) or static property (Foo::$bar)
        // Check constants first (most common for static non-method access)
        if class.constants.iter().any(|c| c.name == member_name) {
            return true;
        }
        // Check static properties
        if class
            .properties
            .iter()
            .any(|p| p.name == member_name && p.is_static)
        {
            return true;
        }
        return false;
    }

    // Instance property access ($obj->prop)
    // Properties are stored without the `$` prefix in ClassInfo.
    class.properties.iter().any(|p| p.name == member_name)
}

/// Check whether the resolved class has magic methods that would handle
/// the given access type dynamically.
///
/// - `__call` handles instance method calls (`$obj->anything()`)
/// - `__callStatic` handles static method calls (`Foo::anything()`)
/// - `__get` handles instance property reads (`$obj->anything`)
/// - `__set` also implies dynamic property support
///
/// When such magic methods exist, we suppress unknown-member diagnostics
/// because the member may be handled at runtime.
fn has_magic_method_for_access(class: &ClassInfo, is_static: bool, is_method_call: bool) -> bool {
    if is_method_call {
        let magic_name = if is_static { "__callStatic" } else { "__call" };
        return class
            .methods
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(magic_name));
    }

    // Property access — check for __get
    if !is_static {
        return class
            .methods
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case("__get"));
    }

    false
}

/// When `resolve_target_classes` returns empty for a chain subject, check
/// whether the terminal type is a known scalar (bool, int, string, etc.).
///
/// Returns `Some(type_name)` when the intermediate base resolves but the
/// terminal property/return type is scalar.  Returns `None` when the
/// subject is truly unresolvable.
fn resolve_scalar_subject_type(
    expr: &SubjectExpr,
    access_kind: AccessKind,
    rctx: &ResolutionCtx<'_>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
    cache: &crate::virtual_members::ResolvedClassCache,
) -> Option<String> {
    match expr {
        // ── Bare variable: $number = 1; $number->foo() ──────────
        SubjectExpr::Variable(var_name) => {
            let raw_type = resolve_variable_assignment_raw_type(
                var_name,
                rctx.content,
                rctx.cursor_offset,
                rctx.current_class,
                rctx.all_classes,
                class_loader,
                rctx.function_loader,
            )?;
            let cleaned = crate::docblock::types::clean_type(&raw_type);
            if is_scalar(&cleaned) {
                Some(cleaned)
            } else {
                None
            }
        }

        // ── Property chain: $user->age->value ───────────────────
        SubjectExpr::PropertyChain { base, property } => {
            // Resolve the base to classes, then look up the property's
            // type hint on the resolved class.
            let base_classes = resolve_target_classes_expr(base, access_kind, rctx);
            for cls in &base_classes {
                let resolved = resolve_class_fully_maybe_cached(cls, class_loader, Some(cache));
                if let Some(hint) = resolve_property_type_hint(&resolved, property, class_loader) {
                    // Check each union branch — if ALL branches are scalar, the
                    // type is scalar.  If any branch is a class, resolve_target_classes
                    // would have returned it, so we wouldn't be here.
                    let cleaned = crate::docblock::types::clean_type(&hint);
                    if is_scalar(&cleaned) {
                        return Some(cleaned);
                    }
                    // Non-scalar, non-class type (e.g. a type alias we can't
                    // resolve) — treat as unresolvable.
                    return None;
                }
            }
            None
        }

        // ── Call expression: getInt()->value, $obj->getAge()->value ──
        SubjectExpr::CallExpr { callee, args_text } => {
            // Resolve the call return type.  If it's a scalar, report it.
            let return_classes = Backend::resolve_call_return_types_expr(callee, args_text, rctx);
            if return_classes.is_empty() {
                // Try to get the raw return type hint from the callable.
                match callee.as_ref() {
                    // Instance method call: $obj->getAge()
                    SubjectExpr::MethodCall { base, method } => {
                        let base_classes = resolve_target_classes_expr(base, access_kind, rctx);
                        for cls in &base_classes {
                            let resolved =
                                resolve_class_fully_maybe_cached(cls, class_loader, Some(cache));
                            if let Some(m) = resolved
                                .methods
                                .iter()
                                .find(|m| m.name.eq_ignore_ascii_case(method))
                                && let Some(ref hint) = m.return_type
                            {
                                let cleaned = crate::docblock::types::clean_type(hint);
                                if is_scalar(&cleaned) {
                                    return Some(cleaned);
                                }
                            }
                        }
                    }
                    // Standalone function call: getInt()
                    SubjectExpr::FunctionCall(fn_name) => {
                        if let Some(func_info) = function_loader(fn_name)
                            && let Some(ref hint) = func_info.return_type
                        {
                            let cleaned = crate::docblock::types::clean_type(hint);
                            if is_scalar(&cleaned) {
                                return Some(cleaned);
                            }
                        }
                    }
                    // Static method call: Foo::getInt()
                    SubjectExpr::StaticMethodCall { class, method } => {
                        let cls = class_loader(class);
                        if let Some(cls) = cls {
                            let resolved =
                                resolve_class_fully_maybe_cached(&cls, class_loader, Some(cache));
                            if let Some(m) = resolved
                                .methods
                                .iter()
                                .find(|m| m.name.eq_ignore_ascii_case(method))
                                && let Some(ref hint) = m.return_type
                            {
                                let cleaned = crate::docblock::types::clean_type(hint);
                                if is_scalar(&cleaned) {
                                    return Some(cleaned);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

/// Return a user-friendly display name for a class.
///
/// Prefers the short name for readability. For anonymous classes, returns
/// the full internal name.
/// When the subject is a variable (or a call whose return type names a
/// class), check whether the raw type is a non-scalar, non-mixed class
/// name that cannot be resolved.  Returns `Some(class_name)` when the
/// subject type is an unresolvable class, `None` otherwise.
///
/// This lets us emit a warning-level "subject type 'X' could not be
/// resolved" instead of silently dropping the diagnostic or emitting a
/// hint-level unresolved-member-access.
fn resolve_unresolvable_class_subject(
    expr: &SubjectExpr,
    rctx: &ResolutionCtx<'_>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
) -> Option<String> {
    let raw_type = match expr {
        SubjectExpr::Variable(var_name) => {
            // Try assignment-based raw type first (covers `$x = new Foo`
            // and native parameter type hints like `int $x`).
            let assignment_type = resolve_variable_assignment_raw_type(
                var_name,
                rctx.content,
                rctx.cursor_offset,
                rctx.current_class,
                rctx.all_classes,
                class_loader,
                rctx.function_loader,
            );
            // Fall back to the hover variable type resolver which also
            // checks PHPDoc `@param` annotations and foreach bindings.
            assignment_type.or_else(|| {
                resolve_variable_type_string(
                    var_name,
                    rctx.content,
                    rctx.cursor_offset,
                    rctx.current_class,
                    rctx.all_classes,
                    class_loader,
                    rctx.function_loader,
                )
            })
        }
        SubjectExpr::CallExpr { callee, .. } => match callee.as_ref() {
            SubjectExpr::FunctionCall(fn_name) => {
                let fi = function_loader(fn_name)?;
                fi.return_type.clone()
            }
            _ => None,
        },
        _ => None,
    }?;

    let cleaned = crate::docblock::types::clean_type(&raw_type);

    // Skip scalars, mixed, void, never, array, callable, object, null,
    // resource, iterable — these are not class names.
    if is_scalar(&cleaned)
        || matches!(
            cleaned.as_str(),
            "mixed"
                | "void"
                | "never"
                | "array"
                | "callable"
                | "object"
                | "null"
                | "resource"
                | "iterable"
        )
    {
        return None;
    }

    // The cleaned type looks like a class name.  If we can't resolve it,
    // the subject type is an unknown class.
    if class_loader(&cleaned).is_none() {
        Some(cleaned)
    } else {
        None
    }
}

fn display_class_name(class: &ClassInfo) -> String {
    if class.name.starts_with("__anonymous@") {
        return "anonymous class".to_string();
    }

    // Show the FQN when available for clarity.
    match &class.file_namespace {
        Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, class.name),
        _ => class.name.clone(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::SCALAR_MEMBER_ACCESS_CODE;
    use super::*;

    fn collect(backend: &Backend, uri: &str, content: &str) -> Vec<Diagnostic> {
        backend.update_ast(uri, content);
        let mut out = Vec::new();
        backend.collect_unknown_member_diagnostics(uri, content, &mut out);
        out
    }

    // ── Basic detection ─────────────────────────────────────────────────

    #[test]
    fn flags_unknown_method_on_known_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->nonexistent();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown method diagnostic for nonexistent(), got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_property_on_known_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public string $name = '';
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->missing;
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("missing") && d.message.contains("not found")),
            "Expected unknown property diagnostic for ->missing, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_static_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public static function bar(): void {}
}

Foo::nonexistent();
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown static method diagnostic, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_constant_on_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    const BAR = 1;
}

$x = Foo::MISSING;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("MISSING") && d.message.contains("not found")),
            "Expected unknown constant diagnostic, got: {:?}",
            diags
        );
    }

    // ── No false positives for existing members ─────────────────────────

    #[test]
    fn no_diagnostic_for_existing_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->bar();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for existing method, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_existing_property() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public string $name = '';
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->name;
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for existing property, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_existing_constant() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    const BAR = 1;
}

$x = Foo::BAR;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for existing constant, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_class_keyword() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {}

$name = Foo::class;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for ::class, got: {:?}",
            diags
        );
    }

    // ── Magic method suppression ────────────────────────────────────────

    #[test]
    fn no_diagnostic_when_class_has_magic_call() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Magic {
    public function __call(string $name, array $args): mixed {}
}

class Consumer {
    public function run(): void {
        $m = new Magic();
        $m->anything();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when __call exists, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_when_class_has_magic_get() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class DynProps {
    public function __get(string $name): mixed {}
}

class Consumer {
    public function run(): void {
        $d = new DynProps();
        $d->anything;
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when __get exists, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_when_class_has_magic_call_static() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class StaticMagic {
    public static function __callStatic(string $name, array $args): mixed {}
}

StaticMagic::anything();
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when __callStatic exists, got: {:?}",
            diags
        );
    }

    // ── Inheritance ─────────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_inherited_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Base {
    public function baseMethod(): void {}
}

class Child extends Base {}

class Consumer {
    public function run(): void {
        $c = new Child();
        $c->baseMethod();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for inherited method, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_trait_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
trait Greetable {
    public function greet(): string { return 'hello'; }
}

class Greeter {
    use Greetable;
}

class Consumer {
    public function run(): void {
        $g = new Greeter();
        $g->greet();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for trait method, got: {:?}",
            diags
        );
    }

    // ── Virtual members (@method / @property) ───────────────────────────

    #[test]
    fn no_diagnostic_for_phpdoc_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/**
 * @method string getName()
 */
class VirtualClass {}

class Consumer {
    public function run(): void {
        $v = new VirtualClass();
        $v->getName();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for @method virtual member, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_phpdoc_property() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/**
 * @property string $name
 */
class VirtualClass {
    public function __get(string $name): mixed {}
}

class Consumer {
    public function run(): void {
        $v = new VirtualClass();
        $v->name;
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for @property virtual member, got: {:?}",
            diags
        );
    }

    // ── Subject resolution contexts ─────────────────────────────────────

    #[test]
    fn flags_unknown_method_on_this() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {
        $this->nonexistent();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown method diagnostic for $this->nonexistent(), got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_this_in_second_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class First {
    public function alpha(): void {}
}

class Second {
    public function beta(): void {}

    public function demo(): void {
        $this->beta();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for $this->beta() inside Second, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_object_shape_property() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
}

class Demo {
    /** @return object{name: string, age: int, active: bool} */
    public function getProfile(): object { return (object) []; }

    /** @return object{tool: Pen, meta: object{page: int, total: int}} */
    public function getResult(): object { return (object) []; }

    public function demo(): void {
        $profile = $this->getProfile();
        $profile->name;
        $profile->age;
        $profile->active;

        $result = $this->getResult();
        $result->tool;
        $result->meta;
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for object shape property access, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_property_on_object_shape() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Demo {
    /** @return object{name: string, age: int} */
    public function getProfile(): object { return (object) []; }

    public function demo(): void {
        $profile = $this->getProfile();
        $profile->missing;
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("missing") && d.message.contains("not found")),
            "Expected unknown property diagnostic on object shape, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_this_in_anonymous_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
}

class Factory {
    public function create(): Pen {
        return new class extends Pen {
            public string $brand;
            public function cap(): string { return ''; }
            public function demo() {
                $this->cap();
                $this->brand;
                $this->write();
            }
        };
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected inside anonymous class body, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_method_on_this_in_anonymous_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
}

class Factory {
    public function create(): Pen {
        return new class extends Pen {
            public function demo() {
                $this->nonexistent();
            }
        };
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown method diagnostic inside anonymous class, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_parent_in_anonymous_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
}

class Factory {
    public function create(): Pen {
        return new class extends Pen {
            public function demo() {
                parent::write();
            }
        };
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for parent::write() in anonymous class, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_method_on_this_in_second_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class First {
    public function alpha(): void {}
}

class Second {
    public function beta(): void {}

    public function demo(): void {
        $this->alpha();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("alpha") && d.message.contains("not found")),
            "Expected unknown method diagnostic for $this->alpha() inside Second, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_this_existing_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}

    public function baz(): void {
        $this->bar();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for $this->bar(), got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_method_on_self() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {
        self::nonexistent();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown method diagnostic for self::nonexistent(), got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_self_existing_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public static function bar(): void {}

    public function baz(): void {
        self::bar();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for self::bar(), got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_parent_existing_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Base {
    public function parentMethod(): void {}
}

class Child extends Base {
    public function childMethod(): void {
        parent::parentMethod();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for parent::parentMethod(), got: {:?}",
            diags
        );
    }

    // ── Diagnostic metadata ─────────────────────────────────────────────

    #[test]
    fn diagnostic_has_warning_severity() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->missing();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(!diags.is_empty(), "Expected at least one diagnostic");
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn diagnostic_has_code_and_source() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->missing();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(!diags.is_empty(), "Expected at least one diagnostic");
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(UNKNOWN_MEMBER_CODE.to_string()))
        );
        assert_eq!(diags[0].source, Some("phpantom".to_string()));
    }

    // ── Case-insensitive method matching ────────────────────────────────

    #[test]
    fn method_matching_is_case_insensitive() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function getData(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->getdata();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "PHP methods are case-insensitive, no diagnostic expected, got: {:?}",
            diags
        );
    }

    // ── Multiple unknown members ────────────────────────────────────────

    #[test]
    fn flags_multiple_unknown_members() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function known(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->unknown1();
        $f->known();
        $f->unknown2();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(
            diags.len(),
            2,
            "Expected exactly 2 diagnostics for 2 unknown members, got: {:?}",
            diags
        );
        assert!(diags.iter().any(|d| d.message.contains("unknown1")));
        assert!(diags.iter().any(|d| d.message.contains("unknown2")));
    }

    // ── Unresolvable subject produces no diagnostic ─────────────────────

    #[test]
    fn no_diagnostic_when_subject_unresolvable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function getUnknown(): mixed { return null; }

$x = getUnknown();
$x->whatever();
"#;
        let diags = collect(&backend, uri, content);
        // We can't resolve the type of $x, so we should not flag ->whatever()
        // as unknown — we'd just produce false positives.
        assert!(
            diags.is_empty(),
            "No diagnostics expected when subject type is unresolvable, got: {:?}",
            diags
        );
    }

    // ── Enum cases ──────────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_enum_case() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}

$c = Color::Red;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for enum case access, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unknown_enum_case() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}

$c = Color::Purple;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("Purple") && d.message.contains("not found")),
            "Expected unknown member diagnostic for Color::Purple, got: {:?}",
            diags
        );
    }

    // ── Parameter type hint resolution ──────────────────────────────────

    #[test]
    fn flags_unknown_method_via_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Service {
    public function doWork(): void {}
}

class Handler {
    public function handle(Service $svc): void {
        $svc->nonexistent();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown method diagnostic via parameter type, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_method_via_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Service {
    public function doWork(): void {}
}

class Handler {
    public function handle(Service $svc): void {
        $svc->doWork();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for existing method via parameter, got: {:?}",
            diags
        );
    }

    // ── Inherited magic methods ─────────────────────────────────────────

    #[test]
    fn no_diagnostic_when_parent_has_magic_call() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Base {
    public function __call(string $name, array $args): mixed {}
}

class Child extends Base {}

class Consumer {
    public function run(): void {
        $c = new Child();
        $c->anything();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when parent has __call, got: {:?}",
            diags
        );
    }

    // ── Interface method access ─────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_interface_method() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
interface Renderable {
    public function render(): string;
}

class View implements Renderable {
    public function render(): string { return ''; }
}

class Consumer {
    public function run(Renderable $r): void {
        $r->render();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for interface method, got: {:?}",
            diags
        );
    }

    // ── Static property access ──────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_existing_static_property() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Config {
    public static string $appName = 'test';
}

$name = Config::$appName;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for existing static property, got: {:?}",
            diags
        );
    }

    // ── Union type suppression ──────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_member_on_any_union_branch() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Lamp {
    public function dim(): void {}
    public function turnOff(): void {}
}

class Faucet {
    public function drip(): void {}
    public function turnOff(): void {}
}

class Consumer {
    public function run(): void {
        if (rand(0, 1)) {
            $ambiguous = new Lamp();
        } else {
            $ambiguous = new Faucet();
        }
        $ambiguous->turnOff();
        $ambiguous->dim();
        $ambiguous->drip();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        // dim() is on Lamp, drip() is on Faucet, turnOff() is on both.
        // None should produce a diagnostic because the member exists on
        // at least one branch of the union.
        assert!(
            diags.is_empty(),
            "No diagnostics expected for union branch members, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_member_missing_from_all_union_branches() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Lamp {
    public function dim(): void {}
    public function turnOff(): void {}
}

class Faucet {
    public function drip(): void {}
    public function turnOff(): void {}
}

class Consumer {
    public function run(): void {
        if (rand(0, 1)) {
            $ambiguous = new Lamp();
        } else {
            $ambiguous = new Faucet();
        }
        $ambiguous->nonexistent();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
            "Expected unknown method diagnostic when member is on no union branch, got: {:?}",
            diags
        );
    }

    #[test]
    fn union_diagnostic_message_mentions_multiple_types() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Lamp {
    public function dim(): void {}
}

class Faucet {
    public function drip(): void {}
}

class Consumer {
    public function run(): void {
        if (rand(0, 1)) {
            $ambiguous = new Lamp();
        } else {
            $ambiguous = new Faucet();
        }
        $ambiguous->nonexistent();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(!diags.is_empty(), "Expected at least one diagnostic");
        // The message should mention both types when the subject is a union.
        assert!(
            diags[0].message.contains("Lamp") && diags[0].message.contains("Faucet"),
            "Expected both union types in the message, got: {}",
            diags[0].message
        );
    }

    #[test]
    fn no_diagnostic_when_any_union_branch_has_magic_call() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Strict {
    public function known(): void {}
}

class Flexible {
    public function __call(string $name, array $args): mixed {}
}

class Consumer {
    public function run(): void {
        if (rand(0, 1)) {
            $obj = new Strict();
        } else {
            $obj = new Flexible();
        }
        $obj->anything();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when any union branch has __call, got: {:?}",
            diags
        );
    }

    // ── stdClass suppression ────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_property_on_stdclass() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
$obj = new \stdClass();
$obj->anything;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for property access on stdClass, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_method_on_stdclass() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
$obj = new \stdClass();
$obj->whatever();
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for method call on stdClass, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_stdclass_in_union() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Strict {
    public function known(): void {}
}

/** @var Strict|\stdClass $obj */
$obj = new Strict();
$obj->unknown_prop;
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when any union branch is stdClass, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_stdclass_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function process(\stdClass $obj): void {
    $obj->foo;
    $obj->bar;
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for property access on stdClass parameter, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_phpdoc_property_on_child_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
abstract class ZooBase
{
    public function falcon(): string { return ''; }
}

/**
 * @property string $gorilla
 * @method bool hyena(string $x)
 */
class Zoo extends ZooBase
{
    public function __get(string $name): mixed { return null; }
    public function __call(string $name, array $args): mixed { return null; }
}

function test(): void {
    $zoo = new Zoo();
    $zoo->gorilla;
    $zoo->hyena('x');
    $zoo->falcon();
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for @property/@method on child class with parent, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_phpdoc_property_from_interface() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/**
 * @property-read string $iguana
 * @method string jaguar()
 */
interface ZooContract {}

abstract class ZooBase
{
    public function falcon(): string { return ''; }
}

/**
 * @property string $gorilla
 * @method bool hyena(string $x)
 */
class Zoo extends ZooBase implements ZooContract
{
    public function __get(string $name): mixed { return null; }
    public function __call(string $name, array $args): mixed { return null; }
}

function test(): void {
    $zoo = new Zoo();
    $zoo->gorilla;
    $zoo->hyena('x');
    $zoo->iguana;
    $zoo->jaguar();
    $zoo->falcon();
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for @property/@method from class and interface, got: {:?}",
            diags
        );
    }

    /// Mirrors the exact pattern from `example.php` `runDemoAssertions()`:
    /// `$zoo->gorilla` inside an `assert(... === ...)` expression, with
    /// the Zoo class defined in a namespace.
    #[test]
    fn no_diagnostic_for_phpdoc_members_inside_assert() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
namespace Demo;

/**
 * @property-read string $iguana
 * @method string jaguar()
 */
interface ZooContract {}

abstract class ZooBase
{
    public function falcon(): string { return ''; }
}

/**
 * @property string $gorilla
 * @method bool hyena(string $x)
 */
class Zoo extends ZooBase implements ZooContract
{
    public function __get(string $name): mixed { return null; }
    public function __call(string $name, array $args): mixed { return null; }
}

function runTest(): void {
    $zoo = new Zoo();
    assert($zoo->gorilla === 'gorilla-value');
    assert($zoo->iguana === 'iguana-value');
    assert($zoo->hyena('x') === true);
    assert($zoo->jaguar() === 'jaguar-value');
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for @property/@method members inside assert(), got: {:?}",
            diags
        );
    }

    /// When `assert($zoo instanceof ZooBase)` narrows `$zoo` to ZooBase,
    /// subsequent `$zoo->gorilla` should still find @property/@method
    /// from the original `Zoo` class (or at least not false-positive
    /// because ZooBase has `__get`/`__call` via its child).
    #[test]
    fn no_diagnostic_for_phpdoc_members_after_instanceof_narrowing() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
namespace Demo;

/**
 * @property-read string $iguana
 * @method string jaguar()
 */
interface ZooContract {}

abstract class ZooBase
{
    public function falcon(): string { return ''; }
}

/**
 * @property string $gorilla
 * @method bool hyena(string $x)
 */
class Zoo extends ZooBase implements ZooContract
{
    public function __get(string $name): mixed { return null; }
    public function __call(string $name, array $args): mixed { return null; }
}

function runTest(): void {
    $zoo = new Zoo();
    assert($zoo instanceof Zoo);
    assert($zoo instanceof ZooBase);
    $zoo->gorilla;
    $zoo->iguana;
    $zoo->hyena('x');
    $zoo->jaguar();
    $zoo->falcon();
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for @property/@method after instanceof narrowing, got: {:?}",
            diags
        );
    }

    /// When accessing a member on a property chain like `$obj->prop->member`,
    /// the diagnostic should resolve the intermediate property type and
    /// flag unknown members on it.  Previously the ad-hoc subject resolver
    /// could not handle chains and silently skipped the diagnostic.
    #[test]
    fn flags_unknown_member_on_property_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Inner {
    public string $valid = '';
}
class Outer {
    public Inner $inner;
}
function test(): void {
    $obj = new Outer();
    $obj->inner->valid;
    $obj->inner->bogus;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("bogus"),
            "Diagnostic should mention 'bogus', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("Inner"),
            "Diagnostic should mention 'Inner', got: {}",
            diags[0].message,
        );
    }

    /// No diagnostic when the chained member actually exists.
    #[test]
    fn no_diagnostic_for_valid_property_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Inner {
    public string $value = '';
    public function greet(): string { return ''; }
}
class Outer {
    public Inner $inner;
}
function test(): void {
    $obj = new Outer();
    $obj->inner->value;
    $obj->inner->greet();
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for valid property chain, got: {:?}",
            diags,
        );
    }

    /// Unknown member on a method call return chain: `$obj->getInner()->bogus`.
    #[test]
    fn flags_unknown_member_on_method_return_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Inner {
    public string $value = '';
}
class Outer {
    public function getInner(): Inner { return new Inner(); }
}
function test(): void {
    $obj = new Outer();
    $obj->getInner()->value;
    $obj->getInner()->nope;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("nope"),
            "Diagnostic should mention 'nope', got: {}",
            diags[0].message,
        );
    }

    /// No diagnostic when method return chain member exists.
    #[test]
    fn no_diagnostic_for_valid_method_return_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Inner {
    public string $value = '';
}
class Outer {
    public function getInner(): Inner { return new Inner(); }
}
function test(): void {
    $obj = new Outer();
    $obj->getInner()->value;
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for valid method return chain, got: {:?}",
            diags,
        );
    }

    /// Property chain with a virtual (@property) member as the
    /// intermediate step — the resolved type of the virtual property
    /// should be used to check the final member.
    #[test]
    fn flags_unknown_member_on_virtual_property_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Related {
    public string $name = '';
}
/**
 * @property Related $relation
 */
class Model {
    public function __get(string $name): mixed { return null; }
}
function test(): void {
    $m = new Model();
    $m->relation->name;
    $m->relation->nonexistent;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("nonexistent"),
            "Diagnostic should mention 'nonexistent', got: {}",
            diags[0].message,
        );
    }

    /// When an intermediate property has a scalar type (bool), accessing
    /// a member on it should produce a diagnostic.  This is the exact
    /// pattern from the bug report: `$brandTranslation->lang_code->value`
    /// where `lang_code` has type `bool`.
    #[test]
    fn flags_member_access_on_scalar_property_type() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class BrandTranslation {
    /** @var bool */
    public $lang_code;
}
function test(): void {
    $bt = new BrandTranslation();
    $bt->lang_code->value;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("value"),
            "Diagnostic should mention 'value', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("bool"),
            "Diagnostic should mention 'bool', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                SCALAR_MEMBER_ACCESS_CODE.to_string()
            )),
            "Scalar member access should use scalar_member_access code",
        );
    }

    /// Member access on a string property should also produce a diagnostic.
    #[test]
    fn flags_member_access_on_string_property_type() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class User {
    public string $name = '';
}
function test(): void {
    $u = new User();
    $u->name->length;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("length"),
            "Diagnostic should mention 'length', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("string"),
            "Diagnostic should mention 'string', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                SCALAR_MEMBER_ACCESS_CODE.to_string()
            )),
            "Scalar member access should use scalar_member_access code",
        );
    }

    /// Member access on a scalar return type from a method call chain
    /// should produce a diagnostic: `$obj->getInt()->value`.
    #[test]
    fn flags_member_access_on_scalar_method_return() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Counter {
    public function getCount(): int { return 0; }
}
function test(): void {
    $c = new Counter();
    $c->getCount()->value;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("value"),
            "Diagnostic should mention 'value', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("int"),
            "Diagnostic should mention 'int', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                SCALAR_MEMBER_ACCESS_CODE.to_string()
            )),
            "Scalar member access should use scalar_member_access code",
        );
    }

    /// Member access on a virtual (@property) scalar type should also
    /// produce a diagnostic.
    #[test]
    fn flags_member_access_on_virtual_scalar_property() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/**
 * @property bool $active
 */
class Model {
    public function __get(string $name): mixed { return null; }
}
function test(): void {
    $m = new Model();
    $m->active->something;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("something"),
            "Diagnostic should mention 'something', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("bool"),
            "Diagnostic should mention 'bool', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                SCALAR_MEMBER_ACCESS_CODE.to_string()
            )),
            "Scalar member access should use scalar_member_access code",
        );
    }

    /// No diagnostic when a scalar property is accessed without chaining
    /// into a member (the scalar property access itself is valid).
    #[test]
    fn no_diagnostic_for_scalar_property_access_itself() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class BrandTranslation {
    /** @var bool */
    public $lang_code;
}
function test(): void {
    $bt = new BrandTranslation();
    $bt->lang_code;
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for accessing a scalar property itself, got: {:?}",
            diags,
        );
    }

    // ── Bare variable scalar member access ──────────────────────────────

    /// `$number = 1; $number->callHome()` — int has no members, this
    /// is always a runtime crash.
    #[test]
    fn flags_member_access_on_bare_int_variable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function test(): void {
    $number = 1;
    $number->callHome();
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("callHome"),
            "Diagnostic should mention 'callHome', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("int"),
            "Diagnostic should mention 'int', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                SCALAR_MEMBER_ACCESS_CODE.to_string()
            )),
            "Scalar member access should use scalar_member_access code",
        );
    }

    /// `$text = 'hello'; $text->length` — string property access.
    #[test]
    fn flags_property_access_on_bare_string_variable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function test(): void {
    $text = 'hello';
    $text->length;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("length"),
            "Diagnostic should mention 'length', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("string"),
            "Diagnostic should mention 'string', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
    }

    /// `$flag = true; $flag->isSet()` — bool method access.
    #[test]
    fn flags_method_access_on_bare_bool_variable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function test(): void {
    $flag = true;
    $flag->isSet();
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("isSet"),
            "Diagnostic should mention 'isSet', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("bool"),
            "Diagnostic should mention 'bool', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
    }

    // ── Function-return scalar member access ────────────────────────────

    /// `getInt()->value` — standalone function returning a scalar.
    #[test]
    fn flags_member_access_on_scalar_function_return() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/** @return int */
function getInt(): int { return 1; }
function test(): void {
    getInt()->value;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("value"),
            "Diagnostic should mention 'value', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("int"),
            "Diagnostic should mention 'int', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                SCALAR_MEMBER_ACCESS_CODE.to_string()
            )),
            "Scalar member access should use scalar_member_access code",
        );
    }

    /// `$count->getCount()->value` — method call chain ending in scalar
    /// return, then member access on that scalar.
    #[test]
    fn flags_member_access_on_scalar_method_return_via_variable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Counter {
    public function getCount(): int { return 0; }
}
function test(): void {
    $count = new Counter();
    $count->getCount()->value;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("value"),
            "Diagnostic should mention 'value', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("int"),
            "Diagnostic should mention 'int', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
    }

    /// No false positive: `$number = 1; $number` used without member
    /// access should produce zero diagnostics.
    #[test]
    fn no_diagnostic_for_bare_scalar_variable_without_member_access() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function test(): int {
    $number = 1;
    return $number;
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for scalar variable without member access, got: {:?}",
            diags,
        );
    }

    /// Scalar via typed parameter: `function f(int $x) { $x->foo(); }`
    #[test]
    fn flags_member_access_on_scalar_typed_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function test(int $x): void {
    $x->foo();
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("foo"),
            "Diagnostic should mention 'foo', got: {}",
            diags[0].message,
        );
        assert!(
            diags[0].message.contains("int"),
            "Diagnostic should mention 'int', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::ERROR),
            "Scalar member access should be ERROR severity",
        );
    }

    // ── Unknown-class subject diagnostics ───────────────────────────────

    /// `@param NoteReal $value` where NoteReal doesn't exist — member
    /// access on `$value` should produce a warning mentioning the
    /// unresolvable class name.
    #[test]
    fn flags_member_access_on_unknown_class_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/** @param NoteReal $value */
function helper($value): void {
    $value->something;
}
"#;
        let diags = collect(&backend, uri, content);
        assert_eq!(diags.len(), 1, "Expected 1 diagnostic, got: {:?}", diags);
        assert!(
            diags[0].message.contains("NoteReal"),
            "Diagnostic should mention 'NoteReal', got: {}",
            diags[0].message,
        );
        assert_eq!(
            diags[0].severity,
            Some(DiagnosticSeverity::WARNING),
            "Unknown-class subject should be WARNING severity",
        );
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(UNKNOWN_MEMBER_CODE.to_string())),
            "Unknown-class subject should use unknown_member code",
        );
    }

    /// Function whose return type is an unknown class — member access
    /// on the result should produce a warning.
    #[test]
    fn flags_member_access_on_unknown_return_type_function() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
/** @return Nope */
function getUnknown(): Nope {}
function test(): void {
    getUnknown()->doStuff();
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.iter().any(|d| d.message.contains("Nope")),
            "Expected a diagnostic mentioning 'Nope', got: {:?}",
            diags,
        );
    }

    /// When the parameter type is `mixed`, no unknown-class diagnostic
    /// should fire (mixed is not a class name).
    #[test]
    fn no_unknown_class_diagnostic_for_mixed_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function helper(mixed $value): void {
    $value->something;
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for mixed parameter member access, got: {:?}",
            diags,
        );
    }

    // ── Bug #1: type alias array shape object values ────────────────────

    /// When a method returns a `@phpstan-type` alias that expands to an
    /// array shape containing object values, accessing a method on the
    /// object value should NOT produce a diagnostic.
    #[test]
    fn no_diagnostic_for_type_alias_array_shape_object_value() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
}

/**
 * @phpstan-type UserData array{name: string, email: string, pen: Pen}
 */
class TypeAliasDemo {
    /** @return UserData */
    public function getUserData(): array {
        return ['name' => 'Alice', 'email' => 'alice@example.com', 'pen' => new Pen()];
    }

    public function demo(): void {
        $data = $this->getUserData();
        $data['pen']->write();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for type alias array shape object value method, got: {:?}",
            diags,
        );
    }

    /// Same as above but with multiple type aliases and nested object
    /// values in the shapes.
    #[test]
    fn no_diagnostic_for_multiple_type_alias_object_values() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class User {
    public function getEmail(): string { return ''; }
}

class Pen {
    public function write(): void {}
}

/**
 * @phpstan-type UserData array{name: string, pen: Pen}
 * @phpstan-type StatusInfo array{code: int, owner: User}
 */
class Demo {
    /** @return UserData */
    public function getUserData(): array { return []; }

    /** @return StatusInfo */
    public function getStatus(): array { return []; }

    public function demo(): void {
        $data = $this->getUserData();
        $data['pen']->write();

        $status = $this->getStatus();
        $status['owner']->getEmail();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for type alias object values, got: {:?}",
            diags,
        );
    }

    // ── Bug #2: inline array-element function calls ─────────────────────

    /// When an array-element function like `end()` is called inline as
    /// the subject of a member access, the diagnostic should resolve the
    /// element type from the array argument's generic annotation, not
    /// fall back to the native `mixed|false` return type.
    #[test]
    fn no_diagnostic_for_inline_array_element_function_call() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
}

class Container {
    /** @var array<int, Pen> */
    public array $members = [];
}

function demo(): void {
    $src = new Container();
    end($src->members)->write();
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for end() inline call with generic array, got: {:?}",
            diags,
        );
    }

    // ── Bug #3: Builder scope chain (pre-resolved base class check) ─────

    /// When `resolve_target_classes` returns a fully-resolved class that
    /// already has the member (e.g. Builder with injected scope methods),
    /// the diagnostic should check the pre-resolved class first before
    /// re-resolving through the cache.  This tests the quick-check path
    /// that prevents cache staleness from causing false positives.
    #[test]
    fn no_diagnostic_when_member_exists_on_pre_resolved_base_class() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): void {}
    public function erase(): void {}
}

class Demo {
    /** @return Pen */
    public function getPen(): Pen { return new Pen(); }

    public function demo(): void {
        $this->getPen()->write();
        $this->getPen()->erase();
    }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when member exists on resolved class, got: {:?}",
            diags,
        );
    }

    /// `@see ClassName::method()` in a docblock should not produce an
    /// unknown-member diagnostic.  The symbol map emits these with
    /// `is_method_call: false` and `is_static: true`, but the relaxed
    /// check must still find the instance method.
    #[test]
    fn no_diagnostic_for_see_tag_method_reference() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class User {
    public function name(): string { return 'Alice'; }
}

/**
 * @see User::name()
 */
class Helper {
    public function greet(): string { return 'hi'; }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "Expected no diagnostics for @see User::name(), got: {:?}",
            diags,
        );
    }

    /// `@see ClassName::CONSTANT` should not produce a false positive either.
    #[test]
    fn no_diagnostic_for_see_tag_constant_reference() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Config {
    public const VERSION = '1.0';
}

/**
 * @see Config::VERSION
 */
class App {}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "Expected no diagnostics for @see Config::VERSION, got: {:?}",
            diags,
        );
    }

    /// Inline `{@see ClassName::method()}` should also be clean.
    #[test]
    fn no_diagnostic_for_inline_see_tag_method_reference() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class User {
    public function name(): string { return 'Alice'; }
}

class Helper {
    /**
     * Wraps {@see User::name()} output.
     */
    public function greet(): string { return 'hi'; }
}
"#;
        let diags = collect(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "Expected no diagnostics for inline {{@see User::name()}}, got: {:?}",
            diags,
        );
    }
}
