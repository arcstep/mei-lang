use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;
use serde::Serialize;

use super::manage_routing::encode_query_value;
use super::route::UiRouteMode;
use super::source_tree::{self, tree_icon_for_upload_entry};
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
    pub modified_ms: Option<u64>,
    pub modified_label: Option<String>,
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

fn upload_entry_size_label(entry: &UploadFileEntry) -> String {
    if entry.is_dir {
        "目录".to_string()
    } else {
        entry.size_bytes
            .map(format_upload_bytes)
            .unwrap_or_else(|| "--".to_string())
    }
}

fn upload_entry_time_label(entry: &UploadFileEntry) -> String {
    entry.modified_label
        .clone()
        .unwrap_or_else(|| "时间未知".to_string())
}

fn upload_parent_rel(path: &str) -> &str {
    path.rsplit_once('/').map(|(parent, _)| parent).unwrap_or("")
}

fn upload_branch_should_open(entry_path: &str, selected: &str) -> bool {
    if selected.is_empty() {
        return false;
    }
    if entry_path == selected {
        return true;
    }
    selected.starts_with(&format!("{entry_path}/"))
}

fn upload_tree_view(
    files: &[UploadFileEntry],
    parent: &str,
    selected: &str,
    app_path: &str,
) -> AnyView {
    let items = files
        .iter()
        .filter(|entry| upload_parent_rel(entry.path.as_str()) == parent)
        .map(|entry| {
            let href = upload_href(app_path, Some(entry.path.as_str()));
            let class = if entry.path == selected {
                "upload-file-row upload-file-row--active"
            } else if entry.is_dir {
                "upload-file-row upload-file-row--dir"
            } else {
                "upload-file-row"
            };
            let icon = tree_icon_for_upload_entry(entry.path.as_str(), entry.is_dir);
            let size_label = upload_entry_size_label(entry);
            let time_label = upload_entry_time_label(entry);
            let entry_kind = if entry.is_dir { "dir" } else { "file" };
            let download_href = if entry.is_dir {
                None
            } else {
                Some(format!(
                    "/api/upload/download/{}?path={}",
                    app_path.trim_start_matches('/'),
                    encode_query_value(entry.path.as_str())
                ))
            };
            let meta = if entry.is_dir {
                view! {
                    <span class="upload-file-time">{time_label.clone()}</span>
                }
                .into_any()
            } else {
                view! {
                    <span class="upload-file-size">{size_label.clone()}</span>
                    <span class="upload-file-dot" aria-hidden="true">"·"</span>
                    <span class="upload-file-time">{time_label.clone()}</span>
                }
                .into_any()
            };
            let aside = if entry.is_dir {
                view! {
                    <span class="upload-file-tag">"目录"</span>
                }
                .into_any()
            } else {
                download_href
                    .map(|href| {
                        view! {
                            <a
                                class="upload-file-tag upload-file-tag--link"
                                href=href
                                download=true
                                title="下载"
                            >
                                "下载"
                            </a>
                        }
                        .into_any()
                    })
                    .unwrap_or_else(|| view! { <></> }.into_any())
            };
            let row = view! {
                <div
                    class=class
                    title=entry.path.clone()
                    data-entry-kind=entry_kind
                    data-entry-path=entry.path.clone()
                    data-entry-name=entry.name.clone()
                    data-entry-size=entry.size_bytes.unwrap_or(0).to_string()
                    data-entry-modified=entry.modified_ms.unwrap_or(0).to_string()
                >
                    <a class="upload-file-body" href=href>
                        <span class="upload-file-leading shrink-0" aria-hidden="true">{icon}</span>
                        <span class="upload-file-text min-w-0">
                            <span class="upload-file-name min-w-0 truncate">{entry.name.clone()}</span>
                            <span class="upload-file-meta">{meta}</span>
                        </span>
                    </a>
                    <span class="upload-file-aside shrink-0">{aside}</span>
                </div>
            };
            if entry.is_dir {
                let branch_open = upload_branch_should_open(entry.path.as_str(), selected);
                let children = upload_tree_view(files, entry.path.as_str(), selected, app_path);
                view! {
                    <li class="tree-node tree-li-branch upload-tree-branch">
                        <details open=branch_open data-upload-branch=entry.path.clone()>
                            <summary class="upload-folder-summary-row">{row}</summary>
                            {children}
                        </details>
                    </li>
                }
                .into_any()
            } else {
                view! {
                    <li class="tree-node upload-file-item">
                        {row}
                    </li>
                }
                .into_any()
            }
        })
        .collect_view();
    view! { <ul class="tree upload-file-list upload-file-tree m-0 grid list-none p-0">{items}</ul> }.into_any()
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
    _source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let selected = selected_file.unwrap_or("");
    let selected_entry = files.iter().find(|entry| entry.path == selected);
    let selected_is_dir = selected_entry.is_some_and(|entry| entry.is_dir);
    let selected_dir = if selected_is_dir {
        selected.to_string()
    } else {
        String::new()
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
        None,
    );
    let file_tree = upload_tree_view(files, "", selected, app_path);
    view! {
        <div class="shell shell-surface upload-view-shell mei-text-primary">
            <div
                id="tree-icons-sprite-root"
                class="pointer-events-none absolute left-0 top-0 -z-10 h-0 w-0 overflow-hidden opacity-0"
                aria-hidden="true"
                inner_html=source_tree::TREE_ICONS_SPRITE_SVG
            ></div>
            {topbar}
            <div
                class="workspace upload-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0"
                id="workspace-root"
                data-app-id=app_path.to_string()
                data-upload-root=upload_root_label.to_string()
            >
                <aside class="upload-sidebar sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-3 py-3">
                    <div class="upload-sidebar-head">
                        <div class="upload-sidebar-title-row">
                            <div class="upload-sidebar-title">"文件清单"</div>
                            <div class="upload-sidebar-root">{upload_root_label}</div>
                        </div>
                        <div class="upload-sidebar-stats">
                            <span class="upload-sidebar-chip">{format!("{file_count} 个文件")}</span>
                            <span class="upload-sidebar-chip">{format!("{dir_count} 个目录")}</span>
                            <span class="upload-sidebar-chip">{format_upload_bytes(total_bytes)}</span>
                        </div>
                    </div>
                    <div class="upload-sidebar-scroll sidebar-scroll flex-1 min-h-0 overflow-auto">
                        {file_tree}
                    </div>
                </aside>
                <div
                    class="splitter splitter-left"
                    data-workspace-splitter="left"
                    role="separator"
                    aria-orientation="vertical"
                    aria-label="调整左侧文件清单宽度"
                >
                    <button
                        class="splitter-toggle"
                        type="button"
                        data-workspace-toggle="left"
                        aria-label="折叠左侧文件清单"
                        title="折叠左侧文件清单"
                    >
                        <span class="splitter-toggle-icon" aria-hidden="true">
                            <svg
                                viewBox="0 0 20 20"
                                width="12"
                                height="12"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.8"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                            >
                                <path d="M12.5 4.5L7.5 10l5 5.5"></path>
                            </svg>
                        </span>
                    </button>
                </div>
                <main class="upload-main main h-full min-h-0 min-w-0 overflow-hidden flex flex-col">
                    <section class="upload-main-pane workspace-panel workspace-panel-main flex min-h-0 flex-1 flex-col overflow-hidden rounded-none border-0 p-0">
                        <div class="upload-workbench-body">
                            <div
                                id="upload-panel-root"
                                class="upload-panel-root"
                                data-app-id=app_path.to_string()
                                data-selected-file=selected.to_string()
                                data-selected-dir=selected_dir
                                data-selected-is-dir=if selected_is_dir { "1" } else { "0" }
                                data-upload-root=upload_root_label.to_string()
                            ></div>
                        </div>
                    </section>
                </main>
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}
