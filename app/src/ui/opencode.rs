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
    view! {
        <section class="author-panel-section h-full min-h-0">
            <div
                id="meilang-author-panel"
                class="author-panel flex h-full min-h-0 flex-col gap-2.5 overflow-hidden pt-0.5"
                data-app=app_path.to_string()
                data-target=selected_target.to_string()
                data-entry=compiled
                    .active_entry
                    .clone()
                    .unwrap_or_else(|| compiled.entry_target.clone())
                data-mode=route_mode.slug()
                data-source-views=if source_views_enabled { "true" } else { "false" }
                data-view-tab=active_tab.to_string()
            >
                <div class="author-top-row sticky top-0 z-[2] flex flex-nowrap items-center justify-between gap-2 overflow-x-auto rounded-xl border border-slate-600/45 bg-slate-900/75 px-2.5 py-2">
                    <div class="author-top-actions flex min-w-0 flex-nowrap items-center gap-1.5">
                        <div class="author-service-status inline-flex items-center gap-1.5 rounded-full border border-slate-500/40 bg-slate-950/45 px-2.5 py-1 text-[11px] text-slate-300" title="服务连接状态">
                            <span id="author-server-dot" class="author-server-dot author-server-dot-off"></span>
                            <strong id="author-server-status">"已断开"</strong>
                        </div>
                        <div class="author-history-controls inline-flex items-center gap-1.5">
                            <sl-select
                                class="max-w-[240px]"
                                id="author-session-select"
                                title="历史对话"
                                size="small"
                                value=""
                                hoist=true
                            >
                                <sl-option value="">"历史"</sl-option>
                            </sl-select>
                        </div>
                        <sl-tooltip content="重连服务" placement="bottom">
                            <sl-button class="author-btn inline-flex items-center justify-center border-0 bg-transparent p-0 text-xs font-bold" id="author-reconnect-btn" size="small" hidden=true>
                                "重连"
                            </sl-button>
                        </sl-tooltip>
                    </div>
                    <div class="author-top-actions author-top-actions-right ml-auto shrink-0">
                        <sl-tooltip content="新建对话" placement="bottom">
                            <sl-button
                                class="author-btn author-btn-icon inline-flex min-w-8 items-center justify-center border-0 bg-transparent px-[9px] py-[7px] text-xs font-bold leading-none text-center"
                                id="author-session-btn"
                                size="small"
                                circle=true
                                aria-label="新建对话"
                            >
                                "+"
                            </sl-button>
                        </sl-tooltip>
                    </div>
                </div>
                <section class="author-chat-section flex min-h-0 flex-1 flex-col gap-2 overflow-hidden rounded-2xl border border-slate-600/50 bg-slate-950/55 p-2.5">
                    <div class="author-progress-strip grid gap-1.5 rounded-xl border border-slate-600/45 bg-slate-900/70 px-3 py-2.5" id="author-progress-strip" hidden>
                        <div class="author-progress-main flex min-w-0 flex-wrap items-center gap-2">
                            <span class="author-progress-label text-xs font-bold tracking-[0.01em] text-slate-100" id="author-progress-label">
                                "准备中"
                            </span>
                            <span class="author-progress-detail text-[11px] leading-5 text-blue-300" id="author-progress-detail"></span>
                        </div>
                        <div class="author-progress-items flex flex-wrap gap-1.5" id="author-progress-items"></div>
                    </div>
                    <div class="author-chat-log grid min-h-0 flex-1 content-start gap-2.5 overflow-auto pr-0.5 [grid-auto-rows:max-content]" id="author-chat-log"></div>
                    <div class="author-composer-row sticky bottom-0 flex flex-col gap-1.5 border-t border-slate-600/40 pt-2 [background:linear-gradient(180deg,rgba(2,6,23,.15),rgba(2,6,23,.95)_50%)]">
                        <div class="author-composer-shell flex flex-col gap-2.5 rounded-2xl border border-slate-600/40 bg-slate-900/90 p-3">
                            <div class="author-composer-header flex min-h-5 items-center justify-end">
                                <sl-button-group class="author-history-actions inline-flex items-center gap-1.5" label="历史操作">
                                    {if source_views_enabled {
                                        view! {
                                            <sl-button
                                                class="author-history-btn text-[10px] font-bold tracking-[0.02em]"
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
                                        class="author-history-btn text-[10px] font-bold tracking-[0.02em]"
                                        id="author-undo-btn"
                                        title="撤回本轮代码修改"
                                        size="small"
                                        disabled=true
                                    >
                                        "Undo"
                                    </sl-button>
                                    <sl-button
                                        class="author-history-btn text-[10px] font-bold tracking-[0.02em]"
                                        id="author-redo-btn"
                                        title="恢复最近撤回的代码修改"
                                        size="small"
                                        disabled=true
                                    >
                                        "Redo"
                                    </sl-button>
                                </sl-button-group>
                            </div>
                            <textarea
                                id="author-intent-input"
                                rows="3"
                                placeholder="输入并发送"
                            ></textarea>
                            <div class="author-composer-footer flex items-center justify-between gap-3">
                                <sl-button-group class="author-agent-mode inline-flex items-center" id="author-agent-mode" label="OpenCode 工作模式">
                                    <sl-button
                                        class="author-mode-btn text-[11px] font-bold tracking-[0.01em]"
                                        id="author-mode-plan-btn"
                                        data-agent-mode="plan"
                                        title="仅分析与规划"
                                        size="small"
                                    >
                                        "Plan"
                                    </sl-button>
                                    <sl-button
                                        class="author-mode-btn is-active text-[11px] font-bold tracking-[0.01em]"
                                        id="author-mode-build-btn"
                                        data-agent-mode="build"
                                        title="直接修改代码"
                                        size="small"
                                    >
                                        "Build"
                                    </sl-button>
                                </sl-button-group>
                                <span class="author-model-label min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[11px] font-bold tracking-[0.01em] text-blue-300/90" id="author-model-label">
                                    "模型"
                                </span>
                                <sl-button class="author-btn author-btn-primary author-btn-icon inline-flex min-w-8 items-center justify-center border-0 bg-transparent px-[9px] py-[7px] text-xs font-bold leading-none text-center" id="author-run-btn" title="发送" size="small" circle=true>
                                    "➤"
                                </sl-button>
                            </div>
                        </div>
                    </div>
                    <sl-tag class="author-skill-line" id="author-skill-line" size="small"></sl-tag>
                    <sl-tag class="author-config-line" id="author-config-line" size="small" variant="warning" hidden=true></sl-tag>
                </section>
            </div>
        </section>
    }
}
