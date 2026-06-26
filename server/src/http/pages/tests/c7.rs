use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use super::app::{app_page, index, AppQuery};
use super::app_render::prepare_landing_artifacts_for_serve;
use super::assets::{app_bundle, workspace_app_asset};
use super::static_serve::content_type_for_path;
use mei_lang_kernel::CompileOptions;
use mei_lang_toolchain as toolchain;

use crate::{agent_runtime, auth::AuthEnforcement, mei_agent, AppState};
use axum::{
    body::to_bytes,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

fn seed_prebuilt_default_scope_artifact(source_root: &std::path::Path, app_id: &str) {
    let components_root = toolchain::resolve_components_root(source_root);
    toolchain::compile_app_with_cache(
        source_root,
        app_id,
        CompileOptions::default(),
        components_root.as_path(),
    )
    .unwrap_or_else(|failure| {
        panic!(
            "seed prebuilt default-scope artifact for test: {}",
            failure.error
        )
    });
}

const VALID_APP_SOURCE: &str = r#"
app(
    id = "good-app",
    default_scene = "home",
)

scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
)

world(
    id = "home_world",
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#;

const MULTI_SCENE_APP_SOURCE: &str = r##"
app(
    id = "multi-scene",
    default_scene = "home",
)

app_add_scene(scene = scene_ref(scene_file = "details.mei", scene_id = "details"))

scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
)

world(
    id = "home_world",
    resources = [
        resource(id = "home_doc", kind = "document", content = "# HOME_VIEW"),
    ],
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "home_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("home_doc")),
    ],
)
"##;

const DETAILS_SCENE_SOURCE: &str = r##"
scene(
    profile = "page",
    summary = "details scene",
)

world()

world.add_resource(
    resource(
        id = "details_doc",
        kind = "document",
        content = "# DETAILS_VIEW",
    ),
)

frame()

frame.set_layout(
    flex(direction = "column"),
)

frame.add_panel(
    id = "details_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("details_doc")),
    ],
)
"##;

const ACCESS_DISABLED_APP_SOURCE: &str = r##"
app(
    id = "access-disabled",
    default_scene = "home",
)

scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
    access_export = False,
)

world(
    id = "home_world",
    resources = [
        resource(id = "home_doc", kind = "document", content = "# ACCESS_DISABLED"),
    ],
)

frame(
    id = "home_frame",
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "home_panel",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("home_doc")),
    ],
)
"##;

fn host_surface_env_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScopedAccessOnlySurface {
    _guard: MutexGuard<'static, ()>,
}

impl ScopedAccessOnlySurface {
    fn new() -> Self {
        let guard = host_surface_env_mutex().lock().expect("lock env guard");
        unsafe {
            std::env::set_var("MEI_HOST_SURFACE", "access-only");
        }
        Self { _guard: guard }
    }
}

impl Drop for ScopedAccessOnlySurface {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("MEI_HOST_SURFACE");
        }
    }
}

#[tokio::test]
async fn dataset_metric_api_echoes_scene_id() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let state = crate::test_support::test_app_state().expect("app state");
    let app = crate::http::router().with_state(state);
    let payload = serde_json::json!({
        "scene_id": "manage_query_state",
        "dataset_id": "orders",
        "metric_ids": ["orders_overview"]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/datasets/metrics/examples%2Fds%2F04-data-table-features")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["scene_id"], "manage_query_state");
    assert_eq!(v["dataset_id"], "orders");
}

#[tokio::test]
async fn http_dataset_query_aligns_with_toolchain_access_query() {
    use axum::{
        body::to_bytes,
        body::Body,
        http::{Request, StatusCode},
    };
    use std::collections::BTreeMap;
    use tower::ServiceExt;

    let state = crate::test_support::test_app_state().expect("app state");
    let source_root = state.source_root.as_path();
    let app_id = "examples/ds/01-dataset-baseline";
    mei_lang_toolchain::clear_compile_cache_for_app(source_root, app_id);
    let toolchain_payload = mei_lang_toolchain::query_world_dataset(
        source_root,
        app_id,
        None,
        "sales_data",
        None,
        &BTreeMap::new(),
        None,
        None,
        None,
    )
    .expect("toolchain dataset query");

    let app = crate::http::router().with_state(state);
    let payload = serde_json::json!({
        "scene_id": "home",
        "dataset_id": "sales_data",
        "page": 1,
        "page_size": 5
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/datasets/query/examples%2Fds%2F01-dataset-baseline")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let http_payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(http_payload["dataset_id"], toolchain_payload["id"]);
    assert_eq!(http_payload["scene_id"], toolchain_payload["scene_id"]);
    assert!(http_payload["total"].as_u64().unwrap_or(0) > 0);
    assert!(toolchain_payload["sample_rows"].as_array().unwrap().len() > 0);
}

#[test]
fn scene_query_coords_builds_compile_options_with_scene_and_focus() {
    use super::scene_qualified::{compile_options_from_coords, SceneQueryCoords};

    let coords = SceneQueryCoords::from_parts(
        Some("home".to_string()),
        Some("scenes/widgets/foo.mei".to_string()),
    );
    let opts = compile_options_from_coords(&coords);
    assert_eq!(opts.scene.as_deref(), Some("home"));
    assert_eq!(
        opts.preview_target.as_deref(),
        Some("scenes/widgets/foo.mei")
    );
}

fn unique_test_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("mei-lang-server-{label}-{nonce}"))
}
