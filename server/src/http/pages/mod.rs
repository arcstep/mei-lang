//! 管理端 / 访问端 HTML 页面、数据集查询 API 与静态资源合并下发。

mod app;
mod app_render;
mod assets;
mod components;
pub mod dataset_api;
mod menus;
pub mod metric_api;
mod static_serve;
mod util;

pub use app::{app_page, index};
pub use assets::{app_asset, app_bundle, workspace_app_asset};
pub use components::component_asset;
pub(crate) use components::resolve_components_root;
pub use dataset_api::dataset_query_api;
pub use metric_api::dataset_metric_api;

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::app::{app_page, index, AppQuery};
    use super::assets::app_bundle;
    use crate::{agent_runtime, mei_agent, AppState};
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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
    ],
)
"#;

    const MULTI_SCENE_APP_SOURCE: &str = r##"
app(
    id = "multi-scene",
    default_scene = "home",
)

app_add_scene(scene_file_ref("details.mei", id = "details"))

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
        doc.markdown(area = "auto", resource = world_ref("home_doc")),
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
        doc.markdown(area = "auto", resource = world_ref("details_doc")),
    ],
)
"##;

    #[tokio::test]
    async fn app_bundle_returns_merged_javascript() {
        let source_root = Arc::new(std::env::temp_dir());
        let native_agent = Arc::new(
            mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"),
        );
        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
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
            script.contains("spaNavigationMounted") || script.contains("spa-navigation"),
            "bundle should include spa navigation code"
        );
    }

    #[tokio::test]
    async fn app_bundle_supports_shoelace_mode() {
        let source_root = Arc::new(std::env::temp_dir());
        let native_agent = Arc::new(
            mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"),
        );
        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
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
        let native_agent = Arc::new(
            mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"),
        );
        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
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
        let native_agent = Arc::new(
            mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"),
        );
        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        };

        let response = app_page(
            State(state),
            AxumPath(("manage".to_string(), "bad-app".to_string())),
            Query(AppQuery {
                file: None,
                scene: None,
                tab: None,
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
        let native_agent = Arc::new(
            mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"),
        );
        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
            native_agent,
        };

        let response = app_page(
            State(state),
            AxumPath(("manage".to_string(), "multi-scene".to_string())),
            Query(AppQuery {
                file: Some("details.mei".to_string()),
                scene: Some("home".to_string()),
                tab: Some("preview".to_string()),
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
        assert!(html.contains("/apps/manage/multi-scene?scene=details"));

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
        let native_agent = Arc::new(
            mei_agent::NativeAgent::open(source_root.as_ref().clone()).expect("native agent"),
        );
        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root,
            agent_preferred_mode: Arc::new("external".to_string()),
            agent_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            agent_auto_start: false,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::new())),
            compile_cache: Arc::new(Mutex::new(HashMap::new())),
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
            Some("/apps/manage/020-good")
        );

        let _ = fs::remove_dir_all(&root);
    }

    fn unique_test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("mei-lang-server-{label}-{nonce}"))
    }
}
