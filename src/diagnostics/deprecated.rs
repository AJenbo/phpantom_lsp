//! `@deprecated` usage diagnostics.
//!
//! Walk the precomputed [`SymbolMap`] for a file and flag every reference
//! to a class, method, property, constant, or function that carries a
//! `@deprecated` PHPDoc tag or a `#[Deprecated]` attribute.
//!
//! Diagnostics use `Severity::Hint` with `DiagnosticTag::Deprecated`,
//! which renders as a subtle strikethrough in most editors — visible but
//! not noisy.  The message includes the deprecation reason when one is
//! provided in the tag (e.g. `@deprecated Use NewHelper instead`).
//!
//! Variable type resolution is cached per `(variable_name, enclosing_class)`
//! pair so that multiple member accesses on the same variable (e.g.
//! `$user->getName()` and `$user->getEmail()`) only trigger a single
//! resolution pass instead of re-parsing the file for each access.

use std::collections::HashMap;
use std::sync::Arc;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::resolver::Loaders;
use crate::completion::variable::resolution::resolve_variable_types;
use crate::names::OwnedResolvedNames;
use crate::symbol_map::SymbolKind;
use crate::types::{ClassInfo, ResolvedType};
use crate::virtual_members::resolve_class_fully_cached;

use super::helpers::resolve_to_fqn;
use super::offset_range_to_lsp_range;

impl Backend {
    /// Collect `@deprecated` usage diagnostics for a single file.
    ///
    /// Appends diagnostics to `out`.  The caller is responsible for
    /// publishing them via `textDocument/publishDiagnostics`.
    pub fn collect_deprecated_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // Cache of resolved variable types.  Keyed by
        // `(variable_name, enclosing_class_name)` so that all member
        // accesses on the same variable within the same class share a
        // single resolution pass.  This turns O(n * parse) into O(k *
        // parse) where k is the number of distinct variables, not the
        // number of member accesses.
        let mut var_type_cache: HashMap<(String, String), Option<ClassInfo>> = HashMap::new();

        // ── Gather context under locks ──────────────────────────────────
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_resolved_names: Option<Arc<OwnedResolvedNames>> =
            self.resolved_names.read().get(uri).cloned();

        let file_use_map: HashMap<String, String> = self.file_use_map(uri);

        let file_namespace: Option<String> = self.namespace_map.read().get(uri).cloned().flatten();

        let local_classes: Vec<Arc<ClassInfo>> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();

        let class_loader = self.class_loader_with(&local_classes, &file_use_map, &file_namespace);
        let function_loader = self.function_loader_with(&file_use_map, &file_namespace);
        let cache = &self.resolved_class_cache;

        // ── Walk every symbol span ──────────────────────────────────────
        for span in &symbol_map.spans {
            match &span.kind {
                // ── Class references (type hints, new Foo, extends, etc.) ─
                SymbolKind::ClassReference { name, is_fqn, .. } => {
                    // Prefer mago-names byte-offset lookup when available —
                    // it applies PHP's full name resolution rules.  Fall
                    // back to the legacy resolve_to_fqn helper otherwise.
                    let resolved_name = if *is_fqn {
                        name.to_string()
                    } else if let Some(ref rn) = file_resolved_names {
                        rn.get(span.start)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| resolve_to_fqn(name, &file_use_map, &file_namespace))
                    } else {
                        resolve_to_fqn(name, &file_use_map, &file_namespace)
                    };

                    if let Some(cls) = self.find_or_load_class(&resolved_name)
                        && let Some(msg) = &cls.deprecation_message
                        && let Some(range) = offset_range_to_lsp_range(
                            content,
                            span.start as usize,
                            span.end as usize,
                        )
                    {
                        let class_fqn = cls.fqn();
                        out.push(deprecated_diagnostic(
                            range,
                            &class_fqn,
                            None,
                            msg,
                            &cls.see_refs,
                        ));
                    }
                }

                // ── Member accesses ($x->method(), Foo::CONST, etc.) ─────
                SymbolKind::MemberAccess {
                    subject_text,
                    member_name,
                    is_static,
                    is_method_call,
                    ..
                } => {
                    // Resolve the subject type to a class.
                    let base_class = resolve_subject_to_class_name(
                        subject_text,
                        *is_static,
                        &file_use_map,
                        &file_namespace,
                        &local_classes,
                        span.start,
                    )
                    .and_then(|name| self.find_or_load_class(&name))
                    .map(|arc| ClassInfo::clone(&arc));

                    // Fall back to variable type resolution for $var->member() calls.
                    // Use the per-variable cache to avoid re-parsing the
                    // file for every member access on the same variable.
                    let base_class = match base_class {
                        Some(c) => c,
                        None if subject_text.starts_with('$') => {
                            let enclosing_name = local_classes
                                .iter()
                                .find(|c| {
                                    !c.name.starts_with("__anonymous@")
                                        && span.start >= c.start_offset
                                        && span.start <= c.end_offset
                                })
                                .map(|c| c.name.clone())
                                .unwrap_or_default();

                            let cache_key = (subject_text.trim().to_string(), enclosing_name);

                            let cached = var_type_cache.entry(cache_key).or_insert_with_key(|_| {
                                resolve_variable_subject(
                                    subject_text,
                                    span.start,
                                    content,
                                    &local_classes,
                                    &class_loader,
                                    &function_loader,
                                )
                            });

                            match cached {
                                Some(c) => c.clone(),
                                None => continue,
                            }
                        }
                        None => continue,
                    };

                    // Resolve with inheritance + virtual members so we find
                    // members from parent classes and traits too.
                    //
                    // Check the base_class directly first: when the base
                    // comes from variable resolution or call-chain return
                    // type inference, it may already carry model-specific
                    // members (e.g. Eloquent scope methods injected onto
                    // Builder<Model>).  The FQN-keyed cache cannot
                    // distinguish between generic instantiations, so a
                    // cached entry may lack these members.
                    let resolved = resolve_class_fully_cached(&base_class, &class_loader, cache);

                    if *is_method_call {
                        // Check method deprecation — try base_class first
                        // (preserves scope methods), fall back to resolved.
                        if let Some(method) = base_class
                            .methods
                            .iter()
                            .find(|m| m.name == *member_name)
                            .or_else(|| resolved.methods.iter().find(|m| m.name == *member_name))
                            && let Some(msg) = &method.deprecation_message
                            && let Some(range) = offset_range_to_lsp_range(
                                content,
                                span.start as usize,
                                span.end as usize,
                            )
                        {
                            let class_fqn = resolved.fqn();
                            out.push(deprecated_diagnostic(
                                range,
                                member_name,
                                Some(&class_fqn),
                                msg,
                                &method.see_refs,
                            ));
                        }
                    } else {
                        // Property or constant access — try base_class
                        // first (same rationale as above), fall back to
                        // resolved.
                        if let Some(prop) = base_class
                            .properties
                            .iter()
                            .find(|p| p.name == *member_name)
                            .or_else(|| resolved.properties.iter().find(|p| p.name == *member_name))
                            && let Some(msg) = &prop.deprecation_message
                            && let Some(range) = offset_range_to_lsp_range(
                                content,
                                span.start as usize,
                                span.end as usize,
                            )
                        {
                            let class_fqn = resolved.fqn();
                            out.push(deprecated_diagnostic(
                                range,
                                member_name,
                                Some(&class_fqn),
                                msg,
                                &prop.see_refs,
                            ));
                            continue;
                        }

                        // Try constant (static access like Foo::BAR)
                        if *is_static
                            && let Some(constant) =
                                resolved.constants.iter().find(|c| c.name == *member_name)
                            && let Some(msg) = &constant.deprecation_message
                            && let Some(range) = offset_range_to_lsp_range(
                                content,
                                span.start as usize,
                                span.end as usize,
                            )
                        {
                            let class_fqn = resolved.fqn();
                            out.push(deprecated_diagnostic(
                                range,
                                member_name,
                                Some(&class_fqn),
                                msg,
                                &constant.see_refs,
                            ));
                        }
                    }
                }

                // ── Standalone function calls ────────────────────────────
                SymbolKind::FunctionCall {
                    name,
                    is_definition,
                } => {
                    // Skip the declaration site — only flag call sites.
                    if *is_definition {
                        continue;
                    }
                    if let Some(func_info) =
                        self.resolve_function_name(name, &file_use_map, &file_namespace)
                        && let Some(msg) = &func_info.deprecation_message
                        && let Some(range) = offset_range_to_lsp_range(
                            content,
                            span.start as usize,
                            span.end as usize,
                        )
                    {
                        out.push(deprecated_diagnostic(
                            range,
                            name,
                            None,
                            msg,
                            &func_info.see_refs,
                        ));
                    }
                }

                // Other symbol kinds are not checked for deprecation.
                _ => {}
            }
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Build a deprecated diagnostic.
fn deprecated_diagnostic(
    range: Range,
    symbol_name: &str,
    class_name: Option<&str>,
    deprecation_message: &str,
    see_refs: &[String],
) -> Diagnostic {
    let display = if let Some(cls) = class_name {
        format!("{}::{}", cls, symbol_name)
    } else {
        symbol_name.to_string()
    };

    // Combine the deprecation message with @see references so the
    // diagnostic tooltip includes pointers to replacement APIs.
    let full_message = if see_refs.is_empty() {
        deprecation_message.to_string()
    } else {
        let see_list = see_refs.join(", ");
        if deprecation_message.is_empty() {
            format!("See: {}", see_list)
        } else {
            format!("{} (see: {})", deprecation_message, see_list)
        }
    };

    let message = if full_message.is_empty() {
        format!("'{}' is deprecated", display)
    } else {
        format!("'{}' is deprecated: {}", display, full_message)
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::HINT),
        code: Some(NumberOrString::String("deprecated".to_string())),
        code_description: None,
        source: Some("phpantom".to_string()),
        message,
        related_information: None,
        tags: Some(vec![DiagnosticTag::DEPRECATED]),
        data: None,
    }
}

/// Resolve a member access subject text to a class FQN.
///
/// Handles:
/// - `self`, `static`, `parent` → resolve from enclosing class
/// - `ClassName` (static access) → resolve via use map
/// - `$this` → resolve from enclosing class
/// - Other `$variable` subjects return `None` (resolved separately
///   by [`resolve_variable_subject`]).
fn resolve_subject_to_class_name(
    subject_text: &str,
    is_static: bool,
    file_use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    access_offset: u32,
) -> Option<String> {
    let trimmed = subject_text.trim();

    match trimmed {
        "self" | "static" => {
            // Find the enclosing class in this file
            find_enclosing_class_fqn(local_classes, file_namespace, access_offset)
        }
        "parent" => {
            // Find the enclosing class that actually has a parent.
            // Prefer a class whose offset range contains the access site
            // and that has `parent_class` set — that's the one where
            // `parent::` is meaningful.  Fall back to any non-anonymous
            // class with a parent, then to the first non-anonymous class
            // (shouldn't happen in valid code, but be defensive).
            let cls = local_classes
                .iter()
                .find(|c| {
                    !c.name.starts_with("__anonymous@")
                        && c.parent_class.is_some()
                        && access_offset >= c.start_offset
                        && access_offset <= c.end_offset
                })
                .or_else(|| {
                    local_classes
                        .iter()
                        .find(|c| !c.name.starts_with("__anonymous@") && c.parent_class.is_some())
                })
                .or_else(|| {
                    local_classes
                        .iter()
                        .find(|c| !c.name.starts_with("__anonymous@"))
                });
            cls.and_then(|c| {
                c.parent_class
                    .as_ref()
                    .map(|p| resolve_to_fqn(p, file_use_map, file_namespace))
            })
        }
        "$this" => find_enclosing_class_fqn(local_classes, file_namespace, access_offset),
        _ if is_static && !trimmed.starts_with('$') => {
            // Static access on a class name: `ClassName::method()`
            Some(resolve_to_fqn(trimmed, file_use_map, file_namespace))
        }
        _ if trimmed.starts_with('$') => {
            // Variable access — resolved separately by
            // resolve_variable_subject().
            None
        }
        _ => {
            // Could be a function return or expression — skip for now
            None
        }
    }
}

/// Resolve a `$variable` subject to a `ClassInfo` using the full
/// variable type resolution pipeline.
///
/// Finds the enclosing class for the access site, then delegates to
/// [`resolve_variable_types`] which re-parses the source and walks the
/// AST to infer the variable's type from assignments, parameter type
/// hints, foreach bindings, etc.
fn resolve_variable_subject(
    subject_text: &str,
    access_offset: u32,
    content: &str,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
) -> Option<ClassInfo> {
    let var_name = subject_text.trim();

    // Find the enclosing class based on offset ranges.
    let enclosing_class = local_classes
        .iter()
        .find(|c| {
            !c.name.starts_with("__anonymous@")
                && access_offset >= c.start_offset
                && access_offset <= c.end_offset
        })
        .map(|c| ClassInfo::clone(c))
        .unwrap_or_default();

    let results = ResolvedType::into_classes(resolve_variable_types(
        var_name,
        &enclosing_class,
        local_classes,
        content,
        access_offset,
        class_loader,
        Loaders::with_function(Some(function_loader)),
    ));

    results.into_iter().next()
}

/// Find the FQN of the first non-anonymous class in the file (heuristic
/// for the "enclosing class" in single-class-per-file projects).
fn find_enclosing_class_fqn(
    local_classes: &[Arc<ClassInfo>],
    file_namespace: &Option<String>,
    offset: u32,
) -> Option<String> {
    // Find the non-anonymous class whose byte range contains the offset.
    // Fall back to the first non-anonymous class for top-level code
    // outside any class body.
    let cls = local_classes
        .iter()
        .find(|c| {
            !c.name.starts_with("__anonymous@")
                && offset >= c.start_offset
                && offset <= c.end_offset
        })
        .or_else(|| {
            local_classes
                .iter()
                .find(|c| !c.name.starts_with("__anonymous@"))
        })?;
    Some(crate::util::build_fqn(&cls.name, file_namespace))
}
