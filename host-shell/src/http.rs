use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use mei_lang_app::scene_theme_style_for_theme_id;
use mei_lang_kernel::{decode_theme_ref_token, load_mei_config_for_app};
use serde_json::json;

use crate::api_stubs::{
    api_agent_config_stub, api_agent_context_preview_stub, api_agent_runtime_stub,
    api_agent_sessions_stub, api_agent_skill_stub,
};
use crate::build_api::{api_build_context_export, api_build_workspace_fragment};
use crate::assets::{app_asset, app_bundle, component_asset, workspace_app_asset};
use crate::build_info::{self, BUILD_VERSION};
use crate::ops_api::{api_host_ops_prebuild, api_host_ops_reload, api_host_ops_status};
use crate::pages::{
    api_host_access_readiness, api_presentation_map, api_scene_bootstrap, api_scene_fragment,
    api_scene_revision, app_page, host_starting_page, index,
};
use crate::presentation_compile::api_presentation_compile;
use crate::presentation_scripts::{
    api_get_presentation_script, api_list_presentation_scripts,
    api_put_presentation_script, api_set_default_presentation_script,
};
use crate::landing::build_discovered_app_summaries;
use crate::runtime_api::{api_host_mrg_activate, api_host_mrg_status, api_runtime_snapshot};
use crate::state::{HostHttpState, SharedState};
use crate::upload_download::upload_file_download_get;

pub fn router(state: HostHttpState) -> Router {
    Router::new()
        .route(
            "/favicon.ico",
            get(|| async { Redirect::permanent("/app-assets/favicon.svg") }),
        )
        .route("/", get(index))
        .route("/host/starting", get(host_starting_page))
        .route("/login", get(mei_host_auth::login_page))
        .route("/logout", get(mei_host_auth::logout_page))
        .route(
            "/account/password",
            get(mei_host_auth::account_change_password_page),
        )
        .route("/api/host/heartbeat", get(api_host_heartbeat))
        .route("/api/host/version", get(api_host_version))
        .route("/api/host/ready", get(api_host_ready))
        .route("/api/host/readiness", get(api_host_ready))
        .route("/api/host/access-readiness", get(api_host_access_readiness))
        .route("/api/host/ops/status", get(api_host_ops_status))
        .route("/api/host/ops/reload", post(api_host_ops_reload))
        .route("/api/host/ops/prebuild", post(api_host_ops_prebuild))
        .route("/api/runtime/snapshot", get(api_runtime_snapshot))
        .route(
            "/api/build/context/export",
            get(api_build_context_export),
        )
        .route(
            "/api/build/workspace-fragment",
            get(api_build_workspace_fragment),
        )
        .route("/api/host/mrg/status", get(api_host_mrg_status))
        .route("/api/host/mrg/activate", post(api_host_mrg_activate))
        .route("/api/host/scene-revision", get(api_scene_revision))
        .route("/api/host/scene-bootstrap", get(api_scene_bootstrap))
        .route("/api/host/scene-fragment", get(api_scene_fragment))
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
        .route("/api/ops/theme/style/:app_id", get(api_ops_theme_style))
        .route(
            "/api/ops/layout-tuning/overlay/:app_id",
            get(crate::ops_layout_tuning_api::api_ops_layout_tuning_overlay_get),
        )
        .route(
            "/api/ops/layout-tuning/draft/:app_id",
            axum::routing::put(crate::ops_layout_tuning_api::api_ops_layout_tuning_draft_put),
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
        .route(
            "/api/upload/download/:app_id",
            get(upload_file_download_get),
        )
        .route("/gis/*path", get(crate::gis_proxy::gis_proxy))
        .route("/apps/:mode/*app_id", get(app_page))
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
    let default_access_ready = guard.imported;
    let default_warmup_ready = guard.warmed_up;
    let default_phase = if !guard.imported {
        "starting"
    } else if default_warmup_ready {
        "ready"
    } else {
        "bound"
    };
    let default_app_id = guard.ctx.app_id.clone();
    let workspace_root = guard.ctx.workspace_root.as_path();
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
        "ready": default_access_ready,
        "hostReady": default_access_ready,
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
    let (imported, warmed_up, startup_phase, startup_detail, startup_error) = {
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        (
            guard.imported,
            guard.warmed_up,
            guard.startup_phase.clone(),
            guard.startup_detail.clone(),
            guard.startup_error.clone(),
        )
    };
    (
        StatusCode::OK,
        Json(json!({
            "hostReady": imported,
            "accessReady": imported,
            "imported": imported,
            "warmedUp": warmed_up,
            "warmupReady": warmed_up,
            "startupPhase": startup_phase,
            "startupDetail": startup_detail,
            "startupError": startup_error,
        })),
    )
}

async fn api_datasets_query_with_app(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    api_datasets_query_inner(state, app_id.as_str(), body).await
}

async fn api_datasets_query_inner(
    state: SharedState,
    app_id: &str,
    body: serde_json::Value,
) -> Response {
    let endpoint = {
        let guard = state.read().expect("state lock");
        if !guard.data_mode_ceiling.allows_eval_api() {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": format!(
                        "datasets eval API unavailable under data mode ceiling `{}`",
                        guard.data_mode_ceiling.slug()
                    )
                })),
            )
                .into_response();
        }
        match guard.plug_ds_endpoint_for(app_id) {
            Some(endpoint) => endpoint.to_string(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("plug-ds endpoint missing for app `{app_id}`")})),
                )
                    .into_response();
            }
        }
    };
    let path = format!("/api/datasets/query/{app_id}");
    crate::plug_proxy::proxy_post_json(endpoint.as_str(), path.as_str(), body).await
}

async fn api_datasets_query(
    State(state): State<SharedState>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let app_id = {
        let guard = state.read().expect("state lock");
        guard.ctx.app_id.clone()
    };
    api_datasets_query_inner(state, app_id.as_str(), body).await
}

async fn api_datasets_metrics(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let endpoint = {
        let guard = state.read().expect("state lock");
        if !guard.data_mode_ceiling.allows_eval_api() {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": format!(
                        "datasets metrics API unavailable under data mode ceiling `{}`",
                        guard.data_mode_ceiling.slug()
                    )
                })),
            )
                .into_response();
        }
        match guard.plug_ds_endpoint_for(app_id.as_str()) {
            Some(endpoint) => endpoint.to_string(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": format!("plug-ds endpoint missing for app `{app_id}`")})),
                )
                    .into_response();
            }
        }
    };
    let path = format!("/api/datasets/metrics/{app_id}");
    crate::plug_proxy::proxy_post_json(endpoint.as_str(), path.as_str(), body).await
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
            package_root: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            plug_ds_endpoint,
            plug_ds_by_app,
            plug_ds_managed: false,
            imported: true,
            warmed_up: true,
            host_started_at_ms: 1,
            ops_job: None,
            last_ops_job: None,
            startup_phase: "ready".to_string(),
            startup_detail: None,
            startup_error: None,
            app_materialization: std::collections::BTreeMap::new(),
            data_mode_ceiling: mei_lang_kernel::DataModeCeiling::Eval,
        }));
        HostHttpState {
            shell,
            auth: mei_host_auth::AuthServeState::new(
                workspace,
                mei_host_auth::AuthEnforcement::Disabled,
            ),
            managed_plug: Arc::new(Mutex::new(None)),
        }
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
    async fn build_workspace_fragment_route_is_registered() {
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
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
