use std::path::Path;
use std::sync::OnceLock;

use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{resolve_app_root, resolve_default_scene_from_root};
use tracing::warn;

use crate::graph::mcg::app_skeleton::load_app_skeleton_artifact;
use crate::graph::mrg::navigation::types::{NavigationEntry, NavigationMatch};
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::http::pages::AppQuery;
use crate::readiness::reachability::legacy_resolve_access_entry;
use crate::readiness::types::{ScopeCoords, UiMode};

pub fn mrg_nav_gate_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_MRG_NAV_GATE")
            .map(|value| {
                let trimmed = value.trim();
                !(trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
            })
            .unwrap_or(true)
    })
}

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

pub fn legacy_fallback_scope(source_root: &Path, app_id: &str, mode: UiMode) -> ScopeCoords {
    let entry = legacy_resolve_access_entry(source_root);
    if entry.app_id == app_id {
        return ScopeCoords::new(app_id, mode, entry.scene_id, entry.target_file);
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
    let key = mode.default_navigation_key();
    if let Some(entry) = find_navigation_by_key(source_root, app_id, key) {
        return NavigationMatch {
            scope: entry.to_scope_coords(app_id, mode),
            entry: Some(entry),
            legacy_fallback: false,
        };
    }
    warn!(
        app_id = %app_id,
        navigation_key = %key,
        "L2: MRG default navigation missing; using legacy scope resolver"
    );
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
    if let Ok(Some(skeleton)) = load_app_skeleton_artifact(app_root.as_path(), None) {
        if let Some(routes) = skeleton
            .payload
            .get("sceneRoutes")
            .and_then(|value| {
                serde_json::from_value::<Vec<mei_lang_kernel::CompiledSceneRoute>>(value.clone()).ok()
            })
        {
            let matches = routes
                .iter()
                .filter(|route| route.target_file == target_file)
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
        return resolve_default_scope(source_root, app_id, mode);
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
        warn!(
            app_id = %app_id,
            scene_id = %scene_id,
            target_file = %target_file,
            "L2: MRG navigation miss for explicit scope; using legacy scope resolver"
        );
        return NavigationMatch {
            scope: ScopeCoords::new(app_id, mode, scene_id, target_file),
            entry: None,
            legacy_fallback: true,
        };
    }

    if mode == UiMode::Build {
        if let Some(target_file) = target {
            warn!(
                app_id = %app_id,
                target_file = %target_file,
                "L2: build file target without MRG navigation; using explicit file scope"
            );
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

    resolve_default_scope(source_root, app_id, mode)
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
        fs::create_dir_all(ws.join("apps/demo/.mei/graph/payloads")).expect("mkdir skel");
        fs::write(
            ws.join("apps/demo/.mei/graph/payloads/app-skeleton.json"),
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
        };
        let nav = match_request_to_navigation(ws, "demo", UiRouteMode::Build, None, &query);
        assert!(!nav.legacy_fallback);
        assert_eq!(nav.scope.target_file, "main.mei");
        assert_eq!(nav.scope.scene_id, "home");
    }
}
