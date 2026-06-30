use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::manage_routing::WorldSemanticQuery;
use super::preview;
use super::route::UiRouteMode;
use super::scene_drilldown_context::host_ssr_bootstrap_scripts;
use super::shell_preview_layout::{
    access_main_preview_class, access_preview_panel_class, access_shell_grid_class,
};
use super::{HostAccountView, TopbarMenuContext};

const MINI_PARK_INTRO_TOUR_JSON: &str = r#"{
  "id": "intro",
  "title": "迷你公园导览",
  "steps": [
    {
      "id": "home_intro",
      "title": "公园总览",
      "caption": "迷你公园驾驶舱以观察窗对准湖心区域，左右 rail 承载四个叙事观点入口。",
      "scene": "home",
      "actions": [
        { "type": "highlight", "viewpoint": "park_overview_stage" }
      ]
    },
    {
      "id": "point_1_story",
      "title": "① 湖心亭",
      "caption": "湖心亭是观景核心，二层论据板展开空间焦点与停留数据。",
      "scene": "park_point_1_board",
      "route": "/apps/app/mini-park/scene/park_point_1_board",
      "actions": [
        { "type": "highlight", "viewpoint": "park_point_1_entry" }
      ]
    },
    {
      "id": "point_2_story",
      "title": "② 樱花道",
      "caption": "樱花道承担动线引导，串联湖心、游乐与配套区。",
      "scene": "park_point_2_board",
      "route": "/apps/app/mini-park/scene/park_point_2_board",
      "actions": [
        { "type": "highlight", "viewpoint": "park_point_2_entry" }
      ]
    }
  ]
}"#;

fn speaker_tour_json(app_path: &str, tour_id: Option<&str>) -> Option<&'static str> {
    let tour = tour_id.map(str::trim).filter(|value| !value.is_empty())?;
    if app_path == "mini-park" && tour == "intro" {
        return Some(MINI_PARK_INTRO_TOUR_JSON);
    }
    None
}

pub(crate) fn speaker_shell(
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
    let tour_id = selected_scene
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("intro");
    let tour_json = speaker_tour_json(app_path, Some(tour_id));
    let active_scene = tour_json
        .and_then(|_| {
            compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == "home")
                .map(|route| route.target_file.as_str())
        })
        .unwrap_or(compiled.active_target_file.as_str());
    let bootstrap_scene = Some("home");
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = format!(
        "{} speaker-shell mei-surface-shell mei-text-inverse",
        access_shell_grid_class(true, stage_enabled)
    );
    let main_class = access_main_preview_class(true, stage_enabled);
    let preview_panel_class = access_preview_panel_class(true, stage_enabled);
    let preview = preview::preview_view(
        compiled,
        app_path,
        active_scene,
        UiRouteMode::Speaker,
        WorldSemanticQuery::default(),
        None,
        None,
    );
    let tour_script = tour_json.map(|json| {
        view! {
            <script type="application/json" id="mei-speaker-tour">{json}</script>
        }
    });

    view! {
        <div id="speaker-shell" class=shell_class data-speaker-tour=tour_id>
            {host_ssr_bootstrap_scripts(compiled, app_path, bootstrap_scene)}
            {tour_script}
            <main class=main_class>
                <section class=preview_panel_class>
                    {preview}
                </section>
            </main>
        </div>
    }
    .into_any()
}
