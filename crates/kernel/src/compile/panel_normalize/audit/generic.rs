use std::collections::BTreeSet;

use crate::model::{Diagnostic, LayoutDecl, UiNodeDecl, Severity, UiTreeNode};

use super::super::constants::{
    LAYOUT_POLICY_METRICS_2X2, LAYOUT_POLICY_METRICS_2_1, LAYOUT_POLICY_METRICS_AUTO,
    LAYOUT_POLICY_METRICS_STRIP, LAYOUT_POLICY_METRIC_COMPOUND_2_1, CONTENT_ZONE, TITLE_ZONE,
};
use super::super::css_util::{
    css_scalar_numbers, is_degenerate_track, layout_gap_y_px, layout_padding_horizontal_px,
    layout_padding_vertical_px, parse_px, sum_fixed_px_tracks,
};
use super::super::layout_policy::layout_has_slot;
use super::super::nodes::panel_head_height_track;
use super::super::nodes::{
    node_area, node_has_explicit_area, node_height_track, node_is_metric_card_like, panel_px_prop,
};
use super::super::spacing::panel_layout_policy;

pub(super) fn audit_layout_matrix(
    panel: &UiNodeDecl,
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
    panel: &UiNodeDecl,
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
    panel: &UiNodeDecl,
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
    panel: &UiNodeDecl,
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
                code: "layout_policy_budget_overflow".to_string(),
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
                code: "layout_policy_budget_overflow".to_string(),
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
    panel: &UiNodeDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if !layout_has_slot(Some(layout), TITLE_ZONE) || !layout_has_slot(Some(layout), CONTENT_ZONE) {
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
            code: "layout_policy_budget_overflow".to_string(),
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

pub(super) fn audit_panel_whitespace_budget(
    panel: &UiNodeDecl,
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

pub(super) fn audit_strategy_bypass_risk(
    panel: &UiNodeDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if panel_layout_policy(panel).is_some() || panel.blocks.is_empty() {
        return;
    }
    let metric_cards: Vec<&UiTreeNode> = panel
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

pub(super) fn estimate_body_required_height(panel: &UiNodeDecl) -> Option<f64> {
    let body_panel = panel.blocks.iter().find_map(|node| match node {
        UiTreeNode::Panel(value) if node_area(node) == Some(CONTENT_ZONE) => Some(value),
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
