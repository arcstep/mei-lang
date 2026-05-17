use leptos::prelude::*;
use mei_lang_kernel::WorkspaceNode;

use super::UiRouteMode;

/// 与 `app/assets/favicon.svg` 相同的梅花铜钱外轮廓（viewBox 32×32）。
const MEI_COIN_GLYPH_D: &str = "M16.000 1.400L17.255 1.656L18.402 2.378L19.366 3.437L20.130 4.652L20.740 5.834L21.294 6.830L21.913 7.556L22.704 8.010L23.728 8.272L24.974 8.470L26.352 8.751L27.713 9.238L28.880 9.994L29.689 11.018L30.032 12.240L29.883 13.552L29.308 14.836L28.450 16.000L27.497 17.006L26.639 17.876L26.020 18.685L25.710 19.534L25.687 20.517L25.851 21.687L26.045 23.034L26.101 24.475L25.878 25.878L25.301 27.085L24.370 27.953L23.156 28.395L21.783 28.401L20.386 28.050L19.078 27.488L17.922 26.900L16.915 26.461L16.000 26.300L15.085 26.461L14.078 26.900L12.922 27.488L11.614 28.050L10.217 28.401L8.844 28.395L7.630 27.953L6.699 27.085L6.122 25.878L5.899 24.475L5.955 23.034L6.149 21.688L6.313 20.517L6.290 19.534L5.980 18.685L5.361 17.876L4.503 17.006L3.550 16.000L2.692 14.836L2.117 13.552L1.968 12.240L2.311 11.018L3.120 9.994L4.287 9.238L5.648 8.751L7.026 8.470L8.272 8.272L9.296 8.010L10.087 7.556L10.706 6.830L11.260 5.834L11.870 4.652L12.634 3.437L13.598 2.378L14.745 1.656L16.000 1.400ZM12.9 11.75H19.1A1.15 1.15 0 0 1 20.25 12.9V19.1A1.15 1.15 0 0 1 19.1 20.25H12.9A1.15 1.15 0 0 1 11.75 19.1V12.9A1.15 1.15 0 0 1 12.9 11.75Z";

pub(super) fn controls_view() -> impl IntoView {
    view! {
        <div class="tree-toolbar mb-2.5 flex gap-2">
            <button
                class="tree-toolbar-btn inline-flex items-center rounded-lg border border-slate-400/20 bg-slate-800/70 px-2.5 py-1.5 text-xs text-slate-300 transition-colors hover:border-blue-400/40 hover:text-slate-100"
                type="button"
                data-tree-expand="1"
            >
                "展开"
            </button>
            <button
                class="tree-toolbar-btn inline-flex items-center rounded-lg border border-slate-400/20 bg-slate-800/70 px-2.5 py-1.5 text-xs text-slate-300 transition-colors hover:border-blue-400/40 hover:text-slate-100"
                type="button"
                data-tree-collapse="1"
            >
                "收起"
            </button>
        </div>
    }
}

pub(super) fn source_tree_view(
    nodes: &[WorkspaceNode],
    route_mode: UiRouteMode,
    app_path: &str,
    selected_target: &str,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> AnyView {
    let items = nodes
        .iter()
        .map(|node| {
            if node.kind == "dir" {
                let open = selected_target.starts_with(&format!("{}/", node.path));
                let children = source_tree_view(
                    &node.children,
                    route_mode,
                    app_path,
                    selected_target,
                    selected_entry,
                    preview_target,
                    active_tab,
                );
                view! {
                    <li class="tree-node tree-li-branch">
                        <details class="pl-1" open=open>
                            <summary class="tree-folder-summary flex min-w-0 cursor-pointer select-none items-center gap-1.5 py-1 text-xs font-bold text-slate-300">
                                <span class="tree-folder-icon inline-flex h-4 w-4 shrink-0 text-slate-400" aria-hidden="true">
                                    <svg viewBox="0 0 16 16" fill="currentColor" class="h-4 w-4">
                                        <path d="M2 4.5A1.5 1.5 0 013.5 3h2.2L6.6 4.2A1.5 1.5 0 007.8 5H12.5A1.5 1.5 0 0114 6.5v6A1.5 1.5 0 0112.5 14h-9A1.5 1.5 0 012 12.5v-8z"/>
                                    </svg>
                                </span>
                                <span class="tree-folder-label min-w-0 truncate">{node.name.clone()}</span>
                            </summary>
                            {children}
                        </details>
                    </li>
                }
                .into_any()
            } else {
                let href = source_href(
                    route_mode,
                    app_path,
                    node.path.as_str(),
                    selected_entry,
                    preview_target,
                    active_tab,
                );
                let class = if node.path == selected_target {
                    "tree-link active flex min-w-0 items-center gap-2 rounded-lg bg-blue-600/30 px-2.5 py-2 text-[13px] text-slate-50 transition-colors"
                } else {
                    "tree-link flex min-w-0 items-center gap-2 rounded-lg bg-slate-800/60 px-2.5 py-2 text-[13px] text-slate-300 transition-colors hover:bg-slate-700/70 hover:text-slate-100"
                };
                let preserve_manage_tab = if is_mei_script_path(node.path.as_str()) {
                    "1"
                } else {
                    "0"
                };
                let icon = file_row_icon(node);
                view! {
                    <li class="tree-node">
                        <a class=class href=href data-preserve-manage-tab=preserve_manage_tab>
                            <span class="shrink-0" aria-hidden="true">{icon}</span>
                            <span class="min-w-0 flex-1 truncate">{node.name.clone()}</span>
                        </a>
                    </li>
                }
                .into_any()
            }
        })
        .collect_view();
    view! { <ul class="tree m-0 grid list-none gap-1.5 p-0">{items}</ul> }.into_any()
}

fn file_row_icon(node: &WorkspaceNode) -> AnyView {
    let path = node.path.as_str();
    if path.ends_with(".mei") {
        return mei_coin_file_icon(node.mei_kind.as_deref());
    }
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let (title, inner): (&str, AnyView) = match ext.as_str() {
        "md" | "markdown" => (
            "Markdown",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-sky-400" fill="none" stroke="currentColor" stroke-width="1.4">
                    <path d="M4 3.5h8v9H4z"/>
                    <path d="M5.5 6h5M5.5 8h5M5.5 10h3"/>
                </svg>
            }
            .into_any(),
        ),
        "json" | "jsonc" => (
            "JSON",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-amber-300" fill="none" stroke="currentColor" stroke-width="1.35">
                    <path d="M5 3.5c-1.2 1.1-1.8 2.4-1.8 4s.6 2.9 1.8 4"/>
                    <path d="M11 3.5c1.2 1.1 1.8 2.4 1.8 4s-.6 2.9-1.8 4"/>
                    <path d="M7 12.5h2"/>
                </svg>
            }
            .into_any(),
        ),
        "js" | "jsx" | "mjs" | "cjs" => (
            "JavaScript",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-yellow-300" fill="currentColor">
                    <path d="M3 2.5h10v11H3z" opacity="0.2"/>
                    <path d="M5.2 11.3l.9-1.1c.5.4 1 .7 1.6.7.7 0 1.1-.3 1.1-.9 0-.5-.3-.8-1.2-1.1-.9-.4-1.5-.8-1.5-1.7 0-.9.7-1.6 1.9-1.6.8 0 1.4.3 1.8.7l-.8 1.1c-.4-.3-.8-.5-1.2-.5-.5 0-.8.3-.8.7 0 .5.3.7 1.1 1 .9.4 1.6.9 1.6 1.9 0 1-.8 1.7-2.1 1.7-.9 0-1.7-.3-2.2-.8z"/>
                </svg>
            }
            .into_any(),
        ),
        "ts" | "tsx" => (
            "TypeScript",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-blue-400" fill="currentColor">
                    <path d="M3 2.5h10v11H3z" opacity="0.15"/>
                    <path d="M5.2 11.3l.9-1.1c.5.4 1 .7 1.6.7.7 0 1.1-.3 1.1-.9 0-.5-.3-.8-1.2-1.1-.9-.4-1.5-.8-1.5-1.7 0-.9.7-1.6 1.9-1.6.8 0 1.4.3 1.8.7l-.8 1.1c-.4-.3-.8-.5-1.2-.5-.5 0-.8.3-.8.7 0 .5.3.7 1.1 1 .9.4 1.6.9 1.6 1.9 0 1-.8 1.7-2.1 1.7-.9 0-1.7-.3-2.2-.8z"/>
                </svg>
            }
            .into_any(),
        ),
        "css" | "scss" | "less" => (
            "CSS",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-indigo-300" fill="none" stroke="currentColor" stroke-width="1.35">
                    <path d="M3 3.5h10l-1 8.2-4.5 1.3L3 11.7z"/>
                    <path d="M6 6.5h4M6.3 8.5h3.4M6.6 10.5h2.8"/>
                </svg>
            }
            .into_any(),
        ),
        "py" | "pyi" => (
            "Python",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-emerald-400" fill="currentColor">
                    <path d="M8 2.2c-2.2 0-2 .6-2 1.4v1.1h2.7v.7H5.1c-1.2 0-2.2 1-2.2 2.2v1.5c0 1.2 1 2.1 2.2 2.1h.9v-1c0-1.3 1.1-2.4 2.4-2.4h2.7c1.1 0 2-.9 2-2V3.6c0-1-1-1.4-2.8-1.4zM6.4 3.5c.3 0 .5.2.5.5s-.2.5-.5.5-.5-.2-.5-.5.2-.5.5-.5z"/>
                    <path d="M10.9 7.8v1c0 1.3-1.1 2.4-2.4 2.4H5.8c-1.1 0-2 .9-2 2v1.4c0 1 1 1.6 2.8 1.6 2.2 0 2-.6 2-1.4v-1.1H6.1v-.7h3.6c1.2 0 2.2-1 2.2-2.2V8.7c0-1.2-1-2.1-2.2-2.1h-.8zM9.6 12c.3 0 .5.2.5.5s-.2.5-.5.5-.5-.2-.5-.5.2-.5.5-.5z"/>
                </svg>
            }
            .into_any(),
        ),
        "csv" => (
            "CSV",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-lime-300" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M3 3.5h10v9H3z"/>
                    <path d="M3 6.5h10M6 3.5v9M9 3.5v9"/>
                </svg>
            }
            .into_any(),
        ),
        "xlsx" | "xls" => (
            "表格",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-green-500" fill="currentColor">
                    <path d="M3 2.5h10v11H3zm3 0v11M3 6.5h10M3 9.5h10" fill="none" stroke="#14532d" stroke-width="0.6"/>
                    <rect x="3" y="2.5" width="10" height="11" rx="0.8" fill="#22c55e" opacity="0.25"/>
                </svg>
            }
            .into_any(),
        ),
        "html" | "htm" => (
            "HTML",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-orange-400" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M3 4l2.5 8L8 8l2.5 4L13 4"/>
                </svg>
            }
            .into_any(),
        ),
        "svg" | "xml" => (
            if ext == "svg" { "SVG" } else { "XML" },
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-fuchsia-300" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M3 12.5L8 3.5l5 9z"/>
                    <path d="M5.5 10h5"/>
                </svg>
            }
            .into_any(),
        ),
        "yaml" | "yml" => (
            "YAML",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-pink-300" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M4 4h2M4 8h2M4 12h2M7 5h5M7 8h5M7 11h3"/>
                </svg>
            }
            .into_any(),
        ),
        "toml" => (
            "TOML",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-rose-300" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M4 4h8M4 8h8M4 12h6"/>
                </svg>
            }
            .into_any(),
        ),
        "rs" => (
            "Rust",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-orange-200" fill="currentColor">
                    <path d="M8 2.5l5.5 3v5L8 13.5 2.5 10.5v-5L8 2.5zm0 2.2L4.8 6.8v2.4L8 11.3l3.2-2.1V6.8L8 4.7z"/>
                </svg>
            }
            .into_any(),
        ),
        "pdf" => (
            "PDF",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-red-400" fill="currentColor">
                    <path d="M4.5 2.5h4.2L11.5 5.3v8.2H4.5V2.5z"/>
                    <path d="M8.7 2.5v2.8h2.8" fill="none" stroke="#fecaca" stroke-width="0.6"/>
                </svg>
            }
            .into_any(),
        ),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" => (
            "图片",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-cyan-300" fill="none" stroke="currentColor" stroke-width="1.25">
                    <rect x="2.5" y="3.5" width="11" height="9" rx="1"/>
                    <path d="M2.5 11.5l3-3 2 2 3.5-3.5 3 3.5"/>
                    <circle cx="5.5" cy="6" r="0.9" fill="currentColor"/>
                </svg>
            }
            .into_any(),
        ),
        "sh" | "bash" | "zsh" => (
            "Shell",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-slate-300" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M3 3.5h10v9H3z"/>
                    <path d="M4.5 11l2-2-2-2"/>
                    <path d="M8.5 10.5h3"/>
                </svg>
            }
            .into_any(),
        ),
        _ => (
            "文件",
            view! {
                <svg viewBox="0 0 16 16" class="h-4 w-4 text-slate-400" fill="none" stroke="currentColor" stroke-width="1.25">
                    <path d="M4.5 2.5H9l2.5 2.5v9h-7v-11z"/>
                    <path d="M9 2.5v2.5H11.5" opacity="0.5"/>
                </svg>
            }
            .into_any(),
        ),
    };
    view! {
        <span class="inline-flex h-4 w-4 items-center justify-center" title=title>
            {inner}
        </span>
    }
    .into_any()
}

fn mei_coin_file_icon(mei_kind: Option<&str>) -> AnyView {
    let (title, fill): (&str, &str) = match mei_kind {
        Some("main") => ("应用入口 main.mei", "#fb7185"),
        Some("scene") => ("包含 scene 的 Mei 脚本", "#b91c1c"),
        _ => ("Mei 脚本", "#eab308"),
    };
    let wrap_class = "mei-tree-icon inline-flex h-4 w-4 shrink-0 items-center justify-center";
    view! {
        <span class=wrap_class title=title>
            <svg viewBox="0 0 32 32" class="h-4 w-4" aria-hidden="true">
                <path fill=fill fill-rule="evenodd" d=MEI_COIN_GLYPH_D></path>
            </svg>
        </span>
    }
    .into_any()
}

fn source_href(
    route_mode: UiRouteMode,
    app_path: &str,
    path: &str,
    selected_entry: Option<&str>,
    _preview_target: Option<&str>,
    _active_tab: Option<&str>,
) -> String {
    if is_mei_script_path(path) {
        return format!("/apps/{}/{}?target={}", route_mode.slug(), app_path, path);
    }
    let mut href = format!("/apps/{}/{}?target={}", route_mode.slug(), app_path, path);
    if let Some(entry) = selected_entry {
        href.push_str("&entry=");
        href.push_str(entry);
    }
    href
}

fn is_mei_script_path(path: &str) -> bool {
    path.ends_with(".mei")
}
