//! View / eval data-plane handlers (mei-host-graph APIs; thinner than host-shell pages.rs).

use axum::{
    extract::{Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_graph::{
    build_client_bootstrap_payload, build_scene_eval_pack, build_semantic_core_for_scene,
    empty_client_bootstrap_payload, manifest_revision_digest, resolve_view_revision,
    surface_revision_digest_from_manifest, take_layer, BootstrapEmbedStatus,
    ComposeRequest, SceneEvalPackBuildOptions, SceneEvalPackStatus, SceneViewManifest,
    ViewRevisionInput, ViewRevisionStatus, SCENE_VIEW_MANIFEST_SCHEMA,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::SharedRuntimeState;

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
}

#[derive(Debug, Deserialize, Default)]
pub struct LayerBatchBody {
    pub app_id: Option<String>,
    pub scene: Option<String>,
    pub layers: Option<Vec<String>>,
    #[serde(default)]
    pub layer_refs: Option<Value>,
}

fn parse_bool_flag(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

fn resolve_app_id(state: &SharedRuntimeState, query_app: Option<&str>, query_app_id: Option<&str>) -> Result<String, Response> {
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

/// Build a minimal scene-view manifest from semantic core (full layer materialize stays in host-shell).
pub fn build_minimal_scene_manifest(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    surface: &str,
    data_mode: &str,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> SceneViewManifest {
    let semantic_core = build_semantic_core_for_scene(workspace_root, app_id, scene_id);
    let tab = tab
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("scene");
    let chrome = chrome
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("full");
    let compose_defaults = ComposeRequest {
        route_mode: Some(surface.to_string()),
        tab: Some(tab.to_string()),
        chrome: Some(chrome.to_string()),
        review_projection: None,
        data_mode: Some(data_mode.to_string()),
        focus: None,
        scope: None,
    };
    let mut layers = std::collections::BTreeMap::new();
    layers.insert(
        format!("shell.{surface}"),
        json!({
            "schema_version": "shell-layer-v1",
            "placeholder": true,
            "surface": surface,
        }),
    );
    let mut manifest = SceneViewManifest {
        schema_version: SCENE_VIEW_MANIFEST_SCHEMA.to_string(),
        app_id: app_id.to_string(),
        scene_id: scene_id.to_string(),
        semantic_core,
        revision_digest: String::new(),
        layers,
        compose_defaults: Some(compose_defaults),
        surface_revision_digest: None,
    };
    manifest.revision_digest = manifest_revision_digest(&manifest, None);
    manifest.surface_revision_digest = surface_revision_digest_from_manifest(&manifest);
    manifest
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
    let surface = query
        .surface
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("app");
    let data_mode = query
        .data_mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("eval");
    let manifest = build_minimal_scene_manifest(
        state.host.workspace_root.as_path(),
        app_id.as_str(),
        scene_id.as_str(),
        surface,
        data_mode,
        query.tab.as_deref(),
        query.chrome.as_deref(),
    );
    let revision = resolve_view_revision(&ViewRevisionInput {
        manifest,
        client_manifest_digest: query.manifest_revision_digest.clone(),
        client_surface_digest: query.surface_revision_digest.clone(),
        recover: parse_bool_flag(query.recover.as_deref()),
        local_miss: parse_bool_flag(query.local_miss.as_deref()),
        client_layers: Vec::new(),
        missing_layers: Vec::new(),
        surface_revision_digest: None,
    });
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
    let surface = query
        .surface
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("app");
    let data_mode = query
        .data_mode
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("eval");
    let manifest = build_minimal_scene_manifest(
        state.host.workspace_root.as_path(),
        app_id.as_str(),
        scene_id.as_str(),
        surface,
        data_mode,
        query.tab.as_deref(),
        query.chrome.as_deref(),
    );
    Json(json!({
        "manifest": manifest,
        "hits": {
            "structure_hit": false,
            "eval_hit": false,
            "theme_hit": false,
            "overlay_hit": false,
            "shell_hit": false,
            "runtime_plans_hit": false,
        },
        "note": "app-runtime serves a minimal manifest; full layer materialize remains on host-shell during migration",
    }))
    .into_response()
}

pub async fn api_host_layer_batch(
    State(state): State<SharedRuntimeState>,
    axum::Json(body): axum::Json<LayerBatchBody>,
) -> Response {
    let app_id = match resolve_app_id(&state, None, body.app_id.as_deref()) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let _scene = resolve_scene(body.scene.as_deref(), None);
    let mut layers = std::collections::BTreeMap::new();
    if let Some(names) = body.layers.as_ref() {
        for name in names {
            if let Some(bytes) = take_layer(name.as_str()) {
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    layers.insert(name.clone(), value);
                    continue;
                }
            }
            layers.insert(name.clone(), Value::Null);
        }
    }
    if let Some(refs) = body.layer_refs.as_ref() {
        if let Some(obj) = refs.as_object() {
            for (name, value) in obj {
                layers.entry(name.clone()).or_insert(value.clone());
            }
        }
    }
    Json(json!({
        "app_id": app_id,
        "layers": layers,
        "note": "app-runtime layer-batch returns cached layers only; full materialize remains on host-shell during migration",
    }))
    .into_response()
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
    let pack = build_scene_eval_pack(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
        SceneEvalPackBuildOptions {
            client_revision: None,
            fingerprint: query
                .fingerprint
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string),
            neighbor_hops: None,
        },
    );
    if pack.status == SceneEvalPackStatus::PackMiss {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "bootstrap unavailable"})),
        )
            .into_response();
    }
    let Some(payload) =
        build_client_bootstrap_payload(workspace_root, app_id.as_str(), scene_id.as_str())
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "bootstrap unavailable"})),
        )
            .into_response();
    };
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

pub fn inject_view_revision_envelope(html: String, app_id: &str, scene_id: &str, surface: &str) -> String {
    let envelope = json!({
        "schema_version": "mei.view-revision-envelope.v1",
        "app_id": app_id,
        "scene_id": scene_id,
        "scene_bundle_url": format!(
            "/api/host/scene-manifest?app_id={}&scene={}&surface={}",
            app_id, scene_id, surface
        ),
    });
    let envelope_json = serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string());
    let script = format!(
        r#"<script>window.__mei=window.__mei||{{}};window.__mei.view_revision_envelope={envelope_json};window.__mei.scene_manifest_refs={envelope_json};window.__mei.thin_shell=true;window.__mei.view_revision_enabled=true;</script>"#,
        envelope_json = envelope_json,
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
