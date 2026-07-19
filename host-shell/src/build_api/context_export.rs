use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use mei_lang_app::prototype_preset::{default_build_preset, match_preset};
use mei_lang_kernel::{
    build_reachability_tree, resolve_build_node_context, resolve_build_view_query,
    tab_visible_for_node, BuildViewTab, LegacyBuildQuery,
};
use serde::Deserialize;

use crate::build_api::assemble::{assemble_enriched_for_build_node, AssembleBuildError};
use crate::build_api::context_append::{
    append_board_template_sections, append_provenance_section, append_readiness_section,
    append_registry_graph_sections, append_runtime_snapshot, append_suggested_tasks,
    append_ux_sections,
};
use crate::state::SharedState;

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
    #[serde(default)]
    pub surface: Option<String>,
}

pub async fn api_build_context_export(
    State(state): State<SharedState>,
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

    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    let host_phase = guard.startup_phase.clone();
    let gate_error = guard.startup_error.clone();
    let app_imported = guard.imported;
    drop(guard);

    let assembled =
        match assemble_enriched_for_build_node(workspace_root.as_path(), app_id, node_raw, None) {
            Ok(value) => value,
            Err(AssembleBuildError::InvalidNode) => {
                return markdown_error(StatusCode::BAD_REQUEST, "invalid node id");
            }
            Err(error) => {
                let status = match &error {
                    AssembleBuildError::NotAssembled(_) => StatusCode::NOT_FOUND,
                    AssembleBuildError::AssembleFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    AssembleBuildError::InvalidNode => StatusCode::BAD_REQUEST,
                };
                return markdown_error(status, error.message().as_str());
            }
        };

    let compiled = &assembled.compiled;
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

    let surface =
        resolve_export_surface(query.surface.as_deref(), query.review_projection.as_deref());

    let build_url = {
        let route = if surface == "prototype" {
            "prototype"
        } else {
            "layout"
        };
        let mut url = format!(
            "/apps/{app_id}/{route}?node={}",
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
        .unwrap_or_else(|| {
            if surface == "prototype" || surface == "layout" {
                "static"
            } else {
                "eval"
            }
        });
    let review_projection = query
        .review_projection
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("plane_region_section_slot");
    let preset = match_preset(data_mode, review_projection)
        .map(|value| value.slug)
        .unwrap_or_else(|| default_build_preset().slug);
    let scene_id = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut md = String::new();
    let title = if surface == "prototype" {
        "## Mei Prototype Context\n\n"
    } else {
        "## Mei Layout Context\n\n"
    };
    md.push_str(title);
    md.push_str(&format!("- **surface**: `{surface}`\n"));
    md.push_str(&format!("- **App**: `{app_id}`\n"));
    md.push_str(&format!("- **Node**: `{}`\n", resolved.node.encode()));
    md.push_str(&format!("- **Tab**: `{}`\n", tab.slug()));
    md.push_str(&format!("- **Intent**: `{intent}`\n"));
    md.push_str(&format!("- **preset**: `{preset}`\n"));
    md.push_str(&format!("- **data_mode**: `{data_mode}`\n"));
    md.push_str(&format!("- **review_projection**: `{review_projection}`\n"));
    if let Some(scope) = query.scope.as_deref().filter(|s| !s.trim().is_empty()) {
        md.push_str(&format!("- **scope**: `{scope}`\n"));
    }
    if let Some(scene) = scene_id {
        md.push_str(&format!("- **scene**: `{scene}`\n"));
    }
    md.push_str(&format!("- **Build URL**: `{build_url}`\n"));
    md.push_str(&format!(
        "- **Gate**: host=`{host_phase}` app=`{}` scope=`registry`\n",
        if app_imported { "ready" } else { "importing" }
    ));
    if let Some(err) = gate_error.as_deref() {
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
    if surface == "layout" {
        append_layout_surface_sections(&mut md, compiled, &resolved.node);
    } else if surface == "prototype" {
        append_prototype_surface_sections(&mut md, compiled, &resolved.node);
    }
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
    md.push_str(&format!(
        "- stage_registry: {}\n",
        compiled.stage_registry.stages.len()
    ));
    md.push_str(&format!(
        "- stage_programs: {}\n",
        compiled.stage_programs.programs.len()
    ));
    if let Some(stage_id) = ctx.scene_id.as_deref() {
        if let Some(program) = crate::review_axes::stage_program_for(compiled, stage_id) {
            md.push_str(&format!(
                "- active_stage_program: `{}` profile=`{}` units={} source=`{}`\n",
                program.stage_id.as_str(),
                program.profile.as_str(),
                program.units.len(),
                program.source_anchor.replace('\\', "/")
            ));
            if !program.units.is_empty() {
                let unit_ids: Vec<&str> = program.unit_ids();
                md.push_str(&format!(
                    "- stage_program_units: `{}`\n",
                    unit_ids.join(", ")
                ));
            }
            if let Some(slot_ref) = program.slot_module_ref.as_deref() {
                md.push_str(&format!("- slot_module_ref: `{slot_ref}`\n"));
            }
            if let Some(digest) = program.structure_digest.as_deref() {
                md.push_str(&format!("- structure_digest: `{digest}`\n"));
            }
            if let Some(digest) = program.narration_digest.as_deref() {
                md.push_str(&format!("- narration_digest: `{digest}`\n"));
            }
            md.push_str(&format!(
                "- stage_surface: `{}`\n",
                program.surface.as_str()
            ));
            let policy = mei_lang_kernel::ProfileLayoutPolicy::for_profile(program.profile);
            md.push_str(&format!(
                "- profile_layout_policy: `{}`\n",
                policy.summary_label()
            ));
            if program.source_anchor.contains(".stage.mdx") {
                md.push_str(&format!(
                    "- stage_mdx_source: `{}`\n",
                    program.source_anchor.replace('\\', "/")
                ));
            }
        }
    }
    md.push_str(&format!(
        "- stage_registry_count: {}\n",
        compiled.stage_registry.stages.len()
    ));
    md.push_str(&format!(
        "- scene_slot_modules: {}\n",
        compiled.scene_slot_modules.len()
    ));
    md.push_str(&format!(
        "- content_capabilities: {}\n",
        compiled.content_capabilities.len()
    ));
    let world_cap_count = compiled
        .content_capabilities
        .values()
        .filter(|c| c.is_world())
        .count();
    md.push_str(&format!(
        "- world_content_capabilities: {} (not Stage identity)\n",
        world_cap_count
    ));
    md.push_str(&format!(
        "- narration_catalogs: {}\n",
        compiled.narration_catalogs.len()
    ));
    if let Some(stage_id) = ctx.scene_id.as_deref() {
        let module_key = format!("scene:{stage_id}");
        if let Some(module) = compiled.scene_slot_modules.get(&module_key) {
            md.push_str(&format!(
                "- public_slots: `{}`\n",
                module.slot_ids().join(", ")
            ));
        }
        let narr_key = format!("narration:{stage_id}");
        if let Some(catalog) = compiled.narration_catalogs.get(&narr_key) {
            md.push_str(&format!("- narration_cues: {}\n", catalog.cue_count()));
        }
    }
    md.push_str(&format!("- resources: {}\n", compiled.resources.len()));
    md.push_str(&format!("- diagnostics: {}\n", compiled.diagnostics.len()));
    md.push_str(&format!(
        "- reachability_roots: {}\n",
        build_reachability_tree(compiled).len()
    ));
    md.push_str(&format!(
        "- compile_revision: `{}`\n",
        assembled.compile_revision
    ));
    md.push('\n');

    append_provenance_section(&mut md, &ctx.provenance);
    append_registry_graph_sections(
        &mut md,
        workspace_root.as_path(),
        app_id,
        include_graph,
        &resolved.node,
    );
    if include_readiness {
        append_readiness_section(&mut md, app_id, host_phase.as_str());
    }
    append_suggested_tasks(&mut md, &intent, &ctx.provenance, &gate_error);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
        .body(md)
        .unwrap()
        .into_response()
}

fn resolve_export_surface(surface: Option<&str>, review_projection: Option<&str>) -> &'static str {
    if let Some(value) = surface.map(str::trim).filter(|s| !s.is_empty()) {
        if value.eq_ignore_ascii_case("prototype") {
            return "prototype";
        }
        if value.eq_ignore_ascii_case("layout") {
            return "layout";
        }
    }
    if review_projection
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("static_full"))
    {
        return "prototype";
    }
    "layout"
}

fn append_layout_surface_sections(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    node: &mei_lang_kernel::BuildNodeId,
) {
    md.push_str("### 布局工作区提示\n\n");
    md.push_str(
        "- 预览为 slot 沙盘：不渲染 content，仅验证 plane/region/section/slot 与 theme.layout。\n",
    );
    md.push_str(
        "- session draft：`theme.layout.session`；确认后 `POST /api/ops/themes/layout/apply`。\n",
    );
    if let Some(manifest) = (!compiled.ui_layout_index.nodes.is_empty()).then(|| {
        compiled
            .ui_layout_index
            .layout_budget_manifest(compiled.app_id.as_str())
    }) {
        md.push_str("\n### layout_budget_manifest（摘要）\n\n");
        for (scope, entry) in manifest.entries.iter().take(24) {
            md.push_str(&format!("- `{scope}`: "));
            if let Some(height) = entry.slot_height_px {
                md.push_str(&format!("slot_height_px={height} "));
            }
            if let Some(profile) = entry.padding_profile.as_deref() {
                md.push_str(&format!("padding_profile={profile} "));
            }
            md.push('\n');
        }
        md.push('\n');
    }
    let _ = node;
}

fn append_prototype_surface_sections(
    md: &mut String,
    compiled: &mei_lang_kernel::CompiledApp,
    node: &mei_lang_kernel::BuildNodeId,
) {
    md.push_str("### 原型工作区提示\n\n");
    md.push_str("- 预览拓扑与 App 一致，但 `data_mode=static`：数值为 `static_skeleton` 桩，禁止与 eval 真值混淆。\n");
    md.push_str("- content/sources 变更须写 Config/Upload，不走 session draft。\n");
    if let Some(scope_md) = mei_lang_kernel::format_ui_scope_agent_context(compiled, node) {
        md.push_str(&scope_md);
        md.push_str("\n");
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
