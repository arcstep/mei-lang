use crate::model::{LayoutDecl, PanelDecl};

use super::super::constants::{PolicySpacing, METRIC_COMPOUND_BOTTOM_MAX, SLOT_BODY, SLOT_HEAD};
use super::super::nodes::{
    node_is_metric_card_like, node_is_metrics_2_1_item_like, panel_head_height_track, set_node_area,
};
use super::metrics_auto::default_metrics_auto_layout;
use super::metrics_grid::{
    default_metric_compound_2_1_layout, default_metrics_2_1_layout, default_metrics_2x2_layout,
    metric_compound_bottom_count,
};

pub(crate) fn inject_default_layout(panel: &mut PanelDecl, has_head: bool, has_body: bool) {
    panel.layout = match (has_head, has_body) {
        (true, true) => Some(default_layout_head_body(panel_head_height_track(panel))),
        (true, false) => Some(default_layout_single_slot(SLOT_HEAD)),
        (false, true) => Some(default_layout_single_slot(SLOT_BODY)),
        (false, false) => None,
    };
}

pub(crate) fn default_layout_head_body(head_track: Option<String>) -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec![
            head_track.unwrap_or_else(|| "auto".to_string()),
            "1fr".to_string(),
        ]),
        areas: Some(vec![
            vec![SLOT_HEAD.to_string()],
            vec![SLOT_BODY.to_string()],
        ]),
        gap: Some("2px".to_string()),
        padding: Some("0".to_string()),
        align: None,
        justify: None,
    }
}

pub(crate) fn default_layout_single_slot(slot: &str) -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string()]),
        areas: Some(vec![vec![slot.to_string()]]),
        gap: Some("0".to_string()),
        padding: Some("0".to_string()),
        align: None,
        justify: None,
    }
}

pub(crate) fn default_metrics_strip_layout(count: usize, spacing: &PolicySpacing) -> LayoutDecl {
    let mut areas = Vec::with_capacity(count);
    let mut columns = Vec::with_capacity(count);
    for idx in 0..count {
        areas.push(format!("m{idx}"));
        columns.push("1fr".to_string());
    }
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(columns),
        rows: Some(vec!["auto".to_string()]),
        areas: Some(vec![areas]),
        gap: Some(spacing.gap.clone()),
        padding: Some(spacing.padding.clone()),
        align: Some("stretch".to_string()),
        justify: None,
    }
}
pub(crate) fn layout_has_slot(layout: Option<&LayoutDecl>, slot: &str) -> bool {
    layout
        .and_then(|value| value.areas.as_ref())
        .is_some_and(|rows| {
            rows.iter()
                .flat_map(|row| row.iter())
                .any(|cell| cell == slot)
        })
}
pub(crate) fn should_inject_metrics_strip(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() < 2 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

pub(crate) fn should_inject_metrics_2x2(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 4 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

pub(crate) fn should_inject_metrics_auto(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() < 2 {
        return false;
    }
    panel.blocks.iter().all(node_is_metrics_2_1_item_like)
}

pub(crate) fn should_inject_metrics_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 3 {
        return false;
    }
    panel.blocks.iter().all(node_is_metrics_2_1_item_like)
}

pub(crate) fn should_inject_metric_compound_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head {
        return false;
    }
    let bottom = metric_compound_bottom_count(panel);
    if bottom < 1 || bottom > METRIC_COMPOUND_BOTTOM_MAX {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

pub(crate) fn inject_default_metrics_strip_layout(panel: &mut PanelDecl, spacing: &PolicySpacing) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_strip_layout(panel.blocks.len(), spacing));
}

pub(crate) fn inject_default_metrics_2x2_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_2x2_layout(panel));
}

pub(crate) fn inject_default_metrics_auto_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_auto_layout(panel));
}

pub(crate) fn inject_default_metrics_2_1_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_2_1_layout(panel));
}

pub(crate) fn inject_default_metric_compound_2_1_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        let area = if idx == 0 {
            "top".to_string()
        } else {
            format!("b{}", idx - 1)
        };
        set_node_area(node, &area);
    }
    panel.layout = Some(default_metric_compound_2_1_layout(panel));
}
