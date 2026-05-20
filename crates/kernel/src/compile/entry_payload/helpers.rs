use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Diagnostic, LoadedResource, ResourceDecl, Severity, UiNodeDecl};

use super::super::decls::{
    LegacyDatasetDecl, LegacyDatasetNodeDecl, LegacyMetricPackDecl, LegacyMetricPackMetaDecl,
    LegacySourceDecl,
};

pub(super) fn partition_world_resources(
    resources: &[ResourceDecl],
) -> (Vec<ResourceDecl>, Vec<ResourceDecl>) {
    let mut normal = Vec::new();
    let mut dataset_like = Vec::new();
    for resource in resources {
        match resource.kind.as_str() {
            "dataset" | "dataset_view" | "metric_pack" => dataset_like.push(resource.clone()),
            _ => normal.push(resource.clone()),
        }
    }
    (normal, dataset_like)
}

pub(super) fn decode_world_dataset_decl(
    resource: ResourceDecl,
) -> std::result::Result<LegacyDatasetDecl, String> {
    let mut dataset_node = resource
        .dataset
        .as_ref()
        .and_then(|value| serde_json::from_value::<LegacyDatasetNodeDecl>(value.clone()).ok())
        .unwrap_or(LegacyDatasetNodeDecl {
            key: resource.id.clone(),
            kind: "dataframe".to_string(),
            columns: Vec::new(),
            normalize: BTreeMap::new(),
            rowset: None,
        });
    dataset_node.key = resource.id.clone();
    if resource.kind == "dataset_view" {
        dataset_node.kind = "dataset_view".to_string();
    }
    if dataset_node.key == "__source_path__" || dataset_node.key.ends_with(".mei") {
        return Err(format!(
            "dataset resource `{}` uses forbidden id `{}`",
            resource.id, dataset_node.key
        ));
    }
    let source = resource
        .source
        .as_ref()
        .map(|source| LegacySourceDecl {
            kind: Some(source.kind.clone()),
            file: if source.path.is_empty() {
                None
            } else {
                Some(source.path.clone())
            },
            path: if source.path.is_empty() {
                None
            } else {
                Some(source.path.clone())
            },
            sheet: source.sheet.clone(),
            header_row: source.header_row,
            preview_rows: source.preview_rows,
            page_size: source.page_size,
            max_page_size: source.max_page_size,
            table: source.table.clone(),
            query: source.query.clone(),
            connection: source.connection.clone(),
            ..LegacySourceDecl::default()
        })
        .unwrap_or_default();
    Ok(LegacyDatasetDecl {
        data_ref: Some(format!("dataset.{}", resource.id)),
        title: resource.title.clone(),
        source,
        dataset: dataset_node,
        metrics: resource.metrics.clone().unwrap_or_default(),
    })
}

pub(super) fn decode_world_metric_pack_decl(
    resource: ResourceDecl,
) -> std::result::Result<LegacyMetricPackDecl, String> {
    if resource.id == "__source_path__" || resource.id.ends_with(".mei") {
        return Err(format!(
            "metric pack resource `{}` uses forbidden id `{}`",
            resource.id, resource.id
        ));
    }
    Ok(LegacyMetricPackDecl {
        metric_pack: LegacyMetricPackMetaDecl {
            id: resource.id.clone(),
            purpose: resource.purpose.or(resource.title),
        },
        metrics: resource.metrics.unwrap_or_default(),
    })
}

pub(super) fn insert_resource_checked(
    resources: &mut Vec<LoadedResource>,
    resource: LoadedResource,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(index) = resources.iter().position(|item| item.id == resource.id) {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "duplicate_world_resource_id".to_string(),
            message: format!(
                "resource id `{}` is declared more than once in world-only mode",
                resource.id
            ),
            source_path: Some(target_file.to_string()),
        });
        resources[index] = resource;
    } else {
        resources.push(resource);
    }
}

pub(super) fn collect_asset_keys_from_nodes(nodes: &[UiNodeDecl], asset_keys: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            UiNodeDecl::Panel(panel) => collect_asset_keys_from_nodes(&panel.blocks, asset_keys),
            UiNodeDecl::Block(block) => {
                asset_keys.insert(block.use_key.clone());
            }
            UiNodeDecl::FrameRef(_) => {}
        }
    }
}
