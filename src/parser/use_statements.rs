/// `use` statement and namespace extraction.
///
/// This module handles parsing PHP `use` statements and namespace
/// declarations from the AST, building a mapping of short (imported)
/// names to their fully-qualified equivalents.
use std::collections::HashMap;

use mago_syntax::ast::*;

use crate::Backend;
use crate::util::short_name;

impl Backend {
    /// Walk statements and extract `use` statement mappings.
    pub(crate) fn extract_use_statements_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
        use_map: &mut HashMap<String, String>,
    ) {
        for statement in statements {
            match statement {
                Statement::Use(use_stmt) => {
                    Self::extract_use_items(&use_stmt.items, use_map);
                }
                Statement::Namespace(namespace) => {
                    // Recurse into namespace bodies to find use statements
                    Self::extract_use_statements_from_statements(
                        namespace.statements().iter(),
                        use_map,
                    );
                }
                _ => {}
            }
        }
    }

    /// Extract individual use items from a `UseItems` node.
    pub(crate) fn extract_use_items(items: &UseItems, use_map: &mut HashMap<String, String>) {
        match items {
            UseItems::Sequence(seq) => {
                // `use Foo\Bar;` or `use Foo\Bar, Baz\Qux;`
                for item in seq.items.iter() {
                    Self::register_use_item(item, None, use_map);
                }
            }
            UseItems::TypedSequence(seq) => {
                // `use function Foo\bar;` or `use const Foo\BAR;`
                // We only care about class imports, skip function/const
                if seq.r#type.is_function() || seq.r#type.is_const() {
                    return;
                }
                for item in seq.items.iter() {
                    Self::register_use_item(item, None, use_map);
                }
            }
            UseItems::TypedList(list) => {
                // `use function Foo\{bar, baz};` — skip function/const
                if list.r#type.is_function() || list.r#type.is_const() {
                    return;
                }
                let prefix = list.namespace.value();
                for item in list.items.iter() {
                    Self::register_use_item(item, Some(prefix), use_map);
                }
            }
            UseItems::MixedList(list) => {
                // `use Foo\{Bar, function baz, const QUX};`
                let prefix = list.namespace.value();
                for maybe_typed in list.items.iter() {
                    // Skip function/const imports
                    if let Some(ref t) = maybe_typed.r#type
                        && (t.is_function() || t.is_const())
                    {
                        continue;
                    }
                    Self::register_use_item(&maybe_typed.item, Some(prefix), use_map);
                }
            }
        }
    }

    /// Register a single `UseItem` into the use_map.
    ///
    /// If `group_prefix` is `Some`, the item name is relative to that prefix
    /// (e.g. for `use Foo\{Bar}`, prefix is `"Foo"` and item name is `"Bar"`,
    /// giving FQN `"Foo\Bar"`).
    fn register_use_item(
        item: &UseItem,
        group_prefix: Option<&str>,
        use_map: &mut HashMap<String, String>,
    ) {
        let item_name = item.name.value();

        // Build the fully-qualified name
        let fqn = if let Some(prefix) = group_prefix {
            format!("{}\\{}", prefix, item_name)
        } else {
            item_name.to_string()
        };

        // The short (imported) name is either the alias or the last segment
        let alias_name = if let Some(ref alias) = item.alias {
            alias.identifier.value.to_string()
        } else {
            // Last segment of the FQN
            short_name(&fqn).to_string()
        };

        use_map.insert(alias_name, fqn);
    }

    /// Walk statements and extract the first namespace declaration found.
    pub(crate) fn extract_namespace_from_statements<'a>(
        statements: impl Iterator<Item = &'a Statement<'a>>,
    ) -> Option<String> {
        for statement in statements {
            if let Statement::Namespace(namespace) = statement {
                // The namespace name is an `Option<Identifier>`.
                // Both implicit (`namespace Foo;`) and brace-delimited
                // (`namespace Foo { ... }`) forms may have a name.
                if let Some(ident) = &namespace.name {
                    let name = ident.value();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
        None
    }
}
