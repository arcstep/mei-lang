use leptos::prelude::*;

pub(crate) fn build_exec_panel_shell(app_path: &str, node: &str) -> impl IntoView {
    view! {
        <section
            id="build-exec-panel"
            class="build-exec-panel build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4"
            data-app-path=app_path.to_string()
            data-node=node.to_string()
        >
            <strong class="build-panel-title mei-text-primary">"执行"</strong>
            <div class="flex flex-wrap gap-2" id="build-exec-scope-buttons">
                <button type="button" class="build-exec-scope is-active" data-scope="warmup">"warmup"</button>
                <button type="button" class="build-exec-scope" data-scope="empty">"empty"</button>
                <button type="button" class="build-exec-scope" data-scope="last_request">"last_request"</button>
            </div>
            <button
                type="button"
                id="build-exec-run"
                class="build-toolbar-btn build-exec-run"
            >
                "运行"
            </button>
            <pre id="build-exec-output" class="build-panel-pre max-h-80 overflow-auto p-3 font-mono mei-font-1"></pre>
        </section>
    }
}

pub(crate) fn build_graph_panel(
    title: &'static str,
    graph_kind: &'static str,
    node: &str,
) -> impl IntoView {
    view! {
        <section
            class="build-graph-panel build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4"
            data-graph-kind=graph_kind
            data-node=node.to_string()
        >
            <strong class="build-panel-title mei-text-primary">{title}</strong>
            <pre
                class="build-graph-markdown build-panel-pre max-h-96 overflow-auto p-3 font-mono mei-font-1 leading-5 mei-text-muted"
            >"加载图摘要…"</pre>
        </section>
    }
}

pub(crate) fn build_artifact_panel(app_path: &str, node: &str) -> impl IntoView {
    view! {
        <section
            id="build-artifact-panel"
            class="build-artifact-panel build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4"
            data-app-path=app_path.to_string()
            data-node=node.to_string()
        >
            <strong class="build-panel-title mei-text-primary">"产物"</strong>
            <div id="build-artifact-summary" class="mei-text-muted">"扫描 .mei/ 产物…"</div>
        </section>
    }
}
