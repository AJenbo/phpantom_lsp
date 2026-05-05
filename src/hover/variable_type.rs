//! Variable type resolution for hover.
//!
//! The entry point is [`resolve_variable_type`], which delegates to the
//! unified `resolve_variable_php_type` pipeline in
//! `completion/variable/resolution.rs`.

use std::sync::Arc;

use crate::completion::resolver::Loaders;
use crate::php_type::PhpType;
use crate::types::ClassInfo;

/// Resolve the type for a variable at `cursor_offset` for hover.
///
/// Delegates to the unified [`resolve_variable_php_type`] pipeline
/// which handles `@var` overrides, parameter types, foreach bindings,
/// catch variables, and assignment types via the forward walker.
pub(crate) fn resolve_variable_type(
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: Option<&ClassInfo>,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    loaders: Loaders<'_>,
) -> Option<PhpType> {
    crate::completion::variable::resolution::resolve_variable_php_type(
        var_name,
        content,
        cursor_offset,
        current_class,
        all_classes,
        class_loader,
        loaders,
    )
}
