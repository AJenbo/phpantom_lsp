//! Transparent PHP proxy relations used by project metadata.
//!
//! The type engine still sees generated proxy subclasses as the classes they
//! actually declare. Metadata consumers use this module when a proxy is only
//! a runtime wrapper and annotations, events, references, or lenses should be
//! attributed to the wrapped parent class instead.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use globset::Glob;
use ignore::WalkBuilder;

use crate::Backend;
use crate::config::PhpProxyConfig;

const CONFIG_SOURCE: &str = "php-config";
const MAX_PROXY_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProxyRelation {
    pub proxy_fqn: String,
    pub target_fqn: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProxyIndex {
    sources: BTreeMap<String, Vec<ProxyRelation>>,
    targets: HashMap<String, ProxyRelation>,
    families: HashMap<String, Vec<String>>,
}

impl ProxyIndex {
    fn replace_source(&mut self, source: String, relations: Vec<ProxyRelation>) {
        if relations.is_empty() {
            self.sources.remove(&source);
        } else {
            self.sources.insert(source, relations);
        }
        self.rebuild_targets();
    }

    fn rebuild_targets(&mut self) {
        self.targets.clear();
        for relations in self.sources.values() {
            for relation in relations {
                let proxy = normalize_class_name(&relation.proxy_fqn);
                let target = normalize_class_name(&relation.target_fqn);
                if proxy.is_empty() || target.is_empty() || proxy.eq_ignore_ascii_case(&target) {
                    continue;
                }
                self.targets.insert(
                    class_key(&proxy),
                    ProxyRelation {
                        proxy_fqn: proxy,
                        target_fqn: target,
                    },
                );
            }
        }

        let mut families: HashMap<String, Vec<String>> = HashMap::new();
        for relation in self.targets.values() {
            if let Some(target) = self.canonical_target(&relation.proxy_fqn) {
                families
                    .entry(class_key(&target))
                    .or_default()
                    .push(relation.proxy_fqn.clone());
            }
        }
        for proxies in families.values_mut() {
            proxies.sort_by_key(|name| name.to_ascii_lowercase());
            proxies.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        }
        self.families = families;
    }

    fn canonical_target(&self, class_fqn: &str) -> Option<String> {
        let original = normalize_class_name(class_fqn);
        let mut current = original.clone();
        let mut seen = HashSet::with_capacity(4);
        let mut changed = false;

        for _ in 0..MAX_PROXY_DEPTH {
            let key = class_key(&current);
            if !seen.insert(key.clone()) {
                return None;
            }
            let Some(relation) = self.targets.get(&key) else {
                return changed.then_some(current);
            };
            current.clone_from(&relation.target_fqn);
            changed = true;
        }

        None
    }

    fn class_family(&self, class_fqn: &str) -> Vec<String> {
        let canonical = self
            .canonical_target(class_fqn)
            .unwrap_or_else(|| normalize_class_name(class_fqn));
        let proxies = self.families.get(&class_key(&canonical));
        let mut family = Vec::with_capacity(proxies.map_or(1, |proxies| proxies.len() + 1));
        family.push(canonical);
        if let Some(proxies) = proxies {
            family.extend(proxies.iter().cloned());
        }
        family
    }

    fn len(&self) -> usize {
        self.targets.len()
    }
}

impl Backend {
    /// Replace the proxy relations contributed by one metadata adapter.
    ///
    /// `source` is stable adapter identity (usually a generated file URI), so
    /// refreshing one adapter cannot discard relations found by another.
    pub(crate) fn replace_proxy_relations(
        &self,
        source: impl Into<String>,
        relations: Vec<ProxyRelation>,
    ) {
        self.proxy_index
            .write()
            .replace_source(source.into(), relations);
    }

    /// Return the real class and every transparent proxy that represents it.
    pub(crate) fn metadata_class_family(&self, class_fqn: &str) -> Vec<String> {
        self.proxy_index.read().class_family(class_fqn)
    }

    /// Rebuild relations discovered from `[[php.proxies]]` rules.
    pub(crate) fn rebuild_configured_proxy_index(&self, workspace_root: &Path) -> usize {
        let rules = self.config().php.proxies;
        let mut relations = Vec::new();

        for rule in &rules {
            if rule.marker_interface.trim().is_empty() {
                continue;
            }
            for path in collect_rule_files(workspace_root, rule) {
                relations.extend(self.proxy_relations_in_file(&path, rule));
            }
        }

        self.replace_proxy_relations(CONFIG_SOURCE, relations);
        self.proxy_index.read().len()
    }

    fn proxy_relations_in_file(&self, path: &Path, rule: &PhpProxyConfig) -> Vec<ProxyRelation> {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let marker = normalize_class_name(&rule.marker_interface);

        Self::parse_php_versioned_with_namespaces(&content, None)
            .into_iter()
            .filter_map(|(class, namespace)| {
                let implements_marker = class.interfaces.iter().any(|interface| {
                    normalize_class_name(interface.as_str()).eq_ignore_ascii_case(&marker)
                });
                if !implements_marker {
                    return None;
                }

                let target = normalize_class_name(class.parent_class?.as_str());
                if target.is_empty() {
                    return None;
                }
                let proxy_fqn = match namespace {
                    Some(namespace) if !namespace.is_empty() => {
                        format!("{}\\{}", namespace, class.name)
                    }
                    _ => class.name.to_string(),
                };
                Some(ProxyRelation {
                    proxy_fqn,
                    target_fqn: target,
                })
            })
            .collect()
    }
}

/// Whether a changed path belongs to an opt-in proxy discovery rule.
pub(crate) fn is_configured_proxy_path(
    workspace_root: &Path,
    path: &Path,
    rules: &[PhpProxyConfig],
) -> bool {
    let Ok(relative) = path.strip_prefix(workspace_root) else {
        return false;
    };
    rules.iter().any(|rule| {
        rule.paths
            .iter()
            .any(|spec| path_matches_spec(relative, spec))
    })
}

fn collect_rule_files(workspace_root: &Path, rule: &PhpProxyConfig) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    for spec in &rule.paths {
        let Some(relative) = safe_relative_path(spec) else {
            continue;
        };

        if has_glob_meta(spec) {
            let Ok(glob) = Glob::new(spec) else {
                tracing::warn!("PHPantom: invalid proxy path glob: {}", spec);
                continue;
            };
            let matcher = glob.compile_matcher();
            let base = workspace_root.join(fixed_glob_prefix(&relative));
            collect_php_files(
                &base,
                |path| {
                    path.strip_prefix(workspace_root)
                        .is_ok_and(|relative| matcher.is_match(relative))
                },
                &mut files,
            );
            continue;
        }

        let absolute = workspace_root.join(relative);
        if absolute.is_file() {
            if is_php_file(&absolute) {
                files.insert(absolute);
            }
        } else if absolute.is_dir() {
            collect_php_files(&absolute, |_| true, &mut files);
        }
    }
    files.into_iter().collect()
}

fn collect_php_files(root: &Path, matches: impl Fn(&Path) -> bool, files: &mut BTreeSet<PathBuf>) {
    if !root.exists() {
        return;
    }
    let walker = WalkBuilder::new(root)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .follow_links(false)
        .build();

    for entry in walker.filter_map(Result::ok) {
        let path = entry.path();
        if entry.file_type().is_some_and(|kind| kind.is_file())
            && is_php_file(path)
            && matches(path)
        {
            files.insert(path.to_path_buf());
        }
    }
}

fn path_matches_spec(relative: &Path, spec: &str) -> bool {
    let Some(spec_path) = safe_relative_path(spec) else {
        return false;
    };
    if has_glob_meta(spec) {
        return Glob::new(spec)
            .ok()
            .is_some_and(|glob| glob.compile_matcher().is_match(relative));
    }
    relative == spec_path || relative.starts_with(spec_path)
}

fn safe_relative_path(spec: &str) -> Option<PathBuf> {
    let path = Path::new(spec.trim());
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(path.to_path_buf())
}

fn fixed_glob_prefix(path: &Path) -> PathBuf {
    path.components()
        .take_while(|component| match component {
            Component::Normal(part) => !has_glob_meta(&part.to_string_lossy()),
            _ => false,
        })
        .collect()
}

fn has_glob_meta(value: &str) -> bool {
    value
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b'{'))
}

fn is_php_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("php"))
}

fn normalize_class_name(name: &str) -> String {
    name.trim().trim_start_matches('\\').to_string()
}

fn class_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_chains_and_builds_class_families() {
        let mut index = ProxyIndex::default();
        index.replace_source(
            "generated".to_string(),
            vec![
                ProxyRelation {
                    proxy_fqn: "Generated\\Outer".to_string(),
                    target_fqn: "Generated\\Inner".to_string(),
                },
                ProxyRelation {
                    proxy_fqn: "Generated\\Inner".to_string(),
                    target_fqn: "App\\Service".to_string(),
                },
            ],
        );

        assert_eq!(
            index.canonical_target("generated\\OUTER").as_deref(),
            Some("App\\Service")
        );
        assert_eq!(
            index.class_family("App\\Service"),
            vec![
                "App\\Service".to_string(),
                "Generated\\Inner".to_string(),
                "Generated\\Outer".to_string(),
            ]
        );
    }

    #[test]
    fn isolates_adapter_sources_and_rejects_cycles() {
        let mut index = ProxyIndex::default();
        index.replace_source(
            "one".to_string(),
            vec![ProxyRelation {
                proxy_fqn: "Generated\\One".to_string(),
                target_fqn: "App\\One".to_string(),
            }],
        );
        index.replace_source(
            "two".to_string(),
            vec![ProxyRelation {
                proxy_fqn: "Generated\\Two".to_string(),
                target_fqn: "App\\Two".to_string(),
            }],
        );
        index.replace_source("one".to_string(), Vec::new());

        assert_eq!(index.canonical_target("Generated\\One"), None);
        assert_eq!(
            index.canonical_target("Generated\\Two").as_deref(),
            Some("App\\Two")
        );

        index.replace_source(
            "cycle".to_string(),
            vec![
                ProxyRelation {
                    proxy_fqn: "Cycle\\A".to_string(),
                    target_fqn: "Cycle\\B".to_string(),
                },
                ProxyRelation {
                    proxy_fqn: "Cycle\\B".to_string(),
                    target_fqn: "Cycle\\A".to_string(),
                },
            ],
        );
        assert_eq!(index.canonical_target("Cycle\\A"), None);
    }

    #[test]
    fn scans_only_marked_proxy_subclasses() {
        let backend = Backend::new_test();
        let dir = tempfile::tempdir().unwrap();
        let proxy = dir.path().join("Proxy.php");
        std::fs::write(
            &proxy,
            r#"<?php
namespace Generated;
class ServiceProxy extends \App\Service implements \Acme\Proxy\TransparentProxy {}
class OrdinaryChild extends \App\Other {}
"#,
        )
        .unwrap();
        let rule = PhpProxyConfig {
            paths: Vec::new(),
            marker_interface: "Acme\\Proxy\\TransparentProxy".to_string(),
        };

        assert_eq!(
            backend.proxy_relations_in_file(&proxy, &rule),
            vec![ProxyRelation {
                proxy_fqn: "Generated\\ServiceProxy".to_string(),
                target_fqn: "App\\Service".to_string(),
            }]
        );
    }

    #[test]
    fn matches_configured_directories_and_globs() {
        assert!(path_matches_spec(
            Path::new("var/cache/prod/proxies/Foo.php"),
            "var/cache/prod/proxies"
        ));
        assert!(path_matches_spec(
            Path::new("var/cache/prod/proxies/Foo.php"),
            "var/cache/*/proxies/*.php"
        ));
        assert!(!path_matches_spec(
            Path::new("src/Foo.php"),
            "var/cache/*/proxies/*.php"
        ));
        assert!(!path_matches_spec(
            Path::new("outside.php"),
            "../outside.php"
        ));
    }
}
