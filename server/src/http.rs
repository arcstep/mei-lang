pub(crate) mod build_api;
pub(crate) mod agent_api;
pub mod auth_api;
mod compile_cache;
mod datasets;
pub(crate) mod host_api;
pub(crate) mod host_error_page;
pub(crate) mod observation;
pub(crate) mod request_trace;
pub mod ops_api;
pub mod pages;
mod runtime_cache;
pub mod projection_api;
pub mod scene_api;
pub(crate) mod scene_bundle;
pub mod upload_api;

use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post, put},
    Router,
};
use serde_json::json;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/app-assets/favicon.svg") }),
        )
        .route("/", get(pages::index))
        .route("/login", get(auth_api::login_page))
        .route("/logout", get(auth_api::logout_page))
        .route(
            "/account/password",
            get(auth_api::account_change_password_page),
        )
        .route("/api/auth/public-key", get(auth_api::auth_public_key))
        .route("/api/auth/session", get(auth_api::auth_session))
        .route("/api/auth/refresh", post(auth_api::auth_refresh))
        .route("/api/auth/login", post(auth_api::auth_login))
        .route("/api/auth/logout", post(auth_api::auth_logout))
        .route("/api/host/ready", get(host_api::api_host_ready))
        .route("/api/host/readiness", get(host_api::api_host_readiness))
        .route("/api/host/heartbeat", get(host_api::api_host_heartbeat))
        .route("/api/host/build", post(host_api::api_host_build))
        .route(
            "/api/build/context/export",
            get(build_api::api_build_context_export),
        )
        .route(
            "/api/host/request-trace",
            get(request_trace::api_request_trace),
        )
        .route(
            "/api/auth/change-password",
            post(auth_api::auth_change_password),
        )
        .route("/apps/:mode/*app_id", get(pages::app_page))
        .route(
            "/api/projection/*app_id",
            get(projection_api::projection_api),
        )
        .route(
            "/api/world/context/*app_id",
            get(scene_api::world_context_api),
        )
        .route(
            "/api/world/assets/*app_id",
            get(scene_api::world_assets_api),
        )
        .route("/api/world/asset/*app_id", get(scene_api::world_asset_api))
        .route(
            "/api/world/runtime/*app_id",
            get(scene_api::world_runtime_api),
        )
        .route("/api/sim/step/*app_id", post(scene_api::sim_step_api))
        .route(
            "/api/datasets/query/*app_id",
            post(pages::dataset_query_api),
        )
        .route(
            "/api/datasets/metrics/*app_id",
            post(pages::dataset_metric_api),
        )
        .route(
            "/api/datasets/recompute/*app_id",
            post(pages::dataset_recompute_api),
        )
        .route("/api/ops/boundary", get(ops_api::ops_boundary_get))
        .route(
            "/api/ops/config/*app_id",
            get(ops_api::ops_config_get).put(ops_api::ops_config_put),
        )
        .route("/api/ops/journal/*app_id", get(ops_api::ops_journal_get))
        .route(
            "/api/upload/init/*app_id",
            post(upload_api::upload_chunk_init_post)
                .layer(DefaultBodyLimit::max(256 * 1024)),
        )
        .route(
            "/api/upload/status/*app_id",
            get(upload_api::upload_chunk_status_get),
        )
        .route(
            "/api/upload/chunk/*app_id",
            put(upload_api::upload_chunk_put)
                .layer(DefaultBodyLimit::max(9 * 1024 * 1024)),
        )
        .route(
            "/api/upload/complete/*app_id",
            post(upload_api::upload_chunk_complete_post)
                .layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/move/*app_id",
            post(upload_api::upload_file_move_post)
                .layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/dir/*app_id",
            post(upload_api::upload_dir_create_post)
                .layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/rename/*app_id",
            post(upload_api::upload_entry_rename_post)
                .layer(DefaultBodyLimit::max(128 * 1024)),
        )
        .route(
            "/api/upload/download/*app_id",
            get(upload_api::upload_file_download_get),
        )
        .route(
            "/api/upload/*app_id",
            post(upload_api::upload_file_post)
                .delete(upload_api::upload_file_delete)
                .layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        .route("/api/agent/config", get(agent_api::api_agent_config))
        .route("/api/agent/runtime", get(agent_api::api_agent_runtime))
        .route("/api/agent/skill", get(agent_api::api_agent_skill))
        .route(
            "/api/agent/skill/sync",
            post(agent_api::api_agent_sync_skill),
        )
        .route(
            "/api/agent/context/preview",
            get(agent_api::api_agent_context_preview),
        )
        .route("/api/agent/health", get(agent_api::api_agent_health))
        .route(
            "/api/agent/model/probe",
            get(agent_api::api_agent_model_probe),
        )
        .route("/api/agent/start", post(agent_api::api_agent_start))
        .route("/api/agent/stop", post(agent_api::api_agent_stop))
        .route(
            "/api/agent/session",
            get(agent_api::api_agent_list_sessions).post(agent_api::api_agent_create_session),
        )
        .route(
            "/api/agent/session/:session_id/message",
            post(agent_api::api_agent_send_message),
        )
        .route(
            "/api/agent/session/:session_id/permissions/pending",
            get(agent_api::api_agent_pending_permissions),
        )
        .route(
            "/api/agent/session/:session_id/events",
            get(agent_api::api_agent_session_events),
        )
        .route(
            "/api/agent/session/:session_id/messages",
            get(agent_api::api_agent_session_messages),
        )
        .route(
            "/api/agent/session/:session_id/diff",
            get(agent_api::api_agent_session_diff),
        )
        .route(
            "/api/agent/session/:session_id/revert",
            post(agent_api::api_agent_revert_session),
        )
        .route(
            "/api/agent/session/:session_id/unrevert",
            post(agent_api::api_agent_unrevert_session),
        )
        .route(
            "/api/agent/session/:session_id/abort",
            post(agent_api::api_agent_abort_session),
        )
        .route(
            "/api/agent/session/:session_id/permissions/:permission_id",
            post(agent_api::api_agent_respond_permission),
        )
        .route("/app-bundles/:mode", get(pages::app_bundle))
        .route("/app-assets/*path", get(pages::app_asset))
        .route("/gis/*path", get(pages::gis_proxy))
        .route(
            "/workspace-app-assets/:app_id/*path",
            get(pages::workspace_app_asset),
        )
        .route("/workspace-components/*path", get(pages::component_asset))
        .fallback(host_error_page::fallback_handler)
}

pub(crate) fn error_response(error: impl std::fmt::Display) -> Response {
    let message = error.to_string();
    tracing::error!(status = %StatusCode::INTERNAL_SERVER_ERROR, error = %message, "request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": message,
            "status": StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        })),
    )
        .into_response()
}
