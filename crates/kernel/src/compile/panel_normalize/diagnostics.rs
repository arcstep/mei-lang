use serde_json::Value;

use crate::model::{Diagnostic, PanelDecl, Severity};

pub(super) fn emit_panel_head_diagnostics(
    panel: &PanelDecl,
    has_head: bool,
    had_title: bool,
    had_head_slot: bool,
    had_head_block: bool,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let show_heading = panel
        .props
        .as_object()
        .and_then(|map| map.get("show_heading"))
        .and_then(Value::as_bool);

    if show_heading == Some(false) && (had_title || had_head_slot || had_head_block) {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "redundant_show_heading".to_string(),
            message: format!(
                "panel `{}`: show_heading=False ignores title/head content",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }

    if show_heading == Some(true) && !has_head {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "empty_panel_head".to_string(),
            message: format!(
                "panel `{}`: show_heading=True but no title, head slot, or area=head block",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }

    if had_title && had_head_block && !had_head_slot {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            code: "panel_head_block_overrides_title".to_string(),
            message: format!(
                "panel `{}`: area=head block overrides title string for display",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }
}
