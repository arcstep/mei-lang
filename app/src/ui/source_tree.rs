use leptos::prelude::*;
use mei_lang_kernel::WorkspaceNode;

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
) -> AnyView {
    let items = nodes
        .iter()
        .map(|node| {
            if node.kind == "dir" {
                let open = selected_target.starts_with(&format!("{}/", node.path));
                let children = source_tree_view(&node.children, route_mode, app_id, selected_target);
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
                let href = format!("/apps/{}/{}?target={}", route_mode.slug(), app_id, node.path);
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
