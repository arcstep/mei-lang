use crate::model::{Diagnostic, LayoutDecl, PanelDecl, Severity};

use super::super::constants::{
    COCKPIT_CARD_GAP_MAX, COCKPIT_CARD_GAP_MIN, COCKPIT_CARD_GAP_TARGET, COCKPIT_PANEL_PADDING_MAX,
    COCKPIT_PANEL_PADDING_MIN, LAYOUT_POLICY_METRICS_AUTO, LAYOUT_POLICY_METRIC_COMPOUND_2_1,
};
use super::super::css_util::{
    css_scalar_numbers, first_css_scalar_px, layout_gap_y_px, layout_padding_vertical_px,
    sum_fixed_px_tracks,
};
use super::super::nodes::{node_height_track, panel_px_prop};
use super::super::spacing::panel_layout_policy;

use super::is_metric_layout_policy;

const METRICS_AUTO_EXPANDED_GAP_MAX: f64 = 36.0;
/// 宽卡 compound 内层需贴背景横线，允许比 metrics_auto 更紧的 gap/padding。
const METRIC_COMPOUND_GAP_MIN: f64 = 2.0;
const METRIC_COMPOUND_PADDING_MIN: f64 = 2.0;
const METRIC_COMPOUND_PADDING_MAX: f64 = 12.0;

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
    let compound_shell = policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1;
    let gap_min = if compound_shell {
        METRIC_COMPOUND_GAP_MIN
    } else {
        COCKPIT_CARD_GAP_MIN
    };
    let gap_budget_max = if fixed_width_auto {
        METRICS_AUTO_EXPANDED_GAP_MAX
    } else {
        COCKPIT_CARD_GAP_MAX
    };
    if let Some(gap) = layout.gap.as_deref().and_then(first_css_scalar_px) {
        if gap < gap_min - 0.1 || gap > gap_budget_max + 0.1 {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_card_gap_out_of_budget".to_string(),
                message: format!(
                    "panel `{}`: card gap {}px is outside cockpit budget [{}, {}]px",
                    panel.id,
                    gap.round(),
                    gap_min,
                    gap_budget_max
                ),
                source_path: Some(source_path.to_string()),
            });
        } else if !fixed_width_auto
            && !compound_shell
            && (gap - COCKPIT_CARD_GAP_TARGET).abs() > 3.0
        {
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
        let (padding_min, padding_budget_max) = if compound_shell {
            (METRIC_COMPOUND_PADDING_MIN, METRIC_COMPOUND_PADDING_MAX)
        } else if fixed_width_auto {
            (
                COCKPIT_PANEL_PADDING_MIN,
                METRICS_AUTO_EXPANDED_GAP_MAX * 2.0 + 4.0,
            )
        } else {
            (COCKPIT_PANEL_PADDING_MIN, COCKPIT_PANEL_PADDING_MAX)
        };
        let too_small = values
            .iter()
            .any(|value| *value > 0.0 && *value < padding_min - 0.1);
        let too_large = values.iter().any(|value| *value > padding_budget_max + 0.1);
        if too_small || too_large {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_eval_panel_padding_out_of_budget".to_string(),
                message: format!(
                    "panel `{}`: layout padding `{padding}` is outside cockpit budget {}-{}px",
                    panel.id, padding_min, padding_budget_max
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

pub(super) fn audit_metric_compound_row_budget(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(policy) = panel_layout_policy(panel) else {
        return;
    };
    if policy != LAYOUT_POLICY_METRIC_COMPOUND_2_1 {
        return;
    }
    let Some(shell_h) = panel_px_prop(panel, "height") else {
        return;
    };
    let padding_v = layout_padding_vertical_px(layout);
    let gap = layout_gap_y_px(layout);
    let available = shell_h - padding_v - gap;
    let Some(row_sum) = layout
        .rows
        .as_ref()
        .and_then(|tracks| sum_fixed_px_tracks(tracks))
    else {
        return;
    };
    if row_sum > available + 1.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "layout_eval_compound_row_clip_risk".to_string(),
            message: format!(
                "panel `{}`: compound rows {}px exceed content budget {}px (padding/gap included); bottom cards may be clipped",
                panel.id,
                row_sum.round(),
                available.round()
            ),
            source_path: Some(source_path.to_string()),
        });
    }
    let hinted_sum = panel
        .blocks
        .first()
        .and_then(node_height_track)
        .into_iter()
        .chain(panel.blocks.iter().skip(1).filter_map(node_height_track))
        .sum::<f64>();
    if hinted_sum > available + 1.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_eval_compound_height_scaled".to_string(),
            message: format!(
                "panel `{}`: child height_px sum {}px exceeds shell content budget {}px; row tracks were scaled to fit",
                panel.id,
                hinted_sum.round(),
                available.round()
            ),
            source_path: Some(source_path.to_string()),
        });
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
