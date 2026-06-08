pub(crate) fn is_mei_script_target(target: &str) -> bool {
    target.ends_with(".mei")
}

/// 工作区内非 `.mei` 的静态资源（HTML 原型、图片、数据文件等）。
pub(crate) fn is_static_workspace_asset_target(target: &str) -> bool {
    let trimmed = target.trim();
    !trimmed.is_empty() && !is_mei_script_target(trimmed)
}

fn file_extension_lower(target: &str) -> String {
    target.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}

/// 管理页「预览 + 源码」双栏资源：可渲染预览且需要独立源码视图。
pub(crate) fn asset_dual_preview_source(target: &str) -> bool {
    matches!(
        file_extension_lower(target).as_str(),
        "md" | "markdown" | "csv" | "svg" | "html" | "htm"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetShellKind {
    /// 预览 + 源码（CodeMirror）
    Dual,
    /// 仅 CodeMirror（无可分栏的预览）
    SourceCode,
    /// 图片 / PDF 等仅预览
    PreviewOnly,
    /// 无合适预览的二进制等
    Unsupported,
}

pub(crate) fn classify_asset_shell(target: &str) -> AssetShellKind {
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
pub(crate) fn codemirror_dataset_lang(target: &str) -> &'static str {
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
