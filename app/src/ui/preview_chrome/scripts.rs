use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::super::route::UiRouteMode;

pub(super) fn chrome_scripts_view(route_mode: UiRouteMode) -> AnyView {
    if route_mode == UiRouteMode::Manage {
        view! {
            <>
                <script src="/app-bundles/manage.js"></script>
            </>
        }
        .into_any()
    } else {
        view! {
            <>
                <script src="/app-bundles/access.js"></script>
            </>
        }
        .into_any()
    }
}

pub(super) fn component_scripts(compiled: &CompiledApp) -> impl IntoView {
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
