use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::model::DatasetView;

use super::{
    predicate::predicate_matches,
    rowset::eval_rowset,
    schema::{parse_number, row_number, row_string},
};

pub(crate) fn eval_scalar_value(
    expr: &Value,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<Value> {
    let Some(object) = expr.as_object() else {
        return Ok(expr.clone());
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(expr.clone());
    }
    let analysis_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("analysis expression missing type"))?;
    match analysis_type {
        "count" => {
            let rows = match object.get("rowset") {
                Some(rowset) => eval_rowset(rowset, datasets)?,
                None => base_rows.to_vec(),
            };
            Ok(json!(rows.len()))
        }
        "sum" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            Ok(json!(values.iter().sum::<f64>()))
        }
        "avg" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            let value = if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            };
            Ok(json!(value))
        }
        "min" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            Ok(json!(values.into_iter().reduce(f64::min).unwrap_or(0.0)))
        }
        "max" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            Ok(json!(values.into_iter().reduce(f64::max).unwrap_or(0.0)))
        }
        "median" => {
            let mut values = eval_numeric_values(object.get("value"), datasets)?;
            values.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
            if values.is_empty() {
                return Ok(json!(0.0));
            }
            let middle = values.len() / 2;
            let median = if values.len() % 2 == 0 {
                (values[middle - 1] + values[middle]) / 2.0
            } else {
                values[middle]
            };
            Ok(json!(median))
        }
        "unique_count" => {
            let Some(value_expr) = object.get("value") else {
                return Ok(json!(0));
            };
            let unique = match value_expr {
                Value::Array(items) => items
                    .iter()
                    .map(Value::to_string)
                    .collect::<BTreeSet<_>>()
                    .len(),
                Value::Object(map) => {
                    if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                        && map.get("type").and_then(Value::as_str) == Some("text")
                    {
                        let source = map
                            .get("source")
                            .or_else(|| map.get("rowset"))
                            .unwrap_or(&Value::Null);
                        let field = map.get("field").and_then(Value::as_str).unwrap_or("");
                        eval_rowset(source, datasets)?
                            .iter()
                            .map(|row| row_string(row, field))
                            .collect::<BTreeSet<_>>()
                            .len()
                    } else {
                        0
                    }
                }
                _ => 0,
            };
            Ok(json!(unique))
        }
        "item_count" => {
            let Some(value_expr) = object.get("value") else {
                return Ok(json!(0));
            };
            let count = match value_expr {
                Value::Array(items) => items.len(),
                _ => eval_rowset(value_expr, datasets)
                    .map(|rows| rows.len())
                    .unwrap_or(0),
            };
            Ok(json!(count))
        }
        "ratio" => {
            let numerator = eval_scalar_value(
                object.get("numerator").unwrap_or(&Value::Null),
                base_rows,
                datasets,
            )?
            .as_f64()
            .unwrap_or(0.0);
            let denominator = eval_scalar_value(
                object.get("denominator").unwrap_or(&Value::Null),
                base_rows,
                datasets,
            )?
            .as_f64()
            .unwrap_or(0.0);
            if denominator.abs() < f64::EPSILON {
                Ok(json!(0.0))
            } else {
                Ok(json!(numerator / denominator))
            }
        }
        "percent" => {
            let rows = object
                .get("rowset")
                .map(|rowset| eval_rowset(rowset, datasets))
                .transpose()?
                .unwrap_or_else(|| base_rows.to_vec());
            let matched = object
                .get("predicate")
                .map(|predicate| {
                    rows.iter()
                        .filter(|row| predicate_matches(row, predicate, datasets))
                        .count()
                })
                .unwrap_or(rows.len());
            if rows.is_empty() {
                Ok(json!(0.0))
            } else {
                Ok(json!(matched as f64 / rows.len() as f64))
            }
        }
        "sum_first_number" => {
            let rows = base_rows;
            let fields = object
                .get("fields")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut total = 0.0;
            for row in rows {
                for field in &fields {
                    if let Some(name) = field.as_str() {
                        if let Some(number) = row_number(row, name) {
                            total += number;
                            break;
                        }
                    }
                }
            }
            Ok(json!(total))
        }
        "sum_rowset_counts" => {
            let rowsets = object
                .get("rowsets")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut total = 0usize;
            for rowset in rowsets {
                total += eval_rowset(&rowset, datasets)?.len();
            }
            let fallback = object
                .get("fallback")
                .and_then(parse_number)
                .unwrap_or(0.0);
            Ok(json!(total as f64 + fallback))
        }
        "number" => {
            let values = eval_numeric_values(Some(expr), datasets)?;
            Ok(Value::Array(
                values.into_iter().map(|value| json!(value)).collect(),
            ))
        }
        "lit" => Ok(object.get("value").cloned().unwrap_or(Value::Null)),
        _ => Ok(expr.clone()),
    }
}

fn eval_numeric_values(
    expr: Option<&Value>,
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<Vec<f64>> {
    let Some(expr) = expr else {
        return Ok(Vec::new());
    };
    if let Some(number) = parse_number(expr) {
        return Ok(vec![number]);
    }
    match expr {
        Value::Array(items) => Ok(items.iter().filter_map(parse_number).collect()),
        Value::Object(map) => {
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("number")
            {
                let rowset_expr = map
                    .get("rowset")
                    .or_else(|| map.get("source"))
                    .ok_or_else(|| anyhow!("number expression missing rowset"))?;
                let field = map
                    .get("field")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("number expression missing field"))?;
                return Ok(eval_rowset(rowset_expr, datasets)?
                    .iter()
                    .filter_map(|row| row_number(row, field))
                    .collect());
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("lit")
            {
                return Ok(map
                    .get("value")
                    .and_then(parse_number)
                    .into_iter()
                    .collect::<Vec<_>>());
            }
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}
