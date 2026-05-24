use serde_json::Value;

use crate::model::{Diagnostic, LayoutDecl, PanelDecl, Severity, UiNodeDecl};

use super::constants::PROP_METRIC_CARD;
use super::nodes::{node_is_metric_card_like, panel_px_prop};

const PROP_METRIC_TEMPLATE: &str = "__mei_metric_template";
const PROP_METRIC_TITLE_RATIO: &str = "__mei_metric_title_ratio";
const PROP_METRIC_CONTENT_RATIO: &str = "__mei_metric_content_ratio";
const PROP_METRIC_V_ALIGN: &str = "metric_v_align";

fn metric_prop_str<'a>(card: &'a PanelDecl, key: &str) -> Option<&'a str> {
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

fn metric_title_content_row_tracks(card: &PanelDecl) -> Vec<String> {
    vec![
        ratio_fr_track(metric_prop_str(card, PROP_METRIC_TITLE_RATIO), 1),
        ratio_fr_track(metric_prop_str(card, PROP_METRIC_CONTENT_RATIO), 1),
    ]
}

fn rows_use_fractional_tracks(rows: &[String]) -> bool {
    rows.iter().any(|track| track.contains("fr"))
}

fn card_has_background_image(card: &PanelDecl) -> bool {
    let Some(background) = card.props.as_object().and_then(|map| map.get("background")) else {
        return false;
    };
    if let Some(image) = background.get("image").and_then(Value::as_str) {
        return image.contains("url(");
    }
    false
}

pub(super) fn normalize_metric_card_vertical_bands(card: &mut PanelDecl) {
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
    layout.justify = Some("center".to_string());
    let rows = layout.rows.get_or_insert_with(|| ratio_rows.clone());
    if template == "stack" {
        if !rows_use_fractional_tracks(rows) {
            *rows = ratio_rows;
        }
        return;
    }
    // stack_desc: title/content bands + auto desc row
    let mut next_rows = ratio_rows;
    next_rows.push("auto".to_string());
    if !rows_use_fractional_tracks(rows) || rows.len() < 3 {
        *rows = next_rows;
    }
}

fn slot_vertical_align_prop_key(role: &str) -> String {
    format!("__mei_metric_{role}_v_align")
}

const METRIC_SLOT_ROLES: [&str; 4] = ["label", "value", "unit", "desc"];

fn overlay_props_has_slot_v_align(overlay_value: &Value, role: &str) -> bool {
    overlay_value
        .get("props")
        .and_then(Value::as_object)
        .and_then(|map| map.get(&slot_vertical_align_prop_key(role)))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn metric_v_align_from_base_block(base: &PanelDecl, role: &str) -> Option<String> {
    for node in &base.blocks {
        let UiNodeDecl::Block(block) = node else {
            continue;
        };
        let Some(block_role) = block
            .props
            .as_object()
            .and_then(|map| map.get("metric_role"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if block_role != role {
            continue;
        }
        let raw = block
            .props
            .as_object()
            .and_then(|map| map.get(PROP_METRIC_V_ALIGN))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        return Some(raw.to_string());
    }
    None
}

/// 将模板 panel 上 `label(..., vertical_align=...)` 等槽位默认值写入 shell props（仅当调用方未显式覆写）。
pub(crate) fn seed_metric_slot_vertical_align_defaults_from_base(
    base: &PanelDecl,
    merged: &mut PanelDecl,
    overlay_value: &Value,
) {
    if !merged
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_CARD))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return;
    }
    let Some(merged_props) = merged.props.as_object_mut() else {
        return;
    };
    for role in METRIC_SLOT_ROLES {
        if overlay_props_has_slot_v_align(overlay_value, role) {
            continue;
        }
        let key = slot_vertical_align_prop_key(role);
        // 模板 blocks 上的 vertical_align 优先于 props.__mei_metric_*（作者按槽位微调）。
        if let Some(raw) = metric_v_align_from_base_block(base, role) {
            merged_props.insert(key, Value::String(raw));
        }
    }
}

fn block_has_metric_v_align(block: &crate::BlockDecl) -> bool {
    block
        .props
        .as_object()
        .and_then(|map| map.get(PROP_METRIC_V_ALIGN))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

pub(super) fn apply_metric_slot_vertical_align_from_props(card: &mut PanelDecl) {
    let Some(shell_props) = card.props.as_object() else {
        return;
    };
    for node in &mut card.blocks {
        let UiNodeDecl::Block(block) = node else {
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

pub(super) fn normalize_panel_metric_cards(panel: &mut PanelDecl) {
    for block in &mut panel.blocks {
        if !node_is_metric_card_like(block) {
            if let UiNodeDecl::Panel(nested) = block {
                normalize_panel_metric_cards(nested);
            }
            continue;
        }
        let UiNodeDecl::Panel(card) = block else {
            continue;
        };
        normalize_metric_card_vertical_bands(card);
        apply_metric_slot_vertical_align_from_props(card);
        normalize_panel_metric_cards(card);
    }
}

fn audit_one_metric_card_vertical_bands(
    card: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let template = metric_prop_str(card, PROP_METRIC_TEMPLATE).unwrap_or("stack");
    if !matches!(template, "stack" | "stack_desc") {
        return;
    }
    let height = panel_px_prop(card, "height").unwrap_or(0.0);
    if height <= 0.0 {
        return;
    }
    let layout = card.layout.as_ref();
    let rows = layout.and_then(|value| value.rows.as_deref()).unwrap_or_default();
    if !rows_use_fractional_tracks(rows) {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_eval_metric_vertical_band_risk".to_string(),
            message: format!(
                "metric_card `{}`: fixed-height stack should use fractional row tracks (e.g. title_ratio/content_ratio → 1fr 1fr) so label/value land in upper/lower bands",
                card.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }
    if layout
        .and_then(|value| value.align.as_deref())
        .is_some_and(|value| value.eq_ignore_ascii_case("end"))
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_eval_metric_vertical_align_risk".to_string(),
            message: format!(
                "metric_card `{}`: stack layout should use `align = stretch` so slots center within each vertical band (not `end`, which pushes label into the lower half)",
                card.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }
    if card_has_background_image(card) && height >= 96.0 {
        let title_ratio = metric_prop_str(card, PROP_METRIC_TITLE_RATIO).unwrap_or("1");
        let content_ratio = metric_prop_str(card, PROP_METRIC_CONTENT_RATIO).unwrap_or("1");
        if title_ratio == "1" && content_ratio == "1" {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_metric_background_band_hint".to_string(),
                message: format!(
                    "metric_card `{}`: background art may expect non-default vertical bands; consider title_ratio/content_ratio (e.g. 3 and 7 for ~30%/70%)",
                    card.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_metric_vertical_bands(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for node in &panel.blocks {
        let UiNodeDecl::Panel(card) = node else {
            continue;
        };
        if node_is_metric_card_like(node) {
            audit_one_metric_card_vertical_bands(card, diagnostics, source_path);
        }
        audit_metric_vertical_bands(card, diagnostics, source_path);
    }
}
