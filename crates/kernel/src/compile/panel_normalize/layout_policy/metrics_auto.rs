use serde_json::Value;

use crate::model::{LayoutDecl, PanelDecl, UiNodeDecl};

use super::super::constants::{
    PolicySpacing, DEFAULT_METRICS_AUTO_GAP, DEFAULT_METRICS_AUTO_PADDING,
    LAYOUT_POLICY_METRIC_COMPOUND_2_1, PROP_LAYOUT_COLUMNS_PREFER, PROP_LAYOUT_SPAN,
};
use super::super::css_util::{first_css_scalar_px, parse_px, px_track, sum_fixed_px_tracks};
use super::super::nodes::{node_height_track, node_width_track, panel_px_prop};
use super::super::spacing::{panel_layout_policy, policy_spacing};

const METRICS_AUTO_MAX_COLUMNS: usize = 6;
const METRICS_AUTO_GAP_EXPAND_MAX: f64 = 36.0;

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
