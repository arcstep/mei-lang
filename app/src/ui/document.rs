use leptos::prelude::*;

use crate::ui::capabilities::HostCapabilities;
use crate::ui::preview_chrome::chrome_script_preload_markup;
use crate::ui::preview_chrome::chrome_script_preloads_view;
use crate::ui::preview_chrome::chrome_scripts_view;
use crate::ui::route::UiRouteMode;
use crate::ui::HostAccountView;

pub(crate) fn render_document(
    app_title: &str,
    route_mode: UiRouteMode,
    chrome_hidden: bool,
    shell: AnyView,
    head_script_preloads_view: AnyView,
    component_scripts_view: AnyView,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    body_theme_style: &str,
) -> String {
    let shell_mode_class = match route_mode {
        UiRouteMode::App if chrome_hidden => "app-view chrome-none",
        UiRouteMode::App => "app-view",
        UiRouteMode::Run => "run-view chrome-none",
        UiRouteMode::Speaker => "speaker-view chrome-none",
        UiRouteMode::Build => "build-view",
        UiRouteMode::Runtime => "runtime-view",
        UiRouteMode::Config => "config-view",
        UiRouteMode::Upload => "upload-view",
    };
    let body_class = format!("{shell_mode_class} sl-theme-dark");
    let chrome_scripts = chrome_scripts_view(route_mode);
    let chrome_script_preloads = chrome_script_preloads_view(route_mode);
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
                <script src="/app-assets/spa-navigation/visit-history-store.js"></script>
                <script src="/app-assets/page-load-progress-shell.js"></script>
                <script>
                    {r#"(function(){try{if(window.MeiPageLoadProgress){window.MeiPageLoadProgress.mountEarlyHandoffOverlay();}}catch(e){}})();"#}
                </script>
                <script defer src="/app-assets/host-http-feedback.js"></script>
                <script
                    type="module"
                    src="/app-bundles/shoelace.js"
                ></script>
                {chrome_script_preloads}
                {head_script_preloads_view}
                {chrome_scripts}
                {component_scripts_view}
                {manage_timing_meta}
            </head>
            <body
                class=body_class
                style=body_theme_style.to_string()
                data-mei-view=route_mode.slug()
                data-mei-handler-html-ready-ms="__MEI_HANDLER_HTML_READY_MS__"
                data-mei-ssr-http-response-body-ms="__MEI_SSR_HTTP_BODY_MS__"
                data-mei-compile-ms="__MEI_COMPILE_MS__"
                data-mei-compile-cache-hit="__MEI_COMPILE_CACHE_HIT__"
                data-mei-html-bytes="__MEI_HTML_BYTES__"
                data-mei-data-props-bytes="__MEI_DATA_PROPS_BYTES__"
                data-mei-data-props-count="__MEI_DATA_PROPS_COUNT__"
                data-mei-auth-user=auth_user_meta
                data-mei-auth-role=auth_role_meta
                data-mei-auth-logged-in=auth_logged_in_meta
                data-mei-auth-capabilities=auth_capabilities_meta
            >
                <script>
                    {r#"(function(){try{if(window.MeiPageLoadProgress){window.MeiPageLoadProgress.mountFromHandoff();}}catch(e){}})();"#}
                </script>
                {shell}
            </body>
        </html>
    };
    inject_chrome_script_preloads(page.to_html(), route_mode)
}

fn inject_chrome_script_preloads(html: String, route_mode: UiRouteMode) -> String {
    let preload = chrome_script_preload_markup(route_mode);
    if preload.is_empty() {
        return html;
    }
    const ANCHOR: &str = "<script defer src=\"/app-assets/host-http-feedback.js\"></script>";
    html.replacen(ANCHOR, &format!("{preload}{ANCHOR}"), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::route::UiRouteMode;

    #[test]
    fn inject_chrome_script_preloads_adds_valid_as_attribute() {
        let html = inject_chrome_script_preloads(
            "<script defer src=\"/app-assets/host-http-feedback.js\"></script>".to_string(),
            UiRouteMode::App,
        );
        assert!(html.contains(r#"rel="preload" href="/app-bundles/access.js" as="script""#));
        assert!(!html.contains(r#"rel="preload" href="/app-bundles/access.js"/>"#));
    }
}
