use crate::model::{Diagnostic, Severity};

pub fn push_deprecated_ref_binding_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    compat_source: Option<&str>,
    target_file: &str,
) {
    let Some(compat_source) = compat_source else {
        return;
    };
    let (code, message) = match compat_source {
        "world_file_ref" => (
            "deprecated_world_file_ref",
            "world_file_ref(...) is deprecated; migrate to world_ref(scene_file = ..., id = ...)",
        ),
        "frame_file_ref" => (
            "deprecated_frame_file_ref",
            "frame_file_ref(...) is deprecated; migrate to frame_ref(scene_file = ..., id = ...)",
        ),
        "flow_file_ref" => (
            "deprecated_flow_file_ref",
            "flow_file_ref(...) is deprecated; migrate to flow_ref(scene_file = ..., id = ...)",
        ),
        _ => return,
    };
    diagnostics.push(Diagnostic {
        severity: Severity::Warning,
        code: code.to_string(),
        message: message.to_string(),
        source_path: Some(target_file.to_string()),
    });
}
