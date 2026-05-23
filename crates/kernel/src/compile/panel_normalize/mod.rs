use serde_json::Value;

use crate::model::{Diagnostic, PanelDecl, Severity, UiNodeDecl};

mod audit;
mod constants;
mod css_util;
mod diagnostics;
mod layout_policy;
mod nodes;
mod spacing;
mod slots;

#[cfg(test)]
mod tests;

use audit::emit_layout_audit_diagnostics;
use constants::{
    DEFAULT_METRICS_STRIP_GAP, DEFAULT_METRICS_STRIP_PADDING, LAYOUT_POLICY_METRIC_COMPOUND_2_1,
    LAYOUT_POLICY_METRICS_2_1, LAYOUT_POLICY_METRICS_STRIP, PROP_HAS_HEAD, SLOT_BODY, SLOT_HEAD,
};
use diagnostics::emit_panel_head_diagnostics;
use layout_policy::{
    inject_default_layout, inject_default_metric_compound_2_1_layout,
    inject_default_metrics_2_1_layout, inject_default_metrics_strip_layout, layout_has_slot,
    should_inject_metric_compound_2_1, should_inject_metrics_2_1, should_inject_metrics_strip,
};
use nodes::{blocks_touch_slot, remap_block_areas_to_body};
use slots::{
    hoist_heading_to_head_props, materialize_title_head_block, merge_head_slot, panel_has_body_blocks,
    resolve_has_head,
};
use spacing::{panel_layout_policy, policy_spacing, stamp_has_head_prop, stamp_layout_policy};

pub fn normalize_panel_slots(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for panel in panels.iter_mut() {
        normalize_panel(panel, diagnostics, source_path);
    }
}

pub fn panel_resolved_has_head(panel: &PanelDecl) -> bool {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_HAS_HEAD))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| resolve_has_head(panel, &[]))
}

fn normalize_panel(panel: &mut PanelDecl, diagnostics: &mut Vec<Diagnostic>, source_path: &str) {
    merge_head_slot(panel);
    for block in &mut panel.blocks {
        if let UiNodeDecl::Panel(nested) = block {
            normalize_panel(nested, diagnostics, source_path);
        }
    }

    let had_title = panel
        .title
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let had_head_slot = panel.head.is_some();
    let had_head_block = blocks_touch_slot(&panel.blocks, SLOT_HEAD);

    let has_head = resolve_has_head(panel, &[]);
    emit_panel_head_diagnostics(
        panel,
        has_head,
        had_title,
        had_head_slot,
        had_head_block,
        diagnostics,
        source_path,
    );

    if has_head {
        materialize_title_head_block(panel);
    }

    let has_body = panel_has_body_blocks(&panel.blocks, has_head);
    if panel.layout.is_none() {
        let requested_policy = panel_layout_policy(panel);
        match requested_policy.as_deref() {
            Some(LAYOUT_POLICY_METRICS_STRIP) => {
                if should_inject_metrics_strip(panel, has_head) {
                    let spacing = policy_spacing(
                        panel,
                        DEFAULT_METRICS_STRIP_GAP,
                        DEFAULT_METRICS_STRIP_PADDING,
                    );
                    inject_default_metrics_strip_layout(panel, &spacing);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRICS_STRIP);
                } else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "layout_policy_metrics_strip_conflict".to_string(),
                        message: format!(
                            "panel `{}`: layout_policy=metrics_strip requires at least 2 metric_card children and no head slot",
                            panel.id
                        ),
                        source_path: Some(source_path.to_string()),
                    });
                    inject_default_layout(panel, has_head, has_body);
                }
            }
            Some(LAYOUT_POLICY_METRICS_2_1) => {
                if should_inject_metrics_2_1(panel, has_head) {
                    inject_default_metrics_2_1_layout(panel);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRICS_2_1);
                } else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "layout_policy_metrics_2_1_conflict".to_string(),
                        message: format!(
                            "panel `{}`: layout_policy=metrics_2_1 requires exactly 3 metric_card children and no head slot",
                            panel.id
                        ),
                        source_path: Some(source_path.to_string()),
                    });
                    inject_default_layout(panel, has_head, has_body);
                }
            }
            Some(LAYOUT_POLICY_METRIC_COMPOUND_2_1) => {
                if should_inject_metric_compound_2_1(panel, has_head) {
                    inject_default_metric_compound_2_1_layout(panel);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRIC_COMPOUND_2_1);
                } else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "layout_policy_metric_compound_2_1_conflict".to_string(),
                        message: format!(
                            "panel `{}`: layout_policy=metric_compound_2_1 requires exactly 4 metric_card children and no head slot",
                            panel.id
                        ),
                        source_path: Some(source_path.to_string()),
                    });
                    inject_default_layout(panel, has_head, has_body);
                }
            }
            Some(policy) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_layout_policy".to_string(),
                    message: format!(
                        "panel `{}`: unknown layout_policy `{policy}`, fallback to default panel layout",
                        panel.id
                    ),
                    source_path: Some(source_path.to_string()),
                });
                inject_default_layout(panel, has_head, has_body);
            }
            None => {
                if should_inject_metrics_strip(panel, has_head) {
                    let spacing = policy_spacing(
                        panel,
                        DEFAULT_METRICS_STRIP_GAP,
                        DEFAULT_METRICS_STRIP_PADDING,
                    );
                    inject_default_metrics_strip_layout(panel, &spacing);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRICS_STRIP);
                } else {
                    inject_default_layout(panel, has_head, has_body);
                }
            }
        }
    }

    if layout_has_slot(panel.layout.as_ref(), SLOT_BODY)
        || panel
            .layout
            .as_ref()
            .is_none_or(|layout| layout.areas.is_none())
    {
        remap_block_areas_to_body(&mut panel.blocks);
    }
    emit_layout_audit_diagnostics(panel, diagnostics, source_path);

    hoist_heading_to_head_props(panel, diagnostics, source_path);
    stamp_has_head_prop(panel, has_head);
    panel.head = None;
}
