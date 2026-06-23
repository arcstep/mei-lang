use serde_json::Value;

use super::types::WorkspaceConfig;
use crate::model::{Diagnostic, Severity};
use crate::theme_tokens::validate_shell_theme_value;

/// Resolve workspace shell theme JSON from `ops.shellTheme` → `ops.themes`.
pub fn resolve_workspace_shell_theme(workspace: &WorkspaceConfig) -> Option<Value> {
    let id = workspace.ops.shell_theme.as_deref()?;
    workspace.ops.themes.get(id).cloned()
}

/// Validate configured workspace shell theme; emits warnings when unset or incomplete.
pub fn validate_workspace_shell_theme(
    workspace: &WorkspaceConfig,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(id) = workspace.ops.shell_theme.as_deref() else {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "missing_shell_theme".to_string(),
            message:
                "workspace is missing ops.shellTheme; host chrome uses builtin page shell fallback"
                    .to_string(),
            source_path: Some(target_file.to_string()),
        });
        return;
    };
    let Some(theme) = workspace.ops.themes.get(id) else {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "missing_shell_theme".to_string(),
            message: format!("ops.shellTheme id `{id}` not found in workspace ops.themes"),
            source_path: Some(target_file.to_string()),
        });
        return;
    };
    validate_shell_theme_value(id, theme, target_file, diagnostics);
}
