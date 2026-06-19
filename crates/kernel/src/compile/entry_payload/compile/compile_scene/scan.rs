use std::path::Path;

use serde_json::Value;

use crate::compile::decls::{
    FrameSetLayoutDecl, WorldAddEntityDecl, WorldAddMetricDecl, WorldAddResourceDecl,
    WorldSetTopologyDecl,
};
use crate::compile::entry_payload::clone_merge::{
    collect_ref_scene_files, normalize_flow_decl, normalize_frame_decl, normalize_world_decl,
    resolve_entity_slot, resolve_panel_slot, resolve_resource_slot,
};
use crate::compile::scene_binding::decode_scene_decl;
use crate::model::{
    Diagnostic, FlowDecl, FrameDecl, FrameExportDecl, SceneExportDecl, Severity, ThemeDecl,
};
use crate::typed_refs::SceneRegistry;

use super::super::scene_binding::{
    normalize_scene_bindings, normalize_scene_examples, normalize_scene_params,
};
use super::super::scene_contract::normalize_shared_context;
use super::state::CompileSceneCtx;

pub(super) fn scan_declarations(
    ctx: &mut CompileSceneCtx,
    app_root: &Path,
    target_file: &str,
    entry_decls: &Value,
    scene_registry: &SceneRegistry,
) -> anyhow::Result<()> {
    if let Some(values) = entry_decls.as_array() {
        for (decl_index, value) in values.iter().enumerate() {
            collect_ref_scene_files(value, &mut ctx.ref_scene_files);
            if value.get("dataset").is_some() && value.get("schema_version").is_some() {
                ctx.top_level_legacy_dataset_count += 1;
                continue;
            }
            if value.get("metric_pack").is_some() && value.get("schema_version").is_some() {
                ctx.top_level_legacy_metric_pack_count += 1;
                continue;
            }
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                if let Some(component) = value.get("component") {
                    if component.get("block_kind").and_then(Value::as_str) == Some("panel_ref") {
                        ctx.diagnostics.push(Diagnostic {
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
                        ctx.diagnostics.push(Diagnostic {
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
                    ctx.frame_decl_count += 1;
                    let frame_value = value.clone();
                    let mut frame_decl = serde_json::from_value::<FrameDecl>(frame_value.clone())?;
                    if frame_decl.base.is_some() || frame_value.get("base").is_some() {
                        match normalize_frame_decl(
                            app_root,
                            frame_decl,
                            &frame_value,
                            scene_registry,
                            &mut ctx.diagnostics,
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
                        crate::theme_tokens::validate_frame_token_refs(
                            &frame_decl,
                            target_file,
                            &mut ctx.diagnostics,
                        );
                        ctx.frames.insert(id.to_string(), frame_decl);
                    } else {
                        if ctx.frame_default.is_none() {
                            crate::theme_tokens::validate_frame_token_refs(
                                &frame_decl,
                                target_file,
                                &mut ctx.diagnostics,
                            );
                            ctx.frame_default = Some(frame_decl);
                        }
                    }
                }
                "frame_export" => {
                    let export = serde_json::from_value::<FrameExportDecl>(value.clone())?;
                    let mut frame_value = export.frame;
                    if frame_value
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .map(|id| id.is_empty())
                        .unwrap_or(true)
                    {
                        frame_value["id"] = Value::String(export.id.clone());
                    }
                    let mut frame_decl =
                        serde_json::from_value::<FrameDecl>(frame_value.clone())?;
                    if frame_decl.base.is_some() || frame_value.get("base").is_some() {
                        match normalize_frame_decl(
                            app_root,
                            frame_decl,
                            &frame_value,
                            scene_registry,
                            &mut ctx.diagnostics,
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
                        crate::theme_tokens::validate_frame_token_refs(
                            &frame_decl,
                            target_file,
                            &mut ctx.diagnostics,
                        );
                        ctx.frames.insert(id.to_string(), frame_decl);
                    }
                }
                "scene" => {
                    if ctx.first_scene_decl_index.is_none() {
                        ctx.first_scene_decl_index = Some(decl_index);
                    }
                    ctx.scene_decl_count += 1;
                    let mut scene_decl =
                        decode_scene_decl(app_root, value, target_file, Some(scene_registry))?;
                    normalize_shared_context(
                        &mut scene_decl.shared,
                        "scene.shared",
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    normalize_scene_params(
                        &mut scene_decl.params,
                        &format!("scene `{}`.params", scene_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    normalize_scene_bindings(
                        &mut scene_decl.bindings,
                        &format!("scene `{}`.bindings", scene_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    normalize_scene_examples(
                        &mut scene_decl.examples,
                        &format!("scene `{}`.examples", scene_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    if ctx
                        .scenes
                        .insert(scene_decl.id.clone(), scene_decl.clone())
                        .is_some()
                    {
                        ctx.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "duplicate_scene_resource_id".to_string(),
                            message: format!(
                                "scene resource id `{}` was declared more than once in `{target_file}`",
                                scene_decl.id
                            ),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                }
                "scene_export" => {
                    let export = serde_json::from_value::<SceneExportDecl>(value.clone())?;
                    let mut scene_value = export.scene;
                    if scene_value
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .map(|id| id.is_empty())
                        .unwrap_or(true)
                    {
                        scene_value["id"] = Value::String(export.id.clone());
                    }
                    let mut scene_decl =
                        decode_scene_decl(app_root, &scene_value, target_file, Some(scene_registry))?;
                    normalize_shared_context(
                        &mut scene_decl.shared,
                        "scene_export.shared",
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    normalize_scene_params(
                        &mut scene_decl.params,
                        &format!("scene export `{}`.params", scene_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    normalize_scene_bindings(
                        &mut scene_decl.bindings,
                        &format!("scene export `{}`.bindings", scene_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    normalize_scene_examples(
                        &mut scene_decl.examples,
                        &format!("scene export `{}`.examples", scene_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    if ctx
                        .scenes
                        .insert(scene_decl.id.clone(), scene_decl.clone())
                        .is_some()
                    {
                        ctx.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "duplicate_scene_resource_id".to_string(),
                            message: format!(
                                "scene resource id `{}` was declared more than once in `{target_file}`",
                                scene_decl.id
                            ),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                }
                "world" => {
                    if ctx.first_world_decl_index.is_none() {
                        ctx.first_world_decl_index = Some(decl_index);
                    }
                    ctx.world_decl_count += 1;
                    let world_value = value.clone();
                    let mut world_decl =
                        serde_json::from_value::<crate::model::WorldDecl>(world_value.clone())?;
                    if world_decl.base.is_some() || world_value.get("base").is_some() {
                        match normalize_world_decl(
                            app_root,
                            world_decl,
                            &world_value,
                            scene_registry,
                            &mut ctx.diagnostics,
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
                        ctx.worlds.insert(id.to_string(), world_decl);
                    } else {
                        if ctx.world_default.is_none() {
                            ctx.world_default = Some(world_decl);
                        }
                    }
                    ctx.seen_world_decl = true;
                }
                "world_add_resource" => {
                    if !ctx.seen_world_decl {
                        ctx.diagnostics.push(Diagnostic {
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
                            &mut ctx.diagnostics,
                            target_file,
                        ) {
                            ctx.pending_world_resources.push(resource);
                        }
                    }
                }
                "world_add_entity" => {
                    if !ctx.seen_world_decl {
                        ctx.diagnostics.push(Diagnostic {
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
                            &mut ctx.diagnostics,
                            target_file,
                        ) {
                            ctx.pending_world_entities.push(entity);
                        }
                    }
                }
                "world_add_metric" => {
                    if !ctx.seen_world_decl {
                        ctx.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldAddMetricDecl>(value.clone())?;
                    if decl.kind == "world_add_metric" {
                        ctx.pending_world_metrics.push(decl.metric);
                    }
                }
                "world_set_topology" => {
                    if !ctx.seen_world_decl {
                        ctx.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldSetTopologyDecl>(value.clone())?;
                    if decl.kind == "world_set_topology" {
                        ctx.world_topology_set_count += 1;
                        if ctx.pending_world_topology.is_none() {
                            ctx.pending_world_topology = Some(decl.topology);
                        }
                    }
                }
                "frame_set_layout" => {
                    let decl = serde_json::from_value::<FrameSetLayoutDecl>(value.clone())?;
                    if decl.kind == "frame_set_layout" {
                        ctx.frame_layout_set_count += 1;
                        if ctx.pending_frame_layout.is_none() {
                            ctx.pending_frame_layout =
                                Some(serde_json::from_value::<crate::model::LayoutDecl>(
                                    decl.layout,
                                )?);
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
                            &mut ctx.diagnostics,
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
                        ctx.flows.insert(id.to_string(), flow_decl);
                    } else {
                        ctx.flow_default = Some(flow_decl);
                    }
                }
                "theme" => {
                    let resolver = crate::config_refs::ConfigRefResolver::new(&ctx.config);
                    let theme_value = resolver.resolve_config_refs_in_value(
                        value,
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    let mut theme_decl = serde_json::from_value::<ThemeDecl>(theme_value)?;
                    normalize_shared_context(
                        &mut theme_decl.shared,
                        &format!("theme `{}`.shared", theme_decl.id),
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    crate::theme_tokens::validate_theme_decl(
                        &theme_decl,
                        target_file,
                        &mut ctx.diagnostics,
                    );
                    ctx.themes.push(theme_decl);
                }
                "panel" => {
                    if let Some(panel) = resolve_panel_slot(
                        app_root,
                        value,
                        scene_registry,
                        &mut ctx.diagnostics,
                        target_file,
                    ) {
                        crate::theme_tokens::validate_panel_token_refs(
                            &panel,
                            target_file,
                            &mut ctx.diagnostics,
                        );
                        ctx.panels.push(panel);
                    }
                }
                "dataset_view" => ctx.top_level_legacy_dataset_view_count += 1,
                "app" | "app_scene_ref" => {}
                _ => ctx.diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(target_file.to_string()),
                }),
            }
        }
    }
    Ok(())
}
