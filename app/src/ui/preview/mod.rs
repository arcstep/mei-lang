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
mod style;
mod theme;
mod viewport;
mod world_capsule_preview;

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
}

pub(crate) fn build_preview_runtime_context(compiled: &CompiledApp) -> PreviewRuntimeContext {
    PreviewRuntimeContext {
        index: build_runtime_resource_index(compiled),
        resources: build_runtime_resource_map(compiled),
    }
}
mod view;

pub(crate) fn preview_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    route_mode: UiRouteMode,
    world_semantic: WorldSemanticQuery<'_>,
) -> AnyView {
    view::preview_view(
        compiled,
        app_path,
        selected_target,
        route_mode,
        world_semantic,
    )
}

#[cfg(test)]
mod tests;
