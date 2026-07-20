use std::path::Path;

use leptos::prelude::*;
use mei_lang_kernel::{load_mei_config_for_app, AccessAiExternalConfig, CompiledApp};

use super::agent_panel;
use super::route::UiRouteMode;

pub(crate) fn resolve_access_ai_external(compiled: &CompiledApp) -> Option<AccessAiExternalConfig> {
    let app_root = Path::new(compiled.app_root.as_str());
    let config = load_mei_config_for_app(app_root, None);
    config
        .features
        .access_ai_external
        .filter(|entry| entry.is_configured())
}

pub(crate) fn resolve_copilot_fab_enabled(compiled: &CompiledApp) -> bool {
    let app_root = Path::new(compiled.app_root.as_str());
    load_mei_config_for_app(app_root, None)
        .features
        .copilot_fab_enabled()
}

pub(crate) fn external_access_ai_floating_entry(
    app_path: &str,
    config: &AccessAiExternalConfig,
) -> impl IntoView {
    let label = config.label_or_default().to_string();
    let target = if config.open_in_new_tab() {
        "_blank"
    } else {
        "_self"
    };
    let rel = if config.open_in_new_tab() {
        "noopener noreferrer"
    } else {
        ""
    };
    view! {
        <div
            id="access-external-ai-floating-root"
            class="access-chat-floating-root access-external-ai-floating-root"
            data-app-id=app_path.to_string()
        >
            <a
                id="access-external-ai-fab"
                class="access-chat-fab access-external-ai-fab"
                href=config.url.clone()
                target=target
                rel=rel
                aria-label=label.clone()
                title=label
            >
                <img
                    class="access-chat-fab-icon access-external-ai-fab-icon"
                    src=config.image.clone()
                    alt=""
                />
            </a>
        </div>
    }
}

pub(crate) fn builtin_access_ai_floating_entry(
    compiled: &CompiledApp,
    app_path: &str,
    current_target: &str,
    panel_tab: &str,
) -> impl IntoView {
    let stage_kind = resolve_access_stage_kind(compiled);
    view! {
        <div
            id="access-chat-floating-root"
            class="access-chat-floating-root"
            data-open="false"
            data-mei-stage-kind=stage_kind
            data-mei-fab-policy="required"
        >
            <button
                id="access-chat-fab"
                class="access-chat-fab"
                type="button"
                aria-label="展开 Copilot 工具条"
                title="展开 Copilot 工具条"
                data-mei-fab-policy="required"
            >
                <img class="access-chat-fab-icon" src="/app-assets/favicon.svg" alt="" />
            </button>
            <aside
                id="access-chat-overlay-panel"
                class="access-chat-overlay-panel"
                hidden=true
            >
                <div class="access-chat-overlay-head">
                    <span class="access-chat-overlay-title">"Mei Access Assistant"</span>
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
}

fn resolve_access_stage_kind(compiled: &CompiledApp) -> &'static str {
    let scene_id = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(route) = compiled.scene_routes.iter().find(|route| {
        scene_id
            .map(|id| route.scene_id == id)
            .unwrap_or(route.is_default)
    }) {
        let kind = route.kind.trim().to_ascii_lowercase();
        if kind == "presentation" {
            return "presentation";
        }
        let target = route.target_file.replace('\\', "/").to_ascii_lowercase();
        if target.contains("/presentation/") || target.starts_with("presentation/") {
            return "presentation";
        }
    }
    "scene"
}

pub(crate) fn access_ai_floating_entry(
    compiled: &CompiledApp,
    app_path: &str,
    current_target: &str,
    panel_tab: &str,
) -> AnyView {
    if let Some(external) = resolve_access_ai_external(compiled) {
        external_access_ai_floating_entry(app_path, &external).into_any()
    } else if !resolve_copilot_fab_enabled(compiled) {
        view! { <></> }.into_any()
    } else {
        builtin_access_ai_floating_entry(compiled, app_path, current_target, panel_tab).into_any()
    }
}
