//! Build Admin Nav chips for Host chrome (0544 §4.3).

use mei_host_auth::AuthPrincipal;
use mei_lang_app::AdminNavItem;
use std::path::Path;

use crate::admin_registry::AdminRegistry;

fn principal_has_cap(principal: Option<&AuthPrincipal>, cap: &str) -> bool {
    let Some(p) = principal else {
        return matches!(cap, "config_upload" | "access_view");
    };
    let caps = p.capabilities();
    match cap {
        "config_upload" => caps.config_upload,
        "build_view" => caps.build_view,
        "access_view" => caps.access_view,
        _ => false,
    }
}

/// Refresh registry for `app_id` then project capability-filtered nav chips (0544 §4.3).
pub fn admin_nav_items_for_app(
    registry: &AdminRegistry,
    workspace_root: &Path,
    app_id: &str,
    principal: Option<&AuthPrincipal>,
) -> Vec<AdminNavItem> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Vec::new();
    }
    registry.refresh_app(workspace_root, app_id);
    let caps = |cap: &str| principal_has_cap(principal, cap);
    registry
        .nav_items_for_capabilities(app_id, &caps)
        .into_iter()
        .map(|r| AdminNavItem {
            id: format!(
                "{}.{}",
                r.registry_entry.resource_id, r.registry_entry.module_id
            ),
            label: r.registry_entry.title.clone(),
            href: r.registry_entry.canonical_route.clone(),
        })
        .collect()
}
