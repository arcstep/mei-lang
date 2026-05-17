use std::{collections::BTreeMap, path::Path};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    model::{CompiledApp, Diagnostic, Severity},
    workspace::{load_component_assets, source_tree},
};

mod analysis;
mod app_decl;
mod decls;
mod entry_payload;
mod load_external;
mod loaders;
mod materialize;
mod mutations;
mod resources;
mod scene;
mod scene_binding;
mod ui_data_policy;

use app_decl::decode_app_decl;
use entry_payload::{compile_entry_payload_for_target, CompiledEntryPayload};
use scene::{find_scene_entry, resolve_scene_entries};

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub entry: Option<String>,
    pub preview_target: Option<String>,
}

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    compile_app_with_options(source_root, app_id, CompileOptions::default())
}

pub fn compile_app_with_options(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
) -> Result<CompiledApp> {
    let app_root = source_root.join(app_id);
    compile_app_from_root_with_options(source_root, &app_root, options)
}

pub fn compile_app_from_root(source_root: &Path, app_root: &Path) -> Result<CompiledApp> {
    compile_app_from_root_with_options(source_root, app_root, CompileOptions::default())
}

pub fn compile_app_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: CompileOptions,
) -> Result<CompiledApp> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let mut entry_registry =
        resolve_scene_entries(&app_main, &app_decl, &app_decls, &mut diagnostics);

    let asset_map = load_component_assets(source_root)?;
    let mut official_results: BTreeMap<String, CompiledEntryPayload> = BTreeMap::new();
    for entry in &entry_registry.entries {
        let result = compile_entry_payload_for_target(
            app_root,
            &app_decls,
            &asset_map,
            entry.target_file.as_str(),
            Some(entry),
        );
        official_results.insert(entry.entry_id.clone(), result);
    }

    let active_entry_meta = if let Some(requested_entry) = options.entry.as_deref() {
        let selected = find_scene_entry(&entry_registry.entries, requested_entry).cloned();
        if selected.is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "unknown_entry".to_string(),
                message: format!("entry `{requested_entry}` not found, fallback to default entry"),
                source_path: Some(app_main.to_string_lossy().to_string()),
            });
            entry_registry
                .default_entry_id
                .as_deref()
                .and_then(|entry_id| find_scene_entry(&entry_registry.entries, entry_id))
                .cloned()
                .or_else(|| entry_registry.entries.first().cloned())
        } else {
            selected
        }
    } else {
        entry_registry
            .default_entry_id
            .as_deref()
            .and_then(|entry_id| find_scene_entry(&entry_registry.entries, entry_id))
            .cloned()
            .or_else(|| entry_registry.entries.first().cloned())
    };

    let selected_target = options
        .preview_target
        .as_deref()
        .filter(|_| options.entry.is_none())
        .map(|value| value.to_string());

    let (active_entry, entry_target, mut active_payload) = if let Some(target_file) =
        selected_target
    {
        if let Some(scene_entry) = entry_registry
            .entries
            .iter()
            .find(|entry| entry.target_file == target_file)
            .cloned()
        {
            let payload = official_results
                .get(&scene_entry.entry_id)
                .cloned()
                .unwrap_or_else(|| {
                    compile_entry_payload_for_target(
                        app_root,
                        &app_decls,
                        &asset_map,
                        target_file.as_str(),
                        Some(&scene_entry),
                    )
                });
            (Some(scene_entry.entry_id), target_file, payload)
        } else {
            let payload = compile_entry_payload_for_target(
                app_root,
                &app_decls,
                &asset_map,
                target_file.as_str(),
                None,
            );
            if target_file == "main.mei" && payload.scene_contract.is_none() {
                let fallback_entry = active_entry_meta.clone().or_else(|| {
                    entry_registry
                        .default_entry_id
                        .as_deref()
                        .and_then(|entry_id| find_scene_entry(&entry_registry.entries, entry_id))
                        .cloned()
                });
                if let Some(entry_meta) = fallback_entry {
                    let fallback_payload = official_results
                        .get(&entry_meta.entry_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            compile_entry_payload_for_target(
                                app_root,
                                &app_decls,
                                &asset_map,
                                entry_meta.target_file.as_str(),
                                Some(&entry_meta),
                            )
                        });
                    (Some(entry_meta.entry_id), target_file, fallback_payload)
                } else {
                    (None, target_file, payload)
                }
            } else {
                (None, target_file, payload)
            }
        }
    } else if let Some(entry_meta) = active_entry_meta {
        let payload = official_results
            .get(&entry_meta.entry_id)
            .cloned()
            .unwrap_or_else(|| {
                compile_entry_payload_for_target(
                    app_root,
                    &app_decls,
                    &asset_map,
                    entry_meta.target_file.as_str(),
                    Some(&entry_meta),
                )
            });
        (Some(entry_meta.entry_id), entry_meta.target_file, payload)
    } else {
        (
            None,
            "main.mei".to_string(),
            compile_entry_payload_for_target(app_root, &app_decls, &asset_map, "main.mei", None),
        )
    };

    diagnostics.append(&mut active_payload.diagnostics);

    if let Some(active_id) = active_entry.as_deref() {
        for entry in &mut entry_registry.entries {
            entry.is_default = entry.entry_id
                == entry_registry
                    .default_entry_id
                    .as_deref()
                    .unwrap_or(active_id);
        }
    }
    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        entries: entry_registry.entries,
        active_entry,
        entry_target,
        file_tree: source_tree(app_root)?,
        scene_contract: active_payload.scene_contract,
        resources: active_payload.resources,
        component_assets: active_payload.component_assets,
        diagnostics,
    })
}

pub fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, crate::model::MetricContract>> {
    materialize::evaluate_runtime_metric_defs(metric_defs, base_rows, datasets, metric_ids)
}

#[cfg(test)]
mod tests;
