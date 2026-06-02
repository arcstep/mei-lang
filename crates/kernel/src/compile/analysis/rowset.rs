use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::model::DatasetView;

use super::{
    dates::{filter_rows_in_latest_days, filter_rows_in_latest_months},
    eval_context::{EvalContext, EvalNodeKind},
    predicate::predicate_matches_with_ctx,
    schema::{row_string, row_value},
    transforms::{
        aggregate_group_rows, bucket_rows_by_month, distinct_rows_by_fields, first_rows_by_field,
        mutate_row, rename_fields, reorder_fields, select_fields, sort_rows_by_field,
        summarize_rows, trend_rows_by_month, trend_year_compare_rows,
    },
};

fn eval_universe_labels(
    expr: &Value,
    datasets: &BTreeMap<String, DatasetView>,
    fallback_field: &str,
    ctx: &mut EvalContext,
) -> Result<Vec<String>> {
    if let Some(items) = expr.as_array() {
        return Ok(items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(str::trim)
                    .filter(|label| !label.is_empty())
                    .map(ToString::to_string)
            })
            .collect());
    }
    let Some(map) = expr.as_object() else {
        return Ok(Vec::new());
    };
    if map.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(Vec::new());
    }
    let field = map
        .get("field")
        .and_then(Value::as_str)
        .unwrap_or(fallback_field);
    let rows = match map.get("type").and_then(Value::as_str).unwrap_or("") {
        "text" => {
            let source = map
                .get("source")
                .or_else(|| map.get("rowset"))
                .ok_or_else(|| anyhow!("text expression missing source"))?;
            eval_rowset_with_ctx(source, datasets, ctx)?
        }
        _ => eval_rowset_with_ctx(expr, datasets, ctx)?,
    };
    Ok(rows
        .into_iter()
        .map(|row| row_string(&row, field))
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
        .collect())
}

fn apply_universe(
    rows: Vec<Value>,
    universe_expr: &Value,
    group_field: &str,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let labels = eval_universe_labels(universe_expr, datasets, group_field, ctx)?;
    if labels.is_empty() {
        return Ok(rows);
    }
    let mut indexed = BTreeMap::<String, Value>::new();
    for row in rows {
        let key = row_string(&row, group_field);
        if !key.is_empty() {
            indexed.insert(key, row);
        }
    }
    Ok(labels
        .into_iter()
        .map(|label| {
            indexed.remove(&label).unwrap_or_else(|| {
                let mut object = serde_json::Map::new();
                object.insert(group_field.to_string(), Value::String(label.clone()));
                object.insert("value".to_string(), json!(0.0));
                Value::Object(object)
            })
        })
        .collect())
}

pub(crate) fn eval_rowset(
    expr: &Value,
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<Vec<Value>> {
    let mut ctx = EvalContext::default();
    eval_rowset_with_ctx(expr, datasets, &mut ctx)
}

pub(crate) fn eval_rowset_with_ctx(
    expr: &Value,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    if let Some(rows) = ctx.cached_rowset(expr) {
        return Ok(rows);
    }
    if let Some(node_key) = ctx.rowset_key(expr) {
        let rows = ctx.with_eval_node(&node_key, EvalNodeKind::Rowset, |ctx| {
            eval_rowset_uncached(expr, datasets, ctx).with_context(|| {
                format!("metric_eval_recursion_guard_tripped(rowset): `{node_key}`")
            })
        })?;
        ctx.store_rowset(expr, &rows);
        return Ok(rows);
    }
    let rows = eval_rowset_uncached(expr, datasets, ctx)?;
    ctx.store_rowset(expr, &rows);
    Ok(rows)
}

fn eval_rowset_uncached(
    expr: &Value,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    match expr {
        Value::Array(items) => Ok(items.clone()),
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                return resolve_data_ref(map, datasets);
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                return eval_analysis_rowset(map, datasets, ctx);
            }
            Err(anyhow!(
                "rowset expression must be data_ref or analysis expression"
            ))
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(anyhow!("rowset expression must be array or object")),
    }
}

fn resolve_data_ref(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<Vec<Value>> {
    let dataset_id = map
        .get("from_dataset")
        .and_then(Value::as_str)
        .or_else(|| map.get("id").and_then(Value::as_str))
        .ok_or_else(|| anyhow!("data_ref missing id"))?;
    let dataset = lookup_dataset_view(datasets, dataset_id)
        .ok_or_else(|| unknown_dataset_error(dataset_id, datasets))?;
    Ok(dataset.rows.clone())
}

fn lookup_dataset_view<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    datasets
        .get(normalized)
        .or_else(|| datasets.get(dataset_id))
        .or_else(|| {
            datasets.iter().find_map(|(key, dataset)| {
                (dataset.id == normalized
                    || key.ends_with(&format!("::{normalized}"))
                    || key.ends_with(&format!("/{normalized}")))
                .then_some(dataset)
            })
        })
}

fn unknown_dataset_error(
    dataset_id: &str,
    datasets: &BTreeMap<String, DatasetView>,
) -> anyhow::Error {
    let available = datasets.keys().take(8).cloned().collect::<Vec<_>>();
    anyhow!(
        "unknown dataset `{dataset_id}`; available keys: {:?}",
        available
    )
}

fn eval_analysis_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let analysis_type = map
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("analysis expression missing type"))?;
    match analysis_type {
        "rows" => {
            let dataset_id = map
                .get("dataset")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("rows expression missing dataset"))?;
            let dataset = lookup_dataset_view(datasets, dataset_id)
                .ok_or_else(|| unknown_dataset_error(dataset_id, datasets))?;
            Ok(dataset.rows.clone())
        }
        "where" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("where expression missing rowset"))?;
            let predicate = map.get("predicate").unwrap_or(&Value::Null);
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?
                .into_iter()
                .filter(|row| predicate_matches_with_ctx(row, predicate, datasets, ctx))
                .collect())
        }
        "select" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("select expression missing rowset"))?;
            let fields = map
                .get("fields")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("select expression missing fields"))?
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?
                .into_iter()
                .map(|row| select_fields(&row, &fields))
                .collect())
        }
        "rename" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("rename expression missing rowset"))?;
            let mapping = map
                .get("mapping")
                .and_then(Value::as_object)
                .ok_or_else(|| anyhow!("rename expression missing mapping"))?;
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?
                .into_iter()
                .map(|row| rename_fields(&row, mapping))
                .collect())
        }
        "mutate" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("mutate expression missing rowset"))?;
            let updates = map
                .get("updates")
                .and_then(Value::as_object)
                .ok_or_else(|| anyhow!("mutate expression missing updates"))?;
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?
                .into_iter()
                .map(|row| mutate_row(&row, updates))
                .collect())
        }
        "sort_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("sort_by expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("sort_by expression missing field"))?;
            let order = map.get("order").and_then(Value::as_str).unwrap_or("asc");
            let mut rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            sort_rows_by_field(&mut rows, field, order);
            Ok(rows)
        }
        "reorder" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("reorder expression missing rowset"))?;
            let fields = map
                .get("fields")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("reorder expression missing fields"))?
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?
                .into_iter()
                .map(|row| reorder_fields(&row, &fields))
                .collect())
        }
        "stage" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("stage expression missing rowset"))?;
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?)
        }
        "first_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("first_by expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("first_by expression missing field"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            Ok(first_rows_by_field(&rows, field))
        }
        "distinct_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("distinct_by expression missing rowset"))?;
            let fields = map
                .get("fields")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("distinct_by expression missing fields"))?
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            Ok(distinct_rows_by_fields(&rows, &fields))
        }
        "group_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("group_by expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let group_field = map
                .get("by")
                .and_then(Value::as_str)
                .or_else(|| map.get("fields").and_then(Value::as_str))
                .or_else(|| {
                    map.get("fields")
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(Value::as_str)
                })
                .or_else(|| map.get("field").and_then(Value::as_str))
                .ok_or_else(|| anyhow!("group_by expression missing by"))?;
            let value_field = map.get("value").and_then(Value::as_str);
            let agg = map.get("agg").and_then(Value::as_str).unwrap_or("count");
            let mut grouped = aggregate_group_rows(
                &rows,
                group_field,
                value_field,
                agg,
                map.get("limit").and_then(Value::as_u64).map(|n| n as usize),
            );
            if let Some(universe) = map.get("universe") {
                grouped = apply_universe(grouped, universe, group_field, datasets, ctx)?;
            }
            Ok(grouped)
        }
        "agg" => {
            let rowset_expr = map
                .get("rowset")
                .or_else(|| map.get("grouped"))
                .ok_or_else(|| anyhow!("agg expression missing rowset"))?;
            let mut rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let agg = map.get("agg").and_then(Value::as_str).unwrap_or("identity");
            if agg != "identity" {
                let value_field = map.get("value").and_then(Value::as_str).unwrap_or("value");
                rows = summarize_rows(&rows, agg, value_field);
            }
            let sort_field = map
                .get("sort")
                .and_then(Value::as_str)
                .or_else(|| map.get("sort_by").and_then(Value::as_str));
            if let Some(field) = sort_field {
                let order = map.get("order").and_then(Value::as_str).unwrap_or("desc");
                sort_rows_by_field(&mut rows, field, order);
            }
            if let Some(limit) = map.get("limit").and_then(Value::as_u64) {
                rows.truncate(limit as usize);
            }
            Ok(rows)
        }
        "trend" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("trend expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let date_field = map
                .get("date_field")
                .and_then(Value::as_str)
                .or_else(|| map.get("field").and_then(Value::as_str))
                .ok_or_else(|| anyhow!("trend expression missing date_field"))?;
            let by = map.get("by").and_then(Value::as_str).unwrap_or("month");
            let value_field = map.get("value").and_then(Value::as_str);
            let agg = map.get("agg").and_then(Value::as_str).unwrap_or("count");
            let months = map
                .get("limit")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(6);
            let label_field = map
                .get("label_field")
                .and_then(Value::as_str)
                .unwrap_or("month");
            if by == "month" {
                let mut out =
                    trend_rows_by_month(&rows, date_field, value_field, agg, months, label_field);
                let order = map.get("order").and_then(Value::as_str).unwrap_or("asc");
                if order.eq_ignore_ascii_case("desc") {
                    out.reverse();
                }
                return Ok(out);
            }
            Ok(aggregate_group_rows(
                &rows,
                date_field,
                value_field,
                agg,
                Some(months),
            ))
        }
        "trend_year_compare" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("trend_year_compare expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let date_field = map
                .get("date_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("trend_year_compare expression missing date_field"))?;
            let value_field = map.get("value").and_then(Value::as_str);
            let agg = map.get("agg").and_then(Value::as_str).unwrap_or("count");
            let months = map
                .get("limit")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(6);
            let month_label_field = map
                .get("month_label_field")
                .and_then(Value::as_str)
                .unwrap_or("month");
            let year_label_field = map
                .get("year_label_field")
                .and_then(Value::as_str)
                .unwrap_or("year");
            let years = map
                .get("years")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.as_i64()
                                .map(|value| value as i32)
                                .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|items| !items.is_empty())
                .unwrap_or_else(|| vec![2024, 2025]);
            Ok(trend_year_compare_rows(
                &rows,
                date_field,
                value_field,
                agg,
                months,
                &years,
                month_label_field,
                year_label_field,
            ))
        }
        "table_rows" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("table_rows expression missing rowset"))?;
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?)
        }
        "split_text" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("split_text expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("split_text expression missing field"))?;
            let delimiter = map.get("delimiter").and_then(Value::as_str).unwrap_or("、");
            let mut out = Vec::new();
            for row in eval_rowset_with_ctx(rowset_expr, datasets, ctx)? {
                let mut base = row.as_object().cloned().unwrap_or_default();
                let text = base
                    .get(field)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let values = text
                    .split(delimiter)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    out.push(Value::Object(base));
                    continue;
                }
                for item in values {
                    base.insert(field.to_string(), Value::String(item.to_string()));
                    out.push(Value::Object(base.clone()));
                }
            }
            Ok(out)
        }
        "lookup_value" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("lookup_value expression missing rowset"))?;
            let lookup_rowset_expr = map
                .get("lookup_rowset")
                .ok_or_else(|| anyhow!("lookup_value expression missing lookup_rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("lookup_value expression missing field"))?;
            let lookup_field = map
                .get("lookup_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("lookup_value expression missing lookup_field"))?;
            let value_field = map
                .get("value_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("lookup_value expression missing value_field"))?;
            let as_field = map
                .get("as_field")
                .and_then(Value::as_str)
                .unwrap_or(value_field)
                .to_string();
            let mut lookup = BTreeMap::new();
            for row in eval_rowset_with_ctx(lookup_rowset_expr, datasets, ctx)? {
                let key = row_string(&row, lookup_field);
                let value = row_value(&row, value_field).cloned().unwrap_or(Value::Null);
                lookup.insert(key, value);
            }
            let mut out = Vec::new();
            for row in eval_rowset_with_ctx(rowset_expr, datasets, ctx)? {
                let mut object = row.as_object().cloned().unwrap_or_default();
                let key = row_string(&row, field);
                object.insert(
                    as_field.clone(),
                    lookup.get(&key).cloned().unwrap_or(Value::Null),
                );
                out.push(Value::Object(object));
            }
            Ok(out)
        }
        "latest_days" | "latest_months" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("{analysis_type} expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("{analysis_type} expression missing field"))?;
            if analysis_type == "latest_days" {
                let days = map.get("days").and_then(Value::as_u64).unwrap_or(7) as usize;
                return Ok(filter_rows_in_latest_days(&rows, field, days));
            }
            let months = map.get("months").and_then(Value::as_u64).unwrap_or(6) as usize;
            Ok(filter_rows_in_latest_months(&rows, field, months))
        }
        "bucket_date" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("bucket_date expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("bucket_date expression missing field"))?;
            let label_field = map
                .get("label_field")
                .and_then(Value::as_str)
                .or_else(|| map.get("by").and_then(Value::as_str))
                .unwrap_or("month");
            Ok(bucket_rows_by_month(&rows, field, label_field))
        }
        "limit" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("limit expression missing rowset"))?;
            let mut rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let limit = map.get("n").and_then(Value::as_u64).unwrap_or(0);
            rows.truncate(limit as usize);
            Ok(rows)
        }
        other => Err(anyhow!("unsupported rowset analysis `{other}`")),
    }
}
