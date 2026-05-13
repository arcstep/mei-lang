use leptos::prelude::*;
use mei_lang_kernel::{CompiledEntryMeta, WorkspaceNode};

use super::UiRouteMode;

pub(super) fn controls_view() -> impl IntoView {
    view! {
        <div class="tree-toolbar">
            <button class="tree-toolbar-btn" type="button" data-tree-expand="1">"展开"</button>
            <button class="tree-toolbar-btn" type="button" data-tree-collapse="1">"收起"</button>
        </div>
    }
}

pub(super) fn source_tree_view(
    nodes: &[WorkspaceNode],
    route_mode: UiRouteMode,
    app_id: &str,
    selected_target: &str,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
) -> AnyView {
    let items = nodes
        .iter()
        .map(|node| {
            if node.kind == "dir" {
                let open = selected_target.starts_with(&format!("{}/", node.path));
                let children = source_tree_view(
                    &node.children,
                    route_mode,
                    app_id,
                    selected_target,
                    selected_entry,
                    preview_target,
                );
                view! {
                    <li class="tree-node tree-li-branch">
                        <details open=open>
                            <summary class="tree-folder-summary">
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
                    app_id,
                    node.path.as_str(),
                    selected_entry,
                    preview_target,
                );
                let class = if node.path == selected_target {
                    "tree-link active"
                } else {
                    "tree-link"
                };
                view! {
                    <li class="tree-node">
                        <a class=class href=href>{node.name.clone()}</a>
                    </li>
                }
                .into_any()
            }
        })
        .collect_view();
    view! { <ul class="tree">{items}</ul> }.into_any()
}

pub(super) fn entry_list_view(
    entries: &[CompiledEntryMeta],
    route_mode: UiRouteMode,
    app_id: &str,
    active_entry: Option<&str>,
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
                app_id,
                entry.entry_id,
                entry.target_file
            );
            let class = if active_entry == Some(entry.entry_id.as_str()) {
                "tree-link active"
            } else {
                "tree-link"
            };
            let label = format!("{} · {}", entry.scene_id, entry.target_file);
            view! {
                <li class="tree-node">
                    <a class=class href=href title=label.clone()>{label.clone()}</a>
                </li>
            }
        })
        .collect_view();
    view! {
        <section class="source-entry-list">
            <div class="panel-heading">
                <h3>"应用入口"</h3>
                <p>"scene / entry"</p>
            </div>
            <ul class="tree">{items}</ul>
        </section>
    }
    .into_any()
}

fn source_href(
    route_mode: UiRouteMode,
    app_id: &str,
    path: &str,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
) -> String {
    if path.ends_with(".mei") {
        return format!(
            "/apps/{}/{}?target={}&preview_target={}",
            route_mode.slug(),
            app_id,
            path,
            path
        );
    }
    let mut href = format!("/apps/{}/{}?target={}", route_mode.slug(), app_id, path);
    if let Some(preview_target) = preview_target {
        href.push_str("&preview_target=");
        href.push_str(preview_target);
    } else if let Some(entry) = selected_entry {
        href.push_str("&entry=");
        href.push_str(entry);
    }
    href
}
