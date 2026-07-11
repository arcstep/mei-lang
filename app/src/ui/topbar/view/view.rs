use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;
use std::collections::BTreeMap;

use crate::ui::manage_routing::access_scene_query;
use crate::ui::route::UiRouteMode;
use crate::ui::view_routing::{
    app_access_href, app_scene_href, cross_app_href, home_href, host_config_href,
    host_runtime_href, host_upload_href, layout_href, mcg_href, prototype_href,
};
use crate::ui::{HostAccountView, TopbarMenuContext};

use super::scene_routing::*;
use crate::ui::topbar::menu_groups::build_topbar_menu_groups;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellNavActive {
    Home,
    Config,
    Upload,
    Runtime,
    Mcg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppViewTab {
    App,
    Layout,
    Prototype,
}

fn resolve_active_app_view_tab(route_mode: UiRouteMode) -> Option<AppViewTab> {
    match route_mode {
        UiRouteMode::App => Some(AppViewTab::App),
        UiRouteMode::Layout => Some(AppViewTab::Layout),
        UiRouteMode::Prototype => Some(AppViewTab::Prototype),
        _ => None,
    }
}

/// 应用入口菜单的链接目标：工作区页无当前应用时用 App，有应用时用当前面 route。
fn app_menu_link_mode(route_mode: UiRouteMode, active_app_path: &str) -> UiRouteMode {
    if active_app_path.trim().is_empty() {
        UiRouteMode::App
    } else {
        route_mode
    }
}

pub(crate) fn topbar_view(
    apps: &[WorkspaceAppMeta],
    active_app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
    access_scene_for_href: Option<&str>,
    _build_file: Option<&str>,
    active_tab: Option<&str>,
    active_catalog: Option<&str>,
    active_stock_pack: Option<&str>,
    _upload_enabled: bool,
    _stage_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    _data_mode: Option<&str>,
    _review_projection: Option<&str>,
    _build_tree_mode: Option<&str>,
    shell_nav_active: Option<ShellNavActive>,
) -> AnyView {
    let has_app_context = !active_app_path.trim().is_empty();
    let menu_link_mode = app_menu_link_mode(route_mode, active_app_path);
    let access_entry_query = access_scene_query(access_scene_for_href);
    let access_disabled = access_entry_query.is_empty();
    let menu_groups = build_topbar_menu_groups(apps, topbar_menu, menu_link_mode);
    let active_app_label = menu_groups
        .iter()
        .flat_map(|group| group.items.iter())
        .find(|item| menu_item_is_active(item, active_app_path, active_catalog, active_stock_pack))
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
    let breadcrumb_root_label = workspace_label;
    let app_tabs = menu_groups
        .into_iter()
        .map(|group| {
            let group_id = group.id.clone();
            let group_label = group.label.clone();
            let group_has_active = group
                .items
                .iter()
                .any(|item| menu_item_is_active(item, active_app_path, active_catalog, active_stock_pack));
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
            let is_stock_pack_group = group_id == "components" || group_id == "templates";
            let is_single_top_level_tab = !is_stock_pack_group
                && direct_items.len() == 1
                && subgroup_items.is_empty();
            if is_single_top_level_tab {
                let item = &direct_items[0];
                let class = if menu_item_is_active(item, active_app_path, active_catalog, active_stock_pack) {
                    "app-tab active"
                } else {
                    "app-tab"
                };
                let href = cross_app_href(
                    menu_link_mode,
                    &item.app_id,
                    item.catalog.as_deref(),
                    item.pack.as_deref(),
                );
                return view! {
                    <a class=class href=href data-app-id=item.app_id.clone() data-topbar-menu-group=group_id.clone()>
                        {item.label.clone()}
                    </a>
                }
                .into_any();
            }
            let direct_links = direct_items
                .iter()
                .map(|item| {
                    let class = if menu_item_is_active(item, active_app_path, active_catalog, active_stock_pack) {
                        "app-tab app-tab-sub active"
                    } else {
                        "app-tab app-tab-sub"
                    };
                    let href = cross_app_href(
                        menu_link_mode,
                        &item.app_id,
                        item.catalog.as_deref(),
                        item.pack.as_deref(),
                    );
                    view! { <a class=class href=href data-app-id=item.app_id.clone()>{item.label.clone()}</a> }
                })
                .collect_view();
            let subgroup_blocks = subgroup_items
                .into_iter()
                .map(|(subgroup, items)| {
                    let links = items
                        .iter()
                        .map(|item| {
                            let class = if menu_item_is_active(item, active_app_path, active_catalog, active_stock_pack) {
                                "app-tab app-tab-sub active"
                            } else {
                                "app-tab app-tab-sub"
                            };
                            let href = cross_app_href(
                                menu_link_mode,
                                &item.app_id,
                                item.catalog.as_deref(),
                                item.pack.as_deref(),
                            );
                            view! { <a class=class href=href data-app-id=item.app_id.clone()>{item.label.clone()}</a> }
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
    let breadcrumb_aria = format!("当前应用：{breadcrumb_root_label} / {active_app_label}");
    let active_item_breadcrumb = if !has_app_context {
        view! { <></> }.into_any()
    } else {
        view! {
        <div class="app-current-path inline-flex min-w-0 max-w-[min(300px,30vw)] items-center gap-1 mei-font-1 mei-text-muted" aria-label=breadcrumb_aria>
            <span class="app-current-path-prefix shrink-0 mei-text-muted">{"应用："}</span>
            <span class="app-current-path-trail inline-flex min-w-0 items-center gap-1 whitespace-nowrap">
                <span class="app-current-path-workspace shrink-0 mei-text-muted">{breadcrumb_root_label}</span>
                <span class="app-current-path-separator shrink-0 mei-text-muted/70" aria-hidden="true">"/"</span>
                <span class="app-current-path-item min-w-0 overflow-hidden text-ellipsis mei-text-primary">{active_app_label}</span>
            </span>
        </div>
    }
    .into_any()
    };
    let app_href = if access_disabled || !has_app_context {
        "#".to_string()
    } else {
        app_access_href(active_app_path)
    };
    let standalone_app_href = if access_disabled || !has_app_context {
        "#".to_string()
    } else {
        app_scene_href(
            active_app_path,
            access_scene_for_href,
            active_tab,
            Some("none"),
            None,
            None,
        )
    };
    let (_show_config_tab, show_build_views, _show_data_tab) =
        auth_surface_tabs_visible(auth_enabled, auth_account);
    let active_app_view = resolve_active_app_view_tab(route_mode);
    let app_view_tabs = if !has_app_context {
        view! { <></> }.into_any()
    } else if !show_build_views {
        view! { <></> }.into_any()
    } else {
        let surface_tab = |tab: AppViewTab, label: &'static str, href: String| {
            let class = if active_app_view == Some(tab) {
                "mode-tab-btn is-active"
            } else {
                "mode-tab-btn"
            };
            view! {
                <sl-button
                    class=class
                    size="small"
                    href=href
                    title=label
                    aria-label=label
                    data-mei-app-view=label
                >
                    <span class="mode-label">{label}</span>
                </sl-button>
            }
        };
        view! {
            <div class="mode-tabs inline-flex shrink-0 items-center">
                <sl-button-group class="mode-tab-group" label="应用视图" data-mei-app-view-tabs="1">
                    {surface_tab(AppViewTab::App, "应用", app_href.clone())}
                    {surface_tab(
                        AppViewTab::Layout,
                        "布局",
                        layout_href(active_app_path, None, None),
                    )}
                    {surface_tab(
                        AppViewTab::Prototype,
                        "原型",
                        prototype_href(active_app_path, None, None),
                    )}
                </sl-button-group>
            </div>
        }
        .into_any()
    };
    let launch_title = if access_disabled {
        "当前没有可独立打开的 scene route".to_string()
    } else {
        "独立打开（新标签页，无 shell）".to_string()
    };
    let launch_aria_label = launch_title.clone();
    let standalone_launch = if !has_app_context {
        view! { <></> }.into_any()
    } else {
        view! {
            <sl-tooltip content=launch_title placement="bottom">
                <sl-button
                    class="topbar-launch-btn"
                    size="small"
                    href=standalone_app_href
                    disabled=access_disabled
                    target="_blank"
                    rel="noopener noreferrer"
                    aria-label=launch_aria_label
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
        }
        .into_any()
    };
    let system_toolbar = shell_nav_view(shell_nav_active);
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
                <div class="topbar-account inline-flex items-center gap-1.5 pl-2">
                    <span class="topbar-account-name mei-font-1 mei-text-body" title=account.username.clone()>
                        {format!("{display} ({role})")}
                    </span>
                    <a class="topbar-account-link mei-font-1 mei-text-body" href="/account/password">
                        "改密"
                    </a>
                    <a class="topbar-account-link mei-font-1 mei-text-body" href="/logout?next=%2Flogin">
                        "退出"
                    </a>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div class="topbar-account inline-flex items-center gap-1.5 pl-2">
                    <a class="topbar-account-link mei-font-1 mei-text-body" href="/login">
                        "登录"
                    </a>
                </div>
            }
            .into_any()
        }
    } else {
        view! { <></> }.into_any()
    };
    let app_context_class = if has_app_context {
        "topbar-app-context has-app-context"
    } else {
        "topbar-app-context"
    };
    view! {
        <header class="topbar topbar-shell chrome-inset chrome-safe-x topbar-safe sticky top-0 z-50 flex items-center gap-2 py-1.5 backdrop-blur-md">
            <div class="brand flex shrink-0 items-center gap-2">
                <div class="brand-title-row flex min-w-0 items-center gap-2">
                    <img
                        class="brand-mark block h-[18px] w-[18px] shrink-0"
                        src="/app-assets/favicon.svg"
                        width="18"
                        height="18"
                        alt=""
                        aria-hidden="true"
                    />
                    <strong class="topbar-brand-title mei-font-2 mei-text-inverse">"MeiLang"</strong>
                </div>
            </div>
            <div class=app_context_class>
                <div
                    class="topbar-app-toolbar flex min-w-0 items-center gap-1 overflow-x-auto"
                    aria-label="应用工具栏"
                >
                    <div class="app-tabs-groups flex min-w-0 shrink-0 flex-nowrap items-center gap-1">{app_tabs}</div>
                    {active_item_breadcrumb}
                    {app_view_tabs}
                    {standalone_launch}
                </div>
            </div>
            <div class="topbar-spacer" aria-hidden="true"></div>
            <div class="topbar-right flex shrink-0 items-center gap-2">
                <div class="topbar-system-toolbar shrink-0" aria-label="系统工具栏">
                    {system_toolbar}
                </div>
                <div class="topbar-actions flex shrink-0 flex-nowrap items-center justify-end gap-1">
                    {account_view}
                </div>
            </div>
        </header>
    }
    .into_any()
}

fn shell_nav_view(active: Option<ShellNavActive>) -> AnyView {
    let nav_class = |item: ShellNavActive| {
        if active == Some(item) {
            "shell-nav-link is-active"
        } else {
            "shell-nav-link"
        }
    };
    view! {
        <div class="shell-nav inline-flex shrink-0 items-center gap-1" aria-label="系统">
            <a class=nav_class(ShellNavActive::Home) href=home_href()>"首页"</a>
            <a class=nav_class(ShellNavActive::Config) href=host_config_href(None)>"配置"</a>
            <a class=nav_class(ShellNavActive::Runtime) href=host_runtime_href(None, None, None)>"运行"</a>
            <a class=nav_class(ShellNavActive::Upload) href=host_upload_href(None, None)>"上传"</a>
            <a class=nav_class(ShellNavActive::Mcg) href=mcg_href(None)>"MCG"</a>
        </div>
    }
    .into_any()
}

fn menu_item_is_active(
    item: &crate::ui::topbar::menu_groups::TopbarMenuItem,
    active_app_path: &str,
    active_catalog: Option<&str>,
    active_stock_pack: Option<&str>,
) -> bool {
    if item.app_id.as_str() != active_app_path {
        return false;
    }
    let cat = active_catalog
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let pack = active_stock_pack
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (item.catalog.as_deref(), item.pack.as_deref()) {
        (None, None) => cat.is_none() && pack.is_none(),
        (Some(item_cat), None) => cat.unwrap_or("components") == item_cat && pack.is_none(),
        (Some(item_cat), Some(item_pack)) => {
            cat.unwrap_or("components") == item_cat && pack == Some(item_pack)
        }
        (None, Some(_)) => false,
    }
}
