use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;

use super::super::build::{
    eval_lookup_value_rowset, eval_rowset_with_ctx, eval_split_text_rowset, lookup_dataset_view,
    unknown_dataset_error,
};
use crate::compile::analysis::{
    dates::{filter_rows_in_latest_days, filter_rows_in_latest_months},
    eval_context::EvalContext,
    predicate::predicate_matches_with_ctx,
    transforms::{
        bucket_rows_by_month, distinct_rows_by_fields, first_rows_by_field, mutate_row,
        rename_fields, reorder_fields, select_fields, sort_rows_by_field,
    },
};

pub(super) fn eval_rowset_rows(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    _ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let dataset_id = map
        .get("dataset")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("rows expression missing dataset"))?;
    let dataset = lookup_dataset_view(datasets, dataset_id)
        .ok_or_else(|| unknown_dataset_error(dataset_id, datasets))?;
    Ok(dataset.rows.clone())
}

pub(super) fn eval_rowset_where(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("where expression missing rowset"))?;
    let predicate = map.get("predicate").unwrap_or(&Value::Null);
    let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
    if let (Some(field), Some(expected)) = (
        predicate.get("field").and_then(Value::as_str),
        predicate
            .get("equals")
            .and_then(Value::as_str)
            .or_else(|| predicate.get("eq").and_then(Value::as_str)),
    ) {
        if let Some(filtered) =
            crate::compile::rowset_engine::try_where_eq_columnar(&rows, field, expected)
        {
            return Ok(filtered);
        }
    }
    Ok(rows
        .into_iter()
        .filter(|row| predicate_matches_with_ctx(row, predicate, datasets, ctx))
        .collect())
}

pub(super) fn eval_rowset_select(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_rename(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_mutate(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_sort_by(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_reorder(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_stage(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("stage expression missing rowset"))?;
    Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?)
}

pub(super) fn eval_rowset_first_by(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_distinct_by(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_table_rows(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("table_rows expression missing rowset"))?;
    Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?)
}

pub(super) fn eval_rowset_latest_window(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let analysis_type = map
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("latest_days");
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

pub(super) fn eval_rowset_bucket_date(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_limit(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("limit expression missing rowset"))?;
    let mut rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
    let limit = map.get("n").and_then(Value::as_u64).unwrap_or(0);
    rows.truncate(limit as usize);
    Ok(rows)
}

pub(super) fn eval_rowset_concat_rowsets(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowsets = map
        .get("rowsets")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("concat_rowsets expression missing rowsets"))?;
    let mut out = Vec::new();
    for rowset_expr in rowsets {
        out.extend(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?);
    }
    Ok(out)
}

pub(super) fn eval_rowset_split_text(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    eval_split_text_rowset(map, datasets, ctx)
}

pub(super) fn eval_rowset_lookup_value(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    eval_lookup_value_rowset(map, datasets, ctx)
}
