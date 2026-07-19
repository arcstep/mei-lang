use leptos::prelude::*;
use mei_lang_kernel::{
    is_stage_registry_candidate, resolve_default_scene_from_root, CompiledSceneRoute, StageProfile,
    WorkspaceAppMeta,
};
use std::collections::BTreeMap;
use std::path::Path;

use crate::ui::manage_routing::access_scene_query;
use crate::ui::route::UiRouteMode;
use crate::ui::view_routing::{app_scene_href, cross_app_href, home_href, host_runtime_href};
use crate::ui::{HostAccountView, TopbarMenuContext};

use crate::ui::topbar::menus::{DEFAULT_BRAND_LOGO_HREF, DEFAULT_BRAND_TITLE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellNavActive {
    Home,
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
        UiRouteMode::Run
        | UiRouteMode::Copilot
        | UiRouteMode::Config
        | UiRouteMode::Upload
        | UiRouteMode::Runtime
        | UiRouteMode::Admin => None,
    }
}

/// 应用入口菜单的链接目标：工作区页无当前应用时用 App；
/// Admin / Config / Upload 面仍切到 Access Stage（0544：App Switcher ≠ 管理面）。
fn app_menu_link_mode(route_mode: UiRouteMode, active_app_path: &str) -> UiRouteMode {
    if active_app_path.trim().is_empty() {
        return UiRouteMode::App;
    }
    match route_mode {
        UiRouteMode::Admin | UiRouteMode::Config | UiRouteMode::Upload => UiRouteMode::App,
        other => other,
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
    admin_nav_items: &[AdminNavItem],
    admin_active_id: Option<&str>,
) -> AnyView {
    let has_app_context = !active_app_path.trim().is_empty();
    let menu_link_mode = app_menu_link_mode(route_mode, active_app_path);
    let access_entry_query = access_scene_query(access_scene_for_href);
    let access_disabled = access_entry_query.is_empty();
    let active_app_label = apps
        .iter()
        .find(|app| app.id.as_str() == active_app_path)
        .map(app_menu_label)
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
    let visible_admin_items = if show_app_admin(has_app_context, auth_enabled, auth_account) {
        admin_nav_items
    } else {
        &[]
    };
    let stage_items = if has_app_context {
        build_stage_nav_items(
            active_app_path,
            access_scene_for_href,
            access_stage_routes,
            active_tab,
            matches!(
                route_mode,
                UiRouteMode::App | UiRouteMode::Layout | UiRouteMode::Prototype | UiRouteMode::Run
            ),
        )
    } else {
        Vec::new()
    };
    let app_view_tabs = stage_strip_view(stage_items.as_slice());
    // Registry projection → chips; empty slice does not render separator.
    let app_admin = app_admin_strip_view(
        visible_admin_items,
        admin_active_id,
        !stage_items.is_empty(),
    );
    let app_more =
        app_navigation_more_view(stage_items.as_slice(), visible_admin_items, admin_active_id);
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
    let system_toolbar = shell_nav_view(shell_nav_active, auth_enabled, auth_account);
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
                    class="topbar-app-toolbar flex min-w-0 items-center gap-1"
                    aria-label="应用工具栏"
                >
                    {app_switcher}
                    {app_view_tabs}
                    {app_admin}
                    {app_more}
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
            let label = app_menu_label(app);
            let full_title = app.title.clone();
            let app_id = app.id.clone();
            view! {
                <a
                    class=item_class
                    href=href
                    data-app-id=app_id.clone()
                    data-default-stage=stage
                    data-mei-app-switcher-item="1"
                    title=full_title
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

fn app_menu_label(app: &WorkspaceAppMeta) -> String {
    app.short_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let title = app.title.trim();
            (!title.is_empty()).then_some(title)
        })
        .unwrap_or(app.id.as_str())
        .to_string()
}

const INLINE_MENU_LIMIT: usize = 3;

/// Admin Platform navigation projected from the declarative Registry.
#[derive(Debug, Clone)]
pub struct AdminNavItem {
    pub id: String,
    /// Menu-sized label: short_title → title.
    pub label: String,
    /// Full title retained for tooltip and accessible context.
    pub title: String,
    pub href: String,
    pub menu: String,
    pub order: i64,
}

#[derive(Debug, Clone)]
struct StageNavItem {
    id: String,
    label: String,
    title: String,
    href: String,
    profile: &'static str,
    group_label: &'static str,
    kind: &'static str,
    surface: &'static str,
    active: bool,
}

pub(super) fn visible_menu_indices(len: usize, active_index: Option<usize>) -> Vec<usize> {
    if len <= INLINE_MENU_LIMIT {
        return (0..len).collect();
    }
    if let Some(index) = active_index.filter(|index| *index >= INLINE_MENU_LIMIT) {
        return vec![0, 1, index];
    }
    (0..INLINE_MENU_LIMIT).collect()
}

fn app_admin_strip_view(
    items: &[AdminNavItem],
    active_id: Option<&str>,
    show_separator: bool,
) -> AnyView {
    if items.is_empty() {
        return view! { <></> }.into_any();
    }
    let active = active_id.map(str::trim).unwrap_or("");
    let active_index = items
        .iter()
        .position(|item| !active.is_empty() && item.id == active);
    let chips = visible_menu_indices(items.len(), active_index)
        .into_iter()
        .map(|index| {
            let item = &items[index];
            let is_active = !active.is_empty() && item.id == active;
            let class = if is_active {
                "topbar-chip is-active"
            } else {
                "topbar-chip"
            };
            let href = item.href.clone();
            let label = item.label.clone();
            let title = item.title.clone();
            let id = item.id.clone();
            view! {
                <a class=class href=href data-mei-admin-item=id title=title>
                    <span class="mode-label">{label}</span>
                </a>
            }
        })
        .collect_view();
    view! {
        <div class="topbar-admin-cluster inline-flex min-w-0 shrink items-center gap-1">
            {show_separator
                .then(|| {
                    view! { <span class="topbar-context-sep" aria-hidden="true">"|"</span> }
                })}
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

pub(super) fn stage_route_profile(route: &CompiledSceneRoute) -> StageProfile {
    StageProfile::from_route_meta(route.kind.as_str(), route.target_file.as_str())
}

fn build_stage_nav_items(
    active_app_path: &str,
    current_scene: Option<&str>,
    access_stage_routes: Option<&[CompiledSceneRoute]>,
    active_tab: Option<&str>,
    mark_active: bool,
) -> Vec<StageNavItem> {
    let Some(routes) = access_stage_routes.filter(|entries| !entries.is_empty()) else {
        return Vec::new();
    };
    let stages = routes
        .iter()
        .filter(|route| is_stage_registry_candidate(route))
        .collect::<Vec<_>>();
    if stages.is_empty() {
        return Vec::new();
    }
    let current = current_scene.map(str::trim).unwrap_or("");
    let active_id = if mark_active {
        stages
            .iter()
            .copied()
            .find(|route| route.scene_id == current)
            .or_else(|| stages.iter().copied().find(|route| route.is_default))
            .or_else(|| stages.first().copied())
            .map(|route| route.scene_id.as_str())
    } else {
        None
    };
    stages
        .into_iter()
        .map(|route| {
            let profile = stage_route_profile(route);
            let (profile_slug, group_label, kind, surface) = match profile {
                StageProfile::Cockpit => ("cockpit", "驾驶舱", "scene", "viewport"),
                StageProfile::Slides => ("slides", "幻灯片", "presentation", "paged"),
                StageProfile::Page => ("page", "页面/报告", "document", "document"),
            };
            let title = route
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(route.scene_id.as_str())
                .to_string();
            let label = route
                .short_title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(title.as_str())
                .to_string();
            StageNavItem {
                id: route.scene_id.clone(),
                label,
                title,
                href: app_scene_href(
                    active_app_path,
                    Some(route.scene_id.as_str()),
                    active_tab,
                    None,
                    None,
                    None,
                ),
                profile: profile_slug,
                group_label,
                kind,
                surface,
                active: active_id == Some(route.scene_id.as_str()),
            }
        })
        .collect()
}

fn stage_link_view(item: &StageNavItem, class: &'static str) -> AnyView {
    view! {
        <a
            class=class
            href=item.href.clone()
            title=item.title.clone()
            role=if class.contains("topbar-more-card") { "menuitem" } else { "link" }
            data-mei-spa-nav="1"
            data-mei-stage-scene=item.id.clone()
            data-mei-stage-kind=item.kind
            data-mei-stage-profile=item.profile
            data-mei-stage-surface=item.surface
        >
            <span class=if class.contains("topbar-more-card") { "topbar-more-card-label" } else { "mode-label" }>
                {item.label.clone()}
            </span>
        </a>
    }
    .into_any()
}

fn stage_strip_view(items: &[StageNavItem]) -> AnyView {
    if items.is_empty() {
        return view! { <></> }.into_any();
    }
    let active_index = items.iter().position(|item| item.active);
    let chips = visible_menu_indices(items.len(), active_index)
        .into_iter()
        .map(|index| {
            let item = &items[index];
            stage_link_view(
                item,
                if item.active {
                    "topbar-chip is-active"
                } else {
                    "topbar-chip"
                },
            )
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

fn app_navigation_more_view(
    stages: &[StageNavItem],
    admin_items: &[AdminNavItem],
    admin_active_id: Option<&str>,
) -> AnyView {
    if stages.len() <= INLINE_MENU_LIMIT && admin_items.len() <= INLINE_MENU_LIMIT {
        return view! { <></> }.into_any();
    }
    let stage_sections = ["cockpit", "slides", "page"]
        .into_iter()
        .filter_map(|profile| {
            let group = stages
                .iter()
                .filter(|item| item.profile == profile)
                .collect::<Vec<_>>();
            let title = group.first()?.group_label;
            let cards = group
                .into_iter()
                .map(|item| {
                    stage_link_view(
                        item,
                        if item.active {
                            "topbar-more-card is-active"
                        } else {
                            "topbar-more-card"
                        },
                    )
                })
                .collect_view();
            Some(view! {
                <section class="topbar-more-section" data-mei-menu-group=profile>
                    <h2 class="topbar-more-section-title">{title}</h2>
                    <div class="topbar-more-grid">{cards}</div>
                </section>
            })
        })
        .collect_view();
    let mut admin_groups: BTreeMap<String, Vec<&AdminNavItem>> = BTreeMap::new();
    for item in admin_items {
        let menu = item.menu.trim();
        admin_groups
            .entry(if menu.is_empty() {
                "应用管理".to_string()
            } else {
                menu.to_string()
            })
            .or_default()
            .push(item);
    }
    let admin_active = admin_active_id.map(str::trim).unwrap_or("");
    let admin_sections = admin_groups
        .into_iter()
        .map(|(menu, mut items)| {
            items.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then(left.label.cmp(&right.label))
            });
            let cards = items
                .into_iter()
                .map(|item| {
                    let active = !admin_active.is_empty() && item.id == admin_active;
                    view! {
                        <a
                            class=if active { "topbar-more-card is-active" } else { "topbar-more-card" }
                            href=item.href.clone()
                            title=item.title.clone()
                            role="menuitem"
                            data-mei-admin-item=item.id.clone()
                        >
                            <span class="topbar-more-card-label">{item.label.clone()}</span>
                        </a>
                    }
                })
                .collect_view();
            view! {
                <section class="topbar-more-section" data-mei-menu-group="admin">
                    <h2 class="topbar-more-section-title">{menu}</h2>
                    <div class="topbar-more-grid">{cards}</div>
                </section>
            }
        })
        .collect_view();
    view! {
        <div class="topbar-more">
            <details class="app-group-dropdown topbar-more-dropdown">
                <summary
                    class="topbar-more-trigger"
                    aria-label="展开全部应用菜单"
                    aria-haspopup="menu"
                    aria-expanded="false"
                    aria-controls="mei-topbar-more-panel"
                    title="更多 Stage 与应用管理"
                >
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" aria-hidden="true">
                        <circle cx="12" cy="5" r="1.75"/>
                        <circle cx="12" cy="12" r="1.75"/>
                        <circle cx="12" cy="19" r="1.75"/>
                    </svg>
                </summary>
                <div
                    id="mei-topbar-more-panel"
                    class="app-group-menu topbar-more-panel"
                    role="menu"
                    aria-label="全部应用菜单"
                >
                    {stage_sections}
                    {admin_sections}
                </div>
            </details>
        </div>
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
