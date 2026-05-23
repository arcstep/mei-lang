use std::collections::BTreeMap;

use crate::model::{DatasetView, LoadedResource};

fn dataset_schema_width(dataset: &DatasetView) -> usize {
    if !dataset.schema.is_empty() {
        return dataset.schema.len();
    }
    dataset.columns.len()
}

pub(crate) fn merge_dataset_resource(existing: &mut LoadedResource, incoming: LoadedResource) {
    let Some(incoming_ds) = incoming.dataset.as_ref() else {
        return;
    };
    let Some(existing_ds) = existing.dataset.as_mut() else {
        existing.dataset = incoming.dataset.clone();
        return;
    };
    for (metric_id, metric) in &incoming_ds.metrics {
        existing_ds
            .metrics
            .insert(metric_id.clone(), metric.clone());
    }
    for (metric_id, raw) in &incoming_ds.runtime_metric_defs {
        existing_ds
            .runtime_metric_defs
            .insert(metric_id.clone(), raw.clone());
    }
    if dataset_schema_width(incoming_ds) > dataset_schema_width(existing_ds) {
        existing_ds.schema = incoming_ds.schema.clone();
        existing_ds.stage_schema = incoming_ds.stage_schema.clone();
        existing_ds.columns = incoming_ds.columns.clone();
        existing_ds.rows = incoming_ds.rows.clone();
        existing_ds.source = incoming_ds.source.clone();
        existing_ds.sources = incoming_ds.sources.clone();
        if let Some(title) = incoming_ds.title.as_ref().filter(|s| !s.is_empty()) {
            existing_ds.title = Some(title.clone());
        }
    }
}

pub(crate) fn upsert_catalog_dataset_resource(
    by_id: &mut BTreeMap<String, LoadedResource>,
    resource: LoadedResource,
) {
    let id = resource.id.clone();
    if resource.dataset.is_none() {
        by_id.insert(id, resource);
        return;
    }
    match by_id.get_mut(&id) {
        None => {
            by_id.insert(id, resource);
        }
        Some(existing) => {
            merge_dataset_resource(existing, resource);
        }
    }
}
