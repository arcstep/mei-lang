//! UI 数据绑定策略：scene 的 panel / block 树中禁止直连行集（`ds.data_ref` 物化形态）。
//!
//! 组件 props 应使用本地 id 的 `dataset_ref` / `metric_ref` / `resource_ref`；
//! `world_ref` 仅用于 `scene.world` 单例槽位，不得作为资源选择器。

mod binding_scan;
mod imported_refs;
mod resource_refs;
mod rules;

use crate::compile::entry_payload::import_scope;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::model::{Diagnostic, LoadedResource, SceneContract, UiNodeDecl};

use binding_scan::{
    scan_deprecated_embed_nodes, scan_panel_props, scan_ui_node, validate_embed_capsule_ui_bindings,
};
use imported_refs::{scan_panel_imported_refs, scan_ui_node_imported_refs};

pub(crate) const IMPORTED_RESOURCE_DOC: &str =
    "see docs/mei-lang-v1/implementation/syntax/12-public-scene-capsule-migration-and-diagnostics.md";

pub(super) fn validate_imported_catalog_world_refs(
    contract: &SceneContract,
    authorized_resources: &[LoadedResource],
    merged_resources: &[LoadedResource],
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authorized_ids: BTreeSet<String> = authorized_resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    let merged_ids: BTreeSet<String> = merged_resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    for panel in &contract.panels {
        scan_panel_imported_refs(
            panel,
            &authorized_ids,
            &merged_ids,
            target_file,
            diagnostics,
        );
        for node in &panel.blocks {
            scan_ui_node_imported_refs(
                node,
                &authorized_ids,
                &merged_ids,
                target_file,
                diagnostics,
            );
        }
    }
}

pub(super) fn validate_scene_ui_data_bindings(
    contract: &SceneContract,
    resources: &[LoadedResource],
    app_root: &Path,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let host_resource_ids = import_scope::host_local_resource_ids(resources);
    let host_metric_ids: BTreeSet<String> = resources
        .iter()
        .filter(|resource| !resource.id.contains("::"))
        .filter_map(|resource| resource.dataset.as_ref())
        .flat_map(|dataset| dataset.metrics.keys().cloned())
        .chain(
            contract
                .world
                .as_ref()
                .into_iter()
                .flat_map(|world| world.metrics.iter())
                .filter_map(|metric| {
                    metric
                        .get("key")
                        .or_else(|| metric.get("id"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToString::to_string)
                }),
        )
        .collect();
    let merged_resource_ids: BTreeSet<String> = resources
        .iter()
        .map(|resource| resource.id.clone())
        .collect();
    let merged_metric_ids: BTreeSet<String> = resources
        .iter()
        .filter_map(|resource| resource.dataset.as_ref())
        .flat_map(|dataset| dataset.metrics.keys().cloned())
        .collect();
    for panel in &contract.panels {
        scan_panel_props(
            panel,
            &host_resource_ids,
            &host_metric_ids,
            &merged_resource_ids,
            &merged_metric_ids,
            target_file,
            diagnostics,
        );
        for node in &panel.blocks {
            scan_ui_node(
                node,
                &host_resource_ids,
                &host_metric_ids,
                &merged_resource_ids,
                &merged_metric_ids,
                target_file,
                diagnostics,
            );
            scan_deprecated_embed_nodes(node, target_file, diagnostics);
            if let UiNodeDecl::PanelRefEmbed(embed) = node {
                validate_embed_capsule_ui_bindings(
                    app_root,
                    embed,
                    resources,
                    target_file,
                    diagnostics,
                );
            }
        }
    }
}
