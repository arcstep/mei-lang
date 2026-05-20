//! UI 数据绑定策略：scene 的 panel / block 树中禁止直连行集（`ds.data_ref` 物化形态）。
//!
//! 组件侧应使用 `world_ref("资源 id")`，由 `world(...)` / `world.add_resource` 与 legacy 物化合并后的
//! `resources` 表统一解析（见预览层 `resolve_value`）。

use serde_json::Value;
use std::collections::BTreeSet;

use crate::model::{Diagnostic, LoadedResource, PanelDecl, SceneContract, Severity, UiNodeDecl};

const IMPORTED_RESOURCE_DOC: &str =
    "see docs/mei-lang/implementation/syntax/12-public-scene-capsule-migration-and-diagnostics.md";

/// 在 catalog 合并后检查：UI `world_ref` 指向 catalog 中可见但未进入当前 scene world 授权表的资源。
pub(super) fn validate_imported_catalog_world_refs(
    contract: &SceneContract,
    authorized_resources: &[LoadedResource],
    merged_resources: &[LoadedResource],
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authorized_ids: BTreeSet<String> = authorized_resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    let merged_ids: BTreeSet<String> = merged_resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    for panel in &contract.panels {
        scan_panel_imported_refs(
            panel,
            &authorized_ids,
            &merged_ids,
            target_file,
            diagnostics,
        );
        for node in &panel.blocks {
            scan_ui_node_imported_refs(
                node,
                &authorized_ids,
                &merged_ids,
                target_file,
                diagnostics,
            );
        }
    }
}

fn scan_panel_imported_refs(
    panel: &PanelDecl,
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

fn scan_ui_node_imported_refs(
    node: &UiNodeDecl,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        UiNodeDecl::Panel(panel) => {
            scan_panel_imported_refs(panel, authorized_ids, merged_ids, target_file, diagnostics);
            for child in &panel.blocks {
                scan_ui_node_imported_refs(child, authorized_ids, merged_ids, target_file, diagnostics);
            }
        }
        UiNodeDecl::Block(block) => {
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
        UiNodeDecl::FrameRef(_) => {}
    }
}

fn push_imported_violations(
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
                "{context}：在 `{path}` 的 world_ref 引用资源 `{id}` 来自 catalog 合并未授权进当前 scene world；请通过 world.add_resource 或 capsule 迁移显式授权（{IMPORTED_RESOURCE_DOC}）"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
}

fn collect_imported_world_ref_paths(
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

fn imported_world_ref_id(
    map: &serde_json::Map<String, Value>,
    authorized_ids: &BTreeSet<String>,
    merged_ids: &BTreeSet<String>,
) -> Option<String> {
    let ref_kind = map.get("__ref").and_then(Value::as_str)?;
    if ref_kind != "world" && ref_kind != "dataset" && ref_kind != "resource" {
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

pub(super) fn validate_scene_ui_data_bindings(
    contract: &SceneContract,
    resources: &[LoadedResource],
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let resource_ids = resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<BTreeSet<_>>();
    for panel in &contract.panels {
        scan_panel_props(panel, &resource_ids, target_file, diagnostics);
        for node in &panel.blocks {
            scan_ui_node(node, &resource_ids, target_file, diagnostics);
        }
    }
}

fn scan_panel_props(
    panel: &PanelDecl,
    resource_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    push_violations(
        &panel.props,
        resource_ids,
        &format!("panel `{}` props", panel.id),
        target_file,
        diagnostics,
    );
}

fn scan_ui_node(
    node: &UiNodeDecl,
    resource_ids: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        UiNodeDecl::Panel(panel) => {
            scan_panel_props(panel, resource_ids, target_file, diagnostics);
            for child in &panel.blocks {
                scan_ui_node(child, resource_ids, target_file, diagnostics);
            }
        }
        UiNodeDecl::Block(block) => {
            push_violations(
                &block.props,
                resource_ids,
                &format!(
                    "block `{}` (use `{}`) props",
                    block.id.as_deref().unwrap_or("?"),
                    block.use_key
                ),
                target_file,
                diagnostics,
            );
        }
        UiNodeDecl::FrameRef(_) => {}
    }
}

fn push_violations(
    value: &Value,
    resource_ids: &BTreeSet<String>,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let paths = collect_forbidden_paths(value, "$");
    for path in paths {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "forbidden_direct_ui_data_binding".to_string(),
            message: format!(
                "{context}：在 `{path}` 发现禁止的数据直连（`ds.data_ref` / `__ref:\"data\"` / `analysis_expr` rows）；请改为 `world_ref(\"资源 id\")`，并确保该 id 出现在本入口编译产出的资源表中"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    let world_ref_issues = collect_invalid_world_ref_paths(value, "$", resource_ids);
    for (path, message) in world_ref_issues {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_world_resource_ref".to_string(),
            message: format!("{context}：在 `{path}` 发现非法 world_ref：{message}"),
            source_path: Some(target_file.to_string()),
        });
    }
}

fn collect_forbidden_paths(value: &Value, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            if forbidden_binding(map) {
                out.push(path.to_string());
            }
            for (key, child) in map {
                let next = format!("{path}.{key}");
                out.extend(collect_forbidden_paths(child, &next));
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                out.extend(collect_forbidden_paths(child, &next));
            }
        }
        _ => {}
    }
    out
}

fn forbidden_binding(map: &serde_json::Map<String, Value>) -> bool {
    if map.get("__ref").and_then(Value::as_str) == Some("data") {
        return true;
    }
    map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
        && map.get("type").and_then(Value::as_str) == Some("rows")
}

fn collect_invalid_world_ref_paths(
    value: &Value,
    path: &str,
    resource_ids: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            if let Some(issue) = world_ref_issue(map, resource_ids) {
                out.push((path.to_string(), issue));
            }
            for (key, child) in map {
                let next = format!("{path}.{key}");
                out.extend(collect_invalid_world_ref_paths(child, &next, resource_ids));
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                out.extend(collect_invalid_world_ref_paths(child, &next, resource_ids));
            }
        }
        _ => {}
    }
    out
}

fn world_ref_issue(
    map: &serde_json::Map<String, Value>,
    resource_ids: &BTreeSet<String>,
) -> Option<String> {
    let ref_kind = map.get("__ref").and_then(Value::as_str)?;
    if ref_kind != "world" && ref_kind != "dataset" && ref_kind != "resource" {
        return None;
    }
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if id.is_empty() {
        return Some("缺少资源 id".to_string());
    }
    if id == "__source_path__" || id.ends_with(".mei") {
        return Some(format!("资源 id `{id}` 已禁用；请使用稳定显式 id"));
    }
    if !resource_ids.contains(id) {
        return Some(format!("资源 id `{id}` 未在当前入口 world 资源清单中声明"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BlockDecl, SceneContract, SceneDecl};

    fn sample_scene() -> SceneDecl {
        serde_json::from_value(serde_json::json!({
            "kind": "scene",
            "id": "s",
            "state": {},
        }))
        .expect("scene")
    }

    #[test]
    fn flags_analysis_expr_rows_in_block_props() {
        let contract = SceneContract {
            scene: sample_scene(),
            themes: vec![],
            world: None,
            flow: None,
            frame: None,
            panels: vec![PanelDecl {
                kind: "panel".to_string(),
                id: "p1".to_string(),
                title: None,
                area: None,
                layout: None,
                blocks: vec![UiNodeDecl::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "dataset.table".to_string(),
                    id: Some("t1".to_string()),
                    title: None,
                    area: None,
                    props: serde_json::json!({
                        "data": {"__kind": "analysis_expr", "type": "rows", "dataset": "x"}
                    }),
                })],
                props: Value::Object(serde_json::Map::new()),
            }],
        };
        let mut diagnostics = Vec::new();
        validate_scene_ui_data_bindings(&contract, &[], "entry.mei", &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "forbidden_direct_ui_data_binding");
    }

    #[test]
    fn flags_imported_catalog_world_ref_as_warning() {
        let contract = SceneContract {
            scene: sample_scene(),
            themes: vec![],
            world: None,
            flow: None,
            frame: None,
            panels: vec![PanelDecl {
                kind: "panel".to_string(),
                id: "p1".to_string(),
                title: None,
                area: None,
                layout: None,
                blocks: vec![UiNodeDecl::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "dataset.table".to_string(),
                    id: None,
                    title: None,
                    area: None,
                    props: serde_json::json!({
                        "data": {"__ref": "world", "id": "catalog_only"}
                    }),
                })],
                props: Value::Object(serde_json::Map::new()),
            }],
        };
        let authorized = vec![LoadedResource {
            id: "local_only".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: None,
        }];
        let merged = vec![
            authorized[0].clone(),
            LoadedResource {
                id: "catalog_only".to_string(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: None,
            },
        ];
        let mut diagnostics = Vec::new();
        validate_imported_catalog_world_refs(
            &contract,
            &authorized,
            &merged,
            "entry.mei",
            &mut diagnostics,
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "imported_resource_not_authorized");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn allows_world_ref_in_props() {
        let contract = SceneContract {
            scene: sample_scene(),
            themes: vec![],
            world: None,
            flow: None,
            frame: None,
            panels: vec![PanelDecl {
                kind: "panel".to_string(),
                id: "p1".to_string(),
                title: None,
                area: None,
                layout: None,
                blocks: vec![UiNodeDecl::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "dataset.table".to_string(),
                    id: None,
                    title: None,
                    area: None,
                    props: serde_json::json!({
                        "data": {"__ref": "world", "id": "my_dataset"}
                    }),
                })],
                props: Value::Object(serde_json::Map::new()),
            }],
        };
        let mut diagnostics = Vec::new();
        let resources = vec![LoadedResource {
            id: "my_dataset".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: None,
        }];
        validate_scene_ui_data_bindings(&contract, &resources, "entry.mei", &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
