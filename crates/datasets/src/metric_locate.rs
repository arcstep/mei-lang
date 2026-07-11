use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    imported_capsule_path_from_world_metrics_resource_id, locate_dataset_resource,
    resolve_metric_contract_key, resolve_runtime_metric_def_key, CompiledApp, DatasetView,
    LoadedResource,
};

const WORLD_METRICS_RESOURCE_ID: &str = "__world_metrics__";

pub struct AccessMetricEvalPlan<'a> {
    pub primary: &'a LoadedResource,
    pub primary_dataset: &'a DatasetView,
    pub owner: &'a LoadedResource,
    pub owner_dataset: &'a DatasetView,
    pub request_metric_ids: Vec<String>,
}

fn try_resolve_metric_on_resource<'a>(
    resource: &'a LoadedResource,
    metric_id: &str,
) -> Option<(&'a LoadedResource, String)> {
    let dataset = resource.dataset.as_ref()?;
    if dataset.has_runtime_metric_defs() {
        let resolved =
            resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)?;
        return Some((resource, resolved));
    }
    if dataset.uses_compiled_metric_snapshot_only() {
        let resolved = resolve_metric_contract_key(&resource.id, metric_id, &dataset.metrics)?;
        return Some((resource, resolved));
    }
    None
}

fn world_metrics_capsule_from_metric_id(metric_id: &str) -> Option<String> {
    metric_id
        .split("::")
        .next()
        .map(str::trim)
        .filter(|segment| segment.starts_with("scenes/") && segment.ends_with(".mei"))
        .map(str::to_string)
}

fn imported_world_metrics_resource_id_for_capsule(capsule_path: &str) -> String {
    format!("__world_metrics__::{capsule_path}::metrics")
}

fn try_resolve_metric_on_imported_world_metrics<'a>(
    compiled: &'a CompiledApp,
    metric_id: &str,
) -> Option<(&'a LoadedResource, String)> {
    let capsule_path = world_metrics_capsule_from_metric_id(metric_id)?;
    let resource_id = imported_world_metrics_resource_id_for_capsule(capsule_path.as_str());
    let resource = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)?;
    try_resolve_metric_on_resource(resource, metric_id)
}

fn try_resolve_metric_on_any_world_metrics<'a>(
    compiled: &'a CompiledApp,
    metric_id: &str,
) -> Option<(&'a LoadedResource, String)> {
    for resource in &compiled.resources {
        if !resource.id.starts_with("__world_metrics__") {
            continue;
        }
        if let Some(resolved) = try_resolve_metric_on_resource(resource, metric_id) {
            return Some(resolved);
        }
    }
    None
}

pub fn locate_runtime_metric_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
    metric_id: &str,
) -> Result<(&'a LoadedResource, String)> {
    let primary =
        locate_dataset_resource(compiled, dataset_id).map_err(|error| anyhow!("{error}"))?;
    if let Some(resolved) = try_resolve_metric_on_resource(primary, metric_id) {
        return Ok(resolved);
    }
    if let Some(resolved) = try_resolve_metric_on_imported_world_metrics(compiled, metric_id) {
        return Ok(resolved);
    }
    if imported_capsule_path_from_world_metrics_resource_id(dataset_id).is_some() {
        if let Ok(host) = locate_dataset_resource(compiled, WORLD_METRICS_RESOURCE_ID) {
            if let Some(resolved) = try_resolve_metric_on_resource(host, metric_id) {
                return Ok(resolved);
            }
        }
    }
    for resource in &compiled.resources {
        if resource.dataset.is_none() {
            continue;
        }
        if let Some(resolved) = try_resolve_metric_on_resource(resource, metric_id) {
            return Ok(resolved);
        }
    }
    if let Some(resolved) = try_resolve_metric_on_any_world_metrics(compiled, metric_id) {
        return Ok(resolved);
    }
    if primary.dataset.as_ref().is_some_and(|dataset| {
        !dataset.has_runtime_metric_defs() && !dataset.uses_compiled_metric_snapshot_only()
    }) {
        return Err(anyhow!("dataset `{dataset_id}` has no runtime metric defs"));
    }
    Err(anyhow!(
        "metric `{metric_id}` not found on dataset `{dataset_id}`"
    ))
}

fn find_world_metrics_resource<'a>(compiled: &'a CompiledApp) -> Option<&'a LoadedResource> {
    compiled.resources.iter().find(|resource| {
        resource.dataset.as_ref().is_some_and(|dataset| {
            if dataset.has_runtime_metric_defs() {
                return resource.id == WORLD_METRICS_RESOURCE_ID
                    || resource.id.starts_with("__world_metrics__::");
            }
            resource.id.starts_with("__world_metrics__") && !dataset.metrics.is_empty()
        })
    })
}

fn plan_access_metric_eval<'a>(
    compiled: &'a CompiledApp,
    dataset_selector: &str,
) -> Result<AccessMetricEvalPlan<'a>> {
    let primary =
        locate_dataset_resource(compiled, dataset_selector).map_err(|error| anyhow!("{error}"))?;
    let primary_dataset = primary
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_selector}` is not a dataset"))?;
    if primary_dataset.has_runtime_metric_defs() {
        return Ok(AccessMetricEvalPlan {
            primary,
            primary_dataset,
            owner: primary,
            owner_dataset: primary_dataset,
            request_metric_ids: primary_dataset
                .runtime_metric_defs
                .keys()
                .take(64)
                .cloned()
                .collect(),
        });
    }
    if primary_dataset.uses_compiled_metric_snapshot_only() {
        return Ok(AccessMetricEvalPlan {
            primary,
            primary_dataset,
            owner: primary,
            owner_dataset: primary_dataset,
            request_metric_ids: primary_dataset.metrics.keys().take(64).cloned().collect(),
        });
    }
    let owner = find_world_metrics_resource(compiled)
        .ok_or_else(|| anyhow!("dataset `{dataset_selector}` has no runtime metric defs"))?;
    let owner_dataset = owner
        .dataset
        .as_ref()
        .expect("world metrics resource should expose dataset view");
    let request_metric_ids = if !owner_dataset.runtime_metric_defs.is_empty() {
        owner_dataset
            .runtime_metric_defs
            .keys()
            .take(64)
            .cloned()
            .collect()
    } else {
        owner_dataset.metrics.keys().take(64).cloned().collect()
    };
    Ok(AccessMetricEvalPlan {
        primary,
        primary_dataset,
        owner,
        owner_dataset,
        request_metric_ids,
    })
}

pub fn plan_access_metric_eval_for_ids<'a>(
    compiled: &'a CompiledApp,
    dataset_selector: &str,
    metric_ids: &[String],
) -> Result<AccessMetricEvalPlan<'a>> {
    if metric_ids.is_empty() {
        return plan_access_metric_eval(compiled, dataset_selector);
    }
    let primary =
        locate_dataset_resource(compiled, dataset_selector).map_err(|error| anyhow!("{error}"))?;
    let primary_dataset = primary
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_selector}` is not a dataset"))?;

    let mut owner_resource: Option<&LoadedResource> = None;
    let mut request_metric_ids = Vec::with_capacity(metric_ids.len());
    for metric_id in metric_ids {
        let trimmed = metric_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (resource, _resolved) =
            locate_runtime_metric_resource(compiled, dataset_selector, trimmed)?;
        if let Some(existing) = owner_resource {
            if existing.id != resource.id {
                return Err(anyhow!(
                    "requested metrics span multiple metric owners (`{}` vs `{}`); query one owner at a time",
                    existing.id,
                    resource.id
                ));
            }
        } else {
            owner_resource = Some(resource);
        }
        request_metric_ids.push(trimmed.to_string());
    }
    if request_metric_ids.is_empty() {
        return Err(anyhow!("at least one metric id is required"));
    }
    let owner = owner_resource.expect("owner resource should be set");
    let owner_dataset = owner
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{}` is not a dataset", owner.id))?;
    Ok(AccessMetricEvalPlan {
        primary,
        primary_dataset,
        owner,
        owner_dataset,
        request_metric_ids,
    })
}

pub fn metric_ids_visible_for_dataset(
    compiled: &CompiledApp,
    primary_dataset: &DatasetView,
    world_metrics_decl: Option<&BTreeMap<String, serde_json::Value>>,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    if !primary_dataset.runtime_metric_defs.is_empty() {
        ids.extend(primary_dataset.runtime_metric_defs.keys().take(64).cloned());
    } else if !primary_dataset.metrics.is_empty() {
        ids.extend(primary_dataset.metrics.keys().take(64).cloned());
    } else if let Some(metrics) = world_metrics_decl {
        ids.extend(metrics.keys().take(64).cloned());
    }
    if ids.is_empty() {
        if let Some(owner) = find_world_metrics_resource(compiled) {
            if let Some(dataset) = owner.dataset.as_ref() {
                if !dataset.runtime_metric_defs.is_empty() {
                    ids.extend(dataset.runtime_metric_defs.keys().take(64).cloned());
                } else {
                    ids.extend(dataset.metrics.keys().take(64).cloned());
                }
            }
        }
    }
    ids.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{MetricContract, MetricShape, SourceDecl};
    use serde_json::json;

    fn orders_dataset() -> DatasetView {
        DatasetView {
            id: "orders".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["amount".to_string()],
            rows: vec![json!({"amount": 10})],
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:orders".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }
    }

    fn world_metrics_dataset() -> DatasetView {
        let mut runtime_metric_defs = BTreeMap::new();
        runtime_metric_defs.insert(
            "orders_total".to_string(),
            json!({"id": "orders_total", "shape": "scalar_map"}),
        );
        let mut runtime_analysis_contracts = BTreeMap::new();
        runtime_analysis_contracts.insert(
            "orders_total".to_string(),
            json!({
                "focus_node_id": "orders_total",
                "root_dataset_id": "orders",
                "title": "订单总额",
                "tabs": ["definition"],
            }),
        );
        DatasetView {
            id: WORLD_METRICS_RESOURCE_ID.to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "world_metrics".to_string(),
                path: String::new(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::from([(
                "orders_total".to_string(),
                MetricContract {
                    id: "orders_total".to_string(),
                    label: None,
                    unit: None,
                    value_format: None,
                    purpose: None,
                    shape: MetricShape::Scalar,
                    schema: Vec::new(),
                    dataset: None,
                    transforms: Vec::new(),
                    value: json!(1),
                },
            )]),
            runtime_metric_defs,
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts,
        }
    }

    fn compiled_with_split_metrics() -> CompiledApp {
        CompiledApp {
            app_id: "test".to_string(),
            title: String::new(),
            app_root: String::new(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: vec![
                LoadedResource {
                    id: "orders".to_string(),
                    kind: "dataset".to_string(),
                    title: None,
                    document: None,
                    dataset: Some(orders_dataset()),
                },
                LoadedResource {
                    id: WORLD_METRICS_RESOURCE_ID.to_string(),
                    kind: "dataset".to_string(),
                    title: None,
                    document: None,
                    dataset: Some(world_metrics_dataset()),
                },
            ],
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        }
    }

    #[test]
    fn locate_runtime_metric_resource_finds_world_metrics_owner() {
        let compiled = compiled_with_split_metrics();
        let (owner, resolved) =
            locate_runtime_metric_resource(&compiled, "orders", "orders_total").expect("locate");
        assert_eq!(owner.id, WORLD_METRICS_RESOURCE_ID);
        assert_eq!(resolved, "orders_total");
    }

    #[test]
    fn locate_runtime_metric_resource_finds_imported_world_metrics_rowset_owner() {
        let parent = "scenes/03-指标体系.mei::inspection_frequency_reduction_rate";
        let resource_id = "__world_metrics__::scenes/03-指标体系.mei::metrics";
        let mut runtime_metric_defs = BTreeMap::new();
        runtime_metric_defs.insert(
            parent.to_string(),
            json!({"id": parent, "shape": "scalar_map"}),
        );
        let compiled = CompiledApp {
            app_id: "test".to_string(),
            title: String::new(),
            app_root: String::new(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: vec![
                LoadedResource {
                    id: "administrative_inspection".to_string(),
                    kind: "dataset".to_string(),
                    title: None,
                    document: None,
                    dataset: Some(orders_dataset()),
                },
                LoadedResource {
                    id: resource_id.to_string(),
                    kind: "dataset".to_string(),
                    title: None,
                    document: None,
                    dataset: Some(DatasetView {
                        id: resource_id.to_string(),
                        title: None,
                        purpose: None,
                        schema: Vec::new(),
                        stage_schema: Vec::new(),
                        columns: Vec::new(),
                        rows: Vec::new(),
                        source: SourceDecl {
                            kind: "world_metrics".to_string(),
                            path: String::new(),
                            sheet: None,
                            header_row: None,
                            preview_rows: None,
                            page_size: None,
                            max_page_size: None,
                            table: None,
                            query: None,
                            connection: None,
                            content: None,
                        },
                        sources: Vec::new(),
                        metrics: BTreeMap::new(),
                        runtime_metric_defs,
                        runtime_analysis_graph: Default::default(),
                        runtime_analysis_contracts: Default::default(),
                    }),
                },
            ],
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        };
        let (owner, resolved) = locate_runtime_metric_resource(
            &compiled,
            "administrative_inspection",
            "scenes/03-指标体系.mei::inspection_frequency_reduction_rate::__scalar_rowset__",
        )
        .expect("locate imported rowset");
        assert_eq!(owner.id, resource_id);
        assert_eq!(
            resolved,
            "scenes/03-指标体系.mei::inspection_frequency_reduction_rate::__scalar_rowset__"
        );
    }

    #[test]
    fn plan_access_metric_eval_for_ids_uses_world_metrics_owner() {
        let compiled = compiled_with_split_metrics();
        let plan =
            plan_access_metric_eval_for_ids(&compiled, "orders", &["orders_total".to_string()])
                .expect("plan");
        assert_eq!(plan.primary.id, "orders");
        assert_eq!(plan.owner.id, WORLD_METRICS_RESOURCE_ID);
        assert!(plan.primary_dataset.runtime_metric_defs.is_empty());
        assert!(!plan.owner_dataset.runtime_metric_defs.is_empty());
    }

    #[test]
    fn locate_runtime_metric_resource_uses_compiled_metric_snapshot_without_runtime_defs() {
        let resource_id = "__world_metrics__::scenes/01-执法要素.mei::metrics";
        let metric_key = "scenes/01-执法要素.mei::enforcement_items_count";
        let compiled = CompiledApp {
            app_id: "test".to_string(),
            title: String::new(),
            app_root: String::new(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: String::new(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: vec![LoadedResource {
                id: resource_id.to_string(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: Some(DatasetView {
                    id: resource_id.to_string(),
                    title: None,
                    purpose: None,
                    schema: Vec::new(),
                    stage_schema: Vec::new(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    source: SourceDecl {
                        kind: "world_metrics".to_string(),
                        path: String::new(),
                        sheet: None,
                        header_row: None,
                        preview_rows: None,
                        page_size: None,
                        max_page_size: None,
                        table: None,
                        query: None,
                        connection: None,
                        content: None,
                    },
                    sources: Vec::new(),
                    metrics: BTreeMap::from([(
                        metric_key.to_string(),
                        MetricContract {
                            id: metric_key.to_string(),
                            label: Some("items".to_string()),
                            unit: None,
                            value_format: None,
                            purpose: None,
                            shape: MetricShape::Scalar,
                            schema: Vec::new(),
                            dataset: None,
                            transforms: Vec::new(),
                            value: json!(42),
                        },
                    )]),
                    runtime_metric_defs: BTreeMap::new(),
                    runtime_analysis_graph: Default::default(),
                    runtime_analysis_contracts: Default::default(),
                }),
            }],
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        };
        let (owner, resolved) = locate_runtime_metric_resource(&compiled, resource_id, metric_key)
            .expect("locate snapshot metric");
        assert_eq!(owner.id, resource_id);
        assert_eq!(resolved, metric_key);
    }
}
