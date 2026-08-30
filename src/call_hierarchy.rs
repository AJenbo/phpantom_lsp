//! PHP call hierarchy support built on the existing definition and reference
//! pipelines.
//!
//! The hierarchy stores only stable declaration coordinates in LSP item data.
//! Incoming calls reuse Find References; outgoing calls reuse Go to Definition
//! for call-like symbol spans inside the callable body. This keeps call
//! hierarchy aligned with every improvement made to the shared type engine.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Location, Position,
    Range, SymbolKind as LspSymbolKind, Url,
};

use crate::Backend;
use crate::symbol_map::{SymbolKind, SymbolMap};
use crate::text_position::{offset_to_position, position_to_offset};
use crate::types::{ClassInfo, FunctionInfo, MethodInfo};

#[derive(Clone)]
struct PhpCallable {
    item: CallHierarchyItem,
    body: Option<(u32, u32)>,
}

impl Backend {
    pub(crate) fn prepare_call_hierarchy_impl(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<Vec<CallHierarchyItem>> {
        let offset = position_to_offset(content, position);
        if let Some(callable) = self.php_callable_at(uri, content, offset) {
            return Some(vec![callable.item]);
        }

        self.resolve_definition(uri, content, position)
            .into_iter()
            .find_map(|location| self.php_callable_at_location(&location))
            .map(|callable| vec![callable.item])
    }

    pub(crate) fn incoming_calls_impl(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<Vec<CallHierarchyIncomingCall>> {
        let target = self.php_callable_from_item(item)?;
        let content = self.get_file_content(target.item.uri.as_str())?;
        let references = self
            .find_references(
                target.item.uri.as_str(),
                &content,
                target.item.selection_range.start,
                false,
            )
            .unwrap_or_default();

        let mut grouped: HashMap<String, (CallHierarchyItem, Vec<Range>)> = HashMap::new();
        for reference in references {
            let Some(caller) = self.php_callable_at_location(&reference) else {
                continue;
            };
            let key = php_item_key(&caller.item);
            grouped
                .entry(key)
                .and_modify(|(_, ranges)| push_unique_range(ranges, reference.range))
                .or_insert_with(|| (caller.item, vec![reference.range]));
        }

        let mut calls: Vec<_> = grouped
            .into_values()
            .map(|(from, from_ranges)| CallHierarchyIncomingCall { from, from_ranges })
            .collect();
        calls.sort_by_key(|left| php_item_key(&left.from));
        calls.dedup_by(|left, right| {
            left.from == right.from && left.from_ranges == right.from_ranges
        });
        Some(calls)
    }

    pub(crate) fn outgoing_calls_impl(
        &self,
        item: &CallHierarchyItem,
    ) -> Option<Vec<CallHierarchyOutgoingCall>> {
        let callable = self.php_callable_from_item(item)?;
        let Some((body_start, body_end)) = callable.body else {
            return Some(Vec::new());
        };
        let uri = callable.item.uri.as_str();
        let content = self.get_file_content(uri)?;
        let symbol_map = self.symbol_maps.read().get(uri).cloned()?;

        let mut grouped: HashMap<String, (CallHierarchyItem, Vec<Range>)> = HashMap::new();
        for span in symbol_map.spans.iter().filter(|span| {
            span.start >= body_start
                && span.start <= body_end
                && matches!(
                    span.kind,
                    SymbolKind::FunctionCall {
                        is_definition: false,
                        ..
                    } | SymbolKind::MemberAccess {
                        is_method_call: true,
                        ..
                    }
                )
        }) {
            let position = offset_to_position(&content, span.start as usize);
            let from_range = Range::new(position, offset_to_position(&content, span.end as usize));
            for location in self.resolve_definition(uri, &content, position) {
                let Some(callee) = self.php_callable_at_location(&location) else {
                    continue;
                };
                let key = php_item_key(&callee.item);
                grouped
                    .entry(key)
                    .and_modify(|(_, ranges)| push_unique_range(ranges, from_range))
                    .or_insert_with(|| (callee.item, vec![from_range]));
            }
        }

        let mut calls: Vec<_> = grouped
            .into_values()
            .map(|(to, from_ranges)| CallHierarchyOutgoingCall { to, from_ranges })
            .collect();
        calls.sort_by_key(|left| php_item_key(&left.to));
        calls.dedup_by(|left, right| left.to == right.to && left.from_ranges == right.from_ranges);
        Some(calls)
    }

    fn php_callable_from_item(&self, item: &CallHierarchyItem) -> Option<PhpCallable> {
        let data = item.data.as_ref()?;
        if data.get("kind")?.as_str()? != "php" {
            return None;
        }
        let offset = data.get("offset")?.as_u64()? as u32;
        let content = self.get_file_content(item.uri.as_str())?;
        self.php_callable_at(item.uri.as_str(), &content, offset)
    }

    fn php_callable_at_location(&self, location: &Location) -> Option<PhpCallable> {
        let uri = location.uri.as_str();
        let content = self.get_file_content(uri)?;
        let offset = position_to_offset(&content, location.range.start);
        self.php_callable_at(uri, &content, offset)
    }

    fn php_callable_at(&self, uri: &str, content: &str, offset: u32) -> Option<PhpCallable> {
        let symbol_map = self.symbol_maps.read().get(uri).cloned()?;
        let classes = self
            .symbols
            .uri_classes_index
            .read()
            .get(uri)
            .cloned()
            .unwrap_or_default();

        for class in &classes {
            if let Some(callable) = method_callable_at(uri, content, &symbol_map, class, offset) {
                return Some(callable);
            }
        }

        let function_names = self
            .symbols
            .uri_globals_index
            .read()
            .get(uri)
            .map(|(functions, _)| functions.clone())
            .unwrap_or_default();
        let functions = self.symbols.global_functions.read();
        for fqn in function_names {
            let Some((declaring_uri, function)) = functions.get(&fqn) else {
                continue;
            };
            if declaring_uri == uri
                && let Some(callable) =
                    function_callable_at(uri, content, &symbol_map, &fqn, function, offset)
            {
                return Some(callable);
            }
        }
        None
    }
}

fn method_callable_at(
    uri: &str,
    content: &str,
    symbol_map: &SymbolMap,
    class: &ClassInfo,
    offset: u32,
) -> Option<PhpCallable> {
    for (index, method) in class.methods.iter().enumerate() {
        if method.is_virtual || method.name_offset == 0 {
            continue;
        }
        let upper = class
            .methods
            .iter()
            .skip(index + 1)
            .filter(|next| next.name_offset > method.name_offset)
            .map(|next| next.name_offset)
            .min()
            .unwrap_or(class.end_offset);
        let body = declaration_body(symbol_map, method.name_offset, upper);
        let name_end = method.name_offset.saturating_add(method.name.len() as u32);
        let contains = (method.name_offset..=name_end).contains(&offset)
            || body.is_some_and(|(start, end)| start <= offset && offset <= end);
        if contains {
            return build_method_callable(uri, content, class, method, body);
        }
    }
    None
}

fn function_callable_at(
    uri: &str,
    content: &str,
    symbol_map: &SymbolMap,
    fqn: &str,
    function: &FunctionInfo,
    offset: u32,
) -> Option<PhpCallable> {
    if function.name_offset == 0 {
        return None;
    }
    let body = declaration_body(symbol_map, function.name_offset, content.len() as u32);
    let name_end = function
        .name_offset
        .saturating_add(function.name.len() as u32);
    if !(function.name_offset..=name_end).contains(&offset)
        && !body.is_some_and(|(start, end)| start <= offset && offset <= end)
    {
        return None;
    }
    build_function_callable(uri, content, fqn, function, body)
}

fn declaration_body(symbol_map: &SymbolMap, name_offset: u32, upper: u32) -> Option<(u32, u32)> {
    symbol_map
        .scopes
        .iter()
        .copied()
        .filter(|(start, _)| *start > name_offset && *start < upper)
        .min_by_key(|(start, _)| *start)
}

fn build_method_callable(
    uri: &str,
    content: &str,
    class: &ClassInfo,
    method: &MethodInfo,
    body: Option<(u32, u32)>,
) -> Option<PhpCallable> {
    let uri = Url::parse(uri).ok()?;
    let selection_range = offset_range(content, method.name_offset, method.name.len() as u32);
    let range = Range::new(
        selection_range.start,
        body.map_or(selection_range.end, |(_, end)| {
            offset_to_position(content, end as usize)
        }),
    );
    let class_fqn = class.fqn().to_string();
    Some(PhpCallable {
        item: CallHierarchyItem {
            name: method.name.to_string(),
            kind: LspSymbolKind::METHOD,
            tags: None,
            detail: Some(class_fqn.clone()),
            uri,
            range,
            selection_range,
            data: Some(serde_json::json!({
                "kind": "php",
                "owner": class_fqn,
                "method": method.name.as_str(),
                "offset": method.name_offset,
            })),
        },
        body,
    })
}

fn build_function_callable(
    uri: &str,
    content: &str,
    fqn: &str,
    function: &FunctionInfo,
    body: Option<(u32, u32)>,
) -> Option<PhpCallable> {
    let uri = Url::parse(uri).ok()?;
    let selection_range = offset_range(content, function.name_offset, function.name.len() as u32);
    let range = Range::new(
        selection_range.start,
        body.map_or(selection_range.end, |(_, end)| {
            offset_to_position(content, end as usize)
        }),
    );
    Some(PhpCallable {
        item: CallHierarchyItem {
            name: function.name.to_string(),
            kind: LspSymbolKind::FUNCTION,
            tags: None,
            detail: function.namespace.clone(),
            uri,
            range,
            selection_range,
            data: Some(serde_json::json!({
                "kind": "php",
                "function": fqn,
                "offset": function.name_offset,
            })),
        },
        body,
    })
}

fn offset_range(content: &str, start: u32, len: u32) -> Range {
    Range::new(
        offset_to_position(content, start as usize),
        offset_to_position(content, start.saturating_add(len) as usize),
    )
}

fn php_item_key(item: &CallHierarchyItem) -> String {
    format!(
        "{}:{}:{}:{}",
        item.uri, item.selection_range.start.line, item.selection_range.start.character, item.name
    )
}

fn push_unique_range(ranges: &mut Vec<Range>, range: Range) {
    if !ranges.contains(&range) {
        ranges.push(range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "file:///call_hierarchy.php";

    fn parse(content: &str) -> Backend {
        let backend = Backend::new_test();
        backend
            .open_files
            .write()
            .insert(URI.to_string(), std::sync::Arc::new(content.to_string()));
        backend.update_ast(URI, content);
        backend
    }

    #[test]
    fn prepares_methods_and_resolves_outgoing_calls() {
        let content = r#"<?php
class Worker {
    public function leaf(): void {}
    public function run(): void { $this->leaf(); }
}
"#;
        let backend = parse(content);
        let run = backend
            .prepare_call_hierarchy_impl(URI, content, Position::new(3, 22))
            .unwrap()
            .remove(0);
        let outgoing = backend.outgoing_calls_impl(&run).unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to.name, "leaf");
        assert_eq!(outgoing[0].from_ranges.len(), 1);
    }

    #[test]
    fn resolves_incoming_calls_through_find_references() {
        let content = r#"<?php
class Worker {
    public function leaf(): void {}
    public function run(): void { $this->leaf(); }
}
"#;
        let backend = parse(content);
        let leaf = backend
            .prepare_call_hierarchy_impl(URI, content, Position::new(2, 22))
            .unwrap()
            .remove(0);
        let incoming = backend.incoming_calls_impl(&leaf).unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].from.name, "run");
    }
}
