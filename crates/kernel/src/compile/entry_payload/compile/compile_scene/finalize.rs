use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::compile::decls::{LegacyDatasetDecl, LegacyMetricPackDecl};
use crate::compile::entry_payload::clone_merge::deep_merge_json;
use crate::compile::entry_payload::helpers::{
    all_world_resource_decls, decode_world_dataset_decl, decode_world_metric_pack_decl,
    insert_resource_checked, partition_world_resources,
};
use crate::compile::entry_payload::CompiledScenePayload;
use crate::compile::materialize::{
    append_world_metrics_dataset_resource_with_id, materialize_legacy_datasets,
    materialize_metric_packs, materialize_world_metrics, WORLD_METRICS_RESOURCE_ID,
};
use crate::compile::resources::load_resources;
use crate::compile::ui_data_policy::validate_scene_ui_data_bindings;
use crate::config_refs::decode_theme_ref_token;
use crate::model::{SceneContract, Severity, WorldMetricLedgerEntry};

use super::super::scene_binding::validate_config_refs;
use super::super::scene_contract::{
    resolve_scene_contract_config_refs, selected_custom_theme_shared,
};
use super::state::CompileSceneCtx;

pub(super) fn finalize_payload(
    ctx: &mut CompileSceneCtx,
    app_root: &Path,
    target_file: &str,
    entry_decls: &Value,
) -> Result<CompiledScenePayload> {
    let mut resources = Vec::new();
    let mut world_dataset_decls: Vec<LegacyDatasetDecl> = Vec::new();
    let mut world_metric_pack_decls: Vec<LegacyMetricPackDecl> = Vec::new();
    if let Some(world_decl) = ctx.world.as_ref() {
        let (normal_resources, dataset_resources) =
            partition_world_resources(&all_world_resource_decls(world_decl));
        resources = load_resources(
            app_root,
            &normal_resources,
            target_file,
            &mut ctx.diagnostics,
        )?;
        for resource in dataset_resources {
            if resource.id == "__source_path__" || resource.id.ends_with(".mei") {
                ctx.diagnostics.push(crate::model::Diagnostic {
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
                    Err(message) => ctx.diagnostics.push(crate::model::Diagnostic {
                        severity: Severity::Error,
                        code: "decode_world_dataset_decl_failed".to_string(),
                        message,
                        source_path: Some(target_file.to_string()),
                    }),
                },
                "metric_pack" => match decode_world_metric_pack_decl(resource.clone()) {
                    Ok(decl) => world_metric_pack_decls.push(decl),
                    Err(message) => ctx.diagnostics.push(crate::model::Diagnostic {
                        severity: Severity::Error,
                        code: "decode_world_metric_pack_decl_failed".to_string(),
                        message,
                        source_path: Some(target_file.to_string()),
                    }),
                },
                _ => ctx.diagnostics.push(crate::model::Diagnostic {
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
        ctx.scenes.values().next(),
        target_file,
        &mut ctx.diagnostics,
    );
    let has_config_ref_errors = ctx.diagnostics.iter().any(|diag| {
        diag.severity == Severity::Error
            && matches!(
                diag.code.as_str(),
                "missing_config_ref" | "invalid_config_ref"
            )
    });
    if !world_dataset_decls.is_empty() && !has_config_ref_errors {
        let derived = materialize_legacy_datasets(app_root, &resources, &world_dataset_decls)?;
        for resource in derived {
            insert_resource_checked(&mut resources, resource, target_file, &mut ctx.diagnostics);
        }
    }
    if !world_metric_pack_decls.is_empty() {
        let derived = materialize_metric_packs(&resources, &world_metric_pack_decls)?;
        for resource in derived {
            insert_resource_checked(&mut resources, resource, target_file, &mut ctx.diagnostics);
        }
    }

    let host_local_ids =
        crate::compile::entry_payload::import_scope::host_local_resource_ids(&resources);
    let mut imported_runtime =
        crate::compile::entry_payload::import_scope::finalize_private_import_world(
            app_root,
            &ctx.panels,
            &host_local_ids,
            target_file,
            &mut ctx.diagnostics,
        );
    resources.append(&mut imported_runtime);

    if let Some(world_decl) = ctx.world.as_ref() {
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

    let frame = ctx.frame.take();
    let world = ctx.world.take();
    let flow = ctx.flow.take();
    let panels = std::mem::take(&mut ctx.panels);
    let themes = std::mem::take(&mut ctx.themes);

    let mut scene_contract = ctx.selected_scene.take().map(|scene_decl| {
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
        let resolver = crate::config_refs::ConfigRefResolver::new(&ctx.config);
        resolve_scene_contract_config_refs(contract, &resolver, target_file, &mut ctx.diagnostics);
    }
    if let Some(ref contract) = scene_contract {
        validate_scene_ui_data_bindings(
            contract,
            &resources,
            app_root,
            target_file,
            &mut ctx.diagnostics,
        );
    }
    if let Some(contract) = scene_contract.as_ref() {
        let config = crate::mei_config::load_mei_config_for_app(app_root, None);
        let resolver = crate::config_refs::ConfigRefResolver::new(&config);
        if let Some(theme) = contract.scene.theme.as_deref() {
            if decode_theme_ref_token(theme).is_some() {
                resolver.validate_theme_token(theme, target_file, &mut ctx.diagnostics);
            }
        }
    }

    Ok(CompiledScenePayload {
        scene_contract,
        resources,
        component_assets: std::mem::take(&mut ctx.component_assets),
        diagnostics: std::mem::take(&mut ctx.diagnostics),
    })
}
