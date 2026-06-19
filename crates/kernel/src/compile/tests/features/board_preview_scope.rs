use crate::compile::{compile_app_from_root_with_options, CompileOptions};
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
    );
    assert!(
        !compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "sibling board assembly should not be pre-warmed, keys: {:?}",
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
            diag.code == "missing_scene_export_selector"
                && matches!(diag.severity, Severity::Error)
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
    );
}
