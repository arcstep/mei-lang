use std::path::Path;

use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{
    is_stock_catalog_app_for_root, resolve_app_root, resolve_default_scene_from_root,
};
use tracing::warn;

use crate::graph::mrg::navigation::NavigationResolveOpts;

use crate::graph::mcg::app_skeleton::load_app_skeleton_from_mcg;
use crate::graph::mrg::navigation::types::{NavigationEntry, NavigationMatch};
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::http::pages::AppQuery;
use crate::readiness::reachability::legacy_resolve_access_entry;
use crate::readiness::types::{ScopeCoords, UiMode};

pub fn list_navigation_entries(source_root: &Path, app_id: &str) -> Vec<NavigationEntry> {
    let registry = MrgRegistryWriter::load(source_root, app_id);
    registry.navigation_entries().into_iter().collect()
}

pub fn find_navigation_by_key(
    source_root: &Path,
    app_id: &str,
    key: &str,
) -> Option<NavigationEntry> {
    let registry = MrgRegistryWriter::load(source_root, app_id);
    registry.navigation_by_key(key)
}

fn default_scene_navigation_alias(
    source_root: &Path,
    app_id: &str,
    mode: UiMode,
) -> Option<NavigationEntry> {
    use mei_lang_kernel::{load_workspace_config, resolve_default_scene_from_root};
    let cfg = load_workspace_config(source_root);
    let app_root = resolve_app_root(source_root, app_id);
    let default_scene = resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .map(|scene| scene.trim().to_string())
        .filter(|scene| !scene.is_empty())
        .or_else(|| {
            cfg.deploy
                .access_entry
                .default_scene
                .as_deref()
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            list_navigation_entries(source_root, app_id)
                .into_iter()
                .find(|nav| nav.key.starts_with("build:") && nav.scene_id != "home")
                .map(|nav| nav.scene_id.clone())
        })
        .unwrap_or_else(|| "home".to_string());
    find_navigation_by_key(
        source_root,
        app_id,
        mode.scene_navigation_key(default_scene.as_str()).as_str(),
    )
}

fn default_build_entry_is_stale(source_root: &Path, app_id: &str, entry: &NavigationEntry) -> bool {
    if entry.scene_id != "home"
        || !matches!(
            entry.target_file.as_str(),
            "scenes/home.mei" | "src/scenes/home.mei"
        )
    {
        return false;
    }
    if find_navigation_by_key(source_root, app_id, "build:home").is_some() {
        return false;
    }
    let app_root = resolve_app_root(source_root, app_id);
    if let Ok(Some(default_scene)) = resolve_default_scene_from_root(app_root.as_path()) {
        let default_scene = default_scene.trim();
        if !default_scene.is_empty() && default_scene != "home" {
            return true;
        }
    }
    list_navigation_entries(source_root, app_id)
        .into_iter()
        .any(|nav| nav.key.starts_with("build:") && nav.scene_id != "home")
}

pub fn legacy_fallback_scope(source_root: &Path, app_id: &str, mode: UiMode) -> ScopeCoords {
    let entry = legacy_resolve_access_entry(source_root);
    if entry.app_id == app_id {
        return ScopeCoords::new(app_id, mode, entry.scene_id, entry.target_file);
    }
    let app_root = resolve_app_root(source_root, app_id);
    if let Ok(Some(default_scene)) = resolve_default_scene_from_root(app_root.as_path()) {
        if let Ok(Some(skeleton)) = load_app_skeleton_from_mcg(source_root, app_id) {
            if let Some(routes) = skeleton
                .payload
                .get("sceneRoutes")
                .and_then(|value| {
                    serde_json::from_value::<Vec<mei_lang_kernel::CompiledSceneRoute>>(
                        value.clone(),
                    )
                    .ok()
                })
            {
                if let Some(route) = routes
                    .iter()
                    .find(|route| route.scene_id == default_scene)
                    .or_else(|| routes.iter().find(|route| route.is_default))
                    .or_else(|| routes.first())
                {
                    return ScopeCoords::new(
                        app_id,
                        mode,
                        route.scene_id.clone(),
                        route.target_file.clone(),
                    );
                }
            }
        }
        if let Ok(routes) = mei_lang_kernel::collect_stock_catalog_routes(source_root) {
            if let Some(route) = routes
                .iter()
                .find(|route| route.route_id == default_scene)
                .or_else(|| routes.first())
            {
                return ScopeCoords::new(
                    app_id,
                    mode,
                    route.route_id.clone(),
                    route.target_rel.clone(),
                );
            }
        }
    }
    ScopeCoords::new(
        app_id,
        mode,
        "home",
        "scenes/home.mei",
    )
}

pub fn resolve_default_scope(
    source_root: &Path,
    app_id: &str,
    mode: UiMode,
) -> NavigationMatch {
    resolve_default_scope_with_opts(source_root, app_id, mode, NavigationResolveOpts::default())
}

pub fn resolve_default_scope_with_opts(
    source_root: &Path,
    app_id: &str,
    mode: UiMode,
    opts: NavigationResolveOpts,
) -> NavigationMatch {
    let key = mode.default_navigation_key();
    if let Some(entry) = find_navigation_by_key(source_root, app_id, key) {
        if !default_build_entry_is_stale(source_root, app_id, &entry) {
            return NavigationMatch {
                scope: entry.to_scope_coords(app_id, mode),
                entry: Some(entry),
                legacy_fallback: false,
            };
        }
    }
    if let Some(entry) = default_scene_navigation_alias(source_root, app_id, mode) {
        return NavigationMatch {
            scope: entry.to_scope_coords(app_id, mode),
            entry: Some(entry),
            legacy_fallback: false,
        };
    }
    if !opts.silent {
        warn!(
            app_id = %app_id,
            navigation_key = %key,
            "L2: MRG default navigation missing; using legacy scope resolver"
        );
    } else {
        tracing::debug!(
            app_id = %app_id,
            navigation_key = %key,
            "L2: MRG default navigation missing; using legacy scope resolver"
        );
    }
    NavigationMatch {
        scope: legacy_fallback_scope(source_root, app_id, mode),
        entry: None,
        legacy_fallback: true,
    }
}

/// Infer scene id for a build `file=` target (MRG nav key lookup only).
pub fn infer_build_scene_for_target(
    source_root: &Path,
    app_id: &str,
    target_file: &str,
) -> Option<String> {
    let target_file = target_file.trim();
    if target_file.is_empty() {
        return None;
    }
    let app_root = resolve_app_root(source_root, app_id);
        if let Ok(Some(skeleton)) = load_app_skeleton_from_mcg(source_root, app_id) {
        if let Some(routes) = skeleton
            .payload
            .get("sceneRoutes")
            .and_then(|value| {
                serde_json::from_value::<Vec<mei_lang_kernel::CompiledSceneRoute>>(value.clone()).ok()
            })
        {
            let canonical_target = mei_lang_kernel::canonical_app_source_rel_path(target_file);
            let matches = routes
                .iter()
                .filter(|route| {
                    route.target_file == canonical_target
                        || route.target_file == target_file
                        || mei_lang_kernel::canonical_app_source_rel_path(route.target_file.as_str())
                            == canonical_target
                })
                .map(|route| route.scene_id.clone())
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                return Some(matches[0].clone());
            }
            if target_file == "main.mei" || target_file.ends_with("/main.mei") {
                if let Some(route) = routes.iter().find(|route| route.is_default) {
                    return Some(route.scene_id.clone());
                }
            }
        }
    }
    if target_file == "main.mei" || target_file.ends_with("/main.mei") {
        return resolve_default_scene_from_root(app_root.as_path())
            .ok()
            .flatten()
            .filter(|scene| !scene.trim().is_empty());
    }
    if is_stock_catalog_app_for_root(source_root, app_id) {
        let catalog_routes = mei_lang_kernel::catalog_scene_routes_from_app_root(app_root.as_path());
        let canonical_target = mei_lang_kernel::canonical_app_source_rel_path(target_file);
        let matches = catalog_routes
            .iter()
            .filter(|route| {
                route.target_file == canonical_target
                    || route.target_file == target_file
                    || mei_lang_kernel::canonical_app_source_rel_path(route.target_file.as_str())
                        == canonical_target
            })
            .map(|route| route.scene_id.clone())
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            return Some(matches[0].clone());
        }
    }
    resolve_default_scene_from_root(app_root.as_path())
        .ok()
        .flatten()
        .filter(|scene| !scene.trim().is_empty())
}

pub fn match_request_to_navigation(
    source_root: &Path,
    app_id: &str,
    route_mode: UiRouteMode,
    scene_id: Option<&str>,
    query: &AppQuery,
) -> NavigationMatch {
    match_request_to_navigation_with_opts(
        source_root,
        app_id,
        route_mode,
        scene_id,
        query,
        NavigationResolveOpts::default(),
    )
}

pub fn match_request_to_navigation_with_opts(
    source_root: &Path,
    app_id: &str,
    route_mode: UiRouteMode,
    scene_id: Option<&str>,
    query: &AppQuery,
    opts: NavigationResolveOpts,
) -> NavigationMatch {
    let mode = UiMode::from_route_mode(route_mode);
    let mut scene = scene_id
        .map(str::trim)
        .filter(|value: &&str| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| {
            query
                .scene
                .as_deref()
                .map(str::trim)
                .filter(|value: &&str| !value.is_empty())
                .map(|value| value.to_string())
        });
    let target = query
        .file
        .as_deref()
        .filter(|value: &&str| !value.trim().is_empty())
        .map(|value| value.trim().to_string());

    if scene.is_none() && target.is_none() {
        return resolve_default_scope_with_opts(source_root, app_id, mode, opts);
    }

    if scene.is_none() {
        if mode == UiMode::Build {
            if let Some(ref target_file) = target {
                if let Some(inferred_scene) =
                    infer_build_scene_for_target(source_root, app_id, target_file.as_str())
                {
                    scene = Some(inferred_scene);
                }
            }
        }
    }

    if let Some(scene_id) = scene.as_deref() {
        let key = mode.scene_navigation_key(scene_id);
        if let Some(entry) = find_navigation_by_key(source_root, app_id, key.as_str()) {
            let mut scope = entry.to_scope_coords(app_id, mode);
            if let Some(target_file) = target {
                scope.target_file = target_file;
            }
            return NavigationMatch {
                scope,
                entry: Some(entry),
                legacy_fallback: false,
            };
        }
    }

    if let (Some(scene_id), Some(target_file)) = (scene.as_deref(), target.as_deref()) {
        let stock_preview = target_file.replace('\\', "/").contains("/stock/");
        if stock_preview {
            tracing::debug!(
                app_id = %app_id,
                scene_id = %scene_id,
                target_file = %target_file,
                "L2: MRG navigation miss for stock preview scope; using legacy scope resolver"
            );
        } else if opts.silent {
            tracing::debug!(
                app_id = %app_id,
                scene_id = %scene_id,
                target_file = %target_file,
                "L2: MRG navigation miss for explicit scope; using legacy scope resolver"
            );
        } else {
            warn!(
                app_id = %app_id,
                scene_id = %scene_id,
                target_file = %target_file,
                "L2: MRG navigation miss for explicit scope; using legacy scope resolver"
            );
        }
        return NavigationMatch {
            scope: ScopeCoords::new(app_id, mode, scene_id, target_file),
            entry: None,
            legacy_fallback: true,
        };
    }

    if mode == UiMode::Build {
        if let Some(target_file) = target {
            if !opts.silent {
                warn!(
                    app_id = %app_id,
                    target_file = %target_file,
                    "L2: build file target without MRG navigation; using explicit file scope"
                );
            } else {
                tracing::debug!(
                    app_id = %app_id,
                    target_file = %target_file,
                    "L2: build file target without MRG navigation; using explicit file scope"
                );
            }
            return NavigationMatch {
                scope: ScopeCoords::new(
                    app_id,
                    mode,
                    scene.unwrap_or_default(),
                    target_file,
                ),
                entry: None,
                legacy_fallback: true,
            };
        }
    }

    resolve_default_scope_with_opts(source_root, app_id, mode, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn build_file_main_mei_uses_scene_navigation_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/demo")).expect("mkdir");
        fs::write(
            ws.join("runtime/platform/graphs/demo/mrg-registry.json"),
            r#"{"schemaVersion":"mei-mrg-registry-v1","appId":"demo","registryRevision":"x","updatedAtMs":0,"nodes":[{"id":{"kind":"navigation","key":"build:home"},"url":"/apps/build/demo?scene=home&file=scenes/home.mei","sceneId":"home","targetFile":"scenes/home.mei","state":"ready"}],"slots":[],"edges":[]}"#,
        )
        .expect("write mrg");
        fs::create_dir_all(ws.join("apps/demo/src")).expect("mkdir app");
        fs::create_dir_all(ws.join("apps/demo/build/active/graph/payloads")).expect("mkdir skel");
        fs::write(
            ws.join("apps/demo/build/active/graph/payloads/app-skeleton.json"),
            r#"{"schemaVersion":"mei-app-skeleton-artifact-v1","revision":"sk:t","payload":{"sceneRoutes":[{"scene_id":"home","target_file":"scenes/home.mei","kind":"scene","access_export":true,"is_default":true}]}}"#,
        )
        .expect("write skeleton");
        fs::write(
            ws.join("apps/demo/src/main.mei"),
            r#"app(id=demo, default_scene=home, scene=scene_ref(scene_file="scenes/home.mei"))"#,
        )
        .expect("write main");
        let query = AppQuery {
            file: Some("main.mei".to_string()),
            scene: None,
            tab: None,
            diag_filter: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            node: None,
            scope: None,
            focus: None,
            chrome: None,
            catalog: None,
            pack: None,
        data_mode: None,
        review_projection: None,
        };
        let nav = match_request_to_navigation(ws, "demo", UiRouteMode::Build, None, &query);
        assert!(!nav.legacy_fallback);
        assert_eq!(nav.scope.target_file, "main.mei");
        assert_eq!(nav.scope.scene_id, "home");
    }

    #[test]
    fn default_build_aliases_to_scene_navigation_when_default_key_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/demo")).expect("mkdir");
        fs::write(
            ws.join("runtime/platform/graphs/demo/mrg-registry.json"),
            r#"{"schemaVersion":"mei-mrg-registry-v1","appId":"demo","registryRevision":"x","updatedAtMs":0,"nodes":[{"id":{"kind":"navigation","key":"build:home"},"url":"/apps/build/demo?scene=home&file=scenes/home.mei","sceneId":"home","targetFile":"scenes/home.mei","state":"ready"}],"slots":[],"edges":[]}"#,
        )
        .expect("write mrg");
        let nav = resolve_default_scope(ws, "demo", UiMode::Build);
        assert!(!nav.legacy_fallback);
        assert_eq!(nav.scope.scene_id, "home");
        assert_eq!(nav.scope.target_file, "scenes/home.mei");
    }

    #[test]
    fn stale_default_build_falls_back_to_main_mei_default_scene() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("runtime/platform/graphs/catalog")).expect("mkdir");
        fs::create_dir_all(ws.join("apps/catalog/src")).expect("mkdir app");
        fs::write(
            ws.join("apps/catalog/src/main.mei"),
            r#"app(id=catalog, default_scene=analytics-drilldown-board)
app_add_scene(scene=scene_ref(id="analytics-drilldown-board", scene_file="../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"))"#,
        )
        .expect("write main");
        fs::write(
            ws.join("runtime/platform/graphs/catalog/mrg-registry.json"),
            r#"{"schemaVersion":"mei-mrg-registry-v1","appId":"catalog","registryRevision":"x","updatedAtMs":0,"nodes":[
                {"id":{"kind":"navigation","key":"default_build"},"url":"/apps/build/catalog","sceneId":"home","targetFile":"scenes/home.mei","state":"ready"},
                {"id":{"kind":"navigation","key":"build:analytics-drilldown-board"},"url":"/apps/build/catalog?scene=analytics-drilldown-board&file=../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei","sceneId":"analytics-drilldown-board","targetFile":"../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei","state":"ready"}
            ],"slots":[],"edges":[]}"#,
        )
        .expect("write mrg");
        let nav = resolve_default_scope(ws, "catalog", UiMode::Build);
        assert!(!nav.legacy_fallback);
        assert_eq!(nav.scope.scene_id, "analytics-drilldown-board");
        assert_eq!(
            nav.scope.target_file,
            "../../stock/templates/cockpit/drilldown/analytics-drilldown-board.mei"
        );
    }
}
