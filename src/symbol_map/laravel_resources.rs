//! Declarative Laravel config-resource families and their string triggers.

use super::LaravelConfigResource;
#[cfg(test)]
use super::{LaravelResourceReceiverRule, LaravelStringKind};

mod receiver_types;

pub(crate) use receiver_types::{classify_connection_property, classify_receiver_type};

/// How a trigger accepts resource names in its selected argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceArgumentShape {
    /// One scalar string literal.
    Scalar,
    /// A scalar string or the string values of an array.
    ScalarOrArray,
    /// Only the string values of an array.
    Array,
}

impl ResourceArgumentShape {
    /// Whether completion may trigger inside an array element.
    pub(crate) const fn accepts_array(self) -> bool {
        matches!(self, Self::ScalarOrArray | Self::Array)
    }

    /// Whether a scalar literal is a valid argument shape.
    pub(crate) const fn accepts_scalar(self) -> bool {
        matches!(self, Self::Scalar | Self::ScalarOrArray)
    }
}

/// Whether a resource-name occurrence reads, defines, or optionally removes
/// the resource it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceAccess {
    Read,
    Write,
    OptionalRead,
}

impl ResourceAccess {
    pub(crate) const fn is_write(self) -> bool {
        matches!(self, Self::Write)
    }

    pub(crate) const fn is_optional(self) -> bool {
        matches!(self, Self::OptionalRead)
    }
}

/// One syntactic place that accepts a member of a config-resource family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigResourceTrigger {
    Function {
        name: &'static str,
        argument: &'static str,
        shape: ResourceArgumentShape,
        access: ResourceAccess,
    },
    StaticMethod {
        facade: &'static str,
        method: &'static str,
        argument: &'static str,
        shape: ResourceArgumentShape,
        access: ResourceAccess,
    },
    InstanceMethod {
        method: &'static str,
        argument: &'static str,
        shape: ResourceArgumentShape,
        access: ResourceAccess,
    },
    Attribute {
        name: &'static str,
        argument: &'static str,
    },
    Middleware {
        prefix: &'static str,
    },
}

/// One named-resource family. Adding another config-backed family is a table
/// row rather than separate completion and symbol-extraction branches.
#[derive(Debug)]
pub(crate) struct ConfigResourceDescriptor {
    pub kind: LaravelConfigResource,
    pub config_prefix: &'static str,
    pub label: &'static str,
    pub hover_label: &'static str,
    pub diagnostic_code: &'static str,
    pub triggers: &'static [ConfigResourceTrigger],
}

/// The semantic payload shared by completion and symbol extraction after a
/// declarative trigger matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceTriggerMatch {
    pub kind: LaravelConfigResource,
    pub argument: &'static str,
    pub shape: ResourceArgumentShape,
    pub access: ResourceAccess,
}

use ConfigResourceTrigger::{Attribute, Function, InstanceMethod, Middleware, StaticMethod};
use ResourceAccess::{OptionalRead, Read, Write};
use ResourceArgumentShape::{Array, Scalar, ScalarOrArray};

/// Every config-backed Laravel string family PHPantom understands.
pub(crate) const CONFIG_RESOURCES: &[ConfigResourceDescriptor] = &[
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::AuthGuard,
        config_prefix: "auth.guards.",
        label: "auth guard",
        hover_label: "Auth guard",
        diagnostic_code: "invalid_laravel_auth_guard",
        triggers: &[
            Function {
                name: "auth",
                argument: "guard",
                shape: Scalar,
                access: Read,
            },
            StaticMethod {
                facade: "Auth",
                method: "guard",
                argument: "name",
                shape: Scalar,
                access: Read,
            },
            Attribute {
                name: "Auth",
                argument: "guard",
            },
            Attribute {
                name: "Authenticated",
                argument: "guard",
            },
            Middleware { prefix: "auth:" },
        ],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::CacheStore,
        config_prefix: "cache.stores.",
        label: "cache store",
        hover_label: "Cache store",
        diagnostic_code: "invalid_laravel_cache_store",
        triggers: &[
            StaticMethod {
                facade: "Cache",
                method: "store",
                argument: "name",
                shape: Scalar,
                access: Read,
            },
            Attribute {
                name: "Cache",
                argument: "store",
            },
        ],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::LogChannel,
        config_prefix: "logging.channels.",
        label: "log channel",
        hover_label: "Log channel",
        diagnostic_code: "invalid_laravel_log_channel",
        triggers: &[
            StaticMethod {
                facade: "Log",
                method: "channel",
                argument: "channel",
                shape: Scalar,
                access: Read,
            },
            StaticMethod {
                facade: "Log",
                method: "stack",
                argument: "channels",
                shape: Array,
                access: Read,
            },
            Attribute {
                name: "Log",
                argument: "channel",
            },
        ],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::StorageDisk,
        config_prefix: "filesystems.disks.",
        label: "storage disk",
        hover_label: "Storage disk",
        diagnostic_code: "invalid_laravel_storage_disk",
        triggers: &[
            StaticMethod {
                facade: "Storage",
                method: "disk",
                argument: "name",
                shape: Scalar,
                access: Read,
            },
            StaticMethod {
                facade: "Storage",
                method: "fake",
                argument: "disk",
                shape: Scalar,
                access: Write,
            },
            StaticMethod {
                facade: "Storage",
                method: "persistentFake",
                argument: "disk",
                shape: Scalar,
                access: Write,
            },
            StaticMethod {
                facade: "Storage",
                method: "forgetDisk",
                argument: "disk",
                shape: ScalarOrArray,
                access: OptionalRead,
            },
            Attribute {
                name: "Storage",
                argument: "disk",
            },
        ],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::DatabaseConnection,
        config_prefix: "database.connections.",
        label: "database connection",
        hover_label: "Database connection",
        diagnostic_code: "invalid_laravel_database_connection",
        triggers: &[
            StaticMethod {
                facade: "DB",
                method: "connection",
                argument: "name",
                shape: Scalar,
                access: Read,
            },
            Attribute {
                name: "Database",
                argument: "connection",
            },
            Attribute {
                name: "DB",
                argument: "connection",
            },
        ],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::QueueConnection,
        config_prefix: "queue.connections.",
        label: "queue connection",
        hover_label: "Queue connection",
        diagnostic_code: "invalid_laravel_queue_connection",
        triggers: &[
            StaticMethod {
                facade: "Queue",
                method: "connection",
                argument: "name",
                shape: Scalar,
                access: Read,
            },
            InstanceMethod {
                method: "onConnection",
                argument: "connection",
                shape: Scalar,
                access: Read,
            },
        ],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::Mailer,
        config_prefix: "mail.mailers.",
        label: "mailer",
        hover_label: "Mailer",
        diagnostic_code: "invalid_laravel_mailer",
        triggers: &[StaticMethod {
            facade: "Mail",
            method: "mailer",
            argument: "name",
            shape: Scalar,
            access: Read,
        }],
    },
    ConfigResourceDescriptor {
        kind: LaravelConfigResource::BroadcastConnection,
        config_prefix: "broadcasting.connections.",
        label: "broadcast connection",
        hover_label: "Broadcast connection",
        diagnostic_code: "invalid_laravel_broadcast_connection",
        triggers: &[StaticMethod {
            facade: "Broadcast",
            method: "connection",
            argument: "name",
            shape: Scalar,
            access: Read,
        }],
    },
];

pub(crate) fn descriptor(kind: LaravelConfigResource) -> &'static ConfigResourceDescriptor {
    match kind {
        LaravelConfigResource::AuthGuard => &CONFIG_RESOURCES[0],
        LaravelConfigResource::CacheStore => &CONFIG_RESOURCES[1],
        LaravelConfigResource::LogChannel => &CONFIG_RESOURCES[2],
        LaravelConfigResource::StorageDisk => &CONFIG_RESOURCES[3],
        LaravelConfigResource::DatabaseConnection => &CONFIG_RESOURCES[4],
        LaravelConfigResource::QueueConnection => &CONFIG_RESOURCES[5],
        LaravelConfigResource::Mailer => &CONFIG_RESOURCES[6],
        LaravelConfigResource::BroadcastConnection => &CONFIG_RESOURCES[7],
    }
}

/// Build the dot key used by Laravel's config index for one short resource
/// name. This allocation is paid only at a config boundary, never per stored
/// symbol span.
pub(crate) fn config_key(kind: LaravelConfigResource, short_name: &str) -> String {
    let prefix = descriptor(kind).config_prefix;
    let mut key = String::with_capacity(prefix.len() + short_name.len());
    key.push_str(prefix);
    key.push_str(short_name);
    key
}

/// Whether `full_key` is the config address of `short_name` in `kind`.
pub(crate) fn matches_config_key(
    kind: LaravelConfigResource,
    short_name: &str,
    full_key: &str,
) -> bool {
    full_key
        .strip_prefix(descriptor(kind).config_prefix)
        .is_some_and(|rest| rest == short_name)
}

/// Interpret a generic config key as a direct resource child.
pub(crate) fn resource_from_config_key(full_key: &str) -> Option<(LaravelConfigResource, &str)> {
    let root = full_key.split_once('.')?.0;
    let kind = config_root_resource(root)?;
    let short = full_key.strip_prefix(descriptor(kind).config_prefix)?;
    (!short.is_empty() && !short.contains('.')).then_some((kind, short))
}

pub(crate) fn function_trigger(name: &str) -> Option<ResourceTriggerMatch> {
    let resource = descriptor(function_resource(name)?);
    function_trigger_from(resource, name)
}

fn function_trigger_from(
    resource: &ConfigResourceDescriptor,
    name: &str,
) -> Option<ResourceTriggerMatch> {
    resource.triggers.iter().find_map(|trigger| match trigger {
        Function {
            name: expected,
            argument,
            shape,
            access,
        } if name.eq_ignore_ascii_case(expected) => Some(ResourceTriggerMatch {
            kind: resource.kind,
            argument,
            shape: *shape,
            access: *access,
        }),
        _ => None,
    })
}

pub(crate) fn static_method_trigger(receiver: &str, method: &str) -> Option<ResourceTriggerMatch> {
    let receiver = receiver.trim_start_matches('\\');
    let short = if let Some((namespace, short)) = receiver.rsplit_once('\\') {
        if !namespace.eq_ignore_ascii_case("Illuminate\\Support\\Facades") {
            return None;
        }
        short
    } else {
        receiver
    };
    let resource = descriptor(static_facade_resource(short)?);
    resource.triggers.iter().find_map(|trigger| match trigger {
        StaticMethod {
            facade,
            method: expected,
            argument,
            shape,
            access,
        } if short.eq_ignore_ascii_case(facade) && method.eq_ignore_ascii_case(expected) => {
            Some(ResourceTriggerMatch {
                kind: resource.kind,
                argument,
                shape: *shape,
                access: *access,
            })
        }
        _ => None,
    })
}

pub(crate) fn instance_method_trigger(method: &str) -> Option<ResourceTriggerMatch> {
    let resource = descriptor(instance_method_resource(method)?);
    resource.triggers.iter().find_map(|trigger| match trigger {
        InstanceMethod {
            method: expected,
            argument,
            shape,
            access,
        } if method.eq_ignore_ascii_case(expected) => Some(ResourceTriggerMatch {
            kind: resource.kind,
            argument,
            shape: *shape,
            access: *access,
        }),
        _ => None,
    })
}

pub(crate) fn attribute_trigger(name: &str) -> Option<ResourceTriggerMatch> {
    let short = name.rsplit('\\').next().unwrap_or(name);
    let resource = descriptor(attribute_resource_kind(short)?);
    resource.triggers.iter().find_map(|trigger| match trigger {
        Attribute { name, argument } if short.eq_ignore_ascii_case(name) => {
            Some(ResourceTriggerMatch {
                kind: resource.kind,
                argument,
                shape: ResourceArgumentShape::Scalar,
                access: ResourceAccess::Read,
            })
        }
        _ => None,
    })
}

#[cfg(test)]
pub(crate) fn attribute_resource(name: &str) -> Option<LaravelConfigResource> {
    attribute_trigger(name).map(|trigger| trigger.kind)
}

pub(crate) fn middleware_resource(prefix: &str) -> Option<LaravelConfigResource> {
    let resource = descriptor(middleware_resource_kind(prefix)?);
    resource.triggers.iter().find_map(|trigger| match trigger {
        Middleware { prefix: expected } if prefix.eq_ignore_ascii_case(expected) => {
            Some(resource.kind)
        }
        _ => None,
    })
}

// These compact indexes keep extraction and completion's negative path from
// walking every descriptor for every PHP call. CONFIG_RESOURCES remains the
// source of trigger metadata; the exhaustive test below rejects an index that
// falls out of sync when a table row is added or moved.
fn config_root_resource(root: &str) -> Option<LaravelConfigResource> {
    use LaravelConfigResource::*;
    match root.len() {
        4 if root == "auth" => Some(AuthGuard),
        4 if root == "mail" => Some(Mailer),
        5 if root == "cache" => Some(CacheStore),
        5 if root == "queue" => Some(QueueConnection),
        7 if root == "logging" => Some(LogChannel),
        8 if root == "database" => Some(DatabaseConnection),
        11 if root == "filesystems" => Some(StorageDisk),
        12 if root == "broadcasting" => Some(BroadcastConnection),
        _ => None,
    }
}

fn function_resource(name: &str) -> Option<LaravelConfigResource> {
    (name.len() == 4 && name.eq_ignore_ascii_case("auth"))
        .then_some(LaravelConfigResource::AuthGuard)
}

fn static_facade_resource(name: &str) -> Option<LaravelConfigResource> {
    use LaravelConfigResource::*;
    match name.len() {
        2 if name.eq_ignore_ascii_case("DB") => Some(DatabaseConnection),
        3 if name.eq_ignore_ascii_case("Log") => Some(LogChannel),
        4 if name.eq_ignore_ascii_case("Auth") => Some(AuthGuard),
        4 if name.eq_ignore_ascii_case("Mail") => Some(Mailer),
        5 if name.eq_ignore_ascii_case("Cache") => Some(CacheStore),
        5 if name.eq_ignore_ascii_case("Queue") => Some(QueueConnection),
        7 if name.eq_ignore_ascii_case("Storage") => Some(StorageDisk),
        9 if name.eq_ignore_ascii_case("Broadcast") => Some(BroadcastConnection),
        _ => None,
    }
}

fn instance_method_resource(method: &str) -> Option<LaravelConfigResource> {
    (method.len() == 12 && method.eq_ignore_ascii_case("onConnection"))
        .then_some(LaravelConfigResource::QueueConnection)
}

fn attribute_resource_kind(name: &str) -> Option<LaravelConfigResource> {
    use LaravelConfigResource::*;
    match name.len() {
        2 if name.eq_ignore_ascii_case("DB") => Some(DatabaseConnection),
        3 if name.eq_ignore_ascii_case("Log") => Some(LogChannel),
        4 if name.eq_ignore_ascii_case("Auth") => Some(AuthGuard),
        5 if name.eq_ignore_ascii_case("Cache") => Some(CacheStore),
        7 if name.eq_ignore_ascii_case("Storage") => Some(StorageDisk),
        8 if name.eq_ignore_ascii_case("Database") => Some(DatabaseConnection),
        13 if name.eq_ignore_ascii_case("Authenticated") => Some(AuthGuard),
        _ => None,
    }
}

fn middleware_resource_kind(prefix: &str) -> Option<LaravelConfigResource> {
    (prefix.len() == 5 && prefix.eq_ignore_ascii_case("auth:"))
        .then_some(LaravelConfigResource::AuthGuard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_resource_has_one_unique_prefix_and_descriptor() {
        for (index, resource) in CONFIG_RESOURCES.iter().enumerate() {
            assert!(resource.config_prefix.ends_with('.'));
            assert_eq!(descriptor(resource.kind).kind, resource.kind);
            assert!(CONFIG_RESOURCES[..index].iter().all(|seen| {
                seen.kind != resource.kind && seen.config_prefix != resource.config_prefix
            }));
        }
    }

    #[test]
    fn fast_indexes_cover_every_declarative_trigger() {
        for resource in CONFIG_RESOURCES {
            let root = resource.config_prefix.split('.').next().unwrap();
            assert_eq!(config_root_resource(root), Some(resource.kind));

            for trigger in resource.triggers {
                match trigger {
                    Function {
                        name,
                        argument,
                        shape,
                        access,
                    } => assert_eq!(
                        function_trigger(name),
                        Some(ResourceTriggerMatch {
                            kind: resource.kind,
                            argument,
                            shape: *shape,
                            access: *access,
                        })
                    ),
                    StaticMethod {
                        facade,
                        method,
                        argument,
                        shape,
                        access,
                    } => {
                        let expected = Some(ResourceTriggerMatch {
                            kind: resource.kind,
                            argument,
                            shape: *shape,
                            access: *access,
                        });
                        assert_eq!(static_method_trigger(facade, method), expected);
                        let fqn = format!("Illuminate\\Support\\Facades\\{facade}");
                        assert_eq!(static_method_trigger(&fqn, method), expected);
                    }
                    InstanceMethod {
                        method,
                        argument,
                        shape,
                        access,
                    } => assert_eq!(
                        instance_method_trigger(method),
                        Some(ResourceTriggerMatch {
                            kind: resource.kind,
                            argument,
                            shape: *shape,
                            access: *access,
                        })
                    ),
                    Attribute { name, argument } => assert_eq!(
                        attribute_trigger(name),
                        Some(ResourceTriggerMatch {
                            kind: resource.kind,
                            argument,
                            shape: ResourceArgumentShape::Scalar,
                            access: ResourceAccess::Read,
                        })
                    ),
                    Middleware { prefix } => {
                        assert_eq!(middleware_resource(prefix), Some(resource.kind));
                    }
                }
            }
        }
    }

    #[test]
    fn trigger_lookups_are_case_insensitive_and_context_specific() {
        assert_eq!(
            function_trigger("AUTH").map(|found| found.kind),
            Some(LaravelConfigResource::AuthGuard)
        );
        assert_eq!(
            static_method_trigger("Illuminate\\Support\\Facades\\Log", "STACK").map(|found| (
                found.kind,
                found.shape,
                found.argument
            )),
            Some((
                LaravelConfigResource::LogChannel,
                ResourceArgumentShape::Array,
                "channels",
            ))
        );
        assert_eq!(
            instance_method_trigger("onConnection").map(|found| found.kind),
            Some(LaravelConfigResource::QueueConnection)
        );
        assert_eq!(
            attribute_resource("Authenticated"),
            Some(LaravelConfigResource::AuthGuard)
        );
        assert!(static_method_trigger("Queue", "mailer").is_none());
        assert!(function_trigger("guard").is_none());
        assert!(
            function_trigger_from(descriptor(LaravelConfigResource::AuthGuard), "guard").is_none()
        );
        assert!(instance_method_trigger("connection").is_none());
        assert!(attribute_trigger("UnknownAttribute").is_none());
        assert!(static_method_trigger("Unknown", "connection").is_none());
        assert!(static_method_trigger("Acme\\Log", "stack").is_none());
        assert_eq!(
            static_method_trigger("ILLUMINATE\\SUPPORT\\FACADES\\CACHE", "STORE")
                .map(|found| found.kind),
            Some(LaravelConfigResource::CacheStore)
        );
        assert_eq!(
            middleware_resource("AUTH:"),
            Some(LaravelConfigResource::AuthGuard)
        );
        assert!(middleware_resource("throttle:").is_none());
    }

    #[test]
    fn canonical_config_conversion_is_exact_and_symmetric() {
        for resource in CONFIG_RESOURCES {
            let full = config_key(resource.kind, "named");
            assert_eq!(
                resource_from_config_key(&full),
                Some((resource.kind, "named"))
            );
            assert!(matches_config_key(resource.kind, "named", &full));
            assert!(!matches_config_key(resource.kind, "other", &full));

            let nested = format!("{full}.option");
            assert!(resource_from_config_key(&nested).is_none());
        }
        assert!(resource_from_config_key("app.name").is_none());
    }

    #[test]
    fn storage_access_modes_and_log_shape_come_from_the_table() {
        for (method, expected_access) in [
            ("disk", ResourceAccess::Read),
            ("fake", ResourceAccess::Write),
            ("persistentFake", ResourceAccess::Write),
            ("forgetDisk", ResourceAccess::OptionalRead),
        ] {
            assert_eq!(
                static_method_trigger("Storage", method).map(|found| found.access),
                Some(expected_access)
            );
        }
        assert_eq!(
            static_method_trigger("Log", "stack").map(|found| found.shape),
            Some(ResourceArgumentShape::Array)
        );
    }

    #[test]
    fn receiver_rules_expose_every_possible_reference_family() {
        use LaravelConfigResource::{BroadcastConnection, DatabaseConnection, QueueConnection};
        use LaravelResourceReceiverRule::{
            ConnectionMethod, ConnectionProperty, QueueName, QueueableConnection,
        };
        use LaravelStringKind::{ConfigResource, QueueName as QueueNameKind};

        assert_eq!(
            ConnectionMethod.candidate_kinds(),
            &[
                ConfigResource(DatabaseConnection),
                ConfigResource(QueueConnection),
                ConfigResource(BroadcastConnection),
            ]
        );
        assert_eq!(
            QueueableConnection.candidate_kinds(),
            &[ConfigResource(QueueConnection)]
        );
        assert_eq!(QueueName.candidate_kinds(), &[QueueNameKind]);
        assert_eq!(
            ConnectionProperty.candidate_kinds(),
            &[
                ConfigResource(DatabaseConnection),
                ConfigResource(QueueConnection),
            ]
        );
    }
}
