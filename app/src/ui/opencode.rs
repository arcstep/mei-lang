use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::UiRouteMode;

pub(super) fn panel_view(
    compiled: &CompiledApp,
    route_mode: UiRouteMode,
    selected_target: &str,
) -> impl IntoView {
    view! {
        <section class="author-panel-section">
            <div
                id="meilang-author-panel"
                class="author-panel"
                data-app=compiled.app_id.clone()
                data-target=selected_target.to_string()
                data-entry=compiled
                    .active_entry
                    .clone()
                    .unwrap_or_else(|| compiled.entry_target.clone())
                data-mode=route_mode.slug()
            >
                <div class="author-top-row">
                    <div class="author-top-actions">
                        <div class="author-service-status" title="服务连接状态">
                            <span id="author-server-dot" class="author-server-dot author-server-dot-off"></span>
                            <strong id="author-server-status">"已断开"</strong>
                        </div>
                        <div class="author-history-controls">
                            <select id="author-session-select" title="历史对话">
                                <option value="">"历史"</option>
                            </select>
                        </div>
                        <button type="button" class="author-btn" id="author-reconnect-btn" title="重连服务" hidden>
                            "重连"
                        </button>
                    </div>
                    <div class="author-top-actions author-top-actions-right">
                        <button
                            type="button"
                            class="author-btn author-btn-icon"
                            id="author-session-btn"
                            title="新建对话"
                            aria-label="新建对话"
                        >
                            "+"
                        </button>
                    </div>
                </div>
                <section class="author-chat-section">
                    <div class="author-progress-strip" id="author-progress-strip" hidden>
                        <div class="author-progress-main">
                            <span class="author-progress-label" id="author-progress-label">
                                "准备中"
                            </span>
                            <span class="author-progress-detail" id="author-progress-detail"></span>
                        </div>
                        <div class="author-progress-items" id="author-progress-items"></div>
                    </div>
                    <div class="author-chat-log" id="author-chat-log"></div>
                    <div class="author-composer-row">
                        <div class="author-composer-shell">
                            <div class="author-composer-header">
                                <div class="author-history-actions" role="group" aria-label="历史操作">
                                    <button
                                        type="button"
                                        class="author-history-btn"
                                        id="source-view-diff-btn"
                                        data-view-mode="diff"
                                        title="查看最后一轮 Build 差异"
                                        disabled
                                    >
                                        "Diff"
                                    </button>
                                    <button
                                        type="button"
                                        class="author-history-btn"
                                        id="author-undo-btn"
                                        title="撤回本轮代码修改"
                                        disabled
                                    >
                                        "Undo"
                                    </button>
                                    <button
                                        type="button"
                                        class="author-history-btn"
                                        id="author-redo-btn"
                                        title="恢复最近撤回的代码修改"
                                        disabled
                                    >
                                        "Redo"
                                    </button>
                                </div>
                            </div>
                            <textarea
                                id="author-intent-input"
                                rows="3"
                                placeholder="输入并发送"
                            ></textarea>
                            <div class="author-composer-footer">
                                <div class="author-agent-mode" id="author-agent-mode" role="group" aria-label="OpenCode 工作模式">
                                    <button
                                        type="button"
                                        class="author-mode-btn"
                                        id="author-mode-plan-btn"
                                        data-agent-mode="plan"
                                        title="仅分析与规划"
                                    >
                                        "Plan"
                                    </button>
                                    <button
                                        type="button"
                                        class="author-mode-btn is-active"
                                        id="author-mode-build-btn"
                                        data-agent-mode="build"
                                        title="直接修改代码"
                                    >
                                        "Build"
                                    </button>
                                </div>
                                <span class="author-model-label" id="author-model-label">
                                    "模型"
                                </span>
                                <button type="button" class="author-btn author-btn-primary author-btn-icon" id="author-run-btn" title="发送">
                                    "➤"
                                </button>
                            </div>
                        </div>
                    </div>
                    <div class="author-skill-line" id="author-skill-line"></div>
                    <div class="author-config-line" id="author-config-line" hidden></div>
                </section>
            </div>
        </section>
    }
}
