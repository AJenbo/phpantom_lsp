/// Go-to-type-definition resolution (`textDocument/typeDefinition`).
///
/// "Go to Type Definition" jumps from a variable or expression to the
/// class/interface/trait/enum declaration of its resolved type, rather
/// than to the definition site (assignment, parameter, etc.).
///
/// For example, if `$user` is typed as `User`, go-to-definition jumps
/// to the `$user = ...` assignment, while go-to-type-definition jumps
/// to the `class User { … }` declaration.
///
/// The implementation reuses the existing variable type resolution and
/// subject resolution pipelines, then looks up each resolved class name
/// via [`resolve_class_reference`](super::resolve) to find its
/// definition location.
use std::sync::Arc;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::resolver::{Loaders, ResolutionCtx};
use crate::hover::variable_type;
use crate::php_type::PhpType;
use crate::symbol_map::{SelfStaticParentKind, SymbolKind};
use crate::types::*;
use crate::util::find_class_at_offset;

impl Backend {
    /// Handle a "go to type definition" request.
    ///
    /// Returns a list of `Location`s pointing to the class declarations
    /// of the resolved type(s) for the symbol under the cursor. For
    /// union types, multiple locations are returned (one per class).
    /// Scalar types (`int`, `string`, `array`, etc.) are skipped since
    /// they have no user-navigable declaration.
    pub(crate) fn resolve_type_definition(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<Vec<Location>> {
        // Look up the symbol at the cursor position (retries one byte
        // earlier for end-of-token edge cases).
        let symbol = self.lookup_symbol_at_position(uri, content, position)?;
        let offset = symbol.start;

        let ctx = self.file_context(uri);
        let current_class = find_class_at_offset(&ctx.classes, offset);
        let class_loader = self.class_loader(&ctx);
        let function_loader = self.function_loader(&ctx);

        let type_names: Vec<String> = match &symbol.kind {
            SymbolKind::Variable { name } => resolve_variable_type_names(
                name,
                content,
                offset,
                current_class,
                &ctx,
                &class_loader,
                &function_loader,
            ),

            SymbolKind::MemberAccess {
                subject_text,
                member_name,
                is_static,
                is_method_call,
                ..
            } => {
                let access_kind = if *is_static {
                    AccessKind::DoubleColon
                } else {
                    AccessKind::Arrow
                };

                let rctx = ResolutionCtx {
                    current_class,
                    all_classes: &ctx.classes,
                    content,
                    cursor_offset: offset,
                    class_loader: &class_loader,
                    resolved_class_cache: Some(&self.resolved_class_cache),
                    function_loader: Some(
                        &function_loader as &dyn Fn(&str) -> Option<FunctionInfo>,
                    ),
                };

                let candidates = ResolvedType::into_arced_classes(
                    crate::completion::resolver::resolve_target_classes(
                        subject_text,
                        access_kind,
                        &rctx,
                    ),
                );

                // Resolve the member's return type / property type.
                self.resolve_member_type_names(
                    &candidates,
                    member_name,
                    *is_method_call,
                    &class_loader,
                )
            }

            SymbolKind::SelfStaticParent(ssp_kind) => match ssp_kind {
                SelfStaticParentKind::Self_
                | SelfStaticParentKind::Static
                | SelfStaticParentKind::This => current_class
                    .map(|cc| vec![cc.name.clone()])
                    .unwrap_or_default(),
                SelfStaticParentKind::Parent => current_class
                    .and_then(|cc| cc.parent_class.as_ref())
                    .map(|p| vec![p.clone()])
                    .unwrap_or_default(),
            },

            SymbolKind::ClassReference { name, .. } => {
                // The type *is* the class itself.
                vec![name.clone()]
            }

            SymbolKind::FunctionCall { name, .. } => {
                self.resolve_function_return_type_names(name, &ctx, &function_loader, symbol.start)
            }

            SymbolKind::ClassDeclaration { .. }
            | SymbolKind::MemberDeclaration { .. }
            | SymbolKind::ConstantReference { .. } => {
                // No meaningful type definition target for these.
                Vec::new()
            }
        };

        if type_names.is_empty() {
            return None;
        }

        let locations = self.resolve_type_names_to_locations(uri, content, &type_names, offset);

        if locations.is_empty() {
            None
        } else {
            Some(locations)
        }
    }

    /// Resolve the type of a member (method return type or property type)
    /// to a list of class names.
    fn resolve_member_type_names(
        &self,
        candidates: &[Arc<ClassInfo>],
        member_name: &str,
        is_method_call: bool,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> Vec<String> {
        for target_class in candidates {
            let merged = crate::virtual_members::resolve_class_fully_cached(
                target_class,
                class_loader,
                &self.resolved_class_cache,
            );

            if is_method_call {
                if let Some(method) = merged
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(member_name))
                {
                    let default_type = PhpType::Named(String::new());
                    let ret_type = method.return_type.as_ref().unwrap_or(&default_type);

                    // Replace self/static/$this with the owning class name.
                    let resolved = ret_type.replace_self(&merged.name);

                    let names = resolved.top_level_class_names();
                    if !names.is_empty() {
                        return names;
                    }
                }
            } else {
                // Property access — resolve the property type.
                if let Some(prop) = merged.properties.iter().find(|p| p.name == member_name) {
                    let default_type = PhpType::Named(String::new());
                    let prop_type = prop.type_hint.as_ref().unwrap_or(&default_type);

                    let resolved = prop_type.replace_self(&merged.name);

                    let names = resolved.top_level_class_names();
                    if !names.is_empty() {
                        return names;
                    }
                }

                // Constants.
                if let Some(constant) = merged.constants.iter().find(|c| c.name == member_name) {
                    let default_type = PhpType::Named(String::new());
                    let const_type = constant.type_hint.as_ref().unwrap_or(&default_type);

                    let names = const_type.top_level_class_names();
                    if !names.is_empty() {
                        return names;
                    }
                }
            }
        }

        Vec::new()
    }

    /// Resolve a function call's return type to a list of class names.
    fn resolve_function_return_type_names(
        &self,
        name: &str,
        ctx: &FileContext,
        function_loader: &dyn Fn(&str) -> Option<FunctionInfo>,
        offset: u32,
    ) -> Vec<String> {
        let fqn = ctx.resolve_name_at(name, offset);
        let candidates = [fqn, name.to_string()];

        for candidate in &candidates {
            if let Some(func) = function_loader(candidate) {
                let default_type = PhpType::Named(String::new());
                let ret_type = func.return_type.as_ref().unwrap_or(&default_type);

                let names = ret_type.top_level_class_names();
                if !names.is_empty() {
                    return names;
                }
            }
        }

        Vec::new()
    }

    /// Look up each type name via the class resolution infrastructure
    /// and return definition locations.
    fn resolve_type_names_to_locations(
        &self,
        uri: &str,
        content: &str,
        type_names: &[String],
        cursor_offset: u32,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        for name in type_names {
            if crate::php_type::is_scalar_name_pub(name) {
                continue;
            }

            if let Some(loc) =
                self.resolve_class_reference(uri, content, name, false, cursor_offset)
            {
                // Avoid duplicate locations.
                if !locations
                    .iter()
                    .any(|l: &Location| l.uri == loc.uri && l.range.start == loc.range.start)
                {
                    locations.push(loc);
                }
            }
        }

        locations
    }
}

/// Resolve a variable's type to a list of class/interface/type names.
///
/// This is a free function to avoid clippy's too-many-arguments lint
/// on `&self` methods.
fn resolve_variable_type_names(
    name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: Option<&ClassInfo>,
    ctx: &FileContext,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<FunctionInfo>,
) -> Vec<String> {
    let var_name = format!("${}", name);

    // $this resolves to the enclosing class.
    if name == "this" {
        if let Some(cc) = current_class {
            return vec![cc.name.clone()];
        }
        return Vec::new();
    }

    // Try the type-string path first (preserves generics, union types).
    if let Some(resolved_type) = variable_type::resolve_variable_type(
        &var_name,
        content,
        cursor_offset,
        current_class,
        &ctx.classes,
        class_loader,
        crate::completion::resolver::Loaders::with_function(Some(function_loader)),
    ) {
        return resolved_type.top_level_class_names();
    }

    // Fall back to ClassInfo-based resolution.
    let dummy_class;
    let effective_class = match current_class {
        Some(cc) => cc,
        None => {
            dummy_class = ClassInfo::default();
            &dummy_class
        }
    };

    let types = ResolvedType::into_classes(
        crate::completion::variable::resolution::resolve_variable_types(
            &var_name,
            effective_class,
            &ctx.classes,
            content,
            cursor_offset,
            class_loader,
            Loaders::with_function(Some(function_loader)),
        ),
    );

    types
        .into_iter()
        .map(|c| c.name.clone())
        .filter(|n| !crate::php_type::is_scalar_name_pub(n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_class() {
        let names = PhpType::parse("User").top_level_class_names();
        assert_eq!(names, vec!["User"]);
    }

    #[test]
    fn test_extract_fqn_class() {
        let names = PhpType::parse("\\App\\Models\\User").top_level_class_names();
        assert_eq!(names, vec!["\\App\\Models\\User"]);
    }

    #[test]
    fn test_extract_nullable() {
        let names = PhpType::parse("?User").top_level_class_names();
        assert_eq!(names, vec!["User"]);
    }

    #[test]
    fn test_extract_union_with_null() {
        let names = PhpType::parse("User|null").top_level_class_names();
        assert_eq!(names, vec!["User"]);
    }

    #[test]
    fn test_extract_union_multiple_classes() {
        let names = PhpType::parse("User|Admin").top_level_class_names();
        assert_eq!(names, vec!["User", "Admin"]);
    }

    #[test]
    fn test_extract_generic_stripped() {
        let names = PhpType::parse("Collection<int, User>").top_level_class_names();
        assert_eq!(names, vec!["Collection"]);
    }

    #[test]
    fn test_extract_scalar_excluded() {
        let names = PhpType::parse("string").top_level_class_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_extract_mixed_union() {
        let names = PhpType::parse("string|User|int|Admin|null").top_level_class_names();
        assert_eq!(names, vec!["User", "Admin"]);
    }

    #[test]
    fn test_extract_void() {
        let names = PhpType::parse("void").top_level_class_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_extract_array_of_class() {
        let names = PhpType::parse("User[]").top_level_class_names();
        assert_eq!(names, vec!["User"]);
    }

    #[test]
    fn test_extract_array_shape_excluded() {
        let names = PhpType::parse("array{name: string}").top_level_class_names();
        assert!(names.is_empty());
    }
}
