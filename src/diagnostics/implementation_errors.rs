//! Implementation error diagnostic.
//!
//! Flags concrete classes that fail to implement all required methods
//! from their interfaces or abstract parents.  Reuses the same
//! missing-method detection logic as the "Implement missing methods"
//! code action (`code_actions::implement_methods::collect_missing_methods`).

use std::collections::HashSet;
use std::sync::Arc;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::code_actions::implement_methods::collect_missing_methods;
use crate::symbol_map::SymbolKind;
use crate::types::ClassLikeKind;

impl Backend {
    /// Collect implementation-error diagnostics for a single file.
    ///
    /// For each concrete (non-abstract) class in the file, checks whether
    /// all required methods from interfaces and abstract parents are
    /// implemented.  Emits an Error-severity diagnostic on the class name
    /// span for each class that has missing methods.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing or returning them.
    pub fn collect_implementation_error_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let ctx = self.file_context(uri);
        let class_loader = self.class_loader(&ctx);

        // Iterate all ClassDeclaration spans in the symbol map.
        for span in &symbol_map.spans {
            let class_name = match &span.kind {
                SymbolKind::ClassDeclaration { name } => name,
                _ => continue,
            };

            // Find the matching ClassInfo in the ast_map.
            let class_info = match ctx
                .classes
                .iter()
                .find(|c| c.name == *class_name || self.class_fqn_matches(c, class_name, &ctx))
            {
                Some(c) => Arc::clone(c),
                None => continue,
            };

            // Only concrete classes and enums can have implementation errors.
            // Abstract classes, interfaces, and traits are skipped.
            let is_concrete_class =
                class_info.kind == ClassLikeKind::Class && !class_info.is_abstract;
            let is_enum = class_info.kind == ClassLikeKind::Enum;
            if !is_concrete_class && !is_enum {
                continue;
            }

            // Skip classes with no interfaces and no parent class — they
            // cannot have missing method implementations.
            if class_info.interfaces.is_empty() && class_info.parent_class.is_none() {
                continue;
            }

            let missing = collect_missing_methods(&class_info, &class_loader);

            if missing.is_empty() {
                continue;
            }

            // Build the diagnostic range from the class name span.
            let range = match self.offset_range_to_lsp_range(
                uri,
                content,
                span.start as usize,
                span.end as usize,
            ) {
                Some(r) => r,
                None => continue,
            };

            // Build a single diagnostic listing all missing methods.
            let kind_label = if class_info.kind == ClassLikeKind::Enum {
                "Enum"
            } else {
                "Class"
            };

            let message = if missing.len() == 1 {
                let m = &missing[0];
                let source = method_source_description(&class_info, &m.name, &class_loader);
                format!(
                    "{} '{}' must implement method '{}()' from {}",
                    kind_label, class_info.name, m.name, source
                )
            } else {
                let method_list: Vec<String> = missing
                    .iter()
                    .map(|m| {
                        let source = method_source_description(&class_info, &m.name, &class_loader);
                        format!("'{}()' from {}", m.name, source)
                    })
                    .collect();
                format!(
                    "{} '{}' must implement {} methods: {}",
                    kind_label,
                    class_info.name,
                    missing.len(),
                    method_list.join(", ")
                )
            };

            out.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("missing_implementation".to_string())),
                code_description: None,
                source: Some("phpantom".to_string()),
                message,
                related_information: None,
                tags: None,
                data: None,
            });
        }
    }

    /// Check if a ClassInfo's fully-qualified name matches the given name.
    ///
    /// The symbol map stores the short class name, but classes in the
    /// ast_map may have their FQN stored differently.  This handles the
    /// common case where the class name is unqualified.
    fn class_fqn_matches(
        &self,
        class: &crate::types::ClassInfo,
        name: &str,
        ctx: &crate::types::FileContext,
    ) -> bool {
        // Build FQN from namespace + class name and compare.
        if let Some(ref ns) = ctx.namespace {
            let fqn = format!("{}\\{}", ns, class.name);
            fqn == name || class.name == name
        } else {
            class.name == name
        }
    }
}

/// Describe where a missing method was required from (interface or
/// abstract parent class).
fn method_source_description(
    class: &crate::types::ClassInfo,
    method_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<crate::types::ClassInfo>>,
) -> String {
    // Check interfaces first.
    for iface_name in &class.interfaces {
        if let Some(iface) = class_loader(iface_name)
            && has_method_in_chain(&iface, method_name, class_loader, &mut HashSet::new())
        {
            return format!("interface '{}'", iface_name);
        }
    }

    // Check parent chain for abstract methods.
    if let Some(ref parent_name) = class.parent_class
        && let Some(parent) = class_loader(parent_name)
        && has_abstract_method_in_chain(&parent, method_name, class_loader, &mut HashSet::new())
    {
        return format!("class '{}'", parent_name);
    }

    // Fallback — shouldn't happen if collect_missing_methods found it.
    "its hierarchy".to_string()
}

/// Check if a class or its parent chain declares a method (abstract or not).
fn has_method_in_chain(
    class: &crate::types::ClassInfo,
    method_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<crate::types::ClassInfo>>,
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(class.name.to_string()) {
        return false;
    }

    let lower = method_name.to_lowercase();
    if class.methods.iter().any(|m| m.name.to_lowercase() == lower) {
        return true;
    }

    // Check parent interfaces.
    for iface_name in &class.interfaces {
        if let Some(iface) = class_loader(iface_name)
            && has_method_in_chain(&iface, method_name, class_loader, visited)
        {
            return true;
        }
    }

    // Check parent class.
    if let Some(ref parent_name) = class.parent_class
        && let Some(parent) = class_loader(parent_name)
        && has_method_in_chain(&parent, method_name, class_loader, visited)
    {
        return true;
    }

    false
}

/// Check if a class or its parent chain declares an abstract method.
fn has_abstract_method_in_chain(
    class: &crate::types::ClassInfo,
    method_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<crate::types::ClassInfo>>,
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(class.name.to_string()) {
        return false;
    }

    let lower = method_name.to_lowercase();
    if class
        .methods
        .iter()
        .any(|m| m.name.to_lowercase() == lower && m.is_abstract)
    {
        return true;
    }

    if let Some(ref parent_name) = class.parent_class
        && let Some(parent) = class_loader(parent_name)
        && has_abstract_method_in_chain(&parent, method_name, class_loader, visited)
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::Backend;
    use tower_lsp::lsp_types::*;

    fn collect(php: &str) -> Vec<Diagnostic> {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        backend.update_ast(uri, &Arc::new(php.to_string()));
        let mut out = Vec::new();
        backend.collect_implementation_error_diagnostics(uri, php, &mut out);
        out
    }

    #[test]
    fn no_diagnostic_for_abstract_class() {
        let php = r#"<?php
interface Foo { public function bar(): void; }
abstract class Baz implements Foo {}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Abstract classes should not get diagnostics"
        );
    }

    #[test]
    fn no_diagnostic_for_interface() {
        let php = r#"<?php
interface Foo { public function bar(): void; }
interface Baz extends Foo { public function qux(): void; }
"#;
        let diags = collect(php);
        assert!(diags.is_empty(), "Interfaces should not get diagnostics");
    }

    #[test]
    fn no_diagnostic_when_all_methods_implemented() {
        let php = r#"<?php
interface Foo { public function bar(): void; }
class Baz implements Foo {
    public function bar(): void {}
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Fully implemented class should have no diagnostics"
        );
    }

    #[test]
    fn diagnostic_for_missing_interface_method() {
        let php = r#"<?php
interface Foo {
    public function bar(): void;
}
class Baz implements Foo {
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Baz"));
        assert!(diags[0].message.contains("bar()"));
        assert!(diags[0].message.contains("interface"));
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn diagnostic_for_missing_abstract_method() {
        let php = r#"<?php
abstract class Base {
    abstract public function doSomething(): void;
}
class Child extends Base {
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Child"));
        assert!(diags[0].message.contains("doSomething()"));
        assert!(diags[0].message.contains("class"));
    }

    #[test]
    fn diagnostic_lists_multiple_missing_methods() {
        let php = r#"<?php
interface Foo {
    public function bar(): void;
    public function baz(): void;
    public function qux(): void;
}
class Impl implements Foo {
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("3 methods"));
        assert!(diags[0].message.contains("bar()"));
        assert!(diags[0].message.contains("baz()"));
        assert!(diags[0].message.contains("qux()"));
    }

    #[test]
    fn no_diagnostic_for_plain_class_without_interfaces() {
        let php = r#"<?php
class Simple {
    public function foo(): void {}
}
"#;
        let diags = collect(php);
        assert!(diags.is_empty());
    }

    #[test]
    fn diagnostic_has_correct_code_and_source() {
        let php = r#"<?php
interface Foo { public function bar(): void; }
class Baz implements Foo {}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("missing_implementation".to_string()))
        );
        assert_eq!(diags[0].source, Some("phpantom".to_string()));
    }

    #[test]
    fn no_diagnostic_for_trait() {
        let php = r#"<?php
trait MyTrait {
    abstract public function doIt(): void;
}
"#;
        let diags = collect(php);
        assert!(diags.is_empty(), "Traits should not get diagnostics");
    }

    #[test]
    fn no_diagnostic_for_enum_with_all_methods_implemented() {
        let php = r#"<?php
interface HasLabel { public function label(): string; }
enum Color implements HasLabel {
    case Red;
    case Blue;

    public function label(): string {
        return $this->name;
    }
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Enum with implemented methods should have no diagnostics"
        );
    }

    #[test]
    fn diagnostic_for_enum_missing_interface_method() {
        let php = r#"<?php
interface HasLabel { public function label(): string; }
enum Color implements HasLabel {
    case Red;
    case Blue;
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Enum"));
        assert!(diags[0].message.contains("Color"));
        assert!(diags[0].message.contains("label()"));
        assert!(diags[0].message.contains("interface"));
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn no_diagnostic_for_enum_without_interfaces() {
        let php = r#"<?php
enum Suit {
    case Hearts;
    case Diamonds;
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Enum without interfaces should have no diagnostics"
        );
    }

    #[test]
    fn enum_multiple_missing_methods() {
        let php = r#"<?php
interface HasLabel {
    public function label(): string;
    public function description(): string;
}
enum Color implements HasLabel {
    case Red;
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Enum"));
        assert!(diags[0].message.contains("2 methods"));
        assert!(diags[0].message.contains("label()"));
        assert!(diags[0].message.contains("description()"));
    }

    #[test]
    fn case_insensitive_method_matching() {
        let php = r#"<?php
interface Foo { public function doSomething(): void; }
class Bar implements Foo {
    public function DOSOMETHING(): void {}
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Method matching should be case-insensitive"
        );
    }

    #[test]
    fn parent_implements_interface_method() {
        let php = r#"<?php
interface Foo { public function bar(): void; }
class Base implements Foo {
    public function bar(): void {}
}
class Child extends Base {}
"#;
        let diags = collect(php);
        // Child doesn't declare implements Foo, so no check needed.
        // But even if it did, bar() is inherited from Base.
        assert!(diags.is_empty());
    }

    #[test]
    fn trait_satisfies_interface_method() {
        let php = r#"<?php
interface Wireable {
    public function toLivewire(): array;
    public function fromLivewire($value): static;
}

trait WireableData {
    public function toLivewire(): array { return []; }
    public static function fromLivewire($value): static { return new static(); }
}

class MyData implements Wireable {
    use WireableData;
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Trait methods should satisfy interface requirements, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn trait_satisfies_abstract_parent_method() {
        let php = r#"<?php
abstract class Base {
    abstract public function doSomething(): void;
}

trait DoesIt {
    public function doSomething(): void {}
}

class Child extends Base {
    use DoesIt;
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Trait methods should satisfy abstract parent requirements, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn nested_trait_satisfies_interface() {
        let php = r#"<?php
interface HasLabel {
    public function label(): string;
}

trait InnerTrait {
    public function label(): string { return 'hi'; }
}

trait OuterTrait {
    use InnerTrait;
}

class Widget implements HasLabel {
    use OuterTrait;
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Nested trait methods should satisfy interface requirements, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn parent_trait_satisfies_interface() {
        let php = r#"<?php
interface Serializable {
    public function toArray(): array;
    public function toJson(): string;
}

trait SerializableTrait {
    public function toArray(): array { return []; }
    public function toJson(): string { return '{}'; }
}

class Base {
    use SerializableTrait;
}

class Child extends Base implements Serializable {
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Parent class trait methods should satisfy child interface requirements, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn trait_with_abstract_method_does_not_satisfy() {
        let php = r#"<?php
interface Foo {
    public function bar(): void;
}

trait HalfImpl {
    abstract public function bar(): void;
}

class Baz implements Foo {
    use HalfImpl;
}
"#;
        let diags = collect(php);
        assert_eq!(
            diags.len(),
            1,
            "Abstract trait methods should not satisfy interface requirements"
        );
        assert!(diags[0].message.contains("bar()"));
    }

    #[test]
    fn cyclic_interface_hierarchy_does_not_stack_overflow() {
        // interface A extends B, interface B extends A — user error, but
        // should not crash the LSP server.
        let php = r#"<?php
interface A extends B { public function foo(): void; }
interface B extends A { public function bar(): void; }
class C implements A {
    public function foo(): void {}
    public function bar(): void {}
}
"#;
        let diags = collect(php);
        // We only care that it doesn't hang or crash.  Whether a
        // diagnostic is emitted is secondary.
        let _ = diags;
    }

    #[test]
    fn cyclic_parent_class_does_not_stack_overflow() {
        // class A extends B, class B extends A — user error.
        let php = r#"<?php
interface I { public function work(): void; }
class A extends B implements I {}
class B extends A {}
"#;
        let diags = collect(php);
        let _ = diags;
    }

    #[test]
    fn diagnostic_range_covers_class_name() {
        let php = r#"<?php
interface Foo { public function bar(): void; }
class MyClass implements Foo {}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1);
        let range = diags[0].range;
        // The range should cover "MyClass" — verify it is on the correct line.
        let class_line = php[..php.find("MyClass").unwrap()]
            .chars()
            .filter(|&c| c == '\n')
            .count() as u32;
        assert_eq!(range.start.line, class_line);
    }

    #[test]
    fn no_diagnostic_for_backed_enum_implicit_interface() {
        // PHP backed enums automatically implement BackedEnum (which
        // extends UnitEnum).  The parser adds these as implicit
        // interfaces, but the implementation checker must skip them
        // because PHP provides from(), tryFrom(), and cases()
        // automatically at runtime.
        let php = r#"<?php
interface UnitEnum {
    public static function cases(): array;
}
interface BackedEnum extends UnitEnum {
    public static function from(int|string $value): static;
    public static function tryFrom(int|string $value): ?static;
}

enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Backed enum should not require explicit BackedEnum/UnitEnum implementations, got: {diags:?}"
        );
    }

    #[test]
    fn no_diagnostic_for_unit_enum_implicit_interface() {
        // PHP enums without a backing type automatically implement
        // UnitEnum.  Same as above — cases() is provided by PHP.
        let php = r#"<?php
interface UnitEnum {
    public static function cases(): array;
}

enum Suit {
    case Hearts;
    case Diamonds;
}
"#;
        let diags = collect(php);
        assert!(
            diags.is_empty(),
            "Unit enum should not require explicit UnitEnum implementation, got: {diags:?}"
        );
    }

    #[test]
    fn diagnostic_for_enum_missing_own_interface_method() {
        // Enums that explicitly implement a user-defined interface
        // must still satisfy those methods — only the implicit
        // BackedEnum/UnitEnum interfaces are exempt.
        let php = r#"<?php
interface Labelable {
    public function label(): string;
}

enum Color: string implements Labelable {
    case Red = 'red';
}
"#;
        let diags = collect(php);
        assert_eq!(
            diags.len(),
            1,
            "Enum should still flag missing methods from user-defined interfaces, got: {diags:?}"
        );
        assert!(
            diags[0].message.contains("label"),
            "Diagnostic should mention the missing 'label' method"
        );
    }
}
