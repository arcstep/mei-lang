use std::collections::BTreeMap;

use mei_lang_kernel::{CompiledApp, LoadedResource, SceneContract};
use serde_json::Value;

pub(super) fn attach_host_meta(
    mut props: Value,
    compiled: &CompiledApp,
    app_path: &str,
    theme_components: &serde_json::Value,
) -> Value {
    if let Some(map) = props.as_object_mut() {
        map.insert(
            "_mei".to_string(),
            serde_json::json!({
                "app_id": compiled.app_id,
                "app_path": app_path,
                "active_target_file": compiled.active_target_file,
                "step_api": format!("/api/sim/step/{}", app_path),
                "dataset_query_api": format!("/api/datasets/query/{}", app_path),
                "metric_query_api": format!("/api/datasets/metrics/{}", app_path),
                "components": theme_components.clone(),
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
                    if is_forbidden_legacy_resource_id(id) {
                        return Value::Null;
                    }
                    if let Some(resource) = resources.get(id) {
                        // 数据集：展开为 DatasetView + __mei_runtime_ref，供 runtime-query 解析
                        // dataset_id 并发起 /api/datasets/query（与 __ref:"data" 一致）。
                        // 若整包返回 LoadedResource，rows 在嵌套 `dataset` 下，前端拿不到 dataset_id。
                        if let Some(dataset) = resource.dataset.as_ref() {
                            return with_runtime_ref(
                                serde_json::to_value(dataset).unwrap_or(Value::Null),
                                serde_json::json!({
                                    "kind": "data",
                                    "dataset_id": id,
                                }),
                            );
                        }
                        return serde_json::to_value(resource).unwrap_or(Value::Null);
                    }
                }
            }
            if map.get("__ref").and_then(Value::as_str) == Some("scene") {
                return serde_json::to_value(scene_contract).unwrap_or(Value::Null);
            }
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some((dataset, dataset_id)) = resolve_data_ref(map, resources) {
                    return with_runtime_ref(
                        serde_json::to_value(dataset).unwrap_or(Value::Null),
                        serde_json::json!({
                            "kind": "data",
                            "dataset_id": dataset_id,
                        }),
                    );
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some((metric, dataset_id)) = resolve_metric_ref(map, resources) {
                    return with_runtime_ref(
                        serde_json::to_value(metric).unwrap_or(Value::Null),
                        serde_json::json!({
                            "kind": "metric",
                            "dataset_id": dataset_id,
                            "metric_id": map.get("id").and_then(Value::as_str).unwrap_or(""),
                        }),
                    );
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
                if let Some((metric, dataset_id)) = resolve_metric_ref(&compat, resources) {
                    return with_runtime_ref(
                        serde_json::to_value(metric).unwrap_or(Value::Null),
                        serde_json::json!({
                            "kind": "metric",
                            "dataset_id": dataset_id,
                            "metric_id": compat.get("id").and_then(Value::as_str).unwrap_or(""),
                        }),
                    );
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some((dataset, dataset_id)) = resolve_rows_expr(map, resources) {
                    return with_runtime_ref(
                        serde_json::to_value(dataset).unwrap_or(Value::Null),
                        serde_json::json!({
                            "kind": "data",
                            "dataset_id": dataset_id,
                        }),
                    );
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
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let id = map.get("id").and_then(Value::as_str)?;
    if is_forbidden_legacy_resource_id(id) {
        return None;
    }
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let dataset_id = from_dataset.unwrap_or(id);
    if is_forbidden_legacy_resource_id(dataset_id) && !resources.contains_key(dataset_id) {
        return None;
    }
    Some((
        resources.get(dataset_id)?.dataset.clone()?,
        dataset_id.to_string(),
    ))
}

fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<(mei_lang_kernel::MetricContract, String)> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(dataset_id) = map.get("from_dataset").and_then(Value::as_str) {
        if is_forbidden_legacy_resource_id(dataset_id) && !resources.contains_key(dataset_id) {
            return None;
        }
        let resource = resources.get(dataset_id)?;
        let metric = resource
            .dataset
            .as_ref()?
            .metrics
            .get(metric_id)
            .cloned()?;
        // 运行时 /api/datasets/metrics 使用 world 资源 id，不用 .mei 路径。
        let runtime_dataset_id = resource.id.clone();
        return Some((metric, runtime_dataset_id));
    }
    resources
        .iter()
        .filter_map(|(dataset_id, resource)| {
            resource
                .dataset
                .as_ref()
                .and_then(|dataset| dataset.metrics.get(metric_id).cloned())
                .map(|metric| (metric, dataset_id.clone()))
        })
        .next()
}

fn resolve_rows_expr(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let dataset = map
        .get("dataset")
        .and_then(Value::as_str)
        .map(|value| value.strip_prefix("dataset.").unwrap_or(value).to_string())?;
    if is_forbidden_legacy_resource_id(dataset.as_str()) {
        return None;
    }
    Some((resources.get(&dataset)?.dataset.clone()?, dataset))
}

fn with_runtime_ref(mut value: Value, runtime_ref: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.insert("__mei_runtime_ref".to_string(), runtime_ref);
    }
    value
}

fn is_forbidden_legacy_resource_id(id: &str) -> bool {
    id.trim() == "__source_path__" || id.trim().ends_with(".mei")
}
