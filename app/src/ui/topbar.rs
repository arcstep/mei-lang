use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use std::collections::BTreeMap;

use super::manage_routing::{
    access_scene_query, access_scene_route_suffix, canonical_scene_for_script_target,
    encode_query_value, route_query,
};
use super::preview;
use super::route::UiRouteMode;
use super::{TopbarMenuConfig, TopbarMenuContext};
#[derive(Debug, Clone)]
struct TopbarMenuItem {
    app_id: String,
    subgroup: Option<String>,
    label: String,
    order: i32,
}

#[derive(Debug, Clone)]
struct TopbarMenuGroup {
    id: String,
    label: String,
    order: i32,
    items: Vec<TopbarMenuItem>,
}

fn first_path_segment(app_id: &str) -> &str {
    app_id
        .split('/')
        .find(|value| !value.is_empty())
        .unwrap_or("")
}

fn build_topbar_menu_groups(
    apps: &[WorkspaceAppMeta],
    menus: Option<&TopbarMenuContext>,
) -> Vec<TopbarMenuGroup> {
    let mut groups: BTreeMap<String, TopbarMenuGroup> = BTreeMap::new();
    for app in apps {
        let segment = first_path_segment(&app.id);
        let config = menus.and_then(|menu| menu.by_segment.get(segment).or(menu.root.as_ref()));

        let mut group_overrides: BTreeMap<String, (Option<String>, i32)> = BTreeMap::new();
        if let Some(cfg) = config {
            for group in &cfg.groups {
                group_overrides.insert(
                    group.id.clone(),
                    (group.label.clone(), group.order.unwrap_or(i32::MAX / 2)),
                );
            }
        }
        let item_overrides = config
            .map(|cfg| {
                cfg.items
                    .iter()
                    .map(|item| (item.app_id.clone(), item.clone()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let skip_prefixes = normalized_skip_prefixes(config);
        let mut segments = app
            .id
            .split('/')
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        while segments.len() > 1 {
            let head = segments.first().map(|value| value.to_ascii_lowercase());
            if head
                .as_deref()
                .is_some_and(|value| skip_prefixes.iter().any(|prefix| prefix == value))
            {
                segments.remove(0);
                continue;
            }
            break;
        }
        if segments.is_empty() {
            continue;
        }
        let (mut group, mut subgroup, mut label) = menu_placement_from_segments(&segments);
        let mut item_order = infer_order_from_label(&label).unwrap_or(i32::MAX / 2);
        if let Some(override_item) = item_overrides.get(&app.id) {
            if let Some(override_group) = &override_item.group {
                group = override_group.clone();
            }
            if override_item.subgroup.is_some() {
                subgroup = override_item.subgroup.clone();
            }
            if let Some(override_label) = &override_item.label {
                label = override_label.clone();
            }
            if let Some(order) = override_item.order {
                item_order = order;
            }
        }
        let (group_label, group_order) =
            if let Some((label_override, order_override)) = group_overrides.get(&group) {
                (
                    label_override
                        .clone()
                        .unwrap_or_else(|| menu_group_display_label(&group)),
                    *order_override,
                )
            } else {
                (menu_group_display_label(&group), i32::MAX / 2)
            };
        groups
            .entry(group.clone())
            .or_insert_with(|| TopbarMenuGroup {
                id: group.clone(),
                label: group_label,
                order: group_order,
                items: Vec::new(),
            })
            .items
            .push(TopbarMenuItem {
                app_id: app.id.clone(),
                subgroup,
                label,
                order: item_order,
            });
    }
    let mut ordered = groups.into_values().collect::<Vec<_>>();
    for group in &mut ordered {
        group.items.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then(left.subgroup.cmp(&right.subgroup))
                .then(left.label.cmp(&right.label))
                .then(left.app_id.cmp(&right.app_id))
        });
    }
    ordered.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then(left.label.cmp(&right.label))
            .then(left.id.cmp(&right.id))
    });
    ordered
}

fn menu_placement_from_segments(segments: &[&str]) -> (String, Option<String>, String) {
    if segments.len() == 1 {
        let only = segments[0];
        if let Some((prefix, rest)) = only.split_once('-') {
            if !prefix.is_empty() && !rest.is_empty() {
                return (
                    prefix.to_string(),
                    None,
                    rest.trim_start_matches('-').to_string(),
                );
            }
        }
        return ("misc".to_string(), None, only.to_string());
    }
    if segments.len() == 2 {
        return (segments[0].to_string(), None, segments[1].to_string());
    }
    (
        segments[0].to_string(),
        Some(segments[1].to_string()),
        segments[2..].join("/"),
    )
}

fn menu_group_display_label(group: &str) -> String {
    if group == "misc" {
        "其他".to_string()
    } else {
        group.to_string()
    }
}

fn normalized_skip_prefixes(config: Option<&TopbarMenuConfig>) -> Vec<String> {
    if let Some(config) = config {
        if !config.skip_prefixes.is_empty() {
            return config
                .skip_prefixes
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect();
        }
    }
    vec!["examples".to_string(), "workspaces".to_string()]
}

fn infer_order_from_label(label: &str) -> Option<i32> {
    let mut digits = String::new();
    for ch in label.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i32>().ok()
}
pub(super) fn topbar_view(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    active_app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> AnyView {
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let route_query = route_query(route_mode, selected_scene, preview_target, active_tab);
    let access_scene_for_href = selected_scene
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter(|sc| {
            compiled
                .scene_routes
                .iter()
                .any(|r| r.scene_id == *sc && r.access_export)
        })
        .filter(|sc| {
            if route_mode == UiRouteMode::Manage {
                canonical_scene_for_script_target(compiled, Some(compiled.active_target_file.as_str()))
                    .is_some_and(|canon| canon == *sc)
            } else {
                compiled.active_scene.as_deref() == Some(*sc)
            }
        });
    let access_entry_query = access_scene_query(access_scene_for_href);
    let access_disabled = access_entry_query.is_empty();
    let menu_groups = build_topbar_menu_groups(apps, topbar_menu);
    let active_menu_context = menu_groups.iter().find_map(|group| {
        group
            .items
            .iter()
            .find(|item| item.app_id.as_str() == active_app_path)
            .map(|item| (group.label.clone(), item.label.clone()))
    });
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
            let direct_links = direct_items
                .iter()
                .map(|item| {
                    let class = if item.app_id.as_str() == active_app_path {
                        "app-tab app-tab-sub active"
                    } else {
                        "app-tab app-tab-sub"
                    };
                    let href = if route_mode == UiRouteMode::Access
                        && item.app_id.as_str() != active_app_path
                    {
                        format!("/apps/manage/{}", item.app_id)
                    } else {
                        format!("/apps/{}/{}{}", route_mode.slug(), item.app_id, route_query)
                    };
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
                            let href = if route_mode == UiRouteMode::Access
                                && item.app_id.as_str() != active_app_path
                            {
                                format!("/apps/manage/{}", item.app_id)
                            } else {
                                format!(
                                    "/apps/{}/{}{}",
                                    route_mode.slug(),
                                    item.app_id,
                                    route_query
                                )
                            };
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
        })
        .collect_view();
    let active_item_breadcrumb = active_menu_context
        .map(|(group_label, item_label)| {
            let aria_label = format!("当前位置：{group_label} / {item_label}");
            view! {
                <div class="app-current-path inline-flex min-w-0 max-w-[min(260px,26vw)] items-center gap-1.5 border-l border-slate-400/15 pl-2.5 text-[11px] text-slate-400" aria-label=aria_label>
                    <span class="app-current-path-trail inline-flex min-w-0 items-center gap-1.5 whitespace-nowrap">
                        <span class="app-current-path-group shrink-0 text-slate-400">{group_label}</span>
                        <span class="app-current-path-separator shrink-0 text-slate-400/70" aria-hidden="true">"/"</span>
                        <span class="app-current-path-item min-w-0 overflow-hidden text-ellipsis text-slate-200">{item_label}</span>
                    </span>
                </div>
            }
            .into_any()
        })
        .unwrap_or_else(|| view! { <></> }.into_any());
    let manage_href = if route_mode == UiRouteMode::Access {
        let mut parts = Vec::new();
        let file = preview_target
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| compiled.active_target_file.as_str());
        if !file.trim().is_empty() {
            parts.push(format!("file={}", encode_query_value(file.trim())));
        }
        if let Some(t) = active_tab.map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("tab={}", encode_query_value(t)));
        }
        if parts.is_empty() {
            format!("/apps/manage/{active_app_path}")
        } else {
            format!("/apps/manage/{active_app_path}?{}", parts.join("&"))
        }
    } else {
        format!("/apps/manage/{}{}", active_app_path, route_query)
    };
    let access_href = if access_disabled {
        "#".to_string()
    } else {
        format!("/apps/access/{}{}", active_app_path, access_entry_query)
    };
    let presentation_suffix =
        access_scene_route_suffix(access_scene_for_href, None, Some("none"));
    let presentation_href = format!("/apps/access/{}{}", active_app_path, presentation_suffix);
    let mode_tabs = view! {
        <div class="mode-tabs inline-flex items-center">
            <sl-button-group class="mode-tab-group" label="模式切换">
                <sl-button
                    class=if route_mode == UiRouteMode::Manage { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                    size="small"
                    href=manage_href
                    title="编辑态"
                    aria-label="编辑态"
                >
                    <span class="mode-btn-content">
                        <span class="mode-icon" aria-hidden="true">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M12 20h9"/>
                                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z"/>
                            </svg>
                        </span>
                        <span class="mode-label">"管理"</span>
                    </span>
                </sl-button>
                <sl-button
                    class=if route_mode == UiRouteMode::Access { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                    size="small"
                    href=access_href
                    disabled=access_disabled
                    title=if access_disabled {
                        "当前文件没有可发布的 scene route，无法进入访问态"
                    } else {
                        "访问态"
                    }
                    aria-label=if access_disabled { "访问态（不可用）" } else { "访问态" }
                >
                    <span class="mode-btn-content">
                        <span class="mode-icon" aria-hidden="true">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <rect x="3" y="4" width="18" height="14" rx="2"/>
                                <path d="M8 20h8"/>
                                <path d="M12 18v2"/>
                            </svg>
                        </span>
                        <span class="mode-label">"访问"</span>
                    </span>
                </sl-button>
            </sl-button-group>
        </div>
    };
    let launch_title = if stage_enabled {
        "在新标签页打开"
    } else {
        "在新标签页打开无 Chrome 应用"
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
            <div class="topbar-actions flex shrink-0 flex-nowrap items-center justify-end gap-1.5">
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
            </div>
        </header>
    }
    .into_any()
}
