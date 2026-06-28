use std::path::Path;

use mei_lang_kernel::{resolve_default_scene_from_root, WorkspaceAppMeta};

use crate::types::AuthPrincipal;

pub fn filter_apps_for_principal(
    apps: &[WorkspaceAppMeta],
    principal: Option<&AuthPrincipal>,
) -> Vec<WorkspaceAppMeta> {
    apps.iter()
        .filter(|app| {
            principal
                .map(|p| p.can_access_app(app.id.as_str()))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

pub fn access_landing_location(app_id: &str, scene_id: &str) -> String {
    format!("/apps/app/{app_id}/scene/{scene_id}")
}

/// v2 host-shell: always land on access scene (build/manage not supported).
pub fn v2_index_landing_location(
    source_root: &Path,
    app: &WorkspaceAppMeta,
    principal: Option<&AuthPrincipal>,
) -> String {
    let app_root = source_root.join("apps").join(app.id.as_str());
    let scene = resolve_default_scene_from_root(&app_root)
        .ok()
        .flatten()
        .filter(|scene| !scene.trim().is_empty())
        .unwrap_or_else(|| "home".to_string());
    if let Some(p) = principal {
        if !p.can_access_scene(app.id.as_str(), scene.as_str()) {
            return format!("/apps/app/{}", app.id);
        }
    }
    access_landing_location(app.id.as_str(), scene.as_str())
}
