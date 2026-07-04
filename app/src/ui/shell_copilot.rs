use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::access_ai_entry::access_ai_floating_entry;
use super::manage_routing::WorldSemanticQuery;
use super::preview;
use super::route::UiRouteMode;
use super::scene_drilldown_context::host_ssr_bootstrap_scripts;
use super::shell_preview_layout::{
    access_main_preview_class, access_preview_panel_class, access_shell_grid_class,
};
use super::{HostAccountView, TopbarMenuContext};

pub(crate) fn copilot_shell(
    _apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    _topbar_menu: Option<&TopbarMenuContext>,
    selected_scene: Option<&str>,
    _file_target: Option<&str>,
    _source: Option<&str>,
    _active_tab: Option<&str>,
    _upload_enabled: bool,
    _auth_enabled: bool,
    _auth_account: Option<&HostAccountView>,
) -> AnyView {
    let presentation_id = selected_scene
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("intro");
    let active_scene = compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id == "home")
        .map(|route| route.target_file.as_str())
        .unwrap_or(compiled.active_target_file.as_str());
    let bootstrap_scene = Some("home");
    let panel_tab = "preview";
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = format!(
        "{} copilot-shell mei-surface-shell mei-text-inverse",
        access_shell_grid_class(true, stage_enabled)
    );
    let main_class = access_main_preview_class(true, stage_enabled);
    let preview_panel_class = access_preview_panel_class(true, stage_enabled);
    let preview = preview::preview_view(
        compiled,
        app_path,
        active_scene,
        UiRouteMode::Copilot,
        WorldSemanticQuery::default(),
        None,
        None,
        None,
        None,
    );
    let floating_entry =
        access_ai_floating_entry(compiled, app_path, active_scene, panel_tab);

    view! {
        <div
            id="copilot-shell"
            class=shell_class
            data-copilot-presentation=presentation_id
        >
            {host_ssr_bootstrap_scripts(compiled, app_path, bootstrap_scene, None)}
            <main class=main_class>
                <section class=preview_panel_class>
                    {preview}
                </section>
                {floating_entry}
            </main>
        </div>
    }
    .into_any()
}
