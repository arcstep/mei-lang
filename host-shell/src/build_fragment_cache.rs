//! Build workspace-fragment cache (route-specific; shares semantic core with access-like pages).

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mei_lang_kernel::{
    load_mei_config_for_app, ops_layout_tuning_revision_digest, resolve_app_root,
    BuildCompileCoordinate, BuildNodeId, BuildNodeKind,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::access_page_cache::{resolve_scene_client_revision, HOST_SSR_PAYLOAD_REVISION};

const BUILD_FRAGMENT_CACHE_TTL_MS: u64 = 300_000;
const MAX_BUILD_FRAGMENT_CACHE_ENTRIES: usize = 128;

#[derive(Debug, Clone)]
pub struct CachedBuildFragment {
    pub expires_at: Instant,
    pub preview_html: String,
    pub drilldown_script: String,
    pub workspace_scripts: Vec<String>,
    pub node: String,
    pub focus: String,
    pub compile_revision: String,
    pub compile_coordinate: BuildCompileCoordinate,
    pub data_mode: String,
    pub review_projection: String,
    pub revision_digest: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildFragmentRevisionPayload {
    pub ready: bool,
    pub app_id: String,
    pub node: String,
    pub scene_id: String,
    pub route_mode: String,
    pub data_mode: String,
    pub review_projection: String,
    pub focus: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_scope: Option<String>,
    pub registry_revision: String,
    pub client_revision: String,
    pub data_generation: String,
    pub compile_epoch: String,
    pub ops_layout_tuning_revision: String,
    pub draft_session: String,
    pub draft_digest: String,
    pub host_ssr_payload_revision: String,
    pub revision_digest: String,
    pub cache_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_coordinate: Option<BuildCompileCoordinate>,
}

#[derive(Debug, Clone)]
pub struct BuildFragmentCacheInput<'a> {
    pub workspace_root: &'a Path,
    pub app_id: &'a str,
    pub node: &'a str,
    pub scene_id: &'a str,
    pub focus: &'a str,
    pub scope: &'a str,
    pub preview_scope: Option<&'a str>,
    pub data_mode: &'a str,
    pub review_projection: &'a str,
    pub compile_coordinate: Option<&'a BuildCompileCoordinate>,
    pub draft_session: &'a str,
    pub draft_digest: &'a str,
}

fn memory_cache() -> &'static Mutex<BTreeMap<String, CachedBuildFragment>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedBuildFragment>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn cache_ttl() -> Duration {
    Duration::from_millis(BUILD_FRAGMENT_CACHE_TTL_MS)
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

pub fn scene_id_from_build_node(node_raw: &str) -> String {
    let Some(parsed) = BuildNodeId::parse(node_raw) else {
        return String::new();
    };
    match parsed.kind {
        BuildNodeKind::Scene | BuildNodeKind::Route => parsed.key,
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock | BuildNodeKind::Projection => parsed
            .key
            .split('/')
            .next()
            .unwrap_or(parsed.key.as_str())
            .to_string(),
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => parsed
            .key
            .split('#')
            .nth(1)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

pub fn draft_digest_for_tuning(tuning: Option<&Value>) -> String {
    tuning
        .filter(|value| !value.is_null())
        .map(serialized_signature)
        .map(|digest| format!("{digest:016x}"))
        .unwrap_or_default()
}

fn resolve_build_client_revision(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> String {
    if scene_id.trim().is_empty() {
        return mei_host_graph::NO_CLIENT_BOOTSTRAP_REVISION.to_string();
    }
    resolve_scene_client_revision(workspace_root, app_id, scene_id)
        .unwrap_or_else(|| mei_host_graph::NO_CLIENT_BOOTSTRAP_REVISION.to_string())
}

fn resolve_build_compile_epoch(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
    client_revision: &str,
) -> String {
    if scene_id.trim().is_empty() {
        return client_revision.to_string();
    }
    mei_host_graph::read_client_bootstrap(workspace_root, app_id, scene_id)
        .map(|manifest| manifest.workset_id)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| client_revision.to_string())
}

fn ops_layout_tuning_revision(workspace_root: &Path, app_id: &str) -> String {
    let app_root = resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
    ops_layout_tuning_revision_digest(&config.ops)
}

fn overlay_revision(
    persisted: &str,
    draft_session: &str,
    draft_digest: &str,
) -> String {
    if draft_digest.trim().is_empty() {
        return persisted.to_string();
    }
    format!(
        "{}+draft:{}:{}",
        persisted,
        draft_session.trim(),
        draft_digest.trim()
    )
}

pub fn build_fragment_cache_key(input: &BuildFragmentCacheInput<'_>) -> String {
    let registry = mei_host_graph::McgRegistryWriter::load(input.workspace_root, input.app_id);
    let registry_revision = registry.registry_revision.trim().to_string();
    let client_revision =
        resolve_build_client_revision(input.workspace_root, input.app_id, input.scene_id);
    let app_root = resolve_app_root(input.workspace_root, input.app_id);
    let data_generation = mei_lang_kernel::load_cache_generation(app_root.as_path(), input.app_id)
        .data_generation;
    let compile_epoch = resolve_build_compile_epoch(
        input.workspace_root,
        input.app_id,
        input.scene_id,
        client_revision.as_str(),
    );
    let preview_scope = input
        .preview_scope
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let semantic_core = mei_host_graph::build_semantic_cache_core(
        input.app_id,
        input.scene_id,
        preview_scope.clone(),
        registry_revision,
        client_revision,
        data_generation,
        compile_epoch,
    );
    let persisted_overlay = ops_layout_tuning_revision(input.workspace_root, input.app_id);
    let overlay = overlay_revision(
        persisted_overlay.as_str(),
        input.draft_session,
        input.draft_digest,
    );
    let view_axes = mei_host_graph::build_page_render_view_axes(
        "build",
        input.data_mode,
        input.review_projection,
        None,
        Some(overlay),
    );
    let extra = json!({
        "semantic_core": semantic_core,
        "view_axes": view_axes,
        "node": input.node,
        "focus": input.focus,
        "scope": input.scope,
        "draft_session": input.draft_session,
        "draft_digest": input.draft_digest,
        "compile_coordinate": input.compile_coordinate,
        "host_ssr_payload_revision": HOST_SSR_PAYLOAD_REVISION,
    });
    serde_json::to_string(&extra).unwrap_or_else(|_| {
        format!(
            "{}:{}:{}:{}",
            input.app_id, input.node, input.data_mode, input.review_projection
        )
    })
}

pub fn build_fragment_revision_digest(cache_key: &str) -> String {
    format!("{:016x}", hash_signature(cache_key))
}

pub fn build_fragment_revision_payload(input: &BuildFragmentCacheInput<'_>) -> BuildFragmentRevisionPayload {
    let registry = mei_host_graph::McgRegistryWriter::load(input.workspace_root, input.app_id);
    let registry_revision = registry.registry_revision.trim().to_string();
    let client_revision =
        resolve_build_client_revision(input.workspace_root, input.app_id, input.scene_id);
    let app_root = resolve_app_root(input.workspace_root, input.app_id);
    let data_generation = mei_lang_kernel::load_cache_generation(app_root.as_path(), input.app_id)
        .data_generation;
    let compile_epoch = resolve_build_compile_epoch(
        input.workspace_root,
        input.app_id,
        input.scene_id,
        client_revision.as_str(),
    );
    let persisted_overlay = ops_layout_tuning_revision(input.workspace_root, input.app_id);
    let cache_key = build_fragment_cache_key(input);
    let revision_digest = build_fragment_revision_digest(cache_key.as_str());
    BuildFragmentRevisionPayload {
        ready: !registry_revision.is_empty(),
        app_id: input.app_id.to_string(),
        node: input.node.to_string(),
        scene_id: input.scene_id.to_string(),
        route_mode: "build".to_string(),
        data_mode: input.data_mode.to_string(),
        review_projection: input.review_projection.to_string(),
        focus: input.focus.to_string(),
        scope: input.scope.to_string(),
        preview_scope: input
            .preview_scope
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        registry_revision,
        client_revision,
        data_generation,
        compile_epoch,
        ops_layout_tuning_revision: persisted_overlay,
        draft_session: input.draft_session.to_string(),
        draft_digest: input.draft_digest.to_string(),
        host_ssr_payload_revision: HOST_SSR_PAYLOAD_REVISION.to_string(),
        revision_digest: revision_digest.clone(),
        cache_key: cache_key.clone(),
        compile_coordinate: input.compile_coordinate.cloned(),
    }
}

pub fn take_build_fragment_cache(key: &str) -> Option<CachedBuildFragment> {
    let Ok(mut cache) = memory_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).cloned()
}

pub fn store_build_fragment_cache(key: String, entry: CachedBuildFragment) {
    let Ok(mut cache) = memory_cache().lock() else {
        return;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    if cache.len() >= MAX_BUILD_FRAGMENT_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(key, entry);
}

pub fn clear_build_fragment_cache_for_app(app_id: &str) -> usize {
    let Ok(mut cache) = memory_cache().lock() else {
        return 0;
    };
    let needle = format!("\"app_id\":\"{app_id}\"");
    let keys: Vec<String> = cache
        .keys()
        .filter(|key| key.contains(needle.as_str()))
        .cloned()
        .collect();
    let cleared = keys.len();
    for key in keys {
        cache.remove(key.as_str());
    }
    cleared
}

pub fn cached_build_fragment(
    preview_html: String,
    drilldown_script: String,
    workspace_scripts: Vec<String>,
    node: String,
    focus: String,
    compile_revision: String,
    compile_coordinate: BuildCompileCoordinate,
    data_mode: String,
    review_projection: String,
    revision_digest: String,
) -> CachedBuildFragment {
    CachedBuildFragment {
        expires_at: Instant::now() + cache_ttl(),
        preview_html,
        drilldown_script,
        workspace_scripts,
        node,
        focus,
        compile_revision,
        compile_coordinate,
        data_mode,
        review_projection,
        revision_digest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_fragment_cache_key_parts_differ_by_axes_and_draft() {
        let base = json!({
            "semantic_core": {"app_id":"demo","scene_id":"home","data_mode":"eval"},
            "view_axes": {"route_mode":"build","data_mode":"eval","review_projection":"live_full"},
            "draft_session": "",
            "draft_digest": "",
        });
        let mut with_static = base.clone();
        with_static["view_axes"]["data_mode"] = json!("static");
        assert_ne!(base.to_string(), with_static.to_string());
        let mut with_draft = base.clone();
        with_draft["draft_session"] = json!("sess-1");
        with_draft["draft_digest"] = json!("abc123");
        assert_ne!(base.to_string(), with_draft.to_string());
    }

    #[test]
    fn scene_id_from_build_node_extracts_scene_panel_prefix() {
        assert_eq!(
            scene_id_from_build_node("scene-panel:home/left_rail"),
            "home"
        );
    }
}
