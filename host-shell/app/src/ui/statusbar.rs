use leptos::prelude::*;

pub(crate) fn statusbar_view(
    app_path: &str,
    _route_mode: &'static str,
    _current_target: &str,
    _upload_root: Option<&str>,
) -> AnyView {
    let show_visit_history = !app_path.trim().is_empty();
    view! {
        <footer class="statusbar statusbar-shell chrome-inset chrome-safe-x sticky bottom-0 z-10 py-1.5 backdrop-blur-md">
            <div class="statusbar-layout min-w-0 mei-font-1">
                <div class="statusbar-track statusbar-track-left min-w-0">
                    {if show_visit_history {
                        view! {
                            <>
                                <button
                                    type="button"
                                    id="mei-visit-history-trigger"
                                    class="status-chip status-chip-visit-history"
                                    data-tone="neutral"
                                    title="最近访问与加载耗时"
                                >
                                    "访问历史"
                                </button>
                                <button
                                    type="button"
                                    id="mei-status-debug-route"
                                    class="status-chip status-chip-debug-route"
                                    data-tone="neutral"
                                    hidden
                                    title="点击复制调试路由"
                                ></button>
                            </>
                        }
                        .into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                </div>
                <div class="statusbar-track statusbar-track-center min-w-0">
                    <span
                        class="status-chip status-chip-compliance max-w-[min(52vw,560px)]"
                        id="mei-status-compliance"
                        data-tone="neutral"
                        hidden
                    ></span>
                </div>
                <span
                    class="status-chip status-chip-host statusbar-right-anchor"
                    id="mei-status-host-version"
                    data-tone="neutral"
                    title="__MEI_HOST_VERSION_TITLE__"
                >
                    "__MEI_HOST_VERSION_LABEL__"
                </span>
            </div>
        </footer>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_scoped_footer_keeps_history_on_admin_routes() {
        let html = statusbar_view("mini-data", "admin", "单位信息", None).to_html();
        assert!(html.contains("mei-visit-history-trigger"));
        assert!(html.contains("mei-status-host-version"));
    }

    #[test]
    fn workspace_footer_keeps_version_without_history() {
        let html = statusbar_view("", "workspace", "/home", None).to_html();
        assert!(!html.contains("mei-visit-history-trigger"));
        assert!(html.contains("mei-status-host-version"));
    }
}
