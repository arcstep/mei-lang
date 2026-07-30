//! Static review placeholders for `data_mode=static` (prototype surface).

use mei_lang_kernel::{ColumnSchema, DatasetView, MetricContract};
use serde_json::{json, Map, Value};

pub const STATIC_DATA_ORIGIN: &str = "static_skeleton";
pub const STATIC_METRIC_VALUE: &str = "xxxx";
pub const STATIC_METRIC_LABEL_FALLBACK: &str = "指标名";
pub const STATIC_METRIC_UNIT_FALLBACK: &str = "单位";

pub fn is_static_data_mode(data_mode: Option<&str>) -> bool {
    data_mode
        .map(str::trim)
        .is_some_and(|mode| mode.eq_ignore_ascii_case("static"))
}

pub fn static_metric_placeholder(contract: &MetricContract, metric_id: &str) -> Value {
    let label = contract
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(metric_id);
    let unit = contract
        .unit
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(STATIC_METRIC_UNIT_FALLBACK);
    let mut map = Map::new();
    map.insert("id".to_string(), Value::String(metric_id.to_string()));
    map.insert("label".to_string(), Value::String(label.to_string()));
    map.insert(
        "value".to_string(),
        Value::String(STATIC_METRIC_VALUE.to_string()),
    );
    map.insert("unit".to_string(), Value::String(unit.to_string()));
    if let Some(value_format) = contract.value_format.clone() {
        map.insert("value_format".to_string(), value_format);
    }
    map.insert("shape".to_string(), json!(contract.shape));
    map.insert(
        "__mei_data_origin".to_string(),
        Value::String(STATIC_DATA_ORIGIN.to_string()),
    );
    Value::Object(map)
}

pub fn static_metric_fallback(metric_id: &str) -> Value {
    let mut map = Map::new();
    map.insert("id".to_string(), Value::String(metric_id.to_string()));
    map.insert(
        "label".to_string(),
        Value::String(STATIC_METRIC_LABEL_FALLBACK.to_string()),
    );
    map.insert(
        "value".to_string(),
        Value::String(STATIC_METRIC_VALUE.to_string()),
    );
    map.insert(
        "unit".to_string(),
        Value::String(STATIC_METRIC_UNIT_FALLBACK.to_string()),
    );
    map.insert(
        "__mei_data_origin".to_string(),
        Value::String(STATIC_DATA_ORIGIN.to_string()),
    );
    Value::Object(map)
}

pub fn strip_static_eval_patch(value: &Value) -> Value {
    let Some(map) = value.as_object() else {
        return value.clone();
    };
    let mut out = Map::new();
    for (key, entry) in map {
        if matches!(key.as_str(), "value" | "values" | "total" | "count") {
            continue;
        }
        out.insert(key.clone(), entry.clone());
    }
    Value::Object(out)
}

fn static_cell_value(column_index: usize, row_index: usize) -> String {
    if column_index == 0 {
        format!("值{}", row_index + 1)
    } else {
        format!("列{}-值{}", column_index + 1, row_index + 1)
    }
}

fn column_label(column: &ColumnSchema, column_index: usize) -> String {
    let trimmed = column.name.trim();
    if trimmed.is_empty() {
        format!("列{}", column_index + 1)
    } else {
        trimmed.to_string()
    }
}

pub fn static_dataset_placeholder(dataset: &DatasetView, row_count: usize) -> Value {
    let row_count = row_count.clamp(3, 8);
    let columns: Vec<ColumnSchema> = if dataset.schema.is_empty() {
        (0..3).map(static_column_schema).collect()
    } else {
        dataset.schema.clone()
    };
    let mut rows = Vec::new();
    for row_index in 0..row_count {
        let mut row = Map::new();
        for (column_index, column) in columns.iter().enumerate() {
            let key = column_label(column, column_index);
            row.insert(
                key,
                Value::String(static_cell_value(column_index, row_index)),
            );
        }
        rows.push(Value::Object(row));
    }
    let mut slim = dataset.clone();
    slim.rows = rows;
    for metric in slim.metrics.values_mut() {
        if metric.shape == mei_lang_kernel::MetricShape::Scalar {
            metric.value = Value::String(STATIC_METRIC_VALUE.to_string());
        } else {
            metric.value = Value::Null;
        }
    }
    let mut payload = serde_json::to_value(slim).unwrap_or(Value::Null);
    if let Some(map) = payload.as_object_mut() {
        map.insert(
            "__mei_data_origin".to_string(),
            Value::String(STATIC_DATA_ORIGIN.to_string()),
        );
    }
    payload
}

fn static_column_schema(index: usize) -> ColumnSchema {
    ColumnSchema {
        name: format!("列{}", index + 1),
        type_name: "string".to_string(),
        source: None,
        optional: false,
        unit: None,
        normalize: None,
    }
}

pub fn static_chart_rows(mapping: Option<&Value>, row_count: usize) -> Vec<Map<String, Value>> {
    let row_count = row_count.clamp(4, 6);
    let x_field = mapping
        .and_then(|value| value.get("x"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("field"))
        .and_then(Value::as_str)
        .unwrap_or("category");
    let y_field = mapping
        .and_then(|value| value.get("y"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("field"))
        .and_then(Value::as_str)
        .unwrap_or("value");
    let y_values = [12_i64, 34, 56, 78, 90, 62];
    (0..row_count)
        .map(|index| {
            let mut row = Map::new();
            row.insert(
                x_field.to_string(),
                Value::String(format!("类目{}", index + 1)),
            );
            row.insert(
                y_field.to_string(),
                Value::Number(serde_json::Number::from(
                    y_values.get(index).copied().unwrap_or(12),
                )),
            );
            row
        })
        .collect()
}

pub fn inject_static_chart_data(props: &mut Value) {
    let Some(map) = props.as_object_mut() else {
        return;
    };
    let mapping = map.get("mapping").cloned();
    let rows = static_chart_rows(mapping.as_ref(), 4);
    let data = map.entry("data".to_string()).or_insert_with(|| json!({}));
    if let Some(data_map) = data.as_object_mut() {
        data_map.insert(
            "rows".to_string(),
            Value::Array(rows.into_iter().map(Value::Object).collect()),
        );
        data_map.insert(
            "__mei_data_origin".to_string(),
            Value::String(STATIC_DATA_ORIGIN.to_string()),
        );
    }
    map.insert(
        "__mei_data_origin".to_string(),
        Value::String(STATIC_DATA_ORIGIN.to_string()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::MetricShape;

    #[test]
    fn static_metric_placeholder_uses_contract_label() {
        let contract = MetricContract {
            id: "sales_total".to_string(),
            label: Some("销售总额".to_string()),
            unit: Some("万元".to_string()),
            shape: MetricShape::Scalar,
            value: json!("999"),
            value_format: None,
            purpose: None,
            schema: Vec::new(),
            dataset: None,
            transforms: Vec::new(),
        };
        let value = static_metric_placeholder(&contract, "sales_total");
        assert_eq!(value.get("label").and_then(Value::as_str), Some("销售总额"));
        assert_eq!(value.get("value").and_then(Value::as_str), Some("xxxx"));
        assert_eq!(value.get("unit").and_then(Value::as_str), Some("万元"));
    }
}
