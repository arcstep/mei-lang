use leptos::prelude::*;

/// 与 `app/assets/tree-icons/icons.svg` 同源；由 shell 注入到页面，`<use href="#i-…"/>` 同文档引用。
pub(crate) const TREE_ICONS_SPRITE_SVG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/tree-icons/icons.svg"
));

fn tree_icons_href(fragment: &str) -> String {
    format!("#{fragment}")
}

fn tree_sprite_icon(
    fragment: &'static str,
    title: &'static str,
    span_class: &'static str,
) -> AnyView {
    let href = tree_icons_href(fragment);
    view! {
        <span class=span_class title=title aria-hidden="true">
            <svg class="h-4 w-4 shrink-0" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
                <use href=href></use>
            </svg>
        </span>
    }
    .into_any()
}

fn folder_icon_svg(title: &'static str, span_class: &'static str) -> AnyView {
    view! {
        <span class=span_class title=title aria-hidden="true">
            <svg class="h-4 w-4 shrink-0" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
                <path
                    fill="#0f172a"
                    stroke="#38bdf8"
                    stroke-width="1.2"
                    stroke-linejoin="round"
                    d="M2.6 4.5h3l1.3 1.4h6.5a1 1 0 0 1 1 1v4.8a1 1 0 0 1-1 1H2.6a1 1 0 0 1-1-1V5.5a1 1 0 0 1 1-1Z"
                />
                <path
                    fill="none"
                    stroke="#7dd3fc"
                    stroke-width="1"
                    stroke-linecap="round"
                    d="M2.8 6.2h10.3"
                />
            </svg>
        </span>
    }
    .into_any()
}

fn tree_file_icon_for_path(path: &str) -> AnyView {
    let span_class = "inline-flex h-4 w-4 items-center justify-center";
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" => tree_sprite_icon("i-md", "Markdown", span_class),
        "json" | "jsonc" => tree_sprite_icon("i-json", "JSON", span_class),
        "js" | "jsx" | "mjs" | "cjs" => tree_sprite_icon("i-js", "JavaScript", span_class),
        "ts" | "tsx" => tree_sprite_icon("i-ts", "TypeScript", span_class),
        "css" | "scss" | "less" => tree_sprite_icon("i-css", "CSS", span_class),
        "py" | "pyi" => tree_sprite_icon("i-py", "Python", span_class),
        "csv" => tree_sprite_icon("i-csv", "CSV", span_class),
        "xlsx" | "xls" => tree_sprite_icon("i-xlsx", "表格", span_class),
        "html" | "htm" => tree_sprite_icon("i-html", "HTML", span_class),
        "svg" => tree_sprite_icon("i-markup", "SVG", span_class),
        "xml" => tree_sprite_icon("i-markup", "XML", span_class),
        "yaml" | "yml" => tree_sprite_icon("i-yaml", "YAML", span_class),
        "toml" => tree_sprite_icon("i-toml", "TOML", span_class),
        "rs" => tree_sprite_icon("i-rs", "Rust", span_class),
        "pdf" => tree_sprite_icon("i-pdf", "PDF", span_class),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" => {
            tree_sprite_icon("i-image", "图片", span_class)
        }
        "sh" | "bash" | "zsh" => tree_sprite_icon("i-shell", "Shell", span_class),
        _ => tree_sprite_icon("i-file", "文件", span_class),
    }
}

pub(crate) fn tree_icon_for_upload_entry(path: &str, is_dir: bool) -> AnyView {
    let span_class = "inline-flex h-4 w-4 items-center justify-center";
    if is_dir {
        folder_icon_svg("文件夹", span_class)
    } else {
        tree_file_icon_for_path(path)
    }
}
