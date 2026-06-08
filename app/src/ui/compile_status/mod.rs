mod asset_shell;
mod diagnostics;

pub(crate) use asset_shell::{
    asset_dual_preview_source, classify_asset_shell, codemirror_dataset_lang, is_mei_script_target,
    is_static_workspace_asset_target, AssetShellKind,
};
pub(crate) use diagnostics::{
    blocking_errors_for_preview, compile_diagnostics_for_mode,
    compile_diagnostics_other_file_count, compile_status_counts_for_display,
    compile_status_counts_for_target, compiled_has_error_diagnostics, is_manage_pipeline_diag,
    is_world_capsule_target, normalize_diagnostic_source, severity_counts,
    visible_diagnostics_count, world_capsule_companion_scene, DiagnosticsFilterMode,
};

use mei_lang_kernel::CompiledApp;

pub(crate) fn compile_status_summary(compiled: &CompiledApp, selected_target: &str) -> String {
    let (errors, warnings, infos) = compile_status_counts_for_display(compiled, selected_target);
    if errors == 0 && warnings == 0 && infos == 0 {
        "编译 正常".to_string()
    } else {
        let mut parts = Vec::new();
        if errors > 0 {
            parts.push(format!("{errors}错"));
        }
        if warnings > 0 {
            parts.push(format!("{warnings}警"));
        }
        if infos > 0 {
            parts.push(format!("{infos}提"));
        }
        format!("编译 {}", parts.join(" "))
    }
}

pub(crate) fn compile_status_title(compiled: &CompiledApp, current_target: &str) -> String {
    let (errors, warnings, infos) = compile_status_counts_for_display(compiled, current_target);
    let (cur_e, cur_w, cur_i) = compile_status_counts_for_target(compiled, current_target);
    if errors == 0 && warnings == 0 && infos == 0 {
        "当前没有编译诊断".to_string()
    } else {
        format!(
            "编译诊断（合计）：{} 错误，{} 警告，{} 提示；当前文件 {}：{} 错 / {} 警 / {} 提。点「调试」页签可按文件查看。",
            errors, warnings, infos, current_target, cur_e, cur_w, cur_i
        )
    }
}

pub(crate) fn compile_status_tone(compiled: &CompiledApp, selected_target: &str) -> &'static str {
    let (errors, warnings, infos) = compile_status_counts_for_display(compiled, selected_target);
    if errors > 0 {
        "danger"
    } else if warnings > 0 {
        "warn"
    } else if infos > 0 {
        "info"
    } else {
        "good"
    }
}
