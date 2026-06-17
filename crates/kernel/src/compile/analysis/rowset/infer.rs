use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;

use super::super::dates::{filter_rows_in_latest_days, filter_rows_in_latest_months};
use super::super::eval_context::EvalContext;
use super::super::predicate::predicate_matches_with_ctx;
use super::super::transforms::{
    aggregate_group_rows, aggregate_group_rows_pivot, bucket_rows_by_month,
    distinct_rows_by_fields, first_rows_by_field, mutate_row, party_year_aggregate_rows,
    rename_fields, reorder_fields, select_fields, sort_rows_by_field, summarize_rows,
    trend_rows_by_month, trend_year_compare_rows, unpivot_columns_rows, pivot_long_rows,
};
use super::build::{
    apply_universe, eval_lookup_value_rowset, eval_rowset_with_ctx, eval_split_text_rowset,
    eval_universe_labels, lookup_dataset_view, unknown_dataset_error,
};

pub(super) fn eval_analysis_rowset(
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
        "party_year_aggregate" => {
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
        "unpivot_columns" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("unpivot_columns expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let id_field = map
                .get("id_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("unpivot_columns expression missing id_field"))?;
            let year_field = map
                .get("year_field")
                .and_then(Value::as_str)
                .unwrap_or("year");
            let value_field = map
                .get("value_field")
                .and_then(Value::as_str)
                .unwrap_or("value");
            let columns = map
                .get("columns")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("unpivot_columns expression missing columns"))?
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    let year = object.get("year")?.as_str()?.to_string();
                    let field = object.get("field")?.as_str()?.to_string();
                    Some((year, field))
                })
                .collect::<Vec<_>>();
            if columns.is_empty() {
                return Err(anyhow!(
                    "unpivot_columns expression missing column mappings"
                ));
            }
            Ok(unpivot_columns_rows(
                &rows,
                id_field,
                &columns,
                year_field,
                value_field,
            ))
        }
        "pivot_long" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("pivot_long expression missing rowset"))?;
            let rows = eval_rowset_with_ctx(rowset_expr, datasets, ctx)?;
            let row_field = map
                .get("row_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("pivot_long expression missing row_field"))?;
            let column_field = map
                .get("column_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("pivot_long expression missing column_field"))?;
            let value_field = map
                .get("value_field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("pivot_long expression missing value_field"))?;
            let columns = map
                .get("columns")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.as_str()
                                .map(str::to_string)
                                .or_else(|| item.as_i64().map(|value| value.to_string()))
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|items| !items.is_empty())
                .ok_or_else(|| anyhow!("pivot_long expression missing columns"))?;
            let row_universe = map
                .get("row_universe")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .filter(|items| !items.is_empty());
            Ok(pivot_long_rows(
                &rows,
                row_field,
                column_field,
                value_field,
                &columns,
                row_universe.as_deref(),
            ))
        }
        "table_rows" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("table_rows expression missing rowset"))?;
            Ok(eval_rowset_with_ctx(rowset_expr, datasets, ctx)?)
        }
        "split_text" => eval_split_text_rowset(map, datasets, ctx),
        "lookup_value" => eval_lookup_value_rowset(map, datasets, ctx),
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
        "concat_rowsets" => {
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
        other => Err(anyhow!("unsupported rowset analysis `{other}`")),
    }
}
