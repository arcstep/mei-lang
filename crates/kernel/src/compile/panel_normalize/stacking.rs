use serde_json::{json, Value};

use crate::model::{Diagnostic, PanelDecl, Severity, UiNodeDecl};

const LAYOUT_STACK_MAX: u64 = 99;

pub fn sanitize_panel_stacking(
    panel: &mut PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    sanitize_props_stacking(&mut panel.props, diagnostics, source_path);
    for block in &mut panel.blocks {
        if let UiNodeDecl::Panel(nested) = block {
            sanitize_panel_stacking(nested, diagnostics, source_path);
        }
    }
}

fn sanitize_props_stacking(
    props: &mut Value,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(map) = props.as_object_mut() else {
        return;
    };
    if map.contains_key("z_index") || map.contains_key("z-index") {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "forbidden_z_index".to_string(),
            message: "props.z_index is forbidden; use stack_order on panel_contract (viewport tier) or layout_stack (local stacking within a parent panel)".to_string(),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let layout_stack = map
        .remove("layout_stack")
        .or_else(|| map.remove("layoutStack"));
    if let Some(raw) = layout_stack {
        match parse_layout_stack(&raw) {
            Ok(z) => {
                map.insert("z_index".to_string(), json!(z));
            }
            Err(message) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_layout_stack".to_string(),
                    message,
                    source_path: Some(source_path.to_string()),
                });
            }
        }
    }
}

fn parse_layout_stack(value: &Value) -> Result<i64, String> {
    let raw = value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
        .ok_or_else(|| "layout_stack must be an integer 0–99".to_string())?;
    if raw > LAYOUT_STACK_MAX {
        return Err(format!("layout_stack {raw} exceeds maximum {LAYOUT_STACK_MAX}"));
    }
    Ok(i64::try_from(raw).unwrap_or(i64::MAX))
}
