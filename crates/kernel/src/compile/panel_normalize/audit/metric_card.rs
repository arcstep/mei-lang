use serde_json::Value;

use crate::model::{Diagnostic, UiNodeDecl, Severity, UiTreeNode};

use super::super::css_util::padding_horizontal_px;
use super::super::nodes::{node_is_metric_card_like, panel_px_prop};

pub(super) fn audit_metric_card_internal_budget(
    panel: &UiNodeDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for node in &panel.blocks {
        let UiTreeNode::Panel(card) = node else {
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
        if template == "row"
            && layout
                .and_then(|value| value.align.as_deref())
                .is_some_and(|value| !value.trim().eq_ignore_ascii_case("end"))
        {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_eval_metric_inline_baseline_risk".to_string(),
                message: format!(
                    "metric_card `{}`: row template slots should align to the bottom baseline (`align = end`)",
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
