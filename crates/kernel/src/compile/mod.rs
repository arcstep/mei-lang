use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use anyhow::{anyhow, Result};
use serde_json::Value;
use walkdir::WalkDir;

use crate::{
    eval::evaluate_mei_file,
    model::{CompiledApp, CompiledSceneRoute, ComponentAsset, Diagnostic, Severity},
    typed_refs::SceneRegistry,
    workspace::{load_component_assets, source_tree},
};

mod analysis;
mod app_decl;
mod catalog;
mod decls;
mod entry_payload;
mod load_external;
mod loaders;
mod materialize;
mod materialize_cache;
mod mutations;
mod scene_payload_cache;
mod resources;
mod scene;
mod scene_binding;
mod ui_data_policy;

use ui_data_policy::validate_imported_catalog_world_refs;

use app_decl::decode_app_decl;
use catalog::{
    build_dataset_catalog_filter, compile_dataset_catalog_resources, merge_resource_catalog,
    DatasetCatalogFilter,
};
use entry_payload::CompiledScenePayload;
use scene_payload_cache::compile_scene_payload_for_target;
use scene::{find_scene_route, resolve_scene_routes};

/// 将「仅声明在入口 .mei 内、未出现在 app 路由表」的 scene 登记为临时 file_ref 路由，
/// 以便管理态预览与访问态 `/scene/<id>` 能解析到同一入口文件。
///
/// 与 `mei-lang-server` 的 `compile_revision`（目录最新 mtime）配合：磁盘一旦有变更即编译缓存失效，
/// 下一次访问会重新走本逻辑。按 `.mei` 修改时间倒序探测，使 Agent 刚写入的入口优先命中。
fn try_push_discovered_entry_route(
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

fn route_targets_preview(route: &CompiledSceneRoute, preview_target: Option<&str>) -> bool {
    let Some(preview) = preview_target.map(str::trim).filter(|target| !target.is_empty()) else {
        return false;
    };
    route.target_file == preview
}

fn manage_preview_target(options: &CompileOptions) -> Option<&str> {
    // scene-first：Manage 可同时带 scene 锚与 preview_target（source-focus）；
    // 单文件预览裁剪仍由 preview_target 决定，不得因 scene 已设置而退回全量 compile。
    options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty() && *target != "main.mei")
}

/// Manage 态打开 dataset 单文件预览时，只编译目标入口，避免扫全库 dataset 入口。
fn is_dataset_manage_preview(options: &CompileOptions) -> bool {
    manage_preview_target(options).is_some_and(|preview| {
        preview.starts_with("data/") || preview.contains("/datasets/")
    })
}

/// Manage 态按 `?file=scenes/...` 预览 widget/layout 等：只编译该入口 scene，不编译 home 与其它路由。
fn is_manage_entry_preview(options: &CompileOptions) -> bool {
    manage_preview_target(options).is_some()
}

fn is_manage_preview_only_compile(options: &CompileOptions) -> bool {
    is_dataset_manage_preview(options) || is_manage_entry_preview(options)
}

fn inject_discovered_entry_scene_routes(
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

    fn file_modified_ms(path: &Path) -> u128 {
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

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub scene: Option<String>,
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
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);

    let asset_map = load_component_assets(source_root)?;
    let preview_only = is_manage_preview_only_compile(&options);
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    inject_discovered_entry_scene_routes(
        app_root,
        source_root,
        &app_decls,
        &asset_map,
        &mut route_registry.routes,
        &scene_registry,
        options.preview_target.as_deref(),
        options.scene.as_deref(),
        preview_only,
    );
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    let mut official_results: BTreeMap<String, CompiledScenePayload> = BTreeMap::new();
    for route in &route_registry.routes {
        if preview_only && !route_targets_preview(route, options.preview_target.as_deref()) {
            continue;
        }
        let result = compile_scene_payload_for_target(
            app_root,
            source_root,
            &app_decls,
            &asset_map,
            route.target_file.as_str(),
            Some(route),
            &scene_registry,
        );
        official_results.insert(route.scene_id.clone(), result);
    }

    let active_route_meta = if let Some(requested) = options.scene.as_deref() {
        let selected = find_scene_route(&route_registry.routes, requested).cloned();
        if selected.is_none() {
            let preview_route = options
                .preview_target
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
                .and_then(|target| {
                    route_registry
                        .routes
                        .iter()
                        .find(|route| route.target_file == target)
                        .cloned()
                });
            if preview_route.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_scene".to_string(),
                    message: format!("scene `{requested}` not found, fallback to default scene"),
                    source_path: Some(app_main.to_string_lossy().to_string()),
                });
            }
            preview_route.or_else(|| {
                route_registry
                    .default_scene_id
                    .as_deref()
                    .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
                    .cloned()
                    .or_else(|| route_registry.routes.first().cloned())
            })
        } else {
            selected
        }
    } else {
        route_registry
            .default_scene_id
            .as_deref()
            .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
            .cloned()
            .or_else(|| route_registry.routes.first().cloned())
    };

    let selected_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|value| value.to_string());

    let (active_scene, active_target_file, mut active_payload) = if let Some(target_file) =
        selected_target
    {
        if let Some(scene_route) = route_registry
            .routes
            .iter()
            .find(|route| route.target_file == target_file)
            .cloned()
        {
            let payload = official_results
                .get(&scene_route.scene_id)
                .cloned()
                .unwrap_or_else(|| {
                    compile_scene_payload_for_target(
                        app_root,
                        source_root,
                        &app_decls,
                        &asset_map,
                        target_file.as_str(),
                        Some(&scene_route),
                        &scene_registry,
                    )
                });
            (Some(scene_route.scene_id), target_file, payload)
        } else {
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                &app_decls,
                &asset_map,
                target_file.as_str(),
                None,
                &scene_registry,
            );
            if target_file == "main.mei" && payload.scene_contract.is_none() {
                let fallback_route = active_route_meta.clone().or_else(|| {
                    route_registry
                        .default_scene_id
                        .as_deref()
                        .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
                        .cloned()
                });
                if let Some(route_meta) = fallback_route {
                    let fallback_payload = official_results
                        .get(&route_meta.scene_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            compile_scene_payload_for_target(
                                app_root,
                                source_root,
                                &app_decls,
                                &asset_map,
                                route_meta.target_file.as_str(),
                                Some(&route_meta),
                                &scene_registry,
                            )
                        });
                    (Some(route_meta.scene_id), target_file, fallback_payload)
                } else {
                    (None, target_file, payload)
                }
            } else {
                (None, target_file, payload)
            }
        }
    } else if let Some(route_meta) = active_route_meta {
        let payload = official_results
            .get(&route_meta.scene_id)
            .cloned()
            .unwrap_or_else(|| {
                compile_scene_payload_for_target(
                    app_root,
                    source_root,
                    &app_decls,
                    &asset_map,
                    route_meta.target_file.as_str(),
                    Some(&route_meta),
                    &scene_registry,
                )
            });
        (Some(route_meta.scene_id), route_meta.target_file, payload)
    } else {
        (
            None,
            "main.mei".to_string(),
            compile_scene_payload_for_target(
                app_root,
                source_root,
                &app_decls,
                &asset_map,
                "main.mei",
                None,
                &scene_registry,
            ),
        )
    };

    diagnostics.append(&mut active_payload.diagnostics);

    if let Some(active_id) = active_scene.as_deref() {
        let default_key = route_registry
            .default_scene_id
            .as_deref()
            .unwrap_or(active_id);
        for route in &mut route_registry.routes {
            route.is_default = route.scene_id == default_key;
        }
    }
    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    let dataset_manage_preview = is_dataset_manage_preview(&options);
    let route_target_files: Vec<String> = route_registry
        .routes
        .iter()
        .map(|route| route.target_file.clone())
        .collect();
    let catalog_filter = if dataset_manage_preview {
        DatasetCatalogFilter::default()
    } else {
        build_dataset_catalog_filter(
            app_root,
            options.preview_target.as_deref(),
            route_target_files.as_slice(),
        )
    };
    let dataset_catalog = if dataset_manage_preview {
        Vec::new()
    } else {
        compile_dataset_catalog_resources(
            app_root,
            source_root,
            &app_decls,
            &asset_map,
            &catalog_filter,
        )
    };
    let scene_resources = active_payload.resources.clone();
    let resources = merge_resource_catalog(dataset_catalog, scene_resources);
    if let Some(contract) = active_payload.scene_contract.as_ref() {
        validate_imported_catalog_world_refs(
            contract,
            &active_payload.resources,
            &resources,
            active_target_file.as_str(),
            &mut diagnostics,
        );
    }

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        scene_routes: route_registry.routes,
        active_scene,
        active_target_file,
        file_tree: source_tree(app_root)?,
        scene_contract: active_payload.scene_contract,
        resources,
        component_assets: active_payload.component_assets,
        diagnostics,
    })
}

pub use materialize_cache::dataset_materialize_cache_epoch;
pub use scene_payload_cache::scene_payload_cache_epoch;

#[cfg(test)]
pub(crate) use materialize_cache::{
    clear_materialize_cache_for_tests, legacy_rows_cache_len_for_tests,
};
#[cfg(test)]
pub(crate) use scene_payload_cache::{
    clear_scene_payload_cache_for_tests, scene_payload_cache_len_for_tests,
};

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
