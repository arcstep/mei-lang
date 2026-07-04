use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{
    build_runtime_resource_index, build_runtime_resource_map, CompiledApp, LoadedResource,
    RuntimeResourceIndex,
};

use super::route::UiRouteMode;
use crate::ui::manage_routing::WorldSemanticQuery;

mod nodes;
mod resolve;
pub(crate) use resolve::host_runtime_capabilities_value;
mod style;
mod theme;
pub use theme::{
    default_shell_body_theme_style, page_body_theme_style, scene_theme_style_for_theme_id,
    scene_viewport_theme_style,
    shell_body_theme_style,
};
mod viewport;
mod world_capsule;

pub(crate) fn compiled_uses_frame_viewport(compiled: &CompiledApp) -> bool {
    let Some(scene_contract) = &compiled.scene_contract else {
        return false;
    };
    let Some(frame) = &scene_contract.frame else {
        return false;
    };
    viewport::resolve_frame_viewport(&frame.props, scene_contract.scene.profile.as_deref())
        .is_some()
}

pub(crate) struct PreviewRuntimeContext {
    pub resources: BTreeMap<String, LoadedResource>,
    pub index: RuntimeResourceIndex,
    /// Host 视图 SSR 不内联 dataset 行集与大块指标值，改由运行时 API 拉取。
    pub host_ssr_slim_payload: bool,
    /// Build 视图：为 panel/block 注入 `data-build-node` 供检视高亮。
    pub build_inspect_enabled: bool,
    /// Build 视图：panel 级 SSR 切片 scope（scene-relative panel path）。
    pub build_preview_scope: Option<String>,
    /// Build 视图：component 节点仅渲染匹配的 `use_key` block。
    pub build_preview_component_use_key: Option<String>,
    /// 审阅数据模式：`eval` | `fixture` | `static`。
    pub data_mode: Option<String>,
    /// 审阅投影：`plane` | `plane_region` | `plane_region_section` | `static_full` | `live_full`。
    pub review_projection: Option<String>,
}

impl PreviewRuntimeContext {
    pub(crate) fn review_projection_max_ui_role(&self) -> Option<&'static str> {
        self.review_projection
            .as_deref()
            .and_then(mei_lang_kernel::ReviewProjection::parse)
            .and_then(|projection| projection.max_ui_role_depth())
    }

    pub(crate) fn ui_role_allowed_for_projection(&self, ui_role: &str) -> bool {
        if !self.build_inspect_enabled {
            return true;
        }
        let Some(max_role) = self.review_projection_max_ui_role() else {
            return true;
        };
        mei_lang_kernel::ui_role_within_max_depth(ui_role, Some(max_role))
    }
}

pub(crate) fn build_preview_runtime_context(
    compiled: &CompiledApp,
    route_mode: UiRouteMode,
    build_preview_scope: Option<&str>,
    build_preview_component_use_key: Option<&str>,
    _selected_target: Option<&str>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> PreviewRuntimeContext {
    let data_mode = data_mode
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let review_projection = review_projection
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let host_ssr_slim_payload = match data_mode.as_deref() {
        Some("static") => false,
        _ => matches!(
            route_mode,
            UiRouteMode::App | UiRouteMode::Run | UiRouteMode::Copilot | UiRouteMode::Build
        ),
    };
    PreviewRuntimeContext {
        index: build_runtime_resource_index(compiled),
        resources: build_runtime_resource_map(compiled),
        host_ssr_slim_payload,
        build_inspect_enabled: route_mode == UiRouteMode::Build,
        build_preview_scope: build_preview_scope
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        build_preview_component_use_key: build_preview_component_use_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        data_mode,
        review_projection,
    }
}
mod view;

pub(crate) fn preview_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    route_mode: UiRouteMode,
    world_semantic: WorldSemanticQuery<'_>,
    build_preview_scope: Option<&str>,
    build_preview_component_use_key: Option<&str>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> AnyView {
    view::preview_view(
        compiled,
        app_path,
        selected_target,
        route_mode,
        world_semantic,
        build_preview_scope,
        build_preview_component_use_key,
        data_mode,
        review_projection,
    )
}

#[cfg(test)]
mod tests;
