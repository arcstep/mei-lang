use leptos::prelude::*;
use mei_lang_kernel::{
    build_experience_path, build_overview_backing, experience_layout_hint, experience_mount_chain,
    format_experience_path, BuildNodeContext, BuildNodeKind, CompiledApp, ProvenanceAnchor,
};

use super::super::view_routing::runtime_href;

pub(crate) fn build_overview_view(
    compiled: &CompiledApp,
    ctx: &BuildNodeContext,
    app_path: &str,
) -> impl IntoView {
    let experience = build_experience_path(compiled, &ctx.node);
    let experience_line = format_experience_path(&experience);
    let backing = build_overview_backing(compiled, &ctx.node);
    let mount_chain = experience_mount_chain(compiled, &ctx.node);
    let layout_hint = experience_layout_hint(compiled, &ctx.node);
    let node_label = ctx.node.encode();
    let diag_count = compiled.diagnostics.len();
    let business_title = experience
        .last()
        .cloned()
        .unwrap_or_else(|| node_label.clone());
    let board_entry = compiled.build_board_index.lookup(&ctx.node);
    let template_entry = compiled.build_template_index.lookup(ctx.node.key.as_str());
    let is_mcg = ctx.node.kind == BuildNodeKind::McgNode;
    let runtime_cross_link = is_mcg.then(|| runtime_href(app_path, None, Some("overview")));

    view! {
        <section class="build-overview build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4 mei-text-body">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <strong class="build-panel-title mei-text-primary">{business_title.clone()}</strong>
                <div class="flex flex-wrap gap-2">
                    <button
                        type="button"
                        class="build-toolbar-btn build-toolbar-btn--accent"
                        data-app-path=app_path.to_string()
                        data-node=node_label.clone()
                        data-tab="overview"
                        data-intent="lock_node"
                    >
                        "复制 Markdown"
                    </button>
                    <button
                        type="button"
                        class="build-toolbar-btn"
                        data-app-path=app_path.to_string()
                        data-node=node_label.clone()
                        data-tab="overview"
                        data-intent="debug_data"
                    >
                        "复制数据调试包"
                    </button>
                </div>
            </div>
            {runtime_cross_link.map(|href| view! {
                <div class="flex flex-col gap-1 rounded-lg border border-white/10 bg-black/10 p-3">
                    <span class="mei-font-2 mei-text-primary">"MCG 编译检查点"</span>
                    <span class="mei-font-1 mei-text-muted">"Materialization 当前态在运行视图观测。"</span>
                    <a class="build-toolbar-btn inline-flex w-fit" href=href>"在运行视图查看 MRG materialization"</a>
                </div>
            })}
            <dl class="grid gap-2">
                {(!is_mcg).then(|| view! {
                    <div class="flex flex-col gap-0.5">
                        <dt class="mei-text-muted">"体验路径"</dt>
                        <dd class="mei-font-2 mei-text-primary">{experience_line}</dd>
                    </div>
                })}
                {(!backing.is_empty()).then(|| view! {
                    <div class="flex flex-col gap-0.5">
                        <dt class="mei-text-muted">"Backing"</dt>
                        <dd class="font-mono mei-font-1 leading-5">
                            {backing.into_iter().map(|item| view! {
                                <div>{item}</div>
                            }).collect_view()}
                        </dd>
                    </div>
                })}
                {(!mount_chain.is_empty()).then(|| view! {
                    <div class="flex flex-col gap-0.5">
                        <dt class="mei-text-muted">"挂载链"</dt>
                        <dd class="font-mono mei-font-1 leading-5">
                            {mount_chain.into_iter().map(|entry| view! {
                                <div>{format!("{}#{} ({})", entry.file, entry.panel_id, entry.role)}</div>
                            }).collect_view()}
                        </dd>
                    </div>
                })}
                {layout_hint.clone().map(|hint| view! {
                    <div class="flex flex-col gap-0.5">
                        <dt class="mei-text-muted">"布局"</dt>
                        <dd class="font-mono mei-font-1 break-all">{hint}</dd>
                    </div>
                })}
                {board_entry.map(|entry| view! {
                    <>
                        {entry.layout_mode.clone().map(|mode| view! {
                            <div class="flex gap-2">
                                <dt class="w-16 shrink-0 mei-text-muted">"Board 模式"</dt>
                                <dd>{mode}</dd>
                            </div>
                        })}
                        {entry.params_summary.clone().map(|params| view! {
                            <div class="flex gap-2">
                                <dt class="w-16 shrink-0 mei-text-muted">"Params"</dt>
                                <dd class="font-mono mei-font-1">{params}</dd>
                            </div>
                        })}
                        {(!entry.slots.is_empty()).then(|| view! {
                            <div class="flex flex-col gap-0.5">
                                <dt class="mei-text-muted">"Slots"</dt>
                                <dd class="font-mono mei-font-1 leading-5">
                                    {entry.slots.iter().map(|slot| view! {
                                        <div>
                                            {format!(
                                                "{} — {}",
                                                slot.slot_id,
                                                slot.component.clone().unwrap_or_else(|| "slot".to_string())
                                            )}
                                        </div>
                                    }).collect_view()}
                                </dd>
                            </div>
                        })}
                    </>
                })}
                {template_entry.map(|entry| view! {
                    <>
                        <div class="flex gap-2">
                            <dt class="w-16 shrink-0 mei-text-muted">"类别"</dt>
                            <dd>{entry.category.clone()}</dd>
                        </div>
                        <div class="flex gap-2">
                            <dt class="w-16 shrink-0 mei-text-muted">"模板文件"</dt>
                            <dd class="font-mono mei-font-1 break-all">{entry.template_file.clone()}</dd>
                        </div>
                        {(!entry.props_schema.is_empty()).then(|| view! {
                            <div class="flex flex-col gap-0.5">
                                <dt class="mei-text-muted">"Props 契约"</dt>
                                <dd class="font-mono mei-font-1 leading-5">
                                    {entry.props_schema.iter().map(|item| view! { <div>{item.clone()}</div> }).collect_view()}
                                </dd>
                            </div>
                        })}
                        {(!entry.consumers.is_empty()).then(|| view! {
                            <div class="flex flex-col gap-0.5">
                                <dt class="mei-text-muted">"Consumers"</dt>
                                <dd class="font-mono mei-font-1 leading-5">
                                    {entry.consumers.iter().map(|item| view! { <div>{item.clone()}</div> }).collect_view()}
                                </dd>
                            </div>
                        })}
                        {entry.agent_hint.clone().map(|hint| view! {
                            <div class="flex flex-col gap-0.5">
                                <dt class="mei-text-muted">"Agent 提示"</dt>
                                <dd class="mei-font-1 leading-5">{hint}</dd>
                            </div>
                        })}
                    </>
                })}
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"Node"</dt>
                    <dd class="font-mono mei-font-1 break-all">{node_label.clone()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"Target"</dt>
                    <dd class="font-mono mei-font-1 break-all">{ctx.target_file.clone()}</dd>
                </div>
                {ctx.scene_id.clone().map(|scene| view! {
                    <div class="flex gap-2">
                        <dt class="w-16 shrink-0 mei-text-muted">"Scene"</dt>
                        <dd>{scene}</dd>
                    </div>
                })}
                {matches!(
                    ctx.node.kind,
                    BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock
                ).then(|| view! {
                    <div class="flex gap-2">
                        <dt class="w-16 shrink-0 mei-text-muted">"符号"</dt>
                        <dd class="font-mono mei-font-1">{ctx.provenance.symbol_id.clone()}</dd>
                    </div>
                })}
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"Diagnostics"</dt>
                    <dd>{diag_count.to_string()}</dd>
                </div>
            </dl>
            <p class="mei-text-muted">"选择左侧体验树节点后，在此查看业务路径与 backing；Copy 输出给外部 Agent。"</p>
            <div
                id="build-overview-gate"
                data-app-path=app_path.to_string()
                data-node=node_label
            ></div>
        </section>
    }
}

pub(crate) fn build_provenance_view(anchor: &ProvenanceAnchor) -> impl IntoView {
    let encoded = anchor.encode();
    view! {
        <section class="build-provenance build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4">
            <strong class="build-panel-title mei-text-primary">"溯源"</strong>
            <dl class="grid gap-2">
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"文件"</dt>
                    <dd class="font-mono mei-font-1 break-all">{anchor.file.clone()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"符号"</dt>
                    <dd class="font-mono mei-font-1">{anchor.symbol_id.clone()}</dd>
                </div>
                <div class="flex gap-2">
                    <dt class="w-16 shrink-0 mei-text-muted">"类型"</dt>
                    <dd>{anchor.symbol_kind.clone()}</dd>
                </div>
            </dl>
            <button
                type="button"
                class="build-toolbar-btn build-copy-provenance"
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
        <section class="build-agent-panel build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4">
            <div class="flex flex-wrap items-center gap-2">
                <strong class="build-panel-title mei-text-primary">"Agent 上下文"</strong>
                <button
                    type="button"
                    id="build-copy-agent-context"
                    class="build-toolbar-btn build-toolbar-btn--accent"
                    data-app-path=app_path.to_string()
                    data-node=node.to_string()
                    data-tab=tab.to_string()
                    data-intent="full"
                >
                    "复制 Markdown 简报"
                </button>
            </div>
            <pre
                id="build-agent-context-preview"
                class="build-panel-pre max-h-96 overflow-auto p-3 font-mono mei-font-1 leading-5 mei-text-muted"
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
            class="build-artifact-panel build-panel-shell grid gap-3 rounded-xl border mei-border-default mei-surface-panel-muted p-4"
            data-app-path=app_path.to_string()
            data-node=node.to_string()
        >
            <strong class="build-panel-title mei-text-primary">"产物"</strong>
            <div id="build-artifact-summary" class="mei-text-muted">"扫描 .mei/ 产物…"</div>
        </section>
    }
}
