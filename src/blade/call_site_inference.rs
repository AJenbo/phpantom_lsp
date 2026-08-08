//! Call-site variable inference for Blade templates.
//!
//! For templates without a declared signature (`@bladestan-signature`
//! or plain `@var` docblocks), infer the variables a template receives
//! from the call sites that reference it: `view()`/`View::make()` calls
//! (literal array keys, `compact()` arguments, `->with()` chains — see
//! [`extract_call_site_vars`]) and, for a component, the attributes each
//! `<x-…>` tag passes (see [`super::component_tags`]). The inferred set
//! is injected into the template's virtual-PHP prologue as `@var`
//! docblock declarations (see `preprocess_with_vars`), so every
//! consumer — completion, hover, go-to-definition, and the
//! undefined-variable diagnostic — sees them through the ordinary
//! resolution pipeline.
//!
//! This is deliberately the lowest-priority source: an in-template
//! `@var` annotation shadows an injected one (it sits closer to every
//! use site in the backward docblock scan), `@props`/`@aware`, a
//! component's backing class (see `super::backing_class`), and a
//! provider's shared and composed data (see `super::shared_vars`) win
//! over it per name, and templates that declare a signature are skipped
//! entirely. Types are "true for the callers we found": multiple call
//! sites union per variable, and dynamic view names contribute nothing.

use std::collections::HashMap;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::literal::{Literal, LiteralString};
use mago_syntax::cst::sequence::TokenSeparatedSequence;
use mago_syntax::cst::*;

use crate::Backend;
use crate::atom::bytes_to_str;
use crate::parser::with_parsed_program;
use crate::php_type::PhpType;
use crate::symbol_map::{LaravelStringKind, SymbolKind, SymbolMap};
use crate::type_engine::resolver::{Loaders, VarResolutionCtx};
use crate::types::ClassInfo;

/// A variable passed to a template at one call site: the name (without
/// `$`) and the expression's resolved type.
type InferredVars = Vec<(String, PhpType)>;

/// A byte range `[start, end)` in a caller file.
pub(crate) type ByteRange = (u32, u32);

/// One variable a `view()` call site passes, with the ranges that let a
/// diagnostic point at the key and at the value independently.
pub(crate) struct PassedVar {
    pub(crate) name: String,
    pub(crate) ty: PhpType,
    /// The key that named the variable — an array key, a `compact()`
    /// argument, or the `->withName()` method name.
    pub(crate) key_range: ByteRange,
    /// The expression that produced the value.
    pub(crate) value_range: ByteRange,
}

/// One resolved `view()` / `View::make()` / `@include` call site.
pub(crate) struct ResolvedViewCall {
    /// The view-name string's contents, matching the offsets the symbol
    /// map records for a Laravel view key.
    pub(crate) name_range: ByteRange,
    pub(crate) vars: Vec<PassedVar>,
    /// Whether every data source at the site was readable, so [`Self::vars`]
    /// is everything the caller hands the template. A `view($name, $data)`
    /// whose data is a variable passes an unknown set, and neither a
    /// missing nor an unwanted name can be concluded from it.
    pub(crate) complete: bool,
}

/// The variables injected into one template's virtual-PHP prologue:
/// (name without `$`, docblock type string).
pub(crate) type InjectedVars = Vec<(String, String)>;

/// What a template's virtual PHP is seeded with beyond the template's own
/// source: the variables its prologue declares, and the class its `$this`
/// is bound to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BladeScope {
    /// Highest-priority source first — the prologue declares the first
    /// entry for a name and skips the rest.
    pub vars: InjectedVars,
    /// The fully qualified name of the component instance a Livewire view
    /// renders with, which the preprocessor wraps the body in a method of
    /// (see `preprocess_with_vars`).  `None` for every other template.
    pub this_class: Option<String>,
}

/// User files that render Blade views, with their symbol maps.  Shared
/// across a whole refresh pass so the workspace is walked once, not once
/// per template.
pub(crate) type ViewCallerSnapshot = Vec<(String, Arc<SymbolMap>)>;

/// Every Blade file's raw source, snapshotted once per refresh pass.
///
/// Unlike [`ViewCallerSnapshot`], there is no pre-built index of `<x-…>`
/// tag usages to filter by first (component tags are HTML, not something
/// the symbol map extracts), so finding a component's callers means
/// scanning every Blade file's content. Sharing this list across a whole
/// refresh pass keeps that scan at O(templates) rather than
/// O(templates × templates).
pub(crate) type BladeCallerSnapshot = Vec<(String, Arc<String>)>;

/// Append the entries whose names nothing has declared yet, so the
/// highest-priority source to carry a name is the one that keeps it.
fn push_undeclared(declared: &mut InjectedVars, vars: InjectedVars) {
    for (name, ty) in vars {
        if declared.iter().any(|(existing, _)| existing == &name) {
            continue;
        }
        declared.push((name, ty));
    }
}

impl Backend {
    /// Compute the variables to inject into a Blade template's virtual
    /// PHP: the members of the class backing a component view (see
    /// [`super::backing_class`]), what the layouts it `@extends` declare
    /// (see [`super::layout`]), the variables a service provider shares
    /// or composes into its scope (see [`super::shared_vars`]), then the
    /// variables its `view()` call sites and, for a component, its `<x-…>`
    /// tag call sites pass.
    ///
    /// Returns pairs of (variable name without `$`, docblock type
    /// string), highest-priority source first — the prologue declares
    /// the first entry for a name and skips the rest — alongside the
    /// class the template's `$this` is bound to.  Empty when the
    /// template has no backing class, extends nothing, and no call site
    /// references it, or when the template's view name cannot be derived
    /// from its path.
    pub(crate) fn compute_blade_injected_vars(
        &self,
        uri: &str,
        blade_content: &str,
        shared: Option<&ViewCallerSnapshot>,
        shared_blade: Option<&BladeCallerSnapshot>,
    ) -> BladeScope {
        let view_names = self.view_names_for_blade_uri(uri);
        if view_names.is_empty() {
            return BladeScope::default();
        }

        // The backing class is a *declared* source, so it stands whatever
        // else the template says; only the names its own signature
        // declares win over it (the preprocessor applies that).
        let (mut declared, this_class) = self.blade_backing_class_vars(&view_names);

        // The layout the template `@extends` is rendered from the same data
        // the template is, so what it declares the template receives too
        // (see [`super::layout`]).
        push_undeclared(&mut declared, self.blade_layout_vars(blade_content));

        // What a service provider shares or composes into this template's
        // scope: no template declares it and no caller passes it, but it is
        // still written down somewhere, so it beats inference (see
        // [`super::shared_vars`]).
        push_undeclared(&mut declared, self.blade_provider_vars(&view_names));

        // A template that declares a signature manages its own contract;
        // inferring on top would fight the declared types.
        if crate::blade::signature::has_declared_signature(blade_content) {
            return BladeScope {
                vars: declared,
                this_class,
            };
        }

        // Find every file whose symbol map contains a View string key
        // matching one of this template's names.
        let own_snapshot;
        let snapshot = match shared {
            Some(shared) => shared.as_slice(),
            None => {
                let keys: Vec<crate::reference_index::ReferenceIndexKey> = view_names
                    .iter()
                    .map(
                        |name| crate::reference_index::ReferenceIndexKey::LaravelString {
                            kind: LaravelStringKind::View,
                            key: name.clone(),
                        },
                    )
                    .collect();
                // Never trigger (or wait on) workspace indexing from here:
                // this runs while a Blade file is being opened or a
                // controller saved, and a keystroke must not pay for a
                // workspace walk.  Before the index is ready this scans
                // whatever is parsed; the post-index refresh pass picks up
                // call sites discovered later.
                own_snapshot = self.user_file_symbol_maps_for_reference_keys_nonblocking(&keys);
                own_snapshot.as_slice()
            }
        };

        // Union the variables from every call site, per name.
        let mut merged: HashMap<String, Vec<PhpType>> = HashMap::new();
        for (file_uri, symbol_map) in snapshot {
            // A template must not feed itself (`@include` spans inside
            // the template's own virtual PHP), and other templates'
            // `@include`s would recurse — skip Blade files entirely.
            if self.is_blade_file(file_uri) {
                continue;
            }
            let offsets: Vec<u32> = symbol_map
                .spans
                .iter()
                .filter_map(|span| match &span.kind {
                    SymbolKind::LaravelStringKey {
                        kind: LaravelStringKind::View,
                        key,
                        ..
                    } if view_names.iter().any(|n| n == key) => Some(span.start),
                    _ => None,
                })
                .collect();
            if offsets.is_empty() {
                continue;
            }
            let Some(content) = self.get_file_content(file_uri) else {
                continue;
            };
            for site in self.extract_call_site_vars(file_uri, &content, &offsets) {
                for var in site.vars {
                    merged.entry(var.name).or_default().push(var.ty);
                }
            }
        }

        // The attributes each `<x-…>` tag passes, for a template
        // addressable as a component tag (`components.*` or a namespaced
        // view name — see `component_tags::component_tag_names`).
        let tag_names = crate::blade::component_tags::component_tag_names(&view_names);
        if !tag_names.is_empty() {
            let own_blade_snapshot;
            let blade_snapshot = match shared_blade {
                Some(shared) => shared.as_slice(),
                None => {
                    own_blade_snapshot = self.blade_caller_snapshot();
                    own_blade_snapshot.as_slice()
                }
            };
            for (file_uri, content) in blade_snapshot {
                if file_uri == uri {
                    continue;
                }
                let occurrences =
                    crate::blade::component_tags::scan_component_tag_calls(content, &tag_names);
                if occurrences.is_empty() {
                    continue;
                }
                let Some(virtual_php) = self.blade_virtual_content.read().get(file_uri).cloned()
                else {
                    continue;
                };
                for vars in
                    self.extract_component_call_site_vars(file_uri, &virtual_php, occurrences)
                {
                    for (name, ty) in vars {
                        merged.entry(name).or_default().push(ty);
                    }
                }
            }
        }

        if merged.is_empty() {
            return BladeScope {
                vars: declared,
                this_class,
            };
        }

        // A name a declared source already carries needs no inference: what
        // the backing class holds, and what a provider writes into the view's
        // data, beat what one caller happened to pass.
        merged.retain(|name, _| !declared.iter().any(|(existing, _)| existing == name));

        let mut result: Vec<(String, String)> = merged
            .into_iter()
            .map(|(name, types)| {
                let mut unique: Vec<PhpType> = Vec::new();
                for ty in types {
                    if !unique.iter().any(|u| u.equivalent(&ty)) {
                        unique.push(ty);
                    }
                }
                let joined = if unique.len() == 1 {
                    unique.pop().unwrap()
                } else {
                    PhpType::union(unique)
                };
                (name, joined.to_string())
            })
            .collect();
        // Deterministic prologue ordering so re-preprocessing an
        // unchanged template produces identical virtual PHP.
        result.sort_by(|a, b| a.0.cmp(&b.0));
        // The declared sources lead, so theirs are the declarations the
        // prologue emits for the names more than one source carries.
        let mut vars = declared;
        vars.extend(result);
        BladeScope { vars, this_class }
    }

    /// Re-run call-site inference for already-preprocessed Blade
    /// templates and re-parse the ones whose inferred variable set
    /// changed.
    ///
    /// Parse order is arbitrary: a template preprocessed before its
    /// controllers were indexed saw no call sites.  Run this after a
    /// pass that parses many files (workspace indexing, the analyse
    /// CLI's parse phase) or after a controller edit, so templates pick
    /// up call sites discovered since they were preprocessed.  Cheap
    /// for templates whose inference is unchanged (no re-parse).
    pub(crate) fn refresh_blade_injected_vars(&self) {
        let blade_uris: Vec<String> = self.blade_virtual_content.read().keys().cloned().collect();
        if blade_uris.is_empty() {
            return;
        }
        // Snapshot the caller files once for the whole pass.  Letting each
        // template take its own snapshot walks every symbol map (and, for
        // component tags, every Blade file) in the workspace per
        // template, which is quadratic in a project with hundreds of
        // templates.
        let shared = self.view_caller_snapshot();
        let shared_blade = self.blade_caller_snapshot();
        for uri in blade_uris {
            let Some(content) = self.get_file_content(&uri) else {
                continue;
            };
            self.reinfer_and_reparse_blade_with(&uri, &content, Some(&shared), Some(&shared_blade));
        }
    }

    /// Every parsed user file that renders at least one Blade view, with
    /// its symbol map.  Non-Blade only: a template must not feed itself
    /// (`@include` spans sit inside its own virtual PHP), and other
    /// templates' `@include`s would recurse.
    fn view_caller_snapshot(&self) -> ViewCallerSnapshot {
        let vendor_prefixes = self.workspace.vendor_uri_prefixes.lock().clone();
        let maps = self.symbol_maps.read();
        maps.iter()
            .filter(|(uri, map)| {
                !uri.starts_with("phpantom-stub://")
                    && !uri.starts_with("phpantom-stub-fn://")
                    && !vendor_prefixes.iter().any(|p| uri.starts_with(p.as_str()))
                    && !self.is_blade_file(uri)
                    && map.spans.iter().any(|span| {
                        matches!(
                            &span.kind,
                            SymbolKind::LaravelStringKey {
                                kind: LaravelStringKind::View,
                                ..
                            }
                        )
                    })
            })
            .map(|(uri, map)| (uri.clone(), Arc::clone(map)))
            .collect()
    }

    /// Every known Blade file's raw source, for the component-tag scan in
    /// [`Self::compute_blade_injected_vars`]. Unlike [`Self::view_caller_snapshot`]
    /// this cannot pre-filter by symbol-map spans (component tags are HTML,
    /// not something the symbol map extracts), so it just snapshots every
    /// Blade file once per refresh pass.
    fn blade_caller_snapshot(&self) -> BladeCallerSnapshot {
        let vendor_prefixes = self.workspace.vendor_uri_prefixes.lock().clone();
        let uris: Vec<String> = self.blade_virtual_content.read().keys().cloned().collect();
        uris.into_iter()
            .filter(|uri| !vendor_prefixes.iter().any(|p| uri.starts_with(p.as_str())))
            .filter_map(|uri| {
                let content = self.get_file_content_arc(&uri)?;
                Some((uri, content))
            })
            .collect()
    }

    /// Re-infer one template on its own, taking its own caller snapshot.
    /// For the single-template triggers (opening a Blade file, saving a
    /// controller) rather than a bulk refresh pass.
    pub(crate) fn reinfer_and_reparse_blade(&self, uri: &str, content: &str) -> bool {
        self.reinfer_and_reparse_blade_with(uri, content, None, None)
    }

    /// Recompute one template's inferred variable set; when it differs
    /// from the cached set, overwrite the cache and re-parse the
    /// template (`update_ast` reads the cache, so it must be written
    /// first).  A missing cache entry counts as empty, matching what
    /// `update_ast` injects on a cache miss.
    fn reinfer_and_reparse_blade_with(
        &self,
        uri: &str,
        content: &str,
        shared: Option<&ViewCallerSnapshot>,
        shared_blade: Option<&BladeCallerSnapshot>,
    ) -> bool {
        let fresh = self.compute_blade_injected_vars(uri, content, shared, shared_blade);
        let unchanged = match self.blade_injected_vars.read().get(uri) {
            Some(prev) => *prev == fresh,
            None => fresh == BladeScope::default(),
        };
        if unchanged {
            return false;
        }
        self.blade_injected_vars
            .write()
            .insert(uri.to_string(), fresh);
        self.update_ast(uri, content);
        true
    }

    /// Re-run call-site inference for the templates referenced by one
    /// caller file (after it was edited or re-indexed), so an updated
    /// `view()` call is reflected in the template without waiting for
    /// the template's own next parse.
    ///
    /// Only templates that are already preprocessed are refreshed; a
    /// template parsed for the first time later runs inference itself.
    pub(crate) fn refresh_blade_inference_for_caller(&self, caller_uri: &str) {
        if self.is_blade_file(caller_uri) {
            self.refresh_blade_component_inference_for_caller(caller_uri);
            self.refresh_blade_layout_children(caller_uri);
            return;
        }
        let Some(map) = self.symbol_maps.read().get(caller_uri).cloned() else {
            return;
        };
        let mut names: Vec<&str> = map
            .spans
            .iter()
            .filter_map(|span| match &span.kind {
                SymbolKind::LaravelStringKey {
                    kind: LaravelStringKind::View,
                    key,
                    ..
                } => Some(key.as_str()),
                _ => None,
            })
            .collect();
        if names.is_empty() {
            return;
        }
        names.sort_unstable();
        names.dedup();

        for name in names {
            for location in crate::virtual_members::laravel::resolve_laravel_string_key(
                self,
                &LaravelStringKind::View,
                name,
            ) {
                let template_uri = location.uri.to_string();
                if !self
                    .blade_virtual_content
                    .read()
                    .contains_key(&template_uri)
                {
                    continue;
                }
                let Some(content) = self.get_file_content(&template_uri) else {
                    continue;
                };
                if self.reinfer_and_reparse_blade(&template_uri, &content) {
                    self.schedule_diagnostics(template_uri);
                }
            }
        }
    }

    /// The Blade-caller equivalent of [`Self::refresh_blade_inference_for_caller`]:
    /// re-run component-tag inference for the templates a *Blade* file's own
    /// `<x-…>` tags reference, after it was edited or saved, so an updated
    /// attribute is reflected without waiting for the referenced
    /// component's own next parse.
    fn refresh_blade_component_inference_for_caller(&self, caller_uri: &str) {
        let Some(content) = self.get_file_content(caller_uri) else {
            return;
        };
        let tags = crate::blade::component_tags::referenced_component_tags(&content);
        if tags.is_empty() {
            return;
        }

        for tag in tags {
            let view_name = crate::blade::component_tags::view_name_for_component_tag(&tag);
            for location in crate::virtual_members::laravel::resolve_laravel_string_key(
                self,
                &LaravelStringKind::View,
                &view_name,
            ) {
                let template_uri = location.uri.to_string();
                if template_uri == caller_uri
                    || !self
                        .blade_virtual_content
                        .read()
                        .contains_key(&template_uri)
                {
                    continue;
                }
                let Some(template_content) = self.get_file_content(&template_uri) else {
                    continue;
                };
                if self.reinfer_and_reparse_blade(&template_uri, &template_content) {
                    self.schedule_diagnostics(template_uri);
                }
            }
        }
    }

    /// Re-run inference for the templates whose layout chain runs through
    /// a Blade file, after it was edited or saved, so a `@var` added to a
    /// layout reaches its children without waiting for each child's own
    /// next parse.
    ///
    /// The walk goes *down* the chain rather than reading every template's
    /// ancestors: each template's own `@extends` target is read once, then
    /// the set of affected view names grows a level per round until no
    /// template joins it. A template that extends a template that extends
    /// the edited layout inherits from it too.
    fn refresh_blade_layout_children(&self, layout_uri: &str) {
        let mut frontier = self.view_names_for_blade_uri(layout_uri);
        if frontier.is_empty() {
            return;
        }
        let mut pending: Vec<(String, Arc<String>, Vec<String>)> = self
            .blade_caller_snapshot()
            .into_iter()
            .filter(|(uri, _)| uri != layout_uri)
            .filter_map(|(uri, content)| {
                let extends = crate::blade::signature::extract_extends(&content);
                (!extends.is_empty()).then_some((uri, content, extends))
            })
            .collect();

        while !frontier.is_empty() && !pending.is_empty() {
            let (children, rest): (Vec<_>, Vec<_>) =
                pending.into_iter().partition(|(_, _, extends)| {
                    extends
                        .iter()
                        .any(|extends| frontier.iter().any(|name| name == extends))
                });
            pending = rest;
            frontier = Vec::new();
            for (uri, content, _) in children {
                frontier.extend(self.view_names_for_blade_uri(&uri));
                if self.reinfer_and_reparse_blade(&uri, &content) {
                    self.schedule_diagnostics(uri);
                }
            }
        }
    }

    /// Derive the view names a Blade file is addressable by: one per
    /// configured view root that contains it, in dot notation, plus
    /// `namespace::name` forms for provider-registered directories.
    pub(crate) fn view_names_for_blade_uri(&self, uri: &str) -> Vec<String> {
        let Ok(url) = tower_lsp::lsp_types::Url::parse(uri) else {
            return Vec::new();
        };
        let Ok(path) = url.to_file_path() else {
            return Vec::new();
        };

        let mut names = Vec::new();
        let mut push_name = |rel: &std::path::Path, namespace: &str| {
            let rel_str = rel.to_string_lossy();
            let stripped = rel_str
                .strip_suffix(".blade.php")
                .or_else(|| rel_str.strip_suffix(".php"));
            if let Some(stem) = stripped {
                let name = stem.replace(['/', '\\'], ".");
                if namespace.is_empty() {
                    names.push(name);
                } else {
                    names.push(format!("{namespace}::{name}"));
                }
            }
        };

        // `path` came from a file URI and is absolute; a view root can
        // be relative when the workspace root was given relative (the
        // analyse CLI passes `--project-root` through as-is), so
        // canonicalize each root before comparing.
        for root in self.laravel_view_roots() {
            let root = root.canonicalize().unwrap_or(root);
            if let Ok(rel) = path.strip_prefix(&root) {
                push_name(rel, "");
            }
        }
        for res in &self.laravel_provider_resources.read().view_dirs {
            let res_path = res.path.canonicalize().unwrap_or_else(|_| res.path.clone());
            if let Ok(rel) = path.strip_prefix(&res_path) {
                push_name(rel, &res.namespace);
            }
        }
        names
    }

    /// Parse one caller file and extract the variables passed to the
    /// template at each `view('name', …)` span offset.
    ///
    /// `offsets` are the byte offsets of the view-name string contents
    /// (as recorded in the symbol map); a call site matches when the
    /// span of one of its string arguments starts at one of them.
    ///
    /// Everything a single site passes lands in one [`ResolvedViewCall`],
    /// including the entries a chained `->with(…)` adds, so a caller that
    /// builds its data over several calls is still judged as one.
    pub(crate) fn extract_call_site_vars(
        &self,
        uri: &str,
        content: &str,
        offsets: &[u32],
    ) -> Vec<ResolvedViewCall> {
        let file_ctx = self.file_context(uri);
        let class_loader = self.class_loader(&file_ctx);
        let function_loader = self.function_loader(&file_ctx);
        let function_loader_cl = |name: &str, offset: u32| function_loader(name, offset);

        with_parsed_program(content, "blade_call_site_inference", |program, content| {
            let default_class = ClassInfo::default();

            // Collect the matching call expressions first, then resolve
            // types — both inside the closure so AST references never
            // outlive the arena.
            let mut collected: Vec<SiteDraft<'_, '_>> = Vec::new();
            let walker = ViewCallWalker { offsets };
            let mut ctx = CollectCtx {
                sites: &mut collected,
            };
            for stmt in program.statements.iter() {
                mago_syntax::walker::Walker::walk_statement(&walker, stmt, &mut ctx);
            }

            let mut result = Vec::new();
            for site in collected {
                let enclosing =
                    crate::class_lookup::find_class_at_offset(&file_ctx.classes, site.offset);
                let current_class = enclosing.unwrap_or(&default_class);
                let loaders = Loaders::with_function(Some(&function_loader_cl));
                let var_ctx = VarResolutionCtx {
                    var_name: "",
                    top_level_scope: None,
                    current_class,
                    all_classes: &file_ctx.classes,
                    content,
                    cursor_offset: site.offset,
                    class_loader: &class_loader,
                    backend: Some(self),
                    loaders,
                    resolved_class_cache: Some(&self.resolved_class_cache),
                    enclosing_return_type: None,
                    branch_aware: false,
                    match_arm_narrowing: HashMap::new(),
                    scope_var_resolver: None,
                };

                let mut vars: Vec<PassedVar> = Vec::new();
                for entry in site.entries {
                    let (name, key_range, value_range, ty) = match entry {
                        SiteEntry::Expr {
                            name,
                            key_range,
                            expr,
                        } => {
                            let span = expr.span();
                            let ty = crate::type_engine::variable::foreach_resolution::resolve_expression_type(
                                expr, &var_ctx,
                            )
                            .unwrap_or_else(PhpType::mixed);
                            (name, key_range, (span.start.offset, span.end.offset), ty)
                        }
                        SiteEntry::Variable { name, key_range } => {
                            let loaders = Loaders::with_function(Some(&function_loader_cl));
                            let ty = crate::type_engine::variable::resolution::resolve_variable_php_type(
                                &name,
                                content,
                                site.offset,
                                Some(current_class),
                                &file_ctx.classes,
                                &class_loader,
                                Some(self),
                                loaders,
                            )
                            .unwrap_or_else(PhpType::mixed);
                            (name, key_range, key_range, ty)
                        }
                    };
                    // Render FQNs so the injected `@var` resolves from the
                    // template's namespace-less context.
                    let ty = ty.resolve_names(&|name: &str| {
                        if let Some(cls) = class_loader(name) {
                            format!("\\{}", cls.fqn())
                        } else {
                            name.to_string()
                        }
                    });
                    vars.push(PassedVar {
                        name,
                        ty,
                        key_range,
                        value_range,
                    });
                }
                result.push(ResolvedViewCall {
                    name_range: site.name_range,
                    vars,
                    complete: site.complete,
                });
            }
            result
        })
    }

    /// Extract the variables one Blade caller passes to component tags,
    /// given the tag occurrences [`super::component_tags::scan_component_tag_calls`]
    /// already found in its raw source.
    ///
    /// `virtual_php` is the caller's own preprocessed content: a bound
    /// attribute on *any* HTML tag compiles down to a `blade_directive(EXPR)`
    /// call, in document order, so an occurrence's
    /// [`super::component_tags::ComponentTagCall::bound`] indices index
    /// directly into that call sequence — no Blade-to-PHP offset
    /// translation needed.
    fn extract_component_call_site_vars(
        &self,
        uri: &str,
        virtual_php: &str,
        occurrences: Vec<crate::blade::component_tags::ComponentTagCall>,
    ) -> Vec<InferredVars> {
        let file_ctx = self.file_context(uri);
        let class_loader = self.class_loader(&file_ctx);
        let function_loader = self.function_loader(&file_ctx);
        let function_loader_cl = |name: &str, offset: u32| function_loader(name, offset);

        with_parsed_program(
            virtual_php,
            "blade_component_call_site",
            |program, content| {
                let default_class = ClassInfo::default();

                let mut ctx = BladeDirectiveCollectCtx { calls: Vec::new() };
                let walker = BladeDirectiveWalker;
                for stmt in program.statements.iter() {
                    mago_syntax::walker::Walker::walk_statement(&walker, stmt, &mut ctx);
                }
                let calls = ctx.calls;

                let mut result = Vec::new();
                for occurrence in occurrences {
                    let mut vars: InferredVars = occurrence.literal;
                    for (name, index) in occurrence.bound {
                        let Some(expr) = calls.get(index).copied() else {
                            continue;
                        };
                        let offset = expr.span().start.offset;
                        let enclosing =
                            crate::class_lookup::find_class_at_offset(&file_ctx.classes, offset);
                        let current_class = enclosing.unwrap_or(&default_class);
                        let loaders = Loaders::with_function(Some(&function_loader_cl));
                        let var_ctx = VarResolutionCtx {
                            var_name: "",
                            top_level_scope: None,
                            current_class,
                            all_classes: &file_ctx.classes,
                            content,
                            cursor_offset: offset,
                            class_loader: &class_loader,
                            backend: Some(self),
                            loaders,
                            resolved_class_cache: Some(&self.resolved_class_cache),
                            enclosing_return_type: None,
                            branch_aware: false,
                            match_arm_narrowing: HashMap::new(),
                            scope_var_resolver: None,
                        };
                        let ty = crate::type_engine::variable::foreach_resolution::resolve_expression_type(
                        expr, &var_ctx,
                    )
                    .unwrap_or_else(PhpType::mixed);
                        let ty = ty.resolve_names(&|name: &str| {
                            if let Some(cls) = class_loader(name) {
                                format!("\\{}", cls.fqn())
                            } else {
                                name.to_string()
                            }
                        });
                        vars.push((name, ty));
                    }
                    if !vars.is_empty() {
                        result.push(vars);
                    }
                }
                result
            },
        )
    }
}

// ─── AST walking ────────────────────────────────────────────────────────────

/// Collects every `blade_directive(EXPR)` call in a Blade file's virtual
/// PHP, in document order. The preprocessor emits exactly one such call
/// per bound HTML attribute (see `super::preprocessor`), so this order
/// matches the order `super::component_tags::scan_component_tag_calls`
/// counts bound attributes in.
struct BladeDirectiveCollectCtx<'ast, 'arena> {
    calls: Vec<&'ast Expression<'arena>>,
}

struct BladeDirectiveWalker;

impl<'ast, 'arena> mago_syntax::walker::Walker<'ast, 'arena, BladeDirectiveCollectCtx<'ast, 'arena>>
    for BladeDirectiveWalker
{
    fn walk_in_function_call(
        &self,
        node: &'ast FunctionCall<'arena>,
        ctx: &mut BladeDirectiveCollectCtx<'ast, 'arena>,
    ) {
        let Expression::Identifier(ident) = node.function else {
            return;
        };
        if bytes_to_str(ident.value()) != "blade_directive" {
            return;
        }
        if let Some(arg) = node.argument_list.arguments.iter().next() {
            ctx.calls.push(arg.value());
        }
    }
}

/// One variable passed at a call site: either the value expression
/// (array entry / `->with()` value) or, for `compact('name')`, the
/// same-named variable to resolve at the call-site offset.
enum SiteEntry<'ast, 'arena> {
    Expr {
        name: String,
        key_range: ByteRange,
        expr: &'ast Expression<'arena>,
    },
    Variable {
        name: String,
        key_range: ByteRange,
    },
}

/// One call site as the walker finds it, before its entries are resolved.
struct SiteDraft<'ast, 'arena> {
    /// The offset of the `view()` call itself, which every chained
    /// `->with(…)` is folded into and which types resolve at.
    offset: u32,
    name_range: ByteRange,
    entries: Vec<SiteEntry<'ast, 'arena>>,
    complete: bool,
}

struct CollectCtx<'w, 'ast, 'arena> {
    sites: &'w mut Vec<SiteDraft<'ast, 'arena>>,
}

impl<'ast, 'arena> CollectCtx<'_, 'ast, 'arena> {
    /// The draft for the view call at `offset`, created if the walker has
    /// not reached it yet.
    ///
    /// A chained `->with(…)` is walked *before* the `view()` call it hangs
    /// off (the method call is the outer node), so either end of the chain
    /// may be the first to open the site.
    fn site(&mut self, offset: u32, name_range: ByteRange) -> &mut SiteDraft<'ast, 'arena> {
        if let Some(index) = self.sites.iter().position(|site| site.offset == offset) {
            return &mut self.sites[index];
        }
        self.sites.push(SiteDraft {
            offset,
            name_range,
            entries: Vec::new(),
            complete: true,
        });
        self.sites.last_mut().expect("just pushed")
    }
}

/// Walker that finds `view('name', …)` / `View::make('name', …)` calls
/// whose view-name string contents sit at one of the requested offsets,
/// and collects the data entries they pass: the array literal /
/// `compact()` call that follows the name, plus any `->with(…)` chained
/// onto the call.
struct ViewCallWalker<'a> {
    offsets: &'a [u32],
}

impl ViewCallWalker<'_> {
    /// The range of the view-name string in an argument list, along with
    /// the index it sits at, when the list names one of the views asked
    /// about.
    ///
    /// The name is looked for at any position rather than only the first:
    /// `Route::view('/about', 'pages.about', …)` puts the URI first, and
    /// the data argument is always the one after the name whatever the
    /// helper's shape.
    fn matches(&self, argument_list: &ArgumentList<'_>) -> Option<(usize, ByteRange)> {
        argument_list
            .arguments
            .iter()
            .enumerate()
            .find_map(|(index, argument)| {
                let Expression::Literal(Literal::String(s)) = argument.value() else {
                    return None;
                };
                let inner = (s.span.start.offset + 1, s.span.end.offset - 1);
                self.offsets.contains(&inner.0).then_some((index, inner))
            })
    }
}

impl<'ast, 'arena, 'w> mago_syntax::walker::Walker<'ast, 'arena, CollectCtx<'w, 'ast, 'arena>>
    for ViewCallWalker<'_>
{
    fn walk_in_function_call(
        &self,
        node: &'ast FunctionCall<'arena>,
        ctx: &mut CollectCtx<'w, 'ast, 'arena>,
    ) {
        let Expression::Identifier(ident) = node.function else {
            return;
        };
        let name = crate::util::strip_fqn_prefix(bytes_to_str(ident.value()));
        if !is_view_render_function(name) {
            return;
        }
        let Some((index, name_range)) = self.matches(&node.argument_list) else {
            return;
        };
        let mut entries = Vec::new();
        let complete = collect_data_argument(&node.argument_list, index + 1, &mut entries);
        let site = ctx.site(node.span().start.offset, name_range);
        site.entries.extend(entries);
        site.complete &= complete;
    }

    fn walk_in_static_method_call(
        &self,
        node: &'ast StaticMethodCall<'arena>,
        ctx: &mut CollectCtx<'w, 'ast, 'arena>,
    ) {
        let ClassLikeMemberSelector::Identifier(method) = &node.method else {
            return;
        };
        if !is_view_render_static_call(node.class, bytes_to_str(method.value)) {
            return;
        }
        let Some((index, name_range)) = self.matches(&node.argument_list) else {
            return;
        };
        let mut entries = Vec::new();
        let complete = collect_data_argument(&node.argument_list, index + 1, &mut entries);
        let site = ctx.site(node.span().start.offset, name_range);
        site.entries.extend(entries);
        site.complete &= complete;
    }

    fn walk_in_method_call(
        &self,
        node: &'ast MethodCall<'arena>,
        ctx: &mut CollectCtx<'w, 'ast, 'arena>,
    ) {
        // `->with('key', $value)` / `->with(['key' => $value])` /
        // `->withKey($value)` chained onto a matching `view()` call.  The
        // receiver chain may pass through other builder methods
        // (`->layout(…)`), so scan the whole spine for the matching view
        // call.
        let ClassLikeMemberSelector::Identifier(method) = &node.method else {
            return;
        };
        let method_name = bytes_to_str(method.value);
        if !method_name.starts_with("with") && !method_name.starts_with("With") {
            return;
        }
        let Some((offset, name_range)) = matching_view_call_in_chain(node.object, self) else {
            return;
        };

        let mut entries = Vec::new();
        let mut complete = true;
        // `->withUser($user)` is Laravel's magic setter for `$user`; the
        // name is the method's own tail, so the method identifier is what
        // a diagnostic points at.
        if let Some(magic) = magic_with_name(method_name) {
            match node.argument_list.arguments.iter().next() {
                Some(value) => entries.push(SiteEntry::Expr {
                    name: magic,
                    key_range: (method.span.start.offset, method.span.end.offset),
                    expr: value.value(),
                }),
                None => complete = false,
            }
        } else {
            let mut args = node.argument_list.arguments.iter();
            match (args.next(), args.next()) {
                (Some(key_arg), Some(value_arg)) => {
                    // ->with('key', $value)
                    match key_arg.value() {
                        Expression::Literal(Literal::String(s)) => {
                            match string_literal_contents(s) {
                                Some(name) => entries.push(SiteEntry::Expr {
                                    name,
                                    key_range: (s.span.start.offset + 1, s.span.end.offset - 1),
                                    expr: value_arg.value(),
                                }),
                                None => complete = false,
                            }
                        }
                        _ => complete = false,
                    }
                }
                (Some(single), None) => {
                    // ->with(['key' => $value, …]) or ->with(compact('key'))
                    complete = collect_from_data_expr(single.value(), &mut entries);
                }
                _ => complete = false,
            }
        }

        let site = ctx.site(offset, name_range);
        site.entries.extend(entries);
        site.complete &= complete;
    }
}

/// Whether a helper function renders a view named by one of its string
/// arguments.
///
/// `blade_view_directive` is what the preprocessor compiles Blade's own
/// `@include` family, `@extends`, `@component`, and `@each` into, so a
/// template rendering another template is judged by the same rules a
/// controller is.
fn is_view_render_function(name: &str) -> bool {
    name.eq_ignore_ascii_case("view") || name.eq_ignore_ascii_case("blade_view_directive")
}

/// Whether a static call renders a view: `View::make()`, or the
/// `Route::view()` shorthand that binds a URI straight to a template.
fn is_view_render_static_call(class: &Expression<'_>, method: &str) -> bool {
    let Expression::Identifier(ident) = class else {
        return false;
    };
    let subject = crate::util::strip_fqn_prefix(bytes_to_str(ident.value()));
    let is_facade = |short: &str, fqn: &str| {
        subject.eq_ignore_ascii_case(short) || subject.eq_ignore_ascii_case(fqn)
    };
    (is_facade("View", "Illuminate\\Support\\Facades\\View") && method.eq_ignore_ascii_case("make"))
        || (is_facade("Route", "Illuminate\\Support\\Facades\\Route")
            && method.eq_ignore_ascii_case("view"))
}

/// The variable a `->withSomething()` magic setter names, following
/// Laravel's `View::__call()` (`with` plus the camel-cased tail).
///
/// Plain `->with(…)` is not a magic setter, and neither is a method whose
/// tail does not start a new word (`->within(…)`).
fn magic_with_name(method: &str) -> Option<String> {
    let rest = method
        .strip_prefix("with")
        .or_else(|| method.strip_prefix("With"))?;
    let mut chars = rest.chars();
    let first = chars.next().filter(|ch| ch.is_uppercase())?;
    Some(first.to_lowercase().chain(chars).collect())
}

/// The offset and view-name range of the `view()` / `View::make()` call a
/// method call's receiver spine ends in, when it names one of the views
/// asked about.
///
/// Walks through chained method calls (`view(…)->with(…)->with(…)`) but
/// not through variables — a `$view = view(…); $view->with(…)` split is
/// out of scope.
fn matching_view_call_in_chain(
    mut expr: &Expression<'_>,
    walker: &ViewCallWalker<'_>,
) -> Option<(u32, ByteRange)> {
    loop {
        match expr {
            Expression::Call(Call::Function(fc)) => {
                let Expression::Identifier(ident) = fc.function else {
                    return None;
                };
                let name = crate::util::strip_fqn_prefix(bytes_to_str(ident.value()));
                if !is_view_render_function(name) {
                    return None;
                }
                let (_, range) = walker.matches(&fc.argument_list)?;
                return Some((fc.span().start.offset, range));
            }
            Expression::Call(Call::StaticMethod(sc)) => {
                let ClassLikeMemberSelector::Identifier(method) = &sc.method else {
                    return None;
                };
                if !is_view_render_static_call(sc.class, bytes_to_str(method.value)) {
                    return None;
                }
                let (_, range) = walker.matches(&sc.argument_list)?;
                return Some((sc.span().start.offset, range));
            }
            Expression::Call(Call::Method(mc)) => {
                expr = mc.object;
            }
            Expression::Parenthesized(p) => {
                expr = p.expression;
            }
            _ => return None,
        }
    }
}

/// Collect variable entries from the data argument at `index` of a
/// `view()` / `View::make()` argument list.
///
/// Returns whether the argument was readable in full: an absent one
/// passes nothing (readable), while one built from a variable or a
/// non-literal key hides names the caller does pass.
fn collect_data_argument<'ast, 'arena>(
    argument_list: &'ast ArgumentList<'arena>,
    index: usize,
    entries: &mut Vec<SiteEntry<'ast, 'arena>>,
) -> bool {
    match argument_list.arguments.iter().nth(index) {
        Some(arg) => collect_from_data_expr(arg.value(), entries),
        None => true,
    }
}

/// Collect entries from a data expression: an array literal with
/// string keys, or a `compact('a', 'b')` call (whose values are the
/// same-named variables at the call site).
///
/// Returns whether every entry of the expression was readable.
fn collect_from_data_expr<'ast, 'arena>(
    expr: &'ast Expression<'arena>,
    entries: &mut Vec<SiteEntry<'ast, 'arena>>,
) -> bool {
    let mut collect_array_elements =
        |elements: &'ast TokenSeparatedSequence<'arena, ArrayElement<'arena>>| {
            let mut complete = true;
            for element in elements.iter() {
                let ArrayElement::KeyValue(kv) = element else {
                    // A spread, or a positional entry Blade's `extract()`
                    // would drop: either way the key set is not the one
                    // written here.
                    complete = false;
                    continue;
                };
                let Expression::Literal(Literal::String(s)) = kv.key else {
                    complete = false;
                    continue;
                };
                match string_literal_contents(s) {
                    Some(name) => entries.push(SiteEntry::Expr {
                        name,
                        key_range: (s.span.start.offset + 1, s.span.end.offset - 1),
                        expr: kv.value,
                    }),
                    None => complete = false,
                }
            }
            complete
        };
    match expr {
        Expression::Array(array) => collect_array_elements(&array.elements),
        Expression::LegacyArray(array) => collect_array_elements(&array.elements),
        Expression::Call(Call::Function(fc)) => {
            let Expression::Identifier(ident) = fc.function else {
                return false;
            };
            let name = crate::util::strip_fqn_prefix(bytes_to_str(ident.value()));
            if !name.eq_ignore_ascii_case("compact") {
                return false;
            }
            let mut complete = true;
            for arg in fc.argument_list.arguments.iter() {
                match arg.value() {
                    Expression::Literal(Literal::String(s)) => match string_literal_contents(s) {
                        Some(name) => entries.push(SiteEntry::Variable {
                            name,
                            key_range: (s.span.start.offset + 1, s.span.end.offset - 1),
                        }),
                        None => complete = false,
                    },
                    _ => complete = false,
                }
            }
            complete
        }
        _ => false,
    }
}

/// The contents of a single- or double-quoted string literal, when it
/// is a plain identifier-safe name.
pub(crate) fn string_literal_contents(s: &LiteralString<'_>) -> Option<String> {
    let value = s.value.map(bytes_to_str)?;
    if value.is_empty()
        || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || value.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(value.to_string())
}
