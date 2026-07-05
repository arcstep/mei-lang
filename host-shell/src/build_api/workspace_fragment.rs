use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{
    compile_coordinate_for_node, resolve_build_view_query, BuildViewTab, LegacyBuildQuery,
};
use serde::Serialize;
use serde_json::json;

use crate::build_api::assemble::{assemble_enriched_for_build_node, AssembleBuildError};
use crate::build_fragment_cache::{
    build_fragment_cache_key, build_fragment_revision_digest, build_fragment_revision_payload,
    cached_build_fragment, draft_digest_for_tuning, scene_id_from_build_node,
    store_build_fragment_cache, take_build_fragment_cache, BuildFragmentCacheInput,
    BuildFragmentRevisionPayload,
};
use crate::build_layout_tuning::{
    apply_build_session_layout_tuning_draft, build_session_layout_tuning_draft,
};
use crate::review_axes::{resolve_page_render_axes, ssr_review_projection_for_axes};
use crate::state::SharedState;

#[derive(Debug, serde::Deserialize)]
pub struct BuildWorkspaceFragmentQuery {
    pub app_id: String,
    pub node: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
}

#[derive(Debug, Serialize)]
struct BuildWorkspaceFragmentResponse {
    compile_revision: String,
    compile_coordinate: mei_lang_kernel::BuildCompileCoordinate,
    drilldown_script: String,
    workspace_scripts: Vec<String>,
    node: String,
    focus: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_projection: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    fragment_cache_hit: bool,
    revision: BuildFragmentRevisionPayload,
    scene_manifest: mei_host_graph::SceneViewManifest,
    compose_defaults: mei_host_graph::ComposeRequest,
}

#[derive(Debug, serde::Deserialize)]
pub struct BuildFragmentRevisionQuery {
    pub app_id: String,
    pub node: String,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
}

fn resolve_fragment_route_mode(surface: Option<&str>) -> UiRouteMode {
    crate::scene_manifest::resolve_route_mode_from_surface(surface)
}

fn draft_context(
    workspace_root: &std::path::Path,
    app_id: &str,
    headers: &HeaderMap,
) -> (String, String, Option<serde_json::Value>) {
    let session_id = mei_host_core::resolve_draft_session_id(headers);
    let storage_key =
        mei_host_core::layout_tuning_draft_storage_key(app_id, session_id.as_str());
    let draft = build_session_layout_tuning_draft(
        workspace_root,
        app_id,
        storage_key.as_str(),
    );
    let digest = draft_digest_for_tuning(draft.as_ref());
    (session_id, digest, draft)
}

fn cache_input<'a>(
    workspace_root: &'a std::path::Path,
    app_id: &'a str,
    node_raw: &'a str,
    scene_id: &'a str,
    focus: &'a str,
    scope: &'a str,
    preview_scope: Option<&'a str>,
    route_mode: UiRouteMode,
    axes: &crate::review_axes::PageRenderAxes,
    draft_session: &'a str,
    draft_digest: &'a str,
    compile_coordinate: Option<&'a mei_lang_kernel::BuildCompileCoordinate>,
) -> BuildFragmentCacheInput<'a> {
    BuildFragmentCacheInput {
        workspace_root,
        app_id,
        node: node_raw,
        scene_id,
        focus,
        scope,
        preview_scope,
        route_mode: route_mode.slug(),
        data_mode: axes.data_mode.slug(),
        review_projection: ssr_review_projection_for_axes(route_mode, *axes).slug(),
        compile_coordinate,
        draft_session,
        draft_digest,
    }
}

pub async fn api_build_fragment_revision(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<BuildFragmentRevisionQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "app_id is required");
    }
    let node_raw = query.node.trim();
    if node_raw.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "node is required");
    }
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let route_mode = resolve_fragment_route_mode(query.surface.as_deref());
    let axes = {
        let guard = state.read().expect("state lock");
        resolve_page_render_axes(
            &guard,
            &crate::pages::AppQuery {
                data_mode: query.data_mode.clone(),
                review_projection: query.review_projection.clone(),
                ..Default::default()
            },
            route_mode,
        )
    };
    let (draft_session, draft_digest, _) = draft_context(workspace_root.as_path(), app_id, &headers);
    let scene_id = scene_id_from_build_node(node_raw);
    let focus = query.focus.as_deref().unwrap_or("").trim();
    let scope = query
        .scope
        .as_deref()
        .map(mei_lang_kernel::BuildExecScope::parse_slug)
        .map(|value| value.slug())
        .unwrap_or("warmup");
    let input = cache_input(
        workspace_root.as_path(),
        app_id,
        node_raw,
        scene_id.as_str(),
        focus,
        scope,
        None,
        route_mode,
        &axes,
        draft_session.as_str(),
        draft_digest.as_str(),
        None,
    );
    let mut payload = build_fragment_revision_payload(&input);
    let compose = mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: Some("scene".to_string()),
        chrome: Some("full".to_string()),
        review_projection: Some(
            ssr_review_projection_for_axes(route_mode, axes).slug().to_string(),
        ),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: query.focus.clone(),
        scope: query.scope.clone(),
    };
    let mut hits = crate::artifact_observability::ArtifactHitMatrix::default();
    if let Ok(manifest) = crate::scene_manifest::build_scene_view_manifest(
        workspace_root.as_path(),
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        draft_session.as_str(),
        draft_digest.as_str(),
        &mut hits,
        None,
    ) {
        payload.manifest_revision_digest = manifest.revision_digest;
    }
    let mut response = Json(payload).into_response();
    *response.status_mut() = StatusCode::OK;
    response
}

fn try_scene_manifest(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    axes: &crate::review_axes::PageRenderAxes,
    draft_session: &str,
    draft_digest: &str,
) -> anyhow::Result<mei_host_graph::SceneViewManifest> {
    let mut hits = crate::artifact_observability::ArtifactHitMatrix::default();
    let compose = mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: Some("scene".to_string()),
        chrome: Some("full".to_string()),
        review_projection: Some(
            ssr_review_projection_for_axes(route_mode, *axes)
                .slug()
                .to_string(),
        ),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: None,
        scope: None,
    };
    crate::scene_manifest::build_scene_view_manifest(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        axes.data_mode,
        &compose,
        draft_session,
        draft_digest,
        &mut hits,
        None,
    )
}

fn compose_defaults_from_manifest(manifest: &mei_host_graph::SceneViewManifest) -> mei_host_graph::ComposeRequest {
    manifest
        .compose_defaults
        .clone()
        .unwrap_or_default()
}

pub async fn api_build_workspace_fragment(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<BuildWorkspaceFragmentQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "app_id is required");
    }
    let node_raw = query.node.trim();
    if node_raw.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "node is required");
    }
    let legacy = LegacyBuildQuery {
        file: None,
        scene: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        tab: query.tab.clone(),
    };
    let Some(resolved) = resolve_build_view_query(
        Some(node_raw),
        query.scope.as_deref(),
        query.tab.as_deref(),
        &legacy,
    ) else {
        return json_error(StatusCode::BAD_REQUEST, "invalid node id");
    };
    let tab = query
        .tab
        .as_deref()
        .and_then(BuildViewTab::parse_slug)
        .unwrap_or(resolved.tab);
    if tab != BuildViewTab::Preview {
        return json_error(
            StatusCode::BAD_REQUEST,
            "workspace fragment requires preview tab",
        );
    }

    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };

    let route_mode = resolve_fragment_route_mode(query.surface.as_deref());
    let axes = {
        let guard = state.read().expect("state lock");
        resolve_page_render_axes(
            &guard,
            &crate::pages::AppQuery {
                data_mode: query.data_mode.clone(),
                review_projection: query.review_projection.clone(),
                ..Default::default()
            },
            route_mode,
        )
    };

    let (draft_session, draft_digest, _) = draft_context(workspace_root.as_path(), app_id, &headers);
    let focus = query.focus.as_deref().unwrap_or("").trim();
    let scope = resolved.scope.slug();
    let scene_id = scene_id_from_build_node(node_raw);
    let preview_scope_hint = query.scope.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let ssr_projection = ssr_review_projection_for_axes(route_mode, axes).slug();

    let preliminary_input = cache_input(
        workspace_root.as_path(),
        app_id,
        node_raw,
        scene_id.as_str(),
        focus,
        scope,
        preview_scope_hint,
        route_mode,
        &axes,
        draft_session.as_str(),
        draft_digest.as_str(),
        None,
    );
    let cache_key = build_fragment_cache_key(&preliminary_input);

    if let Some(cached) = take_build_fragment_cache(cache_key.as_str()) {
        let revision = build_fragment_revision_payload(&preliminary_input);
        let compile_revision = cached.compile_revision.clone();
        let scene_manifest = match try_scene_manifest(
            workspace_root.as_path(),
            app_id,
            scene_id.as_str(),
            route_mode,
            &axes,
            draft_session.as_str(),
            draft_digest.as_str(),
        ) {
            Ok(value) => value,
            Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string().as_str()),
        };
        let compose_defaults = compose_defaults_from_manifest(&scene_manifest);
        let body = BuildWorkspaceFragmentResponse {
            compile_revision: compile_revision.clone(),
            compile_coordinate: cached.compile_coordinate,
            drilldown_script: cached.drilldown_script,
            workspace_scripts: cached.workspace_scripts,
            node: cached.node,
            focus: cached.focus,
            data_mode: Some(cached.data_mode),
            review_projection: Some(cached.review_projection),
            fragment_cache_hit: true,
            revision,
            scene_manifest,
            compose_defaults,
        };
        return fragment_json_response(
            body,
            compile_revision.as_str(),
            true,
            cached.revision_digest.as_str(),
        );
    }

    let mut assembled = match assemble_enriched_for_build_node(
        workspace_root.as_path(),
        app_id,
        node_raw,
        None,
    ) {
        Ok(value) => value,
        Err(AssembleBuildError::InvalidNode) => {
            return json_error(StatusCode::BAD_REQUEST, "invalid node id");
        }
        Err(error) => {
            let status = match &error {
                AssembleBuildError::NotAssembled(_) => StatusCode::SERVICE_UNAVAILABLE,
                AssembleBuildError::AssembleFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                AssembleBuildError::InvalidNode => StatusCode::BAD_REQUEST,
            };
            return (
                status,
                Json(json!({
                    "error": error.message(),
                })),
            )
                .into_response();
        }
    };

    apply_build_session_layout_tuning_draft(
        &mut assembled.compiled,
        workspace_root.as_path(),
        app_id,
        &headers,
    );

    let Some(compile_coordinate) =
        compile_coordinate_for_node(&resolved.node, &assembled.compiled)
    else {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to resolve compile coordinate",
        );
    };

    let preview_scope = mei_lang_kernel::resolve_build_preview_scope_for_ssr(
        &assembled.compiled,
        &resolved.node,
    );
    let scene_for_key = compile_coordinate
        .scene_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| scene_id.clone());
    let final_input = cache_input(
        workspace_root.as_path(),
        app_id,
        node_raw,
        scene_for_key.as_str(),
        focus,
        scope,
        preview_scope.as_deref(),
        route_mode,
        &axes,
        draft_session.as_str(),
        draft_digest.as_str(),
        Some(&compile_coordinate),
    );
    let final_cache_key = build_fragment_cache_key(&final_input);
    let final_revision_digest = build_fragment_revision_digest(final_cache_key.as_str());
    let mut revision = build_fragment_revision_payload(&final_input);

    let scene_manifest = match try_scene_manifest(
        workspace_root.as_path(),
        app_id,
        scene_for_key.as_str(),
        route_mode,
        &axes,
        draft_session.as_str(),
        draft_digest.as_str(),
    ) {
        Ok(value) => value,
        Err(err) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string().as_str()),
    };
    revision.manifest_revision_digest = scene_manifest.revision_digest.clone();
    let compose_defaults = compose_defaults_from_manifest(&scene_manifest);
    let body = BuildWorkspaceFragmentResponse {
        compile_revision: assembled.compile_revision.clone(),
        compile_coordinate: compile_coordinate.clone(),
        drilldown_script: String::new(),
        workspace_scripts: vec![],
        node: node_raw.to_string(),
        focus: focus.to_string(),
        data_mode: Some(axes.data_mode.slug().to_string()),
        review_projection: Some(axes.review_projection.slug().to_string()),
        fragment_cache_hit: false,
        revision: revision.clone(),
        scene_manifest,
        compose_defaults,
    };

    store_build_fragment_cache(
        final_cache_key,
        cached_build_fragment(
            String::new(),
            vec![],
            node_raw.to_string(),
            focus.to_string(),
            assembled.compile_revision.clone(),
            compile_coordinate,
            axes.data_mode.slug().to_string(),
            ssr_projection.to_string(),
            final_revision_digest.clone(),
            body.scene_manifest.revision_digest.clone(),
        ),
    );

    fragment_json_response(
        body,
        assembled.compile_revision.as_str(),
        false,
        final_revision_digest.as_str(),
    )
}

fn fragment_json_response(
    body: BuildWorkspaceFragmentResponse,
    compile_revision: &str,
    cache_hit: bool,
    revision_digest: &str,
) -> Response {
    let mut response = Json(body).into_response();
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-mei-nav-tier"),
        HeaderValue::from_static("fragment"),
    );
    if cache_hit {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-build-fragment-cache-hit"),
            HeaderValue::from_static("1"),
        );
    }
    if let Ok(value) = HeaderValue::from_str(compile_revision) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-compile-revision"), value);
    }
    if let Ok(value) = HeaderValue::from_str(revision_digest) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-fragment-revision-digest"),
            value,
        );
    }
    response
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
