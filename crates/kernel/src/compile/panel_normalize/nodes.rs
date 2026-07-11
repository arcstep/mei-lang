use serde_json::Value;

use crate::model::{UiNodeDecl, UiTreeNode};

use super::constants::{CONTENT_ZONE, LAYOUT_POLICY_METRIC_COMPOUND_2_1, PROP_METRIC_CARD};
use super::css_util::{px_track, value_as_px};
use super::spacing::panel_layout_policy;

pub(super) fn remap_block_areas_to_body(blocks: &mut [UiTreeNode]) {
    for node in blocks {
        match node {
            UiTreeNode::Block(block) => {
                let area = block.area.as_deref().map(str::trim).unwrap_or("");
                if area.is_empty() || area.eq_ignore_ascii_case("auto") {
                    block.area = Some(CONTENT_ZONE.to_string());
                }
            }
            UiTreeNode::Panel(panel) => remap_block_areas_to_body(&mut panel.blocks),
            UiTreeNode::PanelRefEmbed(_) => {}
        }
    }
}
pub(super) fn node_is_metric_card_like(node: &UiTreeNode) -> bool {
    match node {
        UiTreeNode::Panel(panel) => panel
            .props
            .as_object()
            .and_then(|map| map.get(PROP_METRIC_CARD))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

pub(super) fn node_is_metrics_2_1_item_like(node: &UiTreeNode) -> bool {
    if node_is_metric_card_like(node) {
        return true;
    }
    match node {
        UiTreeNode::Panel(panel) => panel_layout_policy(panel)
            .as_deref()
            .is_some_and(|policy| policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1),
        _ => false,
    }
}

pub(super) fn blocks_touch_slot(blocks: &[UiTreeNode], slot: &str) -> bool {
    blocks
        .iter()
        .any(|node| node_area(node).is_some_and(|area| area == slot))
}

pub(super) fn node_area(node: &UiTreeNode) -> Option<&str> {
    match node {
        UiTreeNode::Block(block) => block.area.as_deref(),
        UiTreeNode::Panel(panel) => panel.area.as_deref(),
        UiTreeNode::PanelRefEmbed(embed) => embed.area.as_deref(),
    }
}

pub(super) fn ensure_node_area(node: &mut UiTreeNode, slot: &str) {
    match node {
        UiTreeNode::Block(block) => {
            if block
                .area
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("auto"))
            {
                block.area = Some(slot.to_string());
            }
        }
        UiTreeNode::Panel(panel) => {
            if panel
                .area
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("auto"))
            {
                panel.area = Some(slot.to_string());
            }
        }
        UiTreeNode::PanelRefEmbed(_) => {}
    }
}

pub(super) fn set_node_area(node: &mut UiTreeNode, area: &str) {
    match node {
        UiTreeNode::Block(block) => block.area = Some(area.to_string()),
        UiTreeNode::Panel(panel) => panel.area = Some(area.to_string()),
        UiTreeNode::PanelRefEmbed(embed) => embed.area = Some(area.to_string()),
    }
}
pub(super) fn node_has_explicit_area(node: &UiTreeNode) -> bool {
    node_area(node)
        .map(str::trim)
        .is_some_and(|area| !area.is_empty() && !area.eq_ignore_ascii_case("auto"))
}
pub(super) fn panel_px_prop(panel: &UiNodeDecl, key: &str) -> Option<f64> {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(value_as_px)
}

pub(super) fn panel_head_height_track(panel: &UiNodeDecl) -> Option<String> {
    panel
        .head_props
        .as_object()
        .and_then(|map| map.get("height"))
        .and_then(value_as_px)
        .map(px_track)
}

pub(super) fn node_height_track(node: &UiTreeNode) -> Option<f64> {
    match node {
        UiTreeNode::Panel(panel) => panel_px_prop(panel, "height"),
        _ => None,
    }
}

pub(super) fn node_width_track(node: &UiTreeNode) -> Option<f64> {
    match node {
        UiTreeNode::Panel(panel) => panel_px_prop(panel, "width"),
        _ => None,
    }
}
