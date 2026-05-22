//! UI 数据绑定策略：scene 的 panel / block 树中禁止直连行集（`ds.data_ref` 物化形态）。
//!
//! 组件 props 应使用本地 id 的 `dataset_ref` / `metric_ref` / `resource_ref`；
//! `world_ref` 仅用于 `scene.world` 单例槽位，不得作为资源选择器。

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::eval::evaluate_mei_file;
use crate::model::{
    BlockDecl, Diagnostic, LoadedResource, PanelDecl, PanelRefEmbedDecl, SceneContract, Severity,
    UiNodeDecl,
};

use super::entry_payload::helpers::{
    insert_resource_if_absent, load_resources_from_capsule_file,
};

const IMPORTED_RESOURCE_DOC: &str =
    "see docs/mei-lang/implementation/syntax/12-public-scene-capsule-migration-and-diagnostics.md";

/// 在 catalog 合并后检查：UI 资源 ref 指向 catalog 中可见但未进入当前 scene world 授权表的资源。
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
        UiNodeDecl::PanelRefEmbed(_) => {}
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
                "{context}：在 `{path}` 的资源 ref 引用 `{id}` 来自 catalog 合并未授权进当前 scene world；请通过 world.add_resource 或 capsule 迁移显式授权（{IMPORTED_RESOURCE_DOC}）"
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
    if ref_kind != "dataset" && ref_kind != "metric" && ref_kind != "resource" && ref_kind != "entity" {
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
    app_root: &Path,
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
            scan_deprecated_embed_nodes(node, target_file, diagnostics);
            if let UiNodeDecl::PanelRefEmbed(embed) = node {
                validate_embed_capsule_ui_bindings(
                    app_root,
                    embed,
                    resources,
                    target_file,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_embed_capsule_ui_bindings(
    app_root: &Path,
    embed: &PanelRefEmbedDecl,
    host_resources: &[LoadedResource],
    _target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = embed.scene_file.trim();
    if path.is_empty() {
        return;
    }
    let mut effective_resources = host_resources.to_vec();
    if let Ok(mut capsule_resources) = load_resources_from_capsule_file(app_root, path)
    {
        for resource in capsule_resources.drain(..) {
            insert_resource_if_absent(&mut effective_resources, resource);
        }
    }
    let resource_ids: BTreeSet<String> = effective_resources
        .iter()
        .map(|r| r.id.clone())
        .collect();
    let Ok(decls) = evaluate_mei_file(app_root.join(path)) else {
        return;
    };
    let Some(values) = decls.as_array() else {
        return;
    };
    for value in values {
        if value.get("kind").and_then(Value::as_str) == Some("panel") {
            if let Ok(panel) = serde_json::from_value::<PanelDecl>(value.clone()) {
                scan_panel_props(&panel, &resource_ids, path, diagnostics);
                for node in &panel.blocks {
                    scan_ui_node(node, &resource_ids, path, diagnostics);
                }
            }
        }
        if value.get("kind").and_then(Value::as_str) == Some("block") {
            if let Ok(block) = serde_json::from_value::<BlockDecl>(value.clone()) {
                push_violations(
                    &block.props,
                    &resource_ids,
                    &format!("block `{}` props", block.id.as_deref().unwrap_or("?")),
                    path,
                    diagnostics,
                );
            }
        }
    }
}

fn scan_deprecated_embed_nodes(
    node: &UiNodeDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node {
        UiNodeDecl::Panel(panel) => {
            for child in &panel.blocks {
                scan_deprecated_embed_nodes(child, target_file, diagnostics);
            }
        }
        UiNodeDecl::PanelRefEmbed(embed) => {
            if let Some(legacy) = embed.compat_source.as_deref() {
                let code = match legacy {
                    "frame_ref" => "deprecated_frame_ref_block_embed",
                    "panel_capsule_ref" => "deprecated_panel_capsule_ref",
                    _ => "deprecated_panel_ref_embed",
                };
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: code.to_string(),
                    message: format!(
                        "legacy {legacy} block embed `{}` is removed; use frame.panels panel_ref(id = ..., scene_file = ...)",
                        embed.scene_file
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
        }
        UiNodeDecl::Block(_) => {}
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
        UiNodeDecl::PanelRefEmbed(_) => {}
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
                "{context}：在 `{path}` 发现禁止的数据直连（`ds.data_ref` / `__ref:\"data\"` / `analysis_expr` rows）；请改为 `dataset_ref(id=...)` / `resource_ref(id=...)`，并确保该 id 已在当前 scene world 资源账本中"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    let ref_issues = collect_resource_ref_issues(value, "$", resource_ids);
    for (path, code, message) in ref_issues {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            message: format!("{context}：在 `{path}` {message}"),
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

fn has_external_locator(map: &serde_json::Map<String, Value>) -> bool {
    map.get("scene_file")
        .or_else(|| map.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
        || map
            .get("scene_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
}

fn collect_resource_ref_issues(
    value: &Value,
    path: &str,
    resource_ids: &BTreeSet<String>,
) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            if let Some((code, message)) = resource_ref_issue(map, resource_ids) {
                out.push((path.to_string(), code, message));
            }
            for (key, child) in map {
                let next = format!("{path}.{key}");
                out.extend(collect_resource_ref_issues(child, &next, resource_ids));
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                out.extend(collect_resource_ref_issues(child, &next, resource_ids));
            }
        }
        _ => {}
    }
    out
}

fn resource_ref_issue(
    map: &serde_json::Map<String, Value>,
    resource_ids: &BTreeSet<String>,
) -> Option<(String, String)> {
    let ref_kind = map.get("__ref").and_then(Value::as_str)?;
    if ref_kind == "world" {
        return Some((
            "misused_world_ref_in_props".to_string(),
            "误用 `world_ref` 作资源选择器；`world_ref` 仅用于 scene.world 单例槽位，资源消费请用 dataset_ref/resource_ref/metric_ref".to_string(),
        ));
    }
    if ref_kind != "dataset" && ref_kind != "metric" && ref_kind != "resource" && ref_kind != "entity" {
        return None;
    }
    if has_external_locator(map) {
        return Some((
            "external_ref_requires_world_import".to_string(),
            "不得在 frame/component props 中直接跨文件引用；请先在 world 中引入该对象".to_string(),
        ));
    }
    if ref_kind == "metric"
        && map
            .get("from_dataset")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
    {
        return None;
    }
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if id.is_empty() {
        return Some((
            "invalid_resource_ref".to_string(),
            "缺少资源 id".to_string(),
        ));
    }
    if id == "__source_path__" || id.ends_with(".mei") {
        return Some((
            "invalid_resource_ref".to_string(),
            format!("资源 id `{id}` 已禁用；请使用稳定显式 id"),
        ));
    }
    if !resource_ids.contains(id) {
        return Some((
            "invalid_resource_ref".to_string(),
            format!("资源 id `{id}` 未在当前 scene world 资源清单中可见"),
        ));
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
                head: None,
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
                    base: None,
                    layout: None,
                    blocks: vec![],
                    component: None,
                    placement: None,
                    interactions: vec![],
                    lifecycle: None,
                    constraints: None,
                    data: None,
                })],
                props: Value::Object(serde_json::Map::new()),
                base: None,
            }],
        };
        let mut diagnostics = Vec::new();
        validate_scene_ui_data_bindings(&contract, &[], Path::new("."), "entry.mei", &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "forbidden_direct_ui_data_binding");
    }

    #[test]
    fn flags_imported_catalog_resource_ref_as_warning() {
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
                head: None,
                area: None,
                layout: None,
                blocks: vec![UiNodeDecl::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "dataset.table".to_string(),
                    id: None,
                    title: None,
                    area: None,
                    props: serde_json::json!({
                        "data": {"__ref": "resource", "id": "catalog_only"}
                    }),
                    base: None,
                    layout: None,
                    blocks: vec![],
                    component: None,
                    placement: None,
                    interactions: vec![],
                    lifecycle: None,
                    constraints: None,
                    data: None,
                })],
                props: Value::Object(serde_json::Map::new()),
                base: None,
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
    fn flags_misused_world_ref_in_props() {
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
                head: None,
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
                    base: None,
                    layout: None,
                    blocks: vec![],
                    component: None,
                    placement: None,
                    interactions: vec![],
                    lifecycle: None,
                    constraints: None,
                    data: None,
                })],
                props: Value::Object(serde_json::Map::new()),
                base: None,
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
        validate_scene_ui_data_bindings(&contract, &resources, Path::new("."), "entry.mei", &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "misused_world_ref_in_props");
    }

    #[test]
    fn allows_resource_ref_in_props_when_authorized() {
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
                head: None,
                area: None,
                layout: None,
                blocks: vec![UiNodeDecl::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "dataset.table".to_string(),
                    id: None,
                    title: None,
                    area: None,
                    props: serde_json::json!({
                        "data": {"__ref": "resource", "id": "my_dataset"}
                    }),
                    base: None,
                    layout: None,
                    blocks: vec![],
                    component: None,
                    placement: None,
                    interactions: vec![],
                    lifecycle: None,
                    constraints: None,
                    data: None,
                })],
                props: Value::Object(serde_json::Map::new()),
                base: None,
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
        validate_scene_ui_data_bindings(&contract, &resources, Path::new("."), "entry.mei", &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}
