use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::model::DatasetView;

use super::{
    eval_context::{EvalContext, EvalNodeKind},
    predicate::predicate_matches_with_ctx,
    rowset::eval_rowset_with_ctx,
    schema::{parse_number, row_number, row_string},
};

fn series_value_field(object: &serde_json::Map<String, Value>) -> &str {
    object
        .get("value_field")
        .and_then(Value::as_str)
        .unwrap_or("value")
}

fn period_over_period_rate(rows: &[Value], value_field: &str, offset: usize) -> f64 {
    if rows.len() <= offset {
        return 0.0;
    }
    let current = row_number(&rows[rows.len() - 1], value_field).unwrap_or(0.0);
    let base_idx = rows.len().saturating_sub(1 + offset);
    let base = row_number(&rows[base_idx], value_field).unwrap_or(0.0);
    if base.abs() < f64::EPSILON {
        0.0
    } else {
        (current - base) / base.abs() * 100.0
    }
}

pub(crate) fn eval_scalar_value_with_ctx(
    expr: &Value,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Value> {
    if let Some(value) = ctx.cached_scalar(expr) {
        return Ok(value);
    }
    let Some(node_key) = ctx.scalar_key(expr) else {
        return eval_scalar_uncached(expr, base_rows, datasets, ctx);
    };
    ctx.with_eval_node(&node_key, EvalNodeKind::Scalar, |ctx| {
        eval_scalar_uncached(expr, base_rows, datasets, ctx)
            .with_context(|| format!("metric_eval_recursion_guard_tripped(scalar): `{node_key}`"))
    })
}

fn eval_scalar_uncached(
    expr: &Value,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
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
                Some(rowset) => match eval_rowset_with_ctx(rowset, datasets, ctx) {
                    Ok(rows) => rows,
                    Err(error) => {
                        if let Some(fallback) = object.get("fallback").cloned() {
                            ctx.store_scalar(expr, &fallback);
                            return Ok(fallback);
                        }
                        return Err(error)
                            .with_context(|| format!("count rowset for scalar `{expr}`"));
                    }
                },
                None => base_rows.to_vec(),
            };
            let value = json!(rows.len());
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "sum" => {
            let values = eval_numeric_values(object.get("value"), datasets, ctx)?;
            let value = json!(values.iter().sum::<f64>());
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "avg" => {
            let values = eval_numeric_values(object.get("value"), datasets, ctx)?;
            let value = if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            };
            let value = json!(value);
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "min" => {
            let values = eval_numeric_values(object.get("value"), datasets, ctx)?;
            let value = json!(values.into_iter().reduce(f64::min).unwrap_or(0.0));
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "max" => {
            let values = eval_numeric_values(object.get("value"), datasets, ctx)?;
            let value = json!(values.into_iter().reduce(f64::max).unwrap_or(0.0));
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "median" => {
            let mut values = eval_numeric_values(object.get("value"), datasets, ctx)?;
            values.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
            if values.is_empty() {
                let value = json!(0.0);
                ctx.store_scalar(expr, &value);
                return Ok(value);
            }
            let middle = values.len() / 2;
            let median = if values.len() % 2 == 0 {
                (values[middle - 1] + values[middle]) / 2.0
            } else {
                values[middle]
            };
            let value = json!(median);
            ctx.store_scalar(expr, &value);
            Ok(value)
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
                        eval_rowset_with_ctx(source, datasets, ctx)?
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
            let value = json!(unique);
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "item_count" => {
            let Some(value_expr) = object.get("value") else {
                return Ok(json!(0));
            };
            let count = match value_expr {
                Value::Array(items) => items.len(),
                _ => eval_rowset_with_ctx(value_expr, datasets, ctx)
                    .map(|rows| rows.len())
                    .unwrap_or(0),
            };
            let value = json!(count);
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "ratio" => {
            let numerator = eval_scalar_value_with_ctx(
                object.get("numerator").unwrap_or(&Value::Null),
                base_rows,
                datasets,
                ctx,
            )?
            .as_f64()
            .unwrap_or(0.0);
            let denominator = eval_scalar_value_with_ctx(
                object.get("denominator").unwrap_or(&Value::Null),
                base_rows,
                datasets,
                ctx,
            )?
            .as_f64()
            .unwrap_or(0.0);
            if denominator.abs() < f64::EPSILON {
                let value = json!(0.0);
                ctx.store_scalar(expr, &value);
                Ok(value)
            } else {
                let value = json!(numerator / denominator);
                ctx.store_scalar(expr, &value);
                Ok(value)
            }
        }
        "percent" => {
            let rows = match object.get("rowset") {
                Some(rowset) => match eval_rowset_with_ctx(rowset, datasets, ctx) {
                    Ok(rows) => rows,
                    Err(error) => {
                        if datasets.is_empty() {
                            let fallback = object
                                .get("fallback")
                                .cloned()
                                .unwrap_or_else(|| json!(0.0));
                            ctx.store_scalar(expr, &fallback);
                            return Ok(fallback);
                        }
                        return Err(error);
                    }
                },
                None => base_rows.to_vec(),
            };
            let matched = object
                .get("predicate")
                .map(|predicate| {
                    rows.iter()
                        .filter(|row| predicate_matches_with_ctx(row, predicate, datasets, ctx))
                        .count()
                })
                .unwrap_or(rows.len());
            if rows.is_empty() {
                let value = json!(0.0);
                ctx.store_scalar(expr, &value);
                Ok(value)
            } else {
                let value = json!(matched as f64 / rows.len() as f64);
                ctx.store_scalar(expr, &value);
                Ok(value)
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
            let value = json!(total);
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "sum_rowset_counts" => {
            let rowsets = object
                .get("rowsets")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut total = 0usize;
            for rowset in rowsets {
                total += eval_rowset_with_ctx(&rowset, datasets, ctx)?.len();
            }
            let fallback = object.get("fallback").and_then(parse_number).unwrap_or(0.0);
            let value = json!(total as f64 + fallback);
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "number" => {
            let values = eval_numeric_values(Some(expr), datasets, ctx)?;
            let value = Value::Array(values.into_iter().map(|value| json!(value)).collect());
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "lit" => {
            let value = object.get("value").cloned().unwrap_or(Value::Null);
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "mom" => {
            let series_expr = object
                .get("series")
                .or_else(|| object.get("rowset"))
                .ok_or_else(|| anyhow!("mom expression missing series"))?;
            let rows = eval_rowset_with_ctx(series_expr, datasets, ctx)?;
            let value = json!(period_over_period_rate(
                &rows,
                series_value_field(object),
                1,
            ));
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "yoy" => {
            let series_expr = object
                .get("series")
                .or_else(|| object.get("rowset"))
                .ok_or_else(|| anyhow!("yoy expression missing series"))?;
            let rows = eval_rowset_with_ctx(series_expr, datasets, ctx)?;
            let offset = if rows.len() > 12 {
                12
            } else {
                rows.len().saturating_sub(1).max(1)
            };
            let value = json!(period_over_period_rate(
                &rows,
                series_value_field(object),
                offset,
            ));
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        "change_rate" => {
            let current = eval_scalar_value_with_ctx(
                object.get("current").unwrap_or(&Value::Null),
                base_rows,
                datasets,
                ctx,
            )?
            .as_f64()
            .unwrap_or(0.0);
            let base = eval_scalar_value_with_ctx(
                object.get("base").unwrap_or(&Value::Null),
                base_rows,
                datasets,
                ctx,
            )?
            .as_f64()
            .unwrap_or(0.0);
            let mode = object
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("growth");
            let scale = object.get("scale").and_then(parse_number).unwrap_or(100.0);
            let delta = if mode.eq_ignore_ascii_case("reduction") {
                base - current
            } else {
                current - base
            };
            let value = json!(if base.abs() < f64::EPSILON {
                0.0
            } else {
                delta / base.abs() * scale
            });
            ctx.store_scalar(expr, &value);
            Ok(value)
        }
        _ => Ok(expr.clone()),
    }
}

fn eval_numeric_values(
    expr: Option<&Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
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
                return Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?
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
