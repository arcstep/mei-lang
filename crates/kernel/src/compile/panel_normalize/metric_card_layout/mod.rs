use serde_json::Value;

use crate::model::{BlockDecl, LayoutDecl, UiNodeDecl, UiTreeNode};

use super::nodes::{node_is_metric_card_like, panel_px_prop};

mod audit;
mod seed;

pub(crate) use seed::{
    seed_metric_block_vertical_align_from_base, seed_metric_desc_runtime_from_shell,
    seed_metric_slot_vertical_align_defaults_from_base,
};

pub(super) fn audit_metric_vertical_bands(
    panel: &UiNodeDecl,
    diagnostics: &mut Vec<crate::model::Diagnostic>,
    source_path: &str,
) {
    audit::audit_metric_vertical_bands(panel, diagnostics, source_path);
}

pub(super) const PROP_METRIC_TEMPLATE: &str = "__mei_metric_template";
pub(super) const PROP_METRIC_TITLE_RATIO: &str = "__mei_metric_title_ratio";
pub(super) const PROP_METRIC_CONTENT_RATIO: &str = "__mei_metric_content_ratio";
pub(super) const PROP_METRIC_V_ALIGN: &str = "metric_v_align";
pub(super) const PROP_METRIC_DESC_MODE: &str = "metric_desc_mode";
pub(super) const PROP_MEI_METRIC_DESC_MODE: &str = "__mei_metric_desc_mode";
pub(super) const PROP_METRIC_DESC_SHELL: &str = "metric_desc_shell";
pub(super) const USE_QUNFU_METRIC_TILE: &str = "cockpit.qunfu-metric-tile";
pub(super) const USE_METRIC_PROGRESS: &str = "cockpit.metric-progress";
pub(super) const USE_MEI_TEXT: &str = "mei.text";
pub(super) const METRIC_SLOT_ROLES: [&str; 4] = ["label", "value", "unit", "desc"];

pub(super) fn metric_prop_str<'a>(card: &'a UiNodeDecl, key: &str) -> Option<&'a str> {
    card.props
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn ratio_fr_track(raw: Option<&str>, fallback: u32) -> String {
    let parsed = raw
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(fallback as f64);
    let normalized = if parsed.fract() == 0.0 {
        format!("{}", parsed as u32)
    } else {
        format!("{parsed}")
    };
    format!("{normalized}fr")
}

fn metric_title_content_row_tracks(card: &UiNodeDecl) -> Vec<String> {
    vec![
        ratio_fr_track(metric_prop_str(card, PROP_METRIC_TITLE_RATIO), 1),
        ratio_fr_track(metric_prop_str(card, PROP_METRIC_CONTENT_RATIO), 1),
    ]
}

pub(super) fn rows_use_fractional_tracks(rows: &[String]) -> bool {
    rows.iter().any(|track| track.contains("fr"))
}

/// 作者已为 stack_desc 写明多行 areas（含 desc），保留行轨，勿用 title/content_ratio 覆盖。
fn stack_desc_layout_is_author_defined(layout: &LayoutDecl) -> bool {
    let Some(areas) = layout.areas.as_ref() else {
        return false;
    };
    let has_desc = areas
        .iter()
        .flat_map(|row| row.iter())
        .any(|cell| cell.trim() == "desc");
    let has_label = areas
        .iter()
        .flat_map(|row| row.iter())
        .any(|cell| cell.trim() == "label");
    let row_count = layout.rows.as_ref().map(|rows| rows.len()).unwrap_or(0);
    has_desc && has_label && row_count >= 3
}

pub(super) fn card_has_background_image(card: &UiNodeDecl) -> bool {
    let Some(background) = card.props.as_object().and_then(|map| map.get("background")) else {
        return false;
    };
    if let Some(image) = background.get("image").and_then(Value::as_str) {
        return image.contains("url(");
    }
    false
}

pub(super) fn normalize_metric_card_vertical_bands(card: &mut UiNodeDecl) {
    let template = metric_prop_str(card, PROP_METRIC_TEMPLATE)
        .unwrap_or("stack")
        .to_string();
    if !matches!(template.as_str(), "stack" | "stack_desc") {
        return;
    }
    let height = panel_px_prop(card, "height").unwrap_or(0.0);
    if height <= 0.0 {
        return;
    }
    let ratio_rows = metric_title_content_row_tracks(card);
    if card.layout.is_none() {
        card.layout = Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["auto".to_string(), "auto".to_string()]),
            rows: None,
            areas: None,
            gap: None,
            padding: None,
            align: None,
            justify: None,
        });
    }
    let layout = card.layout.as_mut().expect("metric card layout");
    layout.align = Some("stretch".to_string());
    if layout
        .justify
        .as_ref()
        .map(|value| value.trim())
        .is_none_or(|value| value.is_empty())
    {
        layout.justify = Some("center".to_string());
    }
    if template == "stack_desc" && stack_desc_layout_is_author_defined(layout) {
        return;
    }
    let rows = layout.rows.get_or_insert_with(|| ratio_rows.clone());
    if template == "stack" {
        if !rows_use_fractional_tracks(rows) {
            *rows = ratio_rows;
        }
        return;
    }
    // stack_desc: 默认 title/content 比 + auto desc 行
    let mut next_rows = ratio_rows;
    next_rows.push("auto".to_string());
    if !rows_use_fractional_tracks(rows) || rows.len() < 3 {
        *rows = next_rows;
    }
}

pub(super) fn slot_vertical_align_prop_key(role: &str) -> String {
    format!("__mei_metric_{role}_v_align")
}

pub(super) fn block_metric_role<'a>(block: &'a BlockDecl) -> Option<&'a str> {
    block
        .props
        .as_object()
        .and_then(|map| map.get("metric_role"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            block
                .area
                .as_deref()
                .map(str::trim)
                .filter(|value| METRIC_SLOT_ROLES.contains(value))
        })
}

pub(super) fn block_metric_v_align(block: &BlockDecl) -> Option<String> {
    block
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_V_ALIGN))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn block_has_metric_v_align(block: &BlockDecl) -> bool {
    block
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_V_ALIGN))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

pub(super) fn apply_metric_slot_vertical_align_from_props(card: &mut UiNodeDecl) {
    let Some(shell_props) = card.props.as_object() else {
        return;
    };
    for node in &mut card.blocks {
        let UiTreeNode::Block(block) = node else {
            continue;
        };
        let Some(role) = block
            .props
            .as_object()
            .and_then(|map| map.get("metric_role"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if block_has_metric_v_align(block) {
            continue;
        }
        let Some(raw) = shell_props
            .get(&slot_vertical_align_prop_key(role))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if !block.props.is_object() {
            block.props = Value::Object(Default::default());
        }
        if let Some(block_props) = block.props.as_object_mut() {
            block_props.insert(
                PROP_METRIC_V_ALIGN.to_string(),
                Value::String(raw.to_string()),
            );
        }
    }
}

pub(super) fn normalize_panel_metric_cards(panel: &mut UiNodeDecl) {
    for block in &mut panel.blocks {
        if !node_is_metric_card_like(block) {
            if let UiTreeNode::Panel(nested) = block {
                normalize_panel_metric_cards(nested);
            }
            continue;
        }
        let UiTreeNode::Panel(card) = block else {
            continue;
        };
        normalize_metric_card_vertical_bands(card);
        apply_metric_slot_vertical_align_from_props(card);
        normalize_panel_metric_cards(card);
    }
}
