//! Type-driven classification for Laravel named-resource receivers.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::atom::{Atom, AtomSet};
use crate::php_type::{PhpType, TypeKind};
use crate::symbol_map::{LaravelConfigResource, LaravelResourceReceiverRule, LaravelStringKind};
use crate::types::{ClassInfo, MAX_INHERITANCE_DEPTH};

const SHOULD_QUEUE: &str = "Illuminate\\Contracts\\Queue\\ShouldQueue";
const QUEUEABLE_TRAIT: &str = "Illuminate\\Bus\\Queueable";

/// Classify a type-dependent resource call without guessing from its method
/// name. The same function serves lazy symbol spans and live completion.
pub(crate) fn classify_receiver_type(
    rule: LaravelResourceReceiverRule,
    ty: &PhpType,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<LaravelStringKind> {
    match rule {
        LaravelResourceReceiverRule::ConnectionMethod => {
            let resource = classify_connection_receiver(ty, class_loader)?;
            Some(LaravelStringKind::ConfigResource(resource))
        }
        LaravelResourceReceiverRule::QueueableConnection => {
            type_is_queueable(ty, class_loader, false).then_some(LaravelStringKind::ConfigResource(
                LaravelConfigResource::QueueConnection,
            ))
        }
        LaravelResourceReceiverRule::QueueName => {
            type_is_queueable(ty, class_loader, true).then_some(LaravelStringKind::QueueName)
        }
        LaravelResourceReceiverRule::ConnectionProperty => None,
    }
}

fn classify_connection_receiver(
    ty: &PhpType,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<LaravelConfigResource> {
    use LaravelConfigResource::{BroadcastConnection, DatabaseConnection, QueueConnection};

    match ty.kind() {
        TypeKind::Nullable(inner) => return classify_connection_receiver(inner, class_loader),
        TypeKind::Union(members) => {
            let mut resource = None;
            for member in members.iter().filter(|member| !member.is_null()) {
                let candidate = classify_connection_receiver(member, class_loader)?;
                if resource.is_some_and(|known| known != candidate) {
                    return None;
                }
                resource = Some(candidate);
            }
            return resource;
        }
        _ => {}
    }

    if crate::class_lookup::is_subtype_of_named(
        ty,
        "Illuminate\\Database\\ConnectionResolverInterface",
        class_loader,
    ) || crate::class_lookup::is_subtype_of_named(
        ty,
        "Illuminate\\Database\\Eloquent\\Factories\\Factory",
        class_loader,
    ) {
        Some(DatabaseConnection)
    } else if crate::class_lookup::is_subtype_of_named(
        ty,
        "Illuminate\\Contracts\\Queue\\Factory",
        class_loader,
    ) {
        Some(QueueConnection)
    } else if crate::class_lookup::is_subtype_of_named(
        ty,
        "Illuminate\\Contracts\\Broadcasting\\Factory",
        class_loader,
    ) {
        Some(BroadcastConnection)
    } else {
        None
    }
}

/// Classify a `$connection` property by the class that declares it.
pub(crate) fn classify_connection_property(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<LaravelStringKind> {
    if crate::virtual_members::laravel::extends_eloquent_model(class, class_loader) {
        Some(LaravelStringKind::ConfigResource(
            LaravelConfigResource::DatabaseConnection,
        ))
    } else if class_is_queueable(class, class_loader) {
        Some(LaravelStringKind::ConfigResource(
            LaravelConfigResource::QueueConnection,
        ))
    } else {
        None
    }
}

fn type_is_queueable(
    ty: &PhpType,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    allow_mailer: bool,
) -> bool {
    match ty.kind() {
        TypeKind::Nullable(inner) => {
            return type_is_queueable(inner, class_loader, allow_mailer);
        }
        TypeKind::Union(members) => {
            let mut non_null = members.iter().filter(|member| !member.is_null());
            let Some(first) = non_null.next() else {
                return false;
            };
            return type_is_queueable(first, class_loader, allow_mailer)
                && non_null.all(|member| type_is_queueable(member, class_loader, allow_mailer));
        }
        _ => {}
    }

    // A resolved named type is the overwhelmingly common case. Walking its
    // graph once avoids constructing a target PhpType and then traversing the
    // same ancestry again for the Queueable-trait fallback.
    if let Some(name) = ty.base_name() {
        if name.eq_ignore_ascii_case(SHOULD_QUEUE) {
            return true;
        }
        let Some(class) = class_loader(name) else {
            return false;
        };
        let fqn = class.fqn();
        return is_builtin_queueable(fqn.as_str(), allow_mailer)
            || class_is_queueable(&class, class_loader);
    }

    // Intersections have no single base name. The shared subtype engine
    // correctly accepts one that explicitly carries ShouldQueue.
    crate::class_lookup::is_subtype_of_named(ty, SHOULD_QUEUE, class_loader)
}

fn is_builtin_queueable(fqn: &str, allow_mailer: bool) -> bool {
    [
        "Illuminate\\Bus\\PendingBatch",
        "Illuminate\\Events\\QueuedClosure",
        "Illuminate\\Foundation\\Bus\\PendingDispatch",
        "Illuminate\\Foundation\\Bus\\PendingChain",
    ]
    .iter()
    .any(|expected| fqn.eq_ignore_ascii_case(expected))
        || (allow_mailer && fqn.eq_ignore_ascii_case("Illuminate\\Mail\\Mailer"))
}

#[derive(Clone, Copy)]
enum QueueableEdge {
    Interface(Atom),
    Trait(Atom),
    Parent(Atom),
}

impl QueueableEdge {
    fn name(self) -> Atom {
        match self {
            Self::Interface(name) | Self::Trait(name) | Self::Parent(name) => name,
        }
    }
}

/// Traverse interfaces, traits, and parents breadth-first. The visited set
/// makes diamond graphs and cycles linear in the number of distinct classes;
/// breadth-first order ensures a shared node is first seen at its shallowest
/// depth, preserving the inheritance-depth bound.
fn class_is_queueable(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    if class_has_queueable_marker(class) {
        return true;
    }

    let edge_count = class.interfaces.len()
        + class.used_traits.len()
        + usize::from(class.parent_class.is_some());
    if edge_count == 0 {
        return false;
    }

    let mut pending = VecDeque::with_capacity(edge_count);
    push_edges(class, 1, &mut pending);
    let mut visited = AtomSet::default();
    visited.insert(class.fqn());

    while let Some((edge, depth)) = pending.pop_front() {
        let name = edge.name();
        // Edges are only enqueued below the depth limit, so every item here
        // is valid and the hot loop only needs the cycle/diamond check.
        if !visited.insert(name) {
            continue;
        }
        let Some(next) = class_loader(name.as_str()) else {
            continue;
        };
        if class_has_queueable_marker(&next) {
            return true;
        }
        if depth < MAX_INHERITANCE_DEPTH {
            push_edges(&next, depth + 1, &mut pending);
        }
    }
    false
}

fn class_has_queueable_marker(class: &ClassInfo) -> bool {
    class
        .interfaces
        .iter()
        .any(|name| name.eq_ignore_ascii_case(SHOULD_QUEUE))
        || class
            .used_traits
            .iter()
            .any(|name| name.eq_ignore_ascii_case(QUEUEABLE_TRAIT))
}

fn push_edges(class: &ClassInfo, depth: u32, pending: &mut VecDeque<(QueueableEdge, u32)>) {
    pending.extend(
        class
            .interfaces
            .iter()
            .copied()
            .map(|name| (QueueableEdge::Interface(name), depth)),
    );
    pending.extend(
        class
            .used_traits
            .iter()
            .copied()
            .map(|name| (QueueableEdge::Trait(name), depth)),
    );
    if let Some(parent) = class.parent_class {
        pending.push_back((QueueableEdge::Parent(parent), depth));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::test_fixtures::make_class;

    fn implementing(name: &str, interface: &str) -> Arc<ClassInfo> {
        let mut class = make_class(name);
        class.interfaces.push(crate::atom::atom(interface));
        Arc::new(class)
    }

    fn named(name: &str) -> PhpType {
        PhpType::named(crate::atom::atom(name))
    }

    #[test]
    fn connection_receiver_unions_require_one_consistent_resource_family() {
        let first_database = implementing(
            "App\\FirstDatabaseResolver",
            "Illuminate\\Database\\ConnectionResolverInterface",
        );
        let second_database = implementing(
            "App\\SecondDatabaseResolver",
            "Illuminate\\Database\\ConnectionResolverInterface",
        );
        let queue = implementing("App\\QueueFactory", "Illuminate\\Contracts\\Queue\\Factory");
        let loader = |name: &str| match name {
            "App\\FirstDatabaseResolver" => Some(Arc::clone(&first_database)),
            "App\\SecondDatabaseResolver" => Some(Arc::clone(&second_database)),
            "App\\QueueFactory" => Some(Arc::clone(&queue)),
            _ => None,
        };

        assert_eq!(
            classify_receiver_type(
                LaravelResourceReceiverRule::ConnectionMethod,
                &PhpType::union(vec![
                    named("App\\FirstDatabaseResolver"),
                    named("App\\SecondDatabaseResolver"),
                ]),
                &loader,
            ),
            Some(LaravelStringKind::ConfigResource(
                LaravelConfigResource::DatabaseConnection,
            ))
        );
        assert_eq!(
            classify_receiver_type(
                LaravelResourceReceiverRule::ConnectionMethod,
                &PhpType::union(vec![
                    named("App\\FirstDatabaseResolver"),
                    named("App\\QueueFactory"),
                ]),
                &loader,
            ),
            None
        );
        assert_eq!(
            classify_receiver_type(
                LaravelResourceReceiverRule::ConnectionMethod,
                &PhpType::union(vec![
                    named("App\\FirstDatabaseResolver"),
                    named("App\\MissingResolver"),
                ]),
                &loader,
            ),
            None
        );
        assert_eq!(
            classify_receiver_type(
                LaravelResourceReceiverRule::ConnectionProperty,
                &named("App\\FirstDatabaseResolver"),
                &loader,
            ),
            None
        );
    }

    #[test]
    fn queueable_types_unwrap_null_without_weakening_union_classification() {
        let first = implementing("App\\FirstJob", SHOULD_QUEUE);
        let second = implementing("App\\SecondJob", SHOULD_QUEUE);
        let ordinary = Arc::new(make_class("App\\Ordinary"));
        let pending = Arc::new(make_class("Illuminate\\Foundation\\Bus\\PendingDispatch"));
        let loader = |name: &str| match name {
            "App\\FirstJob" => Some(Arc::clone(&first)),
            "App\\SecondJob" => Some(Arc::clone(&second)),
            "App\\Ordinary" => Some(Arc::clone(&ordinary)),
            "Illuminate\\Foundation\\Bus\\PendingDispatch" => Some(Arc::clone(&pending)),
            _ => None,
        };

        assert!(type_is_queueable(
            &PhpType::nullable(named("App\\FirstJob")),
            &loader,
            false,
        ));
        assert!(type_is_queueable(
            &PhpType::union(vec![named("App\\FirstJob"), PhpType::null()]),
            &loader,
            false,
        ));
        assert!(type_is_queueable(
            &PhpType::union(vec![named("App\\FirstJob"), named("App\\SecondJob")]),
            &loader,
            false,
        ));
        assert!(!type_is_queueable(
            &PhpType::union(vec![named("App\\FirstJob"), named("App\\Ordinary")]),
            &loader,
            false,
        ));
        assert!(type_is_queueable(
            &PhpType::nullable(named("Illuminate\\Foundation\\Bus\\PendingDispatch")),
            &loader,
            false,
        ));
        assert!(type_is_queueable(&named(SHOULD_QUEUE), &loader, false));
        assert!(!type_is_queueable(&named("App\\Missing"), &loader, false));
        let all_null: PhpType = TypeKind::Union(vec![PhpType::null()].into()).into();
        assert!(!type_is_queueable(&all_null, &loader, false));
        assert!(type_is_queueable(
            &PhpType::intersection(vec![named("App\\Ordinary"), named(SHOULD_QUEUE)]),
            &loader,
            false,
        ));
    }

    #[test]
    fn queueable_graphs_follow_every_edge_once() {
        fn with_trait(name: &str, trait_name: &str) -> Arc<ClassInfo> {
            let mut class = make_class(name);
            class.used_traits.push(crate::atom::atom(trait_name));
            Arc::new(class)
        }

        let marker = implementing("App\\QueueMarker", SHOULD_QUEUE);
        let interface_job = implementing("App\\InterfaceJob", "App\\QueueMarker");
        let nested_trait = with_trait("App\\NestedQueueable", QUEUEABLE_TRAIT);
        let trait_job = with_trait("App\\TraitJob", "App\\NestedQueueable");
        let direct_trait_job = with_trait("App\\DirectTraitJob", QUEUEABLE_TRAIT);
        let unknown_edges = {
            let mut class = make_class("App\\UnknownEdges");
            class
                .interfaces
                .push(crate::atom::atom("App\\MissingInterface"));
            class
                .used_traits
                .push(crate::atom::atom("App\\MissingTrait"));
            Arc::new(class)
        };
        let cycle = implementing("App\\Cycle", "App\\Cycle");
        let mut child = make_class("App\\ChildJob");
        child.parent_class = Some(crate::atom::atom("App\\InterfaceJob"));
        let loader = |name: &str| match name {
            "App\\QueueMarker" => Some(Arc::clone(&marker)),
            "App\\InterfaceJob" => Some(Arc::clone(&interface_job)),
            "App\\NestedQueueable" => Some(Arc::clone(&nested_trait)),
            "App\\Cycle" => Some(Arc::clone(&cycle)),
            _ => None,
        };

        assert!(class_is_queueable(&interface_job, &loader));
        assert!(class_is_queueable(&trait_job, &loader));
        assert!(class_is_queueable(&direct_trait_job, &loader));
        assert!(class_is_queueable(&child, &loader));
        assert!(!class_is_queueable(&unknown_edges, &loader));
        assert!(!class_is_queueable(&cycle, &loader));
        assert!(!class_is_queueable(
            &make_class("App\\NoHierarchy"),
            &loader
        ));
    }

    #[test]
    fn diamond_graphs_load_each_distinct_class_once() {
        let shared = Arc::new(make_class("App\\Shared"));
        let left = implementing("App\\Left", "App\\Shared");
        let right = implementing("App\\Right", "App\\Shared");
        let mut root = make_class("App\\Root");
        root.interfaces.push(crate::atom::atom("App\\Left"));
        root.interfaces.push(crate::atom::atom("App\\Right"));
        root.interfaces.push(crate::atom::atom("App\\Missing"));
        let shared_loads = Cell::new(0usize);
        let loader = |name: &str| match name {
            "App\\Left" => Some(Arc::clone(&left)),
            "App\\Right" => Some(Arc::clone(&right)),
            "App\\Shared" => {
                shared_loads.set(shared_loads.get() + 1);
                Some(Arc::clone(&shared))
            }
            _ => None,
        };

        assert!(!class_is_queueable(&root, &loader));
        assert_eq!(shared_loads.get(), 1);
    }
}
