use leptos::prelude::*;
use mei_lang_kernel::{
    compile_coordinate_for_node, default_build_node_for_compiled, resolve_build_node_context,
    resolve_build_view_query, BuildCompileCoordinate, BuildViewTab, CompiledApp, LegacyBuildQuery,
    WorkspaceAppMeta,
};

use super::super::manage_routing::WorldSemanticQuery;
use super::super::preview;
use super::super::preview_chrome::{asset_preview_body, workspace_component_script_urls};
use super::super::route::UiRouteMode;
use super::super::scene_drilldown_context::host_ssr_bootstrap_scripts;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildPreviewFragment {
    pub node: String,
    pub focus: String,
    pub compile_coordinate: BuildCompileCoordinate,
    pub preview_html: String,
    pub drilldown_script: String,
    pub workspace_scripts: Vec<String>,
}

pub fn render_build_preview_fragment(
    _apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    node: Option<&str>,
    scope: Option<&str>,
    focus: Option<&str>,
    tab: Option<&str>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> Option<BuildPreviewFragment> {
    let legacy = LegacyBuildQuery {
        file: None,
        scene: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        tab: tab.map(str::to_string),
    };
    let resolved = resolve_build_view_query(node, scope, tab, &legacy).unwrap_or_else(|| {
        let default_node = default_build_node_for_compiled(compiled);
        mei_lang_kernel::ResolvedBuildViewQuery {
            node: default_node.clone(),
            tab: default_node.default_tab(),
            scope: Default::default(),
        }
    });
    if resolved.tab != BuildViewTab::Preview {
        return None;
    }
    let ctx = resolve_build_node_context(compiled, &resolved.node);
    let selected_target = ctx.target_file.clone();
    let compile_coordinate = compile_coordinate_for_node(&resolved.node, compiled)?;
    let semantic = WorldSemanticQuery {
        world_metric: ctx.world_metric.as_deref(),
        world_dataset: ctx.world_dataset.as_deref(),
        explain: ctx.explain.as_deref(),
    };
    let build_preview_scope =
        mei_lang_kernel::resolve_build_preview_scope_for_ssr(compiled, &resolved.node);
    let build_preview_component_use_key_owned =
        build_preview_component_use_key(&resolved.node);
    let build_preview_component_use_key =
        build_preview_component_use_key_owned.as_deref();
    let preview = preview::preview_view(
        compiled,
        app_path,
        selected_target.as_str(),
        UiRouteMode::Build,
        semantic,
        build_preview_scope.as_deref(),
        build_preview_component_use_key,
        data_mode,
        review_projection,
    );
    let preview_body =
        if selected_target.ends_with(".mei") || selected_target.ends_with(".world.mei") {
            preview.into_any()
        } else {
            asset_preview_body(app_path, selected_target.as_str(), "").into_any()
        };
    let preview_scroll_class = "preview-pane-scroll min-h-0 min-w-0 flex-1 overflow-auto";
    let review_projection_resolved = review_projection
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("plane_region_section");
    let data_mode_attr = data_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("eval");
    let fragment = view! {
        <div
            class=preview_scroll_class
            data-review-projection=review_projection_resolved
            data-data-mode=data_mode_attr
        >
            {preview_body}
        </div>
        <div
            id="build-inspect-bar"
            class="build-inspect-bar shrink-0 border-t mei-border-default px-3 py-2 mei-font-1 mei-text-muted"
            data-build-inspect-bar="true"
        >
            <span id="build-inspect-bar-label">"在左侧体验树选择 Panel/Block，或在预览中点击组件以指认上下文。"</span>
        </div>
    };
    let drilldown_script =
        host_ssr_bootstrap_scripts(compiled, app_path, ctx.scene_id.as_deref(), data_mode)
            .to_html();
    Some(BuildPreviewFragment {
        node: resolved.node.encode(),
        focus: focus.unwrap_or("").to_string(),
        compile_coordinate,
        preview_html: fragment.to_html(),
        drilldown_script,
        workspace_scripts: workspace_component_script_urls(compiled, None),
    })
}

pub(crate) fn build_preview_component_use_key(
    node: &mei_lang_kernel::BuildNodeId,
) -> Option<String> {
    use mei_lang_kernel::BuildNodeKind;
    if node.kind != BuildNodeKind::Component {
        return None;
    }
    let key = node.key.trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}
