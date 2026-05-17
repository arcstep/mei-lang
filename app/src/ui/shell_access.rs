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
        false,
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
                        <>
                            <section class=preview_panel_class>
                                {preview}
                            </section>
                            <div id="access-chat-floating-root" class="access-chat-floating-root" data-open="false">
                                <button
                                    id="access-chat-fab"
                                    class="access-chat-fab"
                                    type="button"
                                    aria-label="打开助手对话框"
                                    title="打开助手对话框"
                                >
                                    <img class="access-chat-fab-icon" src="/app-assets/favicon.svg" alt="" />
                                </button>
                                <aside
                                    id="access-chat-overlay-panel"
                                    class="access-chat-overlay-panel"
                                    hidden=true
                                >
                                    <div class="access-chat-overlay-head">
                                        <span class="access-chat-overlay-title">"Mei Assistant"</span>
                                        <button
                                            id="access-chat-close"
                                            class="access-chat-overlay-close"
                                            type="button"
                                            aria-label="关闭助手对话框"
                                            title="关闭助手对话框"
                                        >
                                            "×"
                                        </button>
                                    </div>
                                    <div class="access-chat-overlay-body">
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
                        </>
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
