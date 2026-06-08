use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::CompiledApp;

use crate::auth::AuthPrincipal;
use crate::http::host_error_page::{self, HostShellAction};

use crate::http::pages::app::page_render::html_escape_min;
use crate::http::pages::app::query::access_canonical_location;

pub(super) fn check_access_scene_gate(
    route_mode: UiRouteMode,
    app_id: &str,
    access_static_file: Option<&str>,
    access_path_scene: Option<&str>,
    compiled: &CompiledApp,
    principal: Option<&AuthPrincipal>,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> Option<Response> {
    if route_mode != UiRouteMode::App || access_static_file.is_some() {
        return None;
    }
    if access_path_scene.is_none() {
        let sid = compiled
            .active_scene
            .clone()
            .filter(|s| !s.trim().is_empty());
        if let Some(ref s) = sid {
            return Some(
                Redirect::temporary(&access_canonical_location(
                    app_id,
                    s,
                    tab,
                    chrome,
                ))
                .into_response(),
            );
        }
        let loc = format!("/apps/build/{}", app_id.trim_start_matches('/'));
        return Some(Redirect::temporary(&loc).into_response());
    }
    let requested = access_path_scene?;
    let rt = requested.trim();
    if let Some(route) = compiled.scene_routes.iter().find(|r| r.scene_id == rt) {
        if !route.access_export {
            let app_esc = html_escape_min(app_id.trim_start_matches('/'));
            let scene_esc = html_escape_min(rt);
            let detail = format!("app={app_esc} scene={scene_esc}");
            return Some((
                StatusCode::FORBIDDEN,
                Html(host_error_page::render_error_page(
                    StatusCode::FORBIDDEN,
                    "场景未导出",
                    "该场景未开启访问侧导出（access_export=false）。",
                    Some(detail.as_str()),
                    &[
                        HostShellAction {
                            href: format!("/apps/build/{app_esc}"),
                            label: "返回构建视图".to_string(),
                            primary: true,
                        },
                        HostShellAction {
                            href: "/".to_string(),
                            label: "返回首页".to_string(),
                            primary: false,
                        },
                    ],
                )),
            )
                .into_response());
        }
    }
    if compiled.active_scene.as_deref() != Some(rt) {
        let app_esc = html_escape_min(app_id.trim_start_matches('/'));
        let scene_esc = html_escape_min(rt);
        let manage_href_app = app_id.trim_start_matches('/');
        let detail = format!("app={app_esc} scene={scene_esc}");
        return Some((
            StatusCode::NOT_FOUND,
            Html(host_error_page::render_error_page(
                StatusCode::NOT_FOUND,
                "场景不存在",
                "该场景不存在，或无法绑定到当前编译结果。",
                Some(detail.as_str()),
                &[HostShellAction {
                    href: format!("/apps/build/{manage_href_app}"),
                    label: "返回构建视图".to_string(),
                    primary: true,
                }],
            )),
        )
            .into_response());
    }
    if let Some(principal) = principal {
        if !principal.can_access_scene(app_id, rt) {
            let app_esc = html_escape_min(app_id.trim_start_matches('/'));
            let scene_esc = html_escape_min(rt);
            let detail = format!("app={app_esc} scene={scene_esc}");
            return Some((
                StatusCode::FORBIDDEN,
                Html(host_error_page::render_error_page(
                    StatusCode::FORBIDDEN,
                    "访问受限",
                    "当前账号未被授权访问此应用场景。",
                    Some(detail.as_str()),
                    &[
                        HostShellAction {
                            href: "/login".to_string(),
                            label: "重新登录".to_string(),
                            primary: true,
                        },
                        HostShellAction {
                            href: "/".to_string(),
                            label: "返回首页".to_string(),
                            primary: false,
                        },
                    ],
                )),
            )
                .into_response());
        }
    }
    None
}
