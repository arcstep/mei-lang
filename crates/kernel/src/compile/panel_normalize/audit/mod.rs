use crate::model::{Diagnostic, Severity, UiNodeDecl};

use super::constants::{
    LAYOUT_POLICY_METRICS_2X2, LAYOUT_POLICY_METRICS_2_1, LAYOUT_POLICY_METRICS_AUTO,
    LAYOUT_POLICY_METRICS_STRIP, LAYOUT_POLICY_METRIC_COMPOUND_2_1,
};

mod generic;
mod metric_card;
mod metrics_strip;

use generic::*;
use metric_card::*;
use metrics_strip::*;

const LAYOUT_EVAL_PREFIX: &str = "layout_eval_";

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
    panel: &UiNodeDecl,
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

pub(super) fn is_metric_layout_policy(policy: &str) -> bool {
    policy == LAYOUT_POLICY_METRICS_AUTO
        || policy == LAYOUT_POLICY_METRICS_STRIP
        || policy == LAYOUT_POLICY_METRICS_2X2
        || policy == LAYOUT_POLICY_METRICS_2_1
        || policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1
}

pub(super) fn emit_layout_audit_diagnostics(
    panel: &UiNodeDecl,
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
    audit_metric_compound_row_budget(panel, layout, diagnostics, source_path);
    audit_metric_card_internal_budget(panel, diagnostics, source_path);
    audit_strategy_bypass_risk(panel, layout, diagnostics, source_path);
    emit_panel_eval_summary(panel, diagnostics, source_path, start_idx);
}
