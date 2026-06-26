use crate::build_runtime_resource_index;
use crate::compile::{
    compile_app_from_root_with_options, compile_scene_from_build_node,
    compile_scene_from_build_node_with_app, preview_target_from_build_node_with_app,
    CompileOptions,
};
use crate::Severity;

use super::super::harness::workspace_root;

fn route_precompile_attempted(compiled: &crate::CompiledApp) -> Option<usize> {
    compiled.diagnostics.iter().find_map(|diag| {
        if diag.code != "route_precompile_stats" {
            return None;
        }
        diag.message.split(',').find_map(|part| {
            part.trim()
                .strip_prefix("routes_attempted=")
                .and_then(|value| value.parse().ok())
        })
    })
}

#[test]
fn board_preview_scope_compiles_single_export_when_scene_set() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let board_target = "scenes/01-执法要素.board.mei";
    let scene_id = "enforcement_personnel_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile enforcement personnel board preview");
    assert_eq!(compiled.active_scene.as_deref(), Some(scene_id));
    assert_eq!(compiled.active_target_file, board_target);
    assert_eq!(
        route_precompile_attempted(&compiled),
        Some(1),
        "multi-export board preview should precompile exactly one route, diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key(scene_id),
        "expected assembly for `{scene_id}`, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "finish should hydrate sibling board assemblies for build-view Boards tree, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(
        !compiled.build_board_index.boards.is_empty(),
        "board index should list all board capsules after finish hydrate"
    );
}

#[test]
fn board_build_node_compile_options_produce_board_resources() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let node = crate::model::BuildNodeId::board_file(
        "scenes/01-执法要素.board.mei#enforcement_units_analytics_board",
    );
    let scene = compile_scene_from_build_node(&node).expect("board scene export id");
    let preview_target =
        preview_target_from_build_node_with_app(&node, None).expect("board preview target");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene),
            preview_target: Some(preview_target),
        },
    )
    .expect("compile board capsule preview");
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enforcement_units_analytics_board")
    );
    assert_eq!(compiled.active_target_file, "scenes/01-执法要素.board.mei");
    assert!(
        !compiled.resources.is_empty(),
        "board preview should materialize scene resources, got {:?}",
        compiled
            .resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    let index = build_runtime_resource_index(&compiled);
    assert!(
        index
            .canonical_id("__world_metrics__::scenes/01-执法要素.mei::metrics")
            .is_some()
            || compiled
                .resources
                .iter()
                .any(|r| r.id.contains("world_metrics")),
        "board preview should expose world metrics dataset aliases"
    );
}

#[test]
fn board_preview_scope_requires_scene_for_multi_export_file() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let board_target = "scenes/01-执法要素.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile multi-export board without scene should return diagnostics");
    assert!(
        compiled.diagnostics.iter().any(|diag| {
            diag.code == "missing_scene_export_selector" && matches!(diag.severity, Severity::Error)
        }),
        "expected missing_scene_export_selector error, got: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn board_preview_scope_single_scene_capsule_without_scene_still_works() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-左栏.mei".to_string()),
        },
    )
    .expect("single-scene capsule preview without scene");
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "missing_scene_export_selector"),
        "single-scene capsule should not require scene selector: {:?}",
        compiled.diagnostics
    );
    assert_eq!(compiled.active_target_file, "scenes/layout-左栏.mei");
}

#[test]
fn board_scoped_compile_materializes_metric_defs_for_penalty_and_mechanism_boards() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");

    let penalty_node = crate::model::BuildNodeId::board_file(
        "scenes/04-行政处罚.board.mei#penalty_today_analytics_board",
    );
    let penalty_scene = compile_scene_from_build_node(&penalty_node).expect("penalty scene");
    let penalty_target =
        preview_target_from_build_node_with_app(&penalty_node, None).expect("penalty target");
    let penalty_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(penalty_scene),
            preview_target: Some(penalty_target),
        },
    )
    .expect("compile penalty today board preview");
    let penalty_dataset = penalty_compiled
        .resources
        .iter()
        .find(|resource| resource.id == "penalty_result_dashboard_ds")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("penalty_result_dashboard_ds resource");
    assert!(
        penalty_dataset.has_runtime_metric_defs(),
        "penalty board preview should materialize runtime_metric_defs, keys: {:?}",
        penalty_dataset
            .runtime_metric_defs
            .keys()
            .collect::<Vec<_>>()
    );

    let mechanism_node = crate::model::BuildNodeId::board_file(
        "scenes/_shared/mechanism-documents.board.mei#effect_mechanism_documents_board",
    );
    let mechanism_scene = compile_scene_from_build_node(&mechanism_node).expect("mechanism scene");
    let mechanism_target =
        preview_target_from_build_node_with_app(&mechanism_node, None).expect("mechanism target");
    let mechanism_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(mechanism_scene),
            preview_target: Some(mechanism_target),
        },
    )
    .expect("compile mechanism documents board preview");
    let world_metrics = mechanism_compiled
        .resources
        .iter()
        .find(|resource| {
            resource.id.starts_with("__world_metrics__")
                && resource
                    .dataset
                    .as_ref()
                    .is_some_and(|dataset| dataset.has_runtime_metric_defs())
        })
        .expect("mechanism board preview should expose imported world metrics defs");
    assert!(
        world_metrics
            .dataset
            .as_ref()
            .and_then(|dataset| dataset
                .runtime_metric_defs
                .get("effectiveness_mechanism_item_count"))
            .is_some()
            || world_metrics.dataset.as_ref().is_some_and(|dataset| {
                dataset
                    .runtime_metric_defs
                    .keys()
                    .any(|key| key.contains("mechanism"))
            }),
        "world metrics should include mechanism effectiveness metric, keys: {:?}",
        world_metrics
            .dataset
            .as_ref()
            .map(|dataset| dataset.runtime_metric_defs.keys().collect::<Vec<_>>())
    );
}

#[test]
fn board_scoped_compile_lists_only_entry_scenes_not_board_exports() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let node = crate::model::BuildNodeId::board_file(
        "scenes/01-执法要素.board.mei#enforcement_personnel_analytics_board",
    );
    let scene = compile_scene_from_build_node(&node).expect("board scene");
    let preview_target =
        preview_target_from_build_node_with_app(&node, None).expect("board preview target");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene),
            preview_target: Some(preview_target),
        },
    )
    .expect("compile board preview");
    let scenes = crate::compile::build_reachability_tree(&compiled)
        .into_iter()
        .find(|group| group.group == "scenes")
        .expect("scenes group");
    assert_eq!(
        scenes.children.len(),
        1,
        "board scoped compile should list entry scenes only, not board exports: {:?}",
        scenes
            .children
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        scenes.children[0].label.contains("home") || scenes.children[0].node_id.contains("home"),
        "expected home entry scene, got {:?}",
        scenes.children[0]
    );
    assert!(
        !scenes.children[0].children.is_empty(),
        "home scene should expose panel subtree for build tree navigation"
    );
}

#[test]
fn board_scoped_compile_does_not_mount_stock_templates_group() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let node = crate::model::BuildNodeId::board_file(
        "scenes/01-执法要素.board.mei#enforcement_personnel_analytics_board",
    );
    let scene = compile_scene_from_build_node(&node).expect("board scene");
    let preview_target =
        preview_target_from_build_node_with_app(&node, None).expect("board preview target");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene),
            preview_target: Some(preview_target),
        },
    )
    .expect("compile enforcement personnel board preview");
    let roots = crate::compile::build_reachability_tree(&compiled);
    let groups: Vec<_> = roots.iter().map(|group| group.group.as_str()).collect();
    assert!(
        !groups.contains(&"templates"),
        "business app build tree must not mount stock components group, got {groups:?}"
    );
    assert!(
        !groups.contains(&"template_files"),
        "business app build tree must not mount stock templates group, got {groups:?}"
    );
}

#[test]
fn parent_scene_preview_still_hydrates_referenced_board_assemblies() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile enforcement elements parent scene preview");
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "parent scene preview should still hydrate linked board assemblies, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn zhifa_compile_includes_boards_group_with_slots() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile zhifa");
    let boards = compiled
        .build_experience_index
        .reachability_snapshot
        .iter()
        .find(|group| group.group == "boards")
        .expect("boards group in reachability snapshot");
    assert!(
        boards.children.len() >= 3,
        "expected multiple board capsules, got {}: {:?}",
        boards.children.len(),
        boards
            .children
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>()
    );
    let warning_board = boards
        .children
        .iter()
        .find(|node| {
            node.label.contains("监督") || node.badges.iter().any(|badge| badge.contains("05"))
        })
        .expect("supervision warning board entry");
    assert!(
        !warning_board.children.is_empty(),
        "board file node should expose slot children: {:?}",
        warning_board
    );
    assert!(
        !compiled.build_board_index.boards.is_empty(),
        "board index should not be empty"
    );
}

#[test]
fn single_export_board_mei_is_listed_in_boards_group() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile zhifa");
    assert!(
        compiled
            .build_board_index
            .boards
            .keys()
            .any(|key| key.starts_with("scenes/_shared/mechanism-documents.board.mei#")),
        "single-export board capsule should be indexed, keys: {:?}",
        compiled.build_board_index.boards.keys().collect::<Vec<_>>()
    );
    let boards = crate::compile::build_reachability_tree(&compiled)
        .into_iter()
        .find(|group| group.group == "boards")
        .expect("boards group");
    assert!(
        boards.children.iter().any(|node| node
            .badges
            .iter()
            .any(|badge| badge.contains("mechanism-documents"))),
        "boards tree should list single-export capsule: {:?}",
        boards.children
    );
}

#[test]
fn world_file_board_node_resolves_default_export_scene_for_multi_export_file() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let board_target = "scenes/02-行政检查.board.mei";
    let baseline = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: None,
        },
    )
    .expect("compile zhifa baseline for board index");
    let node = crate::model::BuildNodeId::new(
        crate::model::BuildNodeKind::WorldFile,
        board_target.to_string(),
    );
    let scene = compile_scene_from_build_node_with_app(&node, Some(&baseline))
        .expect("world-file board node should resolve a default export scene");
    assert_eq!(scene, "ai_warning_cockpit_board");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene),
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile world-file board preview with resolved scene");
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|diag| diag.code == "missing_scene_export_selector"),
        "world-file board preview should compile once scene is resolved: {:?}",
        compiled.diagnostics
    );
    assert_eq!(compiled.active_target_file, board_target);
}
