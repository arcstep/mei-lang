use std::collections::BTreeSet;

use serde_json::Value;

use crate::model::{Diagnostic, LayoutDecl, PanelDecl, Severity, UiNodeDecl};

use super::constants::{
    COCKPIT_CARD_GAP_MAX, COCKPIT_CARD_GAP_MIN, COCKPIT_CARD_GAP_TARGET, COCKPIT_PANEL_PADDING_MAX,
    COCKPIT_PANEL_PADDING_MIN, LAYOUT_POLICY_METRICS_2X2, LAYOUT_POLICY_METRICS_2_1,
    LAYOUT_POLICY_METRICS_AUTO, LAYOUT_POLICY_METRICS_STRIP, LAYOUT_POLICY_METRIC_COMPOUND_2_1,
    SLOT_BODY, SLOT_HEAD,
};
use super::css_util::{
    css_scalar_numbers, first_css_scalar_px, is_degenerate_track, layout_gap_y_px,
    layout_padding_horizontal_px, layout_padding_vertical_px, padding_horizontal_px, parse_px,
    sum_fixed_px_tracks,
};
use super::layout_policy::layout_has_slot;
use super::nodes::panel_head_height_track;
use super::nodes::{
    node_area, node_has_explicit_area, node_height_track, node_is_metric_card_like, panel_px_prop,
};
use super::spacing::panel_layout_policy;

const LAYOUT_EVAL_PREFIX: &str = "layout_eval_";
const METRICS_AUTO_EXPANDED_GAP_MAX: f64 = 36.0;

fn eval_weight(severity: &Severity) -> u32 {
    match severity {
        Severity::Error => 100,
        Severity::Warning => 40,
        Severity::Info => 10,
    }
}

fn is_layout_eval_diag(diag: &Diagnostic) -> bool {
    diag.code.starts_with(LAYOUT_EVAL_PREFIX)
}

fn emit_panel_eval_summary(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
    start_idx: usize,
) {
    let eval_diags: Vec<&Diagnostic> = diagnostics[start_idx..]
        .iter()
        .filter(|diag| is_layout_eval_diag(diag))
        .collect();
    if eval_diags.is_empty() {
        return;
    }
    let score: u32 = eval_diags
        .iter()
        .map(|diag| eval_weight(&diag.severity))
        .sum();
    let blocking = eval_diags
        .iter()
        .any(|diag| matches!(diag.severity, Severity::Error));
    let findings_count = eval_diags.len();
    if !blocking && score < 80 {
        return;
    }
    let worst_codes = eval_diags
        .iter()
        .take(3)
        .map(|diag| diag.code.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    diagnostics.push(Diagnostic {
        severity: if blocking {
            Severity::Error
        } else {
            Severity::Warning
        },
        code: "layout_eval_panel_summary".to_string(),
        message: format!(
            "panel `{}`: layout eval {} (score={}, findings={}){}",
            panel.id,
            if blocking { "blocking" } else { "warning" },
            score,
            findings_count,
            if worst_codes.is_empty() {
                String::new()
            } else {
                format!("; worst={worst_codes}")
            }
        ),
        source_path: Some(source_path.to_string()),
    });
}

pub(super) fn emit_layout_audit_diagnostics(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let start_idx = diagnostics.len();
    let Some(layout) = panel.layout.as_ref() else {
        return;
    };
    audit_layout_matrix(panel, layout, diagnostics, source_path);
    audit_layout_area_mapping(panel, layout, diagnostics, source_path);
    audit_layout_spacing(layout, panel, diagnostics, source_path);
    audit_fixed_track_budget(panel, layout, diagnostics, source_path);
    audit_head_body_balance(panel, layout, diagnostics, source_path);
    audit_policy_spacing_budget(panel, layout, diagnostics, source_path);
    audit_panel_whitespace_budget(panel, layout, diagnostics, source_path);
    audit_metric_group_balance(panel, layout, diagnostics, source_path);
    audit_metric_card_internal_budget(panel, diagnostics, source_path);
    audit_strategy_bypass_risk(panel, layout, diagnostics, source_path);
    emit_panel_eval_summary(panel, diagnostics, source_path, start_idx);
}

fn is_metric_layout_policy(policy: &str) -> bool {
    policy == LAYOUT_POLICY_METRICS_AUTO
        || policy == LAYOUT_POLICY_METRICS_STRIP
        || policy == LAYOUT_POLICY_METRICS_2X2
        || policy == LAYOUT_POLICY_METRICS_2_1
        || policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1
}

pub(super) fn audit_layout_matrix(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(areas) = layout.areas.as_ref() else {
        if panel.blocks.iter().any(node_has_explicit_area) {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_missing_areas".to_string(),
                message: format!(
                    "panel `{}`: blocks declare explicit area but layout.areas is missing",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
        return;
    };
    if areas.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_eval_empty_areas".to_string(),
            message: format!("panel `{}`: layout.areas is empty", panel.id),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let width = areas.first().map(Vec::len).unwrap_or(0);
    if width == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_eval_empty_area_row".to_string(),
            message: format!("panel `{}`: first areas row has zero columns", panel.id),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    for (row_idx, row) in areas.iter().enumerate() {
        if row.len() != width {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_irregular_area_matrix".to_string(),
                message: format!(
                    "panel `{}`: areas row {} has {} columns, expected {}",
                    panel.id,
                    row_idx + 1,
                    row.len(),
                    width
                ),
                source_path: Some(source_path.to_string()),
            });
            break;
        }
    }
    if let Some(columns) = layout.columns.as_ref() {
        if !columns.is_empty() && columns.len() != width {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_columns_area_mismatch".to_string(),
                message: format!(
                    "panel `{}`: columns count ({}) differs from area columns ({width})",
                    panel.id,
                    columns.len()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(rows) = layout.rows.as_ref() {
        if !rows.is_empty() && rows.len() != areas.len() {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_rows_area_mismatch".to_string(),
                message: format!(
                    "panel `{}`: rows count ({}) differs from area rows ({})",
                    panel.id,
                    rows.len(),
                    areas.len()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_layout_area_mapping(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(area_rows) = layout.areas.as_ref() else {
        return;
    };
    let mut declared = BTreeSet::new();
    for row in area_rows {
        for cell in row {
            let cell = cell.trim();
            if cell.is_empty() || cell == "." {
                continue;
            }
            declared.insert(cell.to_string());
        }
    }
    if declared.is_empty() {
        return;
    }
    for node in &panel.blocks {
        let Some(area) = node_area(node) else {
            continue;
        };
        let area = area.trim();
        if area.is_empty() || area.eq_ignore_ascii_case("auto") {
            continue;
        }
        if !declared.contains(area) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_unknown_block_area".to_string(),
                message: format!(
                    "panel `{}`: block area `{area}` not declared in layout.areas",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_layout_spacing(
    layout: &LayoutDecl,
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if let Some(gap) = layout.gap.as_deref() {
        let numbers = css_scalar_numbers(gap);
        if numbers.iter().any(|value| *value < 0.0) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_negative_gap".to_string(),
                message: format!(
                    "panel `{}`: layout.gap has negative value `{gap}`",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(padding) = layout.padding.as_deref() {
        let numbers = css_scalar_numbers(padding);
        if numbers.iter().any(|value| *value < 0.0) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_negative_padding".to_string(),
                message: format!(
                    "panel `{}`: layout.padding has negative value `{padding}`",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(rows) = layout.rows.as_ref() {
        if rows.iter().any(|row| is_degenerate_track(row)) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_degenerate_rows".to_string(),
                message: format!(
                    "panel `{}`: layout.rows contains zero-sized track",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(columns) = layout.columns.as_ref() {
        if columns.iter().any(|col| is_degenerate_track(col)) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_degenerate_columns".to_string(),
                message: format!(
                    "panel `{}`: layout.columns contains zero-sized track",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_fixed_track_budget(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let panel_width = panel_px_prop(panel, "width");
    let panel_height = panel_px_prop(panel, "height");
    let row_budget = layout
        .rows
        .as_ref()
        .and_then(|rows| sum_fixed_px_tracks(rows));
    let col_budget = layout
        .columns
        .as_ref()
        .and_then(|columns| sum_fixed_px_tracks(columns));
    if let (Some(height), Some(rows_px)) = (panel_height, row_budget) {
        if rows_px > height + 1.0 {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "layout_eval_row_budget_overflow".to_string(),
                message: format!(
                    "panel `{}`: fixed rows {}px exceed panel height {}px",
                    panel.id,
                    rows_px.round(),
                    height.round()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let (Some(width), Some(cols_px)) = (panel_width, col_budget) {
        if cols_px > width + 1.0 {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "layout_eval_column_budget_overflow".to_string(),
                message: format!(
                    "panel `{}`: fixed columns {}px exceed panel width {}px",
                    panel.id,
                    cols_px.round(),
                    width.round()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_head_body_balance(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if !layout_has_slot(Some(layout), SLOT_HEAD) || !layout_has_slot(Some(layout), SLOT_BODY) {
        return;
    }
    let Some(panel_height) = panel_px_prop(panel, "height") else {
        return;
    };
    let Some(head_height) = panel_head_height_track(panel).and_then(|value| parse_px(&value))
    else {
        return;
    };
    if panel_height <= head_height + 1.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "layout_eval_head_body_height_conflict".to_string(),
            message: format!(
                "panel `{}`: panel height {}px is not enough for head height {}px",
                panel.id,
                panel_height.round(),
                head_height.round()
            ),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let available_body = panel_height - head_height - layout_gap_y_px(layout);
    let Some(required_body) = estimate_body_required_height(panel) else {
        return;
    };
    if required_body > available_body + 1.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "layout_eval_body_clip_risk".to_string(),
            message: format!(
                "panel `{}`: body available {}px is smaller than inferred content {}px (may clip)",
                panel.id,
                available_body.round(),
                required_body.round()
            ),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let slack = available_body - required_body;
    if slack > 24.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            code: "layout_eval_body_spacing_loose".to_string(),
            message: format!(
                "panel `{}`: body has {}px extra slack over inferred content (may look too loose)",
                panel.id,
                slack.round()
            ),
            source_path: Some(source_path.to_string()),
        });
    }
}

pub(super) fn audit_policy_spacing_budget(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(policy) = panel_layout_policy(panel) else {
        return;
    };
    if !is_metric_layout_policy(&policy) {
        return;
    }
    let fixed_width_auto = policy == LAYOUT_POLICY_METRICS_AUTO
        && layout
            .columns
            .as_ref()
            .and_then(|tracks| sum_fixed_px_tracks(tracks))
            .is_some();
    let gap_budget_max = if fixed_width_auto {
        METRICS_AUTO_EXPANDED_GAP_MAX
    } else {
        COCKPIT_CARD_GAP_MAX
    };
    if let Some(gap) = layout.gap.as_deref().and_then(first_css_scalar_px) {
        if gap < COCKPIT_CARD_GAP_MIN - 0.1 || gap > gap_budget_max + 0.1 {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_card_gap_out_of_budget".to_string(),
                message: format!(
                    "panel `{}`: card gap {}px is outside cockpit budget [{}, {}]px",
                    panel.id,
                    gap.round(),
                    COCKPIT_CARD_GAP_MIN,
                    gap_budget_max
                ),
                source_path: Some(source_path.to_string()),
            });
        } else if !fixed_width_auto && (gap - COCKPIT_CARD_GAP_TARGET).abs() > 3.0 {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_card_gap_off_target".to_string(),
                message: format!(
                    "panel `{}`: card gap {}px deviates from cockpit target {}px",
                    panel.id,
                    gap.round(),
                    COCKPIT_CARD_GAP_TARGET
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(padding) = layout.padding.as_deref() {
        let values = css_scalar_numbers(padding);
        let padding_budget_max = if fixed_width_auto {
            METRICS_AUTO_EXPANDED_GAP_MAX * 2.0 + 4.0
        } else {
            COCKPIT_PANEL_PADDING_MAX
        };
        let too_small = values
            .iter()
            .any(|value| *value > 0.0 && *value < COCKPIT_PANEL_PADDING_MIN - 0.1);
        let too_large = values
            .iter()
            .any(|value| *value > padding_budget_max + 0.1);
        if too_small || too_large {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_panel_padding_out_of_budget".to_string(),
                message: format!(
                    "panel `{}`: layout padding `{padding}` is outside cockpit budget {}-{}px",
                    panel.id, COCKPIT_PANEL_PADDING_MIN, padding_budget_max
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_metric_group_balance(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(policy) = panel_layout_policy(panel) else {
        return;
    };
    if !is_metric_layout_policy(&policy) {
        return;
    }
    let Some(areas) = layout.areas.as_ref() else {
        return;
    };
    let Some(column_count) = areas.first().map(Vec::len) else {
        return;
    };
    if column_count < 2 {
        return;
    }
    for row in areas {
        let first = row
            .iter()
            .position(|cell| !cell.trim().is_empty() && cell.trim() != ".");
        let last = row
            .iter()
            .rposition(|cell| !cell.trim().is_empty() && cell.trim() != ".");
        let (Some(first), Some(last)) = (first, last) else {
            continue;
        };
        let left_gap = first;
        let right_gap = column_count.saturating_sub(last + 1);
        if (left_gap as isize - right_gap as isize).abs() > 1 {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_metric_group_off_center".to_string(),
                message: format!(
                    "panel `{}`: metric group row is visually off-center (left/right spare columns {}:{})",
                    panel.id, left_gap, right_gap
                ),
                source_path: Some(source_path.to_string()),
            });
            return;
        }
    }
}

pub(super) fn audit_panel_whitespace_budget(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(width) = panel_px_prop(panel, "width") else {
        return;
    };
    let Some(height) = panel_px_prop(panel, "height") else {
        return;
    };
    let padding_h = layout_padding_horizontal_px(layout);
    let padding_v = layout_padding_vertical_px(layout);
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let free_width = (padding_h / width).clamp(0.0, 1.0);
    let free_height = (padding_v / height).clamp(0.0, 1.0);
    if free_width > 0.25 || free_height > 0.25 {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            code: "layout_eval_panel_whitespace_loose".to_string(),
            message: format!(
                "panel `{}`: horizontal/vertical whitespace ratio is {:.0}%/{:.0}%, content may look too sparse",
                panel.id,
                free_width * 100.0,
                free_height * 100.0
            ),
            source_path: Some(source_path.to_string()),
        });
    }
}

pub(super) fn audit_metric_card_internal_budget(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for node in &panel.blocks {
        let UiNodeDecl::Panel(card) = node else {
            continue;
        };
        if !node_is_metric_card_like(node) {
            continue;
        }
        let template = card
            .props
            .as_object()
            .and_then(|map| map.get("__mei_metric_template"))
            .and_then(Value::as_str)
            .unwrap_or("stack");
        let inline_align = card
            .props
            .as_object()
            .and_then(|map| map.get("__mei_metric_inline_align"))
            .and_then(Value::as_str)
            .unwrap_or("compact");
        let height = panel_px_prop(card, "height").unwrap_or(0.0);
        if template == "stack_desc" && height > 0.0 && height < 94.0 {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_metric_stack_desc_overlap_risk".to_string(),
                message: format!(
                    "metric_card `{}`: stack_desc height {}px is tight and may overlap label/value/desc rows",
                    card.id,
                    height.round()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
        let layout = card.layout.as_ref();
        if matches!(template, "row" | "stack" | "stack_desc")
            && layout
                .and_then(|value| value.align.as_deref())
                .is_some_and(|value| !value.trim().eq_ignore_ascii_case("end"))
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_metric_inline_baseline_risk".to_string(),
                message: format!(
                    "metric_card `{}`: horizontal slots should align to the bottom baseline (`align = end`)",
                    card.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
        if inline_align != "between"
            && layout
                .and_then(|value| value.justify.as_deref())
                .is_some_and(|value| !value.trim().eq_ignore_ascii_case("center"))
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_metric_compact_center_risk".to_string(),
                message: format!(
                    "metric_card `{}`: compact metric rows should keep the slot group centered",
                    card.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
        if template == "row" {
            let padding = card
                .props
                .as_object()
                .and_then(|map| map.get("padding"))
                .and_then(Value::as_str)
                .unwrap_or("0");
            let padding_h = padding_horizontal_px(padding);
            if padding_h > 8.0 {
                diagnostics.push(Diagnostic {
                    severity: Severity::Info,
                    code: "layout_eval_metric_row_padding_loose".to_string(),
                    message: format!(
                        "metric_card `{}`: row padding `{}` may cause label/value/unit spacing to look loose",
                        card.id, padding
                    ),
                    source_path: Some(source_path.to_string()),
                });
            }
        }
    }
}

pub(super) fn audit_strategy_bypass_risk(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if panel_layout_policy(panel).is_some() || panel.blocks.is_empty() {
        return;
    }
    let metric_cards: Vec<&UiNodeDecl> = panel
        .blocks
        .iter()
        .filter(|node| node_is_metric_card_like(node))
        .collect();
    if metric_cards.len() != panel.blocks.len() {
        return;
    }
    let areas = layout.areas.as_ref();
    let rows = areas.map(Vec::len).unwrap_or(0);
    let cols = areas
        .and_then(|grid| grid.first())
        .map(Vec::len)
        .unwrap_or(0);
    let looks_like_strategy_shape = metric_cards.len() == 3
        || metric_cards.len() == 4
        || metric_cards.len() == 5
        || metric_cards.len() == 6;
    if !looks_like_strategy_shape {
        return;
    }
    diagnostics.push(Diagnostic {
        severity: Severity::Warning,
        code: "layout_eval_strategy_bypass_risk".to_string(),
        message: format!(
            "panel `{}`: explicit layout ({rows}x{cols}) is hand-authoring a metric group shape that cockpit policy could own",
            panel.id
        ),
        source_path: Some(source_path.to_string()),
    });
}

pub(super) fn estimate_body_required_height(panel: &PanelDecl) -> Option<f64> {
    let body_panel = panel.blocks.iter().find_map(|node| match node {
        UiNodeDecl::Panel(value) if node_area(node) == Some(SLOT_BODY) => Some(value),
        _ => None,
    })?;
    let body_layout = body_panel.layout.as_ref()?;
    let policy = panel_layout_policy(body_panel)?;
    if policy == LAYOUT_POLICY_METRICS_AUTO {
        let rows = body_layout.rows.as_ref()?;
        let row_budget = sum_fixed_px_tracks(rows)?;
        let padding_vertical = layout_padding_vertical_px(body_layout);
        let gap = layout_gap_y_px(body_layout) * rows.len().saturating_sub(1) as f64;
        return Some(row_budget + padding_vertical + gap);
    }
    if policy == LAYOUT_POLICY_METRICS_2_1 || policy == LAYOUT_POLICY_METRICS_STRIP {
        let card_height = body_panel
            .blocks
            .iter()
            .filter_map(node_height_track)
            .fold(None, |acc: Option<f64>, value| match acc {
                Some(existing) => Some(existing.max(value)),
                None => Some(value),
            })?;
        let padding_vertical = layout_padding_vertical_px(body_layout);
        return Some(card_height + padding_vertical);
    }
    if policy == LAYOUT_POLICY_METRICS_2X2 {
        let top_row = body_panel
            .blocks
            .iter()
            .take(2)
            .filter_map(node_height_track)
            .fold(None, |acc: Option<f64>, value| match acc {
                Some(existing) => Some(existing.max(value)),
                None => Some(value),
            })?;
        let bottom_row = body_panel
            .blocks
            .iter()
            .skip(2)
            .take(2)
            .filter_map(node_height_track)
            .fold(None, |acc: Option<f64>, value| match acc {
                Some(existing) => Some(existing.max(value)),
                None => Some(value),
            })?;
        let padding_vertical = layout_padding_vertical_px(body_layout);
        let gap = layout_gap_y_px(body_layout);
        return Some(top_row + bottom_row + padding_vertical + gap);
    }
    if policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1 {
        let rows = body_layout.rows.as_ref()?;
        let row_budget = sum_fixed_px_tracks(rows)?;
        let padding_vertical = layout_padding_vertical_px(body_layout);
        let gap = layout_gap_y_px(body_layout);
        return Some(row_budget + padding_vertical + gap);
    }
    None
}
