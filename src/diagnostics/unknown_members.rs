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

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::variable::resolution::resolve_variable_types;
use crate::symbol_map::SymbolKind;
use crate::types::ClassInfo;
use crate::virtual_members::resolve_class_fully_cached;

use super::offset_range_to_lsp_range;

/// Diagnostic code used for unknown-member diagnostics so that code
/// actions can match on it.
pub(crate) const UNKNOWN_MEMBER_CODE: &str = "unknown_member";

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

        let local_classes: Vec<ClassInfo> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();

        let class_loader = self.class_loader_with(&local_classes, &file_use_map, &file_namespace);
        let function_loader = self.function_loader_with(&file_use_map, &file_namespace);
        let cache = &self.resolved_class_cache;

        // ── Walk every symbol span ──────────────────────────────────────
        for span in &symbol_map.spans {
            let (subject_text, member_name, is_static, is_method_call) = match &span.kind {
                SymbolKind::MemberAccess {
                    subject_text,
                    member_name,
                    is_static,
                    is_method_call,
                } => (subject_text, member_name, *is_static, *is_method_call),
                _ => continue,
            };

            // ── Skip the magic `::class` constant ───────────────────────
            if member_name == "class" && is_static {
                continue;
            }

            // ── Resolve the subject to one or more ClassInfo values ─────
            // For union types (e.g. `Lamp|Faucet`) we get multiple entries.
            // The member only needs to exist on ANY branch to suppress the
            // diagnostic — flagging branch-specific members is PHPStan's
            // job, not ours.
            let resolve_ctx = SubjectResolutionCtx {
                file_use_map: &file_use_map,
                file_namespace: &file_namespace,
                local_classes: &local_classes,
                backend: self,
                content,
                class_loader: &class_loader,
                function_loader: &function_loader,
            };
            let base_classes: Vec<ClassInfo> =
                resolve_subject_to_classes(subject_text, is_static, span.start, &resolve_ctx);

            // Can't resolve subject at all — skip (no false positives).
            if base_classes.is_empty() {
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
            if resolved_classes
                .iter()
                .any(|c| member_exists(c, member_name, is_static, is_method_call))
            {
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

            out.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String(UNKNOWN_MEMBER_CODE.to_string())),
                code_description: None,
                source: Some("phpantom".to_string()),
                message,
                related_information: None,
                tags: None,
                data: None,
            });
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

/// Context for subject resolution, bundling the per-file state that
/// would otherwise require many function arguments.
struct SubjectResolutionCtx<'a> {
    file_use_map: &'a HashMap<String, String>,
    file_namespace: &'a Option<String>,
    local_classes: &'a [ClassInfo],
    backend: &'a Backend,
    content: &'a str,
    class_loader: &'a dyn Fn(&str) -> Option<ClassInfo>,
    function_loader: &'a dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
}

/// Resolve a member access subject to all possible `ClassInfo` values.
///
/// For non-variable subjects (`self`, `static`, `parent`, `ClassName`)
/// this returns zero or one entries.  For `$variable` subjects it returns
/// all branches of the union type (e.g. `Lamp|Faucet` → two entries).
fn resolve_subject_to_classes(
    subject_text: &str,
    is_static: bool,
    access_offset: u32,
    ctx: &SubjectResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let trimmed = subject_text.trim();

    match trimmed {
        "self" | "static" | "$this" => {
            resolve_enclosing_class(ctx.local_classes, ctx.file_namespace, access_offset, ctx)
        }
        "parent" => {
            // Find the innermost enclosing class that has a parent.
            let cls =
                find_innermost_enclosing_class(ctx.local_classes, access_offset).or_else(|| {
                    ctx.local_classes
                        .iter()
                        .find(|c| !c.name.starts_with("__anonymous@"))
                });
            let parent_name = match cls.and_then(|c| c.parent_class.as_ref()) {
                Some(p) => p,
                None => return Vec::new(),
            };
            let fqn = resolve_to_fqn(parent_name, ctx.file_use_map, ctx.file_namespace);
            ctx.backend.find_or_load_class(&fqn).into_iter().collect()
        }
        _ if is_static && !trimmed.starts_with('$') => {
            let fqn = resolve_to_fqn(trimmed, ctx.file_use_map, ctx.file_namespace);
            ctx.backend.find_or_load_class(&fqn).into_iter().collect()
        }
        _ if trimmed.starts_with('$') => {
            // Variable subject — resolve to ALL union branches.
            resolve_variable_subjects(
                subject_text,
                access_offset,
                ctx.content,
                ctx.local_classes,
                ctx.class_loader,
                ctx.function_loader,
            )
        }
        _ => Vec::new(),
    }
}

/// Resolve a `$variable` subject to all possible `ClassInfo` values
/// using the full variable type resolution pipeline.
///
/// Returns all branches of the union type rather than just the first,
/// so that diagnostics can check the member against every possibility.
fn resolve_variable_subjects(
    subject_text: &str,
    access_offset: u32,
    content: &str,
    local_classes: &[ClassInfo],
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
) -> Vec<ClassInfo> {
    let var_name = subject_text.trim();

    // Find the innermost enclosing class based on offset ranges.
    // Include anonymous classes so that `$var` inside `new class { … }`
    // resolves against the anonymous class, not the outer named class.
    let enclosing_class = find_innermost_enclosing_class(local_classes, access_offset)
        .cloned()
        .unwrap_or_default();

    resolve_variable_types(
        var_name,
        &enclosing_class,
        local_classes,
        content,
        access_offset,
        class_loader,
        Some(function_loader),
    )
}

/// Find the innermost class whose body span contains `offset`.
///
/// Returns a reference to the `ClassInfo` with the smallest span that
/// encloses `offset`, including anonymous classes.  This is the single
/// source of truth for "which class body am I in?" used by both
/// `$this`/`self`/`static` resolution and variable subject resolution.
fn find_innermost_enclosing_class(local_classes: &[ClassInfo], offset: u32) -> Option<&ClassInfo> {
    local_classes
        .iter()
        .filter(|c| offset >= c.start_offset && offset <= c.end_offset)
        .min_by_key(|c| c.end_offset.saturating_sub(c.start_offset))
}

/// Resolve `$this` / `self` / `static` to the enclosing class.
///
/// For anonymous classes the `ClassInfo` is returned directly from
/// `local_classes` (they are never stored in the cross-file index).
/// For named classes the FQN is constructed and loaded via
/// `find_or_load_class` so that cross-file inheritance is resolved.
fn resolve_enclosing_class(
    local_classes: &[ClassInfo],
    file_namespace: &Option<String>,
    offset: u32,
    ctx: &SubjectResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let cls = match find_innermost_enclosing_class(local_classes, offset)
        // Fallback: first non-anonymous class (top-level code).
        .or_else(|| {
            local_classes
                .iter()
                .find(|c| !c.name.starts_with("__anonymous@"))
        }) {
        Some(c) => c,
        None => return Vec::new(),
    };

    // Anonymous classes are file-local — return the local ClassInfo
    // directly since they are not registered in the cross-file index.
    if cls.name.starts_with("__anonymous@") {
        return vec![cls.clone()];
    }

    let fqn = if let Some(ns) = file_namespace {
        format!("{}\\{}", ns, cls.name)
    } else {
        cls.name.clone()
    };
    ctx.backend.find_or_load_class(&fqn).into_iter().collect()
}

/// Resolve an unqualified/qualified class name to a fully-qualified name
/// using the use map and namespace context.
fn resolve_to_fqn(
    name: &str,
    use_map: &HashMap<String, String>,
    namespace: &Option<String>,
) -> String {
    if let Some(stripped) = name.strip_prefix('\\') {
        return stripped.to_string();
    }

    if !name.contains('\\') {
        if let Some(fqn) = use_map.get(name) {
            return fqn.clone();
        }
        if let Some(ns) = namespace {
            return format!("{}\\{}", ns, name);
        }
        return name.to_string();
    }

    let first_segment = name.split('\\').next().unwrap_or(name);
    if let Some(fqn_prefix) = use_map.get(first_segment) {
        let rest = &name[first_segment.len()..];
        return format!("{}{}", fqn_prefix, rest);
    }
    if let Some(ns) = namespace {
        return format!("{}\\{}", ns, name);
    }
    name.to_string()
}

/// Return a user-friendly display name for a class.
///
/// Prefers the short name for readability. For anonymous classes, returns
/// the full internal name.
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
}
