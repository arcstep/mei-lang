use serde_json::Value;

use crate::model::{LayoutDecl, UiNodeDecl};

use super::super::constants::{
    DEFAULT_METRICS_2X2_COLUMNS, DEFAULT_METRICS_2X2_GAP, DEFAULT_METRICS_2X2_PADDING,
    DEFAULT_METRICS_2_1_COLUMNS, DEFAULT_METRICS_2_1_GAP, DEFAULT_METRICS_2_1_PADDING,
    DEFAULT_METRIC_COMPOUND_2_1_GAP, PROP_COMPOUND_BOTTOM_RATIO, PROP_COMPOUND_TOP_BAND_RATIO,
    PROP_COMPOUND_TOP_RATIO, PROP_LAYOUT_COLUMNS,
};
use super::super::css_util::{parse_px, px_track};
use super::super::nodes::node_height_track;
use super::super::spacing::policy_spacing;

/// `metric-bg-target@3x` 横向分割线在 viewBox 128 高中约 y=56（无 props 覆写时的默认值）。
const DEFAULT_METRIC_COMPOUND_TOP_BAND_RATIO: f64 = 56.0 / 128.0;

pub(super) fn default_metrics_2x2_layout(panel: &UiNodeDecl) -> LayoutDecl {
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

pub(super) fn default_metrics_2_1_layout(panel: &UiNodeDecl) -> LayoutDecl {
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

pub(super) fn metric_compound_bottom_count(panel: &UiNodeDecl) -> usize {
    panel.blocks.len().saturating_sub(1)
}

fn parse_compound_band_fraction(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(stripped) = value.strip_suffix('%') {
        let pct = parse_px(stripped)?;
        return Some((pct / 100.0).clamp(0.05, 0.95));
    }
    if value.contains('/') {
        let mut parts = value.split('/');
        let top = parse_px(parts.next()?.trim())?;
        let bottom = parse_px(parts.next()?.trim())?;
        let denom = top + bottom;
        if denom > 0.0 {
            return Some((top / denom).clamp(0.05, 0.95));
        }
        return None;
    }
    if let Ok(scalar) = value.parse::<f64>() {
        if scalar > 1.0 {
            // 形如 "56"：按 128px 设计稿壳高归一化，便于与 SVG 标注对齐。
            return Some((scalar / 128.0).clamp(0.05, 0.95));
        }
        return Some(scalar.clamp(0.05, 0.95));
    }
    parse_px(value).map(|scalar| {
        if scalar > 1.0 {
            (scalar / 128.0).clamp(0.05, 0.95)
        } else {
            scalar.clamp(0.05, 0.95)
        }
    })
}

fn metric_compound_top_band_fraction(panel: &UiNodeDecl) -> f64 {
    let map = panel.props.as_object();
    if let Some(map) = map {
        if let Some(raw) = map
            .get(PROP_COMPOUND_TOP_BAND_RATIO)
            .and_then(Value::as_str)
        {
            if let Some(fraction) = parse_compound_band_fraction(raw) {
                return fraction;
            }
        }
        let top_weight = map
            .get(PROP_COMPOUND_TOP_RATIO)
            .and_then(metric_ratio_weight);
        let bottom_weight = map
            .get(PROP_COMPOUND_BOTTOM_RATIO)
            .and_then(metric_ratio_weight);
        if let (Some(top), Some(bottom)) = (top_weight, bottom_weight) {
            let denom = top + bottom;
            if denom > 0.0 {
                return (top / denom).clamp(0.05, 0.95);
            }
        }
    }
    DEFAULT_METRIC_COMPOUND_TOP_BAND_RATIO
}

fn metric_ratio_weight(value: &Value) -> Option<f64> {
    if let Some(raw) = value.as_str() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        return parse_px(trimmed).filter(|n| *n > 0.0);
    }
    value
        .as_f64()
        .filter(|n| *n > 0.0)
        .or_else(|| value.as_i64().filter(|n| *n > 0).map(|n| n as f64))
}

/// `32/99` → `(32, 99)fr`；百分比/小数 → 按 band 比例换算为 fr 权重。
fn metric_compound_band_fr_weights(panel: &UiNodeDecl) -> (u32, u32) {
    if let Some(map) = panel.props.as_object() {
        if let Some(raw) = map
            .get(PROP_COMPOUND_TOP_BAND_RATIO)
            .and_then(Value::as_str)
        {
            let value = raw.trim();
            if value.contains('/') {
                let mut parts = value.split('/');
                if let (Some(top), Some(bottom)) = (
                    parts.next().and_then(|part| parse_px(part.trim())),
                    parts.next().and_then(|part| parse_px(part.trim())),
                ) {
                    if top > 0.0 && bottom > 0.0 {
                        return (top.round().max(1.0) as u32, bottom.round().max(1.0) as u32);
                    }
                }
            }
        }
    }
    let top_band = metric_compound_top_band_fraction(panel);
    let top = (top_band * 256.0).round().max(1.0) as u32;
    let bottom = ((1.0 - top_band) * 256.0).round().max(1.0) as u32;
    (top, bottom)
}

fn metric_compound_row_fr_tracks(panel: &UiNodeDecl) -> (String, String) {
    let (top_w, bottom_w) = metric_compound_band_fr_weights(panel);
    (format!("{top_w}fr"), format!("{bottom_w}fr"))
}

pub(super) fn default_metric_compound_2_1_layout(panel: &UiNodeDecl) -> LayoutDecl {
    let spacing = policy_spacing(panel, DEFAULT_METRIC_COMPOUND_2_1_GAP, "0");
    let bottom_cols = metric_compound_bottom_count(panel).max(1);
    let (top_row, bottom_row) = metric_compound_row_fr_tracks(panel);
    let columns = (0..bottom_cols)
        .map(|_| "1fr".to_string())
        .collect::<Vec<_>>();
    let top_areas = vec!["top".to_string(); bottom_cols];
    let bottom_areas = (0..bottom_cols)
        .map(|idx| format!("b{idx}"))
        .collect::<Vec<_>>();
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(columns),
        rows: Some(vec![top_row, bottom_row]),
        areas: Some(vec![top_areas, bottom_areas]),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
        justify: None,
    }
}
