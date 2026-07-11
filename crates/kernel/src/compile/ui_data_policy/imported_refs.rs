use std::collections::BTreeSet;

use serde_json::Value;

use crate::model::{Diagnostic, Severity, UiNodeDecl, UiTreeNode};

use super::IMPORTED_RESOURCE_DOC;

pub(super) fn scan_panel_imported_refs(
    panel: &UiNodeDecl,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    push_imported_violations(
        &panel.props,
        authorized_ids,
        merged_ids,
        &format!("panel `{}` props", panel.id),
        target_file,
        diagnostics,
    );
}

pub(super) fn scan_ui_node_imported_refs(
    node: &UiTreeNode,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        UiTreeNode::Panel(panel) => {
            scan_panel_imported_refs(panel, authorized_ids, merged_ids, target_file, diagnostics);
            for child in &panel.blocks {
                scan_ui_node_imported_refs(
                    child,
                    authorized_ids,
                    merged_ids,
                    target_file,
                    diagnostics,
                );
            }
        }
        UiTreeNode::Block(block) => {
            push_imported_violations(
                &block.props,
                authorized_ids,
                merged_ids,
                &format!(
                    "block `{}` (use `{}`) props",
                    block.id.as_deref().unwrap_or("?"),
                    block.use_key
                ),
                target_file,
                diagnostics,
            );
        }
        UiTreeNode::PanelRefEmbed(_) => {}
    }
}

pub(super) fn push_imported_violations(
    value: &Value,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let paths = collect_imported_world_ref_paths(value, "$", authorized_ids, merged_ids);
    for (path, id) in paths {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "imported_resource_not_authorized".to_string(),
            message: format!(
                "{context}：在 `{path}` 的资源 ref 引用 `{id}` 来自 catalog 合并未授权进当前 scene world；请通过 world.add_resource 或 capsule 迁移显式授权（{IMPORTED_RESOURCE_DOC}）"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
}

pub(super) fn collect_imported_world_ref_paths(
    value: &Value,
    path: &str,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            if let Some(id) = imported_world_ref_id(map, authorized_ids, merged_ids) {
                out.push((path.to_string(), id));
            }
            for (key, child) in map {
                let next = format!("{path}.{key}");
                out.extend(collect_imported_world_ref_paths(
                    child,
                    &next,
                    authorized_ids,
                    merged_ids,
                ));
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                out.extend(collect_imported_world_ref_paths(
                    child,
                    &next,
                    authorized_ids,
                    merged_ids,
                ));
            }
        }
        _ => {}
    }
    out
}

pub(super) fn imported_world_ref_id(
    map: &serde_json::Map<String, Value>,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
) -> Option<String> {
    let ref_kind = map.get("__ref").and_then(Value::as_str)?;
    if ref_kind != "dataset"
        && ref_kind != "metric"
        && ref_kind != "resource"
        && ref_kind != "entity"
    {
        return None;
    }
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if id.is_empty() || id == "__source_path__" || id.ends_with(".mei") {
        return None;
    }
    if authorized_ids.contains(id) {
        return None;
    }
    if merged_ids.contains(id) {
        return Some(id.to_string());
    }
    None
}
