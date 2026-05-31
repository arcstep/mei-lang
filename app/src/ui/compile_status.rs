use mei_lang_kernel::{CompiledApp, Diagnostic};

pub(super) const MANAGE_PIPELINE_DIAG_CODE: &str = "manage_page_pipeline";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiagnosticsFilterMode {
    CurrentFile,
    All,
}

impl DiagnosticsFilterMode {
    pub fn from_query(value: Option<&str>) -> Self {
        match value
            .map(str::trim)
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "all" => Self::All,
            _ => Self::CurrentFile,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::CurrentFile => "current",
            Self::All => "all",
        }
    }
}

pub(super) fn is_manage_pipeline_diag(diag: &Diagnostic) -> bool {
    diag.code == MANAGE_PIPELINE_DIAG_CODE
}

pub(super) fn is_compile_diagnostic(diag: &Diagnostic) -> bool {
    !is_manage_pipeline_diag(diag)
}

pub(super) fn normalize_target_path(target: &str) -> String {
    target
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub(super) fn normalize_diagnostic_source(
    app_root: &str,
    source_path: Option<&str>,
) -> Option<String> {
    let raw = source_path?.trim();
    if raw.is_empty() {
        return None;
    }
    let mut path = raw.replace('\\', "/");
    let root = app_root
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    if !root.is_empty() {
        if let Some(stripped) = path.strip_prefix(&root) {
            path = stripped.trim_start_matches('/').to_string();
        } else if path.starts_with(&format!("{root}/")) {
            path = path[root.len() + 1..].to_string();
        }
    }
    let path = path.trim_start_matches("./").to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

pub(super) fn diagnostic_matches_target(
    compiled: &CompiledApp,
    selected_target: &str,
    diag: &Diagnostic,
) -> bool {
    if !is_compile_diagnostic(diag) {
        return false;
    }
    let target = normalize_target_path(selected_target);
    let Some(source) = normalize_diagnostic_source(&compiled.app_root, diag.source_path.as_deref())
    else {
        return false;
    };
    if source == target {
        return true;
    }
    if source.ends_with(&format!("/{target}")) {
        return true;
    }
    let target_base = target.rsplit('/').next().unwrap_or(target.as_str());
    let source_base = source.rsplit('/').next().unwrap_or(source.as_str());
    !target_base.is_empty() && target_base == source_base
}

#[allow(dead_code)]
pub(super) fn is_global_or_unattributed_diagnostic(
    compiled: &CompiledApp,
    selected_target: &str,
    diag: &Diagnostic,
) -> bool {
    if !is_compile_diagnostic(diag) || diagnostic_matches_target(compiled, selected_target, diag) {
        return false;
    }
    let Some(source) = normalize_diagnostic_source(&compiled.app_root, diag.source_path.as_deref())
    else {
        return true;
    };
    source == "main.mei" || source.ends_with("/main.mei")
}

pub(super) fn compile_diagnostics_for_mode<'a>(
    compiled: &'a CompiledApp,
    selected_target: &str,
    mode: DiagnosticsFilterMode,
) -> Vec<&'a Diagnostic> {
    match mode {
        DiagnosticsFilterMode::All => compiled
            .diagnostics
            .iter()
            .filter(|diag| is_compile_diagnostic(diag))
            .collect(),
        DiagnosticsFilterMode::CurrentFile => compiled
            .diagnostics
            .iter()
            .filter(|diag| diagnostic_matches_target(compiled, selected_target, diag))
            .collect(),
    }
}

pub(super) fn compile_diagnostics_other_file_count(
    compiled: &CompiledApp,
    selected_target: &str,
) -> usize {
    compiled
        .diagnostics
        .iter()
        .filter(|diag| {
            is_compile_diagnostic(diag)
                && !diagnostic_matches_target(compiled, selected_target, diag)
        })
        .count()
}

pub(super) fn severity_counts(diags: &[&Diagnostic]) -> (usize, usize, usize) {
    let errors = diags
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Error))
        .count();
    let warnings = diags
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Warning))
        .count();
    let infos = diags
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Info))
        .count();
    (errors, warnings, infos)
}

pub(super) fn compile_status_counts_for_target(
    compiled: &CompiledApp,
    selected_target: &str,
) -> (usize, usize, usize) {
    let diags: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|diag| diagnostic_matches_target(compiled, selected_target, diag))
        .collect();
    severity_counts(&diags)
}

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

pub(super) fn compile_status_title(compiled: &CompiledApp, current_target: &str) -> String {
    let (errors, warnings, infos) = compile_status_counts(compiled);
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

fn file_extension_lower(target: &str) -> String {
    target.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

/// 管理页「预览 + 源码」双栏资源：可渲染预览且需要独立源码视图。
pub(super) fn asset_dual_preview_source(target: &str) -> bool {
    matches!(
        file_extension_lower(target).as_str(),
        "md" | "markdown" | "csv" | "svg" | "html" | "htm"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssetShellKind {
    /// 预览 + 源码（CodeMirror）
    Dual,
    /// 仅 CodeMirror（无可分栏的预览）
    SourceCode,
    /// 图片 / PDF 等仅预览
    PreviewOnly,
    /// 无合适预览的二进制等
    Unsupported,
}

pub(super) fn classify_asset_shell(target: &str) -> AssetShellKind {
    if asset_dual_preview_source(target) {
        return AssetShellKind::Dual;
    }
    let ext = file_extension_lower(target);
    if matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" | "pdf"
    ) {
        return AssetShellKind::PreviewOnly;
    }
    if matches!(
        ext.as_str(),
        "xlsx"
            | "xls"
            | "docx"
            | "doc"
            | "pptx"
            | "ppt"
            | "zip"
            | "gz"
            | "tgz"
            | "rar"
            | "7z"
            | "wasm"
            | "exe"
            | "dll"
            | "dylib"
            | "so"
            | "bin"
            | "dmg"
            | "apk"
            | "ipa"
    ) {
        return AssetShellKind::Unsupported;
    }
    AssetShellKind::SourceCode
}

/// 供前端 `data-source-lang` 与 CodeMirror 选择模式（非 mei 脚本亦适用）。
pub(super) fn codemirror_dataset_lang(target: &str) -> &'static str {
    match file_extension_lower(target).as_str() {
        "json" | "jsonc" => "json",
        "py" | "pyi" => "python",
        "css" | "scss" | "less" => "css",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "xml" | "svg" => "xml",
        "html" | "htm" => "html",
        "md" | "markdown" => "markdown",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "rs" => "rust",
        "sh" | "zsh" | "bash" => "shell",
        "mei" | "star" => "mei",
        _ => "plain",
    }
}

/// 预览降级：优先当前文件 Error，不足时再补其它文件 Error。
pub(super) fn blocking_errors_for_preview<'a>(
    compiled: &'a CompiledApp,
    selected_target: &str,
    limit: usize,
) -> Vec<&'a Diagnostic> {
    let mut picked = Vec::new();
    for diag in compiled.diagnostics.iter().filter(|diag| {
        matches!(diag.severity, mei_lang_kernel::Severity::Error)
            && diagnostic_matches_target(compiled, selected_target, diag)
    }) {
        picked.push(diag);
        if picked.len() >= limit {
            return picked;
        }
    }
    for diag in compiled.diagnostics.iter().filter(|diag| {
        matches!(diag.severity, mei_lang_kernel::Severity::Error)
            && !diagnostic_matches_target(compiled, selected_target, diag)
    }) {
        picked.push(diag);
        if picked.len() >= limit {
            break;
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use mei_lang_kernel::{Diagnostic, Severity};

    use super::*;

    fn diag(code: &str, path: &str) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: code.to_string(),
            message: "msg".to_string(),
            source_path: Some(path.to_string()),
        }
    }

    fn sample_compiled(diags: Vec<Diagnostic>) -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "/tmp/app".to_string(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: "main.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: std::collections::BTreeMap::new(),
            scene_bindings_by_id: std::collections::BTreeMap::new(),
            scene_examples_by_id: std::collections::BTreeMap::new(),
            scene_projection_assembly_by_id: std::collections::BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: std::collections::BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: diags,
        }
    }

    #[test]
    fn diagnostic_matches_target_by_relative_path() {
        let compiled = sample_compiled(vec![diag("missing_scene", "panels/shared-frame.mei")]);
        assert!(diagnostic_matches_target(
            &compiled,
            "panels/shared-frame.mei",
            &compiled.diagnostics[0]
        ));
        assert!(!diagnostic_matches_target(
            &compiled,
            "main.mei",
            &compiled.diagnostics[0]
        ));
    }

    #[test]
    fn compile_diagnostics_for_mode_filters_current_file() {
        let compiled = sample_compiled(vec![
            diag("missing_scene", "panels/shared-frame.mei"),
            diag("missing_scene", "main.mei"),
        ]);
        let current = compile_diagnostics_for_mode(
            &compiled,
            "panels/shared-frame.mei",
            DiagnosticsFilterMode::CurrentFile,
        );
        assert_eq!(current.len(), 1);
        assert_eq!(
            current[0].source_path.as_deref(),
            Some("panels/shared-frame.mei")
        );
        let all = compile_diagnostics_for_mode(
            &compiled,
            "panels/shared-frame.mei",
            DiagnosticsFilterMode::All,
        );
        assert_eq!(all.len(), 2);
    }
}
