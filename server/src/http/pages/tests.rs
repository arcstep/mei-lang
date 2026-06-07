use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use super::app::{app_page, index, AppQuery};
use super::assets::{app_bundle, workspace_app_asset};
use super::static_serve::content_type_for_path;
use crate::{agent_runtime, auth::AuthEnforcement, mei_agent, AppState};
use axum::{
    body::to_bytes,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

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
async fn app_bundle_returns_merged_javascript() {
    let source_root = Arc::new(std::env::temp_dir());
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

    let response = app_bundle(State(state), AxumPath("manage.js".to_string()))
        .await
        .expect("build manage bundle");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read bundle body");
    let script = String::from_utf8(body.to_vec()).expect("bundle body utf8");
    assert!(script.contains("meiLangBoot"));
    assert!(
        script.contains("manageDiagnosticsMounted"),
        "bundle should include manage-diagnostics.js"
    );
    assert!(
        script.contains("spaNavigationMounted") || script.contains("spa-navigation"),
        "bundle should include spa navigation code"
    );
}

#[tokio::test]
async fn app_bundle_supports_shoelace_mode() {
    let source_root = Arc::new(std::env::temp_dir());
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

    let response = app_bundle(State(state), AxumPath("shoelace.js".to_string()))
        .await
        .expect("build shoelace bundle");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read shoelace body");
    assert!(body.len() > 256, "shoelace bundle should not be empty");
}

#[tokio::test]
async fn app_bundle_supports_styles_mode() {
    let source_root = Arc::new(std::env::temp_dir());
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
    let response = app_bundle(State(state), AxumPath("styles.css".to_string()))
        .await
        .expect("build styles bundle");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read styles body");
    let css = String::from_utf8(body.to_vec()).expect("styles body utf8");
    assert!(
        css.contains(".sl-theme-dark") || css.contains("Generated by scripts/build-assets.mjs"),
        "styles bundle should contain merged stylesheet content"
    );
}

#[tokio::test]
async fn app_page_returns_html_error_page_when_compile_fails() {
    let root = unique_test_root("bad-app");
    let app_root = root.join("bad-app");
    fs::create_dir_all(&app_root).expect("create app root");
    fs::write(
            app_root.join("main.mei"),
            "app(\n    id = \"bad-app\",\n    title = \"Broken\",\n    default_scene = \"home\",\n)\n\nscene(\n    id = \"home\",\n    summary = \"unterminated,\n)\n",
        )
        .expect("write invalid mei file");

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
        AxumPath(("build".to_string(), "bad-app".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: None,
            diag_filter: None,
            chrome: None,
        }),
    )
    .await
    .expect("render app page response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(html.contains("编译失败，预览已降级"));
    assert!(html.contains("bad-app"));
    assert!(html.contains("compile_failed"));
    assert!(html.contains("Parse error"));
    assert!(html.contains("错误诊断"));

    let _ = fs::remove_dir_all(&root);
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
            chrome: None,
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
    fs::write(app_root.join("main.mei"), VALID_APP_SOURCE.replace("good-app", "html-app"))
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
            chrome: None,
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

#[tokio::test]
async fn manage_html_preview_uses_document_iframe() {
    let root = unique_test_root("static-html-manage");
    let app_root = root.join("html-app");
    fs::create_dir_all(app_root.join("demo")).expect("create demo dir");
    fs::write(app_root.join("main.mei"), VALID_APP_SOURCE.replace("good-app", "html-app"))
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
            chrome: None,
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
            chrome: None,
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
async fn access_scene_not_found_returns_404() {
    let root = unique_test_root("access-scene-not-found");
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
        AxumPath(("app".to_string(), "multi-scene/scene/not-found".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            chrome: None,
        }),
    )
    .await
    .expect("render access page");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(html.contains("场景不存在"));

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_only_surface_redirects_build_route_to_access_scene() {
    let _surface = ScopedAccessOnlySurface::new();
    let root = unique_test_root("access-only-build-redirect");
    let app_root = root.join("good-app");
    fs::create_dir_all(&app_root).expect("create app root");
    fs::write(app_root.join("main.mei"), VALID_APP_SOURCE).expect("write main.mei");

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
        AxumPath(("build".to_string(), "good-app".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            chrome: None,
        }),
    )
    .await
    .expect("render app page");

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/apps/app/good-app/scene/home?tab=preview&chrome=none")
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_only_surface_hides_topbar_tabs_on_app_route() {
    let _surface = ScopedAccessOnlySurface::new();
    let root = unique_test_root("access-only-hide-tabs");
    let app_root = root.join("good-app");
    fs::create_dir_all(&app_root).expect("create app root");
    fs::write(app_root.join("main.mei"), VALID_APP_SOURCE).expect("write main.mei");

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
        AxumPath(("app".to_string(), "good-app/scene/home".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            chrome: None,
        }),
    )
    .await
    .expect("render app page");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read html body");
    let html = String::from_utf8(body.to_vec()).expect("response body utf8");
    assert!(
        !html.contains("mode-tab-group"),
        "access-only surface should hide build/config/upload/app topbar tabs"
    );
    assert!(
        html.contains("access-chat-floating-root"),
        "access floating panel should still exist"
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn access_only_mode_slug_routes_to_app_surface() {
    let root = unique_test_root("access-only-mode-slug");
    let app_root = root.join("good-app");
    fs::create_dir_all(&app_root).expect("create app root");
    fs::write(app_root.join("main.mei"), VALID_APP_SOURCE).expect("write main.mei");

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
        AxumPath(("access-only".to_string(), "good-app/scene/home".to_string())),
        Query(AppQuery {
            file: None,
            scene: None,
            tab: Some("preview".to_string()),
            diag_filter: None,
            chrome: None,
        }),
    )
    .await
    .expect("render app page");

    assert_eq!(response.status(), StatusCode::OK);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn index_redirects_to_first_healthy_app_when_first_app_is_broken() {
    let root = unique_test_root("index-redirect");
    let broken_root = root.join("011-bad");
    let good_root = root.join("020-good");
    fs::create_dir_all(&broken_root).expect("create broken app root");
    fs::create_dir_all(&good_root).expect("create good app root");
    fs::write(
            broken_root.join("main.mei"),
            "app(\n    id = \"011-bad\",\n    title = \"Broken\",\n    default_scene = \"home\",\n)\n\nscene(\n    id = \"home\",\n    summary = \"unterminated,\n)\n",
        )
        .expect("write invalid mei file");
    fs::write(good_root.join("main.mei"), VALID_APP_SOURCE).expect("write valid mei file");

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

    let response = index(State(state))
        .await
        .expect("render index redirect")
        .into_response();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/apps/build/020-good")
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn index_redirects_to_access_only_entry_when_surface_enabled() {
    let _surface = ScopedAccessOnlySurface::new();
    let root = unique_test_root("index-access-only");
    let good_root = root.join("010-good");
    fs::create_dir_all(&good_root).expect("create good app root");
    fs::write(good_root.join("main.mei"), VALID_APP_SOURCE).expect("write valid mei file");

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

    let response = index(State(state))
        .await
        .expect("render index redirect")
        .into_response();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/apps/access-only/010-good")
    );

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn dataset_query_api_echoes_scene_id() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let state = crate::test_support::test_app_state().expect("app state");
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
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["scene_id"], "home");
    assert_eq!(v["dataset_id"], "sales_data");
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

#[tokio::test(flavor = "current_thread")]
async fn spbjw_home_indicator_metrics_use_inspection_check_date_xlsx() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let state = crate::test_support::test_app_state().expect("app state");
    let app = crate::http::router().with_state(state);
    let payload = serde_json::json!({
        "scene_id": "home",
        "target": "scenes/home.mei",
        "dataset_id": "__world_metrics__::scenes/03-指标体系.mei::metrics",
        "metric_ids": [
            "scenes/03-指标体系.mei::inspection_frequency_reduction_rate",
            "scenes/03-指标体系.mei::penalty_revenue_growth_rate"
        ]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/datasets/metrics/spbjw")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let metrics = v["metrics"].as_array().expect("metrics array");
    assert!(
        metrics.len() >= 2,
        "expected indicator metrics in response: {metrics:?}"
    );
    for metric in metrics {
        let id = metric["id"].as_str().unwrap_or_default();
        let value = metric["value"]["value"]
            .as_f64()
            .or_else(|| metric["value"].as_f64())
            .unwrap_or(0.0);
        assert!(
            value.is_finite() && value.abs() > f64::EPSILON,
            "metric `{id}` should be non-zero when backed by upload/5.行政检查结果清单.xlsx 检查日期, got {value}"
        );
    }
}

#[tokio::test]
async fn http_dataset_query_aligns_with_toolchain_access_query() {
    use axum::{
        body::Body,
        body::to_bytes,
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
