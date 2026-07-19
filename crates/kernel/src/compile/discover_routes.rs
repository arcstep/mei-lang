use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::Result;

use serde_json::Value;
use walkdir::WalkDir;

use crate::mei_config::{
    app_mei_config_path, load_mei_config_for_app, workspace_config_path,
    MEI_WORKSPACE_CONFIG_FILENAME,
};
use crate::model::{
    CompiledSceneRoute, ComponentAsset, Diagnostic, LoadedResource, Severity,
    WorldMetricLedgerEntry,
};
use crate::typed_refs::SceneRegistry;

use super::load_external::load_scene_decls_from_file;
use super::materialize::materialize_world_metrics;
use super::scene::find_scene_route;
use super::scene_payload_cache::compile_scene_payload_for_target;

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub scene: Option<String>,
    pub preview_target: Option<String>,
    /// When true (default), layout_policy_* Severity::Error diagnostics fail compile.
    pub strict_layout_policy: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            scene: None,
            preview_target: None,
            strict_layout_policy: true,
        }
    }
}

impl CompileOptions {
    pub fn strict_layout() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileWatchedFile {
    pub rel_path: String,
    pub modified_ms: u128,
    pub size_bytes: u64,
    pub content_signature: Option<String>,
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
    if routes
        .iter()
        .any(|r| r.scene_id == scene_id || (r.target_file == target_file && r.scene_id == scene_id))
    {
        return;
    }
    routes.push(CompiledSceneRoute {
        scene_id,
        frame_id: None,
        target_file,
        kind: "file_ref".to_string(),
        title: None,
        short_title: None,
        is_default: false,
        access_export,
    });
}

pub(super) fn route_targets_preview(
    route: &CompiledSceneRoute,
    preview_target: Option<&str>,
) -> bool {
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
    affected_targets: Option<&BTreeSet<String>>,
) -> bool {
    if let Some(targets) = affected_targets {
        if !targets.is_empty() {
            return targets.contains(route.target_file.as_str());
        }
    }
    route_targets_preview(route, preview_target)
}

/// Manage `preview_only` 预编译 route 列表：同文件多 `scene_export` 时按 `options.scene` 裁剪。
pub(super) fn manage_preview_precompile_routes(
    options: &CompileOptions,
    routes: &[CompiledSceneRoute],
    preview_affected_targets: Option<&BTreeSet<String>>,
) -> Result<Vec<CompiledSceneRoute>, Diagnostic> {
    let preview_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty());

    let candidates = routes
        .iter()
        .filter(|route| {
            route_matches_preview_scope(route, preview_target, preview_affected_targets)
        })
        .cloned()
        .collect::<Vec<_>>();

    let Some(preview_file) = preview_target else {
        return Ok(candidates);
    };

    let export_ids = candidates
        .iter()
        .filter(|route| route.target_file == preview_file)
        .map(|route| route.scene_id.as_str())
        .collect::<BTreeSet<_>>();

    if export_ids.len() <= 1 {
        return Ok(candidates);
    }

    let requested_scene = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());

    let Some(requested_scene) = requested_scene else {
        return Err(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene_export_selector".to_string(),
            message: format!(
                "scene resource file declares multiple exported scenes [{}]; select one via scene_id/route before compile",
                export_ids.into_iter().collect::<Vec<_>>().join(", ")
            ),
            source_path: Some(preview_file.to_string()),
        });
    };

    let mut filtered = Vec::with_capacity(candidates.len());
    let mut matched_on_preview_file = false;
    for route in candidates {
        if route.target_file == preview_file {
            if route.scene_id == requested_scene {
                matched_on_preview_file = true;
                filtered.push(route);
            }
            continue;
        }
        filtered.push(route);
    }

    if !matched_on_preview_file {
        return Err(Diagnostic {
            severity: Severity::Error,
            code: "board_export_not_found".to_string(),
            message: format!(
                "scene `{requested_scene}` not found among exported scenes for preview target `{preview_file}`"
            ),
            source_path: Some(preview_file.to_string()),
        });
    }

    Ok(filtered)
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

pub(super) fn is_manage_preview_only_compile(
    options: &CompileOptions,
    app_entry_main: &str,
) -> bool {
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
    let entry_path = crate::mei_config::resolve_app_mei_file_path(app_root, &entry_main);
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
    prune_flat_imported_world_metric_aliases(&mut ledger);
    Ok(ledger)
}

fn prune_flat_imported_world_metric_aliases(ledger: &mut BTreeMap<String, WorldMetricLedgerEntry>) {
    let flat_ids = ledger
        .keys()
        .filter(|id| !id.contains("::"))
        .cloned()
        .collect::<Vec<_>>();
    for flat_id in flat_ids {
        let suffix = format!("::{flat_id}");
        let superseded = ledger
            .keys()
            .any(|namespaced| namespaced.contains(".mei::") && namespaced.ends_with(&suffix));
        if superseded {
            ledger.remove(&flat_id);
        }
    }
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
        if preview.ends_with(".mei") {
            if let Ok(scenes) = load_scene_decls_from_file(app_root, preview) {
                let requested_scene = scene_selector.map(str::trim).filter(|s| !s.is_empty());
                let scenes_to_register = if preview_only {
                    match requested_scene {
                        Some(requested) => scenes
                            .into_iter()
                            .filter(|scene| scene.id.trim() == requested)
                            .collect::<Vec<_>>(),
                        None if scenes.len() == 1 => scenes,
                        None => Vec::new(),
                    }
                } else {
                    scenes
                };
                for scene in scenes_to_register {
                    let sid = scene.id.trim().to_string();
                    if !sid.is_empty() {
                        try_push_discovered_entry_route(
                            routes,
                            sid,
                            preview.to_string(),
                            scene.access_export,
                        );
                    }
                }
            } else {
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
        let rel_str = rel_str
            .strip_prefix("src/")
            .map(str::to_string)
            .unwrap_or(rel_str);
        let full = crate::mei_config::resolve_app_mei_file_path(app_root, rel_str.as_str());
        mei_files.push((rel_str, file_modified_ms(&full)));
    }
    mei_files.sort_by(|a, b| b.1.cmp(&a.1));

    let mut probed = 0usize;
    for (rel_str, _) in mei_files {
        if probed >= MAX_MEI_PROBES {
            break;
        }
        probed += 1;
        if let Ok(scenes) = load_scene_decls_from_file(app_root, rel_str.as_str()) {
            let mut matched = false;
            for scene in scenes {
                if scene.id.trim() != requested {
                    continue;
                }
                try_push_discovered_entry_route(
                    routes,
                    scene.id.trim().to_string(),
                    rel_str.clone(),
                    scene.access_export,
                );
                matched = true;
            }
            if matched {
                break;
            }
            continue;
        }
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
