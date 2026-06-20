use leptos::prelude::*;
use mei_lang_kernel::{BuildNodeContext, CompiledApp, ProvenanceAnchor};

pub(crate) fn build_overview_view(
    compiled: &CompiledApp,
    ctx: &BuildNodeContext,
    app_path: &str,
) -> impl IntoView {
    let node_label = ctx.node.encode();
    let diag_count = compiled.diagnostics.len();
    let route_count = compiled.scene_routes.len();
    let resource_count = compiled.resources.len();
    view! {
        <section class="build-overview grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 text-xs leading-6 mei-text-body">
            <strong class="text-sm mei-text-primary">"概览"</strong>
            <dl class="grid gap-1">
                <div class="flex gap-2">
                    <dt class="mei-text-muted">"Node"</dt>
                    <dd class="font-mono text-[11px]">{node_label}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="mei-text-muted">"Target"</dt>
                    <dd>{ctx.target_file.clone()}</dd>
                </div>
                {ctx.scene_id.clone().map(|scene| view! {
                    <div class="flex gap-2">
                        <dt class="mei-text-muted">"Scene"</dt>
                        <dd>{scene}</dd>
                    </div>
                })}
                <div class="flex gap-2">
                    <dt class="mei-text-muted">"Routes"</dt>
                    <dd>{route_count.to_string()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="mei-text-muted">"Resources"</dt>
                    <dd>{resource_count.to_string()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="mei-text-muted">"Diagnostics"</dt>
                    <dd>{diag_count.to_string()}</dd>
                </div>
            </dl>
            <p class="mei-text-muted">"编译后结构摘要；详细 diagnostics 请在 IDE / CI 中查看。"</p>
            <div
                id="build-overview-gate"
                data-app-path=app_path.to_string()
                data-node=ctx.node.encode()
            ></div>
        </section>
    }
}

pub(crate) fn build_provenance_view(anchor: &ProvenanceAnchor) -> impl IntoView {
    let encoded = anchor.encode();
    view! {
        <section class="build-provenance grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 text-xs leading-6">
            <strong class="text-sm mei-text-primary">"溯源"</strong>
            <dl class="grid gap-2">
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"文件"</dt>
                    <dd class="font-mono text-[11px] break-all">{anchor.file.clone()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"符号"</dt>
                    <dd class="font-mono text-[11px]">{anchor.symbol_id.clone()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"类型"</dt>
                    <dd>{anchor.symbol_kind.clone()}</dd>
                </div>
            </dl>
            <button
                type="button"
                class="build-copy-provenance inline-flex w-fit items-center rounded-md border mei-border-default px-2.5 py-1 text-[11px] mei-text-body hover:mei-text-inverse"
                data-copy-text=encoded
            >
                "复制 file#symbol"
            </button>
            <p class="mei-text-muted">"在 Cursor / Codex 等 IDE 中打开上述文件并搜索符号 id。"</p>
        </section>
    }
}

pub(crate) fn build_agent_view(app_path: &str, node: &str, tab: &str) -> impl IntoView {
    view! {
        <section class="build-agent-panel grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 text-xs leading-6">
            <div class="flex flex-wrap items-center gap-2">
                <strong class="text-sm mei-text-primary">"Agent 上下文"</strong>
                <button
                    type="button"
                    id="build-copy-agent-context"
                    class="inline-flex items-center rounded-md border border-sky-500/40 bg-sky-500/10 px-2.5 py-1 text-[11px] text-sky-100"
                    data-app-path=app_path.to_string()
                    data-node=node.to_string()
                    data-tab=tab.to_string()
                    data-intent="lock_node"
                >
                    "复制 Markdown 简报"
                </button>
            </div>
            <pre
                id="build-agent-context-preview"
                class="max-h-96 overflow-auto rounded-lg bg-black/30 p-3 font-mono text-[11px] leading-5 mei-text-muted"
                data-app-path=app_path.to_string()
                data-node=node.to_string()
                data-tab=tab.to_string()
            >"加载中…"</pre>
        </section>
    }
}

pub(crate) fn build_exec_panel_shell(app_path: &str, node: &str) -> impl IntoView {
    view! {
        <section
            id="build-exec-panel"
            class="build-exec-panel grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 text-xs leading-6"
            data-app-path=app_path.to_string()
            data-node=node.to_string()
        >
            <strong class="text-sm mei-text-primary">"执行"</strong>
            <div class="flex flex-wrap gap-2" id="build-exec-scope-buttons">
                <button type="button" class="build-exec-scope is-active" data-scope="warmup">"warmup"</button>
                <button type="button" class="build-exec-scope" data-scope="empty">"empty"</button>
                <button type="button" class="build-exec-scope" data-scope="last_request">"last_request"</button>
            </div>
            <button
                type="button"
                id="build-exec-run"
                class="inline-flex w-fit items-center rounded-md border mei-border-default px-2.5 py-1 text-[11px]"
            >
                "运行"
            </button>
            <pre id="build-exec-output" class="max-h-80 overflow-auto rounded-lg bg-black/30 p-3 font-mono text-[11px]"></pre>
        </section>
    }
}

pub(crate) fn build_graph_panel(title: &'static str, graph_kind: &'static str, node: &str) -> impl IntoView {
    view! {
        <section
            class="build-graph-panel grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 text-xs leading-6"
            data-graph-kind=graph_kind
            data-node=node.to_string()
        >
            <strong class="text-sm mei-text-primary">{title}</strong>
            <pre
                class="build-graph-markdown max-h-96 overflow-auto rounded-lg bg-black/30 p-3 font-mono text-[11px] leading-5 mei-text-muted"
                data-graph-kind=graph_kind
                data-node=node.to_string()
            >"加载图摘要…"</pre>
        </section>
    }
}

pub(crate) fn build_artifact_panel(app_path: &str, node: &str) -> impl IntoView {
    view! {
        <section
            id="build-artifact-panel"
            class="build-artifact-panel grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 text-xs leading-6"
            data-app-path=app_path.to_string()
            data-node=node.to_string()
        >
            <strong class="text-sm mei-text-primary">"产物"</strong>
            <div id="build-artifact-summary" class="mei-text-muted">"扫描 .mei/ 产物…"</div>
        </section>
    }
}
