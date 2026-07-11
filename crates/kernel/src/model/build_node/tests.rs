use super::*;

#[test]
fn build_node_id_roundtrip() {
    let id = BuildNodeId::world_metric("metrics.world.mei", "total_amount");
    assert_eq!(id.encode(), "world-metric:metrics.world.mei#total_amount");
    assert_eq!(BuildNodeId::parse(&id.encode()), Some(id));
}

#[test]
fn legacy_static_html_file_maps_to_world_file_preview() {
    let resolved = resolve_build_view_query(
        None,
        None,
        Some("preview"),
        &LegacyBuildQuery {
            file: Some("demo/index.html".to_string()),
            scene: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            tab: Some("preview".to_string()),
        },
    )
    .expect("resolved");
    assert_eq!(resolved.node.kind, BuildNodeKind::WorldFile);
    assert_eq!(resolved.tab, BuildViewTab::Preview);
}

#[test]
fn legacy_assembly_capsule_maps_parent_scene_id() {
    let resolved = resolve_build_view_query(
        None,
        None,
        Some("preview"),
        &LegacyBuildQuery {
            file: Some("src/scene/home/assembly.mei".to_string()),
            scene: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            tab: Some("preview".to_string()),
        },
    )
    .expect("resolved");
    assert_eq!(resolved.node.encode(), "scene:home");
    assert_eq!(resolved.tab, BuildViewTab::Preview);
}

#[test]
fn legacy_scene_capsule_file_overrides_scene_query() {
    let resolved = resolve_build_view_query(
        None,
        None,
        Some("preview"),
        &LegacyBuildQuery {
            file: Some("details.mei".to_string()),
            scene: Some("home".to_string()),
            world_metric: None,
            world_dataset: None,
            explain: None,
            tab: Some("preview".to_string()),
        },
    )
    .expect("resolved");
    assert_eq!(resolved.node.encode(), "scene:details");
    assert_eq!(resolved.tab, BuildViewTab::Preview);
}

#[test]
fn legacy_board_file_with_scene_maps_to_board_file_node() {
    let resolved = resolve_build_view_query(
        None,
        None,
        Some("preview"),
        &LegacyBuildQuery {
            file: Some("scenes/01-执法要素.board.mei".to_string()),
            scene: Some("enforcement_units_analytics_board".to_string()),
            world_metric: None,
            world_dataset: None,
            explain: None,
            tab: Some("preview".to_string()),
        },
    )
    .expect("resolved");
    assert_eq!(resolved.node.kind, BuildNodeKind::BoardFile);
    assert_eq!(
        resolved.node.encode(),
        "board-file:scenes/01-执法要素.board.mei#enforcement_units_analytics_board"
    );
    assert_eq!(resolved.tab, BuildViewTab::Preview);
}

#[test]
fn legacy_board_file_without_scene_maps_to_board_file_node() {
    let resolved = resolve_build_view_query(
        None,
        None,
        Some("preview"),
        &LegacyBuildQuery {
            file: Some("scenes/01-执法要素.board.mei".to_string()),
            scene: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            tab: Some("preview".to_string()),
        },
    )
    .expect("resolved");
    assert_eq!(resolved.node.kind, BuildNodeKind::BoardFile);
    assert_eq!(
        resolved.node.encode(),
        "board-file:scenes/01-执法要素.board.mei"
    );
    assert_ne!(resolved.node.kind, BuildNodeKind::WorldFile);
}

#[test]
fn legacy_world_metric_maps_to_node() {
    let resolved = resolve_build_view_query(
        None,
        None,
        Some("preview"),
        &LegacyBuildQuery {
            file: Some("metrics.world.mei".to_string()),
            scene: None,
            world_metric: Some("total_amount".to_string()),
            world_dataset: None,
            explain: None,
            tab: None,
        },
    )
    .expect("resolved");
    assert_eq!(
        resolved.node.encode(),
        "world-metric:metrics.world.mei#total_amount"
    );
    assert_eq!(resolved.tab, BuildViewTab::Preview);
}

#[test]
fn legacy_source_tab_maps_to_overview() {
    assert_eq!(
        BuildViewTab::parse_slug("source"),
        Some(BuildViewTab::Overview)
    );
}

#[test]
fn projection_node_default_tab_is_preview() {
    let node = BuildNodeId::projection("home", "warning_board");
    assert_eq!(node.default_tab(), BuildViewTab::Preview);
    assert!(tab_visible_for_node(&node, BuildViewTab::Preview));
}
