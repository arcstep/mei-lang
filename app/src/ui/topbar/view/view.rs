use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;
use std::collections::BTreeMap;

use crate::ui::manage_routing::access_scene_query;
use crate::ui::route::UiRouteMode;
use crate::ui::view_routing::{
    app_scene_href, build_href_with_catalog, config_href, cross_app_href, presentation_scene_href,
    runtime_href, upload_href,
};
use crate::ui::{HostAccountView, TopbarMenuContext};

use crate::ui::topbar::menu_groups::build_topbar_menu_groups;
use super::scene_routing::*;

pub(crate) fn topbar_view(
    apps: &[WorkspaceAppMeta],
    active_app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
    access_scene_for_href: Option<&str>,
    build_file: Option<&str>,
    active_tab: Option<&str>,
    active_catalog: Option<&str>,
    active_stock_pack: Option<&str>,
    upload_enabled: bool,
    stage_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let access_entry_query = access_scene_query(access_scene_for_href);
    let access_disabled = access_entry_query.is_empty();
    let menu_groups = build_topbar_menu_groups(apps, topbar_menu, route_mode);
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
    let in_stock_catalog = topbar_menu
        .and_then(|menu| menu.stock_catalog_app_id.as_deref())
        .is_some_and(|catalog_id| catalog_id.trim() == active_app_path.trim());
    let breadcrumb_root_label = if in_stock_catalog {
        topbar_menu
            .and_then(|menu| menu.stock_catalog_app_title.as_deref())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("组件库")
    } else {
        workspace_label
    };
    let breadcrumb_kind = if in_stock_catalog {
        "当前组件库"
    } else {
        "当前应用"
    };
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
                let href = cross_app_href(route_mode, &item.app_id, item.catalog.as_deref(), item.pack.as_deref());
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
                    let class = if menu_item_is_active(item, active_app_path, active_catalog, active_stock_pack) {
                        "app-tab app-tab-sub active"
                    } else {
                        "app-tab app-tab-sub"
                    };
                    let href = cross_app_href(route_mode, &item.app_id, item.catalog.as_deref(), item.pack.as_deref());
                    view! { <a class=class href=href>{item.label.clone()}</a> }
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
                            let href = cross_app_href(route_mode, &item.app_id, item.catalog.as_deref(), item.pack.as_deref());
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
    let breadcrumb_aria = format!("{breadcrumb_kind}：{breadcrumb_root_label} / {active_app_label}");
    let active_item_breadcrumb = view! {
        <div class="app-current-path inline-flex min-w-0 max-w-[min(300px,30vw)] items-center gap-1 pl-2 mei-font-1 mei-text-muted" aria-label=breadcrumb_aria>
            <span class="app-current-path-prefix shrink-0 mei-text-muted">{if in_stock_catalog { "库：" } else { "应用：" }}</span>
            <span class="app-current-path-trail inline-flex min-w-0 items-center gap-1 whitespace-nowrap">
                <span class="app-current-path-workspace shrink-0 mei-text-muted">{breadcrumb_root_label}</span>
                <span class="app-current-path-separator shrink-0 mei-text-muted/70" aria-hidden="true">"/"</span>
                <span class="app-current-path-item min-w-0 overflow-hidden text-ellipsis mei-text-primary">{active_app_label}</span>
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
    let build_href = build_href_with_catalog(
        active_app_path,
        Some(build_file),
        active_tab,
        active_catalog,
        active_stock_pack,
    );
    let runtime_href = runtime_href(active_app_path, None, None);
    let config_href = append_scene_query(config_href(active_app_path), access_scene_for_href);
    let upload_href = append_scene_query(upload_href(active_app_path, None), access_scene_for_href);
    let presentation_href = if access_disabled {
        "#".to_string()
    } else {
        presentation_scene_href(active_app_path, access_scene_for_href)
    };
    let (show_config_tab, show_upload_tab, show_build_tab) =
        auth_surface_tabs_visible(auth_enabled, auth_account);
    let show_runtime_tab = show_build_tab;
    let show_upload_mode = upload_enabled && show_upload_tab;
    let visible_mode_tab_count = 1usize
        + usize::from(show_upload_mode)
        + usize::from(show_config_tab)
        + usize::from(show_build_tab)
        + usize::from(show_runtime_tab);
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
                {if show_runtime_tab {
                    view! {
                        <sl-button
                            class=if route_mode == UiRouteMode::Runtime { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                            size="small"
                            href=runtime_href.clone()
                            title="运行"
                            aria-label="运行"
                            data-mei-view="runtime"
                        >
                            <span class="mode-label">"运行"</span>
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
        "在新标签页进入演示模式"
    } else {
        "在新标签页进入 scene 演示模式"
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
                    <strong class="topbar-brand-title mei-font-2 mei-text-inverse">"MeiLang"</strong>
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
                        disabled=access_disabled
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

fn menu_item_is_active(
    item: &crate::ui::topbar::menu_groups::TopbarMenuItem,
    active_app_path: &str,
    active_catalog: Option<&str>,
    active_stock_pack: Option<&str>,
) -> bool {
    if item.app_id.as_str() != active_app_path {
        return false;
    }
    let cat = active_catalog.map(str::trim).filter(|value| !value.is_empty());
    let pack = active_stock_pack.map(str::trim).filter(|value| !value.is_empty());
    match (item.catalog.as_deref(), item.pack.as_deref()) {
        (None, None) => cat.is_none() && pack.is_none(),
        (Some(item_cat), None) => cat.unwrap_or("components") == item_cat && pack.is_none(),
        (Some(item_cat), Some(item_pack)) => {
            cat.unwrap_or("components") == item_cat && pack == Some(item_pack)
        }
        (None, Some(_)) => false,
    }
}
