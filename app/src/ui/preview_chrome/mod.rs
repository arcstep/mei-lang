mod asset_preview;
mod diagnostics;
mod html_escape;
mod markdown;
mod scripts;

use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::route::UiRouteMode;

pub(crate) fn asset_preview_body(app_path: &str, target: &str, source: &str) -> AnyView {
    asset_preview::asset_preview_body(app_path, target, source)
}

pub(crate) fn diagnostics_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    selected_scene: Option<&str>,
    filter_mode: super::compile_status::DiagnosticsFilterMode,
) -> AnyView {
    diagnostics::diagnostics_view(
        compiled,
        app_path,
        selected_target,
        selected_scene,
        filter_mode,
    )
}

pub(crate) fn chrome_scripts_view(route_mode: UiRouteMode) -> AnyView {
    scripts::chrome_scripts_view(route_mode)
}

pub(crate) fn chrome_script_preloads_view(route_mode: UiRouteMode) -> AnyView {
    scripts::chrome_script_preloads_view(route_mode)
}

pub(crate) fn chrome_script_preload_markup(route_mode: UiRouteMode) -> &'static str {
    scripts::chrome_script_preload_markup(route_mode)
}

pub(crate) fn component_scripts(
    compiled: &CompiledApp,
    scene_bundle_url: Option<&str>,
) -> impl IntoView {
    scripts::component_scripts(compiled, scene_bundle_url)
}

pub(crate) fn component_script_preloads(
    compiled: &CompiledApp,
    scene_bundle_url: Option<&str>,
) -> AnyView {
    scripts::component_script_preloads(compiled, scene_bundle_url)
}
