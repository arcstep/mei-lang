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

pub(crate) fn component_scripts(
    compiled: &CompiledApp,
    scene_bundle_url: Option<&str>,
) -> impl IntoView {
    if let Some(bundle_url) = scene_bundle_url.map(str::trim).filter(|value| !value.is_empty()) {
        return view! {
            <script
                type="module"
                src=bundle_url
                data-mei-scene-bundle="true"
                data-mei-persistent-script=bundle_url
            ></script>
        }
        .into_any();
    }
    let scripts = compiled
        .component_assets
        .iter()
        .map(|asset| {
            let src = format!("/workspace-components/{}", asset.script);
            view! { <script type="module" src=src></script> }
        })
        .collect_view();
    view! { <>{scripts}</> }.into_any()
}
