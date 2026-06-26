use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;

use crate::compile::analysis::{
    eval_context::EvalContext,
    transforms::{
        aggregate_group_rows, trend_rows_by_month, trend_year_compare_rows,
    },
};
use super::super::build::eval_rowset_with_ctx;

pub(super) fn eval_rowset_trend(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_rowset_trend_year_compare(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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
let window_mode = map
    .get("window")
    .and_then(Value::as_str)
    .unwrap_or("rolling");
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
    .ok_or_else(|| anyhow!("trend_year_compare expression missing years"))?;
Ok(trend_year_compare_rows(
    &rows,
    date_field,
    value_field,
    agg,
    months,
    &years,
    month_label_field,
    year_label_field,
    window_mode,
))
}

