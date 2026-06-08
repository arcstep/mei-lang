use crate::model::{Diagnostic, Severity};
use crate::typed_refs::{RefExpr, RefKind, SceneRegistry};

pub(crate) fn resolve_ref_path(
    expr: &RefExpr,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    not_resolved_code: &str,
) -> Option<String> {
    if let Some(path) = expr
        .locator
        .scene_file
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Some(path.to_string());
    }
    match scene_registry.resolve_target(&expr.locator) {
        Ok((_, path)) => Some(path),
        Err(message) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: not_resolved_code.to_string(),
                message,
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

pub(crate) fn push_invalid_base_kind(
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    kind_label: &str,
    expected: RefKind,
    got: RefKind,
) {
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: format!("invalid_{kind_label}_base_ref_kind"),
        message: format!("{kind_label}(base=...) requires `{expected:?}` ref, got `{got:?}` ref"),
        source_path: Some(target_file.to_string()),
    });
}
