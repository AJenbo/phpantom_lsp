/// AST update orchestration and name resolution.
///
/// This module contains the `update_ast` method that performs a full
/// parse of a PHP file and updates all the backend maps (ast_map,
/// use_map, namespace_map, global_functions, global_defines, class_index)
/// in a single pass.  It also contains the name resolution helpers
/// (`resolve_parent_class_names`, `resolve_name`) used to convert short
/// class names to fully-qualified names.
use std::collections::HashMap;
use std::panic;

use crate::docblock::types::is_scalar;

use bumpalo::Bump;

use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

use crate::Backend;
use crate::types::ClassInfo;

use super::DocblockCtx;

impl Backend {
    /// Update the ast_map, use_map, and namespace_map for a given file URI
    /// by parsing its content.
    pub fn update_ast(&self, uri: &str, content: &str) {
        // The mago-syntax parser contains `unreachable!()` and `.expect()`
        // calls that can panic on malformed PHP (e.g. partially-written
        // heredocs/nowdocs, which are common while editing).  Wrap the
        // entire parse + extraction in `catch_unwind` so a parser panic
        // doesn't crash the LSP server and produce a zombie process.
        //
        // On panic the file is simply skipped — no maps are updated, and
        // the user gets stale (but not missing) completions until the
        // file is saved in a parseable state.
        let content_owned = content.to_string();
        let uri_owned = uri.to_string();

        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.update_ast_inner(&uri_owned, &content_owned);
        }));

        if result.is_err() {
            log::error!(
                "PHPantomLSP: parser panicked while parsing {}. Skipping file.",
                uri
            );
        }
    }

    /// Inner implementation of [`update_ast`] that performs the actual
    /// parsing and map updates.  Separated so that [`update_ast`] can
    /// wrap the call in [`std::panic::catch_unwind`].
    fn update_ast_inner(&self, uri: &str, content: &str) {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let doc_ctx = DocblockCtx {
            trivias: program.trivia.as_slice(),
            content,
        };

        // Extract all three in a single parse pass
        let mut classes = Vec::new();
        let mut use_map = HashMap::new();
        let mut namespace: Option<String> = None;

        for statement in program.statements.iter() {
            match statement {
                Statement::Use(use_stmt) => {
                    Self::extract_use_items(&use_stmt.items, &mut use_map);
                }
                Statement::Namespace(ns) => {
                    // Capture namespace name
                    if let Some(ident) = &ns.name {
                        let name = ident.value();
                        if !name.is_empty() && namespace.is_none() {
                            namespace = Some(name.to_string());
                        }
                    }
                    // Recurse into namespace body for classes and use statements
                    for inner in ns.statements().iter() {
                        match inner {
                            Statement::Use(use_stmt) => {
                                Self::extract_use_items(&use_stmt.items, &mut use_map);
                            }
                            Statement::Class(_)
                            | Statement::Interface(_)
                            | Statement::Trait(_)
                            | Statement::Enum(_) => {
                                Self::extract_classes_from_statements(
                                    std::iter::once(inner),
                                    &mut classes,
                                    Some(&doc_ctx),
                                );
                            }
                            Statement::Namespace(inner_ns) => {
                                // Nested namespaces (rare but valid)
                                Self::extract_use_statements_from_statements(
                                    inner_ns.statements().iter(),
                                    &mut use_map,
                                );
                                Self::extract_classes_from_statements(
                                    inner_ns.statements().iter(),
                                    &mut classes,
                                    Some(&doc_ctx),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Statement::Class(_)
                | Statement::Interface(_)
                | Statement::Trait(_)
                | Statement::Enum(_) => {
                    Self::extract_classes_from_statements(
                        std::iter::once(statement),
                        &mut classes,
                        Some(&doc_ctx),
                    );
                }
                _ => {}
            }
        }

        // Extract standalone functions (including those inside if-guards
        // like `if (! function_exists('...'))`) using the shared helper
        // which recurses into if/block statements.
        let mut functions = Vec::new();
        Self::extract_functions_from_statements(
            program.statements.iter(),
            &mut functions,
            &namespace,
            Some(&doc_ctx),
        );
        if !functions.is_empty()
            && let Ok(mut fmap) = self.global_functions.lock()
        {
            for func_info in functions {
                let fqn = if let Some(ref ns) = func_info.namespace {
                    format!("{}\\{}", ns, &func_info.name)
                } else {
                    func_info.name.clone()
                };

                // Insert both the FQN and the short name so that
                // callers using bare `func()` can resolve.
                fmap.insert(fqn.clone(), (uri.to_string(), func_info.clone()));
                if func_info.namespace.is_some() {
                    fmap.entry(func_info.name.clone())
                        .or_insert_with(|| (uri.to_string(), func_info));
                }
            }
        }

        // Extract define() constants from the already-parsed AST and
        // store them in the global_defines map so they appear in
        // completions.  This reuses the parse pass above rather than
        // doing a separate regex scan over the raw content.
        let mut define_names = Vec::new();
        Self::extract_defines_from_statements(program.statements.iter(), &mut define_names);
        if !define_names.is_empty()
            && let Ok(mut dmap) = self.global_defines.lock()
        {
            for name in define_names {
                dmap.entry(name).or_insert_with(|| uri.to_string());
            }
        }

        // Post-process: resolve parent_class short names to fully-qualified
        // names using the file's use_map and namespace so that cross-file
        // inheritance resolution can find parent classes via PSR-4.
        Self::resolve_parent_class_names(&mut classes, &use_map, &namespace);

        let uri_string = uri.to_string();

        // Populate the class_index with FQN → URI mappings for every class
        // found in this file.  This enables reliable lookup of classes that
        // don't follow PSR-4 conventions (e.g. classes defined in Composer
        // autoload_files.php entries).
        if let Ok(mut idx) = self.class_index.lock() {
            for class in &classes {
                let fqn = if let Some(ref ns) = namespace {
                    format!("{}\\{}", ns, &class.name)
                } else {
                    class.name.clone()
                };
                idx.insert(fqn, uri_string.clone());
            }
        }

        if let Ok(mut map) = self.ast_map.lock() {
            map.insert(uri_string.clone(), classes);
        }
        if let Ok(mut map) = self.use_map.lock() {
            map.insert(uri_string.clone(), use_map);
        }
        if let Ok(mut map) = self.namespace_map.lock() {
            map.insert(uri_string, namespace);
        }
    }

    /// Resolve `parent_class` short names in a list of `ClassInfo` to
    /// fully-qualified names using the file's `use_map` and `namespace`.
    ///
    /// Rules (matching PHP name resolution):
    ///   1. Already fully-qualified (`\Foo\Bar`) → strip leading `\`
    ///   2. Qualified (`Foo\Bar`) → if first segment is in use_map, expand it;
    ///      otherwise prepend current namespace
    ///   3. Unqualified (`Bar`) → check use_map; otherwise prepend namespace
    ///   4. No namespace and not in use_map → keep as-is
    pub fn resolve_parent_class_names(
        classes: &mut [ClassInfo],
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) {
        for class in classes.iter_mut() {
            if let Some(ref parent) = class.parent_class {
                let resolved = Self::resolve_name(parent, use_map, namespace);
                class.parent_class = Some(resolved);
            }
            // Resolve trait names to fully-qualified names
            class.used_traits = class
                .used_traits
                .iter()
                .map(|t| Self::resolve_name(t, use_map, namespace))
                .collect();

            // Resolve mixin names to fully-qualified names
            class.mixins = class
                .mixins
                .iter()
                .map(|m| Self::resolve_name(m, use_map, namespace))
                .collect();

            // Resolve type arguments in @extends, @implements, and @use
            // generics so that after generic substitution, return types
            // and property types are fully-qualified and can be resolved
            // across files via PSR-4.
            Self::resolve_generics_type_args(&mut class.extends_generics, use_map, namespace);
            Self::resolve_generics_type_args(&mut class.implements_generics, use_map, namespace);
            Self::resolve_generics_type_args(&mut class.use_generics, use_map, namespace);
        }
    }

    /// Resolve type arguments in a generics list (e.g. `@extends`, `@implements`,
    /// `@use`) to fully-qualified names.
    ///
    /// Each entry is `(ClassName, [TypeArg1, TypeArg2, …])`.  The class name
    /// itself is resolved (e.g. `HasFactory` → `App\Concerns\HasFactory`),
    /// and each type argument that looks like a class name (i.e. not a scalar
    /// like `int`, `string`, etc.) is also resolved.
    fn resolve_generics_type_args(
        generics: &mut [(String, Vec<String>)],
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) {
        for (class_name, type_args) in generics.iter_mut() {
            // Resolve the base class/trait/interface name
            *class_name = Self::resolve_name(class_name, use_map, namespace);

            // Resolve each type argument that is a class-like name
            for arg in type_args.iter_mut() {
                if !is_scalar(arg)
                    && *arg != "mixed"
                    && *arg != "object"
                    && *arg != "static"
                    && *arg != "self"
                    && *arg != "$this"
                {
                    *arg = Self::resolve_name(arg, use_map, namespace);
                }
            }
        }
    }

    /// Resolve a class name to its fully-qualified form given a use_map and
    /// namespace context.
    fn resolve_name(
        name: &str,
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) -> String {
        // 1. Already fully-qualified — keep the leading `\` so that
        // downstream `resolve_class_name` recognises the name as a
        // root-namespace FQN and does NOT prepend the current file's
        // namespace.  For example `\RuntimeException` stays as
        // `\RuntimeException`; `resolve_class_name` will strip the
        // prefix itself and look up `RuntimeException` globally.
        if name.starts_with('\\') {
            return name.to_string();
        }

        // 2/3. Check if the (first segment of the) name is in the use_map
        if let Some(pos) = name.find('\\') {
            // Qualified name — check first segment
            let first = &name[..pos];
            let rest = &name[pos..]; // includes leading '\'
            if let Some(fqn) = use_map.get(first) {
                return format!("{}{}", fqn, rest);
            }
        } else {
            // Unqualified name — check directly
            if let Some(fqn) = use_map.get(name) {
                return fqn.clone();
            }
        }

        // 4. Prepend current namespace if available.
        //    When there is NO namespace the name lives in the global scope,
        //    so prefix it with `\` so that downstream `resolve_class_name`
        //    recognises it as a root-namespace FQN and does NOT try to
        //    prepend the caller's file namespace (e.g. avoids resolving
        //    `Exception` as `Demo\Exception` when loading a stub parent).
        if let Some(ns) = namespace {
            format!("{}\\{}", ns, name)
        } else {
            format!("\\{}", name)
        }
    }
}
