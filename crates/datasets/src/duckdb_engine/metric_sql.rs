//! Lower simple MeiLang analysis scalars (count/sum/avg + eq/where) to DuckDB SQL.
//! Complex compositions fall back to the row-eval path.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{ColumnSchema, DatasetView, MetricContract, MetricShape};
use serde_json::{json, Map, Value};

use super::query::{query_parquet_scalar_f64, query_parquet_scalar_i64};
use super::register::resolve_parquet_file_for_source;
use super::sql::{build_where_clause, quote_ident};

#[derive(Debug, Clone)]
pub struct SqlMetricEvalInput<'a> {
    pub app_root: &'a Path,
    pub datasets: &'a BTreeMap<String, DatasetView>,
    pub metric_defs: &'a BTreeMap<String, Value>,
    pub metric_ids: &'a [String],
    /// Extra equality filters applied to every dataset query (already mapped).
    pub global_filters: &'a BTreeMap<String, String>,
    pub search: Option<&'a str>,
}

/// Try to evaluate requested (non-rowset) metrics entirely via DuckDB.
/// Returns `None` when any metric cannot be lowered — caller must use row path.
pub fn try_eval_metrics_via_sql(
    input: SqlMetricEvalInput<'_>,
) -> Result<Option<BTreeMap<String, MetricContract>>> {
    let mut out = BTreeMap::new();
    for metric_id in input.metric_ids {
        if metric_id.contains("__scalar_rowset__") {
            continue;
        }
        let Some(raw) = input.metric_defs.get(metric_id) else {
            return Ok(None);
        };
        let Some(contract) =
            try_eval_one_metric(input.app_root, input.datasets, raw, metric_id, &input)?
        else {
            return Ok(None);
        };
        out.insert(metric_id.clone(), contract);
    }
    Ok(Some(out))
}

fn try_eval_one_metric(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    raw: &Value,
    metric_id: &str,
    input: &SqlMetricEvalInput<'_>,
) -> Result<Option<MetricContract>> {
    let Some(map) = raw.as_object() else {
        return Ok(None);
    };
    let shape_name = map.get("shape").and_then(Value::as_str).unwrap_or_else(|| {
        if map.get("values").is_some() {
            "scalar_map"
        } else {
            "dataframe"
        }
    });
    if !matches!(shape_name, "scalar_map" | "scalar") {
        return Ok(None);
    }
    let schema = map
        .get("schema")
        .and_then(|value| serde_json::from_value::<Vec<ColumnSchema>>(value.clone()).ok())
        .unwrap_or_default();

    let value = if let Some(values) = map.get("values").and_then(Value::as_object) {
        let mut out = Map::new();
        for (entry_key, entry_value) in values {
            let Some(resolved) = try_eval_scalar_expr(
                app_root,
                datasets,
                entry_value,
                input.global_filters,
                input.search,
            )?
            else {
                return Ok(None);
            };
            out.insert(entry_key.clone(), resolved);
        }
        Value::Object(out)
    } else if let Some(expr) = map.get("value") {
        let Some(resolved) =
            try_eval_scalar_expr(app_root, datasets, expr, input.global_filters, input.search)?
        else {
            return Ok(None);
        };
        resolved
    } else {
        return Ok(None);
    };

    Ok(Some(MetricContract {
        id: metric_id.to_string(),
        label: map
            .get("label")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        unit: map
            .get("unit")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        value_format: map.get("value_format").cloned(),
        purpose: None,
        shape: MetricShape::Scalar,
        schema,
        dataset: map
            .get("dataset")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        transforms: Vec::new(),
        value,
    }))
}

fn try_eval_scalar_expr(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    expr: &Value,
    global_filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> Result<Option<Value>> {
    let Some(object) = expr.as_object() else {
        // Literal constants are fine.
        return Ok(Some(expr.clone()));
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(Some(expr.clone()));
    }
    let analysis_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    match analysis_type {
        "count" => {
            let rowset = object.get("rowset").unwrap_or(&Value::Null);
            let Some((dataset_id, mut filters)) = lower_rowset_to_filters(rowset)? else {
                return Ok(None);
            };
            merge_filters(&mut filters, global_filters);
            let Some(view) = lookup_dataset(datasets, &dataset_id) else {
                return Ok(None);
            };
            // GeoJSON / non-snapshot sources have no parquet — fall back to row eval.
            let Some(count) = count_dataset(app_root, view, &filters, search)? else {
                return Ok(None);
            };
            Ok(Some(json!(count)))
        }
        "sum" | "avg" | "min" | "max" => {
            let Some(value_expr) = object.get("value") else {
                return Ok(None);
            };
            let Some((dataset_id, field, mut filters)) = lower_number_source(value_expr)? else {
                return Ok(None);
            };
            merge_filters(&mut filters, global_filters);
            let Some(view) = lookup_dataset(datasets, &dataset_id) else {
                return Ok(None);
            };
            let agg = match analysis_type {
                "sum" => "SUM",
                "avg" => "AVG",
                "min" => "MIN",
                _ => "MAX",
            };
            let Some(n) = agg_dataset_f64(app_root, view, field.as_str(), agg, &filters, search)?
            else {
                return Ok(None);
            };
            Ok(Some(json!(n)))
        }
        "ratio" => {
            let Some(num) = try_eval_scalar_expr(
                app_root,
                datasets,
                object.get("numerator").unwrap_or(&Value::Null),
                global_filters,
                search,
            )?
            else {
                return Ok(None);
            };
            let Some(den) = try_eval_scalar_expr(
                app_root,
                datasets,
                object.get("denominator").unwrap_or(&Value::Null),
                global_filters,
                search,
            )?
            else {
                return Ok(None);
            };
            let n = num.as_f64().or_else(|| num.as_i64().map(|v| v as f64)).unwrap_or(0.0);
            let d = den.as_f64().or_else(|| den.as_i64().map(|v| v as f64)).unwrap_or(0.0);
            let ratio = if d.abs() < f64::EPSILON {
                0.0
            } else {
                n / d
            };
            Ok(Some(json!(ratio)))
        }
        "change_rate" => {
            let Some(current) = try_eval_scalar_expr(
                app_root,
                datasets,
                object.get("current").unwrap_or(&Value::Null),
                global_filters,
                search,
            )?
            else {
                return Ok(None);
            };
            let Some(base) = try_eval_scalar_expr(
                app_root,
                datasets,
                object.get("base").unwrap_or(&Value::Null),
                global_filters,
                search,
            )?
            else {
                return Ok(None);
            };
            let current = current
                .as_f64()
                .or_else(|| current.as_i64().map(|v| v as f64))
                .unwrap_or(0.0);
            let base = base
                .as_f64()
                .or_else(|| base.as_i64().map(|v| v as f64))
                .unwrap_or(0.0);
            let mode = object
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("growth");
            let scale = object
                .get("scale")
                .and_then(Value::as_f64)
                .or_else(|| object.get("scale").and_then(Value::as_i64).map(|v| v as f64))
                .unwrap_or(100.0);
            let delta = if mode.eq_ignore_ascii_case("reduction") {
                base - current
            } else {
                current - base
            };
            let rate = if base.abs() < f64::EPSILON {
                0.0
            } else {
                delta / base.abs() * scale
            };
            Ok(Some(json!(rate)))
        }
        _ => Ok(None),
    }
}

/// Lower a rowset expr into (dataset_id, equality filters).
fn lower_rowset_to_filters(expr: &Value) -> Result<Option<(String, BTreeMap<String, String>)>> {
    let Some(object) = expr.as_object() else {
        return Ok(None);
    };
    // Runtime defs often keep dataset bindings as `__ref: data`.
    if object.get("__ref").and_then(Value::as_str) == Some("data") {
        let id = object
            .get("from_dataset")
            .or_else(|| object.get("id"))
            .or_else(|| object.get("dataset"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.strip_prefix("dataset.").unwrap_or(s).to_string());
        return Ok(id.map(|id| (id, BTreeMap::new())));
    }
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(None);
    }
    match object.get("type").and_then(Value::as_str).unwrap_or("") {
        "rows" => {
            let id = object
                .get("dataset")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.strip_prefix("dataset.").unwrap_or(s).to_string());
            Ok(id.map(|id| (id, BTreeMap::new())))
        }
        "where" => {
            let Some(inner) = object.get("rowset") else {
                return Ok(None);
            };
            let Some((dataset_id, mut filters)) = lower_rowset_to_filters(inner)? else {
                return Ok(None);
            };
            let Some(predicate) = object.get("predicate") else {
                return Ok(None);
            };
            if !append_predicate_filters(predicate, &mut filters)? {
                return Ok(None);
            }
            Ok(Some((dataset_id, filters)))
        }
        _ => Ok(None),
    }
}

fn lower_number_source(
    expr: &Value,
) -> Result<Option<(String, String, BTreeMap<String, String>)>> {
    let Some(object) = expr.as_object() else {
        return Ok(None);
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(None);
    }
    if object.get("type").and_then(Value::as_str) != Some("number") {
        return Ok(None);
    }
    let field = object
        .get("field")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let Some(field) = field else {
        return Ok(None);
    };
    let source = object
        .get("source")
        .or_else(|| object.get("rowset"))
        .unwrap_or(&Value::Null);
    let Some((dataset_id, filters)) = lower_rowset_to_filters(source)? else {
        return Ok(None);
    };
    Ok(Some((dataset_id, field, filters)))
}

fn append_predicate_filters(
    predicate: &Value,
    filters: &mut BTreeMap<String, String>,
) -> Result<bool> {
    let Some(object) = predicate.as_object() else {
        return Ok(false);
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(false);
    }
    match object.get("type").and_then(Value::as_str).unwrap_or("") {
        "eq" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(false);
            }
            let value = match object.get("value") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) => String::new(),
                _ => return Ok(false),
            };
            filters.insert(field.to_string(), value);
            Ok(true)
        }
        "between" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            if field.is_empty() {
                return Ok(false);
            }
            let lower = object
                .get("lower")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let upper = object
                .get("upper")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let (Some(lower), Some(upper)) = (lower, upper) else {
                return Ok(false);
            };
            filters.insert(field.to_string(), format!("between:{lower}..{upper}"));
            Ok(true)
        }
        "and" => {
            let Some(items) = object.get("predicates").and_then(Value::as_array) else {
                return Ok(false);
            };
            for item in items {
                if !append_predicate_filters(item, filters)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn merge_filters(dst: &mut BTreeMap<String, String>, src: &BTreeMap<String, String>) {
    for (k, v) in src {
        dst.entry(k.clone()).or_insert_with(|| v.clone());
    }
}

fn lookup_dataset<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    datasets.get(dataset_id).or_else(|| {
        let local = dataset_id.rsplit("::").next().unwrap_or(dataset_id);
        datasets.get(local).or_else(|| {
            datasets
                .values()
                .find(|view| view.id == dataset_id || view.id.ends_with(dataset_id))
        })
    })
}

/// Whether this dataset may participate in the DuckDB SQL compute path.
///
/// GeoJSON / geometry FeatureCollections stay on the ds row path
/// (`geojson_dataset` + kernel `parse_geojson_rows`). Missing parquet must never
/// hard-fail a metric batch — callers treat `None` as "fall back to row eval".
fn sql_snapshot_eligible(view: &DatasetView) -> bool {
    let kind = view.source.kind.trim().to_ascii_lowercase();
    let path = view.source.path.as_str();
    if kind == "geojson" || path.ends_with(".geojson") {
        return false;
    }
    true
}

fn parquet_for_view(app_root: &Path, view: &DatasetView) -> Option<std::path::PathBuf> {
    if !sql_snapshot_eligible(view) {
        return None;
    }
    let header = view.source.header_row.unwrap_or(1).max(1) as usize;
    // Tabular uploads/imports: resolve parquet snapshot if present; else None → row fallback.
    resolve_parquet_file_for_source(
        app_root,
        view.source.path.as_str(),
        view.source.sheet.as_deref(),
        header,
    )
}

fn count_dataset(
    app_root: &Path,
    view: &DatasetView,
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> Result<Option<i64>> {
    let Some(parquet) = parquet_for_view(app_root, view) else {
        return Ok(None);
    };
    let where_sql = build_where_clause(filters, search, &view.columns)?;
    Ok(Some(query_parquet_scalar_i64(
        app_root,
        parquet.as_path(),
        &view.schema,
        &format!("COUNT(*)"),
        where_sql.as_str(),
    )?))
}

fn agg_dataset_f64(
    app_root: &Path,
    view: &DatasetView,
    field: &str,
    agg: &str,
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> Result<Option<f64>> {
    let Some(parquet) = parquet_for_view(app_root, view) else {
        return Ok(None);
    };
    let col = quote_ident(field)?;
    let where_sql = build_where_clause(filters, search, &view.columns)?;
    Ok(Some(query_parquet_scalar_f64(
        app_root,
        parquet.as_path(),
        &view.schema,
        &format!("COALESCE({agg}(try_cast({col} AS DOUBLE)), 0)"),
        where_sql.as_str(),
    )?))
}

/// Count rows for a primary dataset with query filters (for total_rows without collect_all).
pub fn count_primary_dataset_rows(
    app_root: &Path,
    view: &DatasetView,
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> Result<usize> {
    Ok(count_dataset(app_root, view, filters, search)?
        .unwrap_or(0)
        .max(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lowers_count_rows_dataset() {
        let expr = json!({
            "__kind": "analysis_expr",
            "type": "count",
            "rowset": {
                "__kind": "analysis_expr",
                "type": "rows",
                "dataset": "enforcement_units"
            }
        });
        let lowered = lower_rowset_to_filters(expr.get("rowset").unwrap()).unwrap();
        assert_eq!(
            lowered.unwrap().0,
            "enforcement_units"
        );
    }

    #[test]
    fn lowers_where_eq_filters() {
        let rowset = json!({
            "__kind": "analysis_expr",
            "type": "where",
            "rowset": {
                "__kind": "analysis_expr",
                "type": "rows",
                "dataset": "inspection"
            },
            "predicate": {
                "__kind": "analysis_expr",
                "type": "eq",
                "field": "检查结果",
                "value": "无违规项"
            }
        });
        let (id, filters) = lower_rowset_to_filters(&rowset).unwrap().unwrap();
        assert_eq!(id, "inspection");
        assert_eq!(filters.get("检查结果").map(String::as_str), Some("无违规项"));
    }

    #[test]
    fn lowers_data_ref_and_between_filters() {
        let rowset = json!({
            "__kind": "analysis_expr",
            "type": "where",
            "rowset": {
                "__ref": "data",
                "from_dataset": "administrative_inspection",
                "id": "administrative_inspection"
            },
            "predicate": {
                "__kind": "analysis_expr",
                "type": "between",
                "field": "检查日期",
                "lower": "2024-01-01",
                "upper": "2024-12-31"
            }
        });
        let (id, filters) = lower_rowset_to_filters(&rowset).unwrap().unwrap();
        assert_eq!(id, "administrative_inspection");
        assert_eq!(
            filters.get("检查日期").map(String::as_str),
            Some("between:2024-01-01..2024-12-31")
        );
        let where_sql = build_where_clause(&filters, None, &["检查日期".into()]).unwrap();
        assert!(where_sql.contains("BETWEEN"));
        assert!(where_sql.contains("2024-01-01"));
        // Excel-serial branch must not try_cast the raw column to DOUBLE
        // (DataFusion rejects Date32 → Float64).
        assert!(where_sql.contains("CAST(\"检查日期\" AS VARCHAR)"));
        assert!(!where_sql.contains("try_cast(\"检查日期\" AS DOUBLE)"));
    }

    #[test]
    fn quote_helpers_roundtrip() {
        use super::super::sql::quote_string;
        assert!(quote_ident("").is_err());
        assert_eq!(quote_ident("检查结果").unwrap(), "\"检查结果\"");
        assert_eq!(quote_string("a'b"), "'a''b'");
    }
}
