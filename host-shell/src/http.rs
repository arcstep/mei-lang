use axum::{
    extract::{DefaultBodyLimit, OriginalUri, Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post, put},
    Extension, Router,
};
use mei_lang_app::scene_theme_style_for_theme_id;
use mei_lang_kernel::{decode_theme_ref_token, load_mei_config_for_app};
use serde_json::json;

use crate::api_stubs::{
    api_agent_config_stub, api_agent_context_preview_stub, api_agent_runtime_stub,
    api_agent_sessions_stub, api_agent_skill_stub,
};
use crate::assets::{app_asset, app_bundle, component_asset, workspace_app_asset};
use crate::build_api::{
    api_build_context_export, api_build_graph_mcg, api_build_graph_mcg_artifact,
    api_build_graph_mcg_node,
};
use crate::build_info::{self, BUILD_VERSION};
use crate::generation_lifecycle::{
    api_host_build_activate, api_host_build_rollback, api_host_builds, api_host_builds_cleanup,
    api_host_builds_cleanup_preview,
};
use crate::host_home::host_home_page;
use crate::host_mcg::host_mcg_page;
use crate::host_scoped::{host_config_page, host_runtime_page, host_upload_page};
use crate::instance_api::{
    api_host_instance_restart, api_host_instance_stop, api_host_instances, api_host_launch_manifest,
};
use crate::landing::build_discovered_app_summaries;
use crate::ops_api::{
    api_host_builds_request, api_host_ops_prebuild, api_host_ops_reload, api_host_ops_status,
    api_host_runtime_apply_profile,
};
use crate::ops_config_api::{ops_boundary_get, ops_config_get, ops_config_put, ops_journal_get};
use crate::pages::{
    api_host_access_readiness, api_presentation_map, api_scene_bootstrap,
    api_scene_drilldown_context, api_scene_eval_pack, app_root_page, app_scoped_stage_page,
    app_stage_page, app_temp_stage_page, host_starting_page,
};
use crate::presentation_compile::api_presentation_compile;
use crate::presentation_scripts::{
    api_get_presentation_script, api_list_presentation_scripts, api_put_presentation_script,
    api_set_default_presentation_script,
};
use crate::route_lifecycle::{api_host_route_cutover, api_host_route_rollback};
use crate::runtime_api::{
    api_host_mrg_activate, api_host_mrg_status, api_host_runtime_activate_env, api_runtime_snapshot,
};
use crate::shell_redirects::{
    redirect_apps_access, redirect_apps_app_scene, redirect_apps_app_scene_id,
    redirect_apps_app_to_stage, redirect_apps_config, redirect_apps_layout_to_stage,
    redirect_apps_prototype_to_stage, redirect_apps_runtime, redirect_apps_upload,
    redirect_apps_view_to_stage, redirect_host_config, redirect_host_runtime, redirect_host_upload,
    redirect_mode_first_app_root, redirect_mode_first_app_scene, redirect_mode_first_app_tail,
    redirect_root_to_home,
};
use crate::state::{HostHttpState, SharedState};
use crate::upload_api::{
    upload_chunk_complete_post, upload_chunk_init_post, upload_chunk_put, upload_chunk_status_get,
    upload_dir_create_post, upload_entry_rename_post, upload_file_delete, upload_file_download_get,
    upload_file_move_post, upload_file_post,
};
use crate::workspace_profile_api::{
    runtime_profile_get, workspace_profile_dry_run_post, workspace_profile_get,
    workspace_profile_put, workspace_profile_validate_post, workspace_profiles_get,
};

pub fn router(state: HostHttpState) -> Router {
    Router::new()
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/app-assets/favicon.svg") }),
        )
        .route("/", get(redirect_root_to_home))
        .route("/host", get(redirect_root_to_home))
        .route("/home", get(host_home_page))
        .route("/config", get(host_config_page))
        .route("/upload", get(host_upload_page))
        .route("/runtime", get(host_runtime_page))
        .route("/mcg", get(host_mcg_page))
        .route("/host/config", get(redirect_host_config))
        .route("/host/upload", get(redirect_host_upload))
        .route("/host/runtime", get(redirect_host_runtime))
        .route("/host/starting", get(host_starting_page))
        .route("/apps/upload/:app_id", get(redirect_apps_upload))
        .route("/apps/config/:app_id", get(redirect_apps_config))
        .route("/apps/runtime/:app_id", get(redirect_apps_runtime))
        .route("/apps/access/:app_id", get(redirect_apps_access))
        .route("/apps/app/:app_id", get(redirect_mode_first_app_root))
        .route(
            "/apps/app/:app_id/scene/:scene",
            get(redirect_mode_first_app_scene),
        )
        .route("/apps/app/:app_id/*tail", get(redirect_mode_first_app_tail))
        .route("/login", get(mei_host_auth::login_page))
        .route("/logout", get(mei_host_auth::logout_page))
        .route(
            "/account/password",
            get(mei_host_auth::account_change_password_page),
        )
        .route(
            "/api/host/client-trace",
            post(crate::client_trace::api_host_client_trace),
        )
        .route("/api/host/heartbeat", get(api_host_heartbeat))
        .route("/api/host/version", get(api_host_version))
        .route("/api/host/ready", get(api_host_ready))
        .route("/api/host/readiness", get(api_host_ready))
        .route("/api/host/access-readiness", get(api_host_access_readiness))
        .route("/api/host/events", get(crate::host_events::api_host_events))
        .route("/api/host/launch-manifest", get(api_host_launch_manifest))
        .route("/api/host/instances", get(api_host_instances))
        .route(
            "/api/host/apps",
            get(crate::app_launch_api::api_host_apps_overview),
        )
        .route(
            "/api/host/shell-chrome",
            get(crate::shell_chrome::api_host_shell_chrome),
        )
        .route(
            "/api/host/apps/:app_id/launch-configs",
            get(crate::app_launch_api::api_host_app_launch_configs),
        )
        .route(
            "/api/host/apps/:app_id/launch-configs/default",
            post(crate::app_launch_api::api_host_app_ensure_default_launch),
        )
        .route(
            "/api/host/apps/:app_id/runtime-overlay",
            get(crate::app_launch_api::api_host_app_runtime_overlay_get)
                .put(crate::app_launch_api::api_host_app_runtime_overlay_put),
        )
        .route(
            "/api/host/apps/:app_id/runtime-overlay/reset",
            post(crate::app_launch_api::api_host_app_runtime_overlay_reset),
        )
        .route(
            "/api/host/apps/:app_id/launch-configs/:name",
            axum::routing::put(crate::app_launch_api::api_host_app_save_launch),
        )
        .route(
            "/api/host/apps/:app_id/start",
            post(crate::app_launch_api::api_host_app_start),
        )
        .route(
            "/api/host/apps/:app_id/stop",
            post(crate::app_launch_api::api_host_app_stop),
        )
        .route(
            "/api/host/instances/:instance_id/stop",
            post(api_host_instance_stop),
        )
        .route(
            "/api/host/instances/:instance_id/restart",
            post(api_host_instance_restart),
        )
        .route("/api/host/runtime/profile", get(runtime_profile_get))
        .route("/api/host/workspace-profiles", get(workspace_profiles_get))
        .route(
            "/api/host/workspace-profiles/:id",
            get(workspace_profile_get).put(workspace_profile_put),
        )
        .route(
            "/api/host/workspace-profiles/:id/validate",
            post(workspace_profile_validate_post),
        )
        .route(
            "/api/host/workspace-profiles/:id/dry-run",
            post(workspace_profile_dry_run_post),
        )
        .route("/api/host/ops/status", get(api_host_ops_status))
        .route("/api/host/ops/reload", post(api_host_ops_reload))
        .route("/api/host/ops/prebuild", post(api_host_ops_prebuild))
        .route("/api/host/builds", get(api_host_builds))
        .route("/api/host/builds/request", post(api_host_builds_request))
        .route(
            "/api/host/builds/cleanup-preview",
            post(api_host_builds_cleanup_preview),
        )
        .route("/api/host/builds/cleanup", post(api_host_builds_cleanup))
        .route(
            "/api/host/builds/:generation/activate",
            post(api_host_build_activate),
        )
        .route(
            "/api/host/builds/:generation/rollback",
            post(api_host_build_rollback),
        )
        .route(
            "/api/host/routes/:app_id/cutover",
            post(api_host_route_cutover),
        )
        .route(
            "/api/host/routes/:app_id/rollback",
            post(api_host_route_rollback),
        )
        .route(
            "/api/host/runtime/apply-profile",
            post(api_host_runtime_apply_profile),
        )
        .route(
            "/api/host/runtime/activate-env",
            post(api_host_runtime_activate_env),
        )
        .route("/api/runtime/snapshot", get(api_runtime_snapshot))
        .route("/api/build/graph/mcg", get(api_build_graph_mcg))
        .route("/api/build/graph/mcg/node", get(api_build_graph_mcg_node))
        .route(
            "/api/build/graph/mcg/artifact",
            get(api_build_graph_mcg_artifact),
        )
        .route("/api/build/context/export", get(api_build_context_export))
        .route("/api/host/mrg/status", get(api_host_mrg_status))
        .route("/api/host/mrg/activate", post(api_host_mrg_activate))
        .route("/api/host/view-revision", get(gateway_host_view_revision))
        .route("/api/host/scene-manifest", get(gateway_host_scene_manifest))
        .route("/api/host/layer-batch", post(gateway_host_layer_batch))
        .route("/api/host/scene-bootstrap", get(gateway_scene_bootstrap))
        .route("/api/host/scene-eval-pack", get(gateway_scene_eval_pack))
        .route(
            "/api/host/scene-drilldown-context",
            get(api_scene_drilldown_context),
        )
        .route(
            "/api/agent/context/preview",
            get(api_agent_context_preview_stub),
        )
        .route("/api/auth/public-key", get(mei_host_auth::auth_public_key))
        .route("/api/auth/session", get(mei_host_auth::auth_session))
        .route("/api/auth/refresh", post(mei_host_auth::auth_refresh))
        .route("/api/auth/login", post(mei_host_auth::auth_login))
        .route("/api/auth/logout", post(mei_host_auth::auth_logout))
        .route(
            "/api/auth/change-password",
            post(mei_host_auth::auth_change_password),
        )
        .route("/api/agent/config", get(api_agent_config_stub))
        .route("/api/agent/runtime", get(api_agent_runtime_stub))
        .route("/api/agent/skill", get(api_agent_skill_stub))
        .route("/api/agent/session", get(api_agent_sessions_stub))
        .route(
            "/api/datasets/query/:app_id",
            post(api_datasets_query_with_app),
        )
        .route("/api/datasets/query", post(api_datasets_query))
        .route("/api/datasets/metrics/:app_id", post(api_datasets_metrics))
        .route("/api/datasets/fixture/:app_id", post(api_datasets_fixture))
        .route("/api/ops/theme/style/:app_id", get(api_ops_theme_style))
        .route(
            "/api/ops/themes/layout/overlay/:app_id",
            get(crate::ops_theme_layout_api::api_ops_theme_layout_overlay_get),
        )
        .route(
            "/api/ops/themes/layout/apply/:app_id",
            axum::routing::post(crate::ops_theme_layout_api::api_ops_theme_layout_apply_post),
        )
        .route("/api/presentation/map/:app_id", get(api_presentation_map))
        .route("/api/presentation/compile", post(api_presentation_compile))
        .route(
            "/api/presentation/scripts/:app_id",
            get(api_list_presentation_scripts),
        )
        .route(
            "/api/presentation/scripts/:app_id/:script_id",
            get(api_get_presentation_script).put(api_put_presentation_script),
        )
        .route(
            "/api/presentation/scripts/:app_id/:script_id/default",
            post(api_set_default_presentation_script),
        )
        .route("/api/ops/boundary", get(ops_boundary_get))
        .route("/api/ops/journal/:app_id", get(ops_journal_get))
        .route(
            "/api/ops/config/:app_id",
            get(ops_config_get).put(ops_config_put),
        )
        .route(
            "/api/upload/init/:app_id",
            post(upload_chunk_init_post).layer(DefaultBodyLimit::max(256 * 1024)),
        )
        .route("/api/upload/status/:app_id", get(upload_chunk_status_get))
        .route(
            "/api/upload/chunk/:app_id",
            put(upload_chunk_put).layer(DefaultBodyLimit::max(9 * 1024 * 1024)),
        )
        .route(
            "/api/upload/complete/:app_id",
            post(upload_chunk_complete_post).layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/move/:app_id",
            post(upload_file_move_post).layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/dir/:app_id",
            post(upload_dir_create_post).layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/rename/:app_id",
            post(upload_entry_rename_post).layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/download/:app_id",
            get(upload_file_download_get),
        )
        .route(
            "/api/upload/:app_id",
            post(upload_file_post)
                .delete(upload_file_delete)
                .layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route("/gis/*path", get(crate::gis_proxy::gis_proxy))
        .route("/apps/:app_id/view", get(redirect_apps_view_to_stage))
        .route("/apps/:app_id/view/*tail", get(redirect_apps_view_to_stage))
        .route("/apps/:app_id/app", get(redirect_apps_app_to_stage))
        .route(
            "/apps/:app_id/app/scene/:scene",
            get(redirect_apps_app_scene_id),
        )
        .route("/apps/:app_id/app/*tail", get(redirect_apps_app_scene))
        .route("/apps/:app_id/layout", get(redirect_apps_layout_to_stage))
        .route(
            "/apps/:app_id/layout/*tail",
            get(redirect_apps_layout_to_stage),
        )
        .route(
            "/apps/:app_id/prototype",
            get(redirect_apps_prototype_to_stage),
        )
        .route(
            "/apps/:app_id/prototype/*tail",
            get(redirect_apps_prototype_to_stage),
        )
        // Phase 8.5 temporary Stage MUST register before bare `/:stage_id`.
        .route(
            "/apps/:app_id/~/*target_tail",
            get(app_temp_stage_page),
        )
        // Legacy deep scoped tails redirect to `/apps/{app}/~/…`.
        .route(
            "/apps/:app_id/:stage_id/*scoped_tail",
            get(app_scoped_stage_page),
        )
        .route("/apps/:app_id/:stage_id", get(app_stage_page))
        .route("/apps/:app_id", get(app_root_page))
        // legacy mode-first catch-all removed: conflicts with /apps/:app_id/:stage_id
        .route("/app-bundles/:mode", get(app_bundle))
        .route("/app-assets/*path", get(app_asset))
        .route(
            "/workspace-app-assets/:app_id/*path",
            get(workspace_app_asset),
        )
        .route("/workspace-components/*path", get(component_asset))
        .with_state(state)
}

async fn api_host_heartbeat(State(state): State<SharedState>) -> impl IntoResponse {
    let mut guard = state.write().expect("state lock");
    crate::build_ops::refresh_materialization_flags(&mut guard);
    let default_access_ready = guard.data_plane_enabled && guard.imported;
    let default_warmup_ready = guard.warmed_up;
    let workspace_root = guard.ctx.workspace_root.as_path();
    let has_active_profile = crate::workspace_profile_api::read_host_control_state(workspace_root)
        .is_some_and(|value| value.get("activeProfile").is_some());
    let default_phase = if guard
        .ops_job
        .as_ref()
        .is_some_and(crate::build_ops::OpsJobState::is_running)
    {
        "building"
    } else if !has_active_profile {
        "unconfigured"
    } else if !guard.data_plane_enabled || !guard.imported || guard.startup_error.is_some() {
        "degraded"
    } else if default_warmup_ready {
        "ready"
    } else {
        "degraded"
    };
    let default_app_id = guard.default_app().map(str::to_string);
    let discovered_apps = build_discovered_app_summaries(&guard);
    let any_app_access_ready = discovered_apps
        .iter()
        .any(|app| app.get("accessReady").and_then(|value| value.as_bool()) == Some(true));
    let descriptor =
        build_info::version_descriptor(Some(workspace_root), Some(guard.host_started_at_ms));
    let display_label = descriptor
        .get("displayLabel")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    Json(json!({
        "buildVersion": BUILD_VERSION,
        "buildDescriptor": build_info::binary_descriptor(),
        "displayLabel": display_label,
        "version": descriptor,
        "hostStartedAtMs": guard.host_started_at_ms,
        "ready": true,
        "hostReady": true,
        "accessReady": default_access_ready,
        "warmupReady": default_warmup_ready,
        "fullWarmupReady": default_warmup_ready,
        "defaultAppId": default_app_id,
        "defaultAppAccessReady": default_access_ready,
        "defaultAppWarmupReady": default_warmup_ready,
        "anyAppAccessReady": any_app_access_ready,
        "phase": default_phase,
        "startupPhase": guard.startup_phase,
        "startupDetail": guard.startup_detail,
        "scopeNote": "materialization flags reflect default app; discoveredApps lists all apps",
        "discoveredApps": discovered_apps,
        "apps": discovered_apps,
    }))
}

async fn api_host_version(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    Json(build_info::version_descriptor(
        Some(guard.ctx.workspace_root.as_path()),
        Some(guard.host_started_at_ms),
    ))
}

async fn api_host_ready(State(state): State<SharedState>) -> impl IntoResponse {
    let (
        imported,
        warmed_up,
        data_plane_enabled,
        route_plane_ready,
        startup_phase,
        startup_detail,
        startup_error,
    ) = {
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        (
            guard.imported,
            guard.warmed_up,
            guard.data_plane_enabled,
            guard.route_plane_ready,
            guard.startup_phase.clone(),
            guard.startup_detail.clone(),
            guard.startup_error.clone(),
        )
    };
    (
        StatusCode::OK,
        Json(json!({
            "hostReady": true,
            "controlReady": true,
            "accessReady": data_plane_enabled && imported,
            "routeReady": route_plane_ready,
            "dataPlaneEnabled": data_plane_enabled,
            "imported": imported,
            "warmedUp": warmed_up,
            "warmupReady": warmed_up,
            "startupPhase": startup_phase,
            "startupDetail": startup_detail,
            "startupError": startup_error,
        })),
    )
}

async fn gateway_host_view_revision(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(query): Query<crate::view_revision::ViewRevisionQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if !app_id.is_empty() {
        if let Some(response) = crate::app_runtime_proxy::maybe_proxy_app_request(
            &http,
            app_id,
            Method::GET,
            uri.path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.path()),
            &headers,
            None,
            principal.as_ref().map(|p| (**p).clone()),
        )
        .await
        {
            return response;
        }
    }
    crate::view_revision::api_host_view_revision(
        State(http.shell),
        State(http.auth),
        headers,
        Query(query),
    )
    .await
}

async fn gateway_host_scene_manifest(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(query): Query<crate::scene_manifest::SceneManifestQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if !app_id.is_empty() {
        if let Some(response) = crate::app_runtime_proxy::maybe_proxy_app_request(
            &http,
            app_id,
            Method::GET,
            uri.path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.path()),
            &headers,
            None,
            principal.as_ref().map(|p| (**p).clone()),
        )
        .await
        {
            return response;
        }
    }
    crate::scene_manifest::api_host_scene_manifest(
        State(http.shell),
        State(http.auth),
        headers,
        Query(query),
    )
    .await
}

async fn gateway_host_layer_batch(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    axum::Json(body): axum::Json<crate::scene_manifest::LayerBatchRequest>,
) -> Response {
    let app_id = body.app_id.trim().to_string();
    if !app_id.is_empty() {
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        if let Some(response) = crate::app_runtime_proxy::maybe_proxy_app_request(
            &http,
            app_id.as_str(),
            Method::POST,
            uri.path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.path()),
            &headers,
            Some(bytes),
            principal.as_ref().map(|p| (**p).clone()),
        )
        .await
        {
            return response;
        }
    }
    crate::scene_manifest::api_host_layer_batch(
        State(http.shell),
        State(http.auth),
        headers,
        axum::Json(body),
    )
    .await
}

async fn gateway_scene_bootstrap(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(query): Query<crate::pages::SceneBootstrapQuery>,
) -> Response {
    let app_id = query.app.trim();
    if !app_id.is_empty() {
        if let Some(response) = crate::app_runtime_proxy::maybe_proxy_app_request(
            &http,
            app_id,
            Method::GET,
            uri.path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.path()),
            &headers,
            None,
            principal.as_ref().map(|p| (**p).clone()),
        )
        .await
        {
            return response;
        }
    }
    api_scene_bootstrap(State(http.shell), principal, Query(query)).await
}

async fn gateway_scene_eval_pack(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(query): Query<crate::pages::SceneEvalPackQuery>,
) -> Response {
    let app_id = query.app.trim();
    if !app_id.is_empty() {
        if let Some(response) = crate::app_runtime_proxy::maybe_proxy_app_request(
            &http,
            app_id,
            Method::GET,
            uri.path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.path()),
            &headers,
            None,
            principal.as_ref().map(|p| (**p).clone()),
        )
        .await
        {
            return response;
        }
    }
    api_scene_eval_pack(State(http.shell), principal, Query(query)).await
}

async fn api_datasets_query_with_app(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    api_datasets_query_inner(http, principal, app_id.as_str(), body).await
}

async fn api_datasets_query_inner(
    http: HostHttpState,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    app_id: &str,
    body: serde_json::Value,
) -> Response {
    let (ceiling_slug, plug_ds, runtime_identity) = {
        let guard = http.shell.read().expect("state lock");
        if !guard.data_mode_ceiling_for(app_id).allows_eval_api() {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": format!(
                        "datasets eval API unavailable under data mode ceiling `{}`",
                        guard.data_mode_ceiling_for(app_id).slug()
                    )
                })),
            )
                .into_response();
        }
        let plug_ds = guard.plug_ds_endpoint_for(app_id).map(str::to_string);
        let supervisor = http.app_runtime.lock().ok();
        let runtime_identity = supervisor.as_ref().and_then(|slot| {
            crate::state::runtime_identity_for_app(
                &guard,
                slot,
                app_id,
                principal.as_ref().map(|p| (**p).clone()),
            )
        });
        (
            guard.data_mode_ceiling_for(app_id).slug().to_string(),
            plug_ds,
            runtime_identity,
        )
    };
    let _ = ceiling_slug;
    let path = format!("/api/datasets/query/{app_id}");
    match crate::app_runtime_proxy::resolve_datasets_proxy_target(
        app_id,
        runtime_identity.as_ref(),
        plug_ds.as_deref(),
    ) {
        crate::app_runtime_proxy::DatasetsProxyTarget::AppRuntime(identity) => {
            crate::app_runtime_proxy::proxy_post_json(&identity, path.as_str(), body).await
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::PlugDs(endpoint) => {
            crate::plug_proxy::proxy_post_json(endpoint.as_str(), path.as_str(), body).await
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::RuntimeRequired => {
            crate::legacy_compat::runtime_required_unavailable_response(app_id, "datasets/query")
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no dataset endpoint for app `{app_id}`")})),
        )
            .into_response(),
    }
}

async fn api_datasets_query(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let app_id = {
        let guard = http.shell.read().expect("state lock");
        guard.default_app().map(str::to_string)
    };
    let Some(app_id) = app_id else {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "Access data plane is unconfigured"})),
        )
            .into_response();
    };
    api_datasets_query_inner(http, principal, app_id.as_str(), body).await
}

async fn api_datasets_metrics(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let (plug_ds, runtime_identity) = {
        let guard = http.shell.read().expect("state lock");
        if !guard.data_mode_ceiling_for(app_id.as_str()).allows_eval_api() {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": format!(
                        "datasets metrics API unavailable under data mode ceiling `{}`",
                        guard.data_mode_ceiling_for(app_id.as_str()).slug()
                    )
                })),
            )
                .into_response();
        }
        let plug_ds = guard
            .plug_ds_endpoint_for(app_id.as_str())
            .map(str::to_string);
        let supervisor = http.app_runtime.lock().ok();
        let runtime_identity = supervisor.as_ref().and_then(|slot| {
            crate::state::runtime_identity_for_app(
                &guard,
                slot,
                app_id.as_str(),
                principal.as_ref().map(|p| (**p).clone()),
            )
        });
        (plug_ds, runtime_identity)
    };
    let path = format!("/api/datasets/metrics/{app_id}");
    match crate::app_runtime_proxy::resolve_datasets_proxy_target(
        app_id.as_str(),
        runtime_identity.as_ref(),
        plug_ds.as_deref(),
    ) {
        crate::app_runtime_proxy::DatasetsProxyTarget::AppRuntime(identity) => {
            crate::app_runtime_proxy::proxy_post_json(&identity, path.as_str(), body).await
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::PlugDs(endpoint) => {
            crate::plug_proxy::proxy_post_json(endpoint.as_str(), path.as_str(), body).await
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::RuntimeRequired => {
            crate::legacy_compat::runtime_required_unavailable_response(
                app_id.as_str(),
                "datasets/metrics",
            )
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no dataset endpoint for app `{app_id}`")})),
        )
            .into_response(),
    }
}

async fn api_datasets_fixture(
    State(http): State<HostHttpState>,
    principal: Option<Extension<mei_host_auth::AuthPrincipal>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let (plug_ds, runtime_identity) = {
        let guard = http.shell.read().expect("state lock");
        if matches!(
            guard.data_mode_ceiling,
            mei_lang_kernel::DataModeCeiling::Static
        ) {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "fixture datasets API unavailable under static data mode ceiling"
                })),
            )
                .into_response();
        }
        let plug_ds = guard
            .plug_ds_endpoint_for(app_id.as_str())
            .map(str::to_string);
        let supervisor = http.app_runtime.lock().ok();
        let runtime_identity = supervisor.as_ref().and_then(|slot| {
            crate::state::runtime_identity_for_app(
                &guard,
                slot,
                app_id.as_str(),
                principal.as_ref().map(|p| (**p).clone()),
            )
        });
        (plug_ds, runtime_identity)
    };
    let path = format!("/api/datasets/fixture/{app_id}");
    match crate::app_runtime_proxy::resolve_datasets_proxy_target(
        app_id.as_str(),
        runtime_identity.as_ref(),
        plug_ds.as_deref(),
    ) {
        crate::app_runtime_proxy::DatasetsProxyTarget::AppRuntime(identity) => {
            crate::app_runtime_proxy::proxy_post_json(&identity, path.as_str(), body).await
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::PlugDs(_)
        | crate::app_runtime_proxy::DatasetsProxyTarget::None => {
            let guard = http.shell.read().expect("state lock");
            let scene_id = body
                .get("scene_id")
                .or_else(|| body.get("sceneId"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("home");
            let workspace = guard.ctx.workspace_root.as_path();
            let Some(manifest) =
                mei_host_graph::read_client_bootstrap(workspace, app_id.as_str(), scene_id)
            else {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("fixture bootstrap missing for scene `{scene_id}`")})),
                )
                    .into_response();
            };
            (
                StatusCode::OK,
                Json(json!({
                    "source": "fixture",
                    "app_id": app_id,
                    "scene_id": scene_id,
                    "client_revision": manifest.client_revision,
                    "metrics": manifest.metrics,
                })),
            )
                .into_response()
        }
        crate::app_runtime_proxy::DatasetsProxyTarget::RuntimeRequired => {
            crate::legacy_compat::runtime_required_unavailable_response(
                app_id.as_str(),
                "datasets/fixture",
            )
        }
    }
}

#[derive(serde::Deserialize)]
struct ThemeStyleQuery {
    theme: Option<String>,
}

async fn api_ops_theme_style(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    Query(query): Query<ThemeStyleQuery>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let app_ctx = guard.host_ctx_for_app(app_id.as_str());
    let app_root = app_ctx.app_root();
    let live_config =
        load_mei_config_for_app(app_root.as_path(), Some(guard.ctx.workspace_root.as_path()));
    let theme_id = query
        .theme
        .as_deref()
        .and_then(decode_theme_ref_token)
        .unwrap_or_else(|| "cockpit".to_string());
    let css = scene_theme_style_for_theme_id(theme_id.as_str(), Some(&live_config));
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        css,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, RwLock};

    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use mei_host_core::HostContext;
    use tower::ServiceExt;

    fn test_state(workspace: std::path::PathBuf) -> HostHttpState {
        test_state_with_plug_endpoint(workspace, "http://127.0.0.1:9528")
    }

    fn test_state_with_plug_endpoint(
        workspace: std::path::PathBuf,
        plug_ds_endpoint: &str,
    ) -> HostHttpState {
        test_state_with_plug_apps(workspace, &[("data-demo", plug_ds_endpoint)])
    }

    fn test_state_with_plug_apps(
        workspace: std::path::PathBuf,
        apps: &[(&str, &str)],
    ) -> HostHttpState {
        let mut plug_ds_by_app = std::collections::BTreeMap::new();
        for (app_id, endpoint) in apps {
            plug_ds_by_app.insert(app_id.to_string(), endpoint.to_string());
        }
        let default_app = apps
            .first()
            .map(|(app_id, _)| app_id.to_string())
            .unwrap_or_else(|| "data-demo".to_string());
        let plug_ds_endpoint = apps
            .first()
            .map(|(_, endpoint)| endpoint.to_string())
            .unwrap_or_default();
        let shell = Arc::new(RwLock::new(crate::state::ShellState {
            ctx: HostContext::new(workspace.clone(), default_app),
            default_app_id: Some(
                apps.first()
                    .map(|(app_id, _)| app_id.to_string())
                    .unwrap_or_else(|| "data-demo".to_string()),
            ),
            selected_profile_id: Some("default".to_string()),
            selected_profile_file: Some("workspace.json".to_string()),
            selected_profile_revision: Some("test".to_string()),
            selected_profile_source: Some("workspace_default".to_string()),
            data_plane_enabled: true,
            package_root: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            plug_ds_endpoint,
            plug_ds_by_app,
            plug_ds_managed: false,
            app_runtime_by_instance: std::collections::BTreeMap::new(),
            app_runtime_started_at_ms: std::collections::BTreeMap::new(),
            launch_manifest: mei_host_core::LaunchManifest::empty(),
            route_plane_ready: false,
            imported: true,
            warmed_up: true,
            host_started_at_ms: 1,
            ops_job: None,
            last_ops_job: None,
            cleanup_preview: None,
            events: crate::state::host_event_channel(),
            startup_phase: "ready".to_string(),
            startup_detail: None,
            startup_error: None,
            app_materialization: std::collections::BTreeMap::new(),
            data_mode_ceiling: mei_lang_kernel::DataModeCeiling::Eval,
            data_mode_ceiling_by_app: std::collections::BTreeMap::new(),
        }));
        HostHttpState {
            shell,
            auth: mei_host_auth::AuthServeState::new(
                workspace,
                mei_host_auth::AuthEnforcement::Disabled,
            ),
            managed_plug: Arc::new(Mutex::new(None)),
            app_runtime: Arc::new(Mutex::new(None)),
        }
    }

    fn write_generation_fixture(workspace: &std::path::Path, app_id: &str, generation: &str) {
        let app_root = workspace.join("apps").join(app_id);
        std::fs::create_dir_all(app_root.join("src")).expect("app dir");
        std::fs::write(
            app_root.join("app.config.json"),
            format!(r#"{{"schemaVersion":1,"app":{{"id":"{app_id}"}}}}"#),
        )
        .expect("app config");
        let env_dir = app_root.join("env").join(generation);
        std::fs::create_dir_all(env_dir.join("build/exchange")).expect("build dir");
        std::fs::write(
            env_dir
                .join("build/exchange")
                .join(format!("{app_id}.meibundle")),
            b"fixture",
        )
        .expect("bundle fixture");
        std::fs::create_dir_all(env_dir.join("var")).expect("var dir");
        mei_lang_kernel::write_build_manifest(
            env_dir.as_path(),
            &mei_lang_kernel::BuildManifest {
                schema_version: mei_lang_kernel::BUILD_MANIFEST_SCHEMA.to_string(),
                env_version: generation.to_string(),
                app_id: app_id.to_string(),
                toolchain_version: "test-toolchain".to_string(),
                build_generation: Some(generation.to_string()),
                workspace_version: Some("20260712".to_string()),
                config_digest: Some("config-r1".to_string()),
                source_revision: None,
                stock_revision: None,
                finished_at: "2026-07-12T00:00:00Z".to_string(),
            },
        )
        .expect("manifest");
    }

    #[tokio::test]
    async fn api_host_version_returns_binary_and_workspace_descriptor() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/version")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            value["binary"]["build_version"].as_str(),
            Some(crate::build_info::BUILD_VERSION)
        );
        assert_eq!(value["hostStartedAtMs"], 1);
        assert!(value.get("workspace").is_some());
    }

    #[tokio::test]
    async fn api_host_ops_status_exposes_host_shell_ops_flag() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/ops/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["hostShellOps"], true);
        assert!(value.get("phase").is_some());
        assert!(value.get("env").is_some());
    }

    #[tokio::test]
    async fn api_host_mrg_activate_requires_scope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/mrg/activate?scope=")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn api_host_mrg_activate_reports_unreachable_plug_ds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state_with_plug_endpoint(
            tmp.path().to_path_buf(),
            "http://127.0.0.1:1",
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/mrg/activate?scope=home&hops=1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["scope"], "home");
        assert_eq!(value["hops"], 1);
        assert!(value["error"]
            .as_str()
            .unwrap_or("")
            .contains("plug-ds unreachable"));
    }

    #[tokio::test]
    async fn api_datasets_metrics_routes_by_url_app_not_default_ctx() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state_with_plug_apps(
            tmp.path().to_path_buf(),
            &[
                ("data-demo", "http://127.0.0.1:9001"),
                ("mini-park", "http://127.0.0.1:1"),
            ],
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/datasets/metrics/mini-park")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"metricId":"test"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "mini-park metrics should route to its plug-ds pool entry, not reject as unknown app"
        );
    }

    #[tokio::test]
    async fn api_host_heartbeat_lists_discovered_apps() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("apps/data-demo")).expect("mkdir app");
        std::fs::create_dir_all(tmp.path().join("apps/mini-park")).expect("mkdir app");
        std::fs::write(
            tmp.path().join("apps/data-demo/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"data-demo"}}"#,
        )
        .expect("write app.config");
        std::fs::write(
            tmp.path().join("apps/mini-park/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"mini-park"}}"#,
        )
        .expect("write app.config");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","version":"20260628","defaultApp":"data-demo"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state_with_plug_apps(
            tmp.path().to_path_buf(),
            &[
                ("data-demo", "http://127.0.0.1:9001"),
                ("mini-park", "http://127.0.0.1:9002"),
            ],
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/heartbeat")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["defaultAppId"], "data-demo");
        let apps = value["discoveredApps"].as_array().expect("discoveredApps");
        assert_eq!(apps.len(), 2);
        assert!(apps.iter().any(|app| app["appId"] == "mini-park"));
    }

    #[tokio::test]
    async fn workspace_profile_routes_list_and_report_revision_conflict() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"workspace":{"id":"test"},"future":{"kept":true}}"#,
        )
        .expect("workspace config");
        std::fs::create_dir_all(tmp.path().join("configs")).expect("configs");
        std::fs::write(
            tmp.path().join("configs/local.json"),
            r#"{"workspace":{"id":"local"}}"#,
        )
        .expect("local config");

        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/workspace-profiles")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["profiles"].as_array().map(Vec::len), Some(2));

        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/host/workspace-profiles/default")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"expectedRevision":"stale","config":{"future":{"kept":true}}}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["error"]["code"], "revision_conflict");
        assert!(value["error"]["details"]["currentRevision"].is_string());
    }

    #[tokio::test]
    async fn apply_profile_rejects_concurrent_ops_job() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace config");
        let state = test_state(tmp.path().to_path_buf());
        {
            let mut guard = state.shell.write().expect("state lock");
            crate::build_ops::begin_ops_job(&mut guard, "prebuild").expect("start job");
        }
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/runtime/apply-profile")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"profileId":"default","expectedRevision":"ignored"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn builds_request_rejects_concurrent_ops_job() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace config");
        let state = test_state(tmp.path().to_path_buf());
        {
            let mut guard = state.shell.write().expect("state lock");
            crate::build_ops::begin_ops_job(&mut guard, "prebuild").expect("start job");
        }
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/builds/request")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                          "schemaVersion":"mei-build-request-v1",
                          "profileId":"local",
                          "profileRevision":"r1",
                          "profileFile":"configs/local.json",
                          "apps":["mini-data"]
                        }"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn generation_activate_rejects_missing_target_app() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"workspace":{"defaultApp":"app-a"}}"#,
        )
        .expect("workspace");
        write_generation_fixture(tmp.path(), "app-a", "WS-20260712.0");
        std::fs::create_dir_all(tmp.path().join("apps/app-b/src")).expect("app-b");
        std::fs::write(
            tmp.path().join("apps/app-b/app.config.json"),
            r#"{"app":{"id":"app-b"}}"#,
        )
        .expect("app-b config");
        let response = router(test_state(tmp.path().to_path_buf()))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/builds/WS-20260712.0/activate")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn generation_cleanup_requires_matching_preview_token() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"workspace":{"defaultApp":"app-a"},"build":{"retainBuildGenerations":1}}"#,
        )
        .expect("workspace");
        write_generation_fixture(tmp.path(), "app-a", "WS-20260711.0");
        write_generation_fixture(tmp.path(), "app-a", "WS-20260712.0");
        let state = test_state(tmp.path().to_path_buf());
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/builds/cleanup-preview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let preview: serde_json::Value = serde_json::from_slice(&body).expect("preview json");
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/builds/cleanup")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "previewToken": "wrong-token",
                            "revision": preview["revision"],
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        write_generation_fixture(tmp.path(), "app-a", "WS-20260713.0");
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/builds/cleanup")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "previewToken": preview["previewToken"],
                            "revision": preview["revision"],
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(tmp.path().join("apps/app-a/env/WS-20260711.0").is_dir());
    }

    #[tokio::test]
    async fn generation_cleanup_preview_rejects_running_job() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace");
        let state = test_state(tmp.path().to_path_buf());
        {
            let mut guard = state.shell.write().expect("state lock");
            crate::build_ops::begin_ops_job(&mut guard, "prebuild").expect("start job");
        }
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/builds/cleanup-preview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn launch_manifest_and_instances_endpoints_list_observed_state() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace");
        let mut manifest = mei_host_core::LaunchManifest::empty();
        manifest.instances.insert(
            "inst-a".to_string(),
            mei_host_core::DesiredInstance {
                spec_ref: "sha:a".to_string(),
                desired_state: mei_host_core::DesiredState::Running,
            },
        );
        manifest.routes.insert(
            "mini-data".to_string(),
            mei_host_core::RouteBinding {
                active: Some("inst-a".to_string()),
                candidate: None,
                previous: None,
            },
        );
        manifest = manifest.with_recomputed_revision();
        let revision = manifest.revision.clone();
        let state = test_state(tmp.path().to_path_buf());
        {
            let mut guard = state.shell.write().expect("state lock");
            guard.install_launch_manifest(manifest);
        }
        let app = router(state);
        let manifest_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/host/launch-manifest")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(manifest_response.status(), StatusCode::OK);
        let manifest_body = http_body_util::BodyExt::collect(manifest_response.into_body())
            .await
            .expect("body")
            .to_bytes();
        let manifest_json: serde_json::Value =
            serde_json::from_slice(&manifest_body).expect("json");
        assert_eq!(manifest_json["revision"], revision);
        assert_eq!(
            manifest_json["manifest"]["routes"]["mini-data"]["active"],
            "inst-a"
        );

        let instances_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/instances")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(instances_response.status(), StatusCode::OK);
        let instances_body = http_body_util::BodyExt::collect(instances_response.into_body())
            .await
            .expect("body")
            .to_bytes();
        let instances_json: serde_json::Value =
            serde_json::from_slice(&instances_body).expect("json");
        assert_eq!(instances_json["revision"], revision);
        assert_eq!(instances_json["instances"][0]["instanceId"], "inst-a");
        assert_eq!(instances_json["instances"][0]["appId"], "mini-data");
        assert_eq!(instances_json["instances"][0]["routeRole"], "active");
    }

    #[tokio::test]
    async fn route_cutover_rejects_stale_manifest_revision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace");
        let mut manifest = mei_host_core::LaunchManifest::empty();
        manifest.instances.insert(
            "inst-new".to_string(),
            mei_host_core::DesiredInstance {
                spec_ref: "s".to_string(),
                desired_state: mei_host_core::DesiredState::Running,
            },
        );
        manifest.routes.insert(
            "mini-data".to_string(),
            mei_host_core::RouteBinding {
                active: Some("inst-old".to_string()),
                candidate: Some("inst-new".to_string()),
                previous: None,
            },
        );
        manifest = manifest.with_recomputed_revision();
        let control = mei_host_core::HostControlState::new(manifest.clone());
        mei_host_core::write_host_control_state(tmp.path(), &control).expect("control");
        let state = test_state(tmp.path().to_path_buf());
        {
            let mut guard = state.shell.write().expect("lock");
            guard.install_launch_manifest(manifest);
            guard
                .app_runtime_by_instance
                .insert("inst-new".to_string(), "http://127.0.0.1:1".to_string());
        }
        let response = router(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/routes/mini-data/cutover")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"instanceId":"inst-new","expectedManifestRevision":"stale"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn route_rollback_switches_active_to_previous() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace");
        let old = mei_host_core::InstanceSpec {
            schema_version: mei_host_core::SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: "inst-old".to_string(),
            app_id: "mini-data".to_string(),
            bundle: mei_host_core::BundleRef {
                generation: "WS-20260711.0".to_string(),
                bundle_path: "apps/mini-data/env/WS-20260711.0".to_string(),
                digest: None,
                toolchain_version: None,
                config_digest: None,
            },
            config_snapshot: mei_host_core::ConfigSnapshot {
                profile_id: "local".to_string(),
                profile_revision: "r1".to_string(),
                profile_file: "configs/local.json".to_string(),
                runtime_plan: mei_lang_kernel::RuntimePlan {
                    default_mode: mei_lang_kernel::RuntimeMode::Lazy,
                    apps: Default::default(),
                },
                default_app: None,
                ..Default::default()
            },
            runtime_abi: "1".to_string(),
            data_mode_ceiling: None,
        };
        mei_host_core::write_instance_spec(tmp.path(), &old).expect("spec");
        let mut manifest = mei_host_core::LaunchManifest::empty();
        for id in ["inst-old", "inst-new"] {
            manifest.instances.insert(
                id.to_string(),
                mei_host_core::DesiredInstance {
                    spec_ref: "s".to_string(),
                    desired_state: mei_host_core::DesiredState::Running,
                },
            );
        }
        manifest.routes.insert(
            "mini-data".to_string(),
            mei_host_core::RouteBinding {
                active: Some("inst-new".to_string()),
                candidate: None,
                previous: Some("inst-old".to_string()),
            },
        );
        manifest = manifest.with_recomputed_revision();
        mei_host_core::write_host_control_state(
            tmp.path(),
            &mei_host_core::HostControlState::new(manifest.clone()),
        )
        .expect("control");
        let state = test_state(tmp.path().to_path_buf());
        {
            let mut guard = state.shell.write().expect("lock");
            guard.install_launch_manifest(manifest);
            guard
                .app_runtime_by_instance
                .insert("inst-old".to_string(), "http://127.0.0.1:1".to_string());
            guard
                .app_runtime_by_instance
                .insert("inst-new".to_string(), "http://127.0.0.1:2".to_string());
        }
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/routes/mini-data/rollback")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let loaded = mei_host_core::read_host_control_state(tmp.path())
            .expect("control")
            .launch_manifest;
        assert_eq!(
            loaded.routes["mini-data"].active.as_deref(),
            Some("inst-old")
        );
        assert_eq!(
            loaded.routes["mini-data"].previous.as_deref(),
            Some("inst-new")
        );
    }

    #[tokio::test]
    async fn failed_apply_validation_does_not_change_active_profile() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("workspace.json"), "{}").expect("workspace config");
        std::fs::create_dir_all(tmp.path().join("deploy/state")).expect("state dir");
        let active_path = tmp.path().join("deploy/state/host-control.json");
        let active = r#"{"schemaVersion":"mei-host-control-v1","activeProfile":{"id":"old","revision":"r0"}}"#;
        std::fs::write(&active_path, active).expect("active state");
        let response = router(test_state(tmp.path().to_path_buf()))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/runtime/apply-profile")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"profileId":"default","expectedRevision":"stale"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            std::fs::read_to_string(active_path).expect("active state"),
            active
        );
    }

    #[tokio::test]
    async fn host_events_streams_named_sse_event() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = test_state(tmp.path().to_path_buf());
        let shell = state.shell.clone();
        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/host/events")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        {
            let guard = shell.read().expect("state lock");
            let _ = guard.events.send(crate::state::HostEvent::new(
                "revision-published",
                serde_json::json!({"appId": "data-demo", "revision": "r1"}),
            ));
        }
        let mut body = response.into_body();
        let frame = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            http_body_util::BodyExt::frame(&mut body),
        )
        .await
        .expect("sse timeout")
        .expect("sse body")
        .expect("sse frame");
        let data = frame.into_data().expect("sse data");
        let text = String::from_utf8_lossy(data.as_ref());
        assert!(text.contains("event: revision-published"));
        assert!(text.contains("\"revision\":\"r1\""));
    }

    #[tokio::test]
    async fn build_context_export_route_is_registered() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/build/context/export?app_id=pretty-panels&node=&tab=overview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("Mei Build Context"),
            "expected handler markdown error body, got: {text}"
        );
    }

    #[tokio::test]
    async fn build_workspace_fragment_route_removed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/build/workspace-fragment?app_id=pretty-panels&node=&tab=preview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn scene_manifest_route_requires_app_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/host/scene-manifest")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn layer_batch_route_requires_app_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/host/layer-batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"layers":["structure.full"]}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn host_home_route_returns_shell_page() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/home")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("MeiLang 工作区") || html.contains("mei-workspace-page"));
        assert!(html.contains("topbar-shell"));
        assert!(html.contains("statusbar-shell"));
        assert!(html.contains("host-shell.css"));
        assert!(html.contains("/config"));
    }

    #[tokio::test]
    async fn empty_workspace_control_plane_binds_pages_and_profile_apis() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"first-boot"}}"#,
        )
        .expect("workspace");
        let shell = Arc::new(RwLock::new(crate::state::ShellState::new(
            tmp.path().to_path_buf(),
            String::new(),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            std::collections::BTreeMap::new(),
            false,
        )));
        let selected = crate::workspace_profile_api::resolve_runtime_profile(tmp.path(), None)
            .expect("resolve")
            .expect("default");
        crate::workspace_profile_api::install_selected_profile(&shell, Some(&selected));
        {
            let mut guard = shell.write().expect("state");
            guard.startup_phase = "unconfigured".to_string();
            guard.startup_detail = Some("控制面已就绪".to_string());
        }
        let state = HostHttpState {
            shell,
            auth: mei_host_auth::AuthServeState::new(
                tmp.path().to_path_buf(),
                mei_host_auth::AuthEnforcement::Disabled,
            ),
            managed_plug: Arc::new(Mutex::new(None)),
            app_runtime: Arc::new(Mutex::new(None)),
        };

        for uri in [
            "/home",
            "/runtime",
            "/config",
            "/api/host/runtime/profile",
            "/api/host/workspace-profiles",
        ] {
            let response = router(state.clone())
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
        }

        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/host/access-readiness?app=missing&scene=home")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["ready"], false);
        assert_eq!(value["reason"], "unconfigured");
        assert!(!tmp.path().join("apps").exists());
        assert!(!tmp.path().join("deploy/state/host-control.json").exists());
    }

    #[tokio::test]
    async fn host_config_route_without_app_shows_picker() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("apps/data-demo")).expect("mkdir app");
        std::fs::write(
            tmp.path().join("apps/data-demo/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"data-demo"}}"#,
        )
        .expect("write app.config");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("write workspace.json");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/config")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("请选择要管理的应用"));
        assert!(html.contains("data-demo"));
        assert!(html.contains("topbar-shell"));
    }

    #[tokio::test]
    async fn root_redirects_to_home() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok()),
            Some("/home")
        );
    }

    #[tokio::test]
    async fn legacy_host_upload_redirects_to_shell_upload() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/host/upload?app=demo")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok()),
            Some("/upload?app=demo")
        );
    }

    #[tokio::test]
    async fn legacy_apps_access_redirects_to_stage_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/apps/access/pretty-panels")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok()),
            Some("/apps/pretty-panels/home")
        );
    }

    #[tokio::test]
    async fn legacy_apps_layout_redirects_to_access_stage() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/apps/demo/layout?scene=home&tab=preview")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok()),
            Some("/apps/demo/home")
        );
    }

    #[tokio::test]
    async fn legacy_apps_app_redirects_to_access_stage() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = router(test_state(tmp.path().to_path_buf()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/apps/demo/app?scene=home")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok()),
            Some("/apps/demo/home")
        );
    }
}
