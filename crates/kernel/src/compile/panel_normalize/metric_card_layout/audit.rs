use crate::model::{Diagnostic, Severity, UiNodeDecl, UiTreeNode};

use super::super::nodes::{node_is_metric_card_like, panel_px_prop};
use super::{
    card_has_background_image, metric_prop_str, rows_use_fractional_tracks,
    PROP_METRIC_CONTENT_RATIO, PROP_METRIC_TEMPLATE, PROP_METRIC_TITLE_RATIO,
};

fn audit_one_metric_card_vertical_bands(
    card: &UiNodeDecl,
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
    let rows = layout
        .and_then(|value| value.rows.as_deref())
        .unwrap_or_default();
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
    panel: &UiNodeDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for node in &panel.blocks {
        let UiTreeNode::Panel(card) = node else {
            continue;
        };
        if node_is_metric_card_like(node) {
            audit_one_metric_card_vertical_bands(card, diagnostics, source_path);
        }
        audit_metric_vertical_bands(card, diagnostics, source_path);
    }
}
