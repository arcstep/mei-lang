use std::{fs, path::Path};

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Local};
use mei_lang_app::{render_page, SourcePanelMeta, TopbarMenuConfig, UiRouteMode};
use mei_lang_kernel::{
    compile_app_with_options, discover_apps, read_source_file, CompileOptions, WorkspaceAppMeta,
};
use serde::Deserialize;

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct AppQuery {
    target: Option<String>,
    entry: Option<String>,
    preview_target: Option<String>,
    chrome: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let first = choose_default_app(&state.source_root, &apps).or_else(|| apps.first());
    let first = first.ok_or_else(|| {
        AppError::msg(format!(
            "source root does not contain any apps: {}",
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
            let html = render_compile_error_page(
                &apps,
                &app_id,
                route_mode.slug(),
                target.as_str(),
                &error.to_string(),
                source.as_str(),
                &source_meta,
                chrome_hidden,
            );
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, Html(html)).into_response());
        }
    };
    let target = query
        .target
        .or_else(|| query.preview_target.clone())
        .unwrap_or_else(|| compiled.entry_target.clone());
    let source_path = state.source_root.join(&app_id).join(&target);
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let source_meta = source_panel_meta(&source_path, &source);
    let topbar_menu_config = load_topbar_menu_config(&state.source_root);
    let html = render_page(
        &apps,
        &compiled,
        &app_id,
        topbar_menu_config.as_ref(),
        route_mode,
        Some(target.as_str()),
        Some(source.as_str()),
        Some(&source_meta),
        query.entry.as_deref(),
        query.preview_target.as_deref(),
        chrome_hidden,
    );
    Ok(Html(html).into_response())
}

pub async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.source_root.join("_components").join(&path),
        "component asset",
    )
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

fn load_topbar_menu_config(source_root: &Path) -> Option<TopbarMenuConfig> {
    let candidates = [
        source_root.join("_menu.json"),
        source_root.join("menu.json"),
    ];
    for path in candidates {
        if !path.exists() {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "failed to read topbar menu config");
                continue;
            }
        };
        match serde_json::from_str::<TopbarMenuConfig>(&raw) {
            Ok(config) => return Some(config),
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "failed to parse topbar menu config");
            }
        }
    }
    None
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_compile_error_page(
    apps: &[WorkspaceAppMeta],
    app_id: &str,
    route_mode: &str,
    target: &str,
    error: &str,
    source: &str,
    source_meta: &SourcePanelMeta,
    chrome_hidden: bool,
) -> String {
    let title = format!("MeiLang 编译失败 · {app_id}");
    let app_tabs = apps
        .iter()
        .map(|app| {
            let href = format!("/apps/{}/{}", route_mode, app.id);
            let active_style = if app.id == app_id {
                "background:#2563eb;color:#eff6ff;border-color:#3b82f6;"
            } else {
                "background:#111827;color:#cbd5e1;border-color:rgba(148,163,184,0.25);"
            };
            format!(
                "<a href=\"{}\" style=\"display:inline-flex;align-items:center;padding:8px 12px;border-radius:999px;border:1px solid {};text-decoration:none;{}\">{}</a>",
                escape_html(&href),
                if app.id == app_id { "#3b82f6" } else { "rgba(148,163,184,0.25)" },
                active_style,
                escape_html(&app.id),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let source_block = if source.trim().is_empty() {
        "<p style=\"color:#94a3b8\">当前目标文件内容不可读取。</p>".to_string()
    } else {
        format!(
            "<pre style=\"margin:0;white-space:pre-wrap;word-break:break-word;background:#0f172a;color:#e2e8f0;padding:16px;border-radius:12px;overflow:auto;\">{}</pre>",
            escape_html(source)
        )
    };
    let chrome_style = if chrome_hidden { "display:none;" } else { "" };
    let last_modified = source_meta
        .last_modified_label
        .as_deref()
        .unwrap_or("unknown");
    format!(
        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title></head><body style=\"margin:0;font-family:ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#020617;color:#e2e8f0;\"><div style=\"{}padding:20px 24px;border-bottom:1px solid rgba(148,163,184,0.2);background:#0f172a;\"><div style=\"display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap;\"><div><strong>MeiLang</strong><span style=\"margin-left:12px;color:#94a3b8;\">/{}/{}</span></div><a href=\"/\" style=\"color:#93c5fd;text-decoration:none;\">返回默认应用</a></div><nav style=\"display:flex;gap:8px;flex-wrap:wrap;margin-top:14px;\">{}</nav></div><main style=\"max-width:1120px;margin:0 auto;padding:24px;\"><section style=\"background:#111827;border:1px solid rgba(248,113,113,0.35);border-radius:16px;padding:20px 22px;margin-bottom:20px;\"><h1 style=\"margin:0 0 12px;font-size:22px;\">应用页面编译失败</h1><p style=\"margin:0 0 12px;color:#cbd5e1;\">当前 `.mei` 文件包含编译错误；服务器进程仍在运行，但该应用页面无法成功渲染。你仍可切换到其他应用继续工作。</p><p style=\"margin:0 0 8px;\"><strong>app:</strong> {}</p><p style=\"margin:0 0 8px;\"><strong>target:</strong> {}</p><p style=\"margin:0 0 8px;\"><strong>last modified:</strong> {}</p><pre style=\"margin:12px 0 0;white-space:pre-wrap;word-break:break-word;background:#1e293b;color:#fecaca;padding:16px;border-radius:12px;overflow:auto;\">{}</pre></section><section style=\"background:#111827;border:1px solid rgba(148,163,184,0.2);border-radius:16px;padding:20px 22px;\"><h2 style=\"margin:0 0 12px;font-size:18px;\">当前文件内容</h2><p style=\"margin:0 0 12px;color:#94a3b8;\">lines: {} · chars: {}</p>{}</section></main></body></html>",
        escape_html(&title),
        chrome_style,
        escape_html(route_mode),
        escape_html(app_id),
        app_tabs,
        escape_html(app_id),
        escape_html(target),
        escape_html(last_modified),
        escape_html(error),
        source_meta.line_count,
        source_meta.char_count,
        source_block,
    )
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
                chrome: None,
            }),
        )
        .await
        .expect("render app page response");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read html body");
        let html = String::from_utf8(body.to_vec()).expect("response body utf8");
        assert!(html.contains("应用页面编译失败"));
        assert!(html.contains("bad-app"));
        assert!(html.contains("Parse error"));
        assert!(html.contains("返回默认应用"));

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
