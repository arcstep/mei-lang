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
async fn manage_file_scene_route_overrides_conflicting_scene_query() {
    let root = unique_test_root("multi-scene");
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
        AxumPath(("build".to_string(), "multi-scene".to_string())),
        Query(AppQuery {
            file: Some("details.mei".to_string()),
            scene: Some("home".to_string()),
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
    .expect("render app page response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(html.contains("DETAILS_VIEW"));
    assert!(
        html.contains("/apps/app/multi-scene") && html.contains("/scene/details"),
        "expected access URL to use canonical /scene/details path: {}",
        html.chars().take(1200).collect::<String>()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn static_serve_html_content_type() {
    use std::path::Path;

    assert_eq!(
        content_type_for_path(Path::new("prototype/index.html")),
        "text/html; charset=utf-8"
    );
}

#[tokio::test]
async fn access_static_html_file_renders_without_scene_redirect() {
    let root = unique_test_root("static-html-access");
    let app_root = root.join("html-app");
    fs::create_dir_all(app_root.join("demo")).expect("create demo dir");
    fs::write(
        app_root.join("main.mei"),
        VALID_APP_SOURCE.replace("good-app", "html-app"),
    )
    .expect("write main.mei");
    fs::write(
        app_root.join("demo/index.html"),
        "<!doctype html><html><body id=\"proto\">PROTOTYPE</body></html>",
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
        State(state.clone()),
        None,
        AxumPath(("app".to_string(), "html-app".to_string())),
        Query(AppQuery {
            file: Some("demo/index.html".to_string()),
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
    .expect("render access static html");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(
        html.contains("data-mei-html-document=\"true\""),
        "expected html document iframe preview"
    );
    assert!(
        html.contains("/workspace-app-assets/html-app/demo/index.html"),
        "expected workspace asset href for prototype"
    );

    let asset_response = workspace_app_asset(
        State(state),
        HeaderMap::new(),
        AxumPath(("html-app".to_string(), "demo/index.html".to_string())),
    )
    .await
    .expect("serve workspace html asset");
    assert_eq!(asset_response.status(), StatusCode::OK);
    assert_eq!(
        asset_response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_mei_file_query_still_strips_file_param() {
    let root = unique_test_root("access-mei-file-strip");
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
            file: Some("details.mei".to_string()),
            scene: Some("details".to_string()),
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
    .expect("render access mei file redirect");

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        !location.contains("file="),
        "access mei file should strip file query: {location}"
    );
    assert!(
        location.contains("/scene/"),
        "access mei file should redirect to canonical scene path: {location}"
    );

    let _ = fs::remove_dir_all(&root);
}
