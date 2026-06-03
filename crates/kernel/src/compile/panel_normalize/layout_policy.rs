use serde_json::Value;

use crate::model::{LayoutDecl, PanelDecl, UiNodeDecl};

use super::constants::{
    PolicySpacing, DEFAULT_METRICS_2X2_COLUMNS, DEFAULT_METRICS_2X2_GAP,
    DEFAULT_METRICS_2X2_PADDING, DEFAULT_METRICS_2_1_COLUMNS, DEFAULT_METRICS_2_1_GAP,
    DEFAULT_METRICS_2_1_PADDING, DEFAULT_METRICS_AUTO_GAP, DEFAULT_METRICS_AUTO_PADDING,
    DEFAULT_METRIC_COMPOUND_2_1_GAP, LAYOUT_POLICY_METRIC_COMPOUND_2_1, METRIC_COMPOUND_BOTTOM_MAX,
    PROP_COMPOUND_BOTTOM_RATIO, PROP_COMPOUND_TOP_BAND_RATIO, PROP_COMPOUND_TOP_RATIO,
    PROP_LAYOUT_COLUMNS, PROP_LAYOUT_COLUMNS_PREFER, PROP_LAYOUT_SPAN, SLOT_BODY, SLOT_HEAD,
};
use super::css_util::{first_css_scalar_px, parse_px, px_track, sum_fixed_px_tracks};
use super::nodes::panel_head_height_track;
use super::nodes::{
    node_height_track, node_is_metric_card_like, node_is_metrics_2_1_item_like, node_width_track,
    panel_px_prop, set_node_area,
};
use super::spacing::{panel_layout_policy, policy_spacing};

const METRICS_AUTO_MAX_COLUMNS: usize = 6;
const METRICS_AUTO_GAP_EXPAND_MAX: f64 = 36.0;
/// `metric-bg-target@3x` 横向分割线在 viewBox 128 高中约 y=56（无 props 覆写时的默认值）。
const DEFAULT_METRIC_COMPOUND_TOP_BAND_RATIO: f64 = 56.0 / 128.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MetricSpanHint {
    Units(usize),
    Full,
}

#[derive(Clone, Debug)]
struct MetricAutoItem {
    area: String,
    height: Option<f64>,
    width: Option<f64>,
    span_hint: MetricSpanHint,
}

#[derive(Clone, Debug)]
struct MetricAutoRow {
    placements: Vec<(usize, usize)>,
    used_units: usize,
}

#[derive(Clone, Debug)]
struct MetricAutoCandidate {
    columns: usize,
    rows: Vec<MetricAutoRow>,
    score: f64,
}

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

fn value_as_usize(value: &Value) -> Option<usize> {
    if let Some(raw) = value.as_u64() {
        return usize::try_from(raw).ok().filter(|value| *value > 0);
    }
    if let Some(raw) = value.as_f64() {
        let rounded = raw.round();
        if rounded >= 1.0 {
            return usize::try_from(rounded as u64).ok();
        }
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

fn metric_auto_item_span_hint(node: &UiNodeDecl) -> MetricSpanHint {
    let explicit = match node {
        UiNodeDecl::Panel(panel) => panel
            .props
            .as_object()
            .and_then(|map| map.get(PROP_LAYOUT_SPAN))
            .and_then(|value| {
                if value
                    .as_str()
                    .map(str::trim)
                    .is_some_and(|raw| raw.eq_ignore_ascii_case("full"))
                {
                    return Some(MetricSpanHint::Full);
                }
                value_as_usize(value).map(MetricSpanHint::Units)
            }),
        _ => None,
    };
    if let Some(span) = explicit {
        return span;
    }
    match node {
        UiNodeDecl::Panel(panel) => {
            if panel_px_prop(panel, "width").is_some() {
                return MetricSpanHint::Units(1);
            }
            panel_layout_policy(panel)
                .as_deref()
                .and_then(|policy| {
                    if policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1 {
                        Some(MetricSpanHint::Units(2))
                    } else {
                        None
                    }
                })
                .unwrap_or(MetricSpanHint::Units(1))
        }
        _ => MetricSpanHint::Units(1),
    }
}

fn metric_auto_items(panel: &PanelDecl) -> Vec<MetricAutoItem> {
    panel
        .blocks
        .iter()
        .enumerate()
        .map(|(idx, node)| MetricAutoItem {
            area: format!("m{idx}"),
            height: node_height_track(node),
            width: node_width_track(node),
            span_hint: metric_auto_item_span_hint(node),
        })
        .collect()
}

fn metric_auto_columns_prefer(panel: &PanelDecl) -> Option<usize> {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_COLUMNS_PREFER))
        .and_then(value_as_usize)
}

fn metric_auto_total_units(items: &[MetricAutoItem]) -> usize {
    items
        .iter()
        .map(|item| match item.span_hint {
            MetricSpanHint::Units(value) => value.clamp(1, METRICS_AUTO_MAX_COLUMNS),
            MetricSpanHint::Full => 1,
        })
        .sum::<usize>()
        .max(1)
}

fn pack_metric_auto_rows(items: &[MetricAutoItem], columns: usize) -> Vec<MetricAutoRow> {
    let mut rows = Vec::new();
    let mut current = Vec::new();
    let mut used_units = 0usize;
    for (idx, item) in items.iter().enumerate() {
        let span = match item.span_hint {
            MetricSpanHint::Units(value) => value.clamp(1, columns),
            MetricSpanHint::Full => columns,
        };
        if span >= columns {
            if !current.is_empty() {
                rows.push(MetricAutoRow {
                    placements: current,
                    used_units,
                });
                current = Vec::new();
                used_units = 0;
            }
            rows.push(MetricAutoRow {
                placements: vec![(idx, columns)],
                used_units: columns,
            });
            continue;
        }
        if used_units + span > columns && !current.is_empty() {
            rows.push(MetricAutoRow {
                placements: current,
                used_units,
            });
            current = Vec::new();
            used_units = 0;
        }
        current.push((idx, span));
        used_units += span;
        if used_units == columns {
            rows.push(MetricAutoRow {
                placements: current,
                used_units,
            });
            current = Vec::new();
            used_units = 0;
        }
    }
    if !current.is_empty() {
        rows.push(MetricAutoRow {
            placements: current,
            used_units,
        });
    }
    rows
}

fn metric_auto_ideal_rows(items: &[MetricAutoItem]) -> usize {
    if items.len() <= 3 {
        return 1;
    }
    let base = (items.len() as f64).sqrt().round() as usize;
    base.max(1)
}

fn metric_auto_candidate_score(
    panel: &PanelDecl,
    items: &[MetricAutoItem],
    columns: usize,
    rows: &[MetricAutoRow],
) -> f64 {
    let ideal_rows = metric_auto_ideal_rows(items);
    let target_items_per_row = items.len().div_ceil(ideal_rows);
    let mut score = ((rows.len() as isize - ideal_rows as isize).abs() as f64) * 28.0;
    if rows.len() > 2 {
        score += (rows.len().saturating_sub(2) as f64) * 24.0;
    }
    if let Some(prefer) = metric_auto_columns_prefer(panel) {
        score += ((columns as isize - prefer as isize).abs() as f64) * 60.0;
    }
    for row in rows {
        let item_count = row.placements.len();
        let leftover = columns.saturating_sub(row.used_units);
        let effective_leftover = if item_count == 1 { 0 } else { leftover };
        let left_pad = effective_leftover / 2;
        let right_pad = effective_leftover - left_pad;
        score += ((item_count as isize - target_items_per_row as isize).abs() as f64) * 14.0;
        score += (effective_leftover * effective_leftover) as f64 * 9.0;
        score += ((left_pad as isize - right_pad as isize).abs() as f64) * 4.0;
        if item_count == 1 && row.used_units < columns {
            score += 6.0;
        }
        if item_count == 1 && row.used_units == columns {
            score += 6.0;
        }
        let all_narrow = row.placements.iter().all(|(_, span)| *span == 1);
        if all_narrow && item_count > 3 {
            let honors_prefer =
                metric_auto_columns_prefer(panel).is_some_and(|prefer| prefer == columns);
            if !honors_prefer {
                score += (item_count.saturating_sub(3) as f64) * 40.0;
            }
        }
        for (idx, span) in &row.placements {
            match items[*idx].span_hint {
                MetricSpanHint::Full if !(item_count == 1 && *span == columns) => {
                    score += 160.0;
                }
                MetricSpanHint::Units(expected) if *span != expected.min(columns) => {
                    score += 60.0;
                }
                _ => {}
            }
        }
    }
    score
}

fn metric_auto_area_row(
    items: &[MetricAutoItem],
    row: &MetricAutoRow,
    columns: usize,
) -> Vec<String> {
    if row.placements.len() == 1 && columns > 1 {
        return vec![items[row.placements[0].0].area.clone(); columns];
    }
    let leftover = columns.saturating_sub(row.used_units);
    let left_pad = leftover / 2;
    let right_pad = leftover - left_pad;
    let mut cells = Vec::with_capacity(columns);
    for _ in 0..left_pad {
        cells.push(".".to_string());
    }
    for (idx, span) in &row.placements {
        for _ in 0..*span {
            cells.push(items[*idx].area.clone());
        }
    }
    for _ in 0..right_pad {
        cells.push(".".to_string());
    }
    cells
}

fn metric_auto_column_tracks(
    items: &[MetricAutoItem],
    rows: &[MetricAutoRow],
    columns: usize,
) -> Option<Vec<String>> {
    let mut tracks: Vec<f64> = vec![0.0; columns];
    let mut has_fixed = false;
    for row in rows {
        if row.placements.len() == 1 && columns > 1 {
            let (idx, _) = row.placements[0];
            let width = items[idx].width?;
            has_fixed = true;
            let per_track = width / columns as f64;
            for track in &mut tracks {
                *track = f64::max(*track, per_track);
            }
            continue;
        }
        let leftover = columns.saturating_sub(row.used_units);
        let left_pad = leftover / 2;
        let mut col = left_pad;
        for (idx, span) in &row.placements {
            let width = match items[*idx].width {
                Some(value) => value.max(0.0),
                None => {
                    col += *span;
                    continue;
                }
            };
            has_fixed = true;
            let per_track = width / (*span as f64).max(1.0);
            for track in tracks.iter_mut().skip(col).take(*span) {
                *track = f64::max(*track, per_track);
            }
            col += *span;
        }
    }
    if !has_fixed || tracks.iter().any(|value| *value <= 0.0) {
        return None;
    }
    Some(tracks.into_iter().map(px_track).collect())
}

fn metric_auto_padding_tb(padding: &str) -> (f64, f64) {
    let tokens: Vec<&str> = padding.split_whitespace().collect();
    if tokens.is_empty() {
        return (0.0, 0.0);
    }
    let top = parse_px(tokens[0]).unwrap_or(0.0);
    let bottom = if tokens.len() >= 3 {
        parse_px(tokens[2]).unwrap_or(top)
    } else {
        top
    };
    (top, bottom)
}

fn metric_auto_tuned_spacing(
    panel: &PanelDecl,
    columns: usize,
    column_tracks: &[String],
    spacing: &PolicySpacing,
) -> PolicySpacing {
    let Some(panel_width) = panel_px_prop(panel, "width") else {
        return spacing.clone();
    };
    let Some(column_budget) = sum_fixed_px_tracks(column_tracks) else {
        return spacing.clone();
    };
    if panel_width <= column_budget || columns == 0 {
        return spacing.clone();
    }
    let (padding_top, padding_bottom) = metric_auto_padding_tb(&spacing.padding);
    let vertical_gap = first_css_scalar_px(&spacing.gap).unwrap_or(0.0);
    let target_gap = ((panel_width - column_budget) / (columns as f64 + 3.0))
        .clamp(4.0, METRICS_AUTO_GAP_EXPAND_MAX);
    let target_pad = (target_gap * 2.0).max(8.0);
    PolicySpacing {
        gap: format!("{} {}", px_track(vertical_gap), px_track(target_gap)),
        padding: format!(
            "{} {} {} {}",
            px_track(padding_top),
            px_track(target_pad),
            px_track(padding_bottom),
            px_track(target_pad)
        ),
    }
}

fn metric_auto_row_track(items: &[MetricAutoItem], row: &MetricAutoRow) -> String {
    row.placements
        .iter()
        .filter_map(|(idx, _)| items[*idx].height)
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(existing) => Some(existing.max(value)),
            None => Some(value),
        })
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string())
}

pub(super) fn default_metrics_auto_layout(panel: &PanelDecl) -> LayoutDecl {
    let items = metric_auto_items(panel);
    let min_columns = items
        .iter()
        .filter_map(|item| match item.span_hint {
            MetricSpanHint::Units(value) => Some(value.clamp(1, METRICS_AUTO_MAX_COLUMNS)),
            MetricSpanHint::Full => None,
        })
        .max()
        .unwrap_or(1)
        .max(2);
    let prefer_columns = metric_auto_columns_prefer(panel)
        .map(|value| value.clamp(min_columns, METRICS_AUTO_MAX_COLUMNS))
        .unwrap_or(min_columns);
    let max_columns = metric_auto_total_units(&items)
        .min(METRICS_AUTO_MAX_COLUMNS)
        .max(min_columns)
        .max(prefer_columns);
    let mut candidates = Vec::new();
    for columns in min_columns..=max_columns {
        let rows = pack_metric_auto_rows(&items, columns);
        let score = metric_auto_candidate_score(panel, &items, columns, &rows);
        candidates.push(MetricAutoCandidate {
            columns,
            rows,
            score,
        });
    }
    let best = candidates
        .into_iter()
        .min_by(|left, right| left.score.total_cmp(&right.score))
        .unwrap_or(MetricAutoCandidate {
            columns: items.len().max(1),
            rows: pack_metric_auto_rows(&items, items.len().max(1)),
            score: 0.0,
        });
    let base_spacing = policy_spacing(
        panel,
        DEFAULT_METRICS_AUTO_GAP,
        DEFAULT_METRICS_AUTO_PADDING,
    );
    let columns = metric_auto_column_tracks(&items, &best.rows, best.columns)
        .unwrap_or_else(|| vec!["1fr".to_string(); best.columns]);
    let spacing = metric_auto_tuned_spacing(panel, best.columns, &columns, &base_spacing);
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(columns),
        rows: Some(
            best.rows
                .iter()
                .map(|row| metric_auto_row_track(&items, row))
                .collect(),
        ),
        areas: Some(
            best.rows
                .iter()
                .map(|row| metric_auto_area_row(&items, row, best.columns))
                .collect(),
        ),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
        justify: Some("center".to_string()),
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

pub(super) fn metric_compound_bottom_count(panel: &PanelDecl) -> usize {
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

fn metric_compound_top_band_fraction(panel: &PanelDecl) -> f64 {
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
fn metric_compound_band_fr_weights(panel: &PanelDecl) -> (u32, u32) {
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

fn metric_compound_row_fr_tracks(panel: &PanelDecl) -> (String, String) {
    let (top_w, bottom_w) = metric_compound_band_fr_weights(panel);
    (format!("{top_w}fr"), format!("{bottom_w}fr"))
}

pub(super) fn default_metric_compound_2_1_layout(panel: &PanelDecl) -> LayoutDecl {
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

pub(super) fn should_inject_metrics_auto(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() < 2 {
        return false;
    }
    panel.blocks.iter().all(node_is_metrics_2_1_item_like)
}

pub(super) fn should_inject_metrics_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 3 {
        return false;
    }
    panel.blocks.iter().all(node_is_metrics_2_1_item_like)
}

pub(super) fn should_inject_metric_compound_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head {
        return false;
    }
    let bottom = metric_compound_bottom_count(panel);
    if bottom < 1 || bottom > METRIC_COMPOUND_BOTTOM_MAX {
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

pub(super) fn inject_default_metrics_auto_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_auto_layout(panel));
}

pub(super) fn inject_default_metrics_2_1_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_2_1_layout(panel));
}

pub(super) fn inject_default_metric_compound_2_1_layout(panel: &mut PanelDecl) {
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
