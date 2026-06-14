use leptos::prelude::*;
use mei_lang_kernel::WorkspaceNode;

use super::manage_routing::{access_scene_route_suffix, encode_query_value};
use super::UiRouteMode;

/// 与 `app/assets/favicon.svg` 相同的梅花铜钱外轮廓（viewBox 32×32）。
const MEI_COIN_GLYPH_D: &str = "M16.000 1.400L17.255 1.656L18.402 2.378L19.366 3.437L20.130 4.652L20.740 5.834L21.294 6.830L21.913 7.556L22.704 8.010L23.728 8.272L24.974 8.470L26.352 8.751L27.713 9.238L28.880 9.994L29.689 11.018L30.032 12.240L29.883 13.552L29.308 14.836L28.450 16.000L27.497 17.006L26.639 17.876L26.020 18.685L25.710 19.534L25.687 20.517L25.851 21.687L26.045 23.034L26.101 24.475L25.878 25.878L25.301 27.085L24.370 27.953L23.156 28.395L21.783 28.401L20.386 28.050L19.078 27.488L17.922 26.900L16.915 26.461L16.000 26.300L15.085 26.461L14.078 26.900L12.922 27.488L11.614 28.050L10.217 28.401L8.844 28.395L7.630 27.953L6.699 27.085L6.122 25.878L5.899 24.475L5.955 23.034L6.149 21.688L6.313 20.517L6.290 19.534L5.980 18.685L5.361 17.876L4.503 17.006L3.550 16.000L2.692 14.836L2.117 13.552L1.968 12.240L2.311 11.018L3.120 9.994L4.287 9.238L5.648 8.751L7.026 8.470L8.272 8.272L9.296 8.010L10.087 7.556L10.706 6.830L11.260 5.834L11.870 4.652L12.634 3.437L13.598 2.378L14.745 1.656L16.000 1.400ZM12.9 11.75H19.1A1.15 1.15 0 0 1 20.25 12.9V19.1A1.15 1.15 0 0 1 19.1 20.25H12.9A1.15 1.15 0 0 1 11.75 19.1V12.9A1.15 1.15 0 0 1 12.9 11.75Z";

/// 与 `app/assets/tree-icons/icons.svg` 同源；由 `manage_shell` 注入到页面，`<use href="#i-…"/>` 同文档引用。
pub(crate) const TREE_ICONS_SPRITE_SVG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/tree-icons/icons.svg"
));

fn tree_icons_href(fragment: &str) -> String {
    format!("#{fragment}")
}

/// 非 .mei 树图标：彩色定义在 `TREE_ICONS_SPRITE_SVG` 的 symbol 内；此处仅 `<use href="#…"/>`。
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

pub(crate) fn source_tree_view(
    nodes: &[WorkspaceNode],
    route_mode: UiRouteMode,
    app_path: &str,
    selected_target: &str,
    selected_scene: Option<&str>,
    scene_target_pairs: &[(String, String)],
    omit_file_query_when_path: &str,
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
                    selected_scene,
                    scene_target_pairs,
                    omit_file_query_when_path,
                    active_tab,
                );
                let folder_path = node.path.clone();
                view! {
                    <li class="tree-node tree-li-branch">
                        <details class="pl-1" open=open>
                            <summary
                                class="tree-folder-summary flex min-w-0 cursor-pointer select-none items-center gap-1 py-1 text-xs font-medium text-slate-300"
                                title=folder_path.clone()
                            >
                                <span class="tree-folder-label min-w-0 truncate">{node.name.clone()}</span>
                            </summary>
                            {children}
                        </details>
                    </li>
                }
                .into_any()
            } else if node.kind == "scene_export" {
                scene_export_tree_row(
                    route_mode,
                    app_path,
                    node,
                    selected_target,
                    selected_scene,
                    active_tab,
                )
            } else if route_mode == UiRouteMode::Build
                && node
                    .children
                    .iter()
                    .any(|child| child.kind == "scene_export")
            {
                multi_scene_export_file_branch(
                    route_mode,
                    app_path,
                    node,
                    selected_target,
                    selected_scene,
                    active_tab,
                )
            } else {
                let scene_for_link = if route_mode.uses_scene_route() {
                    scene_target_pairs
                        .iter()
                        .find(|(target_file, _)| target_file.as_str() == node.path.as_str())
                        .map(|(_, scene_id)| scene_id.as_str())
                } else {
                    None
                };
                let file_for_link = if route_mode == UiRouteMode::Build {
                    Some(node.path.as_str())
                } else {
                    None
                };
                let href = source_href(
                    route_mode,
                    app_path,
                    node.path.as_str(),
                    scene_for_link,
                    file_for_link,
                    active_tab,
                );
                let class = if node.path == selected_target {
                    "tree-link tree-link--active flex min-w-0 w-full items-center gap-1.5 border-l-2 border-sky-400 bg-sky-500/15 py-0.5 pl-2 pr-1 text-[13px] font-medium text-sky-100 transition-colors"
                } else {
                    "tree-link flex min-w-0 w-full items-center gap-1.5 border-l-2 border-transparent py-0.5 pl-2 pr-1 text-[13px] text-slate-300 transition-colors hover:text-slate-100"
                };
                let preserve_manage_tab = if is_mei_script_path(node.path.as_str()) {
                    "1"
                } else {
                    "0"
                };
                let icon = file_row_icon(node);
                let file_path = node.path.clone();
                let manage_config_link = false;
                view! {
                    <li class="tree-node">
                        <a
                            class=class
                            href=href
                            data-preserve-manage-tab=preserve_manage_tab
                            data-manage-config-link=manage_config_link.then_some("1")
                            title=file_path.clone()
                        >
                            <span class="shrink-0" aria-hidden="true">{icon}</span>
                            <span class="min-w-0 flex-1 truncate">{node.name.clone()}</span>
                        </a>
                    </li>
                }
                .into_any()
            }
        })
        .collect::<Vec<_>>();
    let items = items.into_iter().collect_view();
    view! { <ul class="tree m-0 grid list-none gap-0.5 p-0">{items}</ul> }.into_any()
}

fn scene_export_is_active(
    file_path: &str,
    export_id: Option<&str>,
    selected_target: &str,
    selected_scene: Option<&str>,
) -> bool {
    if file_path != selected_target {
        return false;
    }
    let Some(export_id) = export_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return false;
    };
    selected_scene
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .is_some_and(|wanted| wanted == export_id)
}

fn multi_scene_export_file_branch(
    route_mode: UiRouteMode,
    app_path: &str,
    node: &WorkspaceNode,
    selected_target: &str,
    selected_scene: Option<&str>,
    active_tab: Option<&str>,
) -> AnyView {
    let open = selected_target == node.path.as_str();
    let icon = file_row_icon(node);
    let export_children = node
        .children
        .iter()
        .filter(|child| child.kind == "scene_export")
        .map(|child| {
            scene_export_tree_row(
                route_mode,
                app_path,
                child,
                selected_target,
                selected_scene,
                active_tab,
            )
        })
        .collect::<Vec<_>>();
    let export_children = export_children.into_iter().collect_view();
    view! {
        <li class="tree-node tree-li-branch tree-li-scene-exports">
            <details class="pl-1" open=open>
                <summary
                    class="tree-folder-summary tree-scene-export-file-summary flex min-w-0 cursor-pointer select-none items-center gap-1.5 py-0.5 pl-2 pr-1 text-[13px] text-slate-300"
                    title=node.path.clone()
                >
                    <span class="shrink-0" aria-hidden="true">{icon}</span>
                    <span class="tree-folder-label min-w-0 truncate">{node.name.clone()}</span>
                </summary>
                <ul class="tree m-0 grid list-none gap-0.5 p-0 pl-3">
                    {export_children}
                </ul>
            </details>
        </li>
    }
    .into_any()
}

fn scene_export_tree_row(
    route_mode: UiRouteMode,
    app_path: &str,
    node: &WorkspaceNode,
    selected_target: &str,
    selected_scene: Option<&str>,
    active_tab: Option<&str>,
) -> AnyView {
    let export_id = node.scene_export_id.as_deref();
    let href = source_href(
        route_mode,
        app_path,
        node.path.as_str(),
        export_id,
        if route_mode == UiRouteMode::Build {
            Some(node.path.as_str())
        } else {
            None
        },
        active_tab,
    );
    let active = scene_export_is_active(
        node.path.as_str(),
        export_id,
        selected_target,
        selected_scene,
    );
    let class = if active {
        "tree-link tree-link--scene-export tree-link--active flex min-w-0 w-full items-center gap-1.5 border-l-2 border-sky-400 bg-sky-500/15 py-0.5 pl-2 pr-1 text-[12px] font-medium text-sky-100 transition-colors"
    } else {
        "tree-link tree-link--scene-export flex min-w-0 w-full items-center gap-1.5 border-l-2 border-transparent py-0.5 pl-2 pr-1 text-[12px] text-slate-400 transition-colors hover:text-slate-100"
    };
    let icon = mei_coin_file_icon(Some("scene"));
    let title = format!(
        "scene_export `{}` · {}",
        export_id.unwrap_or("-"),
        node.path
    );
    view! {
        <li class="tree-node">
            <a
                class=class
                href=href
                data-preserve-manage-tab="1"
                title=title
            >
                <span class="shrink-0" aria-hidden="true">{icon}</span>
                <span class="min-w-0 flex-1 truncate font-mono text-[11px]">{node.name.clone()}</span>
            </a>
        </li>
    }
    .into_any()
}

pub(crate) fn tree_file_icon_for_path(path: &str) -> AnyView {
    if path.ends_with(".mei") {
        return mei_coin_file_icon(None);
    }
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let span_class = "inline-flex h-4 w-4 items-center justify-center";
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

fn file_row_icon(node: &WorkspaceNode) -> AnyView {
    let path = node.path.as_str();
    if path.ends_with(".mei") {
        return mei_coin_file_icon(node.mei_kind.as_deref());
    }
    tree_file_icon_for_path(path)
}

fn mei_coin_file_icon(mei_kind: Option<&str>) -> AnyView {
    let (title, fill): (&str, &str) = match mei_kind {
        Some("main") => ("应用入口 main.mei", "#fb7185"),
        Some("scene") => ("scene capsule（正式页面入口）", "#b91c1c"),
        Some("fragment") => ("迁移中的 fragment（需 scene capsule 化）", "#eab308"),
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
    _path: &str,
    selected_scene: Option<&str>,
    file_for_link: Option<&str>,
    active_tab: Option<&str>,
) -> String {
    match route_mode {
        UiRouteMode::App => {
            if let Some(scene) = selected_scene.map(str::trim).filter(|s| !s.is_empty()) {
                let suffix = access_scene_route_suffix(Some(scene), None, None);
                format!("/apps/app/{app_path}{suffix}")
            } else {
                format!("/apps/app/{app_path}")
            }
        }
        UiRouteMode::Presentation => {
            if let Some(scene) = selected_scene.map(str::trim).filter(|s| !s.is_empty()) {
                let suffix = access_scene_route_suffix(Some(scene), None, None);
                format!("/apps/presentation/{app_path}{suffix}")
            } else {
                format!("/apps/presentation/{app_path}")
            }
        }
        UiRouteMode::Build => {
            let parts = super::manage_routing::build_preview_query_parts(
                file_for_link,
                selected_scene,
                active_tab,
                None,
            );
            if parts.is_empty() {
                format!("/apps/build/{app_path}")
            } else {
                format!("/apps/build/{app_path}?{}", parts.join("&"))
            }
        }
        UiRouteMode::Config => format!("/apps/config/{app_path}"),
        UiRouteMode::Upload => {
            let mut parts = Vec::new();
            if let Some(f) = file_for_link {
                let t = f.trim();
                if !t.is_empty() {
                    parts.push(format!("file={}", encode_query_value(t)));
                }
            }
            if parts.is_empty() {
                format!("/apps/upload/{app_path}")
            } else {
                format!("/apps/upload/{app_path}?{}", parts.join("&"))
            }
        }
    }
}

fn is_mei_script_path(path: &str) -> bool {
    path.ends_with(".mei")
}
