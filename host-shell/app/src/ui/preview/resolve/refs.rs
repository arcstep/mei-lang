use std::collections::BTreeMap;

use mei_lang_kernel::{
    resolve_dataset_resource_id, resolve_runtime_metric_def_key, CompiledApp, LoadedResource,
    MetricContract, MetricShape, RuntimeResourceIndex, WorldMetricLedgerEntry,
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
    resource_id: &str,
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
    let def_key =
        resolve_runtime_metric_def_key(resource_id, metric_id, &dataset.runtime_metric_defs)?;
    let def = dataset.runtime_metric_defs.get(&def_key)?;
    Some(metric_contract_from_runtime_def(def, metric_id))
}

fn metric_shape_from_runtime_def(def: &Value) -> MetricShape {
    match def
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("dataframe")
    {
        "scalar" | "scalar_map" => MetricShape::Scalar,
        "series" | "timeseries" => MetricShape::Series,
        "table" => MetricShape::Table,
        _ => MetricShape::Dataframe,
    }
}

fn metric_contract_from_runtime_def(def: &Value, metric_id: &str) -> MetricContract {
    let map = def.as_object();
    MetricContract {
        id: map
            .and_then(|m| m.get("id"))
            .and_then(Value::as_str)
            .unwrap_or(metric_id)
            .to_string(),
        label: map
            .and_then(|m| m.get("label"))
            .and_then(Value::as_str)
            .map(str::to_string),
        unit: map
            .and_then(|m| m.get("unit"))
            .and_then(Value::as_str)
            .map(str::to_string),
        value_format: map.and_then(|m| m.get("value_format")).cloned(),
        purpose: None,
        shape: metric_shape_from_runtime_def(def),
        schema: map
            .and_then(|m| m.get("schema"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let obj = item.as_object()?;
                        Some(mei_lang_kernel::ColumnSchema {
                            name: obj
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            type_name: obj
                                .get("type")
                                .and_then(Value::as_str)
                                .unwrap_or("string")
                                .to_string(),
                            source: obj
                                .get("source")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            optional: obj
                                .get("optional")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            unit: obj.get("unit").and_then(Value::as_str).map(str::to_string),
                            normalize: obj
                                .get("normalize")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(str::to_string),
                            primary: obj
                                .get("primary")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            hidden: obj
                                .get("hidden")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        dataset: None,
        transforms: Vec::new(),
        value: Value::Null,
    }
}

pub(crate) fn normalize_v2_metric_ref(value: &Value) -> Option<Value> {
    let map = value.as_object()?;
    if map.get("__ref").and_then(Value::as_str) != Some("metric_ref") {
        return None;
    }
    let args = map.get("__args")?.as_object()?;
    let metric_id = args
        .get("arg0")
        .or_else(|| args.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    let bundle = args
        .get("bundle")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())?;
    Some(serde_json::json!({
        "__ref": "metric",
        "id": metric_id,
        "from_dataset": format!("__world_metrics__::{bundle}"),
    }))
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
        let metric = lookup_dataset_metric(resource.dataset.as_ref()?, metric_id, &dataset_id)?;
        return Some((metric, dataset_id));
    }
    resources
        .iter()
        .filter_map(|(dataset_id, resource)| {
            lookup_dataset_metric(resource.dataset.as_ref()?, metric_id, dataset_id)
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
