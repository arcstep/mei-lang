use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use mei_lang_kernel::{
    build_reachability_tree, resolve_build_node_context,
    resolve_build_view_query, tab_visible_for_node, BuildViewTab, LegacyBuildQuery,
};
use mei_lang_toolchain::load_world_runtime_bundle;

use serde::Deserialize;

use crate::http::host_api::artifact_gate_status;
use crate::AppState;

use super::support::*;
use super::append::*;

#[derive(Deserialize)]
pub struct BuildContextExportQuery {
    pub app_id: String,
    pub node: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub include_graph: Option<String>,
    #[serde(default)]
    pub include_readiness: Option<String>,
}

pub async fn api_build_context_export(
    State(state): State<AppState>,
    Query(query): Query<BuildContextExportQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return markdown_error(StatusCode::BAD_REQUEST, "app_id is required");
    }
    let node_raw = query.node.trim();
    if node_raw.is_empty() {
        return markdown_error(StatusCode::BAD_REQUEST, "node is required");
    }
    let legacy = LegacyBuildQuery {
        file: None,
        scene: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        tab: query.tab.clone(),
    };
    let Some(resolved) = resolve_build_view_query(
        Some(node_raw),
        query.scope.as_deref(),
        query.tab.as_deref(),
        &legacy,
    ) else {
        return markdown_error(StatusCode::BAD_REQUEST, "invalid node id");
    };
    let tab = query
        .tab
        .as_deref()
        .and_then(BuildViewTab::parse_slug)
        .filter(|candidate| tab_visible_for_node(&resolved.node, *candidate))
        .unwrap_or(resolved.tab);

    let bundle = match load_world_runtime_bundle(state.source_root.as_path(), app_id, None) {
        Ok(bundle) => bundle,
        Err(error) => {
            return markdown_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to load runtime bundle: {error}"),
            );
        }
    };
    let compiled = &bundle.compiled;
    let ctx = resolve_build_node_context(compiled, &resolved.node);
    let intent = query
        .intent
        .as_deref()
        .unwrap_or("lock_node")
        .trim()
        .to_ascii_lowercase();
    let include_graph = query.include_graph.as_deref().unwrap_or("");
    let include_readiness = query
        .include_readiness
        .as_deref()
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(true);

    let build_url = {
        let mut url = format!(
            "/apps/{app_id}/layout?node={}",
            percent_encode_component(node_raw)
        );
        if let Some(focus) = query
            .focus
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            url.push_str("&focus=");
            url.push_str(&percent_encode_component(focus));
        }
        if let Some(dm) = query
            .data_mode
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            url.push_str("&data_mode=");
            url.push_str(&percent_encode_component(dm));
        }
        if let Some(rp) = query
            .review_projection
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            url.push_str("&review_projection=");
            url.push_str(&percent_encode_component(rp));
        }
        url
    };
    let data_mode = query
        .data_mode
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("eval");
    let review_projection = query
        .review_projection
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("plane_region_section");
    let gate = artifact_gate_status(
        app_id,
        ctx.scene_id.as_deref(),
        Some(ctx.target_file.as_str()),
    );

    let mut md = String::new();
    md.push_str("## Mei Prototype Debug Context\n\n");
    md.push_str(&format!("- **App**: `{app_id}`\n"));
    md.push_str(&format!("- **Node**: `{}`\n", resolved.node.encode()));
    md.push_str(&format!("- **Tab**: `{}`\n", tab.slug()));
    md.push_str(&format!("- **Intent**: `{intent}`\n"));
    md.push_str(&format!("- **data_mode**: `{data_mode}`\n"));
    md.push_str(&format!("- **review_projection**: `{review_projection}`\n"));
    md.push_str(&format!("- **Build URL**: `{build_url}`\n"));
    md.push_str(&format!(
        "- **Gate**: host=`{}` app=`{}` scope=`{}`\n",
        gate.host_phase,
        gate.app_phase.unwrap_or_else(|| "unknown".to_string()),
        gate.scope_phase.unwrap_or_else(|| "unknown".to_string())
    ));
    if let Some(err) = gate.last_error.as_deref() {
        md.push_str(&format!("- **Gate error**: `{err}`\n"));
    }
    md.push('\n');

    append_ux_sections(
        &mut md,
        compiled,
        &ctx,
        &resolved.node,
        query.focus.as_deref(),
    );
    append_board_template_sections(&mut md, compiled, &resolved.node);
    append_runtime_snapshot(&mut md, compiled, &ctx, &intent);

    md.push_str("### 编译摘要\n\n");
    md.push_str(&format!("- target_file: `{}`\n", ctx.target_file));
    if let Some(scene) = ctx.scene_id.as_deref() {
        md.push_str(&format!("- scene_id: `{scene}`\n"));
    }
    md.push_str(&format!(
        "- scene_routes: {}\n",
        compiled.scene_routes.len()
    ));
    md.push_str(&format!("- resources: {}\n", compiled.resources.len()));
    md.push_str(&format!("- diagnostics: {}\n", compiled.diagnostics.len()));
    md.push_str(&format!(
        "- reachability_roots: {}\n",
        build_reachability_tree(compiled).len()
    ));
    md.push('\n');

    append_provenance_section(&mut md, &ctx.provenance);
    append_graph_sections(&mut md, compiled, &ctx, include_graph);
    append_registry_graph_sections(
        &mut md,
        state.source_root.as_path(),
        app_id,
        include_graph,
        &resolved.node,
    );
    if include_readiness {
        append_readiness_section(&mut md, app_id, &gate.host_phase);
    }
    append_suggested_tasks(&mut md, &intent, &ctx.provenance, &gate.last_error);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .body(md)
        .unwrap()
        .into_response()
}

