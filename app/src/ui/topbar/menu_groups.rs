use mei_lang_kernel::WorkspaceAppMeta;
use std::collections::BTreeMap;

use super::super::route::UiRouteMode;
use super::super::{TopbarMenuConfig, TopbarMenuContext};

#[derive(Debug, Clone)]
pub(crate) struct TopbarMenuItem {
    pub(crate) app_id: String,
    pub(crate) subgroup: Option<String>,
    pub(crate) label: String,
    pub(crate) order: i32,
    /// Stock catalog facet: `components` | `templates`.
    pub(crate) catalog: Option<String>,
    /// Component pack path or template top folder within the facet.
    pub(crate) pack: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct TopbarMenuGroup {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) order: i32,
    pub(crate) items: Vec<TopbarMenuItem>,
}

fn first_path_segment(app_id: &str) -> &str {
    app_id
        .split('/')
        .find(|value| !value.is_empty())
        .unwrap_or("")
}

fn menu_item_visible_for_route(
    override_item: Option<&super::super::TopbarMenuConfigItem>,
    route_mode: UiRouteMode,
) -> bool {
    let Some(item) = override_item else {
        return true;
    };
    if item.modes.is_empty() {
        return true;
    }
    let slug = route_mode.slug();
    item.modes.iter().any(|mode| {
        let normalized = mode.trim().to_ascii_lowercase();
        normalized == slug
            || (normalized == "manage" && route_mode == UiRouteMode::Layout)
            || (normalized == "access" && route_mode == UiRouteMode::App)
            || (normalized == "layout" && route_mode == UiRouteMode::Layout)
            || (normalized == "prototype" && route_mode == UiRouteMode::Prototype)
    })
}

pub(crate) fn build_topbar_menu_groups(
    apps: &[WorkspaceAppMeta],
    menus: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
) -> Vec<TopbarMenuGroup> {
    let root_menu = menus.and_then(|menu| menu.root.as_ref());
    let catalog_app_id = menus
        .and_then(|menu| menu.stock_catalog_app_id.as_deref())
        .unwrap_or("_stock-catalog");
    let mut groups: BTreeMap<String, TopbarMenuGroup> = BTreeMap::new();
    let mut group_overrides: BTreeMap<String, (Option<String>, i32)> = BTreeMap::new();
    if let Some(cfg) = root_menu {
        for group in &cfg.groups {
            group_overrides.insert(
                group.id.clone(),
                (group.label.clone(), group.order.unwrap_or(i32::MAX / 2)),
            );
        }
    }
    for app in apps {
        let segment = first_path_segment(&app.id);
        let config = menus.and_then(|menu| menu.by_segment.get(segment).or(menu.root.as_ref()));

        if let Some(cfg) = config {
            for group in cfg.groups.iter() {
                group_overrides.entry(group.id.clone()).or_insert((
                    group.label.clone(),
                    group.order.unwrap_or(i32::MAX / 2),
                ));
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
        if app.id == catalog_app_id {
            continue;
        }
        let (mut group, mut subgroup, mut label) = menu_placement_from_segments(&segments);
        let mut item_order = infer_order_from_label(&label).unwrap_or(i32::MAX / 2);
        let override_item = item_overrides.get(&app.id);
        if !menu_item_visible_for_route(override_item, route_mode) {
            continue;
        }
        if let Some(override_item) = override_item {
            if let Some(override_group) = &override_item.group {
                if !is_aggregate_menu_group(override_group.as_str()) {
                    group = override_group.clone();
                }
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
        push_menu_item(
            &mut groups,
            &group,
            &group_overrides,
            TopbarMenuItem {
                app_id: app.id.clone(),
                subgroup,
                label,
                order: item_order,
                catalog: None,
                pack: None,
            },
        );
    }
    let _ = catalog_app_id;
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

fn is_aggregate_menu_group(group: &str) -> bool {
    matches!(group.trim().to_ascii_lowercase().as_str(), "apps" | "应用")
}

fn push_menu_item(
    groups: &mut BTreeMap<String, TopbarMenuGroup>,
    group: &str,
    group_overrides: &BTreeMap<String, (Option<String>, i32)>,
    item: TopbarMenuItem,
) {
    let (group_label, group_order) =
        if let Some((label_override, order_override)) = group_overrides.get(group) {
            (
                label_override
                    .clone()
                    .unwrap_or_else(|| menu_group_display_label(group)),
                *order_override,
            )
        } else {
            (menu_group_display_label(group), i32::MAX / 2)
        };
    groups
        .entry(group.to_string())
        .or_insert_with(|| TopbarMenuGroup {
            id: group.to_string(),
            label: group_label,
            order: group_order,
            items: Vec::new(),
        })
        .items
        .push(item);
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
    match group {
        "misc" => "其他".to_string(),
        "components" => "组件".to_string(),
        "templates" => "模板".to_string(),
        _ => group.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{TopbarMenuConfig, TopbarMenuConfigGroup, TopbarMenuConfigItem, TopbarMenuContext};

    #[test]
    fn single_segment_apps_without_aggregate_group_land_in_misc() {
        let apps = vec![
            WorkspaceAppMeta {
                id: "data-demo".to_string(),
                title: "Data Demo".to_string(),
                root: "apps/data-demo".to_string(),
            },
            WorkspaceAppMeta {
                id: "mini-park".to_string(),
                title: "Mini Park".to_string(),
                root: "apps/mini-park".to_string(),
            },
        ];
        let groups = build_topbar_menu_groups(apps.as_slice(), None, UiRouteMode::App);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, "misc");
        assert_eq!(groups[0].items.len(), 2);
    }

    #[test]
    fn nested_app_paths_map_to_group_and_subgroup() {
        let apps = vec![WorkspaceAppMeta {
            id: "cockpit/multi-entry/02-demo".to_string(),
            title: "Demo".to_string(),
            root: "apps/cockpit/multi-entry/02-demo".to_string(),
        }];
        let groups = build_topbar_menu_groups(apps.as_slice(), None, UiRouteMode::App);
        let group = groups
            .iter()
            .find(|group| group.id == "cockpit")
            .expect("cockpit group");
        assert_eq!(group.items.len(), 1);
        assert_eq!(group.items[0].subgroup.as_deref(), Some("multi-entry"));
        assert_eq!(group.items[0].label, "02-demo");
    }

    #[test]
    fn stock_pack_groups_are_not_injected_into_topbar() {
        let apps = vec![WorkspaceAppMeta {
            id: "hello".to_string(),
            title: "Hello".to_string(),
            root: "apps/hello".to_string(),
        }];
        let menus = TopbarMenuContext {
            root: None,
            by_segment: Default::default(),
            workspace_label: Some("Hello".to_string()),
            stock_catalog_app_id: Some("_stock-catalog".to_string()),
            stock_catalog_app_title: Some("组件库".to_string()),
            stock_component_packs: vec!["chart/echarts".to_string()],
            stock_template_packs: vec!["cockpit".to_string()],
        };
        let groups = build_topbar_menu_groups(apps.as_slice(), Some(&menus), UiRouteMode::App);
        assert!(!groups.iter().any(|group| group.id == "components"));
        assert!(!groups.iter().any(|group| group.id == "templates"));
    }

    #[test]
    fn aggregate_menu_group_override_is_ignored() {
        let apps = vec![WorkspaceAppMeta {
            id: "data-demo".to_string(),
            title: "Data Demo".to_string(),
            root: "apps/data-demo".to_string(),
        }];
        let menus = TopbarMenuContext {
            root: Some(TopbarMenuConfig {
                groups: vec![TopbarMenuConfigGroup {
                    id: "apps".to_string(),
                    label: Some("应用".to_string()),
                    order: Some(10),
                }],
                items: vec![TopbarMenuConfigItem {
                    app_id: "data-demo".to_string(),
                    group: Some("apps".to_string()),
                    subgroup: None,
                    label: Some("Data Demo v2".to_string()),
                    order: Some(10),
                    modes: vec![],
                    catalog: None,
                    pack: None,
                }],
                skip_prefixes: vec![],
            }),
            by_segment: Default::default(),
            workspace_label: None,
            stock_catalog_app_id: None,
            stock_catalog_app_title: None,
            stock_component_packs: vec![],
            stock_template_packs: vec![],
        };
        let groups = build_topbar_menu_groups(apps.as_slice(), Some(&menus), UiRouteMode::App);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, "misc");
        assert_eq!(groups[0].items[0].label, "Data Demo v2");
    }

    #[test]
    fn skip_prefixes_strip_container_segments_before_placement() {
        let apps = vec![WorkspaceAppMeta {
            id: "examples/core/04-mini-shell".to_string(),
            title: "Mini Shell".to_string(),
            root: "apps/examples/core/04-mini-shell".to_string(),
        }];
        let menus = TopbarMenuContext {
            root: Some(TopbarMenuConfig {
                groups: vec![],
                items: vec![],
                skip_prefixes: vec!["examples".to_string()],
            }),
            by_segment: Default::default(),
            workspace_label: None,
            stock_catalog_app_id: None,
            stock_catalog_app_title: None,
            stock_component_packs: vec![],
            stock_template_packs: vec![],
        };
        let groups = build_topbar_menu_groups(apps.as_slice(), Some(&menus), UiRouteMode::App);
        let group = groups
            .iter()
            .find(|group| group.id == "core")
            .expect("core group after skip");
        assert_eq!(group.items[0].label, "04-mini-shell");
    }
}
