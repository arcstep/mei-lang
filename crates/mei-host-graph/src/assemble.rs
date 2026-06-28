use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    load_component_assets, normalize_panel_slots, resolve_app_root, CompiledApp, CompiledSceneRoute,
    LoadedResource, SceneContract, SceneDecl,
};
use serde_json::{json, Value};

use crate::import::load_block_artifact;
use crate::projection_normalize::normalize_board_assembly_payload;
use crate::v2_lower::{find_panel_contract_node, lower_frame_from_assembly, lower_panel_payload, PanelLowerContext};
use crate::mcg::registry::McgRegistryWriter;
use crate::types::GraphNodeKind;

#[derive(Debug, Clone)]
pub struct AssembleOutcome {
    pub compiled: CompiledApp,
    pub compile_revision: String,
    pub assembly_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScopeRoute {
    pub scene_id: String,
    pub url: String,
    pub assembly_key: String,
}

pub fn list_scope_routes(source_root: &Path, app_id: &str) -> Result<Vec<ScopeRoute>> {
    let registry = McgRegistryWriter::load(source_root, app_id);
    let app_root = resolve_app_root(source_root, app_id);
    let mut routes = Vec::new();
    for node in registry.nodes_of_kind(GraphNodeKind::Navigation) {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(app_root.as_path(), pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let scene_id = payload
            .get("scene")
            .and_then(|v| v.as_str())
            .unwrap_or("home")
            .to_string();
        let url = payload
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let assembly_key = extract_assembly_ref(&payload).unwrap_or_else(|| node.id.key.clone());
        if url.contains("/scene/") || node.id.key.contains("access") {
            routes.push(ScopeRoute {
                scene_id,
                url,
                assembly_key,
            });
        }
    }
    if routes.is_empty() {
        routes.push(ScopeRoute {
            scene_id: "home".to_string(),
            url: format!("/apps/app/{app_id}/scene/home"),
            assembly_key: "home@src/scene/home/assembly.mei".to_string(),
        });
    }
    Ok(routes)
}

pub fn assemble_scope_from_registry(
    source_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Result<Option<AssembleOutcome>> {
    let registry = McgRegistryWriter::load(source_root, app_id);
    if registry.nodes.is_empty() {
        return Ok(None);
    }
    let app_root = resolve_app_root(source_root, app_id);
    let app_root_str = app_root.display().to_string();

    let (title, _default_scene) = load_app_meta(app_root.as_path(), &registry)?;
    let assembly_key = resolve_assembly_key(&registry, scene_id);
    let assembly_payload = load_assembly_payload(app_root.as_path(), &registry, &assembly_key)?;
    let scene_routes = build_scene_routes(source_root, app_id, &registry)?;
    let resources = expand_runtime_metric_resources(crate::metric_hydrate::load_metric_resources_hydrated(
        app_root.as_path(),
        &registry,
    )?);
    let projection_map = load_projection_map(app_root.as_path(), &registry, &resources);
    let scene_local_nav_by_target = load_scene_local_nav_by_target(app_root.as_path(), &registry);
    let mut panels = load_panels_for_assembly(
        app_root.as_path(),
        app_id,
        &registry,
        &assembly_payload,
        scene_id,
    );
    let active_target = assembly_key_to_target(&assembly_key);
    let mut panel_diagnostics = Vec::new();
    normalize_panel_slots(
        &mut panels,
        &mut panel_diagnostics,
        active_target.as_str(),
    );
    let frame = Some(lower_frame_from_assembly(&assembly_payload));
    let component_assets = load_component_assets(source_root)?
        .into_values()
        .collect::<Vec<_>>();

    let scene_contract = SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: scene_id.to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: assembly_payload
                .get("profile")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            theme: assembly_payload
                .get("theme")
                .and_then(theme_ref_to_id)
                .or_else(|| {
                    assembly_payload
                        .get("theme")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                }),
            summary: assembly_payload
                .get("summary")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            goal: None,
            state: json!({}),
            shared: assembly_payload.get("shared").cloned().unwrap_or(json!({})),
            local_nav: assembly_payload
                .get("local_nav")
                .cloned()
                .unwrap_or(json!({})),
            params: assembly_payload.get("params").cloned().unwrap_or(json!({})),
            bindings: assembly_payload.get("bindings").cloned().unwrap_or(json!({})),
            examples: json!({}),
            access_export: true,
        },
        themes: Vec::new(),
        shared: json!({}),
        world: None,
        flow: None,
        frame,
        panels,
    };

    let compiled = CompiledApp {
        app_id: app_id.to_string(),
        title,
        app_root: app_root_str,
        scene_routes,
        active_scene: Some(scene_id.to_string()),
        active_target_file: active_target,
        file_tree: Vec::new(),
        scene_contract: Some(scene_contract),
        scene_local_nav_by_target,
        scene_bindings_by_id: load_link_bindings(app_root.as_path(), &registry),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: projection_map,
        resources,
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets,
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
    };

    Ok(Some(AssembleOutcome {
        compiled,
        compile_revision: registry.registry_revision.clone(),
        assembly_key,
    }))
}

fn load_app_meta(app_root: &Path, registry: &crate::mcg::registry::McgRegistry) -> Result<(String, String)> {
    let skeleton_node = registry
        .nodes
        .iter()
        .find(|n| n.id.kind == GraphNodeKind::AppSkeleton);
    if let Some(node) = skeleton_node {
        if let Some(pref) = node.payload_ref.as_ref() {
            if let Some(artifact) = load_block_artifact(app_root, pref)? {
                let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
                let title = payload
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("App")
                    .to_string();
                let default_scene = payload
                    .get("default_scene")
                    .and_then(|v| v.as_str())
                    .unwrap_or("home")
                    .to_string();
                return Ok((title, default_scene));
            }
        }
    }
    Ok((registry.app_id.clone(), "home".to_string()))
}

fn resolve_assembly_key(registry: &crate::mcg::registry::McgRegistry, scene_id: &str) -> String {
    if scene_id == "home" {
        return registry
            .nodes
            .iter()
            .find(|n| {
                n.id.kind == GraphNodeKind::AssemblyView && n.id.key.contains("home@")
            })
            .map(|n| n.id.key.clone())
            .unwrap_or_else(|| "home@src/scene/home/assembly.mei".to_string());
    }
    registry
        .nodes
        .iter()
        .find(|n| {
            n.id.kind == GraphNodeKind::AssemblyView
                && (n.id.key.contains(scene_id) || n.id.key.contains(&format!("#{scene_id}")))
        })
        .map(|n| n.id.key.clone())
        .unwrap_or_else(|| format!("overlay/boards/{scene_id}"))
}

fn load_assembly_payload(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
    assembly_key: &str,
) -> Result<Value> {
    let node = registry
        .nodes
        .iter()
        .find(|n| n.id.kind == GraphNodeKind::AssemblyView && n.id.key == assembly_key)
        .with_context(|| format!("assembly view not found: {assembly_key}"))?;
    let pref = node
        .payload_ref
        .as_ref()
        .context("assembly view missing payload ref")?;
    let artifact = load_block_artifact(app_root, pref)?
        .with_context(|| format!("assembly artifact missing for {assembly_key}"))?;
    Ok(artifact.get("payload").cloned().unwrap_or(json!({})))
}

pub(crate) fn assembly_key_to_target(assembly_key: &str) -> String {
    if let Some((_, path)) = assembly_key.split_once('@') {
        return path.to_string();
    }
    if let Some((board_path, _scene)) = assembly_key.split_once('#') {
        return overlay_board_path_to_source_file(board_path);
    }
    format!("src/{assembly_key}.mei")
}

fn overlay_board_path_to_source_file(board_path: &str) -> String {
    let stem = board_path
        .strip_prefix("overlay/boards/")
        .unwrap_or(board_path);
    if stem.contains(".card") || stem.ends_with("-detail") {
        format!("src/overlay/boards/{stem}.mei")
    } else {
        format!("src/overlay/boards/{stem}.board.mei")
    }
}

fn build_scene_routes(
    source_root: &Path,
    app_id: &str,
    registry: &crate::mcg::registry::McgRegistry,
) -> Result<Vec<CompiledSceneRoute>> {
    let mut routes = Vec::new();
    for route in list_scope_routes(source_root, app_id)? {
        routes.push(CompiledSceneRoute {
            scene_id: route.scene_id.clone(),
            frame_id: None,
            target_file: assembly_key_to_target(&route.assembly_key),
            kind: "scene".to_string(),
            title: None,
            is_default: route.scene_id == "home",
            access_export: true,
        });
    }
    if routes.is_empty() {
        for node in registry.nodes_of_kind(GraphNodeKind::AssemblyView) {
            let scene = node.id.key.split('#').next_back().unwrap_or("home");
            routes.push(CompiledSceneRoute {
                scene_id: scene.to_string(),
                frame_id: None,
                target_file: assembly_key_to_target(&node.id.key),
                kind: if node.id.key.contains("home@") {
                    "scene".to_string()
                } else {
                    "board".to_string()
                },
                title: None,
                is_default: node.id.key.contains("home@"),
                access_export: true,
            });
        }
    }
    Ok(routes)
}

fn expand_runtime_metric_resources(resources: Vec<LoadedResource>) -> Vec<LoadedResource> {
    let mut expanded = resources;
    let seed = expanded.clone();
    for resource in seed {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        let mut alias_ids = vec![dataset.id.clone()];
        alias_ids.extend(dataset.runtime_metric_defs.keys().cloned());
        for alias in alias_ids {
            let alias = alias.trim().to_string();
            if alias.is_empty() || expanded.iter().any(|entry| entry.id == alias) {
                continue;
            }
            expanded.push(LoadedResource {
                id: alias,
                kind: resource.kind.clone(),
                title: resource.title.clone(),
                document: resource.document.clone(),
                dataset: Some(dataset.clone()),
            });
        }
    }
    expanded
}

fn load_projection_map(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
    resources: &[mei_lang_kernel::LoadedResource],
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for node in registry.nodes_of_kind(GraphNodeKind::AssemblyView) {
        if node.id.key.contains("home@") {
            continue;
        }
        if let Some(pref) = node.payload_ref.as_ref() {
            if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
                let payload = artifact.get("payload").cloned().unwrap_or(json!({}));
                let scene_id = payload
                    .get("scene")
                    .and_then(|v| v.as_str())
                    .or_else(|| node.id.key.split('#').next_back())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !scene_id.is_empty() {
                    let mut normalized = normalize_board_assembly_payload(payload);
                    if let Some(assembly) = normalized.as_object_mut() {
                        let _ = mei_lang_kernel::enrich_runtime_board_assembly_projection_slots(
                            assembly,
                            resources,
                            scene_id.as_str(),
                        );
                    }
                    map.insert(scene_id, normalized);
                }
            }
        }
    }
    map
}

fn load_scene_local_nav_by_target(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for node in registry.nodes_of_kind(GraphNodeKind::AssemblyView) {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(app_root, pref) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(json!({}));
        let local_nav = payload.get("local_nav").cloned().unwrap_or(json!({}));
        let is_empty = local_nav
            .as_object()
            .map(|obj| obj.is_empty())
            .unwrap_or(local_nav.is_null());
        if is_empty {
            continue;
        }
        map.insert(assembly_key_to_target(&node.id.key), local_nav);
    }
    map
}

fn load_link_bindings(app_root: &Path, registry: &crate::mcg::registry::McgRegistry) -> BTreeMap<String, Value> {
    let mut bindings = BTreeMap::new();
    for node in registry.nodes.iter().filter(|n| n.id.kind == GraphNodeKind::Navigation && n.id.key.starts_with("overlay/")) {
        if let Some(pref) = node.payload_ref.as_ref() {
            if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
                bindings.insert(node.id.key.clone(), artifact.get("payload").cloned().unwrap_or(json!({})));
            }
        }
    }
    for node in registry.nodes.iter().filter(|n| n.id.kind == GraphNodeKind::Navigation) {
        if node.id.key.contains("link") {
            if let Some(pref) = node.payload_ref.as_ref() {
                if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
                    bindings.insert(node.id.key.clone(), artifact.get("payload").cloned().unwrap_or(json!({})));
                }
            }
        }
    }
    bindings
}

fn load_panels_for_assembly(
    app_root: &Path,
    app_id: &str,
    registry: &crate::mcg::registry::McgRegistry,
    assembly_payload: &Value,
    scene_id: &str,
) -> Vec<mei_lang_kernel::PanelDecl> {
    let lower_ctx = PanelLowerContext {
        app_root,
        app_id,
        registry,
        scene_id,
        panel_constants: BTreeMap::new(),
    };
    let mut panels = Vec::new();
    let panel_refs = assembly_payload
        .get("panels")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for panel_ref in panel_refs {
        let panel_key = extract_panel_ref_key(&panel_ref);
        let Some(panel_key) = panel_key else {
            continue;
        };
        let contract_key = normalize_panel_contract_key(&panel_key, assembly_payload);
        let Some(node) = find_panel_contract_node(registry, contract_key.as_str(), scene_id) else {
            continue;
        };
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(app_root, pref) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(json!({}));
        let panel_ctx = lower_ctx.with_panel_constants(contract_key.as_str());
        if let Ok(panel) = lower_panel_payload(&payload, contract_key.as_str(), &panel_ctx) {
            panels.push(panel);
        }
    }

    panels
}

fn theme_ref_to_id(value: &Value) -> Option<String> {
    if let Some(args) = value.get("__args").and_then(|v| v.as_object()) {
        return args
            .get("arg0")
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    None
}

fn extract_panel_ref_key(value: &Value) -> Option<String> {
    if let Some(args) = value.get("__args").and_then(|v| v.as_object()) {
        if let Some(arg0) = args.get("arg0").and_then(|v| v.as_str()) {
            return Some(arg0.to_string());
        }
    }
    value.get("id").and_then(|v| v.as_str()).map(str::to_string)
}

fn normalize_panel_contract_key(panel_key: &str, assembly_payload: &Value) -> String {
    if panel_key.contains(':') {
        return panel_key.to_string();
    }
    let scene = assembly_payload
        .get("scene")
        .and_then(|v| v.as_str())
        .unwrap_or("home");
    format!("{scene}:{panel_key}")
}

fn extract_assembly_ref(payload: &Value) -> Option<String> {
    payload
        .get("assembly")
        .and_then(|v| {
            if let Some(args) = v.get("__args").and_then(|o| o.as_object()) {
                args.get("arg0").and_then(|a| a.as_str()).map(str::to_string)
            } else {
                v.as_str().map(str::to_string)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembly_key_to_target_home() {
        assert_eq!(
            assembly_key_to_target("home@src/scene/home/assembly.mei"),
            "src/scene/home/assembly.mei"
        );
    }

    #[test]
    fn assembly_key_to_target_overlay_board() {
        assert_eq!(
            assembly_key_to_target(
                "overlay/boards/supervision-warning#warnings_analytics_board"
            ),
            "src/overlay/boards/supervision-warning.board.mei"
        );
        assert_eq!(
            assembly_key_to_target("overlay/boards/warning-detail.card#warning_detail_card_board"),
            "src/overlay/boards/warning-detail.card.mei"
        );
    }
}
