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
}

pub(crate) fn build_preview_runtime_context(
    compiled: &CompiledApp,
    route_mode: UiRouteMode,
    build_preview_scope: Option<&str>,
    build_preview_component_use_key: Option<&str>,
    _selected_target: Option<&str>,
) -> PreviewRuntimeContext {
    PreviewRuntimeContext {
        index: build_runtime_resource_index(compiled),
        resources: build_runtime_resource_map(compiled),
        host_ssr_slim_payload: matches!(
            route_mode,
            UiRouteMode::App | UiRouteMode::Run | UiRouteMode::Copilot | UiRouteMode::Build
        ),
        build_inspect_enabled: route_mode == UiRouteMode::Build,
        build_preview_scope: build_preview_scope
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        build_preview_component_use_key: build_preview_component_use_key
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
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
) -> AnyView {
    view::preview_view(
        compiled,
        app_path,
        selected_target,
        route_mode,
        world_semantic,
        build_preview_scope,
        build_preview_component_use_key,
    )
}

#[cfg(test)]
mod tests;
