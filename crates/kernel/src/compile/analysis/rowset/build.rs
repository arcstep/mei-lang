use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::model::DatasetView;

use super::super::eval_context::{EvalContext, EvalNodeKind};
use super::super::schema::{row_string, row_value};
use super::infer::eval_analysis_rowset;

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

pub(super) fn lookup_dataset_view<'a>(
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

pub(super) fn unknown_dataset_error(
    dataset_id: &str,
    datasets: &BTreeMap<String, DatasetView>,
) -> anyhow::Error {
    let available = datasets.keys().take(8).cloned().collect::<Vec<_>>();
    anyhow!(
        "unknown dataset `{dataset_id}`; available keys: {:?}",
        available
    )
}
pub(super) fn eval_universe_labels(
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

pub(super) fn apply_universe(
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

pub(super) fn eval_split_text_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

pub(super) fn eval_lookup_value_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
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

