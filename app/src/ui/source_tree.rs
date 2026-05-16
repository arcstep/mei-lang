use leptos::prelude::*;
use mei_lang_kernel::{CompiledEntryMeta, WorkspaceNode};

use super::UiRouteMode;

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
                                <span class="tree-folder-label">{node.name.clone()}</span>
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
                    "tree-link active block rounded-lg bg-blue-600/30 px-2.5 py-2 text-[13px] text-slate-50 transition-colors"
                } else {
                    "tree-link block rounded-lg bg-slate-800/60 px-2.5 py-2 text-[13px] text-slate-300 transition-colors hover:bg-slate-700/70 hover:text-slate-100"
                };
                let preserve_manage_tab = if is_mei_script_path(node.path.as_str()) {
                    "1"
                } else {
                    "0"
                };
                view! {
                    <li class="tree-node">
                        <a class=class href=href data-preserve-manage-tab=preserve_manage_tab>{node.name.clone()}</a>
                    </li>
                }
                .into_any()
            }
        })
        .collect_view();
    view! { <ul class="tree m-0 grid list-none gap-1.5 p-0">{items}</ul> }.into_any()
}

pub(super) fn entry_list_view(
    entries: &[CompiledEntryMeta],
    route_mode: UiRouteMode,
    app_path: &str,
    active_entry: Option<&str>,
    _preview_target: Option<&str>,
    _active_tab: Option<&str>,
) -> AnyView {
    if entries.is_empty() {
        return view! { <></> }.into_any();
    }
    let items = entries
        .iter()
        .map(|entry| {
            let href = format!(
                "/apps/{}/{}?entry={}&target={}",
                route_mode.slug(),
                app_path,
                entry.entry_id,
                entry.target_file
            );
            let class = if active_entry == Some(entry.entry_id.as_str()) {
                "tree-link active block rounded-lg bg-blue-600/30 px-2.5 py-2 text-[13px] text-slate-50 transition-colors"
            } else {
                "tree-link block rounded-lg bg-slate-800/60 px-2.5 py-2 text-[13px] text-slate-300 transition-colors hover:bg-slate-700/70 hover:text-slate-100"
            };
            let label = format!("{} · {}", entry.scene_id, entry.target_file);
            view! {
                <li class="tree-node">
                    <a class=class href=href title=label.clone() data-preserve-manage-tab="1">{label.clone()}</a>
                </li>
            }
        })
        .collect_view();
    view! {
        <section class="source-entry-list mb-3 grid gap-2 border-b border-slate-600/35 pb-3">
            <div class="mb-0.5 grid gap-1">
                <h3 class="m-0 text-[15px] font-semibold text-slate-50">"应用入口"</h3>
                <p class="m-0 text-xs text-slate-400">"scene / entry"</p>
            </div>
            <ul class="tree m-0 grid list-none gap-1.5 p-0">{items}</ul>
        </section>
    }
    .into_any()
}

fn source_href(
    route_mode: UiRouteMode,
    app_path: &str,
    path: &str,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    _active_tab: Option<&str>,
) -> String {
    if is_mei_script_path(path) {
        return format!(
            "/apps/{}/{}?target={}&preview_target={}",
            route_mode.slug(),
            app_path,
            path,
            path
        );
    }
    let mut href = format!("/apps/{}/{}?target={}", route_mode.slug(), app_path, path);
    if let Some(preview_target) = preview_target {
        href.push_str("&preview_target=");
        href.push_str(preview_target);
    } else if let Some(entry) = selected_entry {
        href.push_str("&entry=");
        href.push_str(entry);
    }
    href
}

fn is_mei_script_path(path: &str) -> bool {
    path.ends_with(".mei") || path.ends_with(".star")
}
