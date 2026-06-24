use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use mei_lang_app::{
    HostAccountView, SourcePanelMeta, TopbarMenuContext, UiRouteMode, UploadFileEntry,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone)]
struct CachedPageRenderTemplate {
    expires_at: Instant,
    html: String,
}

const PAGE_RENDER_CACHE_TTL_MS: u64 = 300_000;
const MAX_PAGE_RENDER_CACHE_ENTRIES: usize = 128;
/// SSR `data-props` 策略变更时递增，使旧的大体积 HTML 渲染缓存自动失效。
const HOST_SSR_PAYLOAD_REVISION: &str = "slim-build-v4";

fn page_render_cache() -> &'static Mutex<BTreeMap<String, CachedPageRenderTemplate>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedPageRenderTemplate>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn page_render_cache_ttl() -> Duration {
    Duration::from_millis(PAGE_RENDER_CACHE_TTL_MS)
}

fn take_cached_page_render_template(key: &str) -> Option<String> {
    let Ok(mut cache) = page_render_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.html.clone())
}

fn store_cached_page_render_template(key: String, html: &str) {
    let Ok(mut cache) = page_render_cache().lock() else {
        return;
    };
    cache.retain(|_, entry| entry.expires_at > Instant::now());
    if cache.len() >= MAX_PAGE_RENDER_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(
        key,
        CachedPageRenderTemplate {
            expires_at: Instant::now() + page_render_cache_ttl(),
            html: html.to_string(),
        },
    );
}

pub(crate) fn clear_page_render_cache() -> usize {
    let Ok(mut cache) = page_render_cache().lock() else {
        return 0;
    };
    let cleared = cache.len();
    cache.clear();
    cleared
}

fn hash_signature(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn serialized_signature<T: Serialize + ?Sized>(value: &T) -> u64 {
    serde_json::to_string(value)
        .map(|raw| hash_signature(&raw))
        .unwrap_or(0)
}

pub(super) fn page_render_cache_key(
    app_id: &str,
    route_mode: UiRouteMode,
    compile_revision: &str,
    target: &str,
    source: &str,
    source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    diag_filter: Option<&str>,
    world_metric: Option<&str>,
    world_dataset: Option<&str>,
    explain: Option<&str>,
    node: Option<&str>,
    scope: Option<&str>,
    focus: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    upload_root_label: Option<&str>,
    topbar_menu: Option<&TopbarMenuContext>,
    upload_files: &[UploadFileEntry],
    gis: &crate::gis_config::GisTilesConfig,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    scene_bundle_marker: &str,
    ops_themes_revision: Option<&str>,
) -> Option<String> {
    let compile_revision = compile_revision.trim();
    if compile_revision.is_empty() {
        return None;
    }
    let source_sig = hash_signature(source);
    let source_meta_sig = source_meta.map(serialized_signature).unwrap_or(0);
    let topbar_sig = topbar_menu.map(serialized_signature).unwrap_or(0);
    let upload_sig = serialized_signature(upload_files);
    let auth_sig = auth_account.map(serialized_signature).unwrap_or(0);
    let extra = json!({
        "app_id": app_id,
        "route_mode": route_mode.slug(),
        "compile_revision": compile_revision,
        "target": target,
        "selected_scene": selected_scene.unwrap_or(""),
        "preview_target": preview_target.unwrap_or(""),
        "active_tab": active_tab.unwrap_or(""),
        "diag_filter": diag_filter.unwrap_or(""),
        "world_metric": world_metric.unwrap_or(""),
        "world_dataset": world_dataset.unwrap_or(""),
        "explain": explain.unwrap_or(""),
        "node": node.unwrap_or(""),
        "scope": scope.unwrap_or(""),
        "focus": focus.unwrap_or(""),
        "chrome_hidden": chrome_hidden,
        "upload_enabled": upload_enabled,
        "upload_root_label": upload_root_label.unwrap_or(""),
        "source_sig": source_sig,
        "source_meta_sig": source_meta_sig,
        "topbar_sig": topbar_sig,
        "upload_sig": upload_sig,
        "auth_sig": auth_sig,
        "auth_enabled": auth_enabled,
        "scene_bundle_marker": scene_bundle_marker,
        "ops_themes_revision": ops_themes_revision.unwrap_or(""),
        "host_ssr_payload_revision": HOST_SSR_PAYLOAD_REVISION,
        "gis_base_url": gis.base_url.as_str(),
        "gis_json_path": gis.json_path.as_str(),
    });
    serde_json::to_string(&extra).ok()
}

pub(super) fn render_page_template_with_cache(
    cache_key: Option<String>,
    render: impl FnOnce() -> String,
) -> (String, bool) {
    if let Some(ref key) = cache_key {
        if let Some(html) = take_cached_page_render_template(key) {
            return (html, true);
        }
    }
    let html = render();
    if let Some(key) = cache_key {
        store_cached_page_render_template(key, &html);
    }
    (html, false)
}
