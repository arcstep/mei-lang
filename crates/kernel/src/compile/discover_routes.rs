use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::Result;

use serde_json::Value;
use walkdir::WalkDir;

use crate::mei_config::{
    app_mei_config_path, load_mei_config_for_app, workspace_config_path, MEI_WORKSPACE_CONFIG_FILENAME,
};
use crate::model::{CompiledSceneRoute, ComponentAsset, Diagnostic, LoadedResource, Severity, WorldMetricLedgerEntry};
use crate::typed_refs::SceneRegistry;

use super::materialize::materialize_world_metrics;
use super::scene::find_scene_route;
use super::scene_payload_cache::compile_scene_payload_for_target;

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub scene: Option<String>,
    pub preview_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileWatchedFile {
    pub rel_path: String,
    pub modified_ms: u128,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CompileRevisionPlan {
    pub token: String,
    pub watched_files: Vec<CompileWatchedFile>,
    pub components_revision: u128,
}


pub(super) fn try_push_discovered_entry_route(
    routes: &mut Vec<CompiledSceneRoute>,
    scene_id: String,
    target_file: String,
    access_export: bool,
) {
    let scene_id = scene_id.trim().to_string();
    let target_file = target_file.trim().to_string();
    if scene_id.is_empty() || target_file.is_empty() {
        return;
    }
    if routes.iter().any(|r| r.target_file == target_file) {
        return;
    }
    if routes.iter().any(|r| r.scene_id == scene_id) {
        return;
    }
    routes.push(CompiledSceneRoute {
        scene_id,
        frame_id: None,
        target_file,
        kind: "file_ref".to_string(),
        title: None,
        is_default: false,
        access_export,
    });
}

pub(super) fn route_targets_preview(route: &CompiledSceneRoute, preview_target: Option<&str>) -> bool {
    let Some(preview) = preview_target
        .map(str::trim)
        .filter(|target| !target.is_empty())
    else {
        return false;
    };
    route.target_file == preview
}

pub(super) fn route_matches_preview_scope(
    route: &CompiledSceneRoute,
    preview_target: Option<&str>,
    affected_targets: Option<&std::collections::BTreeSet<String>>,
) -> bool {
    if let Some(targets) = affected_targets {
        if !targets.is_empty() {
            return targets.contains(route.target_file.as_str());
        }
    }
    route_targets_preview(route, preview_target)
}

pub(super) fn catalog_focus_target<'a>(
    options: &'a CompileOptions,
    active_target_file: Option<&'a str>,
) -> Option<&'a str> {
    options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .or_else(|| {
            options
                .scene
                .as_deref()
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .and(active_target_file)
        })
}

pub(super) fn manage_preview_target<'a>(
    options: &'a CompileOptions,
    app_entry_main: &str,
) -> Option<&'a str> {
    // scene-first：Manage 可同时带 scene 锚与 preview_target（source-focus）；
    // 单文件预览裁剪仍由 preview_target 决定，不得因 scene 已设置而退回全量 compile。
    options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty() && *target != app_entry_main)
}

/// Manage 态打开 dataset 单文件预览时，只编译目标入口，避免扫全库 dataset 入口。
pub(super) fn is_dataset_manage_preview(options: &CompileOptions, app_entry_main: &str) -> bool {
    manage_preview_target(options, app_entry_main)
        .is_some_and(|preview| preview.starts_with("data/") || preview.contains("/datasets/"))
}

/// Manage 态按 `?file=scenes/...` 预览 widget/layout 等：只编译该入口 scene，不编译 home 与其它路由。
pub(super) fn is_manage_entry_preview(options: &CompileOptions, app_entry_main: &str) -> bool {
    manage_preview_target(options, app_entry_main).is_some()
}

pub(super) fn is_manage_preview_only_compile(options: &CompileOptions, app_entry_main: &str) -> bool {
    is_dataset_manage_preview(options, app_entry_main)
        || is_manage_entry_preview(options, app_entry_main)
}

pub(super) fn push_app_config_diagnostics(app_root: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let config_path = app_mei_config_path(app_root);
    if !config_path.is_file() {
        return;
    }
    let config = load_mei_config_for_app(app_root, None);
    let entry_main = config.entry.main_rel();
    let entry_path = app_root.join(&entry_main);
    if !entry_path.is_file() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "invalid_entry_main".to_string(),
            message: format!(
                "`.mei-config.json` entry.main `{entry_main}` not found; update entry.main or create the file"
            ),
            source_path: Some(config_path.to_string_lossy().to_string()),
        });
    }
    if config.has_legacy_workspace_fields() {
        let has_workspace = app_root
            .parent()
            .map(workspace_config_path)
            .is_some_and(|path| path.is_file());
        if !has_workspace {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "legacy_app_workspace_fields".to_string(),
                message: format!(
                    "`.mei-config.json` contains discover/menu/runtime fields that belong in `{MEI_WORKSPACE_CONFIG_FILENAME}` at the workspace segment root"
                ),
                source_path: Some(config_path.to_string_lossy().to_string()),
            });
        }
    }
}

pub(super) fn build_world_metric_ledger(
    resources: &[LoadedResource],
    world_metric_values: &[Value],
) -> Result<BTreeMap<String, WorldMetricLedgerEntry>> {
    let mut ledger = BTreeMap::new();
    let mut order = 0usize;
    for resource in resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        for (metric_id, metric) in &dataset.metrics {
            let id = metric_id.trim();
            if id.is_empty() {
                continue;
            }
            order += 1;
            ledger.insert(
                id.to_string(),
                WorldMetricLedgerEntry {
                    id: id.to_string(),
                    owner_resource_id: resource.id.clone(),
                    order,
                    metric: metric.clone(),
                },
            );
        }
    }
    let direct_metrics = materialize_world_metrics(resources, world_metric_values)?;
    for (metric_id, metric) in direct_metrics {
        order += 1;
        ledger.insert(
            metric_id.clone(),
            WorldMetricLedgerEntry {
                id: metric_id,
                owner_resource_id: "__world_metrics__".to_string(),
                order,
                metric,
            },
        );
    }
    Ok(ledger)
}

pub(super) fn inject_discovered_entry_scene_routes(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    routes: &mut Vec<CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
    preview_target: Option<&str>,
    scene_selector: Option<&str>,
    preview_only: bool,
) {
    if let Some(preview) = preview_target.map(str::trim).filter(|s| !s.is_empty()) {
        if preview.ends_with(".mei") && !routes.iter().any(|r| r.target_file == preview) {
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                app_decls,
                asset_map,
                preview,
                None,
                scene_registry,
                None,
            );
            if let Some(contract) = payload.scene_contract.as_ref() {
                let sid = contract.scene.id.trim().to_string();
                if !sid.is_empty() {
                    try_push_discovered_entry_route(
                        routes,
                        sid,
                        preview.to_string(),
                        contract.scene.access_export,
                    );
                }
            }
        }
    }

    if preview_only {
        return;
    }

    let Some(requested) = scene_selector.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    if find_scene_route(routes, requested).is_some() {
        return;
    }

    pub(super) fn file_modified_ms(path: &Path) -> u128 {
        std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u128)
            .unwrap_or(0)
    }

    const MAX_MEI_PROBES: usize = 400;
    let mut mei_files: Vec<(String, u128)> = Vec::new();
    for entry in WalkDir::new(app_root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        if e.depth() > 0 {
            if matches!(name.as_ref(), ".git" | "node_modules" | "target" | ".mei") {
                return false;
            }
        }
        true
    }) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("mei") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(app_root) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if routes.iter().any(|r| r.target_file == rel_str) {
            continue;
        }
        let full: PathBuf = app_root.join(rel);
        mei_files.push((rel_str, file_modified_ms(&full)));
    }
    mei_files.sort_by(|a, b| b.1.cmp(&a.1));

    let mut probed = 0usize;
    for (rel_str, _) in mei_files {
        if probed >= MAX_MEI_PROBES {
            break;
        }
        probed += 1;
        let payload = compile_scene_payload_for_target(
            app_root,
            source_root,
            app_decls,
            asset_map,
            rel_str.as_str(),
            None,
            scene_registry,
            None,
        );
        let Some(contract) = payload.scene_contract.as_ref() else {
            continue;
        };
        if contract.scene.id.trim() != requested {
            continue;
        }
        try_push_discovered_entry_route(
            routes,
            contract.scene.id.trim().to_string(),
            rel_str,
            contract.scene.access_export,
        );
        break;
    }
}
