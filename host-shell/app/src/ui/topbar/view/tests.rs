use super::scene_routing::preferred_access_scene;
use super::view::{stage_route_profile, topbar_view, visible_menu_indices, AdminNavItem};
use crate::ui::route::UiRouteMode;
use leptos::prelude::RenderHtml;
use mei_lang_kernel::{CompiledSceneRoute, StageProfile, WorkspaceAppMeta};

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
        short_title: None,
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

#[test]
fn overflow_keeps_the_active_item_in_the_inline_three() {
    assert_eq!(visible_menu_indices(6, Some(4)), vec![0, 1, 4]);
    assert_eq!(visible_menu_indices(6, Some(1)), vec![0, 1, 2]);
    assert_eq!(visible_menu_indices(3, Some(2)), vec![0, 1, 2]);
}

#[test]
fn page_stage_uses_registry_profile_instead_of_cockpit_fallback() {
    let route = CompiledSceneRoute {
        scene_id: "report".to_string(),
        frame_id: None,
        target_file: "src/stage/report.stage.mdx".to_string(),
        kind: "document".to_string(),
        title: Some("年度报告".to_string()),
        short_title: Some("报告".to_string()),
        is_default: false,
        access_export: true,
    };
    assert_eq!(stage_route_profile(&route), StageProfile::Page);
}

#[test]
fn four_stages_render_three_inline_items_and_one_shared_more_panel() {
    let apps = vec![WorkspaceAppMeta {
        id: "demo".to_string(),
        title: "很长的演示应用标题".to_string(),
        short_title: Some("演示".to_string()),
        root: "apps/demo".to_string(),
    }];
    let mut routes = vec![
        route("s1", "src/scene/s1.mei", true, true),
        route("s2", "src/scene/s2.mei", false, true),
        route("s3", "src/scene/s3.mei", false, true),
        route("s4", "src/scene/s4.mei", false, true),
    ];
    routes[3].short_title = Some("当前".to_string());
    let html = topbar_view(
        apps.as_slice(),
        "demo",
        None,
        UiRouteMode::App,
        Some("s4"),
        None,
        None,
        None,
        None,
        false,
        false,
        false,
        None,
        None,
        None,
        None,
        Some(routes.as_slice()),
        None,
        &[],
        None,
    )
    .to_html();
    assert!(html.contains(">演示<"));
    assert!(html.contains("aria-label=\"展开全部应用菜单\""));
    assert_eq!(html.matches("aria-label=\"展开全部应用菜单\"").count(), 1);
    assert!(html.contains("topbar-more-section-title\">驾驶舱"));
    assert!(html.contains("data-mei-stage-scene=\"s4\""));
    assert!(!html.contains("overflow-x-auto"));
}

#[test]
fn four_admin_entries_share_the_same_grouped_more_panel() {
    let apps = vec![WorkspaceAppMeta {
        id: "demo".to_string(),
        title: "演示应用".to_string(),
        short_title: Some("演示".to_string()),
        root: "apps/demo".to_string(),
    }];
    let admin_items = (1..=4)
        .map(|index| AdminNavItem {
            id: format!("dataset.item{index}"),
            label: format!("条目{index}"),
            title: format!("数据管理条目 {index}"),
            href: format!("/admin/apps/demo/dataset/item{index}"),
            menu: "数据管理".to_string(),
            order: index,
        })
        .collect::<Vec<_>>();
    let html = topbar_view(
        apps.as_slice(),
        "demo",
        None,
        UiRouteMode::Admin,
        Some("home"),
        None,
        None,
        None,
        None,
        false,
        false,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        admin_items.as_slice(),
        Some("dataset.item4"),
    )
    .to_html();
    assert!(html.contains("topbar-more-section-title\">数据管理"));
    assert_eq!(html.matches("aria-label=\"展开全部应用菜单\"").count(), 1);
    assert!(html.contains("data-mei-admin-item=\"dataset.item4\""));
    assert!(html.contains("topbar-more-card is-active"));
}
