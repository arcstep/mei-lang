use std::collections::BTreeSet;

use serde_json::Value;

use crate::model::ColumnSchema;

pub(crate) fn infer_columns(rows: &[Value]) -> Vec<String> {
    let mut fields = BTreeSet::new();
    for row in rows {
        if let Some(object) = row.as_object() {
            for key in object.keys() {
                fields.insert(key.clone());
            }
        }
    }
    fields.into_iter().collect()
}

pub(crate) fn infer_schema_from_rows(rows: &[Value]) -> Vec<ColumnSchema> {
    infer_columns(rows)
        .into_iter()
        .map(|name| ColumnSchema {
            name: name.clone(),
            type_name: infer_column_type(rows, &name),
            source: None,
            optional: false,
            unit: None,
        })
        .collect()
}

fn infer_column_type(rows: &[Value], field: &str) -> String {
    for row in rows {
        let Some(value) = row_value(row, field) else {
            continue;
        };
        return match value {
            Value::Bool(_) => "boolean".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::String(raw) => {
                if raw.parse::<f64>().is_ok() {
                    "number".to_string()
                } else {
                    "string".to_string()
                }
            }
            Value::Array(_) => "object".to_string(),
            Value::Object(_) => "object".to_string(),
            Value::Null => "string".to_string(),
        };
    }
    "string".to_string()
}

pub(super) fn row_value<'a>(row: &'a Value, field: &str) -> Option<&'a Value> {
    row.as_object().and_then(|object| object.get(field))
}

pub(super) fn row_string(row: &Value, field: &str) -> String {
    row_value(row, field)
        .map(|value| match value {
            Value::String(raw) => raw.clone(),
            Value::Number(raw) => raw.to_string(),
            Value::Bool(raw) => raw.to_string(),
            _ => value.to_string(),
        })
        .unwrap_or_default()
}

pub(super) fn row_number(row: &Value, field: &str) -> Option<f64> {
    row_value(row, field).and_then(parse_number)
}

pub(super) fn parse_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.parse::<f64>().ok(),
        _ => None,
    }
}
