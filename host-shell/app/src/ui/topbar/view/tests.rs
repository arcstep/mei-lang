use super::scene_routing::preferred_access_scene;
use crate::ui::route::UiRouteMode;
use mei_lang_kernel::CompiledSceneRoute;

fn route(
    scene_id: &str,
    target_file: &str,
    is_default: bool,
    access_export: bool,
) -> CompiledSceneRoute {
    CompiledSceneRoute {
        scene_id: scene_id.to_string(),
        frame_id: None,
        target_file: target_file.to_string(),
        kind: "scene".to_string(),
        title: None,
        is_default,
        access_export,
    }
}

#[test]
fn preferred_access_scene_falls_back_to_default_exported_scene() {
    let routes = vec![
        route("private", "scenes/private.mei", false, false),
        route("home", "scenes/home.mei", true, true),
    ];
    assert_eq!(
        preferred_access_scene(UiRouteMode::Config, &routes, None, None, None, "main.mei",),
        Some("home")
    );
}

#[test]
fn preferred_access_scene_prefers_build_preview_target_scene() {
    let routes = vec![
        route("home", "scenes/home.mei", true, true),
        route("detail", "scenes/detail.mei", false, true),
    ];
    assert_eq!(
        preferred_access_scene(
            UiRouteMode::Layout,
            &routes,
            None,
            Some("scenes/detail.mei"),
            Some("home"),
            "main.mei",
        ),
        Some("detail")
    );
}
