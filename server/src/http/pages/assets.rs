use std::fs;

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, State},
    http::{header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
};

use mei_lang_kernel::{resolve_app_root, resolve_templates_root};

use crate::{AppError, AppState};

use super::static_serve::serve_static_asset_with_cache;
use mei_host_core::resolve_app_assets_dir;

const PUBLIC_REVALIDATE_CACHE_CONTROL: &str = "public, no-cache";
const PRIVATE_REVALIDATE_CACHE_CONTROL: &str = "private, no-cache";

// 脚本顺序由 `scripts/build/bundle-manifest.json` 定义；`npm run assets:build` 生成下方 include 文件。
include!("bundle_order_generated.rs");

pub async fn app_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    serve_static_asset_with_cache(
        resolve_app_assets_dir(&state.package_root).join(&path),
        "app asset",
        &headers,
        PUBLIC_REVALIDATE_CACHE_CONTROL,
    )
}

pub async fn app_bundle(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(mode): AxumPath<String>,
) -> Result<Response, AppError> {
    let assets_root = resolve_app_assets_dir(&state.package_root);
    if let Some(dist_rel_path) = app_bundle_dist_path(&mode) {
        let dist_path = assets_root.join(dist_rel_path);
        if dist_path.exists() {
            return serve_static_asset_with_cache(
                dist_path,
                "app dist bundle",
                &headers,
                PUBLIC_REVALIDATE_CACHE_CONTROL,
            );
        }
    }
    if matches!(mode.as_str(), "shoelace.js" | "shoelace") {
        return serve_static_asset_with_cache(
            assets_root.join("shoelace-local.js"),
            "shoelace fallback bundle",
            &headers,
            PUBLIC_REVALIDATE_CACHE_CONTROL,
        );
    }
    if matches!(mode.as_str(), "styles.css" | "styles") {
        let styles = app_bundle_styles();
        let mut merged = String::new();
        merged.push_str("/* Runtime merged stylesheet served by mei-lang-server. */\n");
        for style in styles {
            let style_path = assets_root.join(style);
            let content = fs::read_to_string(&style_path)
                .with_context(|| {
                    format!("failed to read style bundle file {}", style_path.display())
                })
                .map_err(AppError::from)?;
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
        return Ok(response);
    }
    let scripts = app_bundle_scripts(&mode).ok_or_else(|| {
        AppError::status(
            StatusCode::NOT_FOUND,
            format!("unsupported app bundle mode: {mode}"),
        )
    })?;
    let mut merged = String::new();
    merged.push_str("// Runtime merged bundle served by mei-lang-server.\n");
    for script in scripts {
        let script_path = assets_root.join(script);
        let content = fs::read_to_string(&script_path)
            .with_context(|| format!("failed to read app bundle script {}", script_path.display()))
            .map_err(AppError::from)?;
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
    Ok(response)
}

pub async fn workspace_app_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((app_id, path)): AxumPath<(String, String)>,
) -> Result<Response, AppError> {
    let asset_root = if app_id == "templates" {
        resolve_templates_root(state.source_root.as_path()).join(&path)
    } else {
        resolve_app_root(state.source_root.as_path(), &app_id).join(&path)
    };
    serve_static_asset_with_cache(
        asset_root,
        "workspace app asset",
        &headers,
        PRIVATE_REVALIDATE_CACHE_CONTROL,
    )
}

fn app_bundle_scripts(mode: &str) -> Option<&'static [&'static str]> {
    match mode {
        "manage.js" | "manage" | "build.js" | "build" => Some(BUNDLE_MANAGE_SCRIPTS),
        "manage-source.js" | "manage-source" => Some(BUNDLE_MANAGE_SOURCE_SCRIPTS),
        "access.js" | "access" | "app.js" | "app" => Some(BUNDLE_ACCESS_SCRIPTS),
        "config.js" | "config" => Some(BUNDLE_CONFIG_SCRIPTS),
        "upload.js" | "upload" => Some(BUNDLE_UPLOAD_SCRIPTS),
        "admin.js" | "admin" => Some(BUNDLE_ADMIN_SCRIPTS),
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
        "admin.js" | "admin" => Some("dist/admin.bundle.js"),
        "shoelace.js" | "shoelace" => Some("dist/shoelace.bundle.js"),
        "auth-rsa.js" | "auth-rsa" => Some("dist/auth-rsa.bundle.js"),
        "styles.css" | "styles" => Some("dist/styles.bundle.css"),
        _ => None,
    }
}

fn app_bundle_styles() -> &'static [&'static str] {
    BUNDLE_STYLES_ORDER
}
