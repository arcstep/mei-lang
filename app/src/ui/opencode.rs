use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::UiRouteMode;

pub(super) fn panel_view(
    compiled: &CompiledApp,
    app_path: &str,
    route_mode: UiRouteMode,
    selected_target: &str,
) -> impl IntoView {
    view! {
        <section class="author-panel-section">
            <div
                id="meilang-author-panel"
                class="author-panel"
                data-app=app_path.to_string()
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
                            <sl-select id="author-session-select" title="历史对话" size="small" value="" hoist=true>
                                <sl-option value="">"历史"</sl-option>
                            </sl-select>
                        </div>
                        <sl-tooltip content="重连服务" placement="bottom">
                            <sl-button class="author-btn" id="author-reconnect-btn" size="small" hidden=true>
                                "重连"
                            </sl-button>
                        </sl-tooltip>
                    </div>
                    <div class="author-top-actions author-top-actions-right">
                        <sl-tooltip content="新建对话" placement="bottom">
                            <sl-button
                                class="author-btn author-btn-icon"
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
                                <sl-button-group class="author-history-actions" label="历史操作">
                                    <sl-button
                                        class="author-history-btn"
                                        id="source-view-diff-btn"
                                        data-view-mode="diff"
                                        title="查看最后一轮 Build 差异"
                                        size="small"
                                        disabled=true
                                    >
                                        "Diff"
                                    </sl-button>
                                    <sl-button
                                        class="author-history-btn"
                                        id="author-undo-btn"
                                        title="撤回本轮代码修改"
                                        size="small"
                                        disabled=true
                                    >
                                        "Undo"
                                    </sl-button>
                                    <sl-button
                                        class="author-history-btn"
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
                            <div class="author-composer-footer">
                                <sl-button-group class="author-agent-mode" id="author-agent-mode" label="OpenCode 工作模式">
                                    <sl-button
                                        class="author-mode-btn"
                                        id="author-mode-plan-btn"
                                        data-agent-mode="plan"
                                        title="仅分析与规划"
                                        size="small"
                                    >
                                        "Plan"
                                    </sl-button>
                                    <sl-button
                                        class="author-mode-btn is-active"
                                        id="author-mode-build-btn"
                                        data-agent-mode="build"
                                        title="直接修改代码"
                                        size="small"
                                    >
                                        "Build"
                                    </sl-button>
                                </sl-button-group>
                                <span class="author-model-label" id="author-model-label">
                                    "模型"
                                </span>
                                <sl-button class="author-btn author-btn-primary author-btn-icon" id="author-run-btn" title="发送" size="small" circle=true>
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
