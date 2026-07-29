//! Conservative diagnostics for project-local Symfony container symbols.

use std::collections::HashSet;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};

use crate::Backend;
use crate::framework::{FrameworkReferenceKind, SymfonySymbolKind};
use crate::text_position::offset_to_position;

impl Backend {
    pub(crate) fn collect_unknown_symfony_container_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(references) = self.framework_references.read().get(uri).cloned() else {
            return;
        };
        let known_services = self
            .framework_symfony_symbol_names(SymfonySymbolKind::Service)
            .into_iter()
            .collect::<HashSet<_>>();
        let known_parameters = self
            .framework_symfony_symbol_names(SymfonySymbolKind::Parameter)
            .into_iter()
            .collect::<HashSet<_>>();

        for reference in references.iter() {
            let FrameworkReferenceKind::SymfonySymbol {
                kind,
                name,
                declaration: false,
            } = &reference.kind
            else {
                continue;
            };

            let known = match kind {
                SymfonySymbolKind::Service => {
                    known_services.contains(name)
                        || (name.starts_with("App\\") && self.find_or_load_class(name).is_some())
                }
                SymfonySymbolKind::Parameter => known_parameters.contains(name),
            };
            if known || !is_project_local_name(*kind, name) {
                continue;
            }

            let label = kind.label();
            out.push(Diagnostic {
                range: Range {
                    start: offset_to_position(content, reference.start as usize),
                    end: offset_to_position(content, reference.end as usize),
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String(format!("unknown_symfony_{label}"))),
                source: Some("PHPantom".to_string()),
                message: format!("Symfony {label} '{}' is not declared", name),
                ..Default::default()
            });
        }
    }
}

fn is_project_local_name(kind: SymfonySymbolKind, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("app.") || (kind == SymfonySymbolKind::Service && name.starts_with("App\\"))
}
