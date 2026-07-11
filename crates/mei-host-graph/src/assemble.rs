use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    load_component_assets, load_mei_config_for_app, normalize_panel_slots, resolve_app_root,
    CompiledApp, CompiledSceneRoute, ComponentAsset, LoadedResource, SceneContract, SceneDecl,
    UiNodeDecl, UiTreeNode,
};
use serde_json::{json, Value};

use crate::import::load_block_artifact;
use crate::layer_plan::{build_layer_plan, flatten_panel_tree, layer_plan_to_value};
use crate::mcg::registry::McgRegistryWriter;
use crate::presentation_map::{build_presentation_map, presentation_map_to_value};
use crate::projection_normalize::normalize_page_instance_payload;
use crate::semantic_scene::{
    assemble_semantic_scene, has_semantic_scene, load_semantic_scene_payload,
};
use crate::tier::canonical_tier;
use crate::types::GraphNodeKind;
use crate::v2_lower::{
    find_content_panel_node, lower_frame_from_assembly, lower_panel_payload,
    lower_v2_inline_panels_from_assembly, PanelLowerContext,
};
use crate::world_plan::build_world_exchange;

#[derive(Debug, Clone)]
pub struct AssembleOutcome {
    pub compiled: CompiledApp,
    pub compile_revision: String,
    pub assembly_key: String,
    pub layer_plan: Value,
    pub presentation_map: Value,
    pub world_plan: Value,
    pub map_projection: Value,
    pub overlay_defaults: BTreeMap<String, Value>,
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
        if url.contains(&format!("/apps/{app_id}/"))
            || url.contains("/scene/")
            || node.id.key.contains("access")
        {
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
            url: format!("/apps/{app_id}/home"),
            assembly_key: mei_lang_kernel::default_scene_assembly_key(app_root.as_path(), "home"),
        });
    }
    Ok(routes)
}

/// Board page scene ids (`page_instance.scene`) under a T2 section scope path.
pub fn t2_page_scenes_for_section_scope(
    source_root: &Path,
    app_id: &str,
    section_scope: &str,
) -> Vec<String> {
    let section_scope = section_scope.trim();
    if section_scope.is_empty() {
        return Vec::new();
    }
    let registry = McgRegistryWriter::load(source_root, app_id);
    let app_root = resolve_app_root(source_root, app_id);
    let section_keys = [
        section_scope.to_string(),
        format!("{app_id}/{section_scope}"),
    ];
    let mut assembly_refs = Vec::new();
    for node in registry.nodes_of_kind(GraphNodeKind::SemanticGraph) {
        if !section_keys.iter().any(|key| node.id.key == key.as_str()) {
            continue;
        }
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(app_root.as_path(), pref) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(json!({}));
        collect_assembly_refs_from_payload(&payload, &mut assembly_refs);
    }
    let mut scenes = BTreeSet::new();
    for assembly_key in assembly_refs {
        for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
            if node.id.key != assembly_key {
                continue;
            }
            if let Some(scene_id) = board_scene_id_for_node(app_root.as_path(), node) {
                scenes.insert(scene_id);
            }
        }
    }
    if !scenes.is_empty() {
        return scenes.into_iter().collect();
    }
    let section_needles = section_keys;
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
        if node.id.key.contains("home@") {
            continue;
        }
        let in_section = section_needles
            .iter()
            .any(|needle| node.id.key.contains(needle.as_str()));
        if !in_section {
            continue;
        }
        if let Some(scene_id) = board_scene_id_for_node(app_root.as_path(), node) {
            scenes.insert(scene_id);
        }
    }
    scenes.into_iter().collect()
}

fn collect_assembly_refs_from_payload(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("assembly_ref") {
                if let Some(arg0) = map
                    .get("__args")
                    .and_then(Value::as_object)
                    .and_then(|args| args.get("arg0"))
                    .and_then(Value::as_str)
                {
                    out.push(arg0.to_string());
                }
            }
            for entry in map.values() {
                collect_assembly_refs_from_payload(entry, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_assembly_refs_from_payload(item, out);
            }
        }
        _ => {}
    }
}

/// Collect scene ids for all T2 page assembly views (warmup / smoke tests).
pub fn collect_all_t2_page_scenes(source_root: &Path, app_id: &str) -> Vec<String> {
    let registry = McgRegistryWriter::load(source_root, app_id);
    let app_root = resolve_app_root(source_root, app_id);
    let mut scenes = BTreeSet::new();
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
        if node.id.key.contains("home@") {
            scenes.insert("home".to_string());
            continue;
        }
        if let Some(scene_id) = board_scene_id_for_node(app_root.as_path(), node) {
            scenes.insert(scene_id);
        }
    }
    scenes.into_iter().collect()
}

pub fn assemble_scope_from_registry(
    source_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Result<Option<AssembleOutcome>> {
    let semantic_core =
        crate::view_artifact::build_semantic_core_for_scene(source_root, app_id, scene_id);
    if let Some(cache_key) = crate::assemble_cache::assemble_cache_key(&semantic_core) {
        if let Some(cached) = crate::assemble_cache::take_assemble_outcome(cache_key.as_str()) {
            return Ok(Some(cached));
        }
    }
    let outcome = assemble_scope_from_registry_uncached(source_root, app_id, scene_id)?;
    if let (Some(outcome), Some(cache_key)) = (
        outcome.as_ref(),
        crate::assemble_cache::assemble_cache_key(&semantic_core),
    ) {
        crate::assemble_cache::store_assemble_outcome(cache_key, outcome.clone());
    }
    Ok(outcome)
}

fn assemble_scope_from_registry_uncached(
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
    let scene_id = resolve_scene_id_for_assembly(source_root, app_id, &registry, scene_id);
    let assembly_key = resolve_assembly_key(source_root, app_id, &registry, scene_id.as_str());
    let scene_routes = build_scene_routes(source_root, app_id, &registry)?;
    let resources = expand_runtime_metric_resources(
        crate::metric_hydrate::load_metric_resources_hydrated(app_root.as_path(), &registry)?,
    );
    let projection_map = load_projection_map(app_root.as_path(), &registry, &resources);
    let scene_examples_by_id = load_scene_examples_by_id(app_root.as_path(), &registry);
    let mut scene_local_nav_by_target =
        load_scene_local_nav_by_target(app_root.as_path(), &registry);
    let active_target =
        assembly_target_for_key(app_root.as_path(), &registry, assembly_key.as_str());
    let overlay_defaults = load_overlay_defaults(app_root.as_path(), &registry);
    let (
        scene_summary,
        scene_profile,
        scene_theme,
        scene_shared,
        scene_local_nav,
        scene_params,
        scene_capabilities,
        scene_bindings,
        scene_examples,
        frame,
        mut panels,
        panel_payloads,
        mut panel_diagnostics,
    ) = if has_semantic_scene(&registry)
        && registry
            .nodes
            .iter()
            .any(|node| node.id.kind == GraphNodeKind::SemanticGraph && node.id.key == assembly_key)
    {
        let semantic_payload =
            load_semantic_scene_payload(app_root.as_path(), &registry, &assembly_key)?;
        let semantic_ctx = PanelLowerContext {
            app_root: app_root.as_path(),
            app_id,
            registry: &registry,
            scene_id: scene_id.as_str(),
            panel_constants: BTreeMap::new(),
            assembly_stack_order: None,
        };
        let semantic = assemble_semantic_scene(&semantic_payload, &semantic_ctx)?;
        (
            semantic.summary,
            semantic.profile,
            semantic.theme,
            semantic.shared,
            semantic.local_nav,
            semantic.params,
            semantic.capabilities,
            semantic.bindings,
            json!({}),
            Some(semantic.frame),
            semantic.panels,
            semantic.panel_payloads,
            Vec::new(),
        )
    } else {
        let assembly_payload = normalize_page_instance_payload(load_assembly_payload(
            app_root.as_path(),
            &registry,
            &assembly_key,
        )?);
        let (panels, panel_payloads) = load_panels_for_assembly(
            app_root.as_path(),
            app_id,
            &registry,
            &assembly_payload,
            &scene_id,
        );
        (
            assembly_payload
                .get("summary")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            assembly_payload
                .get("profile")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            assembly_payload.get("theme").cloned(),
            assembly_payload.get("shared").cloned().unwrap_or(json!({})),
            assembly_payload
                .get("local_nav")
                .cloned()
                .unwrap_or(json!({})),
            assembly_payload.get("params").cloned().unwrap_or(json!({})),
            assembly_payload
                .get("capabilities")
                .cloned()
                .unwrap_or(Value::Null),
            assembly_payload
                .get("bindings")
                .cloned()
                .unwrap_or(json!({})),
            assembly_payload
                .get("examples")
                .cloned()
                .unwrap_or(json!({})),
            Some(lower_frame_from_assembly(&assembly_payload)),
            panels,
            panel_payloads,
            Vec::new(),
        )
    };
    normalize_panel_slots(&mut panels, &mut panel_diagnostics, active_target.as_str());
    let flat_panels = flatten_panel_tree(&panels);
    let layer_plan = layer_plan_to_value(&build_layer_plan(&scene_id, &flat_panels));
    let presentation_map = presentation_map_to_value(&build_presentation_map(
        &scene_id,
        &flat_panels,
        &panel_payloads,
    ));
    let world_exchange =
        build_world_exchange(app_root.as_path(), &registry, app_id).unwrap_or_default();
    let component_assets = collect_component_assets_for_panels(source_root, &panels)?;
    if !scene_contract_local_nav_is_empty(&scene_local_nav) {
        scene_local_nav_by_target.insert(active_target.clone(), scene_local_nav.clone());
    }

    let scene_contract = SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: scene_id.to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: scene_profile,
            theme: scene_theme.as_ref().and_then(theme_ref_to_id).or_else(|| {
                scene_theme
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            }),
            summary: scene_summary,
            goal: None,
            state: json!({}),
            shared: scene_shared,
            local_nav: scene_local_nav,
            params: scene_params,
            capabilities: scene_capabilities,
            bindings: scene_bindings,
            examples: scene_examples,
            access_export: true,
        },
        themes: Vec::new(),
        shared: json!({}),
        world: None,
        flow: None,
        frame,
        panels,
    };

    let mut compiled = CompiledApp {
        app_id: app_id.to_string(),
        title,
        app_root: app_root_str,
        scene_routes,
        active_scene: Some(scene_id.clone()),
        active_target_file: active_target.clone(),
        file_tree: Vec::new(),
        scene_contract: Some(scene_contract),
        scene_local_nav_by_target,
        scene_bindings_by_id: load_link_bindings(app_root.as_path(), &registry),
        scene_examples_by_id,
        scene_projection_assembly_by_id: projection_map,
        resources,
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets,
        diagnostics: panel_diagnostics,
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };

    load_mei_config_for_app(app_root.as_path(), Some(source_root));
    compiled = crate::enrich_compiled_scope::enrich_compiled_scope(
        compiled,
        source_root,
        app_id,
        crate::enrich_compiled_scope::EnrichCompiledScopeOptions::default(),
    );

    crate::mrg::telemetry::record_access(crate::mrg::telemetry::MrgAccessKind::Assemble, true);

    Ok(Some(AssembleOutcome {
        compiled,
        compile_revision: registry.registry_revision.clone(),
        assembly_key,
        layer_plan,
        presentation_map,
        world_plan: world_exchange.world_plan,
        map_projection: world_exchange.map_projection,
        overlay_defaults,
    }))
}

fn load_app_meta(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> Result<(String, String)> {
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

fn resolve_scene_id_for_assembly(
    source_root: &Path,
    app_id: &str,
    registry: &crate::mcg::registry::McgRegistry,
    scene_id: &str,
) -> String {
    let scene_id = canonical_scene_id(scene_id);
    if scene_id != "assembly" {
        return scene_id;
    }
    if let Ok(routes) = list_scope_routes(source_root, app_id) {
        if let Some(route) = routes.into_iter().find(|route| {
            let target = assembly_key_to_target(&route.assembly_key);
            target.ends_with("/assembly.mei")
                || target == "assembly.mei"
                || target
                    .strip_prefix("src/scene/")
                    .is_some_and(|rest| rest.ends_with(".mei") && !rest.contains('/'))
        }) {
            return route.scene_id;
        }
    }
    registry
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                n.id.kind,
                GraphNodeKind::PageInstance | GraphNodeKind::SemanticGraph
            ) && (n.id.key.contains("/assembly.mei")
                || n.id.key.contains("@src/scene/") && n.id.key.ends_with(".mei"))
        })
        .filter_map(|n| {
            let resolved = canonical_scene_id(&n.id.key);
            (!resolved.is_empty() && resolved != "assembly").then_some(resolved)
        })
        .next()
        .unwrap_or(scene_id)
}

fn canonical_scene_id(scene_id: &str) -> String {
    let trimmed = scene_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some((_, tail)) = trimmed.split_once("/scene/") {
        let head = tail
            .split('/')
            .next()
            .unwrap_or(tail)
            .trim_end_matches(".mei");
        if !head.is_empty() {
            return head.to_string();
        }
    }
    if let Some(parent) = trimmed.strip_suffix("/assembly.mei") {
        if let Some(head) = parent.rsplit('/').next() {
            if !head.is_empty() {
                return head.to_string();
            }
        }
    }
    if let Some((head, _)) = trimmed.split_once('/') {
        if !head.is_empty() {
            return head.to_string();
        }
    }
    trimmed.to_string()
}

fn resolve_assembly_key(
    source_root: &Path,
    app_id: &str,
    registry: &crate::mcg::registry::McgRegistry,
    scene_id: &str,
) -> String {
    let scene_id = resolve_scene_id_for_assembly(source_root, app_id, registry, scene_id);
    if scene_id == "home" {
        return registry
            .nodes
            .iter()
            .find(|n| {
                matches!(
                    n.id.kind,
                    GraphNodeKind::PageInstance | GraphNodeKind::SemanticGraph
                ) && n.id.key.contains("home@")
            })
            .map(|n| n.id.key.clone())
            .unwrap_or_else(|| {
                let app_root = resolve_app_root(source_root, app_id);
                mei_lang_kernel::default_scene_assembly_key(app_root.as_path(), "home")
            });
    }
    if let Ok(routes) = list_scope_routes(source_root, app_id) {
        if let Some(route) = routes.into_iter().find(|route| route.scene_id == scene_id) {
            return route.assembly_key;
        }
    }
    let app_root = resolve_app_root(source_root, app_id);
    if let Some(key) = find_assembly_key_by_scene(app_root.as_path(), registry, scene_id.as_str()) {
        return key;
    }
    registry
        .nodes
        .iter()
        .find(|n| {
            matches!(
                n.id.kind,
                GraphNodeKind::PageInstance | GraphNodeKind::SemanticGraph
            ) && n.id.key.split('#').next_back() == Some(scene_id.as_str())
        })
        .map(|n| n.id.key.clone())
        .unwrap_or_else(|| format!("overlay/t2/{scene_id}"))
}

pub fn board_scene_id_for_node(
    app_root: &Path,
    node: &crate::mcg::registry::McgNodeRecord,
) -> Option<String> {
    if let Some(pref) = node.payload_ref.as_ref() {
        if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
            if let Some(scene_id) = artifact
                .get("payload")
                .and_then(|payload| payload.get("scene"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(scene_id.to_string());
            }
        }
    }
    node.id
        .key
        .split('#')
        .next_back()
        .map(str::to_string)
        .filter(|value| !value.is_empty() && value != &node.id.key)
}

pub(crate) fn find_assembly_key_by_scene(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
    scene_id: &str,
) -> Option<String> {
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
        if node.id.key.contains("home@") {
            continue;
        }
        if board_scene_id_for_node(app_root, node).as_deref() == Some(scene_id) {
            return Some(node.id.key.clone());
        }
    }
    None
}

fn load_assembly_payload(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
    assembly_key: &str,
) -> Result<Value> {
    let node = registry
        .nodes
        .iter()
        .find(|n| n.id.kind == GraphNodeKind::PageInstance && n.id.key == assembly_key)
        .with_context(|| format!("assembly view not found: {assembly_key}"))?;
    let pref = node
        .payload_ref
        .as_ref()
        .context("assembly view missing payload ref")?;
    let artifact = load_block_artifact(app_root, pref)?
        .with_context(|| format!("assembly artifact missing for {assembly_key}"))?;
    Ok(artifact.get("payload").cloned().unwrap_or(json!({})))
}

pub(crate) fn assembly_source_file_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("source_file")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|source_file| {
            if source_file.starts_with("src/") {
                source_file.to_string()
            } else {
                format!("src/{source_file}")
            }
        })
}

pub(crate) fn assembly_target_for_node(
    app_root: &Path,
    node: &crate::mcg::registry::McgNodeRecord,
) -> String {
    if let Some(pref) = node.payload_ref.as_ref() {
        if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
            if let Some(payload) = artifact.get("payload") {
                if let Some(target) = assembly_source_file_from_payload(payload) {
                    return target;
                }
            }
        }
    }
    assembly_key_to_target(&node.id.key)
}

pub(crate) fn assembly_target_for_key(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
    assembly_key: &str,
) -> String {
    if let Some(node) = registry
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::PageInstance && node.id.key == assembly_key)
    {
        return assembly_target_for_node(app_root, node);
    }
    assembly_key_to_target(assembly_key)
}

pub(crate) fn assembly_key_to_target(assembly_key: &str) -> String {
    if let Some((_, path)) = assembly_key.split_once('@') {
        return path.to_string();
    }
    if let Some((assembly_path, _scene)) = assembly_key.split_once('#') {
        return overlay_assembly_path_to_source_file(assembly_path);
    }
    format!("src/{assembly_key}.mei")
}

fn overlay_assembly_path_to_source_file(assembly_path: &str) -> String {
    if let Some(stem) = assembly_path.strip_prefix("overlay/t2/") {
        if stem.ends_with(".page") || stem.ends_with(".board") {
            return format!("src/overlay/t2/{stem}.mei");
        }
        return format!("src/overlay/t2/{stem}.page.mei");
    }
    let stem = assembly_path
        .strip_prefix("overlay/boards/")
        .unwrap_or(assembly_path);
    if stem.contains(".card") || stem.ends_with("-detail") {
        format!("src/overlay/boards/{stem}.mei")
    } else {
        format!("src/overlay/boards/{stem}.board.mei")
    }
}

fn route_kind_for_target(target_file: &str) -> String {
    let normalized = target_file.replace('\\', "/").to_ascii_lowercase();
    if normalized.contains("/presentation/") || normalized.starts_with("presentation/") {
        "presentation".to_string()
    } else {
        "scene".to_string()
    }
}

fn scene_title_from_assembly(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
    assembly_key: &str,
) -> Option<String> {
    let payload = load_assembly_payload(app_root, registry, assembly_key).ok()?;
    payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_scene_routes(
    source_root: &Path,
    app_id: &str,
    registry: &crate::mcg::registry::McgRegistry,
) -> Result<Vec<CompiledSceneRoute>> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut routes = Vec::new();
    let mut seen_scenes = BTreeSet::new();
    for route in list_scope_routes(source_root, app_id)? {
        if !seen_scenes.insert(route.scene_id.clone()) {
            continue;
        }
        let target_file = assembly_target_for_key(
            app_root.as_path(),
            registry,
            route.assembly_key.as_str(),
        );
        let kind = route_kind_for_target(target_file.as_str());
        let title = scene_title_from_assembly(app_root.as_path(), registry, route.assembly_key.as_str());
        routes.push(CompiledSceneRoute {
            scene_id: route.scene_id.clone(),
            frame_id: None,
            target_file,
            kind,
            title,
            is_default: route.scene_id == "home",
            access_export: true,
        });
    }
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
        if node.id.key.contains("home@") {
            continue;
        }
        let Some(scene_id) = board_scene_id_for_node(app_root.as_path(), node) else {
            continue;
        };
        if !seen_scenes.insert(scene_id.clone()) {
            continue;
        }
        let title = node
            .payload_ref
            .as_ref()
            .and_then(|pref| load_block_artifact(app_root.as_path(), pref).ok())
            .flatten()
            .and_then(|artifact| {
                artifact
                    .get("payload")
                    .and_then(|payload| payload.get("summary"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
        routes.push(CompiledSceneRoute {
            scene_id,
            frame_id: None,
            target_file: assembly_target_for_node(app_root.as_path(), node),
            kind: "page".to_string(),
            title,
            is_default: false,
            // T2 / board page 不是 Access 顶栏舞台入口
            access_export: false,
        });
    }
    if routes.is_empty() {
        for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
            let scene = node.id.key.split('#').next_back().unwrap_or("home");
            routes.push(CompiledSceneRoute {
                scene_id: scene.to_string(),
                frame_id: None,
                target_file: assembly_target_for_node(app_root.as_path(), node),
                kind: if node.id.key.contains("home@") {
                    "scene".to_string()
                } else {
                    "page".to_string()
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
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
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
                    let mut normalized = normalize_page_instance_payload(payload);
                    if let Some(assembly) = normalized.as_object_mut() {
                        let _ = mei_lang_kernel::enrich_runtime_page_instance_projection_slots(
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

fn load_scene_examples_by_id(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
        if node.id.key.contains("home@") {
            continue;
        }
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(app_root, pref) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(json!({}));
        let scene_id = payload
            .get("scene")
            .and_then(|v| v.as_str())
            .or_else(|| node.id.key.split('#').next_back())
            .unwrap_or("")
            .trim()
            .to_string();
        if scene_id.is_empty() {
            continue;
        }
        let examples = payload.get("examples").cloned().unwrap_or(Value::Null);
        let is_empty = examples.is_null()
            || examples.as_object().is_some_and(|obj| obj.is_empty())
            || examples.as_array().is_some_and(|items| items.is_empty());
        if !is_empty {
            map.insert(scene_id, examples);
        }
    }
    map
}

fn load_scene_local_nav_by_target(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for node in registry.nodes_of_kind(GraphNodeKind::PageInstance) {
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
        map.insert(assembly_target_for_node(app_root, node), local_nav);
    }
    map
}

fn load_link_bindings(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> BTreeMap<String, Value> {
    let mut bindings = BTreeMap::new();
    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::Navigation && n.id.key.starts_with("overlay/"))
    {
        if let Some(pref) = node.payload_ref.as_ref() {
            if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
                bindings.insert(
                    node.id.key.clone(),
                    artifact.get("payload").cloned().unwrap_or(json!({})),
                );
            }
        }
    }
    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::Navigation)
    {
        if node.id.key.contains("link") {
            if let Some(pref) = node.payload_ref.as_ref() {
                if let Ok(Some(artifact)) = load_block_artifact(app_root, pref) {
                    bindings.insert(
                        node.id.key.clone(),
                        artifact.get("payload").cloned().unwrap_or(json!({})),
                    );
                }
            }
        }
    }
    bindings
}

fn load_overlay_defaults(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> BTreeMap<String, Value> {
    let mut defaults = BTreeMap::new();
    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::Navigation)
    {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(app_root, pref) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        if let Some(workspace) = payload.get("overlay_workspace").filter(|v| v.is_object()) {
            defaults.insert(node.id.key.clone(), workspace.clone());
        }
    }
    defaults
}

fn load_panels_for_assembly(
    app_root: &Path,
    app_id: &str,
    registry: &crate::mcg::registry::McgRegistry,
    assembly_payload: &Value,
    scene_id: &str,
) -> (Vec<mei_lang_kernel::UiNodeDecl>, BTreeMap<String, Value>) {
    let lower_ctx = PanelLowerContext {
        app_root,
        app_id,
        registry,
        scene_id,
        panel_constants: BTreeMap::new(),
        assembly_stack_order: None,
    };
    let mut panels = Vec::new();
    let mut panel_payloads = BTreeMap::new();
    let mut tier_assembly_counters: HashMap<String, u8> = HashMap::new();
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
        let contract_key = normalize_content_panel_key(&panel_key, assembly_payload);
        let Some(node) = find_content_panel_node(registry, contract_key.as_str(), scene_id) else {
            continue;
        };
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Ok(Some(artifact)) = load_block_artifact(app_root, pref) else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(json!({}));
        let assembly_order = payload
            .get("tier")
            .and_then(|v| v.as_str())
            .and_then(|raw| canonical_tier(raw).ok())
            .map(|tier| {
                let counter = tier_assembly_counters.entry(tier.to_string()).or_insert(0);
                let order = *counter;
                *counter += 1;
                order
            });
        let mut panel_ctx = lower_ctx.with_panel_constants(contract_key.as_str());
        if let Some(order) = assembly_order {
            panel_ctx = panel_ctx.with_assembly_stack_order(order);
        }
        if let Ok(panel) = lower_panel_payload(&payload, contract_key.as_str(), &panel_ctx) {
            panel_payloads.insert(panel.id.clone(), payload);
            panels.push(panel);
        }
    }

    if let Ok(inline_panels) = lower_v2_inline_panels_from_assembly(assembly_payload, &lower_ctx) {
        for panel in inline_panels {
            if panels.iter().any(|existing| existing.id == panel.id) {
                continue;
            }
            panel_payloads.insert(panel.id.clone(), json!({ "id": panel.id }));
            panels.push(panel);
        }
    }

    (panels, panel_payloads)
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

fn scene_contract_local_nav_is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Object(map) => map.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

fn extract_panel_ref_key(value: &Value) -> Option<String> {
    if let Some(args) = value.get("__args").and_then(|v| v.as_object()) {
        if let Some(arg0) = args.get("arg0").and_then(|v| v.as_str()) {
            return Some(arg0.to_string());
        }
    }
    value.get("id").and_then(|v| v.as_str()).map(str::to_string)
}

fn normalize_content_panel_key(panel_key: &str, assembly_payload: &Value) -> String {
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
    payload.get("assembly").and_then(|v| {
        if let Some(args) = v.get("__args").and_then(|o| o.as_object()) {
            args.get("arg0")
                .and_then(|a| a.as_str())
                .map(str::to_string)
        } else {
            v.as_str().map(str::to_string)
        }
    })
}

fn collect_component_assets_for_panels(
    source_root: &Path,
    panels: &[UiNodeDecl],
) -> Result<Vec<ComponentAsset>> {
    let asset_map = load_component_assets(source_root)?;
    let mut asset_keys = BTreeSet::new();
    for panel in panels {
        collect_asset_keys_from_panel(panel, &mut asset_keys);
    }
    Ok(asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect())
}

fn collect_asset_keys_from_panel(panel: &UiNodeDecl, asset_keys: &mut BTreeSet<String>) {
    collect_asset_keys_from_nodes(&panel.blocks, asset_keys);
    if let Some(head) = panel.head.as_ref() {
        collect_asset_keys_from_nodes(std::slice::from_ref(head.as_ref()), asset_keys);
    }
}

fn collect_asset_keys_from_nodes(nodes: &[UiTreeNode], asset_keys: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            UiTreeNode::Panel(panel) => collect_asset_keys_from_panel(panel, asset_keys),
            UiTreeNode::Block(block) => {
                asset_keys.insert(block.use_key.clone());
            }
            UiTreeNode::PanelRefEmbed(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembly_source_file_from_payload_normalizes_src_prefix() {
        let payload = json!({"source_file": "scene/home/t2/r-drilldown/s-enforcement-elements/c-enforcement-units-analytics/content.mei"});
        assert_eq!(
            assembly_source_file_from_payload(&payload),
            Some("src/scene/home/t2/r-drilldown/s-enforcement-elements/c-enforcement-units-analytics/content.mei".to_string())
        );
        let prefixed = json!({"source_file": "src/scene/home/assembly.mei"});
        assert_eq!(
            assembly_source_file_from_payload(&prefixed),
            Some("src/scene/home/assembly.mei".to_string())
        );
    }

    #[test]
    fn assembly_key_to_target_graph_native_fallback() {
        assert_eq!(
            assembly_key_to_target("data-demo/home/t2/r-drilldown/c-enforcement-units-analytics"),
            "src/data-demo/home/t2/r-drilldown/c-enforcement-units-analytics.mei"
        );
    }

    #[test]
    fn assembly_key_to_target_home() {
        assert_eq!(
            assembly_key_to_target("home@src/scene/home/assembly.mei"),
            "src/scene/home/assembly.mei"
        );
    }

    #[test]
    fn assembly_key_to_target_overlay_capsules() {
        assert_eq!(
            assembly_key_to_target("overlay/boards/supervision-warning#warnings_analytics_board"),
            "src/overlay/boards/supervision-warning.board.mei"
        );
        assert_eq!(
            assembly_key_to_target("overlay/boards/warning-detail.card#warning_detail_card_board"),
            "src/overlay/boards/warning-detail.card.mei"
        );
        assert_eq!(
            assembly_key_to_target("overlay/t2/supervision-warning#warnings_analytics_page"),
            "src/overlay/t2/supervision-warning.page.mei"
        );
        assert_eq!(
            assembly_key_to_target("overlay/t2/warning-detail.detail.page#warning_detail_page"),
            "src/overlay/t2/warning-detail.detail.page.mei"
        );
    }

    #[test]
    fn resolve_assembly_scene_id_maps_legacy_assembly_stem_via_registry_key() {
        let scene_id = canonical_scene_id("home@src/scene/home/assembly.mei");
        assert_eq!(scene_id, "home");
        assert_eq!(canonical_scene_id("assembly"), "assembly");
    }

    #[test]
    fn canonical_scene_id_normalizes_assembly_paths() {
        assert_eq!(canonical_scene_id("home"), "home");
        assert_eq!(canonical_scene_id("home/assembly.mei"), "home");
        assert_eq!(canonical_scene_id("src/scene/home/assembly.mei"), "home");
        assert_eq!(
            canonical_scene_id("home@src/scene/home/assembly.mei"),
            "home"
        );
        assert_eq!(canonical_scene_id("park_point_1_page"), "park_point_1_page");
    }
}
