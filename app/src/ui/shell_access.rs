use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::opencode;
use super::preview;
use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::topbar_view;
use super::TopbarMenuContext;

pub(super) fn access_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    chrome_hidden: bool,
) -> AnyView {
    let preview = preview::preview_view(compiled, app_path);
    let current_target = preview_target.unwrap_or(&compiled.entry_target);
    let panel_tab = active_tab.unwrap_or("preview");
    let topbar = topbar_view(
        apps,
        compiled,
        app_path,
        topbar_menu,
        UiRouteMode::Access,
        selected_entry,
        preview_target,
        active_tab,
    );
    let statusbar = statusbar_view(
        app_path,
        UiRouteMode::Access.slug(),
        current_target,
        None,
        compiled,
        true,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = if chrome_hidden {
        "shell shell-surface min-h-screen h-screen overflow-hidden max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "shell shell-surface grid min-h-screen h-screen overflow-hidden [grid-template-rows:auto_minmax(0,1fr)_auto] max-[1200px]:grid max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "shell shell-surface min-h-screen h-auto overflow-visible"
    };
    let main_class = if chrome_hidden {
        "min-h-0 min-w-0 h-full overflow-hidden p-0 max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden p-4 self-stretch max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-w-0 h-auto overflow-visible p-4 self-start"
    };
    let preview_panel_class = if chrome_hidden {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-surface]:h-full [&_.preview-surface]:min-h-full [&_.preview-viewport]:h-full [&_.preview-viewport]:min-h-full max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-surface]:min-h-auto max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-h-0 min-w-0 overflow-visible [&_.preview-surface]:min-h-auto"
    };
    view! {
        <div class=shell_class>
            {if chrome_hidden {
                view! { <></> }.into_any()
            } else {
                topbar
            }}
            <main class=main_class>
                {if chrome_hidden {
                    view! {
                        <section class=preview_panel_class>
                            {preview}
                        </section>
                    }
                        .into_any()
                } else {
                    view! {
                        <div
                            class="workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0 [grid-template-columns:minmax(0,1fr)_8px_var(--workspace-right-aside)]"
                            id="workspace-root"
                        >
                            <section class=preview_panel_class>
                                {preview}
                            </section>
                            <div
                                class="splitter splitter-right"
                                data-workspace-splitter="right"
                                title="拖拽调整右侧助手栏宽度"
                            >
                                <button
                                    class="splitter-toggle"
                                    type="button"
                                    data-workspace-toggle="right"
                                    aria-label="折叠右侧助手栏"
                                    title="折叠右侧助手栏"
                                >
                                    <span class="splitter-toggle-icon" aria-hidden="true">
                                        <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
                                            <path d="M5.5 3.5 10 8l-4.5 4.5"/>
                                        </svg>
                                    </span>
                                </button>
                            </div>
                            <aside class="sidebar right workspace-panel workspace-panel-side workspace-panel-tool h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-0 py-2.5">
                                <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                                    {opencode::panel_view(
                                        compiled,
                                        app_path,
                                        UiRouteMode::Access,
                                        current_target,
                                        false,
                                        panel_tab,
                                        true,
                                        false,
                                        "ask",
                                        false,
                                    )}
                                </div>
                            </aside>
                        </div>
                    }
                        .into_any()
                }}
            </main>
            {if chrome_hidden {
                view! { <></> }.into_any()
            } else {
                statusbar
            }}
        </div>
    }
    .into_any()
}
