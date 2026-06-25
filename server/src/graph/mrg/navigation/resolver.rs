use std::path::Path;
use std::sync::OnceLock;

use mei_lang_app::UiRouteMode;
use tracing::warn;

use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::graph::mrg::navigation::types::{NavigationEntry, NavigationMatch};
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
    registry
        .navigation_entries()
        .into_iter()
        .collect()
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

pub fn match_request_to_navigation(
    source_root: &Path,
    app_id: &str,
    route_mode: UiRouteMode,
    scene_id: Option<&str>,
    query: &AppQuery,
) -> NavigationMatch {
    let mode = UiMode::from_route_mode(route_mode);
    let scene = scene_id
        .map(str::trim)
        .filter(|value: &&str| !value.is_empty())
        .map(|value| value.to_string());
    let target = query
        .file
        .as_deref()
        .filter(|value: &&str| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            query
                .scene
                .as_deref()
                .filter(|value: &&str| !value.trim().is_empty())
                .map(|value| value.trim().to_string())
        });

    if scene.is_none() && target.is_none() {
        return resolve_default_scope(source_root, app_id, mode);
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

    resolve_default_scope(source_root, app_id, mode)
}
