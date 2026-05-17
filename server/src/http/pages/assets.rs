use std::fs;

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::Response,
};

use crate::{AppError, AppState};

use super::static_serve::serve_static_asset;

pub async fn app_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.package_root.join("app").join("assets").join(&path),
        "app asset",
    )
}

pub async fn app_bundle(
    State(state): State<AppState>,
    AxumPath(mode): AxumPath<String>,
) -> Result<Response, AppError> {
    let assets_root = state.package_root.join("app").join("assets");
    if let Some(dist_rel_path) = app_bundle_dist_path(&mode) {
        let dist_path = assets_root.join(dist_rel_path);
        if dist_path.exists() {
            return serve_static_asset(dist_path, "app dist bundle");
        }
    }
    if matches!(mode.as_str(), "shoelace.js" | "shoelace") {
        return serve_static_asset(
            assets_root.join("shoelace-local.js"),
            "shoelace fallback bundle",
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
    Ok(response)
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

fn app_bundle_scripts(mode: &str) -> Option<&'static [&'static str]> {
    const MANAGE_SCRIPTS: &[&str] = &[
        "frame-stage.js",
        "vendor/diff-match-patch.js",
        "vendor/codemirror.js",
        "source-codemirror-mode.js",
        "vendor/codemirror-merge.js",
        "manage-tabs.js",
        "agent-panel.js",
        "workspace-splitters.js",
        "source-tree-controls.js",
        "source-highlight.js",
        "spa-navigation.js",
    ];
    const ACCESS_SCRIPTS: &[&str] = &[
        "frame-stage.js",
        "statusbar.js",
        "agent-panel.js",
        "workspace-splitters.js",
        "spa-navigation.js",
    ];
    match mode {
        "manage.js" | "manage" => Some(MANAGE_SCRIPTS),
        "access.js" | "access" => Some(ACCESS_SCRIPTS),
        _ => None,
    }
}

fn app_bundle_dist_path(mode: &str) -> Option<&'static str> {
    match mode {
        "manage.js" | "manage" => Some("dist/manage.bundle.js"),
        "access.js" | "access" => Some("dist/access.bundle.js"),
        "shoelace.js" | "shoelace" => Some("dist/shoelace.bundle.js"),
        "styles.css" | "styles" => Some("dist/styles.bundle.css"),
        _ => None,
    }
}

fn app_bundle_styles() -> &'static [&'static str] {
    &[
        "app-shell.css",
        "tailwind.css",
        "vendor/codemirror.css",
        "vendor/codemirror-merge.css",
        "vendor/shoelace/themes/dark.css",
    ]
}
