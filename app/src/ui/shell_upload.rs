use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;
use serde::Serialize;

use super::preview_chrome::asset_preview_body;
use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::topbar_view;
use super::view_routing::upload_href;
use super::{HostAccountView, SourcePanelMeta, TopbarMenuContext};

#[derive(Debug, Clone, Serialize)]
pub struct UploadFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
}

fn format_upload_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

fn upload_entry_depth(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .count()
        .saturating_sub(1)
}

fn upload_file_token(name: &str) -> (&'static str, &'static str) {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" | "svg" => ("image", "IMG"),
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => ("video", "VID"),
        "mp3" | "wav" | "flac" | "aac" | "m4a" => ("audio", "AUD"),
        "csv" | "xlsx" | "xls" => ("sheet", "CSV"),
        "json" | "jsonc" | "yaml" | "yml" | "toml" => ("data", "JSON"),
        "js" | "jsx" | "mjs" | "cjs" => ("code", "JS"),
        "ts" | "tsx" => ("code", "TS"),
        "css" | "scss" | "less" => ("code", "CSS"),
        "md" | "markdown" | "txt" => ("doc", "TXT"),
        "pdf" => ("doc", "PDF"),
        "zip" | "tar" | "gz" | "rar" | "7z" => ("archive", "ZIP"),
        _ => ("file", "FILE"),
    }
}

fn upload_entry_icon(entry: &UploadFileEntry) -> AnyView {
    let (kind, token) = if entry.is_dir {
        ("dir", "DIR")
    } else {
        upload_file_token(entry.name.as_str())
    };
    view! {
        <span class="upload-entry-token shrink-0" data-kind=kind aria-hidden="true">
            {token}
        </span>
    }
    .into_any()
}

pub(crate) fn upload_shell(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    upload_enabled: bool,
    access_scene: Option<&str>,
    upload_root_label: &str,
    files: &[UploadFileEntry],
    selected_file: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let selected = selected_file.unwrap_or("");
    let selected_entry = files.iter().find(|entry| entry.path == selected);
    let selected_is_dir = selected_entry.is_some_and(|entry| entry.is_dir);
    let selected_dir = if selected.is_empty() {
        String::new()
    } else if selected_is_dir {
        selected.to_string()
    } else {
        selected
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_string())
            .unwrap_or_default()
    };
    let target_dir_label = if selected_dir.is_empty() {
        upload_root_label.to_string()
    } else {
        format!("{upload_root_label}/{selected_dir}")
    };
    let file_count = files.iter().filter(|entry| !entry.is_dir).count();
    let dir_count = files.iter().filter(|entry| entry.is_dir).count();
    let total_bytes = files.iter().filter_map(|entry| entry.size_bytes).sum::<u64>();
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Upload,
        access_scene,
        None,
        None,
        upload_enabled,
        false,
        auth_enabled,
        auth_account,
    );
    let statusbar = statusbar_view(
        app_path,
        app_title,
        UiRouteMode::Upload.slug(),
        if selected.is_empty() {
            upload_root_label
        } else {
            selected
        },
        source_meta,
        None,
        false,
        false,
    );
    let file_links = files
        .iter()
        .map(|entry| {
            let href = upload_href(app_path, Some(entry.path.as_str()));
            let class = if entry.path == selected {
                "upload-file-row upload-file-row--active"
            } else {
                "upload-file-row"
            };
            let icon = upload_entry_icon(entry);
            let meta = if entry.is_dir {
                "目录".to_string()
            } else {
                entry.size_bytes
                    .map(format_upload_bytes)
                    .unwrap_or_else(|| "文件".to_string())
            };
            let row_style = format!("padding-left:{}px", 12 + upload_entry_depth(&entry.path) * 14);
            view! {
                <li class="tree-node">
                    <a class=class href=href title=entry.path.clone() style=row_style>
                        {icon}
                        <span class="upload-file-copy min-w-0 flex-1">
                            <span class="upload-file-name min-w-0 truncate">{entry.name.clone()}</span>
                            <span class="upload-file-meta">{meta}</span>
                        </span>
                    </a>
                </li>
            }
        })
        .collect_view();
    let preview = if selected.is_empty() || selected_is_dir {
        let title = if selected_is_dir {
            "目录已选中"
        } else {
            "准备上传"
        };
        let note = if selected_is_dir {
            format!(
                "当前目录：{target_dir_label}。选择目录中的文件即可预览；上传的新文件也会直接落在这里。"
            )
        } else {
            format!(
                "当前目录：{target_dir_label}。从左侧选择上传目录中的文件，或使用下方上传面板添加新文件。"
            )
        };
        view! {
            <section class="upload-empty-state upload-empty-state--hero flex flex-1 items-center justify-center rounded-xl border border-dashed border-slate-600/55 bg-slate-900/35 p-8 text-sm text-slate-400">
                <div class="upload-empty-state-copy">
                    <div class="upload-empty-state-title">{title}</div>
                    <div class="upload-empty-state-note">{note}</div>
                </div>
            </section>
        }
        .into_any()
    } else {
        view! {
            <section class="upload-preview-pane flex min-h-0 flex-1 flex-col overflow-hidden">
                <div class="main-pane-scroll flex-1 min-h-0 overflow-auto p-0">
                    {asset_preview_body(
                        app_path,
                        selected,
                        source.unwrap_or(""),
                    )}
                </div>
            </section>
        }
        .into_any()
    };
    view! {
        <div class="shell shell-surface upload-view-shell text-slate-200">
            {topbar}
            <div
                class="upload-workspace chrome-inset min-h-0 flex flex-1 overflow-hidden"
                id="upload-workspace-root"
                data-app-id=app_path.to_string()
                data-upload-root=upload_root_label.to_string()
            >
                <aside class="upload-sidebar workspace-panel workspace-panel-side h-full min-h-0 min-w-0 w-72 shrink-0 overflow-hidden border-r border-slate-700/40 px-3 py-3">
                    <div class="upload-sidebar-head">
                        <div class="upload-sidebar-title-row">
                            <div class="upload-sidebar-title">"上传目录"</div>
                            <div class="upload-sidebar-root">{upload_root_label}</div>
                        </div>
                        <div class="upload-sidebar-note">
                            "支持滚动浏览已上传文件；新文件默认上传到当前选中文件所在目录。"
                        </div>
                        <div class="upload-sidebar-stats">
                            <span class="upload-sidebar-chip">{format!("{file_count} 个文件")}</span>
                            <span class="upload-sidebar-chip">{format!("{dir_count} 个目录")}</span>
                            <span class="upload-sidebar-chip">{format_upload_bytes(total_bytes)}</span>
                        </div>
                    </div>
                    <div class="upload-sidebar-scroll">
                        <ul class="tree upload-file-list m-0 grid list-none gap-1 p-0">{file_links}</ul>
                    </div>
                    <div
                        id="upload-panel-root"
                        class="upload-panel-root"
                        data-app-id=app_path.to_string()
                        data-selected-file=selected.to_string()
                        data-selected-dir=selected_dir
                        data-selected-is-dir=if selected_is_dir { "1" } else { "0" }
                        data-upload-root=upload_root_label.to_string()
                    ></div>
                </aside>
                <main class="upload-main min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden px-4 py-3">
                    {preview}
                </main>
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}
