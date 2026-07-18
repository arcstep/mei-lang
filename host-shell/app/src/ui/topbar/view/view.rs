use leptos::prelude::*;
use mei_lang_kernel::{
    is_stage_registry_candidate, resolve_default_scene_from_root, CompiledSceneRoute,
    WorkspaceAppMeta,
};
use std::path::Path;

use crate::ui::manage_routing::access_scene_query;
use crate::ui::route::UiRouteMode;
use crate::ui::view_routing::{
    app_scene_href, cross_app_href, home_href, host_runtime_href,
};
use crate::ui::{HostAccountView, TopbarMenuContext};

use crate::ui::topbar::menu_groups::build_topbar_menu_groups;
use crate::ui::topbar::menus::{DEFAULT_BRAND_LOGO_HREF, DEFAULT_BRAND_TITLE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellNavActive {
    Home,
    Config,
    Upload,
    Runtime,
    Mcg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum AppViewTab {
    App,
    Layout,
    Prototype,
}

#[allow(dead_code)]
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

fn show_app_center(auth_enabled: bool, auth_account: Option<&HostAccountView>) -> bool {
    if !auth_enabled {
        return true;
    }
    let Some(account) = auth_account.filter(|item| item.logged_in) else {
        return false;
    };
    // 应用中心：admin + super（config_upload）；与 /runtime 路由鉴权对齐
    account.capabilities.config_upload
}

fn show_app_admin(
    has_app_context: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> bool {
    if !has_app_context {
        return false;
    }
    if !auth_enabled {
        return true;
    }
    auth_account
        .filter(|item| item.logged_in)
        .is_some_and(|item| item.capabilities.config_upload)
}

fn account_role_label(role: &str) -> &'static str {
    match role.trim().to_ascii_lowercase().as_str() {
        "super" => "超级管理员",
        "admin" => "管理员",
        "guest" => "访客",
        _ => "用户",
    }
}

fn account_avatar_view() -> AnyView {
    // Default person glyph; future: replace with custom avatar img under same mount.
    view! {
        <span class="topbar-account-avatar" data-mei-account-avatar="default" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="8" r="3.25"/>
                <path d="M5.5 19.25c1.35-3.1 3.55-4.75 6.5-4.75s5.15 1.65 6.5 4.75"/>
            </svg>
        </span>
    }
    .into_any()
}

fn account_menu_view(auth_enabled: bool, auth_account: Option<&HostAccountView>) -> AnyView {
    if !auth_enabled {
        return view! { <></> }.into_any();
    }
    if let Some(account) = auth_account.filter(|item| item.logged_in) {
        let login_name = if !account.username.trim().is_empty() {
            account.username.clone()
        } else if !account.profile.trim().is_empty() {
            account.profile.clone()
        } else {
            "已登录".to_string()
        };
        let role_slug = if account.role.trim().is_empty() {
            "guest"
        } else {
            account.role.as_str()
        };
        let role_label = account_role_label(role_slug).to_string();
        let username_title = account.username.clone();
        view! {
            <div class="topbar-account">
                <details class="app-group-dropdown topbar-account-dropdown">
                    <summary
                        class="topbar-account-trigger"
                        title=username_title.clone()
                        aria-label=format!("账户菜单：{login_name}")
                    >
                        {account_avatar_view()}
                        <span class="topbar-account-label mei-font-1">{login_name.clone()}</span>
                    </summary>
                    <div
                        class="app-group-menu topbar-account-panel"
                        role="menu"
                        aria-label="账户"
                    >
                        <div class="topbar-account-meta">
                            <p class="topbar-account-meta-name mei-font-2 mei-text-primary">{login_name}</p>
                            <p class="topbar-account-meta-role mei-font-1 mei-text-muted">{role_label}</p>
                        </div>
                        <div class="topbar-account-actions">
                            <a
                                class="topbar-account-action"
                                role="menuitem"
                                href="/account/password"
                            >
                                "改密"
                            </a>
                            <a
                                class="topbar-account-action topbar-account-action--danger"
                                role="menuitem"
                                href="/logout?next=%2Flogin"
                            >
                                "退出"
                            </a>
                        </div>
                    </div>
                </details>
            </div>
        }
        .into_any()
    } else {
        view! {
            <div class="topbar-account">
                <a class="topbar-account-login" href="/login" aria-label="登录">
                    {account_avatar_view()}
                    <span class="topbar-account-label mei-font-1">"登录"</span>
                </a>
            </div>
        }
        .into_any()
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
    access_stage_routes: Option<&[CompiledSceneRoute]>,
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
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| {
            if has_app_context {
                active_app_path.to_string()
            } else {
                "选择应用".to_string()
            }
        });

    let brand_title = topbar_menu
        .and_then(|menu| menu.brand_title.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BRAND_TITLE)
        .to_string();
    let brand_logo_href = topbar_menu
        .and_then(|menu| menu.brand_logo_href.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BRAND_LOGO_HREF)
        .to_string();
    let brand_aria = format!("返回首页 · {brand_title}");
    let brand_title_view = brand_title.clone();
    let brand_logo_view = brand_logo_href.clone();

    let app_switcher = app_switcher_view(
        apps,
        active_app_path,
        active_app_label.as_str(),
        menu_link_mode,
        active_catalog,
        active_stock_pack,
    );
    // 0543 Registry not wired yet: empty strip does not render separator or chips.
    let admin_items: &[AdminStripItem] = &[];
    let app_admin = if show_app_admin(has_app_context, auth_enabled, auth_account) {
        app_admin_strip_view(admin_items, None)
    } else {
        view! { <></> }.into_any()
    };
    let app_view_tabs = if !has_app_context {
        view! { <></> }.into_any()
    } else {
        stage_strip_view(
            active_app_path,
            access_scene_for_href,
            access_stage_routes,
            active_tab,
        )
    };
    let launch_title = if access_disabled {
        "当前没有可独立打开的 Stage".to_string()
    } else {
        "独立打开当前 Stage".to_string()
    };
    let launch_aria_label = launch_title.clone();
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
    let system_toolbar = shell_nav_view(
        shell_nav_active,
        auth_enabled,
        auth_account,
    );
    let account_view = account_menu_view(auth_enabled, auth_account);
    let app_context_class = if has_app_context {
        "topbar-app-context has-app-context"
    } else {
        "topbar-app-context"
    };
    let brand_home = home_href();
    view! {
        <header class="topbar topbar-shell chrome-inset chrome-safe-x topbar-safe sticky top-0 z-50 flex items-center gap-2 py-1.5 backdrop-blur-md">
            <a
                class="brand flex shrink-0 items-center gap-2"
                href=brand_home
                aria-label=brand_aria
            >
                <div class="brand-title-row flex min-w-0 items-center gap-2">
                    <img
                        class="brand-mark block h-[18px] w-[18px] shrink-0"
                        src=brand_logo_view
                        width="18"
                        height="18"
                        alt=""
                        aria-hidden="true"
                    />
                    <strong class="topbar-brand-title mei-font-2 mei-text-inverse">{brand_title_view}</strong>
                </div>
            </a>
            <div class=app_context_class>
                <div
                    class="topbar-app-toolbar flex min-w-0 items-center gap-1 overflow-x-auto"
                    aria-label="应用工具栏"
                >
                    {app_switcher}
                    {app_view_tabs}
                    {app_admin}
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

fn app_switcher_view(
    apps: &[WorkspaceAppMeta],
    active_app_path: &str,
    active_app_label: &str,
    menu_link_mode: UiRouteMode,
    active_catalog: Option<&str>,
    active_stock_pack: Option<&str>,
) -> AnyView {
    let trigger_label = active_app_label.to_string();
    let trigger_class = if !active_app_path.trim().is_empty() {
        "app-group-trigger is-active"
    } else {
        "app-group-trigger"
    };
    let items = apps
        .iter()
        .map(|app| {
            let stage = default_stage_for_app(apps, app.id.as_str());
            let href = cross_app_href(
                menu_link_mode,
                app.id.as_str(),
                active_catalog,
                active_stock_pack,
                Some(stage.as_str()),
            );
            let item_class = if app.id.as_str() == active_app_path {
                "app-menu-item is-active"
            } else {
                "app-menu-item"
            };
            let label = app.title.clone();
            let app_id = app.id.clone();
            view! {
                <a
                    class=item_class
                    href=href
                    data-app-id=app_id.clone()
                    data-default-stage=stage
                    data-mei-app-switcher-item="1"
                >
                    <span class="app-menu-item-label">{label}</span>
                </a>
            }
        })
        .collect_view();
    let empty = if apps.is_empty() {
        view! {
            <p class="app-menu-empty mei-font-1 mei-text-muted px-2 py-1">
                "暂无已启动应用"
            </p>
        }
        .into_any()
    } else {
        view! { <></> }.into_any()
    };
    view! {
        <div class="app-switcher inline-flex shrink-0 items-center" data-mei-app-switcher="1">
            <details class="app-group-dropdown">
                <summary class=trigger_class>
                    <span class="mode-label">{trigger_label}</span>
                </summary>
                <div class="app-group-menu" role="menu" aria-label="应用">
                    {empty}
                    {items}
                </div>
            </details>
        </div>
    }
    .into_any()
}

/// Future 0543 admin strip entry. Empty slice → strip not rendered.
#[derive(Debug, Clone)]
struct AdminStripItem {
    id: String,
    label: String,
    href: String,
}

fn app_admin_strip_view(items: &[AdminStripItem], active_id: Option<&str>) -> AnyView {
    if items.is_empty() {
        return view! { <></> }.into_any();
    }
    let active = active_id.map(str::trim).unwrap_or("");
    let chips = items
        .iter()
        .map(|item| {
            let class = if !active.is_empty() && item.id.as_str() == active {
                "topbar-chip is-active"
            } else {
                "topbar-chip"
            };
            let href = item.href.clone();
            let label = item.label.clone();
            let id = item.id.clone();
            view! {
                <a
                    class=class
                    href=href
                    data-mei-admin-item=id
                >
                    <span class="mode-label">{label}</span>
                </a>
            }
        })
        .collect_view();
    view! {
        <div class="topbar-admin-cluster inline-flex min-w-0 shrink items-center gap-1">
            <span class="topbar-context-sep" aria-hidden="true">"|"</span>
            <nav
                class="admin-strip topbar-chip-strip inline-flex min-w-0 items-center gap-1"
                data-mei-admin-strip="1"
                aria-label="应用管理"
            >
                {chips}
            </nav>
        </div>
    }
    .into_any()
}

fn stage_route_label(route: &CompiledSceneRoute) -> String {
    route
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| route.scene_id.clone())
}

fn is_presentation_stage_route(route: &CompiledSceneRoute) -> bool {
    let kind = route.kind.trim().to_ascii_lowercase();
    if kind == "presentation" {
        return true;
    }
    let target = route.target_file.replace('\\', "/").to_ascii_lowercase();
    target.contains("/presentation/") || target.starts_with("presentation/")
}

fn is_top_level_stage_route(route: &CompiledSceneRoute) -> bool {
    // Phase 1: align Access topbar with StageRegistry candidate rules.
    is_stage_registry_candidate(route)
}

fn stage_strip_view(
    active_app_path: &str,
    current_scene: Option<&str>,
    access_stage_routes: Option<&[CompiledSceneRoute]>,
    active_tab: Option<&str>,
) -> AnyView {
    let Some(routes) = access_stage_routes.filter(|entries| !entries.is_empty()) else {
        return view! { <></> }.into_any();
    };
    let stages: Vec<&CompiledSceneRoute> = routes
        .iter()
        .filter(|route| is_top_level_stage_route(route))
        .collect();
    if stages.is_empty() {
        return view! { <></> }.into_any();
    }
    let current = current_scene.map(str::trim).unwrap_or("");
    let current_route = stages
        .iter()
        .copied()
        .find(|route| route.scene_id == current)
        .or_else(|| stages.iter().copied().find(|route| route.is_default))
        .unwrap_or(stages[0]);
    let chips = stages
        .iter()
        .map(|route| {
            let scene_id = route.scene_id.clone();
            let item_label = stage_route_label(route);
            let surface = if is_presentation_stage_route(route) {
                "paged"
            } else {
                "viewport"
            };
            let profile = if is_presentation_stage_route(route) {
                "slides"
            } else {
                "cockpit"
            };
            let href = app_scene_href(
                active_app_path,
                Some(scene_id.as_str()),
                active_tab,
                None,
                None,
                None,
            );
            let item_class = if scene_id == current_route.scene_id {
                "topbar-chip is-active"
            } else {
                "topbar-chip"
            };
            view! {
                <a
                    class=item_class
                    href=href
                    data-mei-spa-nav="1"
                    data-mei-stage-scene=scene_id.clone()
                    data-mei-stage-kind=if is_presentation_stage_route(route) { "presentation" } else { "scene" }
                    data-mei-stage-profile=profile
                    data-mei-stage-surface=surface
                >
                    <span class="mode-label">{item_label}</span>
                </a>
            }
        })
        .collect_view();
    view! {
        <nav
            class="stage-strip topbar-chip-strip mode-tabs stage-switcher inline-flex min-w-0 items-center gap-1"
            data-mei-stage-strip="1"
            data-mei-stage-switcher="1"
            aria-label="舞台"
        >
            {chips}
        </nav>
    }
    .into_any()
}

fn shell_nav_view(
    active: Option<ShellNavActive>,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    if !show_app_center(auth_enabled, auth_account) {
        return view! { <></> }.into_any();
    }
    let class = if active == Some(ShellNavActive::Runtime) {
        "shell-nav-link is-active"
    } else {
        "shell-nav-link"
    };
    view! {
        <div class="shell-nav inline-flex shrink-0 items-center gap-1" aria-label="系统">
            <a class=class href=host_runtime_href(None, None, None)>"应用中心"</a>
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

fn default_stage_for_app(apps: &[WorkspaceAppMeta], app_id: &str) -> String {
    apps.iter()
        .find(|app| app.id.as_str() == app_id)
        .and_then(|app| {
            let root = Path::new(app.root.as_str());
            if !root.is_dir() {
                return None;
            }
            resolve_default_scene_from_root(root)
                .ok()
                .flatten()
                .map(|stage| stage.trim().to_string())
                .filter(|stage| !stage.is_empty())
        })
        .unwrap_or_else(|| "home".to_string())
}
