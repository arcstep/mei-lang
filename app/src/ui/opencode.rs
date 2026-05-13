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
                        <button type="button" class="author-btn" id="author-skill-sync-btn" title="同步 MeiLang Skill">
                            "同步Skill"
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
                    <div class="author-chat-log" id="author-chat-log"></div>
                    <div class="author-composer-row">
                        <div class="author-composer-shell">
                            <textarea
                                id="author-intent-input"
                                rows="3"
                                placeholder="输入并发送"
                            ></textarea>
                            <div class="author-composer-footer">
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
