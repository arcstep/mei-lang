use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, CompiledSceneRoute, WorkspaceAppMeta};
use std::collections::BTreeMap;

use super::super::manage_routing::{access_scene_query, encode_query_value};
use super::super::route::UiRouteMode;
use super::super::view_routing::{app_scene_href, build_href, config_href, cross_app_href, upload_href};
use super::super::{HostAccountView, HostCapabilities, TopbarMenuContext};

use super::menu_groups::build_topbar_menu_groups;

fn exported_scene_by_id<'a>(
    routes: &'a [CompiledSceneRoute],
    scene_id: Option<&str>,
) -> Option<&'a str> {
    let wanted = scene_id.map(str::trim).filter(|value| !value.is_empty())?;
    routes
        .iter()
        .find(|route| route.scene_id == wanted && route.access_export)
        .map(|route| route.scene_id.as_str())
}

fn canonical_scene_for_target<'a>(
    routes: &'a [CompiledSceneRoute],
    target_file: Option<&str>,
) -> Option<&'a str> {
    let target = target_file.map(str::trim).filter(|value| !value.is_empty())?;
    routes
        .iter()
        .find(|route| route.target_file == target && route.access_export)
        .map(|route| route.scene_id.as_str())
}

fn default_exported_scene(routes: &[CompiledSceneRoute]) -> Option<&str> {
    routes
        .iter()
        .find(|route| route.access_export && route.is_default)
        .or_else(|| routes.iter().find(|route| route.access_export))
        .map(|route| route.scene_id.as_str())
}

fn preferred_access_scene<'a>(
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
    let build_scene = if route_mode == UiRouteMode::Build {
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

fn auth_surface_tabs_visible(
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
    (caps.config_upload, caps.config_upload, caps.build_view)
}

fn append_scene_query(base: String, scene_id: Option<&str>) -> String {
    let Some(scene_id) = scene_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return base;
    };
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}scene={}", encode_query_value(scene_id))
}

pub(crate) fn topbar_view(
    apps: &[WorkspaceAppMeta],
    active_app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
    access_scene_for_href: Option<&str>,
    build_file: Option<&str>,
    active_tab: Option<&str>,
    upload_enabled: bool,
    stage_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let access_entry_query = access_scene_query(access_scene_for_href);
    let access_disabled = access_entry_query.is_empty();
    let menu_groups = build_topbar_menu_groups(apps, topbar_menu);
    let active_app_label = menu_groups
        .iter()
        .flat_map(|group| group.items.iter())
        .find(|item| item.app_id.as_str() == active_app_path)
        .map(|item| item.label.clone())
        .or_else(|| {
            apps.iter()
                .find(|app| app.id.as_str() == active_app_path)
                .map(|app| app.title.clone())
        })
        .unwrap_or_else(|| active_app_path.to_string());
    let workspace_label = topbar_menu
        .and_then(|menu| menu.workspace_label.as_deref())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("工作区");
    let app_tabs = menu_groups
        .into_iter()
        .map(|group| {
            let group_id = group.id.clone();
            let group_label = group.label.clone();
            let group_has_active = group
                .items
                .iter()
                .any(|item| item.app_id.as_str() == active_app_path);
            let trigger_class = if group_has_active {
                "app-group-trigger is-active"
            } else {
                "app-group-trigger"
            };
            let mut direct_items = Vec::new();
            let mut subgroup_items: BTreeMap<String, Vec<_>> = BTreeMap::new();
            for item in &group.items {
                if let Some(subgroup) = &item.subgroup {
                    subgroup_items
                        .entry(subgroup.clone())
                        .or_default()
                        .push(item.clone());
                } else {
                    direct_items.push(item.clone());
                }
            }
            let is_single_top_level_tab =
                direct_items.len() == 1 && subgroup_items.is_empty();
            if is_single_top_level_tab {
                let item = &direct_items[0];
                let class = if item.app_id.as_str() == active_app_path {
                    "app-tab active"
                } else {
                    "app-tab"
                };
                let href = cross_app_href(route_mode, &item.app_id);
                return view! {
                    <a class=class href=href data-topbar-menu-group=group_id.clone()>
                        {item.label.clone()}
                    </a>
                }
                .into_any();
            }
            let direct_links = direct_items
                .iter()
                .map(|item| {
                    let class = if item.app_id.as_str() == active_app_path {
                        "app-tab app-tab-sub active"
                    } else {
                        "app-tab app-tab-sub"
                    };
                    let href = cross_app_href(route_mode, &item.app_id);
                    view! { <a class=class href=href>{item.label.clone()}</a> }
                })
                .collect_view();
            let subgroup_blocks = subgroup_items
                .into_iter()
                .map(|(subgroup, items)| {
                    let links = items
                        .iter()
                        .map(|item| {
                            let class = if item.app_id.as_str() == active_app_path {
                                "app-tab app-tab-sub active"
                            } else {
                                "app-tab app-tab-sub"
                            };
                            let href = cross_app_href(route_mode, &item.app_id);
                            view! { <a class=class href=href>{item.label.clone()}</a> }
                        })
                        .collect_view();
                    view! {
                        <section class="app-subgroup">
                            <h4 class="app-subgroup-title">{subgroup}</h4>
                            <div class="app-subgroup-items">{links}</div>
                        </section>
                    }
                })
                .collect_view();
            view! {
                <sl-dropdown
                    class="app-group-dropdown"
                    data-topbar-menu-group=group_id.clone()
                    placement="bottom-start"
                    distance="4"
                    hoist=true
                >
                    <sl-button
                        slot="trigger"
                        class=trigger_class
                        size="small"
                        caret=true
                    >
                        {group_label}
                    </sl-button>
                    <div class="app-group-menu">
                        {direct_links}
                        {subgroup_blocks}
                    </div>
                </sl-dropdown>
            }
            .into_any()
        })
        .collect_view();
    let breadcrumb_aria = format!("当前应用：{workspace_label} / {active_app_label}");
    let active_item_breadcrumb = view! {
        <div class="app-current-path inline-flex min-w-0 max-w-[min(300px,30vw)] items-center gap-1 border-l border-slate-400/15 pl-2 text-[11px] text-slate-400" aria-label=breadcrumb_aria>
            <span class="app-current-path-prefix shrink-0 text-slate-500">"应用："</span>
            <span class="app-current-path-trail inline-flex min-w-0 items-center gap-1 whitespace-nowrap">
                <span class="app-current-path-workspace shrink-0 text-slate-400">{workspace_label}</span>
                <span class="app-current-path-separator shrink-0 text-slate-400/70" aria-hidden="true">"/"</span>
                <span class="app-current-path-item min-w-0 overflow-hidden text-ellipsis text-slate-200">{active_app_label}</span>
            </span>
        </div>
    }
    .into_any();
    let build_file = build_file
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("main.mei");
    let app_href = if access_disabled {
        "#".to_string()
    } else {
        app_scene_href(active_app_path, access_scene_for_href, active_tab, None)
    };
    let build_href = build_href(
        active_app_path,
        Some(build_file),
        active_tab,
    );
    let config_href = append_scene_query(config_href(active_app_path), access_scene_for_href);
    let upload_href = append_scene_query(upload_href(active_app_path, None), access_scene_for_href);
    let presentation_href = app_scene_href(active_app_path, access_scene_for_href, None, Some("none"));
    let (show_config_tab, show_upload_tab, show_build_tab) =
        auth_surface_tabs_visible(auth_enabled, auth_account);
    let show_upload_mode = upload_enabled && show_upload_tab;
    let visible_mode_tab_count = 1usize
        + usize::from(show_upload_mode)
        + usize::from(show_config_tab)
        + usize::from(show_build_tab);
    let mode_tabs = if visible_mode_tab_count <= 1 {
        view! { <></> }.into_any()
    } else {
        view! {
        <div class="mode-tabs inline-flex items-center">
            <sl-button-group class="mode-tab-group" label="视图切换" data-mei-view-tabs="1">
                <sl-button
                    class=if route_mode == UiRouteMode::App { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                    size="small"
                    href=app_href.clone()
                    disabled=access_disabled
                    title=if access_disabled { "当前没有可发布的 scene route" } else { "访问" }
                    aria-label="访问"
                    data-mei-view="app"
                >
                    <span class="mode-label">"访问"</span>
                </sl-button>
                {if show_upload_mode {
                    view! {
                        <sl-button
                            class=if route_mode == UiRouteMode::Upload { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                            size="small"
                            href=upload_href.clone()
                            title="上传"
                            aria-label="上传"
                            data-mei-view="upload"
                        >
                            <span class="mode-label">"上传"</span>
                        </sl-button>
                    }.into_any()
                } else {
                    view! { <></> }.into_any()
                }}
                {if show_config_tab {
                    view! {
                        <sl-button
                            class=if route_mode == UiRouteMode::Config { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                            size="small"
                            href=config_href.clone()
                            title="配置"
                            aria-label="配置"
                            data-mei-view="config"
                        >
                            <span class="mode-label">"配置"</span>
                        </sl-button>
                    }.into_any()
                } else {
                    view! { <></> }.into_any()
                }}
                {if show_build_tab {
                    view! {
                        <sl-button
                            class=if route_mode == UiRouteMode::Build { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                            size="small"
                            href=build_href.clone()
                            title="构建"
                            aria-label="构建"
                            data-mei-view="build"
                        >
                            <span class="mode-label">"构建"</span>
                        </sl-button>
                    }.into_any()
                } else {
                    view! { <></> }.into_any()
                }}
            </sl-button-group>
        </div>
    }
    .into_any()
    };
    let launch_title = if stage_enabled {
        "在新标签页打开"
    } else {
        "在新标签页打开无 Chrome 应用"
    };
    let account_view = if auth_enabled {
        if let Some(account) = auth_account.filter(|item| item.logged_in) {
            let display = if account.profile.trim().is_empty() {
                account.username.clone()
            } else {
                account.profile.clone()
            };
            let role = if account.role.trim().is_empty() {
                "guest".to_string()
            } else {
                account.role.clone()
            };
            view! {
                <div class="topbar-account inline-flex items-center gap-1.5 border-l border-slate-400/20 pl-2">
                    <span class="topbar-account-name text-[11px] text-slate-300" title=account.username.clone()>
                        {format!("{display} ({role})")}
                    </span>
                    <a class="topbar-account-link text-[11px] text-slate-300 hover:text-slate-100" href="/account/password">
                        "改密"
                    </a>
                    <a class="topbar-account-link text-[11px] text-slate-300 hover:text-slate-100" href="/logout?next=%2Flogin">
                        "退出"
                    </a>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div class="topbar-account inline-flex items-center gap-1.5 border-l border-slate-400/20 pl-2">
                    <a class="topbar-account-link text-[11px] text-slate-300 hover:text-slate-100" href="/login">
                        "登录"
                    </a>
                </div>
            }
            .into_any()
        }
    } else {
        view! { <></> }.into_any()
    };
    view! {
        <header class="topbar topbar-shell chrome-inset chrome-safe-x topbar-safe sticky top-0 z-50 grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2.5 py-1.5 backdrop-blur-md">
            <div class="brand flex min-w-0 items-center gap-2">
                <div class="brand-title-row flex min-w-0 items-center gap-2">
                    <img
                        class="brand-mark block h-[18px] w-[18px] shrink-0"
                        src="/app-assets/favicon.svg"
                        width="18"
                        height="18"
                        alt=""
                        aria-hidden="true"
                    />
                    <strong class="text-[13px] font-semibold text-slate-100">"MeiLang"</strong>
                </div>
            </div>
            <nav class="app-tabs flex min-w-0 items-center gap-2.5">
                <div class="app-tabs-groups flex min-w-0 flex-1 flex-nowrap items-center gap-1.5 overflow-x-auto pr-1">{app_tabs}</div>
                {active_item_breadcrumb}
            </nav>
            <div class="topbar-actions flex shrink-0 flex-nowrap items-center justify-end gap-1">
                {mode_tabs}
                <sl-tooltip content=launch_title placement="bottom">
                    <sl-button
                        class="topbar-launch-btn"
                        size="small"
                        href=presentation_href
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label=launch_title
                    >
                        <span class="mode-icon" aria-hidden="true">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M14 3h7v7"/>
                                <path d="M10 14L21 3"/>
                                <path d="M21 14v4a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3h4"/>
                            </svg>
                        </span>
                    </sl-button>
                </sl-tooltip>
                {account_view}
            </div>
        </header>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::{preferred_access_scene, UiRouteMode};
    use mei_lang_kernel::CompiledSceneRoute;

    fn route(
        scene_id: &str,
        target_file: &str,
        is_default: bool,
        access_export: bool,
    ) -> CompiledSceneRoute {
        CompiledSceneRoute {
            scene_id: scene_id.to_string(),
            frame_id: None,
            target_file: target_file.to_string(),
            kind: "scene".to_string(),
            title: None,
            is_default,
            access_export,
        }
    }

    #[test]
    fn preferred_access_scene_falls_back_to_default_exported_scene() {
        let routes = vec![
            route("private", "scenes/private.mei", false, false),
            route("home", "scenes/home.mei", true, true),
        ];
        assert_eq!(
            preferred_access_scene(
                UiRouteMode::Config,
                &routes,
                None,
                None,
                None,
                "main.mei",
            ),
            Some("home")
        );
    }

    #[test]
    fn preferred_access_scene_prefers_build_preview_target_scene() {
        let routes = vec![
            route("home", "scenes/home.mei", true, true),
            route("detail", "scenes/detail.mei", false, true),
        ];
        assert_eq!(
            preferred_access_scene(
                UiRouteMode::Build,
                &routes,
                None,
                Some("scenes/detail.mei"),
                Some("home"),
                "main.mei",
            ),
            Some("detail")
        );
    }
}
