use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mei_app_runtime::{router, AppRuntimeServeState};
use mei_host_core::{
    BundleRef, ConfigSnapshot, HostContext, InstanceSpec, SCHEMA_INSTANCE_SPEC_V1,
};
use mei_lang_kernel::{RuntimeMode, RuntimePlan};
use tower::ServiceExt;

fn sample_spec(app_id: &str, instance_id: &str, generation: &str) -> InstanceSpec {
    InstanceSpec {
        schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
        instance_id: instance_id.to_string(),
        app_id: app_id.to_string(),
        bundle: BundleRef {
            generation: generation.to_string(),
            bundle_path: format!("apps/{app_id}/env/{generation}"),
            digest: None,
            toolchain_version: None,
            config_digest: Some("cfg-test".to_string()),
        },
        config_snapshot: ConfigSnapshot {
            profile_id: "local".to_string(),
            profile_revision: "r1".to_string(),
            profile_file: String::new(),
            runtime_plan: RuntimePlan {
                default_mode: RuntimeMode::Lazy,
                apps: Default::default(),
            },
            default_app: Some(app_id.to_string()),
        },
        runtime_abi: "2.4".to_string(),
        data_mode_ceiling: None,
    }
}

fn test_state(workspace: std::path::PathBuf) -> mei_app_runtime::SharedRuntimeState {
    let app_id = "demo";
    let host = HostContext::new(workspace, app_id);
    let spec = sample_spec(app_id, "inst-test", "WS-20260712.1");
    let state = AppRuntimeServeState::new(host, spec, "secret-token");
    state.set_phase(mei_host_core::InstancePhase::Ready);
    state.shared()
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn health_ok_without_token() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = router(test_state(tmp.path().to_path_buf()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/app-runtime/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["ok"], true);
    assert_eq!(value["plug"], "app-runtime");
}

#[tokio::test]
async fn token_rejected_on_meta() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = router(test_state(tmp.path().to_path_buf()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/app-runtime/meta")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn meta_fields_with_valid_token() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let digest = state.spec_digest();
    let app = router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/app-runtime/meta")
                .header("x-mei-instance-token", "secret-token")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["appId"], "demo");
    assert_eq!(value["generation"], "WS-20260712.1");
    assert_eq!(value["instanceId"], "inst-test");
    assert_eq!(value["specDigest"], digest);
    assert!(value.get("revisions").is_some());
}

#[tokio::test]
async fn plug_ds_health_ok_without_token() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = router(test_state(tmp.path().to_path_buf()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/plug-ds/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["ok"], true);
}

#[tokio::test]
async fn access_thin_shell_requires_token_and_returns_html() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = router(test_state(tmp.path().to_path_buf()));
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/apps/demo/home")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/apps/demo/home")
                .header("x-mei-instance-token", "secret-token")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let html = String::from_utf8(bytes.to_vec()).expect("utf8");
    assert!(html.contains("mei-compose-host"));
    assert!(html.contains("view_revision_envelope"));
    assert!(html.contains("data-mei-app-runtime"));
}

#[test]
fn router_registration_list_is_complete() {
    let paths = mei_app_runtime::registered_route_paths();
    for required in [
        "/api/app-runtime/health",
        "/api/app-runtime/ready",
        "/api/app-runtime/meta",
        "/api/plug-ds/health",
        "/api/datasets/query",
        "/api/host/view-revision",
        "/api/host/scene-manifest",
        "/api/host/layer-batch",
        "/api/host/scene-eval-pack",
        "/api/host/scene-bootstrap",
        "/apps/:app_id",
        "/apps/:app_id/:stage",
    ] {
        assert!(
            paths.contains(&required),
            "missing registered route {required}"
        );
    }
}
