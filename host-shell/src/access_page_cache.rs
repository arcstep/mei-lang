use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use mei_lang_app::{
    page_body_theme_style, render_page, HostAccountView, TopbarMenuContext, UiRouteMode,
};
use mei_lang_kernel::{
    load_mei_config_for_app, load_workspace_config, resolve_app_root, WorkspaceAppMeta,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::build_info::{fill_page_shell_placeholders, host_asset_version_stamp};
use crate::gis_config::GisTilesConfig;
use crate::pages::{
    inject_client_bootstrap_script, inject_layer_plane_scripts, inject_presentation_manifest_script,
    inject_scene_manifest_refs, AppQuery,
};
use crate::review_axes::PageRenderAxes;

pub const HOST_SSR_PAYLOAD_REVISION: &str = "host-shell-ssr-v2";
pub const THIN_SHELL_PAGE_CACHE_REVISION: &str = "thin-shell-bundle-v2";

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
    let app_root = resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    mei_lang_kernel::ops_layout_tuning_revision_digest(&config.ops)
}

/// Remove legacy on-disk page-render-cache directories (abolished; one-time hygiene).
pub fn clear_legacy_page_render_cache_for_app(workspace_root: &Path, app_id: &str) -> usize {
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

pub fn scene_revision_cache_key(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    axes: PageRenderAxes,
    chrome_hidden: bool,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    gis: &GisTilesConfig,
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
        None,
        None,
        None,
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
            UiRouteMode::Layout | UiRouteMode::Prototype
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
        Some(ops_layout_tuning_revision_digest(workspace_root, app_id)),
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

fn inject_scene_revision_meta(html: String, revision: Option<&SceneRevisionPayload>) -> String {
    let Some(revision) = revision else {
        return html;
    };
    let digest = revision.revision_digest.replace('"', "");
    let cache_key = revision
        .cache_key
        .as_deref()
        .unwrap_or("")
        .replace('"', "");
    let mut injection = format!(
        r#"<meta name="mei-scene-revision-digest" content="{digest}" />"#
    );
    if !cache_key.is_empty() {
        injection.push_str(&format!(
            r#"<meta name="mei-scene-cache-key" content="{cache_key}" />"#
        ));
    }
    if let Some(pos) = html.find("<head>") {
        let insert_at = pos + "<head>".len();
        let mut out = String::with_capacity(html.len() + injection.len());
        out.push_str(&html[..insert_at]);
        out.push_str(&injection);
        out.push_str(&html[insert_at..]);
        return out;
    }
    html
}

fn access_route_chrome_hidden(route_mode: UiRouteMode, query: &AppQuery) -> bool {
    route_mode == UiRouteMode::Run
        || route_mode == UiRouteMode::Copilot
        || query
            .chrome
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("none"))
            .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct ResolvedAccessPageHtml {
    pub html: String,
}

pub fn resolve_access_page_html(
    workspace_root: &Path,
    package_root: &Path,
    apps: &[WorkspaceAppMeta],
    topbar_menu: &TopbarMenuContext,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    query: &AppQuery,
    axes: PageRenderAxes,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    copilot_presentation_id: Option<&str>,
) -> anyhow::Result<ResolvedAccessPageHtml> {
    let html = render_access_page_template(
        workspace_root,
        package_root,
        apps,
        topbar_menu,
        app_id,
        scene_id,
        route_mode,
        query,
        axes,
        auth_enabled,
        account_view,
        copilot_presentation_id,
    )?;
    Ok(ResolvedAccessPageHtml { html })
}

pub fn render_access_page_template(
    workspace_root: &Path,
    package_root: &Path,
    apps: &[WorkspaceAppMeta],
    topbar_menu: &TopbarMenuContext,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    query: &AppQuery,
    axes: PageRenderAxes,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    copilot_presentation_id: Option<&str>,
) -> anyhow::Result<String> {
    let outcome = mei_host_graph::assemble_scope_from_registry(workspace_root, app_id, scene_id)?
        .ok_or_else(|| anyhow::anyhow!("scene `{scene_id}` not assembled for app `{app_id}`"))?;
    let workspace = load_workspace_config(workspace_root);
    let theme_style = page_body_theme_style(&workspace, Some(&outcome.compiled), None);
    let app_ctx = mei_host_core::HostContext::new(workspace_root.to_path_buf(), app_id.to_string());
    let gis = GisTilesConfig::resolve_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root),
        None,
    );
    let app_root = resolve_app_root(workspace_root, app_id);
    let scene_bundle_url: Option<String> = if route_mode.is_access_like()
        && crate::scene_bundle::should_build_scene_bundle(app_root.as_path(), route_mode, scene_id)
    {
        let probe = crate::scene_bundle::probe_scene_component_bundle(
            package_root,
            workspace_root,
            app_id,
            scene_id,
            &outcome.compiled.component_assets,
        );
        if let Some(build) = probe.build.as_ref() {
            crate::scene_bundle::schedule_scene_component_bundle_build(
                package_root,
                workspace_root,
                build,
            );
        }
        probe.bundle.map(|bundle| bundle.url)
    } else {
        None
    };
    let selected_scene = if route_mode == UiRouteMode::Copilot {
        copilot_presentation_id.or(Some(scene_id))
    } else {
        Some(scene_id)
    };
    let chrome_hidden = access_route_chrome_hidden(route_mode, query);
    let revision_payload = build_scene_revision_payload(
        workspace_root,
        package_root,
        app_id,
        scene_id,
        route_mode,
        axes,
        chrome_hidden,
        auth_enabled,
        account_view,
        &gis,
        outcome.compiled.component_assets.as_slice(),
    );
    let html = render_page(
        apps,
        &outcome.compiled,
        app_id,
        Some(topbar_menu),
        route_mode,
        Some(outcome.compiled.active_target_file.as_str()),
        None,
        None,
        selected_scene,
        None,
        query.tab.as_deref(),
        None,
        None,
        None,
        query.node.as_deref(),
        None,
        None,
        None,
        None,
        None,
        chrome_hidden,
        false,
        None,
        &[],
        auth_enabled,
        account_view,
        scene_bundle_url.as_deref(),
        theme_style.as_str(),
        None,
        None,
        Some(axes.data_mode.slug()),
        Some(crate::review_axes::ssr_review_projection_for_axes(route_mode, axes).slug()),
        None,
        None,
        None,
    );
    let html = fill_page_shell_placeholders(html, workspace_root);
    let html = inject_scene_revision_meta(html, revision_payload.as_ref());
    let html = inject_client_bootstrap_script(html, workspace_root, app_id, scene_id);
    let html = inject_scene_manifest_refs(html, workspace_root, app_id, scene_id);
    let html = inject_layer_plane_scripts(html, &outcome);
    let presentation_id = if route_mode == UiRouteMode::Copilot {
        copilot_presentation_id
    } else {
        None
    };
    let html = inject_presentation_manifest_script(html, workspace_root, app_id, presentation_id);
    Ok(crate::gis_config::fill_gis_tiles_placeholders(html, &gis))
}

pub fn clear_legacy_page_render_cache_for_apps(workspace_root: &Path, app_ids: &[String]) -> usize {
    let mut cleared = 0usize;
    for app_id in app_ids {
        cleared += clear_legacy_page_render_cache_for_app(workspace_root, app_id.as_str());
    }
    cleared
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneRevisionPayload {
    #[serde(default)]
    pub ready: bool,
    pub app_id: String,
    pub scene_id: String,
    pub route_mode: String,
    pub data_mode: String,
    pub review_projection: String,
    pub registry_revision: String,
    pub client_revision: String,
    pub data_generation: String,
    pub ops_themes_revision: String,
    pub host_ssr_payload_revision: String,
    pub scene_bundle_revision: Option<String>,
    pub scene_bundle_url: Option<String>,
    pub scene_bundle_status: String,
    pub auth_enabled: bool,
    pub auth_sig: u64,
    pub revision_digest: String,
    pub cache_key: Option<String>,
}

pub fn build_scene_revision_payload(
    workspace_root: &Path,
    package_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    axes: PageRenderAxes,
    chrome_hidden: bool,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    gis: &GisTilesConfig,
    component_assets: &[mei_lang_kernel::ComponentAsset],
) -> Option<SceneRevisionPayload> {
    let registry = mei_host_graph::McgRegistryWriter::load(workspace_root, app_id);
    let registry_revision = registry.registry_revision.trim().to_string();
    if registry_revision.is_empty() {
        return None;
    }
    let client_revision =
        resolve_scene_client_revision(workspace_root, app_id, scene_id)?;
    let app_root = resolve_app_root(workspace_root, app_id);
    let data_generation = mei_lang_kernel::load_cache_generation(app_root.as_path(), app_id)
        .data_generation;
    let ops_themes_revision = ops_themes_revision_digest(workspace_root, app_id);
    let auth_sig = account_view.map(serialized_signature).unwrap_or(0);
    let cache_key = scene_revision_cache_key(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        axes,
        chrome_hidden,
        auth_enabled,
        account_view,
        gis,
    );
    let bundle_probe = if route_mode.is_access_like()
        && crate::scene_bundle::should_build_scene_bundle(app_root.as_path(), route_mode, scene_id)
    {
        crate::scene_bundle::probe_scene_component_bundle(
            package_root,
            workspace_root,
            app_id,
            scene_id,
            component_assets,
        )
    } else {
        crate::scene_bundle::SceneBundleProbe {
            bundle: None,
            cache_marker: crate::scene_bundle::scene_bundle_cache_marker(
                app_root.as_path(),
                route_mode,
                scene_id,
            ),
            build: None,
        }
    };
    if let Some(build) = bundle_probe.build.as_ref() {
        crate::scene_bundle::schedule_scene_component_bundle_build(
            package_root,
            workspace_root,
            build,
        );
    }
    let scene_bundle_status = crate::scene_bundle::scene_bundle_status(&bundle_probe).to_string();
    let (scene_bundle_revision, scene_bundle_url) = bundle_probe
        .bundle
        .as_ref()
        .map(|bundle| (Some(bundle.revision.clone()), Some(bundle.url.clone())))
        .unwrap_or((None, None));
    let revision_digest = cache_key
        .as_deref()
        .map(hash_signature)
        .map(|digest| format!("{digest:016x}"))
        .unwrap_or_else(|| "0".to_string());
    Some(SceneRevisionPayload {
        ready: true,
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        route_mode: route_mode.slug().to_string(),
        data_mode: axes.data_mode.slug().to_string(),
        review_projection: axes.review_projection.slug().to_string(),
        registry_revision,
        client_revision,
        data_generation,
        ops_themes_revision,
        host_ssr_payload_revision: HOST_SSR_PAYLOAD_REVISION.to_string(),
        scene_bundle_revision,
        scene_bundle_url,
        scene_bundle_status,
        auth_enabled,
        auth_sig,
        revision_digest,
        cache_key,
    })
}
