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
            normalize: None,
            primary: false,
        hidden: false,
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

/// 单元格用于谓词、分组键、字符串比较的展示文本（Excel 浮点整数显示为 `10` 而非 `10.0`）。
pub(super) fn value_display_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                return integer.to_string();
            }
            if let Some(float) = number.as_f64() {
                if float.is_finite() && float.fract().abs() < f64::EPSILON {
                    return (float as i64).to_string();
                }
            }
            number.to_string()
        }
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(super) fn row_string(row: &Value, field: &str) -> String {
    match row_value(row, field) {
        Some(value) => value_display_text(value),
        None => String::new(),
    }
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
