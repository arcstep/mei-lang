use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::super::route::UiRouteMode;

pub(crate) fn chrome_scripts_view(route_mode: UiRouteMode) -> AnyView {
    match route_mode {
        UiRouteMode::Build => view! {
            <>
                <script defer src="/app-bundles/manage.js"></script>
            </>
        }
        .into_any(),
        UiRouteMode::App | UiRouteMode::Presentation => view! {
            <>
                <script defer src="/app-bundles/access.js"></script>
            </>
        }
        .into_any(),
        UiRouteMode::Config => view! {
            <>
                <script defer src="/app-bundles/config.js"></script>
            </>
        }
        .into_any(),
        UiRouteMode::Upload => view! {
            <>
                <script defer src="/app-bundles/upload.js"></script>
            </>
        }
        .into_any(),
    }
}

pub(crate) fn component_scripts(compiled: &CompiledApp) -> impl IntoView {
    let scripts = compiled
        .component_assets
        .iter()
        .map(|asset| {
            let src = format!("/workspace-components/{}", asset.script);
            view! { <script type="module" src=src></script> }
        })
        .collect_view();
    view! { <>{scripts}</> }
}
