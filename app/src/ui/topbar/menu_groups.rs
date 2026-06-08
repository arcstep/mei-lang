use mei_lang_kernel::WorkspaceAppMeta;
use std::collections::BTreeMap;

use super::super::{TopbarMenuConfig, TopbarMenuContext};

#[derive(Debug, Clone)]
pub(crate) struct TopbarMenuItem {
    pub(crate) app_id: String,
    pub(crate) subgroup: Option<String>,
    pub(crate) label: String,
    pub(crate) order: i32,
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

pub(crate) fn build_topbar_menu_groups(
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
