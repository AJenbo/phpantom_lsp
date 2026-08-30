//! Code Lens (`textDocument/codeLens`) support.
//!
//! Shows override/implement annotations linking to the prototype declaration.

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::atom::Atom;
use crate::definition::MemberImplementationTarget;
use crate::definition::member::MemberKind;
use crate::text_position::offset_to_position;
use crate::types::{ClassInfo, ClassLikeKind, MAX_INHERITANCE_DEPTH};

fn line_indent(content: &str, byte_offset: usize) -> u32 {
    let line_start = content[..byte_offset]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    content[line_start..]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .count() as u32
}

fn implementation_lens_title(count: usize) -> String {
    format!(
        "{count} {}",
        if count == 1 {
            "implementation"
        } else {
            "implementations"
        }
    )
}

/// Information about a prototype (ancestor) method that a local method
/// overrides or implements.
struct Prototype {
    /// Display name of the ancestor class (short name).
    ancestor_name: String,
    is_interface: bool,
    /// URI of the file containing the ancestor class.
    file_uri: String,
    /// Position of the method declaration in the ancestor's file.
    position: Position,
}

impl Backend {
    /// Handle a `textDocument/codeLens` request.
    ///
    /// Returns a code lens for each method in the file that overrides
    /// a parent class method or implements an interface method.
    pub fn handle_code_lens(&self, uri: &str, content: &str) -> Option<Vec<CodeLens>> {
        let classes = {
            let map = self.symbols.uri_classes_index.read();
            map.get(uri)?.clone()
        };

        let mut lenses = Vec::new();
        let context = self.file_context(uri);
        let class_loader = self.class_loader(&context);

        for class in &classes {
            let class_fqn = class.fqn();
            let implementation_descendants = if self
                .workspace_indexed
                .load(std::sync::atomic::Ordering::Acquire)
            {
                self.implementation_descendants(class, &class_loader, true)
            } else {
                Vec::new()
            };

            if !implementation_descendants.is_empty() {
                let locations = self.class_implementation_locations_from_descendants(
                    uri,
                    content,
                    class,
                    &implementation_descendants,
                );
                if let Some(lens) = self.build_locations_lens(
                    uri,
                    offset_to_position(content, class.keyword_offset as usize),
                    implementation_lens_title(locations.len()),
                    locations,
                ) {
                    lenses.push(lens);
                }
            }

            if let Some(lens) = self.build_covers_lens(class, uri, content) {
                lenses.push(lens);
            }

            for method in &class.methods {
                if method.name_offset == 0
                    || method.is_virtual
                    || method.visibility == crate::types::Visibility::Private
                {
                    continue;
                }

                let pos = offset_to_position(content, method.name_offset as usize);
                let indent = line_indent(content, method.name_offset as usize);
                let range = Range {
                    start: Position {
                        line: pos.line,
                        character: indent,
                    },
                    end: Position {
                        line: pos.line,
                        character: indent,
                    },
                };

                let proto = self.find_prototype(class, &class_fqn, &method.name, uri, content);
                if !implementation_descendants.is_empty() {
                    let locations = self.member_implementation_locations_from_descendants(
                        uri,
                        content,
                        MemberImplementationTarget {
                            class,
                            name: &method.name,
                            kind: MemberKind::Method,
                        },
                        &implementation_descendants,
                        &class_loader,
                    );
                    if let Some(lens) = self.build_locations_lens(
                        uri,
                        pos,
                        implementation_lens_title(locations.len()),
                        locations,
                    ) {
                        lenses.push(lens);
                    }
                }
                if let Some(proto) = proto {
                    let icon = if proto.is_interface { "◆" } else { "↑" };
                    let title = format!("{} {}::{}", icon, proto.ancestor_name, method.name);

                    let target_uri: Url = match proto.file_uri.parse() {
                        Ok(u) => u,
                        Err(_) => continue,
                    };

                    let command = self.build_code_lens_command(title, target_uri, proto.position);

                    lenses.push(CodeLens {
                        range,
                        command: Some(command),
                        data: None,
                    });
                }
            }
        }

        if lenses.is_empty() {
            None
        } else {
            Some(lenses)
        }
    }

    fn build_locations_lens(
        &self,
        origin_uri: &str,
        source_position: Position,
        title: String,
        locations: Vec<Location>,
    ) -> Option<CodeLens> {
        if locations.is_empty() {
            return None;
        }
        let origin_uri = Url::parse(origin_uri).ok()?;
        Some(CodeLens {
            range: Range::new(
                Position::new(source_position.line, 0),
                Position::new(source_position.line, 0),
            ),
            command: Some(Command {
                title,
                command: "editor.action.showReferences".to_string(),
                arguments: Some(vec![
                    serde_json::json!(origin_uri),
                    serde_json::json!(source_position),
                    serde_json::json!(locations),
                ]),
            }),
            data: None,
        })
    }

    /// Build the "which tests cover this class" lens for a class
    /// declaration, from the test classes whose PHPUnit coverage metadata
    /// (`@covers` / `@uses` / `#[CoversClass]` and friends) names it.
    ///
    /// `None` when the class has no keyword position (synthetic/anonymous
    /// classes), no test declares coverage for it, or none of the test
    /// classes that do can be located on disk any more.  Also `None` while
    /// the workspace is still indexing, since
    /// [`Backend::find_covering_test_classes`] cannot answer until then.
    fn build_covers_lens(&self, class: &ClassInfo, uri: &str, content: &str) -> Option<CodeLens> {
        if class.keyword_offset == 0 {
            return None;
        }

        let locations: Vec<(Atom, Location)> = self
            .find_covering_test_classes(&class.fqn())
            .into_iter()
            .filter_map(|(test_fqn, test_uri)| {
                let location =
                    self.covers_lens_target(test_fqn.as_str(), &test_uri, uri, content)?;
                Some((test_fqn, location))
            })
            .collect();
        if locations.is_empty() {
            return None;
        }

        let pos = offset_to_position(content, class.keyword_offset as usize);
        let indent = line_indent(content, class.keyword_offset as usize);
        let range = Range {
            start: Position {
                line: pos.line,
                character: indent,
            },
            end: Position {
                line: pos.line,
                character: indent,
            },
        };

        let command = if let [(test_fqn, location)] = locations.as_slice() {
            let title = format!("Tests: {}", crate::util::short_name(test_fqn));
            self.build_code_lens_command(title, location.uri.clone(), location.range.start)
        } else {
            let title = format!("Tests: {} tests", locations.len());
            let current_uri: Url = match uri.parse() {
                Ok(u) => u,
                Err(_) => return None,
            };
            Command {
                title,
                command: "editor.action.showReferences".to_string(),
                arguments: Some(vec![
                    serde_json::json!(current_uri),
                    serde_json::json!(range.start),
                    serde_json::json!(locations.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>()),
                ]),
            }
        };

        Some(CodeLens {
            range,
            command: Some(command),
            data: None,
        })
    }

    /// The location of a covering test class's own declaration, for the
    /// covers lens to navigate to.
    fn covers_lens_target(
        &self,
        test_fqn: &str,
        test_uri: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<Location> {
        let test_class = self.find_or_load_class(test_fqn)?;
        if test_class.keyword_offset == 0 {
            return None;
        }

        // The search already told us which file declares it, so read that
        // rather than looking the class up by name a second time.
        let file_content = if test_uri == current_uri {
            current_content.to_string()
        } else {
            self.get_file_content(test_uri)?
        };
        let position = offset_to_position(&file_content, test_class.keyword_offset as usize);
        let uri: Url = test_uri.parse().ok()?;

        Some(Location {
            uri,
            range: Range {
                start: position,
                end: position,
            },
        })
    }

    /// Search the inheritance hierarchy for the closest ancestor that
    /// declares a method with the given name.
    ///
    /// Priority order: parent class chain, then used traits, then
    /// implemented interfaces. Returns `None` when no ancestor
    /// declares the method.
    fn find_prototype(
        &self,
        class: &ClassInfo,
        _class_fqn: &str,
        method_name: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<Prototype> {
        // ── 1. Walk the parent class chain ──────────────────────────────
        let mut current = class.clone();
        for _ in 0..MAX_INHERITANCE_DEPTH {
            let parent_name = match current.parent_class {
                Some(name) => name,
                None => break,
            };
            let parent = match self.find_or_load_class(&parent_name) {
                Some(p) => ClassInfo::clone(&p),
                None => break,
            };
            // Check methods declared directly on this parent (not
            // inherited) so we find the actual declaration site.
            if parent
                .methods
                .iter()
                .any(|m| m.name == method_name && !m.is_virtual)
                && let Some(proto) = self.build_prototype(
                    &parent_name,
                    &parent,
                    method_name,
                    false,
                    current_uri,
                    current_content,
                )
            {
                return Some(proto);
            }
            current = parent;
        }

        // ── 2. Check used traits ────────────────────────────────────────
        if let Some(proto) = self.find_prototype_in_traits(
            &class.used_traits,
            method_name,
            current_uri,
            current_content,
            0,
        ) {
            return Some(proto);
        }

        // ── 3. Check implemented interfaces ─────────────────────────────
        if let Some(proto) =
            self.find_prototype_in_interfaces(class, method_name, current_uri, current_content)
        {
            return Some(proto);
        }

        None
    }

    /// Search a list of traits for a method declaration.
    ///
    /// Recursively checks traits used by each trait, up to a depth limit.
    fn find_prototype_in_traits(
        &self,
        trait_names: &[crate::atom::Atom],
        method_name: &str,
        current_uri: &str,
        current_content: &str,
        depth: usize,
    ) -> Option<Prototype> {
        if depth > MAX_INHERITANCE_DEPTH as usize {
            return None;
        }

        for trait_name in trait_names {
            let trait_info = match self.find_or_load_class(trait_name) {
                Some(t) => t,
                None => continue,
            };
            if trait_info
                .methods
                .iter()
                .any(|m| m.name == method_name && !m.is_virtual)
                && let Some(proto) = self.build_prototype(
                    trait_name,
                    &trait_info,
                    method_name,
                    false,
                    current_uri,
                    current_content,
                )
            {
                return Some(proto);
            }
            if let Some(proto) = self.find_prototype_in_traits(
                &trait_info.used_traits,
                method_name,
                current_uri,
                current_content,
                depth + 1,
            ) {
                return Some(proto);
            }
        }

        None
    }

    /// Search implemented interfaces (including those inherited from
    /// parents) for a method declaration.
    fn find_prototype_in_interfaces(
        &self,
        class: &ClassInfo,
        method_name: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<Prototype> {
        // Collect all interface names from the class and its parent chain.
        let mut all_iface_names: Vec<crate::atom::Atom> = class.interfaces.clone();
        let mut current = class.clone();
        for _ in 0..MAX_INHERITANCE_DEPTH {
            let parent_name = match current.parent_class {
                Some(name) => name,
                None => break,
            };
            let parent = match self.find_or_load_class(&parent_name) {
                Some(p) => ClassInfo::clone(&p),
                None => break,
            };
            for iface in &parent.interfaces {
                if !all_iface_names.contains(iface) {
                    all_iface_names.push(*iface);
                }
            }
            current = parent;
        }

        for iface_name in &all_iface_names {
            if let Some(proto) = self.find_prototype_in_interface(
                iface_name,
                method_name,
                current_uri,
                current_content,
            ) {
                return Some(proto);
            }
        }

        None
    }

    /// Check a single interface (and its own extends chain) for the
    /// method declaration.
    fn find_prototype_in_interface(
        &self,
        iface_name: &str,
        method_name: &str,
        current_uri: &str,
        current_content: &str,
    ) -> Option<Prototype> {
        let iface = self.find_or_load_class(iface_name)?;
        if iface
            .methods
            .iter()
            .any(|m| m.name == method_name && !m.is_virtual)
            && let Some(proto) = self.build_prototype(
                iface_name,
                &iface,
                method_name,
                true,
                current_uri,
                current_content,
            )
        {
            return Some(proto);
        }

        // Walk the interface's own extends chain (interfaces can extend
        // other interfaces via `parent_class` and `interfaces`).
        for parent_iface in &iface.interfaces {
            if let Some(proto) = self.find_prototype_in_interface(
                parent_iface,
                method_name,
                current_uri,
                current_content,
            ) {
                return Some(proto);
            }
        }
        if let Some(parent_name) = iface.parent_class
            && let Some(proto) = self.find_prototype_in_interface(
                &parent_name,
                method_name,
                current_uri,
                current_content,
            )
        {
            return Some(proto);
        }

        None
    }

    /// Build the LSP `Command` for a code lens that navigates to a target
    /// location.
    ///
    /// LSP has no standard "go to this location" command, so the client
    /// decides which of the two shapes it can act on:
    ///
    /// * A client that advertises `window.showDocument` gets the custom
    ///   `phpantom.navigateToPrototype` command.  It comes back as a
    ///   `workspace/executeCommand`, and the server answers it with a
    ///   `window/showDocument` request.
    /// * A client that does not gets `editor.action.showReferences` with
    ///   the `(uri, position, locations)` argument triple VS Code
    ///   established, which such clients resolve on their own without a
    ///   round trip.  Zed is the notable one: it recognises the command
    ///   name but never answers `window/showDocument`.
    fn build_code_lens_command(&self, title: String, uri: Url, position: Position) -> Command {
        if self
            .supports_show_document
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Command {
                title,
                command: "phpantom.navigateToPrototype".to_string(),
                arguments: Some(vec![serde_json::json!(uri), serde_json::json!(position)]),
            };
        }

        let location = Location {
            uri: uri.clone(),
            range: Range {
                start: position,
                end: position,
            },
        };
        Command {
            title,
            command: "editor.action.showReferences".to_string(),
            arguments: Some(vec![
                serde_json::json!(uri),
                serde_json::json!(position),
                serde_json::json!([location]),
            ]),
        }
    }

    /// Build a `Prototype` by locating the method's position in the
    /// ancestor's source file.
    fn build_prototype(
        &self,
        ancestor_fqn: &str,
        ancestor_class: &ClassInfo,
        method_name: &str,
        is_interface: bool,
        current_uri: &str,
        current_content: &str,
    ) -> Option<Prototype> {
        let (file_uri, file_content) =
            self.find_class_file_content(ancestor_fqn, current_uri, current_content)?;

        let name_offset = ancestor_class.member_name_offset(method_name, "method");

        let position = Self::find_member_position(
            &file_content,
            method_name,
            MemberKind::Method,
            name_offset,
        )?;

        // Determine whether to treat this as an interface based on the
        // ancestor's kind (the caller's hint is a fallback).
        let is_iface = ancestor_class.kind == ClassLikeKind::Interface || is_interface;

        Some(Prototype {
            ancestor_name: ancestor_class.name.to_string(),
            is_interface: is_iface,
            file_uri,
            position,
        })
    }
}
