//! Scene view manifest and layer batch APIs (shared materialize in mei-host-graph).

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_auth::AuthServeState;
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use mei_lang_kernel::DataMode;

use crate::pages::AppQuery;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::artifact_observability::{ArtifactHitMatrix, LayerArtifactObservability};
use crate::review_axes::{resolve_page_render_axes_for_stage, StageKind};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct SceneManifestQuery {
    #[serde(default, alias = "app")]
    pub app_id: String,
    pub scene: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
    pub tab: Option<String>,
    pub chrome: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LayerBatchRequest {
    #[serde(default)]
    pub app_id: String,
    pub scene: Option<String>,
    #[serde(default)]
    pub layers: Vec<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub local_miss: bool,
    #[serde(default)]
    pub client_layers: Vec<mei_host_graph::ClientLayerHolding>,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub chrome: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LayerBatchResponse {
    pub layers: std::collections::BTreeMap<String, Value>,
    pub hits: ArtifactHitMatrix,
}

/// Host chrome inputs for `shell.*` layer (real topbar/statusbar SSR).
pub(crate) struct SceneChromeHostContext<'a> {
    pub apps: &'a [mei_lang_kernel::WorkspaceAppMeta],
    pub topbar_menu: Option<&'a mei_lang_app::TopbarMenuContext>,
    pub auth_enabled: bool,
    pub auth_account: Option<&'a mei_lang_app::HostAccountView>,
}

fn render_shell_with_host(
    host: &SceneChromeHostContext<'_>,
    args: mei_host_graph::ShellChromeRenderArgs<'_>,
) -> Option<mei_host_graph::ShellLayerDocument> {
    let compiled = args.compiled?;
    let route_mode = UiRouteMode::from_slug(args.route_mode);
    let stage_kind = StageKind::resolve(
        &compiled.stage_registry,
        &compiled.scene_routes,
        args.scene_id,
    );
    let review_projection =
        crate::review_axes::ssr_review_projection(route_mode, stage_kind, args.data_mode).slug();
    let (mut topbar_html, mut statusbar_html) = mei_lang_app::render_access_shell_chrome_html(
        host.apps,
        compiled,
        args.app_id,
        host.topbar_menu,
        route_mode,
        Some(args.scene_id),
        None,
        Some(args.tab),
        host.auth_enabled,
        host.auth_account,
        Some(args.data_mode.slug()),
        Some(review_projection),
        args.chrome == "none",
    );
    topbar_html = crate::build_info::fill_page_shell_placeholders(topbar_html, args.workspace_root);
    statusbar_html =
        crate::build_info::fill_page_shell_placeholders(statusbar_html, args.workspace_root);
    Some(mei_host_graph::ShellLayerDocument {
        schema_version: mei_host_graph::SHELL_LAYER_SCHEMA.to_string(),
        route_mode: args.route_mode.to_string(),
        tab: args.tab.to_string(),
        chrome: args.chrome.to_string(),
        topbar_html,
        statusbar_html,
    })
}

fn with_shell_chrome<'a, T>(
    chrome_host: Option<&'a SceneChromeHostContext<'a>>,
    f: impl FnOnce(Option<&mei_host_graph::ShellChromeRenderer<'a>>) -> T,
) -> T {
    match chrome_host {
        Some(host) => {
            let render = move |args: mei_host_graph::ShellChromeRenderArgs<'_>| {
                render_shell_with_host(host, args)
            };
            f(Some(&render))
        }
        None => f(None),
    }
}

pub(crate) fn ensure_manifest_index(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    data_mode: DataMode,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::ManifestIndexDocument> {
    with_shell_chrome(chrome_host, |shell_chrome| {
        mei_host_graph::ensure_manifest_index(
            workspace_root,
            app_id,
            scene_id,
            data_mode,
            hits,
            shell_chrome,
        )
    })
}

pub(crate) fn build_scene_view_manifest(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    data_mode: DataMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    draft_digest: &str,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::SceneViewManifest> {
    with_shell_chrome(chrome_host, |shell_chrome| {
        mei_host_graph::build_scene_view_manifest(
            workspace_root,
            app_id,
            scene_id,
            route_mode.slug(),
            data_mode,
            compose,
            draft_session,
            draft_digest,
            hits,
            shell_chrome,
        )
    })
}

pub(crate) fn materialize_layers_for_request(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    data_mode: DataMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    draft_digest: &str,
    layer_names: &[String],
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<std::collections::BTreeMap<String, Value>> {
    with_shell_chrome(chrome_host, |shell_chrome| {
        mei_host_graph::materialize_layers_for_request(
            workspace_root,
            app_id,
            scene_id,
            route_mode.slug(),
            data_mode,
            compose,
            draft_session,
            draft_digest,
            layer_names,
            hits,
            shell_chrome,
        )
    })
}

pub(crate) fn resolve_view_revision_for_surface(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    data_mode: DataMode,
    preview_scope: Option<&str>,
    client_manifest_digest: Option<String>,
    client_surface_digest: Option<String>,
    recover: bool,
    local_miss: bool,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::ViewRevisionResponse> {
    with_shell_chrome(chrome_host, |shell_chrome| {
        mei_host_graph::resolve_view_revision_for_surface(
            workspace_root,
            app_id,
            scene_id,
            route_mode.slug(),
            data_mode,
            preview_scope,
            client_manifest_digest,
            client_surface_digest,
            recover,
            local_miss,
            hits,
            shell_chrome,
        )
    })
}

fn stage_kind_for_scene(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
) -> StageKind {
    match mei_host_graph::assemble_scope_from_registry(workspace_root, app_id, scene_id) {
        Ok(Some(outcome)) => StageKind::resolve(
            &outcome.compiled.stage_registry,
            &outcome.compiled.scene_routes,
            scene_id,
        ),
        _ => StageKind::Scene,
    }
}

pub(crate) fn resolve_route_mode_from_surface(surface: Option<&str>) -> UiRouteMode {
    match surface.map(str::trim).filter(|value| !value.is_empty()) {
        Some("build") | Some("manage") | Some("layout") | Some("prototype") => UiRouteMode::App,
        Some("run") | Some("copilot") | Some("speaker") | Some("presentation") | Some("slides") => {
            UiRouteMode::App
        }
        Some("app") | None => UiRouteMode::App,
        Some(other) => UiRouteMode::from_slug(other),
    }
}

pub async fn api_host_scene_manifest(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    _headers: HeaderMap,
    Query(query): Query<SceneManifestQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let scene_id = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let route_mode = resolve_route_mode_from_surface(query.surface.as_deref());
    let stage_kind = stage_kind_for_scene(workspace_root, app_id, scene_id.as_str());
    let axes = resolve_page_render_axes_for_stage(
        &guard,
        &AppQuery {
            data_mode: query.data_mode.clone(),
            review_projection: query.review_projection.clone(),
            ..Default::default()
        },
        route_mode,
        stage_kind,
    );
    let draft_digest = String::new();

    let compose = mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: query.tab.clone(),
        chrome: query.chrome.clone(),
        review_projection: Some(axes.review_projection.slug().to_string()),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: None,
        scope: None,
        scope_target: None,
    };
    let mut hits = ArtifactHitMatrix::default();
    let manifest = match build_scene_view_manifest(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        "",
        draft_digest.as_str(),
        &mut hits,
        None,
    ) {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let obs = LayerArtifactObservability { hits };
    let mut response = Json(json!({
        "manifest": manifest,
        "hits": obs.hits,
    }))
    .into_response();
    for (name, value) in obs.response_headers() {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    response
}

pub async fn api_host_layer_batch(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    _headers: HeaderMap,
    Json(body): Json<LayerBatchRequest>,
) -> Response {
    let app_id = body.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let scene_id = body
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let route_mode = resolve_route_mode_from_surface(body.surface.as_deref());
    let stage_kind = stage_kind_for_scene(workspace_root, app_id, scene_id.as_str());
    let axes = resolve_page_render_axes_for_stage(
        &guard,
        &AppQuery {
            data_mode: body.data_mode.clone(),
            review_projection: body.review_projection.clone(),
            ..Default::default()
        },
        route_mode,
        stage_kind,
    );
    let draft_digest = String::new();
    let compose = mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: body.tab.clone(),
        chrome: body.chrome.clone(),
        review_projection: body.review_projection.clone(),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: None,
        scope: None,
        scope_target: None,
    };

    let mut hits = ArtifactHitMatrix::default();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = crate::shell_chrome::apps_for_topbar(&guard);
    let chrome_host = SceneChromeHostContext {
        apps: apps.as_slice(),
        topbar_menu: Some(&topbar_menu),
        auth_enabled: false,
        auth_account: None,
    };
    let layers = match materialize_layers_for_request(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        "",
        draft_digest.as_str(),
        body.layers.as_slice(),
        &mut hits,
        Some(&chrome_host),
    ) {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let obs = LayerArtifactObservability { hits };
    let status = if body.local_miss {
        "refetch"
    } else {
        "refetch"
    };
    let mut response = Json(LayerBatchResponse {
        layers,
        hits: obs.hits,
    })
    .into_response();
    for (name, value) in obs.response_headers() {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    if let Ok(header_value) = HeaderValue::from_str(status) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-view-revision-status"),
            header_value,
        );
    }
    if body.local_miss {
        if let Ok(header_value) = HeaderValue::from_str("1") {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-mei-local-miss"), header_value);
        }
    }
    if !body.client_layers.is_empty() {
        if let Ok(header_value) =
            HeaderValue::from_str(body.client_layers.len().to_string().as_str())
        {
            response.headers_mut().insert(
                HeaderName::from_static("x-mei-client-holdings-count"),
                header_value,
            );
        }
    }
    response
}

#[cfg(test)]
mod cross_surface_manifest_tests {
    use super::*;
    use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
    use mei_lang_kernel::DataMode;
    use std::path::PathBuf;

    fn ws_demo_workspace() -> Option<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../workspaces/ws-demo-v2")
            .canonicalize()
            .ok()?;
        let registry = root.join("apps/data-demo/build/active/registry/mcg-registry.json");
        if !registry.is_file() {
            return None;
        }
        match mei_host_graph::assemble_scope_from_registry(root.as_path(), "data-demo", "home") {
            Ok(Some(_)) => Some(root),
            _ => None,
        }
    }

    fn structure_artifact_id(manifest: &mei_host_graph::SceneViewManifest) -> String {
        manifest
            .layers
            .get("structure.full")
            .and_then(|value| value.get("artifact_id"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    }

    #[test]
    fn app_surface_emits_structure_full_artifact_id() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let app_id = "data-demo";
        let scene_id = "home";
        let compose = mei_host_graph::ComposeRequest {
            route_mode: Some("app".to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: None,
            data_mode: Some("static".to_string()),
            focus: None,
            scope: None,
            scope_target: None,
        };
        let mut hits = ArtifactHitMatrix::default();
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            app_id,
            scene_id,
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        let artifact_id = structure_artifact_id(&manifest);
        assert!(
            !artifact_id.is_empty(),
            "missing structure.full for App surface"
        );
    }

    fn layer_content_hash(manifest: &mei_host_graph::SceneViewManifest, name: &str) -> String {
        manifest
            .layers
            .get(name)
            .and_then(|value| value.get("content_hash"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn static_compose(route_slug: &str) -> mei_host_graph::ComposeRequest {
        mei_host_graph::ComposeRequest {
            route_mode: Some(route_slug.to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: None,
            data_mode: Some("static".to_string()),
            focus: None,
            scope: None,
            scope_target: None,
        }
    }

    #[test]
    fn app_surface_has_semantic_layer_hashes_and_digest() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let app_id = "data-demo";
        let scene_id = "home";
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            app_id,
            scene_id,
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        for layer_name in [
            "structure.full",
            "theme.tokens",
            "layout.overlay",
            "runtime.plans",
            "eval.slot_group.scene:default",
        ] {
            let hash = layer_content_hash(&manifest, layer_name);
            assert!(!hash.is_empty(), "missing {layer_name}");
        }
        let semantic = mei_host_graph::semantic_revision_digest(&manifest, None);
        assert!(!semantic.is_empty());
        let surface = mei_host_graph::surface_revision_digest_from_manifest(&manifest);
        assert!(surface.is_some());
    }

    #[test]
    fn app_compose_defaults_follow_stage_kind_scene() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = mei_host_graph::ComposeRequest {
            route_mode: Some("app".to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: Some("live_full".to_string()),
            data_mode: Some("static".to_string()),
            focus: None,
            scope: None,
            scope_target: None,
        };
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            "data-demo",
            "home",
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        let defaults = manifest
            .compose_defaults
            .as_ref()
            .expect("compose_defaults");
        assert_eq!(defaults.review_projection.as_deref(), Some("live_full"));
        assert_eq!(
            StageKind::from_route_meta("scene", "src/scene/home.mei"),
            StageKind::Scene
        );
    }

    #[test]
    fn data_demo_ssr_manifest_refs_stay_compact() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            "data-demo",
            "home",
            UiRouteMode::App,
            DataMode::Eval,
            &compose,
            "",
            "",
            &mut hits,
            None,
        )
        .expect("scene manifest");
        let serialized = serde_json::to_string(&manifest).expect("manifest should serialize");
        assert!(
            serialized.len() < 512 * 1024,
            "SSR manifest refs should be compact (got {} bytes)",
            serialized.len()
        );
        for (name, layer) in &manifest.layers {
            if name.starts_with("eval.slot_group.") {
                assert!(
                    layer.get("document").is_none(),
                    "eval layer `{name}` should be ref-only in SSR manifest"
                );
                assert!(
                    layer.get("bootstrap_seed").is_none(),
                    "eval layer `{name}` must not duplicate bootstrap_seed"
                );
            }
        }
    }

    #[test]
    fn resolve_route_mode_from_surface_maps_legacy_slugs() {
        assert_eq!(
            resolve_route_mode_from_surface(Some("build")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("manage")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("run")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("copilot")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("layout")),
            UiRouteMode::App
        );
        assert_eq!(
            resolve_route_mode_from_surface(Some("prototype")),
            UiRouteMode::App
        );
    }

    #[test]
    fn shell_app_topbar_non_placeholder_with_chrome_host() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
        let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
        let apps = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
        let chrome_host = SceneChromeHostContext {
            apps: apps.as_slice(),
            topbar_menu: Some(&topbar_menu),
            auth_enabled: false,
            auth_account: None,
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            "data-demo",
            "home",
            UiRouteMode::App,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            Some(&chrome_host),
        )
        .expect("scene manifest");
        let shell = manifest
            .layers
            .get("shell.app")
            .and_then(|value| value.get("document"))
            .expect("shell.app document");
        let topbar = shell
            .get("topbar_html")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        assert!(
            topbar.len() > 500,
            "expected real topbar html, got len {}",
            topbar.len()
        );
        let doc: mei_host_graph::ShellLayerDocument =
            serde_json::from_value(shell.clone()).expect("shell document");
        assert!(
            !mei_host_graph::is_placeholder_shell_document(&doc),
            "shell.app must not be placeholder"
        );
    }

    #[test]
    fn materialize_shell_replaces_placeholder_cache() {
        let Some(workspace_root) = ws_demo_workspace() else {
            return;
        };
        let app_id = "data-demo";
        let route_mode = UiRouteMode::App;
        let tab = "scene";
        let chrome = "full";
        let placeholder =
            mei_host_graph::build_shell_layer_document(route_mode.slug(), tab, chrome);
        assert!(mei_host_graph::is_placeholder_shell_document(&placeholder));
        mei_host_graph::store_shell_layer_document(
            app_id,
            "home",
            route_mode.slug(),
            tab,
            chrome,
            None,
            &placeholder,
        );
        let topbar_menu = load_topbar_menu_context(workspace_root.as_path());
        let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
        let apps = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
        let chrome_host = SceneChromeHostContext {
            apps: apps.as_slice(),
            topbar_menu: Some(&topbar_menu),
            auth_enabled: false,
            auth_account: None,
        };
        let mut hits = ArtifactHitMatrix::default();
        let compose = static_compose("app");
        let manifest = build_scene_view_manifest(
            workspace_root.as_path(),
            app_id,
            "home",
            route_mode,
            DataMode::Static,
            &compose,
            "",
            "",
            &mut hits,
            Some(&chrome_host),
        )
        .expect("scene manifest");
        let shell = manifest
            .layers
            .get("shell.app")
            .and_then(|value| value.get("document"))
            .expect("shell.app document");
        let doc: mei_host_graph::ShellLayerDocument =
            serde_json::from_value(shell.clone()).expect("shell document");
        assert!(
            !mei_host_graph::is_placeholder_shell_document(&doc),
            "placeholder cache must be replaced by real chrome"
        );
        assert!(doc.topbar_html.len() > 500);
    }
}
