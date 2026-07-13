//! View / eval data-plane handlers (mei-host-graph shared materialize; thin Access shell).

use axum::{
    extract::{Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_graph::{
    build_client_bootstrap_payload, build_scene_eval_pack, build_scene_view_manifest,
    empty_client_bootstrap_payload, materialize_layers_for_request,
    resolve_view_revision_for_surface, ArtifactHitMatrix, BootstrapEmbedStatus, ComposeRequest,
    SceneEvalPackBuildOptions, ShellChromeRenderArgs, ShellLayerDocument, ViewRevisionStatus,
    SHELL_LAYER_SCHEMA,
};
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use mei_lang_kernel::{discover_apps, DataMode, ReviewProjection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use crate::state::SharedRuntimeState;

fn html_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneQuery {
    pub app: Option<String>,
    pub app_id: Option<String>,
    pub scene: Option<String>,
    pub scope: Option<String>,
    pub fingerprint: Option<String>,
    #[serde(rename = "client_revision")]
    pub client_revision: Option<String>,
    #[serde(rename = "neighbor_hops")]
    pub neighbor_hops: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ViewRevisionQuery {
    pub app_id: String,
    pub scene: Option<String>,
    pub surface: Option<String>,
    pub manifest_revision_digest: Option<String>,
    pub surface_revision_digest: Option<String>,
    pub recover: Option<String>,
    pub local_miss: Option<String>,
    pub data_mode: Option<String>,
    #[serde(default)]
    pub chrome: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneManifestQuery {
    pub app_id: String,
    pub scene: Option<String>,
    pub surface: Option<String>,
    pub data_mode: Option<String>,
    #[serde(default)]
    pub chrome: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LayerBatchBody {
    pub app_id: Option<String>,
    pub scene: Option<String>,
    pub layers: Option<Vec<String>>,
    #[serde(default)]
    pub layer_refs: Option<Value>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub chrome: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub local_miss: Option<bool>,
}

fn parse_bool_flag(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

fn resolve_app_id(
    state: &SharedRuntimeState,
    query_app: Option<&str>,
    query_app_id: Option<&str>,
) -> Result<String, Response> {
    let app = query_app
        .or(query_app_id)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(state.app_id());
    if app != state.app_id() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch", "expected": state.app_id(), "got": app})),
        )
            .into_response());
    }
    Ok(app.to_string())
}

fn resolve_scene(query_scene: Option<&str>, query_scope: Option<&str>) -> String {
    query_scope
        .or(query_scene)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("home")
        .to_string()
}

fn resolve_route_mode_from_surface(surface: Option<&str>) -> UiRouteMode {
    match surface.map(str::trim).filter(|value| !value.is_empty()) {
        Some("build") | Some("manage") | Some("layout") | Some("prototype") => UiRouteMode::App,
        Some("run") | Some("copilot") | Some("speaker") | Some("presentation") | Some("slides") => {
            UiRouteMode::App
        }
        Some("app") | None => UiRouteMode::App,
        Some(other) => UiRouteMode::from_slug(other),
    }
}

fn parse_data_mode(raw: Option<&str>) -> DataMode {
    raw.and_then(DataMode::parse).unwrap_or(DataMode::Eval)
}

fn clamp_data_mode(raw: Option<&str>, ceiling: Option<&str>) -> DataMode {
    let requested = parse_data_mode(raw);
    let Some(ceiling) = ceiling
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(mei_lang_kernel::DataModeCeiling::parse)
    else {
        return requested;
    };
    DataMode::clamp_to_ceiling(requested, ceiling).unwrap_or_else(|| ceiling.as_data_mode())
}

fn default_review_projection(data_mode: DataMode) -> &'static str {
    match data_mode {
        DataMode::Static => ReviewProjection::StaticFull.slug(),
        _ => ReviewProjection::LiveFull.slug(),
    }
}

fn menu_label_for_app(
    topbar_menu: &mei_lang_app::TopbarMenuContext,
    app_id: &str,
) -> Option<String> {
    let from_root = topbar_menu.root.as_ref().and_then(|menu| {
        menu.items
            .iter()
            .find(|item| item.app_id == app_id)
            .and_then(|item| item.label.clone())
    });
    if from_root.is_some() {
        return from_root;
    }
    topbar_menu.by_segment.values().find_map(|menu| {
        menu.items
            .iter()
            .find(|item| item.app_id == app_id)
            .and_then(|item| item.label.clone())
    })
}

fn enrich_apps(
    apps: &[mei_lang_kernel::WorkspaceAppMeta],
    topbar_menu: &mei_lang_app::TopbarMenuContext,
) -> Vec<mei_lang_kernel::WorkspaceAppMeta> {
    apps.iter()
        .map(|app| {
            let mut enriched = app.clone();
            if let Some(label) = menu_label_for_app(topbar_menu, app.id.as_str()) {
                enriched.title = label;
            }
            enriched
        })
        .collect()
}

fn apps_for_runtime_shell_chrome(
    workspace_root: &std::path::Path,
    app_id: &str,
) -> (
    mei_lang_app::TopbarMenuContext,
    Vec<mei_lang_kernel::WorkspaceAppMeta>,
) {
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let discovered = discover_apps(workspace_root).unwrap_or_default();
    // Runtime is single-app: never advertise sibling workspace apps in shell chrome.
    // Host `/api/host/shell-chrome` (LaunchManifest running set) is the multi-app truth.
    let apps = enrich_apps(discovered.as_slice(), &topbar_menu)
        .into_iter()
        .filter(|app| app.id == app_id)
        .collect::<Vec<_>>();
    (topbar_menu, apps)
}

fn scrub_host_shell_placeholders(mut html: String) -> String {
    // Runtime does not own Host build identity; leave safe stubs so attribute parsers
    // never see raw `__MEI_*__` or unescaped JSON in `title="..."`.
    html = html.replace("__MEI_HOST_VERSION_TITLE__", "");
    html = html.replace("__MEI_HOST_VERSION_LABEL__", "mei-app-runtime");
    html = html.replace("__MEI_HOST_VERSION__", "mei-app-runtime");
    // Must bust immutable `/app-bundles/*` cache when dist changes (same stamp Host uses).
    html = html.replace(
        "__MEI_HOST_ASSET_VERSION__",
        runtime_asset_version().as_str(),
    );
    html = html.replace("__MEI_HOST_ICP_RECORD__", "");
    html = html.replace("__MEI_HOST_PSB_RECORD__", "");
    html = html.replace("__MEI_HOST_COPYRIGHT__", "");
    html = html.replace("__MEI_WORKSPACE_LABEL__", "");
    html
}

/// Stamp for `?v=` on `/app-bundles/*` — mirrors host-shell `host_asset_version`.
pub fn runtime_asset_version() -> String {
    use std::time::UNIX_EPOCH;
    let dist_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/assets/dist");
    let newest_stamp = [
        dist_root.join("access.bundle.js"),
        dist_root.join("manage.bundle.js"),
        dist_root.join("styles.bundle.css"),
        dist_root.join("shoelace.bundle.js"),
    ]
    .into_iter()
    .filter_map(|path| {
        let modified = std::fs::metadata(path).ok()?.modified().ok()?;
        let elapsed = modified.duration_since(UNIX_EPOCH).ok()?;
        Some(elapsed.as_millis())
    })
    .max();
    match newest_stamp {
        Some(stamp) => format!("runtime.{stamp}"),
        None => "runtime".to_string(),
    }
}

pub fn fill_runtime_asset_version(html: String) -> String {
    html.replace(
        "__MEI_HOST_ASSET_VERSION__",
        runtime_asset_version().as_str(),
    )
}

fn render_runtime_shell_chrome(
    apps: &[mei_lang_kernel::WorkspaceAppMeta],
    topbar_menu: &mei_lang_app::TopbarMenuContext,
    args: ShellChromeRenderArgs<'_>,
) -> Option<ShellLayerDocument> {
    let compiled = args.compiled?;
    let route_mode = UiRouteMode::from_slug(args.route_mode);
    let review_projection = default_review_projection(args.data_mode);
    let (topbar_html, statusbar_html) = mei_lang_app::render_access_shell_chrome_html(
        apps,
        compiled,
        args.app_id,
        Some(topbar_menu),
        route_mode,
        Some(args.scene_id),
        None,
        Some(args.tab),
        false,
        None,
        Some(args.data_mode.slug()),
        Some(review_projection),
        args.chrome == "none",
    );
    Some(ShellLayerDocument {
        schema_version: SHELL_LAYER_SCHEMA.to_string(),
        route_mode: args.route_mode.to_string(),
        tab: args.tab.to_string(),
        chrome: args.chrome.to_string(),
        topbar_html: scrub_host_shell_placeholders(topbar_html),
        statusbar_html: scrub_host_shell_placeholders(statusbar_html),
    })
}

fn hit_headers(hits: &ArtifactHitMatrix) -> [(&'static str, String); 5] {
    let flag = |v: bool| if v { "1" } else { "0" }.to_string();
    [
        ("x-mei-structure-hit", flag(hits.structure_hit)),
        ("x-mei-eval-hit", flag(hits.eval_hit)),
        ("x-mei-theme-hit", flag(hits.theme_hit)),
        ("x-mei-overlay-hit", flag(hits.overlay_hit)),
        ("x-mei-shell-hit", flag(hits.shell_hit)),
    ]
}

pub async fn api_host_view_revision(
    State(state): State<SharedRuntimeState>,
    Query(query): Query<ViewRevisionQuery>,
) -> Response {
    let app_id = match resolve_app_id(&state, None, Some(query.app_id.as_str())) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let scene_id = resolve_scene(query.scene.as_deref(), None);
    let route_mode = resolve_route_mode_from_surface(query.surface.as_deref());
    let data_mode = clamp_data_mode(
        query.data_mode.as_deref(),
        state.spec.data_mode_ceiling.as_deref(),
    );
    let workspace_root = state.host.workspace_root.as_path();
    let (topbar_menu, apps) = apps_for_runtime_shell_chrome(workspace_root, app_id.as_str());
    // Host API parity: accept compose axes on the query even though revision is index-driven.
    let _compose = ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: query.tab.clone(),
        chrome: query.chrome.clone(),
        review_projection: query
            .review_projection
            .clone()
            .or_else(|| Some(default_review_projection(data_mode).to_string())),
        data_mode: Some(data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let render = |args: ShellChromeRenderArgs<'_>| {
        render_runtime_shell_chrome(apps.as_slice(), &topbar_menu, args)
    };
    let mut hits = ArtifactHitMatrix::default();
    let revision = match resolve_view_revision_for_surface(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
        route_mode.slug(),
        data_mode,
        query
            .manifest_revision_digest
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        query
            .surface_revision_digest
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        parse_bool_flag(query.recover.as_deref()),
        parse_bool_flag(query.local_miss.as_deref()),
        &mut hits,
        Some(&render),
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
    let status = match revision.status {
        ViewRevisionStatus::Refetch => "refetch",
        ViewRevisionStatus::AssembleLocal => "assemble_local",
    };
    let mut response = Json(revision).into_response();
    if let Ok(value) = HeaderValue::from_str(status) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-view-revision-status"), value);
    }
    for (name, value) in hit_headers(&hits) {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    response
}

pub async fn api_host_scene_manifest(
    State(state): State<SharedRuntimeState>,
    Query(query): Query<SceneManifestQuery>,
) -> Response {
    let app_id = match resolve_app_id(&state, None, Some(query.app_id.as_str())) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let scene_id = resolve_scene(query.scene.as_deref(), None);
    let route_mode = resolve_route_mode_from_surface(query.surface.as_deref());
    let data_mode = clamp_data_mode(
        query.data_mode.as_deref(),
        state.spec.data_mode_ceiling.as_deref(),
    );
    let review_projection = query
        .review_projection
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default_review_projection(data_mode));
    let compose = ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: query.tab.clone(),
        chrome: query.chrome.clone(),
        review_projection: Some(review_projection.to_string()),
        data_mode: Some(data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let workspace_root = state.host.workspace_root.as_path();
    let (topbar_menu, apps) = apps_for_runtime_shell_chrome(workspace_root, app_id.as_str());
    let render = |args: ShellChromeRenderArgs<'_>| {
        render_runtime_shell_chrome(apps.as_slice(), &topbar_menu, args)
    };
    let mut hits = ArtifactHitMatrix::default();
    let manifest = match build_scene_view_manifest(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
        route_mode.slug(),
        data_mode,
        &compose,
        "",
        "",
        &mut hits,
        Some(&render),
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
    let mut response = Json(json!({
        "manifest": manifest,
        "hits": hits,
    }))
    .into_response();
    for (name, value) in hit_headers(&hits) {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    response
}

pub async fn api_host_layer_batch(
    State(state): State<SharedRuntimeState>,
    axum::Json(body): axum::Json<LayerBatchBody>,
) -> Response {
    let app_id = match resolve_app_id(&state, None, body.app_id.as_deref()) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let scene_id = resolve_scene(body.scene.as_deref(), None);
    let route_mode = resolve_route_mode_from_surface(body.surface.as_deref());
    let data_mode = clamp_data_mode(
        body.data_mode.as_deref(),
        state.spec.data_mode_ceiling.as_deref(),
    );
    let review_projection = body
        .review_projection
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default_review_projection(data_mode));
    let compose = ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: body.tab.clone(),
        chrome: body.chrome.clone(),
        review_projection: Some(review_projection.to_string()),
        data_mode: Some(data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    let layer_names = body.layers.clone().unwrap_or_default();
    let workspace_root = state.host.workspace_root.as_path();
    let (topbar_menu, apps) = apps_for_runtime_shell_chrome(workspace_root, app_id.as_str());
    let render = |args: ShellChromeRenderArgs<'_>| {
        render_runtime_shell_chrome(apps.as_slice(), &topbar_menu, args)
    };
    let mut hits = ArtifactHitMatrix::default();
    let mut layers = match materialize_layers_for_request(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
        route_mode.slug(),
        data_mode,
        &compose,
        "",
        "",
        layer_names.as_slice(),
        &mut hits,
        Some(&render),
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
    if let Some(refs) = body.layer_refs.as_ref() {
        if let Some(obj) = refs.as_object() {
            for (name, value) in obj {
                layers.entry(name.clone()).or_insert(value.clone());
            }
        }
    }
    let mut response = Json(json!({
        "app_id": app_id,
        "layers": layers,
        "hits": hits,
    }))
    .into_response();
    for (name, value) in hit_headers(&hits) {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    if body.local_miss.unwrap_or(false) {
        if let Ok(header_value) = HeaderValue::from_str("1") {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-mei-local-miss"), header_value);
        }
    }
    response
}

pub async fn api_scene_eval_pack(
    State(state): State<SharedRuntimeState>,
    Query(query): Query<SceneQuery>,
) -> Response {
    let app_id = match resolve_app_id(&state, query.app.as_deref(), query.app_id.as_deref()) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let scene_id = resolve_scene(query.scene.as_deref(), query.scope.as_deref());
    let pack = build_scene_eval_pack(
        state.host.workspace_root.as_path(),
        app_id.as_str(),
        scene_id.as_str(),
        SceneEvalPackBuildOptions {
            client_revision: query
                .client_revision
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            fingerprint: query
                .fingerprint
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            neighbor_hops: query.neighbor_hops,
        },
    );
    Json(pack).into_response()
}

pub async fn api_scene_bootstrap(
    State(state): State<SharedRuntimeState>,
    Query(query): Query<SceneQuery>,
) -> Response {
    let app_id = match resolve_app_id(&state, query.app.as_deref(), query.app_id.as_deref()) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let scene_id = resolve_scene(query.scene.as_deref(), query.scope.as_deref());
    let workspace_root = state.host.workspace_root.as_path();
    let bootstrap: BootstrapEmbedStatus =
        mei_host_graph::bootstrap_embed_status(workspace_root, app_id.as_str(), scene_id.as_str());
    if bootstrap.allowed && bootstrap.reason == "no_client_bootstrap_required" {
        return Json(empty_client_bootstrap_payload(
            workspace_root,
            app_id.as_str(),
            scene_id.as_str(),
        ))
        .into_response();
    }
    // Stale/missing bootstrap must not 404 Access; degrade to empty and let eval layers drive.
    let payload =
        build_client_bootstrap_payload(workspace_root, app_id.as_str(), scene_id.as_str())
            .unwrap_or_else(|| {
                empty_client_bootstrap_payload(workspace_root, app_id.as_str(), scene_id.as_str())
            });
    let mut response = Json(payload).into_response();
    response.headers_mut().insert(
        HeaderName::from_static("deprecation"),
        HeaderValue::from_static("true"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("link"),
        HeaderValue::from_static("</api/host/scene-eval-pack>; rel=\"successor-version\""),
    );
    response
}

pub fn inject_view_revision_envelope_with_dev_eval(
    html: String,
    app_id: &str,
    scene_id: &str,
    surface: &str,
    dev_eval: Option<&serde_json::Value>,
    client_revision: Option<&str>,
) -> String {
    let client_revision = client_revision
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let envelope = json!({
        "schema_version": "mei.view-revision-envelope.v1",
        "app_id": app_id,
        "scene_id": scene_id,
        "client_revision": client_revision,
        "semantic_core": {
            "client_revision": client_revision,
        },
        "scene_bundle_url": format!(
            "/api/host/scene-manifest?app_id={}&scene={}&surface={}",
            app_id, scene_id, surface
        ),
    });
    let envelope_json = serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string());
    let dev_eval_assign = match dev_eval {
        Some(payload) => {
            let json = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
            format!("window.__mei.dev_eval={json};")
        }
        None => String::new(),
    };
    let client_revision_assign = client_revision
        .and_then(|revision| serde_json::to_string(revision).ok())
        .map(|revision| format!("window.__mei.client_revision={revision};"))
        .unwrap_or_default();
    let client_revision_meta = client_revision
        .map(|revision| {
            format!(
                r#"<meta name="mei-bootstrap-client-revision" content="{}"/>"#,
                html_escape_attr(revision)
            )
        })
        .unwrap_or_default();
    let script = format!(
        r#"{client_revision_meta}<script>window.__mei=window.__mei||{{}};window.__mei.view_revision_envelope={envelope_json};window.__mei.scene_manifest_refs={envelope_json};{client_revision_assign}{dev_eval_assign}window.__mei.thin_shell=true;window.__mei.view_revision_enabled=true;</script>"#,
        client_revision_meta = client_revision_meta,
        envelope_json = envelope_json,
        client_revision_assign = client_revision_assign,
        dev_eval_assign = dev_eval_assign,
    );
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + script.len());
        out.push_str(&html[..pos]);
        out.push_str(&script);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{script}{html}")
    }
}
