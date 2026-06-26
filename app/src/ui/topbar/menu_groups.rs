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
            || (normalized == "manage" && route_mode == UiRouteMode::Build)
            || (normalized == "access" && route_mode == UiRouteMode::App)
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
    inject_stock_pack_menu_groups(
        &mut groups,
        menus,
        catalog_app_id,
        &group_overrides,
    );
    if let Some(cfg) = root_menu {
        for item in &cfg.items {
            let catalog = item
                .catalog
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            let pack = item
                .pack
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            if catalog.is_none() || pack.is_none() {
                continue;
            }
            if !menu_item_visible_for_route(Some(item), route_mode) {
                continue;
            }
            let group = item
                .group
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    if catalog.as_deref() == Some("templates") {
                        "templates".to_string()
                    } else {
                        "components".to_string()
                    }
                });
            let label = item
                .label
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| pack.clone().unwrap_or_else(|| item.app_id.clone()));
            push_menu_item(
                &mut groups,
                &group,
                &group_overrides,
                TopbarMenuItem {
                    app_id: item.app_id.clone(),
                    subgroup: item.subgroup.clone(),
                    label,
                    order: item.order.unwrap_or(i32::MAX / 2),
                    catalog,
                    pack,
                },
            );
        }
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

fn inject_stock_pack_menu_groups(
    groups: &mut BTreeMap<String, TopbarMenuGroup>,
    menus: Option<&TopbarMenuContext>,
    catalog_app_id: &str,
    group_overrides: &BTreeMap<String, (Option<String>, i32)>,
) {
    let Some(menus) = menus else {
        return;
    };
    let facets = [
        ("components", "components", menus.stock_component_packs.as_slice()),
        ("templates", "templates", menus.stock_template_packs.as_slice()),
    ];
    for (group_id, catalog, packs) in facets {
        if packs.is_empty() {
            continue;
        }
        for (index, pack) in packs.iter().enumerate() {
            push_menu_item(
                groups,
                group_id,
                group_overrides,
                TopbarMenuItem {
                    app_id: catalog_app_id.to_string(),
                    subgroup: None,
                    label: pack.clone(),
                    order: (index as i32 + 1) * 10,
                    catalog: Some(catalog.to_string()),
                    pack: Some(pack.clone()),
                },
            );
        }
    }
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
    use crate::ui::{TopbarMenuConfig, TopbarMenuConfigGroup, TopbarMenuContext};

    #[test]
    fn stock_pack_groups_visible_in_all_route_modes() {
        let apps = vec![WorkspaceAppMeta {
            id: "hello".to_string(),
            title: "Hello".to_string(),
            root: "apps/hello".to_string(),
        }];
        let menus = TopbarMenuContext {
            root: Some(TopbarMenuConfig {
                groups: vec![
                    TopbarMenuConfigGroup {
                        id: "components".to_string(),
                        label: Some("组件".to_string()),
                        order: Some(5),
                    },
                    TopbarMenuConfigGroup {
                        id: "templates".to_string(),
                        label: Some("模板".to_string()),
                        order: Some(6),
                    },
                ],
                items: vec![],
                skip_prefixes: vec![],
            }),
            by_segment: Default::default(),
            workspace_label: Some("Hello".to_string()),
            stock_catalog_app_id: Some("_stock-catalog".to_string()),
            stock_catalog_app_title: Some("组件库".to_string()),
            stock_component_packs: vec!["chart/echarts".to_string()],
            stock_template_packs: vec!["cockpit".to_string()],
        };
        for mode in [
            UiRouteMode::Build,
            UiRouteMode::App,
            UiRouteMode::Runtime,
            UiRouteMode::Config,
        ] {
            let groups = build_topbar_menu_groups(apps.as_slice(), Some(&menus), mode);
            assert!(
                groups.iter().any(|group| group.id == "components"),
                "components group missing in {:?}",
                mode
            );
            assert!(
                groups.iter().any(|group| group.id == "templates"),
                "templates group missing in {:?}",
                mode
            );
        }
    }
}
