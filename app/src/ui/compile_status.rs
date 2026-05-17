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

fn file_extension_lower(target: &str) -> String {
    target
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
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
        "xlsx" | "xls" | "docx" | "doc" | "pptx" | "ppt" | "zip" | "gz" | "tgz" | "rar"
            | "7z" | "wasm" | "exe" | "dll" | "dylib" | "so" | "bin" | "dmg" | "apk" | "ipa"
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
