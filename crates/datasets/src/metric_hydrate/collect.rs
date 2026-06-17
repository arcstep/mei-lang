use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

pub(crate) fn collect_dataset_ids_from_values(values: &[Value]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for value in values {
        collect_dataset_ids_from_value(value, &mut ids);
    }
    ids
}

pub(crate) fn collect_dataset_ids_from_value(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_dataset_ids_from_value(item, out);
            }
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(id) = map
                    .get("from_dataset")
                    .or_else(|| map.get("id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    out.insert(id.to_string());
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                if map.get("type").and_then(Value::as_str) == Some("rows") {
                    if let Some(id) = map.get("dataset").and_then(Value::as_str) {
                        let text = id.trim();
                        if !text.is_empty() {
                            out.insert(text.strip_prefix("dataset.").unwrap_or(text).to_string());
                        }
                    }
                }
            }
            for nested in map.values() {
                collect_dataset_ids_from_value(nested, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_metric_ref_ids_from_value(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_metric_ref_ids_from_value(item, out);
            }
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                if let Some(id) = map
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    out.insert(id.to_string());
                }
            }
            for nested in map.values() {
                collect_metric_ref_ids_from_value(nested, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_metric_ref_ids_from_values(values: &[Value]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for value in values {
        collect_metric_ref_ids_from_value(value, &mut ids);
    }
    ids
}

/// 将 hydrate 种子 metric 沿 `metric_ref` 边扩展，确保 composition 等引用
/// `__scalar_rowset__` 的请求也会物化上游 xlsx/csv 数据集。
pub(crate) fn expand_metric_defs_for_hydrate(
    all_defs: &BTreeMap<String, Value>,
    seed_metric_ids: &[String],
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let mut queue = VecDeque::from(seed_metric_ids.to_vec());
    let mut visited = BTreeSet::<String>::new();
    while let Some(metric_id) = queue.pop_front() {
        if !visited.insert(metric_id.clone()) {
            continue;
        }
        let Some(def) = all_defs.get(&metric_id) else {
            continue;
        };
        out.insert(metric_id.clone(), def.clone());
        for ref_id in collect_metric_ref_ids_from_values(&[def.clone()]) {
            if !visited.contains(&ref_id) {
                queue.push_back(ref_id);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn expand_metric_defs_for_hydrate_follows_metric_ref_to_scalar_rowset() {
        let scalar_id = "sales_total::__scalar_rowset__".to_string();
        let composition_id = "sales_total::composition_by_agency".to_string();
        let all_defs = BTreeMap::from([
            (
                scalar_id.clone(),
                json!({
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "rows",
                        "dataset": "sales"
                    }
                }),
            ),
            (
                composition_id.clone(),
                json!({
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "group_by",
                        "rowset": {"__ref": "metric", "id": scalar_id},
                        "by": "agency"
                    }
                }),
            ),
        ]);
        let expanded =
            expand_metric_defs_for_hydrate(&all_defs, std::slice::from_ref(&composition_id));
        assert!(expanded.contains_key(&composition_id));
        assert!(
            expanded.contains_key(&scalar_id),
            "hydrate defs should include metric_ref target: {:?}",
            expanded.keys().collect::<Vec<_>>()
        );
    }
}
