use mei_lang_kernel::{CompiledApp, CompiledSceneRoute};

use crate::ui::manage_routing::encode_query_value;
use crate::ui::route::UiRouteMode;
use crate::ui::{HostAccountView, HostCapabilities};

pub(super) fn exported_scene_by_id<'a>(
    routes: &'a [CompiledSceneRoute],
    scene_id: Option<&str>,
) -> Option<&'a str> {
    let wanted = scene_id.map(str::trim).filter(|value| !value.is_empty())?;
    routes
        .iter()
        .find(|route| route.scene_id == wanted && route.access_export)
        .map(|route| route.scene_id.as_str())
}

pub(super) fn canonical_scene_for_target<'a>(
    routes: &'a [CompiledSceneRoute],
    target_file: Option<&str>,
) -> Option<&'a str> {
    let target = target_file
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    routes
        .iter()
        .find(|route| route.target_file == target && route.access_export)
        .map(|route| route.scene_id.as_str())
}

pub(super) fn default_exported_scene(routes: &[CompiledSceneRoute]) -> Option<&str> {
    routes
        .iter()
        .find(|route| route.access_export && route.is_default)
        .or_else(|| routes.iter().find(|route| route.access_export))
        .map(|route| route.scene_id.as_str())
}

pub(super) fn preferred_access_scene<'a>(
    route_mode: UiRouteMode,
    routes: &'a [CompiledSceneRoute],
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
    active_scene: Option<&str>,
    active_target_file: &str,
) -> Option<&'a str> {
    let build_target = preview_target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(Some(active_target_file));
    let build_scene = if route_mode == UiRouteMode::Layout {
        canonical_scene_for_target(routes, build_target)
    } else {
        None
    };
    exported_scene_by_id(routes, selected_scene)
        .or(build_scene)
        .or_else(|| exported_scene_by_id(routes, active_scene))
        .or_else(|| default_exported_scene(routes))
}

pub(crate) fn access_scene_for_topbar<'a>(
    route_mode: UiRouteMode,
    compiled: &'a CompiledApp,
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
) -> Option<&'a str> {
    preferred_access_scene(
        route_mode,
        &compiled.scene_routes,
        selected_scene,
        preview_target,
        compiled.active_scene.as_deref(),
        compiled.active_target_file.as_str(),
    )
}

pub(super) fn auth_surface_tabs_visible(
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> (bool, bool, bool) {
    let caps = if auth_enabled {
        auth_account
            .map(|account| account.capabilities)
            .unwrap_or_else(|| HostCapabilities::from_role_slug("guest"))
    } else {
        HostCapabilities::auth_disabled()
    };
    (caps.config_upload, caps.build_view, caps.config_upload)
}

#[allow(dead_code)]
pub(crate) fn append_scene_query(base: String, scene_id: Option<&str>) -> String {
    let Some(scene_id) = scene_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return base;
    };
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}scene={}", encode_query_value(scene_id))
}
