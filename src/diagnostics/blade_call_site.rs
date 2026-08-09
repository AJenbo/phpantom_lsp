//! Validation of `view()` call sites against a template's declared
//! contract.
//!
//! This is the diagnostics counterpart of [`crate::blade::signature`]'s
//! declaration chain, and the editor half of the rule Bladestan (the
//! PHPStan extension for Blade) enforces in CI: one annotation, the same
//! errors in both places.
//!
//! A template that declares a contract is treated exactly like a function
//! signature. Every `view('name', […])`, `View::make()`, `Route::view()`,
//! and `@include('name', […])` that names it is checked against the merged
//! contract of the template and the layouts it `@extends`:
//!
//! * a declared variable the call does not pass is **missing**;
//! * a passed variable whose type the declaration does not accept is a
//!   **mismatch**;
//! * a passed variable nothing in the template (or in anything it renders
//!   from the same data) reads is **unknown**.
//!
//! A template that declares nothing produces no call-site diagnostics at
//! all, which is what keeps this opt-in. Every check also stands down as
//! soon as the data involved stops being readable: a data argument built
//! from a variable, a template that reads its scope wholesale, or a view
//! name no view root holds each leave a hole through which a name could
//! legitimately arrive.

use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::Backend;
use crate::blade::contract::{AcceptedNames, TemplateContract};
use crate::symbol_map::{LaravelStringKind, SymbolKind};

use super::helpers::make_diagnostic;
use super::type_errors::compatibility::is_type_compatible;

/// A declared variable the call site does not pass.
const MISSING_CODE: &str = "missing_view_variable";
/// A passed variable the template has no use for.
const UNKNOWN_CODE: &str = "unknown_view_variable";
/// A passed variable whose type the declaration does not accept.
const TYPE_MISMATCH_CODE: &str = "type_mismatch_view_variable";

impl Backend {
    /// Check every view render in `uri` against the contract the template
    /// it names declares.
    ///
    /// `content` is the file's effective source — the virtual PHP for a
    /// Blade template — so the spans line up with the symbol map and the
    /// ranges translate back through the source map.
    pub(super) fn collect_blade_call_site_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // The view name each recorded key sits at, so a call site found by
        // offset can be traced back to the template it names.
        let keys: HashMap<u32, String> = {
            let maps = self.symbol_maps.read();
            let Some(symbol_map) = maps.get(uri) else {
                return;
            };
            symbol_map
                .spans
                .iter()
                .filter_map(|span| match &span.kind {
                    SymbolKind::LaravelStringKey {
                        kind: LaravelStringKind::View,
                        key,
                        is_write: false,
                        ..
                    } => Some((span.start, key.clone())),
                    _ => None,
                })
                .collect()
        };
        if keys.is_empty() {
            return;
        }

        let offsets: Vec<u32> = keys.keys().copied().collect();
        let sites = self.extract_call_site_vars(uri, content, &offsets);
        if sites.is_empty() {
            return;
        }

        // A template renders its includes from its own scope, so what is
        // already in scope here decides whether an `@include` is short of
        // anything.  A plain PHP call site passes exactly what it lists.
        let inherited = if self.is_blade_file(uri) {
            self.blade_rendering_scope(uri)
        } else {
            Some(HashSet::new())
        };
        // What a render that forwards nothing inherits, so a site can stand
        // in for the file-wide answer without cloning it.
        let no_inherited = HashSet::new();

        let file_ctx = self.file_context(uri);
        let class_loader = self.class_loader(&file_ctx);

        let mut contracts: HashMap<String, Option<TemplateContract>> = HashMap::new();
        let mut accepted: HashMap<String, AcceptedNames> = HashMap::new();
        let mut component_scopes: HashMap<usize, HashSet<String>> = HashMap::new();

        for site in sites {
            let Some(view_name) = keys.get(&site.name_range.0) else {
                continue;
            };
            let contract = contracts
                .entry(view_name.clone())
                .or_insert_with(|| self.blade_template_contract(view_name));
            let Some(contract) = contract.as_ref() else {
                continue;
            };

            // ── Types ────────────────────────────────────────────────
            // Judged per name, so a call site that builds part of its data
            // dynamically is still checked on the part it writes out.
            for var in &site.vars {
                if crate::blade::contract::is_framework_var(&var.name) {
                    continue;
                }
                let Some(declared) = contract.declared(&var.name) else {
                    continue;
                };
                if is_type_compatible(&var.ty, declared, &class_loader, true) {
                    continue;
                }
                let Some(range) = self.offset_range_to_lsp_range(
                    uri,
                    content,
                    var.value_range.0 as usize,
                    var.value_range.1 as usize,
                ) else {
                    continue;
                };
                out.push(make_diagnostic(
                    range,
                    DiagnosticSeverity::ERROR,
                    TYPE_MISMATCH_CODE,
                    format!(
                        "View '{}' expects ${} of type {}, got {}",
                        view_name,
                        var.name,
                        declared.conditionals_as_branch_unions(),
                        var.ty,
                    ),
                ));
            }

            // Everything below asks what the call site did *not* pass,
            // which only means something when what it does pass is fully
            // readable.
            if !site.complete {
                continue;
            }

            // ── Missing ──────────────────────────────────────────────
            // An `@each` partial sees only the item and the key, so what
            // the surrounding template holds does not reach it.
            let site_inherited = match site.forwards_scope {
                true => inherited.as_ref(),
                false => Some(&no_inherited),
            };
            if let Some(inherited) = site_inherited {
                // A component's `render()` hands its template every public
                // member of the class, so those are not the call's to pass.
                let from_class = self.component_render_scope(
                    &file_ctx,
                    site.name_range.0,
                    &mut component_scopes,
                );
                for (name, ty) in &contract.vars {
                    if contract.supplied.contains(name)
                        || inherited.contains(name)
                        || from_class.contains(name)
                        || site.vars.iter().any(|var| &var.name == name)
                    {
                        continue;
                    }
                    let Some(range) = self.offset_range_to_lsp_range(
                        uri,
                        content,
                        site.name_range.0 as usize,
                        site.name_range.1 as usize,
                    ) else {
                        continue;
                    };
                    out.push(make_diagnostic(
                        range,
                        DiagnosticSeverity::ERROR,
                        MISSING_CODE,
                        format!(
                            "View '{}' expects ${} of type {}, which is not passed",
                            view_name, name, ty,
                        ),
                    ));
                }
            }

            // ── Unknown ──────────────────────────────────────────────
            let accepted = accepted
                .entry(view_name.clone())
                .or_insert_with(|| self.blade_accepted_names(view_name));
            if !accepted.closed {
                continue;
            }
            for var in &site.vars {
                if accepted.names.contains(&var.name) || var.framework_bound {
                    continue;
                }
                let Some(range) = self.offset_range_to_lsp_range(
                    uri,
                    content,
                    var.key_range.0 as usize,
                    var.key_range.1 as usize,
                ) else {
                    continue;
                };
                out.push(make_diagnostic(
                    range,
                    DiagnosticSeverity::WARNING,
                    UNKNOWN_CODE,
                    format!("View '{}' has no variable ${}", view_name, var.name),
                ));
            }
        }
    }

    /// The names the class enclosing the call at `offset` puts in the
    /// rendered template's scope.
    ///
    /// Empty for a plain controller, whose properties never reach a view,
    /// and for a component that exposes nothing — the two are the same
    /// answer as far as a call site is concerned.
    ///
    /// Cached per enclosing class for the file's whole pass: resolving a
    /// class fully to read its public members is far too much work to
    /// repeat per call site in a component with several renders.
    fn component_render_scope<'a>(
        &self,
        file_ctx: &crate::types::FileContext,
        offset: u32,
        cache: &'a mut HashMap<usize, HashSet<String>>,
    ) -> &'a HashSet<String> {
        let key = crate::class_lookup::find_class_at_offset(&file_ctx.classes, offset)
            .map_or(0, |class| std::ptr::from_ref(class) as usize);
        cache.entry(key).or_insert_with(|| {
            crate::class_lookup::find_class_at_offset(&file_ctx.classes, offset)
                .and_then(|class| self.component_render_scope_names(class))
                .map(|names| names.into_iter().collect())
                .unwrap_or_default()
        })
    }
}
