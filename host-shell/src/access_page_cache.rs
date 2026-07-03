use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Context;
use mei_lang_app::{
    load_topbar_menu_context, page_body_theme_style, render_page, HostAccountView,
    TopbarMenuContext, UiRouteMode,
};
use mei_lang_kernel::{
    load_mei_config_for_app, load_workspace_config, resolve_app_root, WorkspaceAppMeta,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::build_info::fill_page_shell_placeholders;
use crate::gis_config::GisTilesConfig;
use crate::pages::{
    inject_client_bootstrap_script, inject_layer_plane_scripts, inject_presentation_manifest_script,
    AppQuery,
};

const HOST_SSR_PAYLOAD_REVISION: &str = "host-shell-ssr-v2";
const PAGE_RENDER_CACHE_TTL_MS: u64 = 300_000;
const MAX_PAGE_RENDER_CACHE_ENTRIES: usize = 64;

fn resolve_scene_client_revision(
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

#[derive(Debug, Clone)]
struct CachedPageTemplate {
    expires_at: Instant,
    html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskPageTemplateMeta {
    cache_key: String,
    html_bytes: usize,
    written_at_ms: u64,
}

fn memory_cache() -> &'static Mutex<BTreeMap<String, CachedPageTemplate>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedPageTemplate>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn page_render_cache_ttl() -> Duration {
    Duration::from_millis(PAGE_RENDER_CACHE_TTL_MS)
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

fn page_cache_disk_dir(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root).join("page-render-cache")
}

fn page_cache_disk_html_path(app_root: &Path, scene_id: &str) -> PathBuf {
    page_cache_disk_dir(app_root).join(format!("{scene_id}.html"))
}

fn page_cache_disk_meta_path(app_root: &Path, scene_id: &str) -> PathBuf {
    page_cache_disk_dir(app_root).join(format!("{scene_id}.meta.json"))
}

pub fn clear_access_page_render_cache_for_app(workspace_root: &Path, app_id: &str) -> usize {
    let mut cleared = 0usize;
    if let Ok(mut cache) = memory_cache().lock() {
        let prefix = format!("{app_id}:");
        let keys: Vec<String> = cache
            .keys()
            .filter(|key| key.starts_with(prefix.as_str()))
            .cloned()
            .collect();
        cleared += keys.len();
        for key in keys {
            cache.remove(key.as_str());
        }
    }
    let app_root = resolve_app_root(workspace_root, app_id);
    let disk_dir = page_cache_disk_dir(app_root.as_path());
    if disk_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&disk_dir) {
            cleared += entries.flatten().count();
        }
        let _ = fs::remove_dir_all(&disk_dir);
    }
    cleared
}

pub fn access_page_cache_key(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    gis: &GisTilesConfig,
) -> Option<String> {
    if !route_mode.is_access_like() {
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
    let extra = json!({
        "app_id": app_id,
        "scene_id": scene_id,
        "route_mode": route_mode.slug(),
        "registry_revision": registry_revision,
        "client_revision": client_revision,
        "data_generation": data_generation,
        "auth_enabled": auth_enabled,
        "auth_sig": account_view.map(serialized_signature).unwrap_or(0),
        "gis_base_url": gis.base_url,
        "gis_json_path": gis.json_path,
        "ops_themes_revision": ops_themes_revision_digest(workspace_root, app_id),
        "host_ssr_payload_revision": HOST_SSR_PAYLOAD_REVISION,
    });
    serde_json::to_string(&extra).ok()
}

fn take_memory_template(key: &str) -> Option<String> {
    let Ok(mut cache) = memory_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.html.clone())
}

fn store_memory_template(key: String, html: &str) {
    let Ok(mut cache) = memory_cache().lock() else {
        return;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    if cache.len() >= MAX_PAGE_RENDER_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(
        key,
        CachedPageTemplate {
            expires_at: now + page_render_cache_ttl(),
            html: html.to_string(),
        },
    );
}

fn try_load_disk_template(
    app_root: &Path,
    scene_id: &str,
    expected_key: &str,
) -> Option<String> {
    let meta_path = page_cache_disk_meta_path(app_root, scene_id);
    let html_path = page_cache_disk_html_path(app_root, scene_id);
    let raw = fs::read_to_string(&meta_path).ok()?;
    let meta: DiskPageTemplateMeta = serde_json::from_str(raw.as_str()).ok()?;
    if meta.cache_key != expected_key {
        return None;
    }
    fs::read_to_string(&html_path).ok()
}

fn persist_disk_template(
    app_root: &Path,
    scene_id: &str,
    cache_key: &str,
    html: &str,
) -> anyhow::Result<()> {
    let dir = page_cache_disk_dir(app_root);
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let meta = DiskPageTemplateMeta {
        cache_key: cache_key.to_string(),
        html_bytes: html.len(),
        written_at_ms: crate::state::current_time_ms(),
    };
    fs::write(
        page_cache_disk_meta_path(app_root, scene_id),
        serde_json::to_string_pretty(&meta)?,
    )?;
    fs::write(page_cache_disk_html_path(app_root, scene_id), html)?;
    Ok(())
}

pub fn take_access_page_template(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    cache_key: &str,
) -> Option<String> {
    if let Some(html) = take_memory_template(cache_key) {
        return Some(html);
    }
    let app_root = resolve_app_root(workspace_root, app_id);
    let html = try_load_disk_template(app_root.as_path(), scene_id, cache_key)?;
    store_memory_template(cache_key.to_string(), html.as_str());
    Some(html)
}

pub fn store_access_page_template(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    cache_key: &str,
    html: &str,
) -> anyhow::Result<()> {
    store_memory_template(cache_key.to_string(), html);
    let app_root = resolve_app_root(workspace_root, app_id);
    persist_disk_template(app_root.as_path(), scene_id, cache_key, html)
}

#[derive(Debug, Clone)]
pub struct ResolvedAccessPageHtml {
    pub html: String,
    pub page_render_cache_hit: bool,
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
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    copilot_presentation_id: Option<&str>,
) -> anyhow::Result<ResolvedAccessPageHtml> {
    let app_ctx = mei_host_core::HostContext::new(workspace_root.to_path_buf(), app_id.to_string());
    let gis = GisTilesConfig::resolve_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root),
        None,
    );
    let cache_key = access_page_cache_key(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        auth_enabled,
        account_view,
        &gis,
    );
    let mut page_render_cache_hit = false;
    let html = if let Some(ref key) = cache_key {
        if let Some(cached) = take_access_page_template(
            workspace_root,
            app_id,
            scene_id,
            key.as_str(),
        ) {
            page_render_cache_hit = true;
            cached
        } else {
            let template = render_access_page_template(
                workspace_root,
                package_root,
                apps,
                topbar_menu,
                app_id,
                scene_id,
                route_mode,
                query,
                auth_enabled,
                account_view,
                copilot_presentation_id,
            )?;
            let _ = store_access_page_template(
                workspace_root,
                app_id,
                scene_id,
                key.as_str(),
                template.as_str(),
            );
            template
        }
    } else {
        render_access_page_template(
            workspace_root,
            package_root,
            apps,
            topbar_menu,
            app_id,
            scene_id,
            route_mode,
            query,
            auth_enabled,
            account_view,
            copilot_presentation_id,
        )?
    };
    Ok(ResolvedAccessPageHtml {
        html,
        page_render_cache_hit,
    })
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
        false,
        false,
        None,
        &[],
        auth_enabled,
        account_view,
        scene_bundle_url.as_deref(),
        theme_style.as_str(),
        None,
        None,
    );
    let html = fill_page_shell_placeholders(html, workspace_root);
    let html = inject_client_bootstrap_script(html, workspace_root, app_id, scene_id);
    let html = inject_layer_plane_scripts(html, &outcome);
    let presentation_id = if route_mode == UiRouteMode::Copilot {
        copilot_presentation_id
    } else {
        None
    };
    let html = inject_presentation_manifest_script(html, workspace_root, app_id, presentation_id);
    Ok(crate::gis_config::fill_gis_tiles_placeholders(html, &gis))
}

pub fn hot_scenes_for_app(workspace_root: &Path, app_id: &str) -> Vec<String> {
    let config = load_workspace_config(workspace_root);
    if let Some(app_cfg) = config.warmup.apps.get(app_id) {
        let scenes: Vec<String> = app_cfg
            .hot_scenes
            .iter()
            .map(|scene| scene.trim().to_string())
            .filter(|scene| !scene.is_empty())
            .collect();
        if !scenes.is_empty() {
            return scenes;
        }
    }
    vec!["home".to_string()]
}

pub fn prime_access_page_render_cache(
    workspace_root: &Path,
    package_root: &Path,
    app_id: &str,
    scene_id: &str,
    auth_enabled: bool,
) -> anyhow::Result<bool> {
    let route_mode = UiRouteMode::App;
    let app_ctx = mei_host_core::HostContext::new(workspace_root.to_path_buf(), app_id.to_string());
    let gis = GisTilesConfig::resolve_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root),
        None,
    );
    let cache_key = access_page_cache_key(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        auth_enabled,
        None,
        &gis,
    )
    .ok_or_else(|| anyhow::anyhow!("page cache key unavailable for {app_id}/{scene_id}"))?;
    let discovered = crate::landing::discover_workspace_apps(workspace_root)?;
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = crate::landing::enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    let template = render_access_page_template(
        workspace_root,
        package_root,
        apps.as_slice(),
        &topbar_menu,
        app_id,
        scene_id,
        route_mode,
        &AppQuery::default(),
        auth_enabled,
        None,
        None,
    )?;
    store_access_page_template(workspace_root, app_id, scene_id, cache_key.as_str(), template.as_str())?;
    Ok(true)
}

pub fn warm_access_page_render_caches(
    workspace_root: &Path,
    package_root: &Path,
    app_ids: &[String],
    auth_enabled: bool,
) -> usize {
    let mut warmed = 0usize;
    for app_id in app_ids {
        for scene_id in hot_scenes_for_app(workspace_root, app_id.as_str()) {
            match prime_access_page_render_cache(
                workspace_root,
                package_root,
                app_id.as_str(),
                scene_id.as_str(),
                auth_enabled,
            ) {
                Ok(true) => warmed += 1,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(
                        app_id = %app_id,
                        scene_id = %scene_id,
                        detail = %error,
                        "access page render cache prime skipped"
                    );
                }
            }
        }
    }
    warmed
}

pub fn insert_page_render_cache_hit_header(response: &mut axum::response::Response, cache_hit: bool) {
    if let Ok(value) = axum::http::HeaderValue::from_str(if cache_hit { "1" } else { "0" }) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static("x-mei-page-render-cache-hit"),
            value,
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneRevisionPayload {
    #[serde(default)]
    pub ready: bool,
    pub app_id: String,
    pub scene_id: String,
    pub route_mode: String,
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
    let cache_key = access_page_cache_key(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
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
