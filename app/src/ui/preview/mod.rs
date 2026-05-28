use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{
    build_runtime_resource_index, build_runtime_resource_map, CompiledApp, LoadedResource,
    RuntimeResourceIndex,
};

use super::route::UiRouteMode;

mod nodes;
mod resolve;
mod style;
mod theme;
mod viewport;

pub(super) fn compiled_uses_frame_viewport(compiled: &CompiledApp) -> bool {
    let Some(scene_contract) = &compiled.scene_contract else {
        return false;
    };
    let Some(frame) = &scene_contract.frame else {
        return false;
    };
    viewport::resolve_frame_viewport(&frame.props, scene_contract.scene.profile.as_deref()).is_some()
}

pub(super) struct PreviewRuntimeContext {
    pub resources: BTreeMap<String, LoadedResource>,
    pub index: RuntimeResourceIndex,
}

pub(super) fn build_preview_runtime_context(compiled: &CompiledApp) -> PreviewRuntimeContext {
    PreviewRuntimeContext {
        index: build_runtime_resource_index(compiled),
        resources: build_runtime_resource_map(compiled),
    }
}
mod view;

pub(super) fn preview_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    route_mode: UiRouteMode,
) -> AnyView {
    view::preview_view(compiled, app_path, selected_target, route_mode)
}

#[cfg(test)]
mod tests;
