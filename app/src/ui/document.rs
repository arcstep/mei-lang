use leptos::prelude::*;

use crate::ui::capabilities::HostCapabilities;
use crate::ui::preview_chrome::chrome_scripts_view;
use crate::ui::route::UiRouteMode;
use crate::ui::HostAccountView;

pub(crate) fn render_document(
    app_title: &str,
    route_mode: UiRouteMode,
    chrome_hidden: bool,
    shell: AnyView,
    component_scripts_view: AnyView,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> String {
    let shell_mode_class = match route_mode {
        UiRouteMode::App if chrome_hidden => "app-view chrome-none",
        UiRouteMode::App => "app-view",
        UiRouteMode::Presentation => "presentation-view chrome-none",
        UiRouteMode::Build => "build-view",
        UiRouteMode::Config => "config-view",
        UiRouteMode::Upload => "upload-view",
    };
    let body_class = format!("{shell_mode_class} sl-theme-dark");
    let chrome_scripts = chrome_scripts_view(route_mode);
    let auth_user_meta = if auth_enabled {
        auth_account
            .map(|view| view.username.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("")
    } else {
        ""
    };
    let auth_role_meta = if auth_enabled {
        auth_account
            .map(|view| view.role.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("")
    } else {
        ""
    };
    let auth_logged_in_meta = if auth_enabled {
        auth_account
            .map(|view| if view.logged_in { "1" } else { "0" })
            .unwrap_or("0")
    } else {
        "0"
    };
    let auth_capabilities_meta = if auth_enabled {
        auth_account
            .map(|view| serde_json::to_string(&view.capabilities).unwrap_or_default())
            .unwrap_or_default()
    } else {
        serde_json::to_string(&HostCapabilities::auth_disabled()).unwrap_or_default()
    };

    let manage_timing_meta = match route_mode {
        UiRouteMode::Build => view! {
            <meta name="mei-handler-html-ready-ms" content="__MEI_HANDLER_HTML_READY_MS__"/>
            <meta name="mei-ssr-http-response-body-ms" content="__MEI_SSR_HTTP_BODY_MS__"/>
        }
        .into_any(),
        _ => view! { <></> }.into_any(),
    };

    let page = view! {
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="mei-tiles-base-url" content="__MEI_TILES_BASE_URL__"/>
                <meta name="mei-tiles-json-path" content="__MEI_TILES_JSON_PATH__"/>
                <meta name="mei-host-version" content="__MEI_HOST_VERSION__"/>
                <meta name="mei-host-version-label" content="__MEI_HOST_VERSION_LABEL__"/>
                <meta name="mei-host-icp-record" content="__MEI_HOST_ICP_RECORD__"/>
                <meta name="mei-host-psb-record" content="__MEI_HOST_PSB_RECORD__"/>
                <meta name="mei-host-copyright" content="__MEI_HOST_COPYRIGHT__"/>
                <meta name="mei-workspace-label" content="__MEI_WORKSPACE_LABEL__"/>
                <meta name="mei-view" content=route_mode.slug()/>
                <meta name="mei-auth-user" content=auth_user_meta/>
                <meta name="mei-auth-role" content=auth_role_meta/>
                <meta name="mei-auth-logged-in" content=auth_logged_in_meta/>
                <meta name="mei-auth-capabilities" content=auth_capabilities_meta.clone()/>
                <title>{format!("{app_title} - MeiLang")}</title>
                <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/>
                <link rel="stylesheet" href="/app-bundles/styles.css"/>
                <script defer src="/app-assets/host-http-feedback.js"></script>
                <script
                    type="module"
                    src="/app-bundles/shoelace.js"
                ></script>
                {manage_timing_meta}
            </head>
            <body
                class=body_class
                data-mei-view=route_mode.slug()
                data-mei-handler-html-ready-ms="__MEI_HANDLER_HTML_READY_MS__"
                data-mei-ssr-http-response-body-ms="__MEI_SSR_HTTP_BODY_MS__"
                data-mei-auth-user=auth_user_meta
                data-mei-auth-role=auth_role_meta
                data-mei-auth-logged-in=auth_logged_in_meta
                data-mei-auth-capabilities=auth_capabilities_meta
            >
                {shell}
                {chrome_scripts}
                {component_scripts_view}
            </body>
        </html>
    };
    page.to_html()
}
