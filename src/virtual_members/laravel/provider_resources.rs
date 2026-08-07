use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::cst::*;

#[derive(Debug, Clone)]
pub(crate) struct ProviderResource {
    pub path: PathBuf,
    pub namespace: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProviderResources {
    pub config_files: Vec<ProviderResource>,
    pub view_dirs: Vec<ProviderResource>,
    pub trans_dirs: Vec<ProviderResource>,
    pub route_files: Vec<PathBuf>,
}

impl ProviderResources {
    pub fn merge(&mut self, other: ProviderResources) {
        self.config_files.extend(other.config_files);
        self.view_dirs.extend(other.view_dirs);
        self.trans_dirs.extend(other.trans_dirs);
        self.route_files.extend(other.route_files);
    }
}

pub(crate) fn extract_provider_resources(
    content: &str,
    file_path: &Path,
    workspace_root: &Path,
) -> ProviderResources {
    let mut resources = ProviderResources::default();
    let file_dir = file_path.parent().unwrap_or(file_path);
    // Route files reached through `Route::…->group('path')`.  They are only
    // kept when the provider turns out not to register any routes inline:
    // an inline registration means the provider itself is scanned as a route
    // source, and that scan reaches the same files *with* the name and URI
    // prefixes their enclosing group applies.
    let mut grouped_route_files: Vec<PathBuf> = Vec::new();
    let mut registers_routes_inline = false;

    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());

    super::helpers::walk_program_expressions(program, &mut |expr| {
        // Any direct use of the `Route` facade means routes are registered
        // from this file rather than only pointed at.
        if let Expression::Call(Call::StaticMethod(sc)) = expr
            && let Expression::Identifier(id) = sc.class
            && id
                .value()
                .rsplit(|&b| b == b'\\')
                .next()
                .is_some_and(|seg| seg.eq_ignore_ascii_case(b"Route"))
        {
            registers_routes_inline = true;
        }

        let Expression::Call(Call::Method(mc)) = expr else {
            return ControlFlow::Continue(());
        };

        let ClassLikeMemberSelector::Identifier(ident) = &mc.method else {
            return ControlFlow::Continue(());
        };

        let method_lower = ident.value.to_ascii_lowercase();

        // `Route::middleware(...)->group(base_path('routes/web.php'))` registers
        // a route file without `$this->loadRoutesFrom(...)`.  The `->group()`
        // argument is either a closure (inline routes, ignored here) or a path
        // to a file whose routes we must scan.
        if method_lower == b"group"
            && chain_roots_at_route(mc.object)
            && let Some(first_arg) = mc.argument_list.arguments.iter().next()
            && let Some(path) = resolve_path_arg(
                first_arg.value(),
                content,
                file_dir,
                workspace_root,
                Some(program),
            )
        {
            grouped_route_files.push(path);
            return ControlFlow::Continue(());
        }

        if !is_this_expr(mc.object) {
            return ControlFlow::Continue(());
        }

        let args: Vec<_> = mc.argument_list.arguments.iter().collect();

        if method_lower == b"mergeconfigfrom" && args.len() >= 2 {
            if let Some(path) = resolve_path_arg(
                args[0].value(),
                content,
                file_dir,
                workspace_root,
                Some(program),
            ) && let Some((ns, _, _)) =
                super::helpers::extract_string_literal(args[1].value(), content)
            {
                resources.config_files.push(ProviderResource {
                    path,
                    namespace: ns.to_string(),
                });
            }
        } else if method_lower == b"loadviewsfrom" && args.len() >= 2 {
            if let Some(path) = resolve_path_arg(
                args[0].value(),
                content,
                file_dir,
                workspace_root,
                Some(program),
            ) && let Some((ns, _, _)) =
                super::helpers::extract_string_literal(args[1].value(), content)
            {
                resources.view_dirs.push(ProviderResource {
                    path,
                    namespace: ns.to_string(),
                });
            }
        } else if method_lower == b"loadtranslationsfrom" && args.len() >= 2 {
            if let Some(path) = resolve_path_arg(
                args[0].value(),
                content,
                file_dir,
                workspace_root,
                Some(program),
            ) && let Some((ns, _, _)) =
                super::helpers::extract_string_literal(args[1].value(), content)
            {
                resources.trans_dirs.push(ProviderResource {
                    path,
                    namespace: ns.to_string(),
                });
            }
        } else if method_lower == b"loadjsontranslationsfrom" && !args.is_empty() {
            if let Some(path) = resolve_path_arg(
                args[0].value(),
                content,
                file_dir,
                workspace_root,
                Some(program),
            ) {
                resources.trans_dirs.push(ProviderResource {
                    path,
                    namespace: String::new(),
                });
            }
        } else if method_lower == b"loadroutesfrom"
            && !args.is_empty()
            && let Some(path) = resolve_path_arg(
                args[0].value(),
                content,
                file_dir,
                workspace_root,
                Some(program),
            )
        {
            resources.route_files.push(path);
        }

        ControlFlow::Continue(())
    });

    if registers_routes_inline {
        resources.route_files.push(file_path.to_path_buf());
    } else {
        resources.route_files.extend(grouped_route_files);
    }

    resources
}

fn is_this_expr(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Variable(Variable::Direct(dv)) if dv.name == b"$this"
    )
}

/// Resolve an expression that names a file to the path it points at.
///
/// Covers the forms Laravel projects use to locate route, config, view, and
/// translation files: `__DIR__ . '/…'`, `base_path('…')`, a bare literal
/// (absolute, or relative to the referring file), and a local variable
/// assigned one of those forms earlier in the same method (Livewire's
/// service provider writes `$config = __DIR__.'/../config/x.php';` before
/// passing `$config` to `mergeConfigFrom`).  `program` is only needed for
/// that last form, so callers that cannot easily thread it through (e.g. a
/// route-file `require` reached mid-traversal) may pass `None` and simply
/// lose local-variable resolution.
pub(crate) fn resolve_path_arg(
    expr: &Expression<'_>,
    content: &str,
    file_dir: &Path,
    workspace_root: &Path,
    program: Option<&Program<'_>>,
) -> Option<PathBuf> {
    if let Some(rel) = super::helpers::extract_dir_concat_path(expr, content) {
        let resolved = file_dir.join(rel.trim_start_matches('/'));
        return resolved.canonicalize().ok().or(Some(resolved));
    }

    // `base_path('app/.../web.php')` resolves relative to the workspace root.
    if let Expression::Call(Call::Function(fc)) = expr
        && let Expression::Identifier(id) = fc.function
        && id
            .value()
            .rsplit(|&b| b == b'\\')
            .next()
            .is_some_and(|seg| seg.eq_ignore_ascii_case(b"base_path"))
        && let Some(first_arg) = fc.argument_list.arguments.iter().next()
        && let Some((val, _, _)) =
            super::helpers::extract_string_literal(first_arg.value(), content)
    {
        let resolved = workspace_root.join(val.trim_start_matches('/'));
        return resolved.canonicalize().ok().or(Some(resolved));
    }

    if let Some((val, _, _)) = super::helpers::extract_string_literal(expr, content) {
        if val.starts_with('/') {
            let p = PathBuf::from(val);
            return p.canonicalize().ok().or(Some(p));
        }
        let resolved = file_dir.join(val);
        return resolved.canonicalize().ok().or(Some(resolved));
    }

    if let Expression::Variable(Variable::Direct(dv)) = expr {
        let program = program?;
        let assigned = last_assignment_before(program, dv.start_offset(), dv.name)?;
        return resolve_path_arg(assigned, content, file_dir, workspace_root, Some(program));
    }

    None
}

/// The RHS of the last `$name = <expr>;` assignment before `offset` in the
/// enclosing method or function body: PHP's own resolution rule for a
/// variable read, the most recent write to it in the same function scope.
fn last_assignment_before<'ast, 'arena>(
    program: &'ast Program<'arena>,
    offset: u32,
    name: &[u8],
) -> Option<&'ast Expression<'arena>> {
    let body = super::helpers::enclosing_body(Node::Program(program), offset)?;

    let mut best: Option<(u32, &Expression<'_>)> = None;
    super::helpers::walk_before_cursor(body, offset, &mut |node| {
        let Node::Assignment(assignment) = node else {
            return;
        };
        if !assignment.operator.is_assign() {
            return;
        }
        let Expression::Variable(Variable::Direct(target)) = assignment.lhs else {
            return;
        };
        if target.name != name {
            return;
        }
        let end = node.span().end.offset;
        if super::helpers::beats_best(&best, end, offset) {
            best = Some((end, assignment.rhs));
        }
    });
    best.map(|(_, rhs)| rhs)
}

/// Check whether an instance-method call chain roots at the `Route` facade,
/// i.e. `Route::middleware(...)->namespace(...)->…`.  Walks down the `->object`
/// chain until it reaches the static entry point and matches its class name.
fn chain_roots_at_route(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Call(Call::Method(mc)) => chain_roots_at_route(mc.object),
        Expression::Call(Call::StaticMethod(sc)) => {
            if let Expression::Identifier(id) = sc.class {
                id.value()
                    .rsplit(|&b| b == b'\\')
                    .next()
                    .is_some_and(|seg| seg.eq_ignore_ascii_case(b"Route"))
            } else {
                false
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_route_group_base_path_registration() {
        // A RouteServiceProvider that registers routes via the fluent
        // `Route::middleware(...)->group(base_path('...'))` API rather than
        // `$this->loadRoutesFrom(...)`.  Because the provider touches the
        // `Route` facade it is itself the route source: scanning it applies
        // the group's prefixes to the file it points at.
        let content = "<?php\n\
            class RouteServiceProvider {\n\
                protected function mapWebRoutes(): void {\n\
                    Route::middleware('web')\n\
                        ->namespace($this->namespace)\n\
                        ->group(base_path('app/Contexts/Backoffice/Routes/web.php'));\n\
                }\n\
            }\n";
        let file_path = Path::new("/ws/app/Providers/RouteServiceProvider.php");
        let resources = extract_provider_resources(content, file_path, Path::new("/ws"));
        assert_eq!(
            resources.route_files,
            vec![file_path.to_path_buf()],
            "a provider that uses the Route facade is scanned as a route source"
        );
    }

    #[test]
    fn treats_inline_route_registration_as_a_route_source() {
        // An inline `Route::group(function () { ... })` registers its routes
        // in the provider itself, so the provider is the file to scan.
        let content = "<?php\n\
            Route::middleware('web')->group(function () {\n\
                Route::get('/')->name('home');\n\
            });\n";
        let file_path = Path::new("/ws/app/Providers/RouteServiceProvider.php");
        let resources = extract_provider_resources(content, file_path, Path::new("/ws"));
        assert_eq!(resources.route_files, vec![file_path.to_path_buf()]);
    }

    #[test]
    fn still_detects_load_routes_from() {
        // The existing `$this->loadRoutesFrom(__DIR__ . '/routes.php')` path
        // must keep working alongside the new fluent detection.
        let content = "<?php\n\
            class PackageServiceProvider {\n\
                public function boot(): void {\n\
                    $this->loadRoutesFrom(__DIR__ . '/../routes/pkg.php');\n\
                }\n\
            }\n";
        let file_path = Path::new("/ws/vendor/acme/src/PackageServiceProvider.php");
        let resources = extract_provider_resources(content, file_path, Path::new("/ws"));
        assert_eq!(
            resources.route_files,
            vec![Path::new("/ws/vendor/acme/src").join("../routes/pkg.php")],
            "loadRoutesFrom must still be detected"
        );
    }

    #[test]
    fn ignores_non_route_facade_group() {
        // A `->group()` call whose chain does not root at the Route facade
        // must not be misread as a route-file registration.
        let content = "<?php\n\
            Blade::directive('x')->group(base_path('resources/views'));\n";
        let resources =
            extract_provider_resources(content, Path::new("/ws/Provider.php"), Path::new("/ws"));
        assert!(resources.route_files.is_empty());
    }

    #[test]
    fn resolves_config_path_behind_a_local_variable() {
        // Livewire's own service provider assigns the path to a local
        // variable before passing it to `mergeConfigFrom`, rather than
        // writing the `__DIR__ . '...'` concatenation inline.
        let content = "<?php\n\
            class LivewireServiceProvider {\n\
                protected function registerConfig(): void {\n\
                    $config = __DIR__.'/../config/livewire.php';\n\
                    $this->mergeConfigFrom($config, 'livewire');\n\
                }\n\
            }\n";
        let file_path = Path::new("/ws/vendor/livewire/livewire/src/LivewireServiceProvider.php");
        let resources = extract_provider_resources(content, file_path, Path::new("/ws"));
        assert_eq!(resources.config_files.len(), 1);
        assert_eq!(
            resources.config_files[0].path,
            Path::new("/ws/vendor/livewire/livewire/src").join("../config/livewire.php")
        );
        assert_eq!(resources.config_files[0].namespace, "livewire");
    }

    #[test]
    fn local_variable_scan_stays_within_its_own_method() {
        // `$path` is assigned in `registerConfig` but `registerViews` never
        // assigns it: resolving `registerViews`'s `$path` must not pick up
        // the other method's assignment.
        let content = "<?php\n\
            class PackageServiceProvider {\n\
                public function registerConfig(): void {\n\
                    $path = __DIR__.'/../config/a.php';\n\
                    $this->mergeConfigFrom($path, 'a');\n\
                }\n\
                public function registerViews(): void {\n\
                    $this->loadViewsFrom($path, 'b');\n\
                }\n\
            }\n";
        let file_path = Path::new("/ws/vendor/acme/src/PackageServiceProvider.php");
        let resources = extract_provider_resources(content, file_path, Path::new("/ws"));
        assert_eq!(resources.config_files.len(), 1);
        assert!(
            resources.view_dirs.is_empty(),
            "an undefined `$path` in a different method must not resolve to another method's assignment"
        );
    }
}
