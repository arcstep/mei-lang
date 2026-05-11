use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::DatasetView;

use super::{
    predicate::predicate_matches,
    schema::{row_string, row_value},
    transforms::{
        aggregate_group_rows, distinct_rows_by_fields, first_rows_by_field, mutate_row, rename_fields,
        reorder_fields, select_fields, sort_rows_by_field, summarize_rows,
    },
};

pub(crate) fn eval_rowset(expr: &Value, datasets: &BTreeMap<String, DatasetView>) -> Result<Vec<Value>> {
    match expr {
        Value::Array(items) => Ok(items.clone()),
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                return resolve_data_ref(map, datasets);
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                return eval_analysis_rowset(map, datasets);
            }
            Err(anyhow!("rowset expression must be data_ref or analysis expression"))
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
    let dataset = datasets
        .get(dataset_id)
        .ok_or_else(|| anyhow!("unknown dataset `{dataset_id}`"))?;
    Ok(dataset.rows.clone())
}

fn eval_analysis_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
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
            let normalized = dataset_id
                .strip_prefix("dataset.")
                .unwrap_or(dataset_id)
                .to_string();
            let dataset = datasets
                .get(&normalized)
                .or_else(|| datasets.get(dataset_id))
                .ok_or_else(|| anyhow!("unknown dataset `{dataset_id}`"))?;
            Ok(dataset.rows.clone())
        }
        "where" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("where expression missing rowset"))?;
            let predicate = map.get("predicate").unwrap_or(&Value::Null);
            Ok(eval_rowset(rowset_expr, datasets)?
                .into_iter()
                .filter(|row| predicate_matches(row, predicate))
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
            Ok(eval_rowset(rowset_expr, datasets)?
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
            Ok(eval_rowset(rowset_expr, datasets)?
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
            Ok(eval_rowset(rowset_expr, datasets)?
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
            let mut rows = eval_rowset(rowset_expr, datasets)?;
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
            Ok(eval_rowset(rowset_expr, datasets)?
                .into_iter()
                .map(|row| reorder_fields(&row, &fields))
                .collect())
        }
        "stage" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("stage expression missing rowset"))?;
            Ok(eval_rowset(rowset_expr, datasets)?)
        }
        "first_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("first_by expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("first_by expression missing field"))?;
            let rows = eval_rowset(rowset_expr, datasets)?;
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
            let rows = eval_rowset(rowset_expr, datasets)?;
            Ok(distinct_rows_by_fields(&rows, &fields))
        }
        "group_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("group_by expression missing rowset"))?;
            let rows = eval_rowset(rowset_expr, datasets)?;
            let group_field = map
                .get("by")
                .and_then(Value::as_str)
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
            Ok(aggregate_group_rows(
                &rows,
                group_field,
                value_field,
                agg,
                map.get("limit").and_then(Value::as_u64).map(|n| n as usize),
            ))
        }
        "agg" => {
            let rowset_expr = map
                .get("rowset")
                .or_else(|| map.get("grouped"))
                .ok_or_else(|| anyhow!("agg expression missing rowset"))?;
            let mut rows = eval_rowset(rowset_expr, datasets)?;
            let agg = map
                .get("agg")
                .and_then(Value::as_str)
                .unwrap_or("identity");
            if agg != "identity" {
                let value_field = map.get("value").and_then(Value::as_str).unwrap_or("value");
                rows = summarize_rows(&rows, agg, value_field);
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
            let rows = eval_rowset(rowset_expr, datasets)?;
            let group_field = map
                .get("by")
                .or_else(|| map.get("date_field"))
                .or_else(|| map.get("field"))
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("trend expression missing field"))?;
            let value_field = map.get("value").and_then(Value::as_str);
            let agg = map.get("agg").and_then(Value::as_str).unwrap_or("count");
            Ok(aggregate_group_rows(
                &rows,
                group_field,
                value_field,
                agg,
                map.get("limit").and_then(Value::as_u64).map(|n| n as usize),
            ))
        }
        "table_rows" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("table_rows expression missing rowset"))?;
            Ok(eval_rowset(rowset_expr, datasets)?)
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
            for row in eval_rowset(rowset_expr, datasets)? {
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
            for row in eval_rowset(lookup_rowset_expr, datasets)? {
                let key = row_string(&row, lookup_field);
                let value = row_value(&row, value_field).cloned().unwrap_or(Value::Null);
                lookup.insert(key, value);
            }
            let mut out = Vec::new();
            for row in eval_rowset(rowset_expr, datasets)? {
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
            let mut rows = eval_rowset(rowset_expr, datasets)?;
            let limit = if analysis_type == "latest_days" {
                map.get("days").and_then(Value::as_u64).unwrap_or(rows.len() as u64) as usize
            } else {
                map.get("months").and_then(Value::as_u64).unwrap_or(rows.len() as u64) as usize
            };
            if rows.len() > limit {
                rows = rows.split_off(rows.len() - limit);
            }
            Ok(rows)
        }
        "bucket_date" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("bucket_date expression missing rowset"))?;
            Ok(eval_rowset(rowset_expr, datasets)?)
        }
        "limit" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("limit expression missing rowset"))?;
            let mut rows = eval_rowset(rowset_expr, datasets)?;
            let limit = map.get("n").and_then(Value::as_u64).unwrap_or(0);
            rows.truncate(limit as usize);
            Ok(rows)
        }
        other => Err(anyhow!("unsupported rowset analysis `{other}`")),
    }
}
