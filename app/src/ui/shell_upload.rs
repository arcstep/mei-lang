use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;
use serde::Serialize;

use super::preview_chrome::asset_preview_body;
use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::topbar_view;
use super::view_routing::upload_href;
use super::{SourcePanelMeta, TopbarMenuContext};

#[derive(Debug, Clone, Serialize)]
pub struct UploadFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

pub(super) fn upload_shell(
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
) -> AnyView {
    let selected = selected_file.unwrap_or("");
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
                "tree-link tree-link--active flex min-w-0 w-full items-center gap-1.5 border-l-2 border-sky-400 bg-sky-500/15 py-0.5 pl-2 pr-1 text-[13px] font-medium text-sky-100 transition-colors"
            } else {
                "tree-link flex min-w-0 w-full items-center gap-1.5 border-l-2 border-transparent py-0.5 pl-2 pr-1 text-[13px] text-slate-300 transition-colors hover:text-slate-100"
            };
            let icon = if entry.is_dir { "📁" } else { "📄" };
            view! {
                <li class="tree-node">
                    <a class=class href=href title=entry.path.clone()>
                        <span class="shrink-0" aria-hidden="true">{icon}</span>
                        <span class="min-w-0 flex-1 truncate">{entry.name.clone()}</span>
                    </a>
                </li>
            }
        })
        .collect_view();
    let preview = if selected.is_empty() {
        view! {
            <section class="upload-empty-state flex flex-1 items-center justify-center rounded-xl border border-dashed border-slate-600/55 bg-slate-900/35 p-8 text-sm text-slate-400">
                "从左侧选择上传目录中的文件，或使用下方表单上传新文件。"
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
                    <div class="mb-3 text-[11px] font-medium uppercase tracking-wide text-slate-400">
                        {format!("上传目录 · {upload_root_label}")}
                    </div>
                    <ul class="tree m-0 grid list-none gap-0.5 p-0">{file_links}</ul>
                    <div
                        id="upload-panel-root"
                        class="upload-panel-root mt-4 border-t border-slate-700/40 pt-4"
                        data-app-id=app_path.to_string()
                        data-selected-file=selected.to_string()
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
