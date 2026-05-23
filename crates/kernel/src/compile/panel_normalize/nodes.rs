use serde_json::Value;

use crate::model::{PanelDecl, UiNodeDecl};

use super::constants::{LAYOUT_POLICY_METRIC_COMPOUND_2_1, PROP_METRIC_CARD, SLOT_BODY};
use super::css_util::{value_as_px, px_track};
use super::spacing::panel_layout_policy;

pub(super) fn remap_block_areas_to_body(blocks: &mut [UiNodeDecl]) {
    for node in blocks {
        match node {
            UiNodeDecl::Block(block) => {
                let area = block.area.as_deref().map(str::trim).unwrap_or("");
                if area.is_empty() || area.eq_ignore_ascii_case("auto") {
                    block.area = Some(SLOT_BODY.to_string());
                }
            }
            UiNodeDecl::Panel(panel) => remap_block_areas_to_body(&mut panel.blocks),
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}
pub(super) fn node_is_metric_card_like(node: &UiNodeDecl) -> bool {
    match node {
        UiNodeDecl::Panel(panel) => panel
            .props
            .as_object()
            .and_then(|map| map.get(PROP_METRIC_CARD))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

pub(super) fn node_is_metrics_2_1_item_like(node: &UiNodeDecl) -> bool {
    if node_is_metric_card_like(node) {
        return true;
    }
    match node {
        UiNodeDecl::Panel(panel) => panel_layout_policy(panel)
            .as_deref()
            .is_some_and(|policy| policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1),
        _ => false,
    }
}

pub(super) fn blocks_touch_slot(blocks: &[UiNodeDecl], slot: &str) -> bool {
    blocks
        .iter()
        .any(|node| node_area(node).is_some_and(|area| area == slot))
}

pub(super) fn node_area(node: &UiNodeDecl) -> Option<&str> {
    match node {
        UiNodeDecl::Block(block) => block.area.as_deref(),
        UiNodeDecl::Panel(panel) => panel.area.as_deref(),
        UiNodeDecl::PanelRefEmbed(embed) => embed.area.as_deref(),
    }
}

pub(super) fn ensure_node_area(node: &mut UiNodeDecl, slot: &str) {
    match node {
        UiNodeDecl::Block(block) => {
            if block
                .area
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("auto"))
            {
                block.area = Some(slot.to_string());
            }
        }
        UiNodeDecl::Panel(panel) => {
            if panel
                .area
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("auto"))
            {
                panel.area = Some(slot.to_string());
            }
        }
        UiNodeDecl::PanelRefEmbed(_) => {}
    }
}

pub(super) fn set_node_area(node: &mut UiNodeDecl, area: &str) {
    match node {
        UiNodeDecl::Block(block) => block.area = Some(area.to_string()),
        UiNodeDecl::Panel(panel) => panel.area = Some(area.to_string()),
        UiNodeDecl::PanelRefEmbed(embed) => embed.area = Some(area.to_string()),
    }
}
pub(super) fn node_has_explicit_area(node: &UiNodeDecl) -> bool {
    node_area(node)
        .map(str::trim)
        .is_some_and(|area| !area.is_empty() && !area.eq_ignore_ascii_case("auto"))
}
pub(super) fn panel_px_prop(panel: &PanelDecl, key: &str) -> Option<f64> {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(value_as_px)
}

pub(super) fn panel_head_height_track(panel: &PanelDecl) -> Option<String> {
    panel
        .head_props
        .as_object()
        .and_then(|map| map.get("height"))
        .and_then(value_as_px)
        .map(px_track)
}

pub(super) fn node_height_track(node: &UiNodeDecl) -> Option<f64> {
    match node {
        UiNodeDecl::Panel(panel) => panel_px_prop(panel, "height"),
        _ => None,
    }
}
