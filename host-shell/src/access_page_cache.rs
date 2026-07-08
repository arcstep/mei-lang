use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use mei_lang_app::{HostAccountView, UiRouteMode};
use mei_lang_kernel::{load_mei_config_for_app, resolve_app_root};
use serde::Serialize;
use serde_json::json;

use crate::build_info::host_asset_version_stamp;
use crate::gis_config::GisTilesConfig;
use crate::review_axes::PageRenderAxes;

pub const HOST_SSR_PAYLOAD_REVISION: &str = "host-shell-ssr-v2";
pub const THIN_SHELL_PAGE_CACHE_REVISION: &str = "thin-shell-bundle-v6";

pub fn resolve_scene_client_revision(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Option<String> {
    let bootstrap = mei_host_graph::bootstrap_embed_status(workspace_root, app_id, scene_id);
    if let Some(revision) = bootstrap
        .client_revision
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(revision.to_string());
    }
    if bootstrap.allowed && bootstrap.reason == "no_client_bootstrap_required" {
        return Some(mei_host_graph::NO_CLIENT_BOOTSTRAP_REVISION.to_string());
    }
    None
}

fn hash_signature(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn serialized_signature<T: Serialize + ?Sized>(value: &T) -> u64 {
    serde_json::to_string(value)
        .map(|raw| hash_signature(raw.as_str()))
        .unwrap_or(0)
}

fn ops_themes_revision_digest(workspace_root: &Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    let digest = serialized_signature(&config.ops.themes);
    format!("{digest:016x}")
}

fn ops_layout_tuning_revision_digest(workspace_root: &Path, app_id: &str) -> String {
    // Legacy digest retained for cache key compatibility; prefer ops_themes_revision_digest for new keys.
    let app_root = resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    mei_lang_kernel::ops_layout_tuning_revision_digest(&config.ops)
}

fn layout_policy_revision_digest(workspace_root: &Path, app_id: &str) -> String {
    let theme = ops_themes_revision_digest(workspace_root, app_id);
    let legacy = ops_layout_tuning_revision_digest(workspace_root, app_id);
    format!("{theme}:{legacy}")
}

/// Remove legacy on-disk page-render-cache directories (abolished; one-time hygiene).
pub fn clear_legacy_page_render_cache_for_app(workspace_root: &Path, app_id: &str) -> usize {
    crate::thin_shell_page_cache::clear_for_app(app_id);
    let app_root = resolve_app_root(workspace_root, app_id);
    let disk_dir = mei_lang_kernel::resolve_app_var_root(app_root.as_path()).join("page-render-cache");
    if !disk_dir.is_dir() {
        return 0;
    }
    let cleared = fs::read_dir(&disk_dir)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);
    let _ = fs::remove_dir_all(&disk_dir);
    cleared
}

pub fn unified_view_page_cache_key(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    axes: PageRenderAxes,
    chrome_hidden: bool,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    gis: &GisTilesConfig,
    node: Option<&str>,
    focus: Option<&str>,
    tab: Option<&str>,
) -> Option<String> {
    scene_revision_cache_key_for_route(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        axes,
        chrome_hidden,
        auth_enabled,
        account_view,
        gis,
        node,
        focus,
        tab,
    )
}

fn scene_revision_cache_key_for_route(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    axes: PageRenderAxes,
    chrome_hidden: bool,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    gis: &GisTilesConfig,
    node: Option<&str>,
    focus: Option<&str>,
    tab: Option<&str>,
) -> Option<String> {
    let thin_shell_route = route_mode.is_access_like()
        || matches!(
            route_mode,
            UiRouteMode::Layout | UiRouteMode::Prototype | UiRouteMode::App
        );
    if !thin_shell_route {
        return None;
    }
    let registry = mei_host_graph::McgRegistryWriter::load(workspace_root, app_id);
    let registry_revision = registry.registry_revision.trim();
    if registry_revision.is_empty() {
        return None;
    }
    let client_revision = resolve_scene_client_revision(workspace_root, app_id, scene_id)?;
    let app_root = resolve_app_root(workspace_root, app_id);
    let data_generation = mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id)
        .data_generation;
    let compile_epoch = mei_host_graph::read_client_bootstrap(workspace_root, app_id, scene_id)
        .map(|manifest| manifest.workset_id)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| client_revision.clone());
    let semantic_core = mei_host_graph::build_semantic_cache_core(
        app_id,
        scene_id,
        None,
        registry_revision,
        client_revision.clone(),
        data_generation.clone(),
        compile_epoch,
    );
    let view_axes = mei_host_graph::build_page_render_view_axes(
        route_mode.slug(),
        axes.data_mode.slug(),
        crate::review_axes::ssr_review_projection_for_axes(route_mode, axes).slug(),
        account_view.map(serialized_signature),
        Some(layout_policy_revision_digest(workspace_root, app_id)),
    );
    let extra = json!({
        "semantic_core": semantic_core,
        "view_axes": view_axes,
        "auth_enabled": auth_enabled,
        "chrome_hidden": chrome_hidden,
        "gis_base_url": gis.base_url,
        "gis_json_path": gis.json_path,
        "ops_themes_revision": ops_themes_revision_digest(workspace_root, app_id),
        "host_ssr_payload_revision": HOST_SSR_PAYLOAD_REVISION,
        "thin_shell_page_cache_revision": THIN_SHELL_PAGE_CACHE_REVISION,
        "host_asset_version": host_asset_version_stamp(),
        "route_mode": route_mode.slug(),
        "build_node": node.unwrap_or(""),
        "focus": focus.unwrap_or(""),
        "tab": tab.unwrap_or(""),
    });
    serde_json::to_string(&extra).ok()
}


pub fn clear_legacy_page_render_cache_for_apps(workspace_root: &Path, app_ids: &[String]) -> usize {
    let mut cleared = 0usize;
    for app_id in app_ids {
        cleared += clear_legacy_page_render_cache_for_app(workspace_root, app_id.as_str());
    }
    cleared
}
