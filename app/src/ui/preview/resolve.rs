use std::collections::BTreeMap;

use mei_lang_kernel::{
    dataset_materialize_cache_epoch, resolve_dataset_resource_id, resolve_dataset_selector_value,
    scene_payload_cache_epoch, CompiledApp, LoadedResource, RuntimeResourceIndex, SceneContract,
};
use serde_json::{json, Value};

/// Scene anchor injected into `__mei_runtime_ref` for scene-qualified runtime APIs.
#[derive(Debug, Clone)]
pub(super) struct RuntimeSceneAnchor {
    pub scene_id: String,
    pub scene_path: Option<String>,
}

impl RuntimeSceneAnchor {
    pub fn from_compiled(compiled: &CompiledApp) -> Self {
        let scene_id = compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                compiled
                    .scene_routes
                    .iter()
                    .find(|route| route.target_file == compiled.active_target_file)
                    .map(|route| route.scene_id.clone())
            })
            .unwrap_or_else(|| "default".to_string());
        let scene_path = compiled.active_target_file.trim().to_string();
        Self {
            scene_id,
            scene_path: if scene_path.is_empty() {
                None
            } else {
                Some(scene_path)
            },
        }
    }

    fn runtime_ref_extra(&self, kind: &str, dataset_id: &str, metric_id: Option<&str>) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("kind".to_string(), Value::String(kind.to_string()));
        obj.insert(
            "scene_id".to_string(),
            Value::String(self.scene_id.clone()),
        );
        if let Some(path) = self.scene_path.as_deref().filter(|s| !s.is_empty()) {
            obj.insert("scene_path".to_string(), Value::String(path.to_string()));
        }
        obj.insert(
            "dataset_id".to_string(),
            Value::String(dataset_id.to_string()),
        );
        if let Some(mid) = metric_id.filter(|s| !s.is_empty()) {
            obj.insert("metric_id".to_string(), Value::String(mid.to_string()));
        }
        Value::Object(obj)
    }
}

pub(super) fn attach_host_meta(
    mut props: Value,
    compiled: &CompiledApp,
    app_path: &str,
    theme_components: &serde_json::Value,
    preview_scene_path: Option<&str>,
) -> Value {
    let mut anchor = RuntimeSceneAnchor::from_compiled(compiled);
    if let Some(path) = preview_scene_path.map(str::trim).filter(|s| !s.is_empty()) {
        anchor.scene_path = Some(path.to_string());
    }
    let active_target_file = anchor
        .scene_path
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    if let Some(map) = props.as_object_mut() {
        map.insert(
            "_mei".to_string(),
            json!({
                "app_id": compiled.app_id,
                "app_path": app_path,
                "active_scene_id": anchor.scene_id,
                "active_target_file": active_target_file,
                "entry_target": active_target_file,
                "compile_epoch": format!(
                    "{}|{}|{}",
                    scene_payload_cache_epoch(),
                    dataset_materialize_cache_epoch(),
                    active_target_file
                ),
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
    scene_anchor: &RuntimeSceneAnchor,
    resource_index: &RuntimeResourceIndex,
    compiled: &CompiledApp,
) -> Value {
    match value {
        Value::Object(map) => {
            if matches!(
                map.get("__ref").and_then(Value::as_str),
                Some("dataset") | Some("resource") | Some("entity")
            ) {
                if let Some(canonical_id) =
                    resolve_dataset_selector_value(compiled, value, resource_index)
                {
                    if let Some(resource) = resources.get(&canonical_id) {
                        if let Some(dataset) = resource.dataset.as_ref() {
                            return with_runtime_ref(
                                serde_json::to_value(dataset).unwrap_or(Value::Null),
                                scene_anchor.runtime_ref_extra("data", &canonical_id, None),
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
                if let Some((dataset, dataset_id)) =
                    resolve_data_ref(map, resources, compiled, resource_index)
                {
                    return with_runtime_ref(
                        serde_json::to_value(dataset).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra("data", &dataset_id, None),
                    );
                }
                return Value::Null;
            }
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some((metric, dataset_id)) =
                    resolve_metric_ref(map, resources, compiled, resource_index)
                {
                    let metric_id = map.get("id").and_then(Value::as_str).unwrap_or("");
                    return with_runtime_ref(
                        serde_json::to_value(metric).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra("metric", &dataset_id, Some(metric_id)),
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
                if let Some((metric, dataset_id)) =
                    resolve_metric_ref(&compat, resources, compiled, resource_index)
                {
                    let metric_id = compat.get("id").and_then(Value::as_str).unwrap_or("");
                    return with_runtime_ref(
                        serde_json::to_value(metric).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra("metric", &dataset_id, Some(metric_id)),
                    );
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some((dataset, dataset_id)) =
                    resolve_rows_expr(map, resources, compiled, resource_index)
                {
                    return with_runtime_ref(
                        serde_json::to_value(dataset).unwrap_or(Value::Null),
                        scene_anchor.runtime_ref_extra("data", &dataset_id, None),
                    );
                }
                return Value::Null;
            }
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(
                    key.clone(),
                    resolve_value(
                        entry,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                    ),
                );
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    resolve_value(
                        item,
                        scene_contract,
                        resources,
                        scene_anchor,
                        resource_index,
                        compiled,
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::DatasetView, String)> {
    let id = map.get("id").and_then(Value::as_str)?;
    let from_dataset = map.get("from_dataset").and_then(Value::as_str);
    let selector = from_dataset.unwrap_or(id);
    let dataset_id = resolve_dataset_resource_id(compiled, selector, Some(resource_index)).ok()?;
    Some((
        resources.get(&dataset_id)?.dataset.clone()?,
        dataset_id,
    ))
}

fn resolve_metric_ref(
    map: &serde_json::Map<String, Value>,
    resources: &BTreeMap<String, LoadedResource>,
    compiled: &CompiledApp,
    resource_index: &RuntimeResourceIndex,
) -> Option<(mei_lang_kernel::MetricContract, String)> {
    let metric_id = map.get("id").and_then(Value::as_str)?;
    if let Some(entry) = compiled.world_metrics.get(metric_id) {
        if let Some(from_dataset) = map.get("from_dataset").and_then(Value::as_str) {
            let dataset_id =
                resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)).ok()?;
            if dataset_id != entry.owner_resource_id {
                return None;
            }
        }
        return Some((entry.metric.clone(), entry.owner_resource_id.clone()));
    }
    if let Some(from_dataset) = map.get("from_dataset").and_then(Value::as_str) {
        let dataset_id =
            resolve_dataset_resource_id(compiled, from_dataset, Some(resource_index)).ok()?;
        let resource = resources.get(&dataset_id)?;
        let metric = resource
            .dataset
            .as_ref()?
            .metrics
            .get(metric_id)
            .cloned()?;
        return Some((metric, dataset_id));
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

fn with_runtime_ref(mut value: Value, runtime_ref: Value) -> Value {
    if let Some(map) = value.as_object_mut() {
        map.insert("__mei_runtime_ref".to_string(), runtime_ref);
    }
    value
}
