use std::path::Path;

use crate::compile::mutations::{apply_frame_mutations, apply_world_mutations};
use crate::compile::panel_normalize::normalize_panel_slots;
use crate::model::{Diagnostic, Severity};
use crate::typed_refs::SceneRegistry;

use super::super::flow_binding::merge_frame_panel_slots;
use super::state::CompileSceneCtx;

pub(super) fn validate_and_apply_mutations(
    ctx: &mut CompileSceneCtx,
    app_root: &Path,
    target_file: &str,
    scene_registry: &SceneRegistry,
) {
    if let (Some(scene_idx), Some(world_idx)) =
        (ctx.first_scene_decl_index, ctx.first_world_decl_index)
    {
        if world_idx < scene_idx {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "world_before_scene_decl".to_string(),
                message: "`world(...)` must appear after `scene(...)` in the same file when both are declared (_declare order)"
                    .to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
    let has_dataset_library_content = ctx.top_level_legacy_dataset_count > 0
        || ctx.top_level_legacy_metric_pack_count > 0
        || ctx.top_level_legacy_dataset_view_count > 0;
    let had_pending_topology = ctx.pending_world_topology.is_some();
    let had_pending_frame_layout = ctx.pending_frame_layout.is_some();
    let has_authoring_surface = ctx.scene_decl_count > 0
        || ctx.frame_decl_count > 0
        || ctx.world_decl_count > 0
        || !ctx.flows.is_empty()
        || ctx.flow_default.is_some()
        || !ctx.panels.is_empty()
        || !ctx.themes.is_empty()
        || ctx.world_topology_set_count > 0
        || ctx.frame_layout_set_count > 0
        || !ctx.pending_world_resources.is_empty()
        || !ctx.pending_world_entities.is_empty()
        || !ctx.pending_world_metrics.is_empty()
        || had_pending_topology
        || had_pending_frame_layout;
    ctx.dataset_library_only = has_dataset_library_content
        && !has_authoring_surface
        && target_file != ctx.app_entry_main;

    if ctx.top_level_legacy_dataset_count > 0
        || ctx.top_level_legacy_dataset_view_count > 0
        || ctx.top_level_legacy_metric_pack_count > 0
    {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "forbidden_top_level_dataset_decls".to_string(),
            message: "world-only mode forbids top-level dataset()/dataset_view()/metric_pack(); use world.add_dataset()/world.add_dataset_view()/world.add_metric_pack() or world resources list".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    if ctx.world_topology_set_count > 1 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_world_topologies".to_string(),
            message: format!(
                "file `{target_file}` declares {} world.set_topology(...) blocks, expected at most one",
                ctx.world_topology_set_count
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if ctx.frame_layout_set_count > 1 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_frame_layouts".to_string(),
            message: format!(
                "file `{target_file}` declares {} frame.set_layout(...) blocks, expected at most one",
                ctx.frame_layout_set_count
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    apply_world_mutations(
        &mut ctx.worlds,
        &mut ctx.world_default,
        &ctx.pending_world_resources,
        &ctx.pending_world_entities,
        &ctx.pending_world_metrics,
        ctx.pending_world_topology.take(),
        &mut ctx.diagnostics,
        target_file,
        ctx.world_decl_count,
    );
    apply_frame_mutations(
        &mut ctx.frames,
        &mut ctx.frame_default,
        ctx.pending_frame_layout.take(),
        &mut ctx.diagnostics,
        target_file,
        ctx.frame_decl_count,
    );
    merge_frame_panel_slots(
        app_root,
        &ctx.frames,
        ctx.frame_default.as_ref(),
        &mut ctx.panels,
        scene_registry,
        &mut ctx.diagnostics,
        target_file,
    );
    normalize_panel_slots(&mut ctx.panels, &mut ctx.diagnostics, target_file);
    if ctx.scene_decl_count > 1 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_scenes".to_string(),
            message: format!(
                "file `{target_file}` declares {} scene(...) blocks, expected exactly one",
                ctx.scene_decl_count
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if ctx.world_decl_count > 1 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_worlds".to_string(),
            message: format!(
                "file `{target_file}` declares {} world(...) blocks, expected exactly one",
                ctx.world_decl_count
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if ctx.frame_decl_count > 1 {
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_frames".to_string(),
            message: format!(
                "file `{target_file}` declares {} frame(...) blocks, expected exactly one",
                ctx.frame_decl_count
            ),
            source_path: Some(target_file.to_string()),
        });
    }
}
