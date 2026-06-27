mod static_serve;

use std::fs;
use std::path::{Path, PathBuf};

use axum::{
    extract::{Path as AxumPath, State},
    http::{header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use mei_lang_kernel::{resolve_app_root, resolve_components_root, resolve_templates_root};

use crate::state::SharedState;

use static_serve::{asset_not_found, serve_static_asset_with_cache};

const PUBLIC_REVALIDATE_CACHE_CONTROL: &str = "public, no-cache";
const PRIVATE_REVALIDATE_CACHE_CONTROL: &str = "private, no-cache";
const COMPONENT_REVALIDATE_CACHE_CONTROL: &str = "private, no-cache";
const VENDOR_REVALIDATE_CACHE_CONTROL: &str = "public, no-cache";

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../server/src/http/pages/bundle_order_generated.rs"
));

pub async fn app_asset(
    State(state): State<SharedState>,
    headers: HeaderMap,
    AxumPath(path): AxumPath<String>,
) -> Response {
    let package_root = state.read().expect("state lock").package_root.clone();
    if path == "page-load-progress-shell.js" {
        if let Some(content) = merged_page_load_progress_shell(&package_root) {
            return (
                StatusCode::OK,
                [(
                    HeaderName::from_static("content-type"),
                    HeaderValue::from_static("application/javascript; charset=utf-8"),
                )],
                content,
            )
                .into_response();
        }
    }
    let asset_path = package_root.join("app/assets").join(&path);
    serve_static_asset_with_cache(
        asset_path,
        "app asset",
        &headers,
        PUBLIC_REVALIDATE_CACHE_CONTROL,
    )
    .unwrap_or_else(|error| (StatusCode::NOT_FOUND, error.to_string()).into_response())
}

pub async fn app_bundle(
    State(state): State<SharedState>,
    headers: HeaderMap,
    AxumPath(mode): AxumPath<String>,
) -> Response {
    let package_root = state.read().expect("state lock").package_root.clone();
    let assets_root = package_root.join("app/assets");
    if let Some(dist_rel_path) = app_bundle_dist_path(&mode) {
        let dist_path = assets_root.join(dist_rel_path);
        if dist_path.exists() {
            return serve_static_asset_with_cache(
                dist_path,
                "app dist bundle",
                &headers,
                PUBLIC_REVALIDATE_CACHE_CONTROL,
            )
            .unwrap_or_else(|error| (StatusCode::NOT_FOUND, error.to_string()).into_response());
        }
    }
    if matches!(mode.as_str(), "shoelace.js" | "shoelace") {
        return serve_static_asset_with_cache(
            assets_root.join("shoelace-local.js"),
            "shoelace fallback bundle",
            &headers,
            PUBLIC_REVALIDATE_CACHE_CONTROL,
        )
        .unwrap_or_else(|error| (StatusCode::NOT_FOUND, error.to_string()).into_response());
    }
    if matches!(mode.as_str(), "styles.css" | "styles") {
        return merged_styles_response(&assets_root, &headers);
    }
    let Some(scripts) = app_bundle_scripts(&mode) else {
        return (StatusCode::NOT_FOUND, format!("unsupported app bundle mode: {mode}")).into_response();
    };
    merged_scripts_response(&assets_root, scripts, &headers)
}

pub async fn workspace_app_asset(
    State(state): State<SharedState>,
    headers: HeaderMap,
    AxumPath((app_id, path)): AxumPath<(String, String)>,
) -> Response {
    let workspace_root = state.read().expect("state lock").ctx.workspace_root.clone();
    let asset_root = if app_id == "templates" {
        resolve_templates_root(workspace_root.as_path()).join(&path)
    } else {
        resolve_app_root(workspace_root.as_path(), &app_id).join(&path)
    };
    serve_static_asset_with_cache(
        asset_root,
        "workspace app asset",
        &headers,
        PRIVATE_REVALIDATE_CACHE_CONTROL,
    )
    .unwrap_or_else(|error| (StatusCode::NOT_FOUND, error.to_string()).into_response())
}

pub async fn component_asset(
    State(state): State<SharedState>,
    headers: HeaderMap,
    AxumPath(path): AxumPath<String>,
) -> Response {
    let workspace_root = state.read().expect("state lock").ctx.workspace_root.clone();
    let components_root = resolve_components_root(workspace_root.as_path());
    let asset_path = resolve_component_asset_path(&components_root, &path);
    if path.ends_with(".js.map") && !asset_path.exists() {
        return StatusCode::NO_CONTENT.into_response();
    }
    let cache_control = if path.starts_with("vendor/") {
        VENDOR_REVALIDATE_CACHE_CONTROL
    } else {
        COMPONENT_REVALIDATE_CACHE_CONTROL
    };
    serve_static_asset_with_cache(asset_path, "component asset", &headers, cache_control)
        .unwrap_or_else(|error| (StatusCode::NOT_FOUND, error.to_string()).into_response())
}

fn merged_page_load_progress_shell(package_root: &Path) -> Option<String> {
    let base = package_root.join("app/assets/page-load-progress-shell");
    let p1 = fs::read_to_string(base.join("p1.js")).ok()?;
    let p2 = fs::read_to_string(base.join("p2.js")).ok()?;
    Some(format!("{p1}\n{p2}"))
}

fn merged_styles_response(assets_root: &Path, headers: &HeaderMap) -> Response {
    let styles = app_bundle_styles();
    let mut merged = String::new();
    merged.push_str("/* Runtime merged stylesheet served by mei-host-shell. */\n");
    for style in styles {
        let style_path = assets_root.join(style);
        let Ok(content) = fs::read_to_string(&style_path) else {
            return asset_not_found("style bundle file", &style_path);
        };
        merged.push_str("\n/* ===== ");
        merged.push_str(style);
        merged.push_str(" ===== */\n");
        merged.push_str(&content);
        merged.push('\n');
    }
    let mut response = Response::new(merged.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/css; charset=utf-8"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static(PRIVATE_REVALIDATE_CACHE_CONTROL),
    );
    let _ = headers;
    response
}

fn merged_scripts_response(
    assets_root: &Path,
    scripts: &[&str],
    headers: &HeaderMap,
) -> Response {
    let mut merged = String::new();
    merged.push_str("// Runtime merged bundle served by mei-host-shell.\n");
    for script in scripts {
        let script_path = assets_root.join(script);
        let Ok(content) = fs::read_to_string(&script_path) else {
            return asset_not_found("app bundle script", &script_path);
        };
        merged.push_str("\n/* ===== ");
        merged.push_str(script);
        merged.push_str(" ===== */\n");
        merged.push_str(&content);
        merged.push_str("\n;\n");
    }
    let mut response = Response::new(merged.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static(PRIVATE_REVALIDATE_CACHE_CONTROL),
    );
    let _ = headers;
    response
}

fn app_bundle_scripts(mode: &str) -> Option<&'static [&'static str]> {
    match mode {
        "manage.js" | "manage" | "build.js" | "build" => Some(BUNDLE_MANAGE_SCRIPTS),
        "manage-source.js" | "manage-source" => Some(BUNDLE_MANAGE_SOURCE_SCRIPTS),
        "access.js" | "access" | "app.js" | "app" => Some(BUNDLE_ACCESS_SCRIPTS),
        "config.js" | "config" => Some(BUNDLE_CONFIG_SCRIPTS),
        "upload.js" | "upload" => Some(BUNDLE_UPLOAD_SCRIPTS),
        _ => None,
    }
}

fn app_bundle_dist_path(mode: &str) -> Option<&'static str> {
    match mode {
        "manage.js" | "manage" | "build.js" | "build" => Some("dist/manage.bundle.js"),
        "manage-source.js" | "manage-source" => Some("dist/manage-source.bundle.js"),
        "access.js" | "access" | "app.js" | "app" => Some("dist/access.bundle.js"),
        "config.js" | "config" => Some("dist/config.bundle.js"),
        "upload.js" | "upload" => Some("dist/upload.bundle.js"),
        "shoelace.js" | "shoelace" => Some("dist/shoelace.bundle.js"),
        "auth-rsa.js" | "auth-rsa" => Some("dist/auth-rsa.bundle.js"),
        "styles.css" | "styles" => Some("dist/styles.bundle.css"),
        _ => None,
    }
}

fn app_bundle_styles() -> &'static [&'static str] {
    BUNDLE_STYLES_ORDER
}

fn resolve_component_asset_path(components_root: &Path, request_path: &str) -> PathBuf {
    let requested = components_root.join(request_path);
    if requested.exists() {
        return requested;
    }
    requested
}
