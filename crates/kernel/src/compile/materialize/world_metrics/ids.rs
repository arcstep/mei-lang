use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::MetricContract;

pub const WORLD_METRICS_RESOURCE_ID: &str = "__world_metrics__";

pub(crate) fn imported_world_metrics_resource_id(relative_path: &str) -> String {
    let path = relative_path.trim();
    if path.is_empty() {
        return WORLD_METRICS_RESOURCE_ID.to_string();
    }
    format!("{WORLD_METRICS_RESOURCE_ID}::{path}::metrics")
}

const IMPORTED_WORLD_METRICS_PREFIX: &str = "__world_metrics__::";
const IMPORTED_WORLD_METRICS_SUFFIX: &str = "::metrics";

/// 从 `__world_metrics__::{capsule_path}::metrics` 解析嵌入 capsule 的相对路径。
const CAPSULE_DATASET_MARKER: &str = ".mei::";

/// 从 `{capsule_path}.mei::{local_dataset_id}` 资源 id 提取 capsule 相对路径（含 `.mei`）。
pub fn capsule_path_from_namespaced_resource_id(resource_id: &str) -> Option<&str> {
    let resource_id = resource_id.trim();
    let idx = resource_id.find(CAPSULE_DATASET_MARKER)?;
    let path = resource_id[..idx + 4].trim();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// 将 `scenes/foo.mei::dataset_id` 还原为 capsule 内局部 dataset id（用于宿主短 id 回退）。
pub fn local_dataset_id_from_namespaced_token(dataset_id: &str) -> Option<&str> {
    let dataset_id = dataset_id.trim();
    let idx = dataset_id.find(CAPSULE_DATASET_MARKER)?;
    let local = dataset_id[idx + CAPSULE_DATASET_MARKER.len()..].trim();
    if local.is_empty() {
        None
    } else {
        Some(local)
    }
}

pub fn imported_capsule_path_from_world_metrics_resource_id(resource_id: &str) -> Option<String> {
    let resource_id = resource_id.trim();
    let inner = resource_id.strip_prefix(IMPORTED_WORLD_METRICS_PREFIX)?;
    let path = inner.strip_suffix(IMPORTED_WORLD_METRICS_SUFFIX)?;
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

/// 解析 runtime metric def 键：直查失败后，对 imported world metrics 尝试 `{capsule}::{local_id}`。
pub(crate) fn namespaced_world_metric_key(capsule_path: Option<&str>, local_key: &str) -> String {
    let local_key = local_key.trim();
    if local_key.is_empty() {
        return String::new();
    }
    if local_key.contains("::") {
        return local_key.to_string();
    }
    let Some(path) = capsule_path.map(str::trim).filter(|p| !p.is_empty()) else {
        return local_key.to_string();
    };
    format!("{path}::{local_key}")
}

fn rewrite_metric_def_identity(value: &mut Value, namespaced_key: &str) {
    let Value::Object(map) = value else {
        return;
    };
    for field in ["id", "key"] {
        let Some(id) = map
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if !id.contains("::") {
            map.insert(field.to_string(), Value::String(namespaced_key.to_string()));
        }
    }
}

pub(crate) fn namespace_runtime_metric_defs(
    defs: BTreeMap<String, Value>,
    capsule_path: Option<&str>,
) -> BTreeMap<String, Value> {
    let Some(path) = capsule_path.map(str::trim).filter(|p| !p.is_empty()) else {
        return defs;
    };
    let mut out = BTreeMap::new();
    for (key, mut value) in defs {
        let namespaced = namespaced_world_metric_key(Some(path), key.as_str());
        rewrite_metric_def_identity(&mut value, &namespaced);
        out.insert(namespaced, value);
    }
    out
}

pub fn resolve_runtime_metric_def_key(
    resource_id: &str,
    metric_id: &str,
    defs: &BTreeMap<String, Value>,
) -> Option<String> {
    resolve_storage_map_key(resource_id, metric_id, |key| defs.contains_key(key))
}

/// Resolve a compile-time metric snapshot key after artifact reload.
///
/// `runtime_metric_defs` are not persisted in scene payloads; access routes must
/// locate requested metrics via the serialized `metrics` map instead.
pub fn resolve_metric_contract_key(
    resource_id: &str,
    metric_id: &str,
    metrics: &BTreeMap<String, MetricContract>,
) -> Option<String> {
    resolve_storage_map_key(resource_id, metric_id, |key| metrics.contains_key(key))
}

fn resolve_storage_map_key<F>(resource_id: &str, metric_id: &str, contains_key: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    let metric_id = metric_id.trim();
    if metric_id.is_empty() {
        return None;
    }
    if contains_key(metric_id) {
        return Some(metric_id.to_string());
    }
    if let Some(resolved) =
        resolve_storage_map_key_without_rowset_fallback(resource_id, metric_id, &contains_key)
    {
        return Some(resolved);
    }
    resolve_storage_scalar_rowset_key(resource_id, metric_id, contains_key)
}

fn resolve_storage_scalar_rowset_key<F>(
    resource_id: &str,
    metric_id: &str,
    contains_key: F,
) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    let parent_metric_id = metric_id.strip_suffix("::__scalar_rowset__")?;
    let resolved_parent = resolve_storage_map_key_without_rowset_fallback(
        resource_id,
        parent_metric_id,
        &contains_key,
    )?;
    let rowset_metric_id = format!("{resolved_parent}::__scalar_rowset__");
    if contains_key(&rowset_metric_id) {
        return Some(rowset_metric_id);
    }
    if contains_key(&resolved_parent) {
        return Some(rowset_metric_id);
    }
    None
}

fn resolve_storage_map_key_without_rowset_fallback<F>(
    resource_id: &str,
    metric_id: &str,
    contains_key: &F,
) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    let metric_id = metric_id.trim();
    if metric_id.is_empty() {
        return None;
    }
    if contains_key(metric_id) {
        return Some(metric_id.to_string());
    }
    if let Some(capsule_path) = imported_capsule_path_from_world_metrics_resource_id(resource_id) {
        let namespaced = format!("{capsule_path}::{metric_id}");
        if contains_key(&namespaced) {
            return Some(namespaced);
        }
        if let Some(local) = metric_id.strip_prefix(&format!("{capsule_path}::")) {
            if contains_key(local) {
                return Some(local.to_string());
            }
        }
        return None;
    }
    if let Some(capsule_path) = capsule_path_from_namespaced_resource_id(resource_id) {
        let namespaced = format!("{capsule_path}::{metric_id}");
        if contains_key(&namespaced) {
            return Some(namespaced);
        }
    }
    if let Some(local) = local_dataset_id_from_namespaced_token(metric_id) {
        if contains_key(local) {
            return Some(local.to_string());
        }
    }
    None
}
