use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::agent_panel;
use super::compile_status::is_static_workspace_asset_target;
use super::preview;
use super::preview_chrome::asset_preview_body;
use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::topbar_view;
use super::TopbarMenuContext;

pub(super) fn access_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    selected_scene: Option<&str>,
    file_target: Option<&str>,
    source: Option<&str>,
    active_tab: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
) -> AnyView {
    let current_target = file_target
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    let static_asset = is_static_workspace_asset_target(current_target);
    let preview = if static_asset {
        asset_preview_body(app_path, current_target, source.unwrap_or(""))
    } else {
        preview::preview_view(compiled, app_path, current_target, UiRouteMode::App)
    };
    let topbar_preview_target = if static_asset {
        None
    } else {
        file_target
    };
    let panel_tab = active_tab.unwrap_or("preview");
    let topbar = topbar_view(
        apps,
        compiled,
        app_path,
        topbar_menu,
        UiRouteMode::App,
        selected_scene,
        topbar_preview_target,
        active_tab,
        upload_enabled,
    );
    let statusbar = statusbar_view(
        app_path,
        UiRouteMode::App.slug(),
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
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-viewport]:h-full [&_.preview-viewport]:min-h-full [&_.preview-surface:not(.preview-stage)]:h-full [&_.preview-surface:not(.preview-stage)]:min-h-full max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-viewport-fluid-width]:max-h-full [&_.preview-viewport-fluid-width]:min-h-0 [&_.preview-viewport-fluid-width]:overflow-y-auto [&_.preview-surface]:min-h-auto max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-h-0 min-w-0 overflow-visible [&_.preview-surface]:min-h-auto"
    };
    let floating_entry = || {
        view! {
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
                        {agent_panel::panel_view(
                            compiled,
                            app_path,
                            UiRouteMode::App,
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
                        <>
                            <section class=preview_panel_class>
                                {preview}
                            </section>
                            {floating_entry()}
                        </>
                    }
                        .into_any()
                } else {
                    view! {
                        <>
                            <section class=preview_panel_class>
                                {preview}
                            </section>
                            {floating_entry()}
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
