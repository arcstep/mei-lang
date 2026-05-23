use serde_json::Value;

use crate::model::{LayoutDecl, PanelDecl};

use super::constants::{
    DEFAULT_METRIC_COMPOUND_2_1_GAP, DEFAULT_METRICS_2X2_COLUMNS, DEFAULT_METRICS_2X2_GAP,
    DEFAULT_METRICS_2X2_PADDING, DEFAULT_METRICS_2_1_COLUMNS, DEFAULT_METRICS_2_1_GAP,
    DEFAULT_METRICS_2_1_PADDING, PROP_LAYOUT_COLUMNS, SLOT_BODY, SLOT_HEAD, PolicySpacing,
};
use super::css_util::px_track;
use super::nodes::{
    node_height_track, node_is_metric_card_like, node_is_metrics_2_1_item_like, set_node_area,
};
use super::spacing::policy_spacing;
use super::nodes::panel_head_height_track;

pub(super) fn inject_default_layout(panel: &mut PanelDecl, has_head: bool, has_body: bool) {
    panel.layout = match (has_head, has_body) {
        (true, true) => Some(default_layout_head_body(panel_head_height_track(panel))),
        (true, false) => Some(default_layout_single_slot(SLOT_HEAD)),
        (false, true) => Some(default_layout_single_slot(SLOT_BODY)),
        (false, false) => None,
    };
}

pub(super) fn default_layout_head_body(head_track: Option<String>) -> LayoutDecl {
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
        gap: Some("0".to_string()),
        padding: Some("0".to_string()),
        align: None,
        justify: None,
    }
}

pub(super) fn default_layout_single_slot(slot: &str) -> LayoutDecl {
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

pub(super) fn default_metrics_strip_layout(count: usize, spacing: &PolicySpacing) -> LayoutDecl {
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

pub(super) fn default_metrics_2x2_layout(panel: &PanelDecl) -> LayoutDecl {
    let spacing = policy_spacing(panel, DEFAULT_METRICS_2X2_GAP, DEFAULT_METRICS_2X2_PADDING);
    let top_row = panel
        .blocks
        .iter()
        .take(2)
        .filter_map(node_height_track)
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(existing) => Some(existing.max(value)),
            None => Some(value),
        })
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string());
    let bottom_row = panel
        .blocks
        .iter()
        .skip(2)
        .take(2)
        .filter_map(node_height_track)
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(existing) => Some(existing.max(value)),
            None => Some(value),
        })
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string());
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(
            DEFAULT_METRICS_2X2_COLUMNS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        ),
        rows: Some(vec![top_row, bottom_row]),
        areas: Some(vec![
            vec!["m0".to_string(), "m1".to_string()],
            vec!["m2".to_string(), "m3".to_string()],
        ]),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
        justify: None,
    }
}

pub(super) fn default_metrics_2_1_layout(panel: &PanelDecl) -> LayoutDecl {
    let columns = panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_COLUMNS))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| values.len() == 3)
        .unwrap_or_else(|| {
            DEFAULT_METRICS_2_1_COLUMNS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        });
    let spacing = policy_spacing(panel, DEFAULT_METRICS_2_1_GAP, DEFAULT_METRICS_2_1_PADDING);
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(columns),
        rows: Some(vec!["auto".to_string()]),
        areas: Some(vec![vec![
            "m0".to_string(),
            "m1".to_string(),
            "m2".to_string(),
        ]]),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
        justify: None,
    }
}

pub(super) fn default_metric_compound_2_1_layout(panel: &PanelDecl) -> LayoutDecl {
    let spacing = policy_spacing(panel, DEFAULT_METRIC_COMPOUND_2_1_GAP, "0");
    let top_row = panel
        .blocks
        .first()
        .and_then(node_height_track)
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string());
    let bottom_row = panel
        .blocks
        .iter()
        .skip(1)
        .filter_map(node_height_track)
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(existing) => Some(existing.max(value)),
            None => Some(value),
        })
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string());
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec![
            "1fr".to_string(),
            "1fr".to_string(),
            "1fr".to_string(),
        ]),
        rows: Some(vec![top_row, bottom_row]),
        areas: Some(vec![
            vec!["top".to_string(), "top".to_string(), "top".to_string()],
            vec!["b0".to_string(), "b1".to_string(), "b2".to_string()],
        ]),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
        justify: None,
    }
}
pub(super) fn layout_has_slot(layout: Option<&LayoutDecl>, slot: &str) -> bool {
    layout
        .and_then(|value| value.areas.as_ref())
        .is_some_and(|rows| {
            rows.iter()
                .flat_map(|row| row.iter())
                .any(|cell| cell == slot)
        })
}
pub(super) fn should_inject_metrics_strip(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() < 2 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

pub(super) fn should_inject_metrics_2x2(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 4 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

pub(super) fn should_inject_metrics_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 3 {
        return false;
    }
    panel.blocks.iter().all(node_is_metrics_2_1_item_like)
}

pub(super) fn should_inject_metric_compound_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 4 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

pub(super) fn inject_default_metrics_strip_layout(panel: &mut PanelDecl, spacing: &PolicySpacing) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_strip_layout(panel.blocks.len(), spacing));
}

pub(super) fn inject_default_metrics_2x2_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_2x2_layout(panel));
}

pub(super) fn inject_default_metrics_2_1_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_2_1_layout(panel));
}

pub(super) fn inject_default_metric_compound_2_1_layout(panel: &mut PanelDecl) {
    let areas = ["top", "b0", "b1", "b2"];
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, areas[idx]);
    }
    panel.layout = Some(default_metric_compound_2_1_layout(panel));
}
