//! Resolve runtime metric defs across business datasets and `__world_metrics__`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    imported_capsule_path_from_world_metrics_resource_id, locate_dataset_resource,
    resolve_runtime_metric_def_key, CompiledApp, DatasetView, LoadedResource,
};

const WORLD_METRICS_RESOURCE_ID: &str = "__world_metrics__";

pub(crate) struct AccessMetricEvalPlan<'a> {
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
    if !dataset.has_runtime_metric_defs() {
        return None;
    }
    let resolved =
        resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)?;
    Some((resource, resolved))
}

pub(crate) fn locate_runtime_metric_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
    metric_id: &str,
) -> Result<(&'a LoadedResource, String)> {
    let primary =
        locate_dataset_resource(compiled, dataset_id).map_err(|error| anyhow!("{error}"))?;
    if let Some(resolved) = try_resolve_metric_on_resource(primary, metric_id) {
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
    if primary
        .dataset
        .as_ref()
        .is_some_and(|dataset| !dataset.has_runtime_metric_defs())
    {
        return Err(anyhow!("dataset `{dataset_id}` has no runtime metric defs"));
    }
    Err(anyhow!(
        "metric `{metric_id}` not found on dataset `{dataset_id}`"
    ))
}

fn find_world_metrics_resource<'a>(compiled: &'a CompiledApp) -> Option<&'a LoadedResource> {
    compiled.resources.iter().find(|resource| {
        resource
            .dataset
            .as_ref()
            .is_some_and(|dataset| dataset.has_runtime_metric_defs())
            && (resource.id == WORLD_METRICS_RESOURCE_ID
                || resource.id.starts_with("__world_metrics__::"))
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
    let owner = find_world_metrics_resource(compiled)
        .ok_or_else(|| anyhow!("dataset `{dataset_selector}` has no runtime metric defs"))?;
    let owner_dataset = owner
        .dataset
        .as_ref()
        .expect("world metrics resource should expose dataset view");
    Ok(AccessMetricEvalPlan {
        primary,
        primary_dataset,
        owner,
        owner_dataset,
        request_metric_ids: owner_dataset
            .runtime_metric_defs
            .keys()
            .take(64)
            .cloned()
            .collect(),
    })
}

pub(crate) fn plan_access_metric_eval_for_ids<'a>(
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

pub(crate) fn metric_ids_visible_for_dataset(
    compiled: &CompiledApp,
    primary_dataset: &DatasetView,
    world_metrics_decl: Option<&BTreeMap<String, serde_json::Value>>,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    if !primary_dataset.runtime_metric_defs.is_empty() {
        ids.extend(primary_dataset.runtime_metric_defs.keys().take(64).cloned());
    } else if let Some(metrics) = world_metrics_decl {
        ids.extend(metrics.keys().take(64).cloned());
    }
    if ids.is_empty() {
        if let Some(owner) = find_world_metrics_resource(compiled) {
            if let Some(dataset) = owner.dataset.as_ref() {
                ids.extend(dataset.runtime_metric_defs.keys().take(64).cloned());
            }
        }
    }
    ids.into_iter().collect()
}
