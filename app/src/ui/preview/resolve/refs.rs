use std::collections::BTreeMap;

use mei_lang_kernel::{
    resolve_dataset_resource_id, CompiledApp, LoadedResource, MetricContract, RuntimeResourceIndex,
    WorldMetricLedgerEntry,
};
use serde_json::Value;

fn metric_lookup_key_aliases(metric_id: &str) -> Vec<String> {
    let metric_id = metric_id.trim();
    if metric_id.is_empty() {
        return Vec::new();
    }
    let mut aliases = vec![metric_id.to_string()];
    if let Some((_, local)) = metric_id.rsplit_once("::") {
        if !local.is_empty() && !aliases.iter().any(|key| key == local) {
            aliases.push(local.to_string());
        }
    }
    aliases
}

fn lookup_world_metric_ledger_entry<'a>(
    compiled: &'a CompiledApp,
    metric_id: &str,
) -> Option<&'a WorldMetricLedgerEntry> {
    for alias in metric_lookup_key_aliases(metric_id) {
        if let Some(entry) = compiled.world_metrics.get(alias.as_str()) {
            return Some(entry);
        }
        let suffix = format!("::{alias}");
        if let Some((_, entry)) = compiled
            .world_metrics
            .iter()
            .find(|(key, _)| key.as_str() == alias.as_str() || key.ends_with(suffix.as_str()))
        {
            return Some(entry);
        }
    }
    None
}

fn lookup_dataset_metric(
    dataset: &mei_lang_kernel::DatasetView,
    metric_id: &str,
) -> Option<MetricContract> {
    for alias in metric_lookup_key_aliases(metric_id) {
        if let Some(metric) = dataset.metrics.get(alias.as_str()) {
            return Some(metric.clone());
        }
        let suffix = format!("::{alias}");
        if let Some((_, metric)) = dataset
            .metrics
            .iter()
            .find(|(key, _)| key.as_str() == alias.as_str() || key.ends_with(suffix.as_str()))
        {
            return Some(metric.clone());
        }
    }
    None
}

pub(crate) fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let id = map.get("id").and_then(Value::as_str)?;
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let selector = from_dataset.unwrap_or(id);
    let dataset_id = resolve_dataset_resource_id(compiled, selector, Some(resource_index)).ok()?;
    Some((resources.get(&dataset_id)?.dataset.clone()?, dataset_id))
}

/// `world.add_metric` / `world(metrics=...)` 物化进 ledger 时 owner 为 `__world_metrics__`（或带路径后缀），
/// 与 `metric_ref(..., from_dataset = "<源数据集>")` 中的 lineage 提示 id 不同，不应因此拒绝解析。
fn is_scene_direct_world_metric_owner(owner_resource_id: &str) -> bool {
    owner_resource_id == "__world_metrics__" || owner_resource_id.starts_with("__world_metrics__::")
}

pub(crate) fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::MetricContract, String)> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(entry) = lookup_world_metric_ledger_entry(compiled, metric_id) {
        if let Some(from_dataset) = map.get("from_dataset").and_then(Value::as_str) {
            match resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)) {
                Ok(dataset_id) => {
                    if dataset_id != entry.owner_resource_id
                        && !is_scene_direct_world_metric_owner(&entry.owner_resource_id)
                    {
                        return None;
                    }
                }
                Err(_) if !is_scene_direct_world_metric_owner(&entry.owner_resource_id) => {
                    return None;
                }
                Err(_) => {}
            }
        }
        return Some((entry.metric.clone(), entry.owner_resource_id.clone()));
    }
    if let Some(from_dataset) = map.get("from_dataset").and_then(Value::as_str) {
        let dataset_id =
            resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)).ok()?;
        let resource = resources.get(&dataset_id)?;
        let metric = lookup_dataset_metric(resource.dataset.as_ref()?, metric_id)?;
        return Some((metric, dataset_id));
    }
    resources
        .iter()
        .filter_map(|(dataset_id, resource)| {
            lookup_dataset_metric(resource.dataset.as_ref()?, metric_id)
                .map(|metric| (metric, dataset_id.clone()))
        })
        .next()
}

pub(crate) fn resolve_rows_expr(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let dataset = map
        .get("dataset")
        .and_then(Value::as_str)
        .map(|value| value.strip_prefix("dataset.").unwrap_or(value).to_string())?;
    let dataset_id = resolve_dataset_resource_id(compiled, &dataset, Some(resource_index)).ok()?;
    Some((resources.get(&dataset_id)?.dataset.clone()?, dataset_id))
}

pub(crate) fn with_runtime_ref(mut value: Value, runtime_ref: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.insert("__mei_runtime_ref".to_string(), runtime_ref);
    }
    value
}
