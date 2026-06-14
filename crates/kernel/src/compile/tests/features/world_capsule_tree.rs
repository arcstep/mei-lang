use std::collections::BTreeMap;

use crate::compile::source_tree_world::{
    build_world_semantic_index, enrich_source_tree_with_world_capsules,
};
use crate::compile::{compile_app_from_root_with_options, CompileOptions};
use crate::workspace::source_tree;
use crate::WorkspaceNode;

use super::super::harness::workspace_root;

fn walk_file_tree<'a>(nodes: &'a [WorkspaceNode], out: &mut Vec<&'a WorkspaceNode>) {
    for node in nodes {
        out.push(node);
        walk_file_tree(&node.children, out);
    }
}

#[test]
fn enrich_source_tree_world_capsule_children() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let target = "scenes/07-问题办理.world.mei";
    let mut tree = source_tree(app_root.as_path()).expect("source tree");
    let mut index_cache = BTreeMap::new();
    enrich_source_tree_with_world_capsules(app_root.as_path(), &mut tree, &mut index_cache);
    let mut nodes = Vec::new();
    walk_file_tree(&tree, &mut nodes);
    let file_node = nodes
        .into_iter()
        .find(|node| node.path == target && node.kind == "file")
        .unwrap_or_else(|| panic!("`{target}` missing from file_tree"));
    assert!(
        file_node
            .children
            .iter()
            .any(|child| child.kind == "world_group" && child.name == "datasets"),
        "expected datasets group: {:?}",
        file_node.children
    );
    assert!(
        file_node
            .children
            .iter()
            .any(|child| child.kind == "world_group" && child.name == "metrics"),
        "expected metrics group: {:?}",
        file_node.children
    );
    let datasets_group = file_node
        .children
        .iter()
        .find(|child| child.kind == "world_group" && child.name == "datasets")
        .expect("datasets group");
    assert_eq!(datasets_group.children.len(), 1);
    assert_eq!(
        datasets_group.children[0].world_dataset_id.as_deref(),
        Some("warning_list")
    );
    let metrics_group = file_node
        .children
        .iter()
        .find(|child| child.kind == "world_group" && child.name == "metrics")
        .expect("metrics group");
    let pending_metric = metrics_group
        .children
        .iter()
        .find(|child| child.world_metric_id.as_deref() == Some("warnings_pending_count"))
        .expect("warnings_pending_count metric node");
    assert_eq!(pending_metric.children.len(), 3);
    assert!(
        pending_metric
            .children
            .iter()
            .any(|child| child.explain_block_id.as_deref() == Some("composition_by_category"))
    );
    assert!(
        pending_metric
            .children
            .iter()
            .any(|child| child.explain_block_id.as_deref() == Some("composition_by_agency"))
    );
    assert!(
        pending_metric
            .children
            .iter()
            .any(|child| child.kind == "explain_block" && child.explain_block_id.is_some())
    );
}

#[test]
fn build_world_semantic_index_explain_blocks() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let target = "scenes/07-问题办理.world.mei";
    let index = build_world_semantic_index(app_root.as_path(), target)
        .unwrap_or_else(|| panic!("index for `{target}`"));
    assert_eq!(index.datasets.len(), 1);
    assert_eq!(index.datasets[0].id, "warning_list");
    assert!(index.datasets[0].filter_field_count >= 10);
    let pending = index
        .metrics
        .iter()
        .find(|metric| metric.id == "warnings_pending_count")
        .expect("warnings_pending_count");
    assert_eq!(pending.label.as_deref(), Some("待办"));
    assert_eq!(pending.unit.as_deref(), Some("件"));
    assert!(pending.note.as_ref().is_some_and(|note| note.contains("承办部门")));
    let composition = pending
        .explain
        .iter()
        .find(|block| block.id == "composition_by_category")
        .expect("composition_by_category");
    assert_eq!(composition.kind, "composition");
    assert_eq!(composition.by.as_deref(), Some("问题分类名称"));
}

#[test]
fn compile_world_capsule_preview_materializes_world_metrics() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let target = "scenes/07-问题办理.world.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let index = compiled
        .world_semantic_by_file
        .get(target)
        .unwrap_or_else(|| panic!("missing world_semantic_by_file for `{target}`"));
    assert!(!index.metrics.is_empty());
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("`{target}` direct preview should include __world_metrics__"));
    assert!(
        dataset
            .runtime_metric_defs
            .keys()
            .any(|key| key.contains("warnings_pending_count")),
        "expected warnings_pending_count in runtime_metric_defs: {:?}",
        dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
    );
}

#[test]
fn compile_world_capsule_preview_includes_dataset_table_component_assets() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let target = "scenes/01-执法要素.world.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    assert!(
        compiled
            .component_assets
            .iter()
            .any(|asset| asset.key == "dataset.table" && asset.tag == "mei-dataset-table"),
        "world capsule manage preview should ship dataset.table assets: {:?}",
        compiled.component_assets
    );
    let agency_objects = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "agency_objects")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("`agency_objects` should be materialized for `{target}`"));
    assert!(
        !agency_objects.rows.is_empty(),
        "agency_objects preview should include materialized rows"
    );
}
