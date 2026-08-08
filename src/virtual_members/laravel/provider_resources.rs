use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_names::resolver::NameResolver;
use mago_span::HasSpan;
use mago_syntax::cst::*;

use super::const_eval::{ClassContext, Scope, const_string};
use crate::atom::bytes_to_str;
use crate::names::OwnedResolvedNames;

#[derive(Debug, Clone)]
pub(crate) struct ProviderResource {
    pub path: PathBuf,
    pub namespace: String,
}

/// How a service provider came to be registered, which decides whose binding
/// the container ends up with when two providers bind the same string key.
///
/// The order mirrors `Application::registerConfiguredProviders()`, which
/// registers the `Illuminate\*` entries of the configured list first, then the
/// providers vendor packages auto-discover, then everything else the
/// application lists.  Each registration replaces the one before it, so a
/// higher variant wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) enum ProviderOrigin {
    /// An `Illuminate\*` provider: a framework default.
    #[default]
    Framework,
    /// Auto-discovered from a vendor package's `extra.laravel.providers`.
    Package,
    /// Listed by the application in `bootstrap/providers.php` or the
    /// `providers` key of `config/app.php`.
    Application,
}

/// The provider a scan is reading, as far as binding precedence goes.
#[derive(Debug, Clone, Default)]
pub(crate) struct ProviderIdentity {
    /// The provider class's FQN.
    pub fqn: String,
    /// The provider classes it extends.  Subclassing a provider and re-binding
    /// one of its keys is how a replacement is written, so the parent must not
    /// win the key back merely by being scanned later.
    pub ancestors: Vec<String>,
    pub origin: ProviderOrigin,
}

impl ProviderIdentity {
    /// Whether a binding this provider made survives one `other` makes for the
    /// same key.
    ///
    /// Providers are scanned in registration order and each registration
    /// replaces the one before it, so the tie goes to `other` unless this
    /// provider is registered later (a higher [`ProviderOrigin`]) or extends
    /// it.
    fn outranks(&self, other: &ProviderIdentity) -> bool {
        match self.origin.cmp(&other.origin) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => self.ancestors.iter().any(|parent| parent == &other.fqn),
        }
    }
}

/// The concrete class behind a container key, with the provider that put it
/// there so a provider scanned later is weighed against it rather than
/// overwriting it outright.
#[derive(Debug, Clone)]
pub(crate) struct Binding {
    pub class: String,
    provider: Arc<ProviderIdentity>,
}

/// An `alias('other-key', 'key')` entry: the key it stands for, and the
/// provider that named it, so precedence applies to aliases too.
#[derive(Debug, Clone)]
pub(crate) struct Alias {
    target: String,
    provider: Arc<ProviderIdentity>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProviderResources {
    pub config_files: Vec<ProviderResource>,
    pub view_dirs: Vec<ProviderResource>,
    pub trans_dirs: Vec<ProviderResource>,
    pub route_files: Vec<PathBuf>,
    /// Container binding key → the class bound to it, for the bindings a
    /// provider makes under a *string* abstract
    /// (`$this->app->singleton('sentry', fn () => new HubAdapter())`).  A
    /// binding keyed by `Contract::class` needs no table: the written name
    /// already resolves.
    pub bindings: HashMap<String, Binding>,
    /// `alias('other-key', 'key')` entries, as key → the key they stand for.
    /// These name a binding by another *string* key rather than by a class,
    /// so they are folded into `bindings` by [`Self::resolve_aliases`] once
    /// every provider has been scanned: the key an alias points at may well
    /// be bound by a provider that comes later.
    pub aliases: HashMap<String, Alias>,
    /// `Blade::componentNamespace('Nightshade\Views\Components', 'nightshade')`
    /// entries, as (tag prefix, class namespace).  A view addressed under the
    /// prefix (`nightshade::calendar`) is backed by a component class in that
    /// namespace, whose members its template reads.
    pub class_component_namespaces: Vec<(String, String)>,
    /// A provider rebound `translator` or `translation.loader` to something
    /// other than Laravel's own file-based pair, so the strings come from a
    /// source we cannot enumerate (a database table, say) and the set of
    /// valid translation keys is unknowable.
    pub custom_translation_loader: bool,
}

impl ProviderResources {
    pub fn merge(&mut self, other: ProviderResources) {
        self.config_files.extend(other.config_files);
        self.view_dirs.extend(other.view_dirs);
        self.trans_dirs.extend(other.trans_dirs);
        self.route_files.extend(other.route_files);
        self.class_component_namespaces
            .extend(other.class_component_namespaces);
        for (key, binding) in other.bindings {
            self.record_binding(key, binding);
        }
        for (key, alias) in other.aliases {
            match self.aliases.entry(key) {
                Entry::Occupied(mut slot) => {
                    if !slot.get().provider.outranks(&alias.provider) {
                        slot.insert(alias);
                    }
                }
                Entry::Vacant(slot) => {
                    slot.insert(alias);
                }
            }
        }
        self.custom_translation_loader |= other.custom_translation_loader;
    }

    /// Bind `key`, unless a provider that outranks this one already claimed it.
    ///
    /// Two providers binding the same key is the normal way an application
    /// swaps a framework or package implementation out, so the key has to end
    /// up with the class the container would hold once every provider has
    /// registered.
    fn record_binding(&mut self, key: String, binding: Binding) {
        match self.bindings.entry(key) {
            Entry::Occupied(mut slot) => {
                if !slot.get().provider.outranks(&binding.provider) {
                    slot.insert(binding);
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(binding);
            }
        }
    }

    /// Give every alias the concrete class of the key it stands for.
    ///
    /// Laravel resolves an alias by following it until it reaches a key that
    /// is not itself aliased, so a chain (`'a'` → `'b'` → a bound class) is
    /// followed here too.  The walk is bounded by the number of aliases, which
    /// leaves a cycle unresolved instead of looping forever.
    pub fn resolve_aliases(&mut self) {
        let limit = self.aliases.len();
        let resolved: Vec<(String, Binding)> = self
            .aliases
            .iter()
            .filter_map(|(key, alias)| {
                let mut target = &alias.target;
                for _ in 0..limit {
                    match self.aliases.get(target) {
                        Some(next) => target = &next.target,
                        None => break,
                    }
                }
                let concrete = self.bindings.get(target)?;
                Some((
                    key.clone(),
                    Binding {
                        class: concrete.class.clone(),
                        provider: Arc::clone(&alias.provider),
                    },
                ))
            })
            .collect();
        // The container consults its alias table before its bindings, so an
        // alias decides the key it covers, subject to the same precedence as
        // any other registration.
        for (key, binding) in resolved {
            self.record_binding(key, binding);
        }
    }
}

/// Container keys whose binding decides where translation strings come from.
const TRANSLATION_BINDINGS: [&str; 2] = ["translator", "translation.loader"];

/// The classes Laravel's own `TranslationServiceProvider` binds those keys
/// to.  A factory that builds anything else reads its lines from somewhere
/// other than the `lang/` directories we scan.
const FILE_TRANSLATION_CLASSES: [&str; 2] = ["FileLoader", "Translator"];

/// Container methods that put a new value behind a key.
const BINDING_METHODS: &[&[u8]] = &[
    b"bind",
    b"bindif",
    b"singleton",
    b"singletonif",
    b"scoped",
    b"scopedif",
    b"instance",
    b"extend",
];

/// Scan a service provider for the resources it registers.
///
/// `class_context` carries the provider class's own constants and static
/// property defaults, merged over its parent chain, so a binding key written
/// as `static::$abstract` folds to the string it holds.  `provider` records
/// which provider this is, so a key two providers bind ends up with the class
/// the container would hold.
pub(crate) fn extract_provider_resources(
    content: &str,
    file_path: &Path,
    workspace_root: &Path,
    class_context: ClassContext,
    provider: Arc<ProviderIdentity>,
) -> ProviderResources {
    let mut resources = ProviderResources::default();
    let scope = Scope::for_class(class_context);
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
    // Container bindings name their concrete by short name (`new HubAdapter()`
    // under a `use` statement), so the file's resolved-name table is needed to
    // turn that into the FQN the class index is keyed by.
    let resolved = OwnedResolvedNames::from_resolved(&NameResolver::new(&arena).resolve(program));

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

        // `Blade::componentNamespace('Nightshade\\Views\\Components',
        // 'nightshade')` says which classes back the views a package
        // registers under its own prefix.
        if let Expression::Call(Call::StaticMethod(sc)) = expr
            && let ClassLikeMemberSelector::Identifier(method) = &sc.method
            && method.value.eq_ignore_ascii_case(b"componentNamespace")
            && let Expression::Identifier(id) = sc.class
            && id
                .value()
                .rsplit(|&b| b == b'\\')
                .next()
                .is_some_and(|seg| seg.eq_ignore_ascii_case(b"Blade"))
            && let Some(entry) = component_namespace_args(&sc.argument_list, content, &scope)
        {
            resources.class_component_namespaces.push(entry);
            return ControlFlow::Continue(());
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
                program,
            )
        {
            grouped_route_files.push(path);
            return ControlFlow::Continue(());
        }

        // `$this->app->alias(Concrete::class, 'key')` gives an existing
        // binding another name.  The arguments read the other way round from
        // `bind()`: the key is the second one, and the first names what it
        // stands for.
        if method_lower == b"alias"
            && is_app_container_expr(mc.object)
            && let Some(target) = mc.argument_list.arguments.iter().next()
            && let Some(key) = mc
                .argument_list
                .arguments
                .iter()
                .nth(1)
                .and_then(|arg| alias_key(arg.value(), content, &scope, &resolved))
        {
            match binding_concrete(Some(target.value()), &resolved) {
                Some(concrete) => {
                    resources.bindings.insert(
                        key,
                        Binding {
                            class: concrete,
                            provider: Arc::clone(&provider),
                        },
                    );
                }
                // The target is another string key, whose own binding may not
                // have been scanned yet.
                None => {
                    if let Some(aliased) = const_string(target.value(), content, &scope) {
                        resources.aliases.insert(
                            key,
                            Alias {
                                target: aliased,
                                provider: Arc::clone(&provider),
                            },
                        );
                    }
                }
            }
            return ControlFlow::Continue(());
        }

        // `$this->app->singleton('translation.loader', …)` and friends decide
        // where translation lines come from, and the container is reached
        // through `$this->app`, not `$this`, so this is checked ahead of the
        // `$this->…` resource loaders below.
        if BINDING_METHODS.contains(&method_lower.as_slice())
            && is_app_container_expr(mc.object)
            && let Some(key_arg) = mc.argument_list.arguments.iter().next()
            && let Some(key) = const_string(key_arg.value(), content, &scope)
        {
            let factory = mc.argument_list.arguments.iter().nth(1).map(|a| a.value());

            // A translation binding decides where the strings come from, on top
            // of naming a class: anything but Laravel's own file-based pair
            // reads its lines from a source we cannot enumerate.  `extend`
            // decorates whatever is already bound, so even a file-based wrapper
            // adds lines from somewhere else.
            if TRANSLATION_BINDINGS.contains(&key.as_str())
                && (method_lower == b"extend" || !builds_file_translator(factory))
            {
                resources.custom_translation_loader = true;
            }

            // `extend` wraps whatever the key already holds; which class comes
            // out depends on the binding it decorates, so only the calls that
            // *replace* the value tell us the concrete type.
            if method_lower != b"extend"
                && let Some(concrete) = binding_concrete(factory, &resolved)
            {
                resources.bindings.insert(
                    key,
                    Binding {
                        class: concrete,
                        provider: Arc::clone(&provider),
                    },
                );
            }
            return ControlFlow::Continue(());
        }

        // The same registration written against the compiler instance a
        // deferred callback receives (`$blade->componentNamespace(…)`),
        // which is how a package registers before Blade is resolved.
        if method_lower == b"componentnamespace"
            && let Some(entry) = component_namespace_args(&mc.argument_list, content, &scope)
        {
            resources.class_component_namespaces.push(entry);
            return ControlFlow::Continue(());
        }

        if !is_this_expr(mc.object) {
            return ControlFlow::Continue(());
        }

        let args: Vec<_> = mc.argument_list.arguments.iter().collect();

        if method_lower == b"mergeconfigfrom" && args.len() >= 2 {
            if let Some(path) =
                resolve_path_arg(args[0].value(), content, file_dir, workspace_root, program)
                && let Some((ns, _, _)) =
                    super::helpers::extract_string_literal(args[1].value(), content)
            {
                resources.config_files.push(ProviderResource {
                    path,
                    namespace: ns.to_string(),
                });
            }
        } else if method_lower == b"loadviewsfrom" && args.len() >= 2 {
            if let Some(path) =
                resolve_path_arg(args[0].value(), content, file_dir, workspace_root, program)
                && let Some((ns, _, _)) =
                    super::helpers::extract_string_literal(args[1].value(), content)
            {
                resources.view_dirs.push(ProviderResource {
                    path,
                    namespace: ns.to_string(),
                });
            }
        } else if method_lower == b"loadtranslationsfrom" && args.len() >= 2 {
            if let Some(path) =
                resolve_path_arg(args[0].value(), content, file_dir, workspace_root, program)
                && let Some((ns, _, _)) =
                    super::helpers::extract_string_literal(args[1].value(), content)
            {
                resources.trans_dirs.push(ProviderResource {
                    path,
                    namespace: ns.to_string(),
                });
            }
        } else if method_lower == b"loadjsontranslationsfrom" && !args.is_empty() {
            if let Some(path) =
                resolve_path_arg(args[0].value(), content, file_dir, workspace_root, program)
            {
                resources.trans_dirs.push(ProviderResource {
                    path,
                    namespace: String::new(),
                });
            }
        } else if method_lower == b"loadroutesfrom"
            && !args.is_empty()
            && let Some(path) =
                resolve_path_arg(args[0].value(), content, file_dir, workspace_root, program)
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

/// The (tag prefix, class namespace) pair a `componentNamespace()` call
/// registers, as `(prefix, namespace)`.
///
/// The namespace is read from source text, where a single-quoted literal
/// still carries its doubled separators, so those are collapsed back to
/// the namespace the application sees.
fn component_namespace_args(
    argument_list: &ArgumentList<'_>,
    content: &str,
    scope: &Scope,
) -> Option<(String, String)> {
    let mut args = argument_list.arguments.iter();
    let namespace = const_string(args.next()?.value(), content, scope)?;
    let prefix = const_string(args.next()?.value(), content, scope)?;
    let namespace = namespace
        .replace("\\\\", "\\")
        .trim_matches('\\')
        .to_string();
    if namespace.is_empty() || prefix.is_empty() {
        return None;
    }
    Some((prefix, namespace))
}

fn is_this_expr(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Variable(Variable::Direct(dv)) if dv.name == b"$this"
    )
}

/// Whether `expr` names the service container: `$this->app` in a provider
/// method, the `$app` a deferred callback receives, or the `app()` helper.
fn is_app_container_expr(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => dv.name == b"$app",
        Expression::Access(Access::Property(pa)) => {
            is_this_expr(pa.object)
                && matches!(
                    &pa.property,
                    ClassLikeMemberSelector::Identifier(ident)
                        if ident.value.eq_ignore_ascii_case(b"app")
                )
        }
        Expression::Call(Call::Function(fc)) => {
            matches!(fc.function, Expression::Identifier(id)
                if id.value()
                    .rsplit(|&b| b == b'\\')
                    .next()
                    .is_some_and(|seg| seg.eq_ignore_ascii_case(b"app")))
                && fc.argument_list.arguments.is_empty()
        }
        _ => false,
    }
}

/// The concrete class a container binding puts behind its key.
///
/// Covers the shapes a service provider writes: the class itself
/// (`bind('foo', Foo::class)`), a ready-made instance
/// (`instance('foo', new Foo())`), and the usual factory
/// (`singleton('foo', fn () => new Foo())` or its closure equivalent, whose
/// first `return` decides the type).  A factory that hands back anything else
/// (a container lookup, a variable, a conditional) yields `None`: guessing
/// there would bind the key to a class the application never resolves.
fn binding_concrete(
    expr: Option<&Expression<'_>>,
    resolved: &OwnedResolvedNames,
) -> Option<String> {
    let expr = expr?;
    match expr {
        Expression::Instantiation(inst) => match inst.class {
            Expression::Identifier(id) => resolved_class_fqn(id, resolved),
            _ => None,
        },
        Expression::Access(Access::ClassConstant(_)) => class_string_fqn(expr, resolved),
        Expression::ArrowFunction(arrow) => binding_concrete(Some(arrow.expression), resolved),
        Expression::Closure(closure) => {
            closure.body.statements.iter().find_map(|stmt| match stmt {
                Statement::Return(ret) => binding_concrete(ret.value, resolved),
                _ => None,
            })
        }
        Expression::Parenthesized(inner) => binding_concrete(Some(inner.expression), resolved),
        _ => None,
    }
}

/// The container key an `alias()` argument names.
///
/// An alias is keyed either by a string (`alias(HubInterface::class,
/// 'sentry')`) or by a contract's name (`alias('sentry',
/// HubInterface::class)`), and both sides of the call accept both forms.
fn alias_key(
    expr: &Expression<'_>,
    content: &str,
    scope: &Scope,
    resolved: &OwnedResolvedNames,
) -> Option<String> {
    const_string(expr, content, scope).or_else(|| class_string_fqn(expr, resolved))
}

/// The FQN an `X::class` expression spells, or `None` for any other
/// expression.
fn class_string_fqn(expr: &Expression<'_>, resolved: &OwnedResolvedNames) -> Option<String> {
    let Expression::Access(Access::ClassConstant(access)) = expr else {
        return None;
    };
    if !matches!(
        &access.constant,
        ClassLikeConstantSelector::Identifier(constant)
            if constant.value.eq_ignore_ascii_case(b"class")
    ) {
        return None;
    }
    match access.class {
        Expression::Identifier(id) => resolved_class_fqn(id, resolved),
        _ => None,
    }
}

/// The FQN a class-name identifier resolves to, through the file's namespace
/// and `use` statements, falling back to the written name when the resolver
/// did not track the offset.
fn resolved_class_fqn(ident: &Identifier<'_>, resolved: &OwnedResolvedNames) -> Option<String> {
    if let Some(fqn) = resolved.get(ident.span().start.offset) {
        return Some(fqn.trim_start_matches('\\').to_string());
    }
    let raw = bytes_to_str(ident.value()).trim_start_matches('\\');
    (!raw.is_empty()).then(|| raw.to_string())
}

/// Whether a translation binding's factory builds Laravel's own file-based
/// translator, i.e. every class it names is one of `FILE_TRANSLATION_CLASSES`.
///
/// A factory that reaches for anything else has moved the lines out of the
/// `lang/` directories, and one that names no class at all (a container
/// lookup, a variable) says nothing either way, which is equally unknowable.
fn builds_file_translator(factory: Option<&Expression<'_>>) -> bool {
    let Some(factory) = factory else {
        return false;
    };

    let mut named_any = false;
    let mut all_file_based = true;
    super::helpers::walk_expression_tree(factory, &mut |expr| {
        if let Some(name) = instantiated_or_class_string(expr) {
            named_any = true;
            if !FILE_TRANSLATION_CLASSES
                .iter()
                .any(|known| crate::util::short_name(name).eq_ignore_ascii_case(known))
            {
                all_file_based = false;
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    });

    named_any && all_file_based
}

/// The class an expression names, either by instantiating it (`new X(…)`) or
/// by referring to it as a string (`X::class`).
///
/// An empty name means the expression names a class that is only known at
/// runtime (`new $loaderClass`), which no more resolves to Laravel's own
/// loader than an explicit replacement does.
fn instantiated_or_class_string<'arena>(expr: &Expression<'arena>) -> Option<&'arena str> {
    let class = match expr {
        Expression::Instantiation(inst) => inst.class,
        Expression::Access(Access::ClassConstant(access))
            if matches!(
                &access.constant,
                ClassLikeConstantSelector::Identifier(constant)
                    if constant.value.eq_ignore_ascii_case(b"class")
            ) =>
        {
            access.class
        }
        _ => return None,
    };
    match class {
        Expression::Identifier(id) => Some(crate::atom::bytes_to_str(id.value())),
        _ => Some(""),
    }
}

/// Resolve an expression that names a file to the path it points at.
///
/// Covers the forms Laravel projects use to locate route, config, view, and
/// translation files: `__DIR__ . '/…'`, `base_path('…')`, a bare literal
/// (absolute, or relative to the referring file), and a local variable
/// assigned one of those forms earlier in the same scope (Livewire's
/// service provider writes `$config = __DIR__.'/../config/x.php';` before
/// passing `$config` to `mergeConfigFrom`).  `program` is the parse of
/// `content`, which that last form is resolved against.
pub(crate) fn resolve_path_arg(
    expr: &Expression<'_>,
    content: &str,
    file_dir: &Path,
    workspace_root: &Path,
    program: &Program<'_>,
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
        let assigned = last_assignment_before(program, dv.start_offset(), dv.name)?;
        return resolve_path_arg(assigned, content, file_dir, workspace_root, program);
    }

    None
}

/// The RHS of the last `$name = <expr>;` assignment before `offset` in the
/// scope enclosing it: PHP's own resolution rule for a variable read, the
/// most recent write to it in the same scope.
///
/// A service provider assigns inside a method; a route file assigns at the
/// top level of the script, where the enclosing scope is the file itself.
fn last_assignment_before<'ast, 'arena>(
    program: &'ast Program<'arena>,
    offset: u32,
    name: &[u8],
) -> Option<&'ast Expression<'arena>> {
    let mut best: Option<(u32, &'ast Expression<'arena>)> = None;
    let mut record = |node: Node<'ast, 'arena>| {
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
    };

    match super::helpers::enclosing_body(Node::Program(program), offset) {
        Some(body) => super::helpers::walk_before_cursor(body, offset, &mut record),
        None => super::helpers::walk_file_scope_before_cursor(
            Node::Program(program),
            offset,
            &mut record,
        ),
    }
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
    fn records_registered_class_component_namespaces() {
        // Both shapes a package registers its components with: the facade,
        // and the compiler instance a deferred callback receives.  A
        // single-quoted namespace keeps its doubled separators in source,
        // so the recorded value must be the namespace itself.
        let content = "<?php\n\
            class PackageServiceProvider {\n\
                public function boot(): void {\n\
                    Blade::componentNamespace('Nightshade\\\\Views\\\\Components', 'nightshade');\n\
                    $this->callAfterResolving('blade.compiler', function ($blade) {\n\
                        $blade->componentNamespace('Acme\\\\Ui', 'acme');\n\
                    });\n\
                }\n\
            }\n";
        let resources = extract_provider_resources(
            content,
            Path::new("/ws/app/Providers/PackageServiceProvider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert_eq!(
            resources.class_component_namespaces,
            vec![
                (
                    "nightshade".to_string(),
                    "Nightshade\\Views\\Components".to_string()
                ),
                ("acme".to_string(), "Acme\\Ui".to_string()),
            ]
        );
    }

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
        let resources = extract_provider_resources(
            content,
            file_path,
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
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
        let resources = extract_provider_resources(
            content,
            file_path,
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
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
        let resources = extract_provider_resources(
            content,
            file_path,
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
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
        let resources = extract_provider_resources(
            content,
            Path::new("/ws/Provider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
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
        let resources = extract_provider_resources(
            content,
            file_path,
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert_eq!(resources.config_files.len(), 1);
        assert_eq!(
            resources.config_files[0].path,
            Path::new("/ws/vendor/livewire/livewire/src").join("../config/livewire.php")
        );
        assert_eq!(resources.config_files[0].namespace, "livewire");
    }

    #[test]
    fn detects_a_database_backed_translation_loader() {
        // An application that keeps its strings in a database still builds a
        // FileLoader to hand to its own loader, so the decision has to follow
        // what the factory *returns*, not merely which classes it mentions.
        let content = "<?php\n\
            class TranslationServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('translation.loader', function ($app) {\n\
                        $fileLoader = new FileLoader($app->make('files'), $app->make('path.lang'));\n\
                        return new DatabaseTranslationLoader($fileLoader);\n\
                    });\n\
                }\n\
            }\n";
        let resources = extract_provider_resources(
            content,
            Path::new("/ws/src/TranslationServiceProvider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert!(resources.custom_translation_loader);
    }

    #[test]
    fn detects_a_replaced_translator() {
        let content = "<?php\n\
            class TranslationServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('translator', fn ($app) => new DatabaseTranslator(\n\
                        $app->make('translation.loader'),\n\
                        $app->getLocale(),\n\
                    ));\n\
                }\n\
            }\n";
        let resources = extract_provider_resources(
            content,
            Path::new("/ws/src/TranslationServiceProvider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert!(resources.custom_translation_loader);
    }

    #[test]
    fn laravels_own_translation_bindings_are_not_a_replacement() {
        // Laravel's own TranslationServiceProvider is itself scanned when the
        // project lists the framework providers in `config/app.php`.  Reading
        // its bindings as a replacement would silence translation diagnostics
        // for every Laravel project.
        let content = "<?php\n\
            class TranslationServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('translator', function ($app) {\n\
                        $loader = $app['translation.loader'];\n\
                        $trans = new Translator($loader, $app->getLocale());\n\
                        $trans->setFallback($app->getFallbackLocale());\n\
                        return $trans;\n\
                    });\n\
                    $this->registerLoader();\n\
                }\n\
                protected function registerLoader(): void {\n\
                    $this->app->singleton('translation.loader', function ($app) {\n\
                        return new FileLoader($app['files'], [__DIR__.'/lang', $app['path.lang']]);\n\
                    });\n\
                }\n\
            }\n";
        let resources = extract_provider_resources(
            content,
            Path::new(
                "/ws/vendor/laravel/framework/src/Illuminate/Translation/TranslationServiceProvider.php",
            ),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert!(!resources.custom_translation_loader);
    }

    #[test]
    fn a_decorated_translation_loader_counts_as_a_replacement() {
        // `extend` wraps whatever is already bound, so the lines it serves are
        // not limited to the ones on disk even when the wrapper is file-based.
        let content = "<?php\n\
            class CacheTranslationServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->extend('translation.loader', fn ($loader) => new FileLoader($loader));\n\
                }\n\
            }\n";
        let resources = extract_provider_resources(
            content,
            Path::new("/ws/src/CacheTranslationServiceProvider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert!(resources.custom_translation_loader);
    }

    #[test]
    fn unrelated_container_bindings_are_ignored() {
        let content = "<?php\n\
            class AppServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('sentry', fn () => new HubAdapter());\n\
                    $this->app->bind(Contract::class, Implementation::class);\n\
                }\n\
            }\n";
        let resources = extract_provider_resources(
            content,
            Path::new("/ws/src/AppServiceProvider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert!(!resources.custom_translation_loader);
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
        let resources = extract_provider_resources(
            content,
            file_path,
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        assert_eq!(resources.config_files.len(), 1);
        assert!(
            resources.view_dirs.is_empty(),
            "an undefined `$path` in a different method must not resolve to another method's assignment"
        );
    }

    /// The fold table a provider whose base class declares
    /// `public static $abstract = 'sentry';` is scanned with, as
    /// `provider_class_context` builds it from the inheritance-merged class.
    fn sentry_context() -> ClassContext {
        let mut class = crate::test_fixtures::make_class("ServiceProvider");
        let mut abstract_key = crate::test_fixtures::make_property("abstract", Some("string"));
        abstract_key.is_static = true;
        abstract_key.source = Some(crate::types::PropertySource::DeclaredDefault {
            value: "'sentry'".into(),
        });
        class.properties = vec![std::sync::Arc::new(abstract_key)].into();

        let mut version = crate::test_fixtures::make_constant("VERSION");
        version.value = Some("'4.0'".to_string());
        class.constants = vec![std::sync::Arc::new(version)].into();

        ClassContext::from_class(&class)
    }

    fn scan_sentry_provider(content: &str) -> ProviderResources {
        let mut resources = extract_provider_resources(
            content,
            Path::new("/ws/vendor/sentry/sentry-laravel/src/Sentry/Laravel/ServiceProvider.php"),
            Path::new("/ws"),
            sentry_context(),
            Default::default(),
        );
        resources.resolve_aliases();
        resources
    }

    #[test]
    fn binds_a_key_named_by_an_inherited_static_property() {
        // Sentry declares the container key on the base provider and binds
        // under `static::$abstract` from the subclass the application
        // registers, so `app('sentry')` only resolves once the property folds.
        let content = "<?php\n\
            namespace Sentry\\Laravel;\n\
            class ServiceProvider extends BaseServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton(static::$abstract, fn () => new HubAdapter());\n\
                }\n\
            }\n";
        let resources = scan_sentry_provider(content);
        assert_eq!(
            resources.bindings.get("sentry").map(|b| b.class.as_str()),
            Some("Sentry\\Laravel\\HubAdapter")
        );
    }

    #[test]
    fn binds_a_key_built_from_a_class_constant() {
        let content = "<?php\n\
            namespace Sentry\\Laravel;\n\
            class ServiceProvider extends BaseServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton(self::VERSION . '.hub', fn () => new HubAdapter());\n\
                }\n\
            }\n";
        let resources = scan_sentry_provider(content);
        assert_eq!(
            resources.bindings.get("4.0.hub").map(|b| b.class.as_str()),
            Some("Sentry\\Laravel\\HubAdapter")
        );
    }

    #[test]
    fn a_key_named_by_another_class_does_not_fold() {
        // `Unrelated::$abstract` names a class this scan never read; borrowing
        // the scanned provider's property of that name would bind the wrong key.
        let content = "<?php\n\
            namespace Sentry\\Laravel;\n\
            class ServiceProvider extends BaseServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton(Unrelated::$abstract, fn () => new HubAdapter());\n\
                }\n\
            }\n";
        assert!(scan_sentry_provider(content).bindings.is_empty());
    }

    #[test]
    fn alias_binds_its_second_argument_to_the_class_in_its_first() {
        // `alias()` takes its arguments the other way round from `bind()`.
        let content = "<?php\n\
            namespace Sentry\\Laravel;\n\
            use Sentry\\State\\HubInterface;\n\
            class ServiceProvider extends BaseServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->alias(HubInterface::class, static::$abstract);\n\
                }\n\
            }\n";
        let resources = scan_sentry_provider(content);
        assert_eq!(
            resources.bindings.get("sentry").map(|b| b.class.as_str()),
            Some("Sentry\\State\\HubInterface")
        );
    }

    #[test]
    fn alias_to_another_key_resolves_to_that_keys_concrete() {
        // Aliasing an already-bound string key is how a package exposes its
        // service under a contract name as well.
        let content = "<?php\n\
            namespace Sentry\\Laravel;\n\
            use Sentry\\State\\HubInterface;\n\
            class ServiceProvider extends BaseServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton(static::$abstract, fn () => new HubAdapter());\n\
                    $this->app->alias(static::$abstract, HubInterface::class);\n\
                }\n\
            }\n";
        let resources = scan_sentry_provider(content);
        assert_eq!(
            resources
                .bindings
                .get("Sentry\\State\\HubInterface")
                .map(|b| b.class.as_str()),
            Some("Sentry\\Laravel\\HubAdapter"),
            "the contract name has to reach the class the aliased key is bound to"
        );
    }

    #[test]
    fn an_alias_cycle_leaves_its_keys_unresolved() {
        let content = "<?php\n\
            class AppServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->alias('a', 'b');\n\
                    $this->app->alias('b', 'a');\n\
                }\n\
            }\n";
        let mut resources = extract_provider_resources(
            content,
            Path::new("/ws/src/AppServiceProvider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Default::default(),
        );
        resources.resolve_aliases();
        assert!(resources.bindings.is_empty());
    }

    /// Scan a provider under the registration origin and parent chain that
    /// decide the keys it shares with another provider.
    fn scan_as(
        content: &str,
        fqn: &str,
        ancestors: &[&str],
        origin: ProviderOrigin,
    ) -> ProviderResources {
        extract_provider_resources(
            content,
            Path::new("/ws/src/Provider.php"),
            Path::new("/ws"),
            ClassContext::default(),
            Arc::new(ProviderIdentity {
                fqn: fqn.to_string(),
                ancestors: ancestors.iter().map(|a| a.to_string()).collect(),
                origin,
            }),
        )
    }

    const FRAMEWORK_TRANSLATION_PROVIDER: &str = "<?php\n\
        namespace Illuminate\\Translation;\n\
        class TranslationServiceProvider {\n\
            public function register(): void {\n\
                $this->app->singleton('translator', fn ($app) => new Translator($app));\n\
            }\n\
        }\n";

    const APP_TRANSLATION_PROVIDER: &str = "<?php\n\
        namespace Acme\\Translation;\n\
        class TranslationServiceProvider extends \\Illuminate\\Translation\\TranslationServiceProvider {\n\
            public function register(): void {\n\
                $this->app->singleton('translator', fn ($app) => new DatabaseTranslator($app));\n\
            }\n\
        }\n";

    #[test]
    fn an_application_binding_beats_a_framework_default() {
        // Replacing a framework binding from an application provider is the
        // normal way to swap an implementation, and the two providers may be
        // scanned in either order, so neither order may hand the key back to
        // the framework.
        let framework = || {
            scan_as(
                FRAMEWORK_TRANSLATION_PROVIDER,
                "Illuminate\\Translation\\TranslationServiceProvider",
                &[],
                ProviderOrigin::Framework,
            )
        };
        let application = || {
            scan_as(
                APP_TRANSLATION_PROVIDER,
                "Acme\\Translation\\TranslationServiceProvider",
                &["Illuminate\\Translation\\TranslationServiceProvider"],
                ProviderOrigin::Application,
            )
        };

        let mut framework_first = ProviderResources::default();
        framework_first.merge(framework());
        framework_first.merge(application());

        let mut application_first = ProviderResources::default();
        application_first.merge(application());
        application_first.merge(framework());

        for resources in [framework_first, application_first] {
            assert_eq!(
                resources
                    .bindings
                    .get("translator")
                    .map(|b| b.class.as_str()),
                Some("Acme\\Translation\\DatabaseTranslator"),
                "the application's registration decides the key"
            );
        }
    }

    #[test]
    fn a_subclass_provider_beats_the_parent_it_extends() {
        let parent = "<?php\n\
            namespace Acme;\n\
            class ServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('acme.client', fn () => new Client());\n\
                }\n\
            }\n";
        let child = "<?php\n\
            namespace App\\Providers;\n\
            class AcmeServiceProvider extends \\Acme\\ServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('acme.client', fn () => new TracingClient());\n\
                }\n\
            }\n";

        // Both are registered the same way, and the subclass is scanned first,
        // so only its parent chain marks it as the later registration.
        let mut resources = ProviderResources::default();
        resources.merge(scan_as(
            child,
            "App\\Providers\\AcmeServiceProvider",
            &["Acme\\ServiceProvider"],
            ProviderOrigin::Application,
        ));
        resources.merge(scan_as(
            parent,
            "Acme\\ServiceProvider",
            &[],
            ProviderOrigin::Application,
        ));

        assert_eq!(
            resources
                .bindings
                .get("acme.client")
                .map(|b| b.class.as_str()),
            Some("App\\Providers\\TracingClient")
        );
    }

    #[test]
    fn two_unrelated_providers_leave_the_key_to_the_later_one() {
        // Nothing ranks one above the other, and the container keeps whichever
        // registered last, which is the order they are scanned in.
        let first = "<?php\n\
            namespace A;\n\
            class ServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('shared', fn () => new First());\n\
                }\n\
            }\n";
        let second = "<?php\n\
            namespace B;\n\
            class ServiceProvider {\n\
                public function register(): void {\n\
                    $this->app->singleton('shared', fn () => new Second());\n\
                }\n\
            }\n";

        let mut resources = ProviderResources::default();
        resources.merge(scan_as(
            first,
            "A\\ServiceProvider",
            &[],
            ProviderOrigin::Application,
        ));
        resources.merge(scan_as(
            second,
            "B\\ServiceProvider",
            &[],
            ProviderOrigin::Application,
        ));

        assert_eq!(
            resources.bindings.get("shared").map(|b| b.class.as_str()),
            Some("B\\Second")
        );
    }

    #[test]
    fn a_replaced_translator_binds_the_key_to_the_replacement() {
        // Rebinding `translator` says both that the strings come from a source
        // we cannot enumerate *and* which class `app('translator')` hands back.
        let resources = scan_as(
            APP_TRANSLATION_PROVIDER,
            "Acme\\Translation\\TranslationServiceProvider",
            &["Illuminate\\Translation\\TranslationServiceProvider"],
            ProviderOrigin::Application,
        );
        assert!(resources.custom_translation_loader);
        assert_eq!(
            resources
                .bindings
                .get("translator")
                .map(|b| b.class.as_str()),
            Some("Acme\\Translation\\DatabaseTranslator")
        );
    }
}
