

use super::{
    theme_decl_to_value, validate_required_scene_theme_tokens, validate_required_shell_theme_tokens,
    validate_theme_layout_value, validate_theme_value_refs, walk_value_for_token_refs,
};

use serde_json::Value;

use crate::model::{Diagnostic, FrameDecl, UiNodeDecl, ThemeDecl};

pub fn validate_theme_decl(
    theme: &ThemeDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let value = theme_decl_to_value(theme);
    validate_theme_value_refs(&value, "theme", target_file, diagnostics);
    let profile = theme.id.as_str();
    validate_required_scene_theme_tokens(&value, profile, target_file, diagnostics);
}

pub fn validate_scene_theme_value_from_ops(
    id: &str,
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_theme_value_refs(
        value,
        &format!("ops.themes.`{id}`"),
        target_file,
        diagnostics,
    );
    validate_required_scene_theme_tokens(value, id, target_file, diagnostics);
    if let Some(layout) = value.get("layout") {
        validate_theme_layout_value(
            layout,
            &format!("ops.themes.`{id}`"),
            target_file,
            diagnostics,
        );
    }
}

/// Workspace / host shell theme (`ops.themes` on `.mei-workspace.json`).
pub fn validate_shell_theme_value(
    id: &str,
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_theme_value_refs(
        value,
        &format!("workspace.ops.themes.`{id}`"),
        target_file,
        diagnostics,
    );
    validate_required_shell_theme_tokens(value, target_file, diagnostics);
}

#[deprecated(note = "use validate_scene_theme_value_from_ops for app scene themes")]
pub fn validate_theme_value_from_ops(
    id: &str,
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_scene_theme_value_from_ops(id, value, target_file, diagnostics);
}

pub fn validate_frame_token_refs(
    frame: &FrameDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = frame
        .id
        .as_deref()
        .map(|id| format!("frame `{id}`"))
        .unwrap_or_else(|| "frame".to_string());
    validate_props_token_refs(&frame.props, context.as_str(), target_file, diagnostics);
}

pub fn validate_panel_token_refs(
    panel: &UiNodeDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = format!("panel `{}`", panel.id);
    validate_props_token_refs(&panel.props, context.as_str(), target_file, diagnostics);
    validate_props_token_refs(
        &panel.head_props,
        &format!("{context}.head_props"),
        target_file,
        diagnostics,
    );
    validate_props_token_refs(
        &panel.body_props,
        &format!("{context}.body_props"),
        target_file,
        diagnostics,
    );
}

fn validate_props_token_refs(
    value: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    walk_value_for_token_refs(value, context, false, target_file, diagnostics);
}

