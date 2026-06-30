use std::path::Path;

use leptos::prelude::*;
use mei_lang_kernel::{read_source_file, CompiledApp, WorkspaceAppMeta};

use super::access_ai_entry::access_ai_floating_entry;
use super::manage_routing::WorldSemanticQuery;
use super::preview;
use super::route::UiRouteMode;
use super::scene_drilldown_context::host_ssr_bootstrap_scripts;
use super::shell_preview_layout::{
    access_main_preview_class, access_preview_panel_class, access_shell_grid_class,
};
use super::{HostAccountView, TopbarMenuContext};

fn presentation_manifest_candidates(presentation_id: &str) -> [String; 2] {
    [
        format!("src/presentation/{presentation_id}.presentation.json"),
        format!("presentation/{presentation_id}.presentation.json"),
    ]
}

fn load_presentation_manifest_json(
    compiled: &CompiledApp,
    presentation_id: &str,
) -> Option<String> {
    let app_root = Path::new(compiled.app_root.as_str());
    for rel in presentation_manifest_candidates(presentation_id) {
        let path = app_root.join(&rel);
        if let Ok(content) = read_source_file(&path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Some(content);
            }
        }
    }
    None
}

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
    let manifest_json = load_presentation_manifest_json(compiled, presentation_id);
    let active_scene = manifest_json
        .as_ref()
        .and_then(|_| {
            compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == "home")
                .map(|route| route.target_file.as_str())
        })
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
    );
    let manifest_script = manifest_json.map(|json| {
        view! {
            <script type="application/json" id="mei-presentation-manifest">{json}</script>
        }
    });
    let floating_entry =
        access_ai_floating_entry(compiled, app_path, active_scene, panel_tab);

    view! {
        <div
            id="copilot-shell"
            class=shell_class
            data-copilot-presentation=presentation_id
        >
            {host_ssr_bootstrap_scripts(compiled, app_path, bootstrap_scene)}
            {manifest_script}
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
