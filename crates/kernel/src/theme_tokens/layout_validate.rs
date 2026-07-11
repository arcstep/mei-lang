//! Validate `ops.themes.*.layout` keys (0327 D3).

use serde_json::Value;

use crate::model::{Diagnostic, Severity};

use super::push_diagnostic;

pub fn validate_theme_layout_value(
    layout: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(scopes) = layout.as_object() else {
        return;
    };
    for (scope_path, patch) in scopes {
        let scope_ctx = format!("{context}.layout.`{scope_path}`");
        let Some(patch_obj) = patch.as_object() else {
            push_layout_diagnostic(
                diagnostics,
                "theme_layout_invalid_scope",
                format!("{scope_ctx} must be an object"),
                target_file,
            );
            continue;
        };
        if let Some(rows) = patch_obj
            .get("sectionRows")
            .or_else(|| patch_obj.get("section_rows"))
        {
            validate_section_rows(rows, scope_ctx.as_str(), target_file, diagnostics);
        }
        if let Some(header_height) = patch_obj
            .get("headerHeight")
            .or_else(|| patch_obj.get("header_height"))
        {
            validate_header_height(header_height, scope_ctx.as_str(), target_file, diagnostics);
        }
    }
}

fn validate_section_rows(
    rows: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(items) = rows.as_array() else {
        push_layout_diagnostic(
            diagnostics,
            "theme_layout_section_rows_invalid",
            format!("{context}.sectionRows must be an array"),
            target_file,
        );
        return;
    };
    if items.is_empty() {
        push_layout_diagnostic(
            diagnostics,
            "theme_layout_section_rows_invalid",
            format!("{context}.sectionRows must not be empty"),
            target_file,
        );
        return;
    }
    for (idx, item) in items.iter().enumerate() {
        let Some(text) = item.as_str() else {
            push_layout_diagnostic(
                diagnostics,
                "theme_layout_section_rows_invalid",
                format!("{context}.sectionRows[{idx}] must be a string"),
                target_file,
            );
            continue;
        };
        if !is_fr_track(text) {
            push_layout_diagnostic(
                diagnostics,
                "theme_layout_section_rows_px_forbidden",
                format!("{context}.sectionRows[{idx}] must be Nfr, got `{text}`"),
                target_file,
            );
        }
    }
}

fn validate_header_height(
    value: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(text) = value.as_str() else {
        push_layout_diagnostic(
            diagnostics,
            "theme_layout_header_height_invalid",
            format!("{context}.headerHeight must be a px string"),
            target_file,
        );
        return;
    };
    if !text.ends_with("px") {
        push_layout_diagnostic(
            diagnostics,
            "theme_layout_header_height_invalid",
            format!("{context}.headerHeight must use px, got `{text}`"),
            target_file,
        );
    }
}

fn is_fr_track(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.ends_with("fr") && trimmed.trim_end_matches("fr").parse::<f64>().is_ok()
}

fn push_layout_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    code: &str,
    message: String,
    target_file: &str,
) {
    push_diagnostic(diagnostics, Severity::Error, code, message, target_file);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn section_rows_rejects_px_tracks() {
        let layout = json!({
            "home/T1/left_rail": {"sectionRows": ["120px"]}
        });
        let mut diagnostics = Vec::new();
        validate_theme_layout_value(
            &layout,
            "ops.themes.cockpit",
            "app.config.json",
            &mut diagnostics,
        );
        assert!(diagnostics
            .iter()
            .any(|d| d.code == "theme_layout_section_rows_px_forbidden"));
    }

    #[test]
    fn section_rows_accepts_fr_tracks() {
        let layout = json!({
            "home/T1/left_rail": {"sectionRows": ["1fr", "2.52fr"]}
        });
        let mut diagnostics = Vec::new();
        validate_theme_layout_value(
            &layout,
            "ops.themes.cockpit",
            "app.config.json",
            &mut diagnostics,
        );
        assert!(diagnostics.is_empty());
    }
}
