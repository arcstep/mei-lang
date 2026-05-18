use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::UiRouteMode;

pub(super) fn panel_view(
    compiled: &CompiledApp,
    app_path: &str,
    route_mode: UiRouteMode,
    selected_target: &str,
    source_views_enabled: bool,
    active_tab: &str,
    allow_ask_mode: bool,
    allow_build_mode: bool,
    default_agent_mode: &str,
    history_actions_enabled: bool,
) -> impl IntoView {
    let allowed_modes = match (allow_ask_mode, allow_build_mode) {
        (true, true) => "ask,build",
        (true, false) => "ask",
        (false, true) => "build",
        (false, false) => "build",
    };
    let default_agent_mode = if allow_ask_mode && !allow_build_mode {
        "ask".to_string()
    } else if !allow_ask_mode && allow_build_mode {
        "build".to_string()
    } else if matches!(default_agent_mode, "ask" | "build") {
        default_agent_mode.to_string()
    } else {
        "build".to_string()
    };
    let active_scene = compiled
        .active_scene
        .clone()
        .or_else(|| {
            compiled
                .scene_routes
                .iter()
                .find(|item| item.target_file == compiled.active_target_file)
                .map(|item| item.scene_id.clone())
                .or_else(|| {
                    compiled
                        .scene_routes
                        .first()
                        .map(|item| item.scene_id.clone())
                })
        })
        .unwrap_or_default();
    let active_scene_target = compiled
        .scene_routes
        .iter()
        .find(|item| item.scene_id == active_scene)
        .map(|item| item.target_file.clone())
        .unwrap_or_else(|| compiled.active_target_file.clone());
    let contract_scene_id = compiled
        .scene_contract
        .as_ref()
        .map(|item| item.scene.id.clone())
        .unwrap_or_default();
    view! {
        <section class="author-panel-section h-full min-h-0">
            <div
                id="meilang-author-panel"
                class="author-panel flex h-full min-h-0 flex-col gap-1.5 overflow-hidden"
                data-app=app_path.to_string()
                data-file=selected_target.to_string()
                data-scene=active_scene
                data-scene-target=active_scene_target
                data-contract-scene=contract_scene_id
                data-mode=route_mode.slug()
                data-allowed-modes=allowed_modes
                data-default-agent-mode=default_agent_mode
                data-history-actions=if history_actions_enabled { "true" } else { "false" }
                data-source-views=if source_views_enabled { "true" } else { "false" }
                data-view-tab=active_tab.to_string()
            >
                <div class="author-top-row author-surface-top sticky top-0 z-[2] flex flex-nowrap items-center justify-between gap-1 overflow-x-auto px-1.5 py-1.5">
                    <div class="author-top-actions flex min-w-0 flex-nowrap items-center gap-1">
                        <div class="author-history-controls inline-flex items-center gap-1">
                            <sl-select
                                class="max-w-[200px]"
                                id="author-session-select"
                                title="历史对话"
                                size="small"
                                value=""
                                hoist=true
                            >
                                <sl-option value="">"历史"</sl-option>
                            </sl-select>
                        </div>
                        <sl-button
                            class="author-btn inline-flex items-center justify-center border-0 bg-transparent p-0 text-xs font-bold"
                            id="author-reconnect-btn"
                            size="small"
                            title="重连服务"
                            aria-label="重连服务"
                            hidden=true
                        >
                            "重连"
                        </sl-button>
                    </div>
                    <div class="author-top-actions author-top-actions-right ml-auto shrink-0">
                        <sl-button
                            class="author-btn author-btn-icon inline-flex min-w-7 items-center justify-center border-0 bg-transparent px-1.5 py-1 text-[11px] font-bold leading-none text-center"
                            id="author-session-btn"
                            size="small"
                            circle=true
                            title="新建对话"
                            aria-label="新建对话"
                        >
                            "+"
                        </sl-button>
                    </div>
                </div>
                <section class="author-chat-section author-surface-chat flex min-h-0 flex-1 flex-col gap-1.5 overflow-hidden p-1.5">
                    <div class="author-progress-strip author-surface-progress grid gap-1 px-2 py-1.5" id="author-progress-strip" hidden>
                        <div class="author-progress-main flex min-w-0 flex-wrap items-center gap-1.5">
                            <span class="author-progress-label text-xs font-bold tracking-[0.01em] text-slate-100" id="author-progress-label">
                                "准备中"
                            </span>
                            <span class="author-progress-detail text-[11px] leading-5 text-blue-300" id="author-progress-detail"></span>
                        </div>
                        <div class="author-progress-items flex flex-wrap gap-1" id="author-progress-items"></div>
                    </div>
                    <div
                        id="author-context-preview"
                        class="author-context-preview min-h-0 overflow-hidden rounded-xl border border-slate-700/55 bg-slate-950/35 px-2 py-1.5"
                    >
                        <details class="grid gap-1">
                            <summary class="cursor-pointer text-[10px] font-bold tracking-[0.02em] text-slate-300">"上下文预期"</summary>
                            <div
                                class="mt-1 grid min-h-0 gap-1 pr-1"
                                style="max-height:50vh;min-height:10rem;overflow-y:auto;overscroll-behavior:contain;"
                            >
                                <div class="flex flex-wrap items-center justify-between gap-2">
                                    <sl-button
                                        class="author-history-btn text-[10px] font-bold tracking-[0.02em]"
                                        id="author-context-refresh-btn"
                                        title="刷新上下文预期"
                                        size="small"
                                    >
                                        "刷新"
                                    </sl-button>
                                </div>
                                <div class="flex flex-wrap items-center gap-2 text-[10px] text-slate-300">
                                    <span class="shrink-0 font-bold tracking-[0.02em] text-slate-400">
                                        "引用可见"
                                    </span>
                                    <sl-select
                                        id="author-resource-visibility-select"
                                        class="min-w-[140px] max-w-[220px]"
                                        size="small"
                                        placeholder="自动(route+mode)"
                                        hoist=true
                                    >
                                        <sl-option value="">"自动"</sl-option>
                                        <sl-option value="local_only">"仅当前入口"</sl-option>
                                        <sl-option value="allow_direct_refs">"直接引用"</sl-option>
                                        <sl-option value="allow_scene_reachable">"场景可达"</sl-option>
                                    </sl-select>
                                </div>
                                <div id="author-context-preview-scope" class="text-[10px] text-slate-400"></div>
                                <div id="author-context-preview-skill" class="text-[10px] text-slate-400"></div>
                                <details class="rounded-lg border border-slate-700/60 bg-slate-900/45 px-2 py-1">
                                    <summary class="cursor-pointer text-[10px] font-bold text-slate-300">"可用工具"</summary>
                                    <pre id="author-context-preview-tools" class="mt-1 whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-slate-200"></pre>
                                </details>
                                <details class="rounded-lg border border-slate-700/60 bg-slate-900/45 px-2 py-1">
                                    <summary class="cursor-pointer text-[10px] font-bold text-slate-300">"资源树（按类型）"</summary>
                                    <div id="author-context-preview-inventory" class="mt-1 grid gap-1 text-[10px] text-slate-200"></div>
                                </details>
                                <details class="rounded-lg border border-slate-700/60 bg-slate-900/45 px-2 py-1">
                                    <summary class="cursor-pointer text-[10px] font-bold text-slate-300">"提示语注入预览"</summary>
                                    <pre id="author-context-preview-prompt" class="mt-1 min-h-0 whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-slate-200"></pre>
                                </details>
                                <details class="rounded-lg border border-slate-700/60 bg-slate-900/45 px-2 py-1">
                                    <summary class="cursor-pointer text-[10px] font-bold text-slate-300">"Delta（srv / cli_rx / cli_paint；与管理页「调试」同步）"</summary>
                                    <pre
                                        id="author-context-preview-delta-debug"
                                        class="mt-1 min-h-0 whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-slate-200"
                                    ></pre>
                                </details>
                            </div>
                        </details>
                    </div>
                    <div class="author-chat-log grid min-h-0 flex-1 content-start gap-1.5 overflow-auto pr-0 [grid-auto-rows:max-content]" id="author-chat-log"></div>
                    <div class="author-composer-row author-surface-composer-row sticky bottom-0 flex flex-col gap-1 pt-0">
                        <div class="author-composer-tool-rail flex min-h-0 items-center justify-end gap-1 pr-1">
                            {if source_views_enabled {
                                view! {
                                    <sl-button
                                        class="author-history-btn author-history-btn-diff text-[10px] font-bold tracking-[0.02em]"
                                        id="source-view-diff-btn"
                                        data-view-mode="diff"
                                        title="查看最后一轮 Build 差异"
                                        size="small"
                                        disabled=true
                                    >
                                        "修改"
                                    </sl-button>
                                }
                                    .into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
                            {if history_actions_enabled {
                                view! {
                                    <>
                                        <sl-button
                                            class="author-history-btn author-history-btn-icon text-[10px] font-bold tracking-[0.02em]"
                                            id="author-undo-btn"
                                            title="撤回本轮代码修改"
                                            size="small"
                                            disabled=true
                                        >
                                            "Undo"
                                        </sl-button>
                                        <sl-button
                                            class="author-history-btn author-history-btn-icon text-[10px] font-bold tracking-[0.02em]"
                                            id="author-redo-btn"
                                            title="恢复最近撤回的代码修改"
                                            size="small"
                                            disabled=true
                                        >
                                            "Redo"
                                        </sl-button>
                                    </>
                                }
                                    .into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
                        </div>
                        <div class="author-composer-shell author-surface-composer flex flex-col gap-1.5 p-2">
                            <div class="author-composer-body min-h-0">
                                <textarea
                                    id="author-intent-input"
                                    rows="2"
                                    placeholder="输入并发送"
                                ></textarea>
                            </div>
                            <div class="author-composer-footer flex items-center justify-between gap-1.5">
                                <div class="flex min-w-0 flex-1 items-center gap-1.5">
                                    <sl-button-group class="author-agent-mode inline-flex shrink-0 items-center" id="author-agent-mode" label="助手工作模式">
                                        {if allow_ask_mode {
                                            view! {
                                                <sl-button
                                                    class="author-mode-btn text-[10px] font-bold tracking-[0.01em]"
                                                    id="author-mode-ask-btn"
                                                    data-agent-mode="ask"
                                                    title="问答模式（只读）"
                                                    size="small"
                                                >
                                                    "Ask"
                                                </sl-button>
                                            }
                                                .into_any()
                                        } else {
                                            view! { <></> }.into_any()
                                        }}
                                        {if allow_build_mode {
                                            view! {
                                                <sl-button
                                                    class="author-mode-btn text-[10px] font-bold tracking-[0.01em]"
                                                    id="author-mode-build-btn"
                                                    data-agent-mode="build"
                                                    title="生成并改写当前脚本"
                                                    size="small"
                                                >
                                                    "Build"
                                                </sl-button>
                                            }
                                                .into_any()
                                        } else {
                                            view! { <></> }.into_any()
                                        }}
                                    </sl-button-group>
                                    <span
                                        id="author-completion-model-wrap"
                                        class="author-completion-model-wrap relative hidden max-w-[min(18rem,50vw)] shrink-0"
                                    >
                                        <select
                                            id="author-completion-model-select"
                                            class="author-completion-select box-border max-w-full min-w-0 cursor-pointer appearance-none border-0 bg-transparent py-0.5 pl-0 pr-4 text-left text-[10px] font-medium leading-tight text-slate-100 outline-none ring-0 focus:outline-none focus:ring-0"
                                            title="点击展开可切换补全模型（与 .env 中 OPENAI_IMITATORS 及 *_COMPLETION_MODEL 顺序一致）"
                                            hidden=true
                                        ></select>
                                        <span
                                            class="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-0.5 text-[9px] leading-none text-slate-500 select-none"
                                            aria-hidden="true"
                                        >
                                            "▾"
                                        </span>
                                    </span>
                                </div>
                                <sl-button class="author-btn author-btn-primary author-btn-icon inline-flex min-w-7 shrink-0 items-center justify-center border-0 bg-transparent px-1.5 py-1 text-[11px] font-bold leading-none text-center" id="author-run-btn" title="发送" size="small" circle=true>
                                    "➤"
                                </sl-button>
                            </div>
                        </div>
                    </div>
                </section>
            </div>
        </section>
    }
}
