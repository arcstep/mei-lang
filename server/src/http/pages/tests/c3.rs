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
    default_stage = "home",
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
    default_stage = "home",
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
    default_stage = "home",
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
async fn manage_html_preview_uses_document_iframe() {
    let root = unique_test_root("static-html-manage");
    let app_root = root.join("html-app");
    fs::create_dir_all(app_root.join("demo")).expect("create demo dir");
    fs::write(
        app_root.join("main.mei"),
        VALID_APP_SOURCE.replace("good-app", "html-app"),
    )
    .expect("write main.mei");
    fs::write(
        app_root.join("demo/index.html"),
        "<!doctype html><html><body>MANAGE_HTML</body></html>",
    )
    .expect("write index.html");

    let source_root = Arc::new(root.clone());
    let native_agent =
        Arc::new(mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"));
    let state = AppState {
        package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
        source_root,
        agent_preferred_mode: Arc::new("external".to_string()),
        agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
        agent_auto_start: false,
        auth_enforcement: AuthEnforcement::Disabled,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
        agent_session_context: Arc::new(Mutex::new(HashMap::new())),
        native_agent,
    };

    let response = app_page(
        State(state),
        None,
        AxumPath(("build".to_string(), "html-app".to_string())),
        Query(AppQuery {
            file: Some("demo/index.html".to_string()),
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            node: Some("world-file:demo/index.html".to_string()),
            scope: None,
            focus: None,
            chrome: None,
            catalog: None,
            pack: None,
        }),
    )
    .await
    .expect("render manage html preview");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(html.contains("data-mei-html-document=\"true\""));
    assert!(
        !html.contains("<pre class=\"asset-text-preview"),
        "html preview should not fall back to text pre"
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_root_redirects_to_default_scene_path() {
    let root = unique_test_root("multi-scene-access");
    let app_root = root.join("multi-scene");
    fs::create_dir_all(&app_root).expect("create multi-scene app root");
    fs::write(app_root.join("main.mei"), MULTI_SCENE_APP_SOURCE).expect("write main.mei");
    fs::write(app_root.join("details.mei"), DETAILS_SCENE_SOURCE).expect("write details.mei");

    let source_root = Arc::new(root.clone());
    let native_agent =
        Arc::new(mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"));
    let state = AppState {
        package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
        source_root,
        agent_preferred_mode: Arc::new("external".to_string()),
        agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
        agent_auto_start: false,
        auth_enforcement: AuthEnforcement::Disabled,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
        agent_session_context: Arc::new(Mutex::new(HashMap::new())),
        native_agent,
    };

    let response = app_page(
        State(state),
        None,
        AxumPath(("app".to_string(), "multi-scene".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            node: None,
            scope: None,
            focus: None,
            chrome: Some("none".to_string()),
        }),
    )
    .await
    .expect("render access redirect response");

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/apps/app/multi-scene/scene/home?tab=preview&chrome=none")
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_scene_not_exported_returns_403() {
    let root = unique_test_root("access-scene-not-exported");
    let app_root = root.join("access-disabled");
    fs::create_dir_all(&app_root).expect("create app root");
    fs::write(app_root.join("main.mei"), ACCESS_DISABLED_APP_SOURCE).expect("write main.mei");

    let source_root = Arc::new(root.clone());
    let native_agent =
        Arc::new(mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"));
    let state = AppState {
        package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
        source_root,
        agent_preferred_mode: Arc::new("external".to_string()),
        agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
        agent_auto_start: false,
        auth_enforcement: AuthEnforcement::Disabled,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
        agent_session_context: Arc::new(Mutex::new(HashMap::new())),
        native_agent,
    };

    let response = app_page(
        State(state),
        None,
        AxumPath(("app".to_string(), "access-disabled/scene/home".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            node: None,
            scope: None,
            focus: None,
            chrome: None,
            catalog: None,
            pack: None,
        }),
    )
    .await
    .expect("render access page");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(html.contains("access_export=false"));

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_page_launch_button_targets_presentation_route() {
    let root = unique_test_root("access-launch-presentation");
    let app_root = root.join("multi-scene");
    fs::create_dir_all(&app_root).expect("create app root");
    fs::write(app_root.join("main.mei"), MULTI_SCENE_APP_SOURCE).expect("write main.mei");
    fs::write(app_root.join("details.mei"), DETAILS_SCENE_SOURCE).expect("write details.mei");

    let source_root = Arc::new(root.clone());
    let native_agent =
        Arc::new(mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"));
    let state = AppState {
        package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
        source_root,
        agent_preferred_mode: Arc::new("external".to_string()),
        agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
        agent_auto_start: false,
        auth_enforcement: AuthEnforcement::Disabled,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
        agent_session_context: Arc::new(Mutex::new(HashMap::new())),
        native_agent,
    };

    let response = app_page(
        State(state),
        None,
        AxumPath(("app".to_string(), "multi-scene/scene/home".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            node: None,
            scope: None,
            focus: None,
            chrome: None,
            catalog: None,
            pack: None,
        }),
    )
    .await
    .expect("render access page");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(html.contains("/apps/presentation/multi-scene/scene/home"));

    let _ = fs::remove_dir_all(&root);
}
