use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::{page_body_theme_style, render_page, UiRouteMode};
use mei_lang_kernel::{load_workspace_config, WorkspaceAppMeta};
use serde::Deserialize;

use crate::build_info::fill_host_build_placeholders;
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct AppQuery {
    pub tab: Option<String>,
    pub scene: Option<String>,
}

pub async fn app_page(
    State(state): State<SharedState>,
    Path((mode, app_tail)): Path<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Response {
    let route_mode = UiRouteMode::from_slug(mode.as_str());
    let app_tail = app_tail.trim_start_matches('/').to_string();
    let (app_id, scene_id) = parse_app_scene_path(&app_tail, query.scene.as_deref());
    let guard = state.read().expect("state lock");
    if guard.ctx.app_id != app_id {
        return (StatusCode::NOT_FOUND, "app not found").into_response();
    }
    if !route_mode.is_access_like() && route_mode != UiRouteMode::Runtime {
        return (
            StatusCode::NOT_IMPLEMENTED,
            format!("route mode `{}` not supported in mei-host-shell yet", mode),
        )
            .into_response();
    }
    let scene_id = scene_id.unwrap_or_else(|| "home".to_string());
    let assemble_result = mei_host_graph::assemble_scope_from_registry(
        guard.ctx.workspace_root.as_path(),
        app_id.as_str(),
        scene_id.as_str(),
    );
    let outcome = match assemble_result {
        Ok(Some(outcome)) => outcome,
        Ok(None) => {
            tracing::warn!(app_id = %app_id, scene_id = %scene_id, "assemble returned None (empty registry or missing scene)");
            return (StatusCode::NOT_FOUND, "scene not assembled").into_response();
        }
        Err(error) => {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_id,
                error = %error,
                "assemble failed"
            );
            return (
                StatusCode::NOT_FOUND,
                format!("scene not assembled: {error}"),
            )
                .into_response();
        }
    };
    let workspace = load_workspace_config(guard.ctx.workspace_root.as_path());
    let theme_style = page_body_theme_style(&workspace, Some(&outcome.compiled), None);
    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
        guard.ctx.app_root().as_path(),
        Some(guard.ctx.workspace_root.as_path()),
        None,
    );
    let apps = vec![WorkspaceAppMeta {
        id: app_id.clone(),
        title: outcome.compiled.title.clone(),
        root: guard.ctx.app_root().display().to_string(),
    }];
    let html = crate::gis_config::fill_gis_tiles_placeholders(
        inject_client_bootstrap_script(
            fill_host_build_placeholders(
                render_page(
                &apps,
                &outcome.compiled,
                app_id.as_str(),
                None,
                route_mode,
                Some(outcome.compiled.active_target_file.as_str()),
                None,
                None,
                Some(scene_id.as_str()),
                None,
                query.tab.as_deref(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                None,
                &[],
                false,
                None,
                None,
                theme_style.as_str(),
                None,
                None,
            ),
            guard.ctx.workspace_root.as_path(),
        ),
            guard.ctx.workspace_root.as_path(),
            app_id.as_str(),
            scene_id.as_str(),
        ),
        &gis,
    );
    Html(html).into_response()
}

pub async fn index(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    Redirect::temporary(&format!(
        "/apps/app/{}/scene/home",
        guard.ctx.app_id
    ))
}

fn parse_app_scene_path(app_tail: &str, scene_query: Option<&str>) -> (String, Option<String>) {
    let parts: Vec<&str> = app_tail.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return (String::new(), scene_query.map(str::to_string));
    }
    let app_id = parts[0].to_string();
    let scene = if parts.len() >= 3 && parts[1] == "scene" {
        Some(parts[2].to_string())
    } else {
        scene_query.map(str::to_string)
    };
    (app_id, scene)
}

fn inject_client_bootstrap_script(
    html: String,
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
) -> String {
    let Some(manifest) = mei_host_graph::read_client_bootstrap(workspace_root, app_id, scene_id)
    else {
        return html;
    };
    let registry =
        mei_host_graph::MrgRegistryWriter::load(workspace_root, app_id);
    if !mei_host_graph::bootstrap_embed_allowed(&registry, &manifest) {
        return html;
    }
    let metrics_json = serde_json::to_string(&manifest.metrics).unwrap_or_else(|_| "[]".to_string());
    let script = format!(
        r#"<script>window.__mei=window.__mei||{{}};window.__mei.client_revision={client_revision:?};window.__mei.bootstrap_scope={scope:?};window.__mei.bootstrap_metrics={metrics_json};</script>"#,
        client_revision = manifest.client_revision,
        scope = manifest.scope,
        metrics_json = metrics_json,
    );
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + script.len());
        out.push_str(&html[..pos]);
        out.push_str(&script);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{script}{html}")
    }
}
