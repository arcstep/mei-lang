use std::collections::BTreeMap;

use mei_lang_kernel::{CompiledApp, LoadedResource, SceneContract};
use serde_json::Value;

pub(super) fn attach_host_meta(mut props: Value, compiled: &CompiledApp, app_path: &str) -> Value {
    if let Some(map) = props.as_object_mut() {
        map.insert(
            "_mei".to_string(),
            serde_json::json!({
                "app_id": compiled.app_id,
                "app_path": app_path,
                "entry_target": compiled.entry_target,
                "step_api": format!("/api/sim/step/{}", app_path),
            }),
        );
    }
    props
}

pub(super) fn resolve_value(
    value: &Value,
    scene_contract: &SceneContract,
    resources: &BTreeMap<String, LoadedResource>,
) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("world") {
                if let Some(id) = map.get("id").and_then(Value::as_str) {
                    if let Some(resource) = resources.get(id) {
                        return serde_json::to_value(resource).unwrap_or(Value::Null);
                    }
                }
            }
            if map.get("__ref").and_then(Value::as_str) == Some("scene") {
                return serde_json::to_value(scene_contract).unwrap_or(Value::Null);
            }
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(dataset) = resolve_data_ref(map, resources) {
                    return serde_json::to_value(dataset).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some(metric) = resolve_metric_ref(map, resources) {
                    return serde_json::to_value(metric).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            if map.get("metric").and_then(Value::as_str).is_some() {
                let mut compat = serde_json::Map::new();
                compat.insert("__ref".to_string(), Value::String("metric".to_string()));
                if let Some(id) = map.get("metric").cloned() {
                    compat.insert("id".to_string(), id);
                }
                if let Some(from) = map
                    .get("from_dataset")
                    .cloned()
                    .or_else(|| map.get("from").cloned())
                {
                    compat.insert("from_dataset".to_string(), from);
                }
                if let Some(metric) = resolve_metric_ref(&compat, resources) {
                    return serde_json::to_value(metric).unwrap_or(Value::Null);
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some(dataset) = resolve_rows_expr(map, resources) {
                    return serde_json::to_value(dataset).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), resolve_value(entry, scene_contract, resources));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_value(item, scene_contract, resources))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::DatasetView> {
    let id = map.get("id").and_then(Value::as_str)?;
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let dataset_id = from_dataset.unwrap_or(id);
    resources.get(dataset_id)?.dataset.clone()
}

fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::MetricContract> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(dataset_id) = map.get("from_dataset").and_then(Value::as_str) {
        return resources
            .get(dataset_id)?
            .dataset
            .as_ref()?
            .metrics
            .get(metric_id)
            .cloned();
    }
    resources
        .values()
        .filter_map(|resource| resource.dataset.as_ref())
        .find_map(|dataset| dataset.metrics.get(metric_id).cloned())
}

fn resolve_rows_expr(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<mei_lang_kernel::DatasetView> {
    let dataset = map
        .get("dataset")
        .and_then(Value::as_str)
        .map(|value| value.strip_prefix("dataset.").unwrap_or(value).to_string())?;
    resources.get(&dataset)?.dataset.clone()
}
