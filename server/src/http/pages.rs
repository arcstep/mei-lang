use std::{fs, path::Path};

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Local};
use mei_lang_app::{render_page, SourcePanelMeta, TopbarMenuConfig, TopbarMenuContext, UiRouteMode};
use mei_lang_kernel::{
    compile_app_with_options, discover_apps, read_source_file, source_tree, CompileOptions,
    CompiledApp, Diagnostic, Severity, WorkspaceAppMeta,
};
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct AppQuery {
    target: Option<String>,
    entry: Option<String>,
    preview_target: Option<String>,
    tab: Option<String>,
    chrome: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let first = choose_default_app(&state.source_root, &apps).or_else(|| apps.first());
    let first = first.ok_or_else(|| {
        AppError::msg(format!(
            "source root has no discoverable apps (need at least one first-level subdirectory under `{}` containing `main.mei`; root-level `main.mei` is ignored)",
            state.source_root.display()
        ))
    })?;
    Ok(Redirect::to(&format!("/apps/manage/{}", first.id)))
}

pub async fn app_page(
    State(state): State<AppState>,
    AxumPath((mode, app_id_raw)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Response, AppError> {
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let route_mode = UiRouteMode::from_slug(&mode);
    let chrome_hidden = query
        .chrome
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("none"))
        .unwrap_or(false);
    let compile_options = CompileOptions {
        entry: query.entry.clone(),
        preview_target: query.preview_target.clone(),
    };
    let compiled = match compile_app_with_options(&state.source_root, &app_id, compile_options) {
        Ok(compiled) => compiled,
        Err(error) => {
            tracing::warn!(app_id = %app_id, %error, "failed to compile app page");
            let target = query
                .target
                .clone()
                .or_else(|| query.preview_target.clone())
                .unwrap_or_else(|| "main.mei".to_string());
            let source_path = state.source_root.join(&app_id).join(&target);
            let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
            let source_meta = source_panel_meta(&source_path, &source);
            let topbar_menus = load_segment_topbar_menus(&state.source_root);
            let compiled = compile_error_fallback_app(
                &state.source_root,
                &app_id,
                target.as_str(),
                error.to_string().as_str(),
            );
            let html = render_page(
                &apps,
                &compiled,
                &app_id,
                Some(&topbar_menus),
                route_mode,
                Some(target.as_str()),
                Some(source.as_str()),
                Some(&source_meta),
                query.entry.as_deref(),
                query.preview_target.as_deref(),
                query.tab.as_deref(),
                chrome_hidden,
            );
            return Ok(Html(html).into_response());
        }
    };
    let target = query
        .target
        .or_else(|| query.preview_target.clone())
        .unwrap_or_else(|| compiled.entry_target.clone());
    let source_path = state.source_root.join(&app_id).join(&target);
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let source_meta = source_panel_meta(&source_path, &source);
    let topbar_menus = load_segment_topbar_menus(&state.source_root);
    let html = render_page(
        &apps,
        &compiled,
        &app_id,
        Some(&topbar_menus),
        route_mode,
        Some(target.as_str()),
        Some(source.as_str()),
        Some(&source_meta),
        query.entry.as_deref(),
        query.preview_target.as_deref(),
        query.tab.as_deref(),
        chrome_hidden,
    );
    Ok(Html(html).into_response())
}

pub async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    let components_root = resolve_components_root(&state.source_root);
    serve_static_asset(components_root.join(&path), "component asset")
}

pub async fn app_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.package_root.join("app").join("assets").join(&path),
        "app asset",
    )
}

pub async fn workspace_app_asset(
    State(state): State<AppState>,
    AxumPath((app_id, path)): AxumPath<(String, String)>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.source_root.join(&app_id).join(&path),
        "workspace app asset",
    )
}

fn serve_static_asset(asset_path: std::path::PathBuf, label: &str) -> Result<Response, AppError> {
    if !asset_path.exists() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            format!("{label} not found: {}", asset_path.display()),
        ));
    }
    let bytes = fs::read(&asset_path)
        .with_context(|| format!("failed to read {}", asset_path.display()))
        .map_err(AppError::from)?;
    let mut response = Response::new(bytes.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(&asset_path)),
    );
    Ok(response)
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("md") | Some("markdown") => "text/markdown; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("tsv") => "text/tab-separated-values; charset=utf-8",
        Some("pdf") => "application/pdf",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        _ => "text/plain; charset=utf-8",
    }
}

fn source_panel_meta(source_path: &Path, source: &str) -> SourcePanelMeta {
    let line_count = if source.is_empty() {
        0
    } else {
        source.split('\n').count()
    };
    let char_count = source.chars().count();
    let last_modified_label = fs::metadata(source_path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|modified| {
            let modified: DateTime<Local> = modified.into();
            modified.format("%Y-%m-%d %H:%M:%S").to_string()
        });
    SourcePanelMeta {
        line_count,
        char_count,
        last_modified_label,
    }
}

fn choose_default_app<'a>(
    source_root: &Path,
    apps: &'a [WorkspaceAppMeta],
) -> Option<&'a WorkspaceAppMeta> {
    for app in apps {
        if compile_app_with_options(source_root, &app.id, CompileOptions::default()).is_ok() {
            return Some(app);
        }
        tracing::warn!(app_id = %app.id, "skip broken app as default landing target");
    }
    None
}

fn resolve_components_root(source_root: &Path) -> std::path::PathBuf {
    let local = source_root.join("_components");
    if local.exists() {
        return local;
    }
    if let Some(parent) = source_root.parent() {
        let shared = parent.join("_components");
        if shared.exists() {
            return shared;
        }
    }
    local
}

#[derive(Debug, Deserialize)]
struct MeiConfigMenuEnvelope {
    #[serde(default)]
    menu: Option<TopbarMenuConfig>,
}

fn read_topbar_menu_json(path: &Path) -> Option<TopbarMenuConfig> {
    if !path.is_file() {
        return None;
    }
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read topbar menu file");
            return None;
        }
    };
    match serde_json::from_str::<TopbarMenuConfig>(&raw) {
        Ok(config) => Some(config),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to parse topbar menu json");
            None
        }
    }
}

fn read_menu_from_mei_config(path: &Path) -> Option<TopbarMenuConfig> {
    if !path.is_file() {
        return None;
    }
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read .mei-config.json");
            return None;
        }
    };
    match serde_json::from_str::<MeiConfigMenuEnvelope>(&raw) {
        Ok(envelope) => {
            if let Some(menu) = envelope.menu {
                return Some(menu);
            }
            None
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to parse .mei-config.json menu envelope");
            None
        }
    }
}

fn load_topbar_menu_from_dir(dir: &Path) -> Option<TopbarMenuConfig> {
    let modern = dir.join(".mei-config.json");
    if let Some(menu) = read_menu_from_mei_config(&modern) {
        return Some(menu);
    }
    read_topbar_menu_json(&dir.join("_menu.json"))
}

fn load_segment_topbar_menus(source_root: &Path) -> TopbarMenuContext {
    let mut by_segment = BTreeMap::new();
    let root = load_topbar_menu_from_dir(source_root);
    let Ok(entries) = fs::read_dir(source_root) else {
        return TopbarMenuContext { root, by_segment };
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        if let Some(config) = load_topbar_menu_from_dir(&entry.path()) {
            by_segment.insert(name, config);
        }
    }
    TopbarMenuContext { root, by_segment }
}

fn compile_error_fallback_app(
    source_root: &Path,
    app_id: &str,
    target: &str,
    error: &str,
) -> CompiledApp {
    let app_root = source_root.join(app_id);
    let source_path = app_root.join(target);
    CompiledApp {
        app_id: app_id.to_string(),
        title: app_id.to_string(),
        app_root: app_root.to_string_lossy().to_string(),
        entries: Vec::new(),
        active_entry: None,
        entry_target: target.to_string(),
        file_tree: source_tree(&app_root).unwrap_or_default(),
        scene_contract: None,
        resources: Vec::new(),
        component_assets: Vec::new(),
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            code: "compile_failed".to_string(),
            message: error.to_string(),
            source_path: Some(source_path.to_string_lossy().to_string()),
        }],
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use axum::{
        body::to_bytes,
        extract::{Path as AxumPath, Query, State},
        http::StatusCode,
        response::IntoResponse,
    };
    use reqwest::Client as HttpClient;

    use super::{app_page, index, AppQuery};
    use crate::{opencode, AppState};

    const VALID_APP_SOURCE: &str = r#"
app(
    id = "good-app",
    default_scene = "home",
    entries = [
        entry(id = "home", scene = "home", frame = "home_frame"),
    ],
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

        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root: Arc::new(root.clone()),
            opencode_preferred_mode: Arc::new("external".to_string()),
            opencode_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            opencode_auto_start: false,
            opencode_runtime: Arc::new(Mutex::new(opencode::ManagedOpencodeRuntime::default())),
            opencode_session_context: Arc::new(Mutex::new(HashMap::new())),
            opencode_http: Arc::new(HttpClient::new()),
        };

        let response = app_page(
            State(state),
            AxumPath(("manage".to_string(), "bad-app".to_string())),
            Query(AppQuery {
                target: None,
                entry: None,
                preview_target: None,
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

        let state = AppState {
            package_root: Arc::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")),
            source_root: Arc::new(root.clone()),
            opencode_preferred_mode: Arc::new("external".to_string()),
            opencode_preferred_server_url: Arc::new("http://127.0.0.1:4099".to_string()),
            opencode_auto_start: false,
            opencode_runtime: Arc::new(Mutex::new(opencode::ManagedOpencodeRuntime::default())),
            opencode_session_context: Arc::new(Mutex::new(HashMap::new())),
            opencode_http: Arc::new(HttpClient::new()),
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
