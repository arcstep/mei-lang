mod asset_preview;
mod diagnostics;
mod html_escape;
mod markdown;
mod scripts;

use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::route::UiRouteMode;

pub(super) fn asset_preview_body(app_path: &str, target: &str, source: &str) -> AnyView {
    asset_preview::asset_preview_body(app_path, target, source)
}

pub(super) fn diagnostics_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    filter_mode: super::compile_status::DiagnosticsFilterMode,
) -> AnyView {
    diagnostics::diagnostics_view(compiled, app_path, selected_target, filter_mode)
}

pub(super) fn chrome_scripts_view(route_mode: UiRouteMode) -> AnyView {
    scripts::chrome_scripts_view(route_mode)
}

pub(super) fn component_scripts(compiled: &CompiledApp) -> impl IntoView {
    scripts::component_scripts(compiled)
}
