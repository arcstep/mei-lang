use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::model::{
    Diagnostic, FrameDecl, LoadedResource, PanelDecl, ResourceDecl, Severity, UiNodeDecl,
};

use super::super::decls::{
    LegacyDatasetDecl, LegacyDatasetNodeDecl, LegacyMetricPackDecl, LegacyMetricPackMetaDecl,
    LegacySourceDecl,
};

pub(super) fn all_world_resource_decls(world: &crate::model::WorldDecl) -> Vec<ResourceDecl> {
    let mut all = world.resources.clone();
    all.extend(world.datasets.clone());
    all.extend(world.metric_packs.clone());
    all
}

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

/// Panel-ref source imports: only fill ids not already present (host scene world wins on conflict).
pub(crate) fn insert_resource_if_absent(resources: &mut Vec<LoadedResource>, resource: LoadedResource) {
    if resources.iter().any(|item| item.id == resource.id) {
        return;
    }
    resources.push(resource);
}

pub(crate) fn merge_panel_ref_source_resources(
    app_root: &Path,
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<&FrameDecl>,
    resources: &mut Vec<LoadedResource>,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_paths = BTreeSet::new();
    collect_panel_ref_scene_files_from_frames(frames, frame_default, &mut seen_paths);
    for path in seen_paths {
        match load_resources_from_capsule_file(app_root, path.as_str()) {
            Ok(capsule_resources) => {
                for resource in capsule_resources {
                    insert_resource_if_absent(resources, resource);
                }
            }
            Err(error) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "panel_ref_resources_load_failed".to_string(),
                    message: format!(
                        "failed to load world resources from panel_ref source `{path}`: {error}"
                    ),
                    source_path: Some(target_file.to_string()),
                });
            }
        }
    }
}

fn collect_panel_ref_scene_files_from_frames(
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<&FrameDecl>,
    out: &mut BTreeSet<String>,
) {
    let mut sources: Vec<&FrameDecl> = frames.values().collect();
    if let Some(frame) = frame_default {
        sources.push(frame);
    }
    for frame in sources {
        for slot in &frame.panels {
            let Some(expr) = crate::typed_refs::decode_ref_value(slot) else {
                continue;
            };
            if expr.kind != crate::typed_refs::RefKind::Panel {
                continue;
            }
            let path = expr
                .locator
                .scene_file
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty());
            if let Some(path) = path {
                out.insert(path.to_string());
            }
        }
    }
}

pub(crate) fn load_resources_from_capsule_file(
    app_root: &Path,
    relative_path: &str,
) -> anyhow::Result<Vec<LoadedResource>> {
    use super::super::load_external::load_world_from_file;
    use super::super::materialize::{materialize_legacy_datasets, materialize_metric_packs};
    use super::super::resources::load_resources;

    let world_decl = match load_world_from_file(app_root, relative_path, None) {
        Ok(decl) => decl,
        Err(_) => return Ok(Vec::new()),
    };

    let (normal_resources, dataset_resources) =
        partition_world_resources(&all_world_resource_decls(&world_decl));
    let mut resources = load_resources(app_root, &normal_resources)?;
    let mut world_dataset_decls = Vec::new();
    let mut world_metric_pack_decls = Vec::new();

    for resource in dataset_resources {
        if resource.id == "__source_path__" || resource.id.ends_with(".mei") {
            continue;
        }
        match resource.kind.as_str() {
            "dataset" | "dataset_view" => {
                if let Ok(decl) = decode_world_dataset_decl(resource.clone()) {
                    world_dataset_decls.push(decl);
                }
            }
            "metric_pack" => {
                if let Ok(decl) = decode_world_metric_pack_decl(resource.clone()) {
                    world_metric_pack_decls.push(decl);
                }
            }
            _ => {}
        }
    }

    if !world_dataset_decls.is_empty() {
        let derived = materialize_legacy_datasets(app_root, &resources, &world_dataset_decls)?;
        for resource in derived {
            insert_resource_if_absent(&mut resources, resource);
        }
    }
    if !world_metric_pack_decls.is_empty() {
        let derived = materialize_metric_packs(&resources, &world_metric_pack_decls)?;
        for resource in derived {
            insert_resource_if_absent(&mut resources, resource);
        }
    }

    Ok(resources)
}

pub(super) fn collect_asset_keys_from_nodes(nodes: &[UiNodeDecl], asset_keys: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            UiNodeDecl::Panel(panel) => collect_asset_keys_from_nodes(&panel.blocks, asset_keys),
            UiNodeDecl::Block(block) => {
                asset_keys.insert(block.use_key.clone());
            }
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}
