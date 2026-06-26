use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;

use crate::compile::analysis::{
    eval_context::EvalContext,
    transforms::{
        aggregate_group_rows, aggregate_group_rows_pivot, party_year_aggregate_rows, sort_rows_by_field,
        summarize_rows,
    },
};
use super::super::build::{
    apply_universe, eval_rowset_with_ctx,
    eval_universe_labels,
};

pub(super) fn eval_rowset_group_by(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
let rowset_expr = map
    .get("rowset")
    .ok_or_else(|| anyhow!("group_by expression missing rowset"))?;
let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
let group_fields = map
    .get("fields")
    .and_then(Value::as_array)
    .map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    })
    .filter(|items| !items.is_empty())
    .or_else(|| {
        map.get("by")
            .and_then(Value::as_str)
            .map(|field| vec![field.to_string()])
    })
    .or_else(|| {
        map.get("fields")
            .and_then(Value::as_str)
            .map(|field| vec![field.to_string()])
    })
    .or_else(|| {
        map.get("field")
            .and_then(Value::as_str)
            .map(|field| vec![field.to_string()])
    })
    .ok_or_else(|| anyhow!("group_by expression missing by or fields"))?;
let group_field = group_fields
    .first()
    .map(String::as_str)
    .ok_or_else(|| anyhow!("group_by expression missing by"))?;
let pivot_field = map.get("pivot_field").and_then(Value::as_str);
let pivot_columns = map
    .get("pivot_columns")
    .and_then(Value::as_array)
    .map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    })
    .filter(|items| !items.is_empty());
if let (Some(pivot_field), Some(pivot_columns)) = (pivot_field, pivot_columns) {
    let universe = map
        .get("universe")
        .map(|expr| {
            eval_universe_labels(expr, datasets, group_field, ctx).unwrap_or_default()
        })
        .filter(|labels| !labels.is_empty());
    return Ok(aggregate_group_rows_pivot(
        &rows,
        &group_fields,
        pivot_field,
        &pivot_columns,
        universe.as_deref(),
    ));
}
let value_field = map.get("value").and_then(Value::as_str);
let agg = map.get("agg").and_then(Value::as_str).unwrap_or("count");
let limit = map.get("limit").and_then(Value::as_u64).map(|n| n as usize);
let mut grouped = if value_field.is_none() && agg == "count" && group_fields.len() == 1
{
    crate::compile::rowset_engine::try_group_by_count_columnar(
        &rows,
        group_field,
        limit,
    )
    .or_else(|| {
        crate::compile::rowset_engine::try_polars_group_by(&rows, group_field, agg)
    })
    .unwrap_or_else(|| {
        aggregate_group_rows(&rows, group_field, value_field, agg, limit)
    })
} else {
    aggregate_group_rows(&rows, group_field, value_field, agg, limit)
};
if let Some(universe) = map.get("universe") {
    grouped = apply_universe(grouped, universe, group_field, datasets, ctx)?;
}
Ok(grouped)
}

pub(super) fn eval_rowset_agg(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_party_year_aggregate(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
let rowset_expr = map
    .get("rowset")
    .ok_or_else(|| anyhow!("party_year_aggregate expression missing rowset"))?;
let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
let party_field = map
    .get("party_field")
    .and_then(Value::as_str)
    .ok_or_else(|| anyhow!("party_year_aggregate expression missing party_field"))?;
let date_field = map
    .get("date_field")
    .and_then(Value::as_str)
    .ok_or_else(|| anyhow!("party_year_aggregate expression missing date_field"))?;
let value_field = map
    .get("value_field")
    .and_then(Value::as_str)
    .ok_or_else(|| anyhow!("party_year_aggregate expression missing value_field"))?;
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
    .ok_or_else(|| anyhow!("party_year_aggregate expression missing years"))?;
Ok(party_year_aggregate_rows(
    &rows,
    party_field,
    date_field,
    value_field,
    &years,
))
}

