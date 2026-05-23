use std::collections::BTreeSet;

use serde_json::Value;


use super::rules::has_external_locator;

pub(super) fn collect_resource_ref_issues(
    value: &Value,
    path: &str,
    resource_ids: &BTreeSet<String>,
    metric_ids: &BTreeSet<String>,
) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            if let Some((code, message)) = resource_ref_issue(map, resource_ids, metric_ids) {
                out.push((path.to_string(), code, message));
            }
            for (key, child) in map {
                let next = format!("{path}.{key}");
                out.extend(collect_resource_ref_issues(
                    child, &next, resource_ids, metric_ids,
                ));
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                out.extend(collect_resource_ref_issues(
                    child, &next, resource_ids, metric_ids,
                ));
            }
        }
        _ => {}
    }
    out
}

pub(super) fn resource_ref_issue(
    map: &serde_json::Map<String, Value>,
    resource_ids: &BTreeSet<String>,
    metric_ids: &BTreeSet<String>,
) -> Option<(String, String)> {
    let ref_kind = map.get("__ref").and_then(Value::as_str)?;
    if ref_kind == "world" {
        return Some((
            "misused_world_ref_in_props".to_string(),
            "误用 `world_ref` 作资源选择器；`world_ref` 仅用于 scene.world 单例槽位，资源消费请用 dataset_ref/resource_ref/metric_ref".to_string(),
        ));
    }
    if ref_kind != "dataset"
        && ref_kind != "metric"
        && ref_kind != "resource"
        && ref_kind != "entity"
    {
        return None;
    }
    if has_external_locator(map) {
        return Some((
            "external_ref_requires_world_import".to_string(),
            "不得在 frame/component props 中直接跨文件引用；请先在 world 中引入该对象".to_string(),
        ));
    }
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if id.is_empty() {
        return Some((
            "invalid_resource_ref".to_string(),
            "缺少资源 id".to_string(),
        ));
    }
    if id == "__source_path__" || id.ends_with(".mei") {
        return Some((
            "invalid_resource_ref".to_string(),
            format!("资源 id `{id}` 已禁用；请使用稳定显式 id"),
        ));
    }
    if ref_kind == "metric" {
        if !metric_ids.contains(id) {
            return Some((
                "invalid_resource_ref".to_string(),
                format!("metric id `{id}` 未在当前 scene world metric 账本中可见"),
            ));
        }
        return None;
    }
    if !resource_ids.contains(id) {
        return Some((
            "invalid_resource_ref".to_string(),
            format!("资源 id `{id}` 未在当前 scene world 资源清单中可见"),
        ));
    }
    None
}
