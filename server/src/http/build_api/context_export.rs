use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use mei_lang_kernel::{
    build_reachability_tree, resolve_build_node_context, resolve_build_view_query,
    tab_visible_for_node, BuildViewTab, LegacyBuildQuery, ProvenanceAnchor,
};
use mei_lang_toolchain::{format_semantic_graph_markdown, load_world_runtime_bundle};

use serde::Deserialize;

use crate::http::host_api::artifact_gate_status;
use crate::AppState;

use super::graph_markdown;

#[derive(Debug, Deserialize)]
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

    let build_url = format!("/apps/build/{app_id}?node={}", percent_encode_component(node_raw));
    let gate = artifact_gate_status(
        app_id,
        ctx.scene_id.as_deref(),
        Some(ctx.target_file.as_str()),
    );

    let mut md = String::new();
    md.push_str("## Mei Build Context\n\n");
    md.push_str(&format!("- **App**: `{app_id}`\n"));
    md.push_str(&format!("- **Node**: `{}`\n", resolved.node.encode()));
    md.push_str(&format!("- **Tab**: `{}`\n", tab.slug()));
    md.push_str(&format!("- **Intent**: `{intent}`\n"));
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

    md.push_str("### 编译摘要\n\n");
    md.push_str(&format!("- target_file: `{}`\n", ctx.target_file));
    if let Some(scene) = ctx.scene_id.as_deref() {
        md.push_str(&format!("- scene_id: `{scene}`\n"));
    }
    md.push_str(&format!("- scene_routes: {}\n", compiled.scene_routes.len()));
    md.push_str(&format!("- resources: {}\n", compiled.resources.len()));
    md.push_str(&format!("- diagnostics: {}\n", compiled.diagnostics.len()));
    md.push_str(&format!(
        "- reachability_roots: {}\n",
        build_reachability_tree(compiled).len()
    ));
    md.push('\n');

    append_provenance_section(&mut md, &ctx.provenance);
    append_graph_sections(&mut md, compiled, &ctx, include_graph);
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

fn append_provenance_section(md: &mut String, anchor: &ProvenanceAnchor) {
    md.push_str("### 溯源\n\n");
    if !anchor.file.is_empty() {
        md.push_str(&format!("- file: `{}`\n", anchor.file));
    }
    md.push_str(&format!("- symbol_id: `{}`\n", anchor.symbol_id));
    md.push_str(&format!("- symbol_kind: `{}`\n", anchor.symbol_kind));
    md.push_str(&format!("- anchor: `{}`\n", anchor.encode()));
    md.push('\n');
}

fn append_graph_sections(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    ctx: &mei_lang_kernel::BuildNodeContext,
    include_graph: &str,
) {
    let want_semantic = include_graph.is_empty() || include_graph.contains("semantic");
    let want_eval = include_graph.is_empty() || include_graph.contains("eval");
    if !want_semantic && !want_eval {
        return;
    }
    if let Some(metric_id) = ctx.world_metric.as_deref() {
        for resource in &compiled.resources {
            let Some(dataset) = resource.dataset.as_ref() else {
                continue;
            };
            if want_semantic && !dataset.runtime_analysis_graph.nodes.is_empty() {
                md.push_str("### 语义图摘要\n\n");
                md.push_str(&format_semantic_graph_markdown(
                    &dataset.runtime_analysis_graph,
                    Some(metric_id),
                ));
                md.push('\n');
            }
            if want_eval && !dataset.runtime_metric_defs.is_empty() {
                md.push_str("### 求值图摘要\n\n");
                if let Ok(plan_md) =
                    graph_markdown::eval_plan_markdown_for_metric(compiled, dataset, metric_id)
                {
                    md.push_str(&plan_md);
                    md.push('\n');
                }
            }
        }
    }
}

fn append_readiness_section(md: &mut String, app_id: &str, host_phase: &str) {
    md.push_str("### Readiness\n\n");
    md.push_str(&format!("- app_id: `{app_id}`\n"));
    md.push_str(&format!("- host_phase: `{host_phase}`\n"));
    md.push('\n');
}

fn append_suggested_tasks(
    md: &mut String,
    intent: &str,
    anchor: &ProvenanceAnchor,
    gate_error: &Option<String>,
) {
    md.push_str("### 建议 Agent 任务\n\n");
    match intent {
        "debug_render" => {
            md.push_str(&format!(
                "> 检查 `{}` 中符号 `{}` 的 projection / preview 渲染是否与 compile 结构一致。\n",
                anchor.file, anchor.symbol_id
            ));
        }
        "debug_eval" => {
            md.push_str(&format!(
                "> 检查 metric `{}` 的 eval plan / hydrate 链；对比 world 定义与 AOT artifact。\n",
                anchor.symbol_id
            ));
        }
        "debug_artifact" => {
            md.push_str("> 检查 prebuild 产物是否齐全且 revision 匹配；必要时运行 `mei-toolchain prebuild --verify`。\n");
            if let Some(err) = gate_error {
                md.push_str(&format!("> Gate 错误: `{err}`\n"));
            }
        }
        "full" => {
            md.push_str("> 掌握当前 node 的编译/runtime 真值；如需改源码，在 IDE 打开溯源 file + symbol_id。\n");
        }
        _ => {
            md.push_str(&format!(
                "> 在 IDE 打开 `{}`，定位符号 `{}`（{}）。\n",
                anchor.file, anchor.symbol_id, anchor.symbol_kind
            ));
        }
    }
}

fn markdown_error(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .body(format!("## Mei Build Context Error\n\n{message}\n"))
        .unwrap()
        .into_response()
}

fn percent_encode_component(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b))
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_tasks_lock_node_contains_symbol() {
        let mut md = String::new();
        append_suggested_tasks(
            &mut md,
            "lock_node",
            &ProvenanceAnchor {
                file: "metrics.world.mei".to_string(),
                symbol_id: "total".to_string(),
                symbol_kind: "metric".to_string(),
            },
            &None,
        );
        assert!(md.contains("total"));
    }
}
