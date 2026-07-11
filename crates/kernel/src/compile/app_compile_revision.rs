use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Result};

use crate::{
    eval::evaluate_mei_file,
    mei_config::{
        app_mei_config_path, resolve_app_entry_main, resolve_app_main_path, MeiConfig,
        MEI_CONFIG_FILENAME,
    },
    model::CompiledSceneRoute,
    typed_refs::SceneRegistry,
    workspace::load_component_assets,
};

use crate::compile::app_decl::decode_app_decl;
use crate::compile::catalog::{
    build_dataset_catalog_filter, resolve_dataset_catalog_compile_rels, DatasetCatalogFilter,
};
use crate::compile::dependency_graph::DependencyGraph;
use crate::compile::discover_routes::{
    catalog_focus_target, inject_discovered_entry_scene_routes, is_dataset_manage_preview,
    is_manage_preview_only_compile, CompileOptions, CompileRevisionPlan, CompileWatchedFile,
};
use crate::compile::scene::{find_scene_route, resolve_scene_routes};

pub fn resolve_default_scene_from_root(app_root: &Path) -> Result<Option<String>> {
    let app_main = resolve_app_main_path(app_root);
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let route_registry = resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
    Ok(route_registry
        .default_scene_id
        .or_else(|| {
            route_registry
                .routes
                .first()
                .map(|route| route.scene_id.clone())
        })
        .map(|scene_id| scene_id.trim().to_string())
        .filter(|scene_id| !scene_id.is_empty()))
}

pub fn compile_revision_plan_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: &CompileOptions,
) -> Result<CompileRevisionPlan> {
    let app_entry_main = resolve_app_entry_main(app_root);
    let app_main = resolve_app_main_path(app_root);
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
    let asset_map = load_component_assets(source_root)?;
    let preview_only = is_manage_preview_only_compile(options, app_entry_main.as_str());
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
    let dependency_graph_routes =
        scoped_dependency_graph_routes(&route_registry.routes, active_route_meta.as_ref(), options);
    let dependency_graph =
        DependencyGraph::build_cached(app_root, &app_decls, &dependency_graph_routes);

    let selected_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|value| value.to_string());
    let primary_target = selected_target
        .or_else(|| {
            active_route_meta
                .as_ref()
                .map(|route| route.target_file.clone())
        })
        .unwrap_or_else(|| app_entry_main.clone());

    let dataset_manage_preview = is_dataset_manage_preview(options, app_entry_main.as_str());
    let catalog_focus = catalog_focus_target(options, Some(primary_target.as_str()));
    let catalog_filter = if dataset_manage_preview {
        DatasetCatalogFilter::default()
    } else {
        build_dataset_catalog_filter(app_root, &app_decls, &dependency_graph, catalog_focus)
    };
    Ok(build_compile_revision_plan_from_inputs(
        source_root,
        app_root,
        app_entry_main.as_str(),
        &app_decls,
        &dependency_graph,
        primary_target.as_str(),
        dataset_manage_preview,
        &catalog_filter,
    ))
}

pub fn compile_revision_token_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: &CompileOptions,
) -> Result<String> {
    Ok(compile_revision_plan_from_root_with_options(source_root, app_root, options)?.token)
}

pub(crate) fn build_compile_revision_plan_from_inputs(
    source_root: &Path,
    app_root: &Path,
    app_entry_main: &str,
    app_decls: &serde_json::Value,
    dependency_graph: &DependencyGraph,
    primary_target: &str,
    dataset_manage_preview: bool,
    catalog_filter: &DatasetCatalogFilter,
) -> CompileRevisionPlan {
    let mut token_parts = BTreeMap::<String, String>::new();
    let mut watched_paths = BTreeSet::<String>::new();
    watched_paths.insert(app_entry_main.to_string());
    if let Some(main_token) =
        dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, app_entry_main)
    {
        token_parts.insert("main".to_string(), main_token);
        watched_paths.extend(dependency_graph.closure_for_target(
            app_root,
            app_decls,
            app_entry_main,
        ));
    }
    if let Some(primary_token) =
        dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, primary_target)
    {
        token_parts.insert(format!("target:{primary_target}"), primary_token);
        watched_paths.extend(dependency_graph.closure_for_target(
            app_root,
            app_decls,
            primary_target,
        ));
    }
    if !dataset_manage_preview {
        for rel in resolve_dataset_catalog_compile_rels(app_root, catalog_filter)
            .into_iter()
            .filter(|rel| rel != primary_target)
        {
            if let Some(token) =
                dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, &rel)
            {
                token_parts.insert(format!("catalog:{rel}"), token);
                watched_paths
                    .extend(dependency_graph.closure_for_target(app_root, app_decls, &rel));
            }
        }
    }

    let config_path = app_mei_config_path(app_root);
    if config_path.is_file() {
        watched_paths.insert(MEI_CONFIG_FILENAME.to_string());
        if let Ok(config) = MeiConfig::load_from_path(&config_path) {
            token_parts.insert(
                "mei-config".to_string(),
                crate::mei_config::mei_config_compile_revision_digest(&config),
            );
            let themes_rev = crate::mei_config::ops_themes_revision_digest(&config);
            if !themes_rev.is_empty() {
                token_parts.insert("ops-themes".to_string(), themes_rev);
            }
        }
        append_ops_source_revision_tokens(app_root, &mut token_parts, &mut watched_paths);
    }

    let components_revision = crate::compile::scene_payload_cache::components_revision(source_root);
    token_parts.insert("components".to_string(), components_revision.to_string());
    let watched_files = watched_paths
        .into_iter()
        .map(|rel_path| {
            let path = crate::mei_config::resolve_app_mei_file_path(app_root, &rel_path);
            let metadata = std::fs::metadata(&path).ok();
            CompileWatchedFile {
                content_signature: path.is_file().then(|| {
                    crate::compile::source_file_content_signature(path.as_path(), &rel_path)
                }),
                rel_path,
                modified_ms: crate::compile::scene_payload_cache::file_mtime_ms(&path),
                size_bytes: metadata.map(|meta| meta.len()).unwrap_or(0),
            }
        })
        .collect();
    CompileRevisionPlan {
        token: token_parts.into_values().collect::<Vec<_>>().join("||"),
        watched_files,
        components_revision,
    }
}

fn append_ops_source_revision_tokens(
    app_root: &Path,
    token_parts: &mut BTreeMap<String, String>,
    watched_paths: &mut BTreeSet<String>,
) {
    let config_path = app_mei_config_path(app_root);
    let Ok(config) = MeiConfig::load_from_path(&config_path) else {
        return;
    };
    for (source_id, entry) in &config.ops.sources {
        let rel = entry.path.trim().replace('\\', "/");
        if rel.is_empty() {
            continue;
        }
        let resolved = crate::resolve_versioned_source_identifier(app_root, rel.as_str());
        watched_paths.insert(resolved.clone());
        let absolute = crate::resolve_versioned_source_path(app_root, rel.as_str());
        let content_signature =
            crate::compile::source_file_content_signature(absolute.as_path(), resolved.as_str());
        token_parts.insert(
            format!("source:{source_id}"),
            format!("content:{content_signature}"),
        );
    }
}

pub(crate) fn scoped_dependency_graph_routes(
    routes: &[CompiledSceneRoute],
    active_route_meta: Option<&CompiledSceneRoute>,
    options: &CompileOptions,
) -> Vec<CompiledSceneRoute> {
    let explicit_scene_scope = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || options
            .preview_target
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
    if !explicit_scene_scope {
        return routes.to_vec();
    }
    let mut scoped = BTreeMap::<String, CompiledSceneRoute>::new();
    if let Some(route) = active_route_meta.cloned() {
        scoped.insert(route.target_file.clone(), route);
    }
    if let Some(preview_route) = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .and_then(|target| routes.iter().find(|route| route.target_file == target))
        .cloned()
    {
        scoped.insert(preview_route.target_file.clone(), preview_route);
    }
    if scoped.is_empty() {
        return routes.to_vec();
    }
    scoped.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::append_ops_source_revision_tokens;
    use crate::compile::source_file_content_signature;
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;

    fn temp_app_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mei-app-compile-revision-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn ops_sources_revision_token_uses_content_signature() {
        let app_root = temp_app_root("ops-source");
        fs::create_dir_all(app_root.join("upload")).expect("create upload dir");
        fs::write(app_root.join("upload/test.xlsx"), b"hello world").expect("write source");
        fs::write(
            app_root.join(".mei-config.json"),
            serde_json::to_string_pretty(&json!({
                "ops": {
                    "sources": {
                        "ledger": {
                            "kind": "xlsx",
                            "path": "upload/test.xlsx"
                        }
                    }
                }
            }))
            .expect("serialize config"),
        )
        .expect("write config");

        let mut token_parts = BTreeMap::new();
        let mut watched_paths = BTreeSet::new();
        append_ops_source_revision_tokens(app_root.as_path(), &mut token_parts, &mut watched_paths);

        let expected_rel = "upload/test.xlsx";
        let expected_sig =
            source_file_content_signature(&app_root.join(expected_rel), expected_rel);
        let expected_token = format!("content:{expected_sig}");
        assert_eq!(
            token_parts.get("source:ledger").map(String::as_str),
            Some(expected_token.as_str())
        );
        assert!(watched_paths.contains(expected_rel));

        let _ = fs::remove_dir_all(app_root);
    }
}
