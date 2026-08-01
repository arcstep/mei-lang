use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::local_dataset_id_from_namespaced_token;
use crate::model::DatasetView;

use super::super::eval_context::{EvalContext, EvalNodeKind};
use super::super::object_keys::split_multi_object_keys;
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
        return Ok(rows.as_ref().clone());
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
            if map.get("__ref").and_then(Value::as_str) == Some("metric") {
                return resolve_metric_ref_rowset(map, datasets, ctx);
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                return eval_analysis_rowset(map, datasets, ctx);
            }
            Err(anyhow!(
                "rowset expression must be data_ref, metric_ref, or analysis expression"
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

fn resolve_metric_ref_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let metric_id = map
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("metric_ref missing id"))?;
    if let Some(rows) = ctx.resolved_metric_rowset(metric_id) {
        return Ok(rows.as_ref().clone());
    }
    let node_key = format!("metric_ref:{metric_id}");
    ctx.with_eval_node(&node_key, EvalNodeKind::Rowset, |ctx| {
        let def = ctx
            .metric_def(metric_id)
            .ok_or_else(|| anyhow!("unknown metric_ref `{metric_id}`"))?
            .clone();
        let def_map = def
            .as_object()
            .ok_or_else(|| anyhow!("metric_ref `{metric_id}` is not an object"))?;
        let rowset_expr = def_map
            .get("series")
            .or_else(|| def_map.get("list"))
            .or_else(|| def_map.get("value"))
            .cloned()
            .ok_or_else(|| anyhow!("metric_ref `{metric_id}` has no rowset value"))?;
        let rows = eval_rowset_with_ctx(&rowset_expr, datasets, ctx)?;
        ctx.store_resolved_metric_rowset(metric_id, &rows);
        Ok(rows)
    })
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
        .or_else(|| {
            local_dataset_id_from_namespaced_token(normalized)
                .and_then(|local| lookup_dataset_view(datasets, local))
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
    let on_empty = map
        .get("on_empty")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("keep");
    let drop_empty = on_empty.eq_ignore_ascii_case("drop");
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
            if !drop_empty {
                out.push(Value::Object(base));
            }
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
        let value = lookup
            .get(&key)
            .cloned()
            .or_else(|| {
                split_multi_object_keys(&key)
                    .into_iter()
                    .find_map(|token| lookup.get(&token).cloned())
            })
            .unwrap_or(Value::Null);
        object.insert(as_field.clone(), value);
        out.push(Value::Object(object));
    }
    Ok(out)
}

/// Like `lookup_value`, but collects **all** matching lookup rows per multi-value key
/// (典型案例：一个单元格多个处理结果ID，每个 ID 可对应多条联合主键)。
pub(super) fn eval_lookup_collect_rowset(
    map: &serde_json::Map<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    ctx: &mut EvalContext,
) -> Result<Vec<Value>> {
    let rowset_expr = map
        .get("rowset")
        .ok_or_else(|| anyhow!("lookup_collect expression missing rowset"))?;
    let lookup_rowset_expr = map
        .get("lookup_rowset")
        .ok_or_else(|| anyhow!("lookup_collect expression missing lookup_rowset"))?;
    let field = map
        .get("field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("lookup_collect expression missing field"))?;
    let lookup_field = map
        .get("lookup_field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("lookup_collect expression missing lookup_field"))?;
    let value_field = map
        .get("value_field")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("lookup_collect expression missing value_field"))?;
    let as_field = map
        .get("as_field")
        .and_then(Value::as_str)
        .unwrap_or(value_field)
        .to_string();
    let delimiter = map
        .get("delimiter")
        .and_then(Value::as_str)
        .unwrap_or("、");
    let mut lookup_rows = Vec::new();
    for row in eval_rowset_with_ctx(lookup_rowset_expr, datasets, ctx)? {
        lookup_rows.push(row);
    }
    let mut out = Vec::new();
    for row in eval_rowset_with_ctx(rowset_expr, datasets, ctx)? {
        let mut object = row.as_object().cloned().unwrap_or_default();
        let mut collected = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for key in split_multi_object_keys(&row_string(&row, field)) {
            if key.is_empty() {
                continue;
            }
            for lookup_row in &lookup_rows {
                let lookup_key = row_string(lookup_row, lookup_field);
                if lookup_key != key {
                    continue;
                }
                let value = row_string(lookup_row, value_field);
                if value.is_empty() || !seen.insert(value.clone()) {
                    continue;
                }
                collected.push(value);
            }
        }
        let joined = collected.join(delimiter);
        object.insert(
            as_field.clone(),
            if joined.is_empty() {
                Value::Null
            } else {
                Value::String(joined)
            },
        );
        out.push(Value::Object(object));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::analysis::eval_context::RuntimeMetricEvalScope;
    use crate::compile::analysis::schema::row_number;
    use crate::model::SourceDecl;
    use serde_json::json;

    #[test]
    fn eval_rowset_resolves_metric_ref_to_grouped_dataframe() {
        let scalar_rowset_id = "sales_total::__scalar_rowset__".to_string();
        let composition_id = "sales_total::composition_by_agency".to_string();
        let composition_expr = json!({
            "__kind": "analysis_expr",
            "type": "group_by",
            "rowset": {"__ref": "metric", "id": scalar_rowset_id},
            "by": "agency",
            "agg": "count"
        });
        let mut metric_defs = BTreeMap::new();
        metric_defs.insert(
            scalar_rowset_id.clone(),
            json!({
                "shape": "dataframe",
                "value": {
                    "__kind": "analysis_expr",
                    "type": "rows",
                    "dataset": "sales"
                }
            }),
        );
        metric_defs.insert(
            composition_id.clone(),
            json!({
                "shape": "dataframe",
                "value": composition_expr
            }),
        );
        let mut datasets = BTreeMap::new();
        datasets.insert(
            "sales".to_string(),
            DatasetView {
                id: "sales".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: vec![
                    json!({"agency": "A"}),
                    json!({"agency": "A"}),
                    json!({"agency": "B"}),
                ],
                source: SourceDecl {
                    kind: "inline".to_string(),
                    path: "test".to_string(),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    primary_key: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: BTreeMap::new(),
            },
        );
        let mut ctx =
            EvalContext::with_scope_and_metric_defs(RuntimeMetricEvalScope::default(), metric_defs);
        let rows = eval_rowset_with_ctx(&composition_expr, &datasets, &mut ctx).expect("group_by");
        assert_eq!(rows.len(), 2);
        assert_eq!(row_string(&rows[0], "agency"), "A");
        assert_eq!(row_number(&rows[0], "value"), Some(2.0));
        assert_eq!(row_string(&rows[1], "agency"), "B");
        assert_eq!(row_number(&rows[1], "value"), Some(1.0));
    }

    #[test]
    fn eval_rowset_metric_ref_chain_trips_depth_guard() {
        // m0 → m1 → m2 → m3 (leaf rows). With max_eval_depth=1, nested metric_ref
        // frames overflow before the leaf can evaluate.
        let mut metric_defs = BTreeMap::new();
        for i in 0..3 {
            let id = format!("m{i}");
            let next = format!("m{}", i + 1);
            metric_defs.insert(
                id,
                json!({
                    "shape": "dataframe",
                    "value": {"__ref": "metric", "id": next}
                }),
            );
        }
        metric_defs.insert(
            "m3".to_string(),
            json!({
                "shape": "dataframe",
                "value": {
                    "__kind": "analysis_expr",
                    "type": "rows",
                    "dataset": "sales"
                }
            }),
        );
        let mut datasets = BTreeMap::new();
        datasets.insert(
            "sales".to_string(),
            DatasetView {
                id: "sales".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: Vec::new(),
                rows: vec![json!({"agency": "A"})],
                source: SourceDecl {
                    kind: "inline".to_string(),
                    path: "test".to_string(),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    primary_key: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: BTreeMap::new(),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: BTreeMap::new(),
            },
        );
        let mut ctx =
            EvalContext::with_scope_and_metric_defs(RuntimeMetricEvalScope::default(), metric_defs);
        ctx.max_eval_depth = 1;
        let root = json!({"__ref": "metric", "id": "m0"});
        let err = eval_rowset_with_ctx(&root, &datasets, &mut ctx)
            .expect_err("deep metric_ref chain should trip depth guard");
        let message = err.to_string();
        assert!(
            message.contains("metric_eval_recursion_guard_tripped"),
            "unexpected error: {message}"
        );
    }
}
