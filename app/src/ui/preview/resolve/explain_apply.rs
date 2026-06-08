use serde_json::{json, Value};

use super::drilldown::{
    apply_ratio_parts, first_non_empty_string, metric_note_text, MetricDrilldownMeta,
};
use super::explain::string_array_from_value;
use super::explain_normalize::{
    dataset_id_from_source, explain_metric_entries_from_value, normalize_analysis_node_object,
    normalize_analysis_tab_id, normalize_explain_entry_object, normalize_explain_source,
    table_metric_id_from_source,
};

pub(crate) fn apply_explain_items(items: &[Value], meta: &mut MetricDrilldownMeta) {
    for item in items {
        let Some(map) = item.as_object() else {
            continue;
        };
        if map.get("__kind").and_then(Value::as_str) == Some("data_product") {
            if let Some(node) = normalize_analysis_node_object(map) {
                if let Some(node_id) = node
                    .as_object()
                    .and_then(|value| value.get("metric_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                {
                    meta.analysis_objects.insert(node_id, node.clone());
                }
                meta.analysis_nodes.push(node);
            }
            continue;
        }
        let raw_kind = first_non_empty_string(map, &["kind", "type", "id"]).unwrap_or_default();
        let normalized_kind =
            normalize_analysis_tab_id(&raw_kind).unwrap_or_else(|| raw_kind.trim().to_string());
        if normalized_kind == "note" {
            if meta.drilldown_note.is_none() {
                meta.drilldown_note = map.get("note").and_then(metric_note_text).or_else(|| {
                    first_non_empty_string(
                        map,
                        &["content", "text", "markdown", "md", "desc", "description"],
                    )
                });
            } else {
                continue;
            }
            let mut block = map.clone();
            block.insert("id".to_string(), Value::String("note".to_string()));
            block.insert("kind".to_string(), Value::String("note".to_string()));
            meta.analysis_blocks.push(Value::Object(block));
            continue;
        }
        let Some(entry) = normalize_explain_entry_object(map) else {
            continue;
        };
        let entry_obj = entry.as_object().expect("normalized explain entry");
        let tab_id = entry_obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_default();
        let kind = entry_obj
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if !tab_id.is_empty() && kind != "definition" && !meta.drilldown_tabs.contains(&tab_id) {
            meta.drilldown_tabs.push(tab_id);
        }
        if kind == "definition" {
            if meta.drilldown_note.is_none() {
                meta.drilldown_note = first_non_empty_string(
                    entry_obj,
                    &[
                        "note",
                        "content",
                        "text",
                        "markdown",
                        "md",
                        "desc",
                        "description",
                    ],
                );
            }
            if meta.drilldown_basis_refs.is_empty() {
                if let Some(value) = entry_obj
                    .get("basis_refs")
                    .or_else(|| entry_obj.get("basisRefs"))
                {
                    meta.drilldown_basis_refs = string_array_from_value(value);
                }
            }
            if meta.drilldown_recommended_dimensions.is_empty() {
                if let Some(value) = entry_obj
                    .get("recommended_dimensions")
                    .or_else(|| entry_obj.get("recommendedDimensions"))
                {
                    meta.drilldown_recommended_dimensions = string_array_from_value(value);
                }
            }
            continue;
        } else if kind == "detail" {
            if meta.drilldown_detail_fields.is_empty() {
                if let Some(value) = entry_obj.get("fields") {
                    meta.drilldown_detail_fields = string_array_from_value(value);
                }
            }
            if meta.drilldown_headers.is_empty() {
                if let Some(value) = entry_obj.get("headers") {
                    meta.drilldown_headers = string_array_from_value(value);
                }
            }
            if meta.drilldown_table_metric_id.is_none() {
                meta.drilldown_table_metric_id = first_non_empty_string(
                    entry_obj,
                    &[
                        "table_metric_id",
                        "tableMetricId",
                        "metric_id",
                        "metricId",
                        "detail_table_metric_id",
                        "detailTableMetricId",
                    ],
                );
            }
            if meta.explain_detail_dataset.is_none() {
                meta.explain_detail_dataset = first_non_empty_string(
                    entry_obj,
                    &["detail_dataset", "detailDataset", "dataset_id", "datasetId"],
                );
            }
        } else if kind == "composition" {
            if meta.explain_composition_by.is_empty() {
                if let Some(by) = first_non_empty_string(entry_obj, &["by"]) {
                    meta.explain_composition_by = vec![by];
                }
            }
        } else if kind == "trend" {
            if meta.explain_trend_field.is_none() {
                meta.explain_trend_field =
                    first_non_empty_string(entry_obj, &["date_field", "dateField"]);
            }
            if meta.explain_trend_grain.is_none() {
                meta.explain_trend_grain = first_non_empty_string(entry_obj, &["grain"]);
            }
        } else if kind == "numerator_denominator" {
            if meta.drilldown_ratio_numerator.is_none() {
                meta.drilldown_ratio_numerator = first_non_empty_string(entry_obj, &["numerator"]);
            }
            if meta.drilldown_ratio_denominator.is_none() {
                meta.drilldown_ratio_denominator =
                    first_non_empty_string(entry_obj, &["denominator"]);
            }
            if meta.drilldown_ratio_formula.is_none() {
                meta.drilldown_ratio_formula = first_non_empty_string(entry_obj, &["formula"]);
            }
        }
        meta.explain_metrics.push(entry.clone());
        meta.analysis_blocks.push(entry);
    }
    sync_explain_metric_tab_overrides(meta);
}

pub(crate) fn apply_explain_object(
    map: &serde_json::Map<String, Value>,
    meta: &mut MetricDrilldownMeta,
) {
    if meta.drilldown_note.is_none() {
        meta.drilldown_note = first_non_empty_string(map, &["note", "desc", "description"]);
    }
    if meta.drilldown_detail_fields.is_empty() {
        if let Some(value) = map.get("detail_fields").or_else(|| map.get("detailFields")) {
            meta.drilldown_detail_fields = string_array_from_value(value);
        }
    }
    if meta.drilldown_recommended_dimensions.is_empty() {
        if let Some(value) = map
            .get("recommended_dimensions")
            .or_else(|| map.get("recommendedDimensions"))
        {
            meta.drilldown_recommended_dimensions = string_array_from_value(value);
        }
    }
    if meta.drilldown_basis_refs.is_empty() {
        if let Some(value) = map.get("basis_refs").or_else(|| map.get("basisRefs")) {
            meta.drilldown_basis_refs = string_array_from_value(value);
        }
    }
    if meta.drilldown_table_metric_id.is_none() {
        meta.drilldown_table_metric_id = first_non_empty_string(
            map,
            &[
                "detail_table_metric_id",
                "detailTableMetricId",
                "detail_metric_id",
                "detailMetricId",
                "table_metric_id",
                "tableMetricId",
            ],
        );
    }
    let uses_table_metric = meta
        .drilldown_table_metric_id
        .as_deref()
        .is_some_and(|value| !value.is_empty());
    if meta.explain_detail_dataset.is_none() {
        meta.explain_detail_dataset = first_non_empty_string(
            map,
            &["detail_dataset", "detailDataset", "dataset_id", "datasetId"],
        );
    }
    if meta.drilldown_dataset_id.is_none() && !uses_table_metric {
        meta.drilldown_dataset_id = meta.explain_detail_dataset.clone().or_else(|| {
            first_non_empty_string(map, &["dataset_id", "datasetId", "drilldown_dataset_id"])
        });
    }
    if meta.explain_metrics.is_empty() {
        if let Some(value) = map.get("metrics") {
            let metrics = explain_metric_entries_from_value(value);
            if !metrics.is_empty() {
                meta.explain_metrics = metrics.clone();
                if meta.drilldown_tabs.is_empty() {
                    meta.drilldown_tabs = metrics
                        .iter()
                        .filter_map(|entry| {
                            entry
                                .as_object()
                                .and_then(|obj| obj.get("id"))
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|id| !id.is_empty())
                                .map(str::to_string)
                        })
                        .collect();
                }
            }
        }
    }
    sync_explain_metric_tab_overrides(meta);
    if meta.explain_composition_by.is_empty() {
        if let Some(value) = map
            .get("composition_by")
            .or_else(|| map.get("compositionBy"))
        {
            meta.explain_composition_by = string_array_from_value(value);
        }
    }
    if meta.explain_trend_field.is_none() {
        meta.explain_trend_field = first_non_empty_string(
            map,
            &["trend_field", "trendField", "date_field", "dateField"],
        );
    }
    if meta.explain_trend_grain.is_none() {
        meta.explain_trend_grain =
            first_non_empty_string(map, &["trend_grain", "trendGrain", "trend_by", "trendBy"]);
    }
    if meta.drilldown_ratio_numerator.is_none()
        || meta.drilldown_ratio_denominator.is_none()
        || meta.drilldown_ratio_formula.is_none()
    {
        if let Some(value) = map.get("ratio_parts").or_else(|| map.get("ratioParts")) {
            apply_ratio_parts(value, meta);
        }
    }
    if meta.analysis_blocks.is_empty() {
        if let Some(note) = meta
            .drilldown_note
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            meta.analysis_blocks.push(json!({
                "__kind": "explain_item",
                "id": "note",
                "kind": "note",
                "note": note,
                "content": note,
                "format": "text",
            }));
        }
        for entry in &meta.explain_metrics {
            meta.analysis_blocks.push(entry.clone());
        }
    }
}

/// 将 `explain_metric` 上的 `by` / `date_field` 同步到 `drilldown_tab_metrics`，供 popup 派生查询使用。
fn sync_explain_metric_tab_overrides(meta: &mut MetricDrilldownMeta) {
    for entry in &meta.explain_metrics {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        let Some(tab_id) = obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let normalized_source = obj.get("source").and_then(normalize_explain_source);
        let mut override_obj = serde_json::Map::new();
        if kind == "detail" {
            let detail_table_metric_id = first_non_empty_string(
                obj,
                &[
                    "table_metric_id",
                    "tableMetricId",
                    "metric_id",
                    "metricId",
                    "detail_table_metric_id",
                    "detailTableMetricId",
                    "detail_metric_id",
                    "detailMetricId",
                ],
            )
            .or_else(|| {
                meta.drilldown_table_metric_id
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            });
            let detail_table_metric_id = detail_table_metric_id.or_else(|| {
                normalized_source
                    .as_ref()
                    .and_then(table_metric_id_from_source)
            });
            if let Some(table_metric_id) = detail_table_metric_id {
                override_obj.insert(
                    "table_metric_id".to_string(),
                    Value::String(table_metric_id),
                );
            }
            let detail_dataset_id = first_non_empty_string(
                obj,
                &["detail_dataset", "detailDataset", "dataset_id", "datasetId"],
            )
            .or_else(|| {
                meta.drilldown_dataset_id
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            });
            let detail_dataset_id = detail_dataset_id
                .or_else(|| normalized_source.as_ref().and_then(dataset_id_from_source));
            if let Some(dataset_id) = detail_dataset_id {
                override_obj.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        } else if kind == "composition" {
            if let Some(by) = obj.get("by").and_then(Value::as_str).map(str::trim) {
                if !by.is_empty() {
                    override_obj.insert(
                        "composition_by".to_string(),
                        Value::Array(vec![Value::String(by.to_string())]),
                    );
                }
            }
            if let Some(table_metric_id) = first_non_empty_string(
                obj,
                &["table_metric_id", "tableMetricId", "metric_id", "metricId"],
            )
            .or_else(|| {
                normalized_source
                    .as_ref()
                    .and_then(table_metric_id_from_source)
            }) {
                override_obj.insert(
                    "table_metric_id".to_string(),
                    Value::String(table_metric_id),
                );
            }
            if let Some(dataset_id) = first_non_empty_string(obj, &["dataset_id", "datasetId"])
                .or_else(|| normalized_source.as_ref().and_then(dataset_id_from_source))
            {
                override_obj.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        } else if kind == "trend" {
            if let Some(field) = first_non_empty_string(obj, &["date_field", "dateField"]) {
                override_obj.insert("trend_field".to_string(), Value::String(field));
            }
            if let Some(grain) = first_non_empty_string(obj, &["grain"]) {
                override_obj.insert("trend_grain".to_string(), Value::String(grain));
            }
            if let Some(table_metric_id) = first_non_empty_string(
                obj,
                &["table_metric_id", "tableMetricId", "metric_id", "metricId"],
            )
            .or_else(|| {
                normalized_source
                    .as_ref()
                    .and_then(table_metric_id_from_source)
            }) {
                override_obj.insert(
                    "table_metric_id".to_string(),
                    Value::String(table_metric_id),
                );
            }
            if let Some(dataset_id) = first_non_empty_string(obj, &["dataset_id", "datasetId"])
                .or_else(|| normalized_source.as_ref().and_then(dataset_id_from_source))
            {
                override_obj.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        }
        if let Some(source) = normalized_source {
            override_obj.insert("source".to_string(), source);
        }
        if override_obj.is_empty() {
            continue;
        }
        match meta.drilldown_tab_metrics.get_mut(tab_id) {
            Some(Value::Object(existing)) => {
                for (key, value) in override_obj {
                    existing.insert(key, value);
                }
            }
            _ => {
                meta.drilldown_tab_metrics
                    .insert(tab_id.to_string(), Value::Object(override_obj));
            }
        }
    }
}

pub(crate) fn apply_analyses_value(value: &Value, meta: &mut MetricDrilldownMeta) {
    let Some(items) = value.as_array() else {
        return;
    };
    for item in items {
        let Some(entry) = item.as_object() else {
            continue;
        };
        let kind = first_non_empty_string(entry, &["kind", "type", "id"])
            .and_then(|raw| normalize_analysis_tab_id(raw.as_str()));
        let Some(tab_id) = kind else {
            continue;
        };
        if !meta.drilldown_tabs.contains(&tab_id) {
            meta.drilldown_tabs.push(tab_id.clone());
        }
        let override_obj = build_analysis_override_object(entry);
        if !override_obj.is_empty() {
            meta.drilldown_tab_metrics
                .insert(tab_id, Value::Object(override_obj));
        }
    }
}

fn build_analysis_override_object(
    entry: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    let mut obj = serde_json::Map::new();
    if let Some(table_metric_id) = first_non_empty_string(
        entry,
        &["table_metric_id", "tableMetricId", "metric_id", "metricId"],
    ) {
        obj.insert(
            "table_metric_id".to_string(),
            Value::String(table_metric_id),
        );
    }
    if let Some(dataset_id) = first_non_empty_string(entry, &["dataset_id", "datasetId"]) {
        obj.insert("dataset_id".to_string(), Value::String(dataset_id));
    }
    if let Some(columns) = entry
        .get("columns")
        .or_else(|| entry.get("detail_fields"))
        .or_else(|| entry.get("detailFields"))
    {
        let values = string_array_from_value(columns);
        if !values.is_empty() {
            obj.insert(
                "columns".to_string(),
                Value::Array(values.into_iter().map(Value::String).collect()),
            );
        }
    }
    if let Some(headers) = entry.get("headers") {
        let values = string_array_from_value(headers);
        if !values.is_empty() {
            obj.insert(
                "headers".to_string(),
                Value::Array(values.into_iter().map(Value::String).collect()),
            );
        }
    }
    if let Some(mapping) = entry.get("mapping").and_then(Value::as_object) {
        obj.insert("mapping".to_string(), Value::Object(mapping.clone()));
    }
    if let Some(chart_kind) = first_non_empty_string(entry, &["chart_kind", "chartKind", "chart"]) {
        obj.insert("chart_kind".to_string(), Value::String(chart_kind));
    }
    obj
}
