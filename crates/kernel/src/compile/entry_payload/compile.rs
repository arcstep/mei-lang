use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::model::{
    CompiledSceneRoute, ComponentAsset, Diagnostic, FlowDecl, FrameDecl, PanelDecl, SceneContract,
    SceneDecl, Severity, ThemeDecl,
};

use super::super::decls::{
    FrameSetLayoutDecl, LegacyDatasetDecl, LegacyMetricPackDecl, WorldAddEntityDecl,
    WorldAddMetricDecl, WorldAddResourceDecl, WorldSetTopologyDecl,
};
use super::super::load_external::{
    load_flow_from_file, load_frame_from_file, load_world_from_file,
};
use super::super::materialize::{
    append_world_metrics_dataset_resource_with_id, materialize_legacy_datasets,
    materialize_metric_packs, materialize_world_metrics, WORLD_METRICS_RESOURCE_ID,
};
use super::super::mutations::{apply_frame_mutations, apply_world_mutations};
use super::super::panel_normalize::normalize_panel_slots;
use super::super::resources::load_resources;
use super::super::scene_binding::{
    decode_scene_decl, parse_flow_binding, parse_frame_binding, parse_world_binding,
    pick_only_frame, pick_only_world, SceneBinding,
};
use super::super::ui_data_policy::validate_scene_ui_data_bindings;
use super::clone_merge::{
    collect_ref_scene_files, deep_merge_json, normalize_flow_decl, normalize_frame_decl,
    normalize_world_decl, resolve_entity_slot, resolve_panel_slot, resolve_resource_slot,
};
use super::helpers::{
    all_world_resource_decls, collect_asset_keys_from_nodes, decode_world_dataset_decl,
    decode_world_metric_pack_decl, insert_resource_checked, partition_world_resources,
};
use super::CompiledScenePayload;
use crate::model::WorldMetricLedgerEntry;
use crate::config_refs::{
    decode_theme_ref_token, theme_decl_from_value, walk_value_for_config_refs, ConfigRefResolver,
};
use crate::typed_refs::{decode_binding_value, SceneRegistry};

fn push_deprecated_ref_binding_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    compat_source: Option<&str>,
    target_file: &str,
) {
    let Some(compat_source) = compat_source else {
        return;
    };
    let (code, message) = match compat_source {
        "world_file_ref" => (
            "deprecated_world_file_ref",
            "world_file_ref(...) is deprecated; migrate to world_ref(scene_file = ..., id = ...)",
        ),
        "frame_file_ref" => (
            "deprecated_frame_file_ref",
            "frame_file_ref(...) is deprecated; migrate to frame_ref(scene_file = ..., id = ...)",
        ),
        "flow_file_ref" => (
            "deprecated_flow_file_ref",
            "flow_file_ref(...) is deprecated; migrate to flow_ref(scene_file = ..., id = ...)",
        ),
        _ => return,
    };
    diagnostics.push(Diagnostic {
        severity: Severity::Warning,
        code: code.to_string(),
        message: message.to_string(),
        source_path: Some(target_file.to_string()),
    });
}

pub(super) fn compile_scene_payload(
    app_root: &Path,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    entry_decls: &Value,
    route_meta: Option<&CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
) -> Result<CompiledScenePayload> {
    let mut diagnostics = Vec::new();
    let config = crate::mei_config::load_mei_config_for_app(app_root, None);
    let app_entry_main = config.entry.main_rel();
    let resolver = ConfigRefResolver::new(&config);
    let mut scenes: BTreeMap<String, SceneDecl> = BTreeMap::new();
    let mut frames: BTreeMap<String, FrameDecl> = BTreeMap::new();
    let mut worlds: BTreeMap<String, crate::model::WorldDecl> = BTreeMap::new();
    let mut flows: BTreeMap<String, FlowDecl> = BTreeMap::new();
    let mut scene_decl_count = 0usize;
    let mut frame_decl_count = 0usize;
    let mut world_decl_count = 0usize;
    let mut world_topology_set_count = 0usize;
    let mut frame_layout_set_count = 0usize;
    let mut frame_default: Option<FrameDecl> = None;
    let mut world_default: Option<crate::model::WorldDecl> = None;
    let mut flow_default: Option<FlowDecl> = None;
    let mut pending_world_resources = Vec::new();
    let mut pending_world_entities = Vec::new();
    let mut pending_world_metrics = Vec::new();
    let mut pending_world_topology: Option<crate::model::WorldGridDecl> = None;
    let mut pending_frame_layout: Option<crate::model::LayoutDecl> = None;
    let mut themes: Vec<ThemeDecl> = Vec::new();
    let mut panels: Vec<PanelDecl> = Vec::new();
    let mut top_level_legacy_dataset_count = 0usize;
    let mut top_level_legacy_dataset_view_count = 0usize;
    let mut top_level_legacy_metric_pack_count = 0usize;
    let mut ref_scene_files = BTreeSet::new();
    let mut seen_world_decl = false;
    let mut first_scene_decl_index: Option<usize> = None;
    let mut first_world_decl_index: Option<usize> = None;

    if let Some(values) = entry_decls.as_array() {
        for (decl_index, value) in values.iter().enumerate() {
            collect_ref_scene_files(value, &mut ref_scene_files);
            if value.get("dataset").is_some() && value.get("schema_version").is_some() {
                top_level_legacy_dataset_count += 1;
                continue;
            }
            if value.get("metric_pack").is_some() && value.get("schema_version").is_some() {
                top_level_legacy_metric_pack_count += 1;
                continue;
            }
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                if let Some(component) = value.get("component") {
                    if component.get("block_kind").and_then(Value::as_str) == Some("panel_ref") {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "panel_ref_embed_removed".to_string(),
                            message: "panel_ref only references external panels in frame.panels; \
                                      block embed with `area` was removed"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    } else if matches!(
                        component.get("block_kind").and_then(Value::as_str),
                        Some("panel_capsule_ref") | Some("frame_ref")
                    ) {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "top_level_panel_ref_embed".to_string(),
                            message: "legacy panel embed block must appear inside frame.add_panel(...).blocks, not at scene top level"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                }
                continue;
            };
            match kind {
                "frame" => {
                    frame_decl_count += 1;
                    let frame_value = value.clone();
                    let mut frame_decl = serde_json::from_value::<FrameDecl>(frame_value.clone())?;
                    if frame_decl.base.is_some() || frame_value.get("base").is_some() {
                        match normalize_frame_decl(
                            app_root,
                            frame_decl,
                            &frame_value,
                            scene_registry,
                            &mut diagnostics,
                            target_file,
                        ) {
                            Some(normalized) => frame_decl = normalized,
                            None => continue,
                        }
                    }
                    if let Some(id) = frame_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        frames.insert(id.to_string(), frame_decl);
                    } else {
                        if frame_default.is_none() {
                            frame_default = Some(frame_decl);
                        }
                    }
                }
                "scene" => {
                    if first_scene_decl_index.is_none() {
                        first_scene_decl_index = Some(decl_index);
                    }
                    scene_decl_count += 1;
                    let mut scene_decl =
                        decode_scene_decl(app_root, value, target_file, Some(scene_registry))?;
                    normalize_shared_context(
                        &mut scene_decl.shared,
                        "scene.shared",
                        target_file,
                        &mut diagnostics,
                    );
                    normalize_scene_bindings(
                        &mut scene_decl.bindings,
                        &format!("scene `{}`.bindings", scene_decl.id),
                        target_file,
                        &mut diagnostics,
                    );
                    normalize_scene_examples(
                        &mut scene_decl.examples,
                        &format!("scene `{}`.examples", scene_decl.id),
                        target_file,
                        &mut diagnostics,
                    );
                    scenes.insert(scene_decl.id.clone(), scene_decl);
                }
                "world" => {
                    if first_world_decl_index.is_none() {
                        first_world_decl_index = Some(decl_index);
                    }
                    world_decl_count += 1;
                    let world_value = value.clone();
                    let mut world_decl =
                        serde_json::from_value::<crate::model::WorldDecl>(world_value.clone())?;
                    if world_decl.base.is_some() || world_value.get("base").is_some() {
                        match normalize_world_decl(
                            app_root,
                            world_decl,
                            &world_value,
                            scene_registry,
                            &mut diagnostics,
                            target_file,
                        ) {
                            Some(normalized) => world_decl = normalized,
                            None => continue,
                        }
                    }
                    if let Some(id) = world_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        worlds.insert(id.to_string(), world_decl);
                    } else {
                        if world_default.is_none() {
                            world_default = Some(world_decl);
                        }
                    }
                    seen_world_decl = true;
                }
                "world_add_resource" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldAddResourceDecl>(value.clone())?;
                    if decl.kind == "world_add_resource" {
                        let resource_value = serde_json::to_value(&decl.resource)?;
                        if let Some(resource) = resolve_resource_slot(
                            app_root,
                            &resource_value,
                            scene_registry,
                            &mut diagnostics,
                            target_file,
                        ) {
                            pending_world_resources.push(resource);
                        }
                    }
                }
                "world_add_entity" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldAddEntityDecl>(value.clone())?;
                    if decl.kind == "world_add_entity" {
                        let entity_value = serde_json::to_value(&decl.entity)?;
                        if let Some(entity) = resolve_entity_slot(
                            app_root,
                            &entity_value,
                            scene_registry,
                            &mut diagnostics,
                            target_file,
                        ) {
                            pending_world_entities.push(entity);
                        }
                    }
                }
                "world_add_metric" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldAddMetricDecl>(value.clone())?;
                    if decl.kind == "world_add_metric" {
                        pending_world_metrics.push(decl.metric);
                    }
                }
                "world_set_topology" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldSetTopologyDecl>(value.clone())?;
                    if decl.kind == "world_set_topology" {
                        world_topology_set_count += 1;
                        if pending_world_topology.is_none() {
                            pending_world_topology = Some(decl.topology);
                        }
                    }
                }
                "frame_set_layout" => {
                    let decl = serde_json::from_value::<FrameSetLayoutDecl>(value.clone())?;
                    if decl.kind == "frame_set_layout" {
                        frame_layout_set_count += 1;
                        if pending_frame_layout.is_none() {
                            pending_frame_layout = Some(serde_json::from_value::<
                                crate::model::LayoutDecl,
                            >(decl.layout)?);
                        }
                    }
                }
                "flow" => {
                    let flow_value = value.clone();
                    let mut flow_decl = serde_json::from_value::<FlowDecl>(flow_value.clone())?;
                    if flow_decl.base.is_some() || flow_value.get("base").is_some() {
                        match normalize_flow_decl(
                            app_root,
                            flow_decl,
                            &flow_value,
                            scene_registry,
                            &mut diagnostics,
                            target_file,
                        ) {
                            Some(normalized) => flow_decl = normalized,
                            None => continue,
                        }
                    }
                    if let Some(id) = flow_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        flows.insert(id.to_string(), flow_decl);
                    } else {
                        flow_default = Some(flow_decl);
                    }
                }
                "theme" => {
                    let theme_value =
                        resolver.resolve_config_refs_in_value(value, target_file, &mut diagnostics);
                    let mut theme_decl = serde_json::from_value::<ThemeDecl>(theme_value)?;
                    normalize_shared_context(
                        &mut theme_decl.shared,
                        &format!("theme `{}`.shared", theme_decl.id),
                        target_file,
                        &mut diagnostics,
                    );
                    themes.push(theme_decl);
                }
                "panel" => {
                    if let Some(panel) = resolve_panel_slot(
                        app_root,
                        value,
                        scene_registry,
                        &mut diagnostics,
                        target_file,
                    ) {
                        panels.push(panel);
                    }
                }
                "dataset_view" => top_level_legacy_dataset_view_count += 1,
                "app" | "app_scene_ref" => {}
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(target_file.to_string()),
                }),
            }
        }
    }
    if let (Some(scene_idx), Some(world_idx)) = (first_scene_decl_index, first_world_decl_index) {
        if world_idx < scene_idx {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "world_before_scene_decl".to_string(),
                message: "`world(...)` must appear after `scene(...)` in the same file when both are declared (_declare order)"
                    .to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
    let has_dataset_library_content = top_level_legacy_dataset_count > 0
        || top_level_legacy_metric_pack_count > 0
        || top_level_legacy_dataset_view_count > 0;
    let had_pending_topology = pending_world_topology.is_some();
    let had_pending_frame_layout = pending_frame_layout.is_some();
    let has_authoring_surface = scene_decl_count > 0
        || frame_decl_count > 0
        || world_decl_count > 0
        || !flows.is_empty()
        || flow_default.is_some()
        || !panels.is_empty()
        || !themes.is_empty()
        || world_topology_set_count > 0
        || frame_layout_set_count > 0
        || !pending_world_resources.is_empty()
        || !pending_world_entities.is_empty()
        || !pending_world_metrics.is_empty()
        || had_pending_topology
        || had_pending_frame_layout;
    let dataset_library_only = has_dataset_library_content
        && !has_authoring_surface
        && target_file != app_entry_main;

    if top_level_legacy_dataset_count > 0
        || top_level_legacy_dataset_view_count > 0
        || top_level_legacy_metric_pack_count > 0
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "forbidden_top_level_dataset_decls".to_string(),
            message: "world-only mode forbids top-level dataset()/dataset_view()/metric_pack(); use world.add_dataset()/world.add_dataset_view()/world.add_metric_pack() or world resources list".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    if world_topology_set_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_world_topologies".to_string(),
            message: format!(
                "file `{target_file}` declares {world_topology_set_count} world.set_topology(...) blocks, expected at most one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if frame_layout_set_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_frame_layouts".to_string(),
            message: format!(
                "file `{target_file}` declares {frame_layout_set_count} frame.set_layout(...) blocks, expected at most one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    apply_world_mutations(
        &mut worlds,
        &mut world_default,
        &pending_world_resources,
        &pending_world_entities,
        &pending_world_metrics,
        pending_world_topology,
        &mut diagnostics,
        target_file,
        world_decl_count,
    );
    apply_frame_mutations(
        &mut frames,
        &mut frame_default,
        pending_frame_layout,
        &mut diagnostics,
        target_file,
        frame_decl_count,
    );
    merge_frame_panel_slots(
        app_root,
        &frames,
        frame_default.as_ref(),
        &mut panels,
        scene_registry,
        &mut diagnostics,
        target_file,
    );
    normalize_panel_slots(&mut panels, &mut diagnostics, target_file);
    if scene_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_scenes".to_string(),
            message: format!(
                "file `{target_file}` declares {scene_decl_count} scene(...) blocks, expected exactly one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if world_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_worlds".to_string(),
            message: format!(
                "file `{target_file}` declares {world_decl_count} world(...) blocks, expected exactly one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if frame_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_frames".to_string(),
            message: format!(
                "file `{target_file}` declares {frame_decl_count} frame(...) blocks, expected exactly one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }

    let mut asset_keys = BTreeSet::new();
    for panel in &panels {
        collect_asset_keys_from_nodes(&panel.blocks, &mut asset_keys);
    }
    let component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect::<Vec<ComponentAsset>>();

    let selected_scene = route_meta
        .and_then(|meta| scenes.get(meta.scene_id.as_str()).cloned())
        .or_else(|| {
            if scenes.len() == 1 {
                scenes.values().next().cloned()
            } else {
                None
            }
        });
    if let Some(scene_decl) = selected_scene.as_ref() {
        if let Some(theme_token) = scene_decl.theme.as_deref() {
            if let Some(theme_id) = decode_theme_ref_token(theme_token) {
                if !themes.iter().any(|item| item.id == theme_id) {
                    if let Some(theme_value) = resolver.resolve_theme_token(theme_token) {
                        let theme_value = resolver.resolve_config_refs_in_value(
                            &theme_value,
                            target_file,
                            &mut diagnostics,
                        );
                        match theme_decl_from_value(theme_id.as_str(), theme_value) {
                            Ok(theme_decl) => themes.push(theme_decl),
                            Err(message) => diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: "invalid_config_ref".to_string(),
                                message,
                                source_path: Some(target_file.to_string()),
                            }),
                        }
                    }
                }
            }
        }
    }
    let requires_scene_contract =
        (route_meta.is_some() || target_file != app_entry_main) && !dataset_library_only;
    if requires_scene_contract && selected_scene.is_none() {
        let is_legacy_fragment = frame_decl_count > 0
            || !panels.is_empty()
            || world_decl_count > 0
            || frame_default.is_some()
            || world_default.is_some();
        if is_legacy_fragment {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "public_fragment_file_deprecated".to_string(),
                message: "legacy frame/world/panel fragment without scene(...); migrate to a minimal scene capsule with scene(...) and typed refs (world_ref/frame_ref)".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "scene file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let frame = if let Some(frame_id) = route_meta
        .and_then(|meta| meta.frame_id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
    {
        let matched = frames.get(frame_id.as_str()).cloned();
        if matched.is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_bound_frame".to_string(),
                message: format!("declared frame `{frame_id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
        }
        matched
    } else if let Some(scene_decl) = selected_scene.as_ref() {
        let binding = scene_decl
            .frame
            .as_ref()
            .map(|value| parse_frame_binding(value, Some(scene_registry)));
        match binding {
            Some(Ok(SceneBinding::LocalId(frame_id))) => {
                let matched = frames.get(frame_id.as_str()).cloned();
                if matched.is_none() {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "missing_bound_frame".to_string(),
                        message: format!("declared frame `{frame_id}` was not found"),
                        source_path: Some(target_file.to_string()),
                    });
                }
                matched
            }
            Some(Ok(SceneBinding::FileRef {
                path,
                id,
                compat_source,
            })) => {
                push_deprecated_ref_binding_diagnostic(
                    &mut diagnostics,
                    compat_source.as_deref(),
                    target_file,
                );
                match load_frame_from_file(app_root, path.as_str(), id.as_deref()) {
                    Ok(frame_decl) => {
                        if frame_decl.base.is_some() {
                            let frame_value = serde_json::to_value(&frame_decl)?;
                            normalize_frame_decl(
                                app_root,
                                frame_decl,
                                &frame_value,
                                scene_registry,
                                &mut diagnostics,
                                target_file,
                            )
                        } else {
                            Some(frame_decl)
                        }
                    }
                    Err(error) => {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "load_frame_ref_failed".to_string(),
                            message: error.to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        None
                    }
                }
            }
            Some(Ok(SceneBinding::Absent)) => pick_only_frame(&frames, frame_default.clone()),
            Some(Err(message)) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_scene_frame_binding".to_string(),
                    message: message.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
            None => pick_only_frame(&frames, frame_default.clone()),
        }
    } else {
        pick_only_frame(&frames, frame_default.clone())
    };
    if selected_scene.is_some() && frame.is_none() && frame_decl_count == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_frame".to_string(),
            message: "scene route requires a frame(...) declaration or frame_ref(...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let world = if let Some(scene_decl) = selected_scene.as_ref() {
        let binding = scene_decl
            .world
            .as_ref()
            .map(|value| parse_world_binding(value, Some(scene_registry)));
        match binding {
            Some(Ok(SceneBinding::LocalId(world_id))) => {
                let matched = worlds.get(world_id.as_str()).cloned();
                if matched.is_none() {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "missing_bound_world".to_string(),
                        message: format!("declared world `{world_id}` was not found"),
                        source_path: Some(target_file.to_string()),
                    });
                }
                matched
            }
            Some(Ok(SceneBinding::FileRef {
                path,
                id,
                compat_source,
            })) => {
                push_deprecated_ref_binding_diagnostic(
                    &mut diagnostics,
                    compat_source.as_deref(),
                    target_file,
                );
                match load_world_from_file(app_root, path.as_str(), id.as_deref()) {
                    Ok(world_decl) => {
                        let world_value = serde_json::to_value(&world_decl)?;
                        normalize_world_decl(
                            app_root,
                            world_decl,
                            &world_value,
                            scene_registry,
                            &mut diagnostics,
                            target_file,
                        )
                    }
                    Err(error) => {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "load_world_ref_failed".to_string(),
                            message: error.to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        None
                    }
                }
            }
            Some(Ok(SceneBinding::Absent)) => pick_only_world(&worlds, world_default.clone()),
            Some(Err(message)) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_scene_world_binding".to_string(),
                    message: message.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
            None => pick_only_world(&worlds, world_default.clone()),
        }
    } else {
        pick_only_world(&worlds, world_default.clone())
    };
    if selected_scene.is_some() && world.is_none() && world_decl_count == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_world".to_string(),
            message: "scene entry requires a world(...) declaration or world_ref(...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }
    if let (Some(scene_decl), Some(world_decl)) = (selected_scene.as_ref(), world.as_ref()) {
        validate_scene_binding_contract(scene_decl, world_decl, target_file, &mut diagnostics);
    }

    let flow = selected_scene
        .as_ref()
        .and_then(|scene| scene.flow.as_ref())
        .and_then(|value| {
            resolve_flow_binding(
                value,
                &flows,
                app_root,
                Some(scene_registry),
                &mut diagnostics,
                target_file,
            )
        })
        .or_else(|| {
            flow_default.clone().or_else(|| {
                (flows.len() == 1)
                    .then(|| flows.values().next().cloned())
                    .flatten()
            })
        });

    let mut resources = Vec::new();
    let mut world_dataset_decls: Vec<LegacyDatasetDecl> = Vec::new();
    let mut world_metric_pack_decls: Vec<LegacyMetricPackDecl> = Vec::new();
    if let Some(world_decl) = world.as_ref() {
        let (normal_resources, dataset_resources) =
            partition_world_resources(&all_world_resource_decls(world_decl));
        resources = load_resources(app_root, &normal_resources, target_file, &mut diagnostics)?;
        for resource in dataset_resources {
            if resource.id == "__source_path__" || resource.id.ends_with(".mei") {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "forbidden_legacy_resource_id".to_string(),
                    message: format!(
                        "resource id `{}` is forbidden in world-only mode; use a stable explicit id",
                        resource.id
                    ),
                    source_path: Some(target_file.to_string()),
                });
                continue;
            }
            match resource.kind.as_str() {
                "dataset" | "dataset_view" => match decode_world_dataset_decl(resource.clone()) {
                    Ok(decl) => world_dataset_decls.push(decl),
                    Err(message) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_world_dataset_decl_failed".to_string(),
                        message,
                        source_path: Some(target_file.to_string()),
                    }),
                },
                "metric_pack" => match decode_world_metric_pack_decl(resource.clone()) {
                    Ok(decl) => world_metric_pack_decls.push(decl),
                    Err(message) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_world_metric_pack_decl_failed".to_string(),
                        message,
                        source_path: Some(target_file.to_string()),
                    }),
                },
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "unsupported_world_resource_kind".to_string(),
                    message: format!(
                        "resource `{}` has unsupported kind `{}` in world-only mode",
                        resource.id, resource.kind
                    ),
                    source_path: Some(target_file.to_string()),
                }),
            }
        }
    }
    validate_config_refs(
        app_root,
        entry_decls,
        scenes.values().next(),
        target_file,
        &mut diagnostics,
    );
    let has_config_ref_errors = diagnostics.iter().any(|diag| {
        diag.severity == Severity::Error
            && matches!(
                diag.code.as_str(),
                "missing_config_ref" | "invalid_config_ref"
            )
    });
    if !world_dataset_decls.is_empty() && !has_config_ref_errors {
        let derived = materialize_legacy_datasets(app_root, &resources, &world_dataset_decls)?;
        for resource in derived {
            insert_resource_checked(&mut resources, resource, target_file, &mut diagnostics);
        }
    }
    if !world_metric_pack_decls.is_empty() {
        let derived = materialize_metric_packs(&resources, &world_metric_pack_decls)?;
        for resource in derived {
            insert_resource_checked(&mut resources, resource, target_file, &mut diagnostics);
        }
    }

    let host_local_ids = super::import_scope::host_local_resource_ids(&resources);
    let mut imported_runtime = super::import_scope::finalize_private_import_world(
        app_root,
        &panels,
        &host_local_ids,
        target_file,
        &mut diagnostics,
    );
    resources.append(&mut imported_runtime);

    if let Some(world_decl) = world.as_ref() {
        if !world_decl.metrics.is_empty() {
            // 当前 scene 自身的 world(metrics=...) 使用宿主 `__world_metrics__`；
            // imported capsule 的 namespaced owner 由 finalize_private_import_world 另行并入。
            let owner_resource_id = WORLD_METRICS_RESOURCE_ID.to_string();
            if let Ok(world_metrics) = materialize_world_metrics(&resources, &world_decl.metrics) {
                let ledger = world_metrics
                    .into_iter()
                    .enumerate()
                    .map(|(idx, (metric_id, metric))| {
                        (
                            metric_id.clone(),
                            WorldMetricLedgerEntry {
                                id: metric_id,
                                owner_resource_id: owner_resource_id.clone(),
                                order: idx + 1,
                                metric,
                            },
                        )
                    })
                    .collect::<std::collections::BTreeMap<_, _>>();
                append_world_metrics_dataset_resource_with_id(
                    &mut resources,
                    &ledger,
                    &world_decl.metrics,
                    &owner_resource_id,
                );
            }
        }
    }

    let mut scene_contract = selected_scene.map(|scene_decl| {
        let shared = deep_merge_json(
            &selected_custom_theme_shared(&scene_decl, &themes),
            &scene_decl.shared,
        );
        SceneContract {
            scene: scene_decl,
            themes,
            shared,
            world,
            flow,
            frame,
            panels,
        }
    });
    if let Some(ref mut contract) = scene_contract {
        resolve_scene_contract_config_refs(contract, &resolver, target_file, &mut diagnostics);
        super::super::projection_assembly::lower_projection_assembly_in_panels(
            &mut contract.panels,
            &resources,
            target_file,
            &mut diagnostics,
        );
    }
    if let Some(ref contract) = scene_contract {
        validate_scene_ui_data_bindings(
            contract,
            &resources,
            app_root,
            target_file,
            &mut diagnostics,
        );
    }
    if let Some(contract) = scene_contract.as_ref() {
        let config = crate::mei_config::load_mei_config_for_app(app_root, None);
        let resolver = ConfigRefResolver::new(&config);
        if let Some(theme) = contract.scene.theme.as_deref() {
            if decode_theme_ref_token(theme).is_some() {
                resolver.validate_theme_token(theme, target_file, &mut diagnostics);
            }
        }
    }

    Ok(CompiledScenePayload {
        scene_contract,
        resources,
        component_assets,
        diagnostics,
    })
}

fn resolve_flow_binding(
    value: &Value,
    flows: &BTreeMap<String, FlowDecl>,
    app_root: &Path,
    scene_registry: Option<&SceneRegistry>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FlowDecl> {
    if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
        if let Some(flow) = flows.get(id) {
            return Some(flow.clone());
        }
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_bound_flow".to_string(),
            message: format!("declared flow `{id}` was not found"),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    match parse_flow_binding(value, scene_registry) {
        Ok(SceneBinding::LocalId(id)) => {
            if let Some(flow) = flows.get(id.as_str()) {
                return Some(flow.clone());
            }
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_bound_flow".to_string(),
                message: format!("declared flow `{id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
            None
        }
        Ok(SceneBinding::FileRef {
            path,
            id,
            compat_source,
        }) => {
            push_deprecated_ref_binding_diagnostic(
                diagnostics,
                compat_source.as_deref(),
                target_file,
            );
            match load_flow_from_file(app_root, path.as_str(), id.as_deref()) {
                Ok(flow_decl) => {
                    let Some(registry) = scene_registry else {
                        return Some(flow_decl);
                    };
                    let flow_value = serde_json::to_value(&flow_decl).ok()?;
                    normalize_flow_decl(
                        app_root,
                        flow_decl,
                        &flow_value,
                        registry,
                        diagnostics,
                        target_file,
                    )
                }
                Err(error) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "load_flow_ref_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    });
                    None
                }
            }
        }
        Ok(SceneBinding::Absent) => None,
        Err(message) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_flow_binding".to_string(),
                message: message.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

fn merge_frame_panel_slots(
    app_root: &Path,
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<&FrameDecl>,
    panels: &mut Vec<PanelDecl>,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) {
    let mut sources: Vec<&FrameDecl> = frames.values().collect();
    if let Some(frame) = frame_default {
        sources.push(frame);
    }
    for frame in sources {
        for slot in &frame.panels {
            if let Some(panel) =
                resolve_panel_slot(app_root, slot, scene_registry, diagnostics, target_file)
            {
                upsert_panel(panels, panel);
            }
        }
    }
}

fn upsert_panel(panels: &mut Vec<PanelDecl>, panel: PanelDecl) {
    if let Some(existing) = panels.iter_mut().find(|item| item.id == panel.id) {
        *existing = panel;
        return;
    }
    panels.push(panel);
}

fn selected_custom_theme_shared(scene: &SceneDecl, themes: &[ThemeDecl]) -> Value {
    let theme_id = scene
        .theme
        .as_deref()
        .and_then(decode_theme_ref_token)
        .or_else(|| scene.theme.clone())
        .or_else(|| scene.profile.clone())
        .unwrap_or_else(|| "page".to_string());
    themes
        .iter()
        .find(|item| item.id == theme_id)
        .or_else(|| themes.first())
        .map(|theme| theme.shared.clone())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn resolve_scene_contract_config_refs(
    contract: &mut SceneContract,
    resolver: &ConfigRefResolver<'_>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let raw = match serde_json::to_value(&*contract) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_config_ref".to_string(),
                message: format!("failed to serialize scene contract for config ref resolution: {error}"),
                source_path: Some(target_file.to_string()),
            });
            return;
        }
    };
    let resolved = resolver.resolve_config_refs_in_value(&raw, target_file, diagnostics);
    match serde_json::from_value::<SceneContract>(resolved) {
        Ok(next) => *contract = next,
        Err(error) => diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_config_ref".to_string(),
            message: format!("failed to decode resolved scene contract: {error}"),
            source_path: Some(target_file.to_string()),
        }),
    }
}

fn normalize_shared_context(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = serde_json::json!({});
        return;
    }
    if !value.is_object() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_shared_context_value".to_string(),
            message: format!("{context} 必须是对象（dict），不能是数组、标量或 null"),
            source_path: Some(target_file.to_string()),
        });
        *value = serde_json::json!({});
        return;
    }
    let mut invalid_paths = Vec::new();
    collect_invalid_shared_paths(value, "$", &mut invalid_paths);
    if invalid_paths.is_empty() {
        return;
    }
    for path in invalid_paths {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_shared_context_value".to_string(),
            message: format!(
                "{context} 只允许字面量 JSON 值；`{path}` 处检测到 ref 或分析表达式，请改为显式常量"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    *value = strip_invalid_shared_entries(value);
}

fn collect_invalid_shared_paths(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").is_some()
                || (map.get("__kind").and_then(Value::as_str) == Some("analysis_expr"))
            {
                out.push(path.to_string());
                return;
            }
            for (key, child) in map {
                collect_invalid_shared_paths(child, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                collect_invalid_shared_paths(child, &format!("{path}[{idx}]"), out);
            }
        }
        _ => {}
    }
}

fn strip_invalid_shared_entries(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").is_some()
                || (map.get("__kind").and_then(Value::as_str) == Some("analysis_expr"))
            {
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                out.insert(key.clone(), strip_invalid_shared_entries(child));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(strip_invalid_shared_entries).collect())
        }
        _ => value.clone(),
    }
}

fn normalize_scene_bindings(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = serde_json::json!({});
        return;
    }
    let Some(map) = value.as_object() else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_scene_bindings_value".to_string(),
            message: format!("{context} 必须是对象（dict），键为 slot/entry，值为 ref 或内联对象"),
            source_path: Some(target_file.to_string()),
        });
        *value = serde_json::json!({});
        return;
    };
    let mut out = serde_json::Map::new();
    for (key, binding) in map {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_binding_key".to_string(),
                message: format!("{context} 含空 binding key"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        }
        if decode_binding_value(binding).is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_binding_value".to_string(),
                message: format!(
                    "{context}.{normalized_key} 必须是 *_ref(...)、非空字符串或内联对象"
                ),
                source_path: Some(target_file.to_string()),
            });
            continue;
        }
        out.insert(normalized_key.to_string(), binding.clone());
    }
    *value = Value::Object(out);
}

fn normalize_scene_examples(
    value: &mut Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        *value = Value::Array(Vec::new());
        return;
    }
    let mut items: Vec<Value> = Vec::new();
    if let Some(array) = value.as_array() {
        items = array.clone();
    } else if let Some(map) = value.as_object() {
        if map.contains_key("bindings") || map.contains_key("id") || map.contains_key("title") {
            items.push(Value::Object(map.clone()));
        } else {
            for (id, entry) in map {
                let Some(entry_map) = entry.as_object() else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "invalid_scene_example_value".to_string(),
                        message: format!("{context}.{id} 必须是对象"),
                        source_path: Some(target_file.to_string()),
                    });
                    continue;
                };
                let mut out = entry_map.clone();
                out.entry("id".to_string())
                    .or_insert_with(|| Value::String(id.to_string()));
                items.push(Value::Object(out));
            }
        }
    } else {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_scene_examples_value".to_string(),
            message: format!("{context} 必须是数组或对象"),
            source_path: Some(target_file.to_string()),
        });
        *value = Value::Array(Vec::new());
        return;
    }
    let mut normalized = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let Some(obj) = item.as_object() else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_example_value".to_string(),
                message: format!("{context}[{index}] 必须是对象"),
                source_path: Some(target_file.to_string()),
            });
            continue;
        };
        let mut example = Value::Object(obj.clone());
        let bindings_value = example
            .get("bindings")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut normalized_bindings = bindings_value;
        normalize_scene_bindings(
            &mut normalized_bindings,
            &format!("{context}[{index}].bindings"),
            target_file,
            diagnostics,
        );
        if let Some(example_map) = example.as_object_mut() {
            example_map.insert("bindings".to_string(), normalized_bindings);
        }
        normalized.push(example);
    }
    *value = Value::Array(normalized);
}

fn validate_scene_binding_contract(
    scene: &SceneDecl,
    world: &crate::model::WorldDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let provided_keys = collect_scene_binding_keys(scene);
    for resource in all_world_resource_decls(world) {
        let binding = resource
            .dataset
            .as_ref()
            .and_then(binding_meta_from_value)
            .or_else(|| binding_meta_from_object_field(resource.dataset.as_ref(), "binding"));
        if let Some(meta) = binding {
            validate_binding_meta(
                &resource.id,
                "resource",
                &meta,
                &provided_keys,
                target_file,
                diagnostics,
            );
        }
        if let Some(metrics) = resource.metrics.as_ref() {
            for (metric_id, metric_value) in metrics {
                if let Some(meta) = binding_meta_from_value(metric_value) {
                    validate_binding_meta(
                        metric_id,
                        "metric",
                        &meta,
                        &provided_keys,
                        target_file,
                        diagnostics,
                    );
                }
            }
        }
    }
    for metric_value in &world.metrics {
        let metric_id = metric_value
            .get("key")
            .or_else(|| metric_value.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or("<unnamed_metric>");
        if let Some(meta) = binding_meta_from_value(metric_value) {
            validate_binding_meta(
                metric_id,
                "metric",
                &meta,
                &provided_keys,
                target_file,
                diagnostics,
            );
        }
    }
}

fn collect_scene_binding_keys(scene: &SceneDecl) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(map) = scene.bindings.as_object() {
        for key in map.keys() {
            let normalized = key.trim();
            if !normalized.is_empty() {
                keys.insert(normalized.to_string());
            }
        }
    }
    if let Some(examples) = scene.examples.as_array() {
        for example in examples {
            let Some(bindings) = example.get("bindings").and_then(Value::as_object) else {
                continue;
            };
            for key in bindings.keys() {
                let normalized = key.trim();
                if !normalized.is_empty() {
                    keys.insert(normalized.to_string());
                }
            }
        }
    }
    keys
}

fn binding_meta_from_object_field(value: Option<&Value>, field: &str) -> Option<Value> {
    value
        .and_then(Value::as_object)
        .and_then(|map| map.get(field))
        .cloned()
        .filter(|value| value.is_object())
}

fn validate_config_refs(
    app_root: &Path,
    entry_decls: &Value,
    scene: Option<&SceneDecl>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let config = crate::mei_config::load_mei_config_for_app(app_root, None);
    let resolver = ConfigRefResolver::new(&config);
    walk_value_for_config_refs(entry_decls, target_file, &resolver, diagnostics);
    if let Some(scene) = scene {
        if let Some(theme) = scene.theme.as_deref() {
            if decode_theme_ref_token(theme).is_some() {
                resolver.validate_theme_token(theme, target_file, diagnostics);
            }
        }
    }
}

fn binding_meta_from_value(value: &Value) -> Option<Value> {
    value
        .as_object()
        .and_then(|map| map.get("binding"))
        .cloned()
        .filter(|value| value.is_object())
}

fn validate_binding_meta(
    binding_key: &str,
    subject: &str,
    meta: &Value,
    provided_keys: &BTreeSet<String>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(meta_map) = meta.as_object() else {
        return;
    };
    let enabled = meta_map
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    if let Some(replace) = meta_map.get("replace").and_then(Value::as_str) {
        if replace != "source" && replace != "full" {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_binding_replace_mode".to_string(),
                message: format!(
                    "{subject} `{binding_key}` 的 binding.replace 仅支持 `source` 或 `full`"
                ),
                source_path: Some(target_file.to_string()),
            });
        }
    }
    if let Some(accept) = meta_map.get("accept") {
        let Some(accept_map) = accept.as_object() else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_binding_accept".to_string(),
                message: format!("{subject} `{binding_key}` 的 binding.accept 必须是对象"),
                source_path: Some(target_file.to_string()),
            });
            return;
        };
        for key in accept_map.keys() {
            if key != "shape" && key != "schema" && key != "kind" {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_binding_accept_key".to_string(),
                    message: format!(
                        "{subject} `{binding_key}` 的 binding.accept 仅支持 `shape` / `schema` / `kind`"
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
        }
    }
    if meta_map
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !provided_keys.contains(binding_key)
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_required_scene_binding".to_string(),
            message: format!(
                "{subject} `{binding_key}` 声明了 required binding，但 scene.bindings / scene.examples 未提供对应条目"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
}
