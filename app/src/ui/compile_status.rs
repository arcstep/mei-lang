use mei_lang_kernel::CompiledApp;

pub(super) fn compile_status_summary(compiled: &CompiledApp) -> String {
    let (errors, warnings, infos) = compile_status_counts(compiled);
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

pub(super) fn compile_status_title(compiled: &CompiledApp) -> String {
    let (errors, warnings, infos) = compile_status_counts(compiled);
    if errors == 0 && warnings == 0 && infos == 0 {
        "当前没有编译诊断".to_string()
    } else {
        format!(
            "编译诊断：{} 错误，{} 警告，{} 提示",
            errors, warnings, infos
        )
    }
}

pub(super) fn compile_status_tone(compiled: &CompiledApp) -> &'static str {
    let (errors, warnings, infos) = compile_status_counts(compiled);
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

pub(super) fn compile_status_counts(compiled: &CompiledApp) -> (usize, usize, usize) {
    let errors = compiled
        .diagnostics
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Error))
        .count();
    let warnings = compiled
        .diagnostics
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Warning))
        .count();
    let infos = compiled
        .diagnostics
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Info))
        .count();
    (errors, warnings, infos)
}

pub(super) fn compiled_has_error_diagnostics(compiled: &CompiledApp) -> bool {
    compiled
        .diagnostics
        .iter()
        .any(|diag| matches!(diag.severity, mei_lang_kernel::Severity::Error))
}

pub(super) fn is_mei_script_target(target: &str) -> bool {
    target.ends_with(".mei")
}

pub(super) fn source_language(target: &str) -> &'static str {
    if is_mei_script_target(target) {
        "mei"
    } else {
        "plain"
    }
}
