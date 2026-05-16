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
) -> impl IntoView {
    let active_entry = compiled
        .active_entry
        .clone()
        .unwrap_or_else(|| compiled.entry_target.clone());
    let active_scene = compiled
        .entries
        .iter()
        .find(|item| item.entry_id == active_entry)
        .map(|item| item.scene_id.clone())
        .or_else(|| {
            compiled
                .scene_contract
                .as_ref()
                .map(|item| item.scene.id.clone())
        })
        .unwrap_or_default();
    view! {
        <section class="author-panel-section h-full min-h-0">
            <div
                id="meilang-author-panel"
                class="author-panel flex h-full min-h-0 flex-col gap-1.5 overflow-hidden"
                data-app=app_path.to_string()
                data-target=selected_target.to_string()
                data-entry=active_entry
                data-scene=active_scene
                data-mode=route_mode.slug()
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
                        class="author-context-preview grid gap-1 rounded-xl border border-slate-700/55 bg-slate-950/35 px-2 py-1.5"
                    >
                        <div class="flex items-center justify-between gap-2">
                            <span class="text-[10px] font-bold tracking-[0.02em] text-slate-300">"上下文预期"</span>
                            <sl-button
                                class="author-history-btn text-[10px] font-bold tracking-[0.02em]"
                                id="author-context-refresh-btn"
                                title="刷新上下文预期"
                                size="small"
                            >
                                "刷新"
                            </sl-button>
                        </div>
                        <div id="author-context-preview-scope" class="text-[10px] text-slate-400"></div>
                        <div id="author-context-preview-skill" class="text-[10px] text-slate-400"></div>
                        <details class="rounded-lg border border-slate-700/60 bg-slate-900/45 px-2 py-1">
                            <summary class="cursor-pointer text-[10px] font-bold text-slate-300">"可用工具"</summary>
                            <pre id="author-context-preview-tools" class="mt-1 whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-slate-200"></pre>
                        </details>
                        <details class="rounded-lg border border-slate-700/60 bg-slate-900/45 px-2 py-1">
                            <summary class="cursor-pointer text-[10px] font-bold text-slate-300">"提示语注入预览"</summary>
                            <pre id="author-context-preview-prompt" class="mt-1 max-h-48 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] leading-5 text-slate-200"></pre>
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
                                        "Diff"
                                    </sl-button>
                                }
                                    .into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
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
                                <sl-button-group class="author-agent-mode inline-flex items-center" id="author-agent-mode" label="OpenCode 工作模式">
                                    <sl-button
                                        class="author-mode-btn text-[10px] font-bold tracking-[0.01em]"
                                        id="author-mode-plan-btn"
                                        data-agent-mode="plan"
                                        title="仅分析与规划"
                                        size="small"
                                    >
                                        "Plan"
                                    </sl-button>
                                    <sl-button
                                        class="author-mode-btn is-active text-[10px] font-bold tracking-[0.01em]"
                                        id="author-mode-build-btn"
                                        data-agent-mode="build"
                                        title="直接修改代码"
                                        size="small"
                                    >
                                        "Build"
                                    </sl-button>
                                </sl-button-group>
                                <sl-button class="author-btn author-btn-primary author-btn-icon inline-flex min-w-7 items-center justify-center border-0 bg-transparent px-1.5 py-1 text-[11px] font-bold leading-none text-center" id="author-run-btn" title="发送" size="small" circle=true>
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
