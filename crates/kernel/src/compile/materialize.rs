use std::{collections::BTreeMap, path::Path};

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::model::{
    ColumnSchema, DataTransform, DatasetView, LoadedResource, MetricContract, MetricShape,
    SourceDecl,
};

use super::{
    analysis::{
        rowset::eval_rowset,
        scalar::eval_scalar_value,
        schema::{infer_columns, infer_schema_from_rows},
    },
    decls::{
        LegacyDatasetDecl, LegacyMetricPackDecl, LegacySourceDecl,
    },
    loaders::load_legacy_xlsx_rows,
    resources::csv_record_to_json,
};

const DEFAULT_PREVIEW_ROWS: usize = 1000;
const DEFAULT_PAGE_SIZE: usize = 100;
const DEFAULT_MAX_PAGE_SIZE: usize = 1000;

#[derive(Debug, Clone)]
struct LegacyRowsSnapshot {
    rows: Vec<Value>,
    truncated: bool,
}

pub(super) fn materialize_legacy_datasets(
    app_root: &Path,
    resources: &[LoadedResource],
    decls: &[LegacyDatasetDecl],
) -> Result<Vec<LoadedResource>> {
    let mut datasets = BTreeMap::<String, DatasetView>::new();
    for resource in resources {
        if let Some(dataset) = &resource.dataset {
            datasets.insert(resource.id.clone(), dataset.clone());
        }
    }

    let mut compiled = Vec::new();
    for decl in decls {
        let dataset_id = decl
            .data_ref
            .as_deref()
            .and_then(|value| value.strip_prefix("dataset."))
            .map(ToString::to_string)
            .unwrap_or_else(|| decl.dataset.key.clone());
        let source_decl = legacy_dataset_source_decl(&decl.source, &decl.dataset.normalize, false);
        let mut source_truncated = false;
        let mut rows = if decl.dataset.kind == "dataframe" {
            let snapshot = load_legacy_rows_from_source(app_root, &decl.source)?;
            source_truncated = snapshot.truncated;
            snapshot.rows
        } else {
            Vec::new()
        };
        if let Some(rowset) = &decl.dataset.rowset {
            rows = eval_rowset(rowset, &datasets)
                .with_context(|| format!("failed to materialize legacy rowset `{dataset_id}`"))?;
        }
        if !decl.dataset.normalize.is_empty() {
            rows = apply_legacy_normalize(rows, &decl.dataset.normalize);
        }
        let schema = if decl.dataset.columns.is_empty() {
            infer_schema_from_rows(&rows)
        } else {
            decl.dataset.columns.clone()
        };
        let columns = if schema.is_empty() {
            infer_columns(&rows)
        } else {
            schema.iter().map(|column| column.name.clone()).collect()
        };
        let dataset_stub = DatasetView {
            id: dataset_id.clone(),
            title: decl.title.clone(),
            purpose: None,
            schema: schema.clone(),
            stage_schema: Vec::new(),
            columns: columns.clone(),
            rows: rows.clone(),
            source: source_decl.clone(),
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
        };
        datasets.insert(dataset_id.clone(), dataset_stub.clone());
        let metrics = materialize_legacy_metric_map(&decl.metrics, &rows, &datasets)
            .with_context(|| format!("failed to compile legacy metrics for `{dataset_id}`"))?;
        let dataset = DatasetView {
            id: dataset_id.clone(),
            title: decl.title.clone(),
            purpose: None,
            schema,
            stage_schema: Vec::new(),
            columns,
            rows,
            source: legacy_dataset_source_decl(
                &decl.source,
                &decl.dataset.normalize,
                source_truncated,
            ),
            sources: Vec::new(),
            metrics,
            runtime_metric_defs: decl.metrics.clone(),
        };
        datasets.insert(dataset_id.clone(), dataset.clone());
        compiled.push(LoadedResource {
            id: dataset_id.clone(),
            kind: "dataset".to_string(),
            title: decl.title.clone(),
            document: None,
            dataset: Some(dataset.clone()),
        });
    }
    Ok(compiled)
}

fn load_legacy_rows_from_source(
    app_root: &Path,
    source: &LegacySourceDecl,
) -> Result<LegacyRowsSnapshot> {
    let source_path = source
        .file
        .as_deref()
        .or(source.path.as_deref())
        .unwrap_or("");
    let preview_rows = source_preview_rows(source);
    if source_path.is_empty() && source.connection.is_none() {
        return Ok(LegacyRowsSnapshot {
            rows: Vec::new(),
            truncated: false,
        });
    }
    let path_lower = source_path.to_ascii_lowercase();
    let inferred = if path_lower.ends_with(".xlsx") || path_lower.ends_with(".xls") {
        "xlsx"
    } else {
        "csv"
    };
    let source_kind = source.kind.as_deref().unwrap_or(inferred);
    let path = app_root.join(source_path);
    match source_kind {
        "csv" => {
            let mut reader = csv::Reader::from_path(&path)
                .with_context(|| format!("failed to open dataset {}", path.display()))?;
            let headers = reader
                .headers()
                .context("failed to read csv headers")?
                .clone();
            let mut rows = Vec::new();
            let mut truncated = false;
            for record in reader.records() {
                let record = record.context("failed to read csv row")?;
                if rows.len() >= preview_rows {
                    truncated = true;
                    break;
                }
                rows.push(csv_record_to_json(&headers, &record));
            }
            Ok(LegacyRowsSnapshot { rows, truncated })
        }
        "db" => {
            let rows = load_legacy_db_rows(app_root, source, preview_rows)?;
            let truncated = rows.len() >= preview_rows;
            Ok(LegacyRowsSnapshot { rows, truncated })
        }
        "json" => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read json dataset {}", path.display()))?;
            let json: Value = serde_json::from_str(&raw)
                .with_context(|| format!("invalid json dataset {}", path.display()))?;
            let mut rows = json.as_array().cloned().unwrap_or_default();
            let truncated = rows.len() > preview_rows;
            rows.truncate(preview_rows);
            Ok(LegacyRowsSnapshot { rows, truncated })
        }
        "xlsx" => {
            let header_row = source.header_row.unwrap_or(1).max(1) as usize;
            let rows = load_legacy_xlsx_rows(
                &path,
                source.sheet.as_deref(),
                header_row,
                Some(preview_rows),
            )
            .with_context(|| format!("failed to read xlsx dataset {}", path.display()))?;
            let truncated = rows.len() >= preview_rows;
            Ok(LegacyRowsSnapshot { rows, truncated })
        }
        other => Err(anyhow!("unsupported legacy dataset source kind `{other}`")),
    }
}

fn source_preview_rows(source: &LegacySourceDecl) -> usize {
    source
        .preview_rows
        .unwrap_or(DEFAULT_PREVIEW_ROWS as i64)
        .max(1) as usize
}

fn source_page_size(source: &LegacySourceDecl) -> usize {
    source.page_size.unwrap_or(DEFAULT_PAGE_SIZE as i64).max(1) as usize
}

fn source_max_page_size(source: &LegacySourceDecl) -> usize {
    source
        .max_page_size
        .unwrap_or(DEFAULT_MAX_PAGE_SIZE as i64)
        .max(1) as usize
}

fn legacy_dataset_source_decl(
    source: &LegacySourceDecl,
    normalize: &BTreeMap<String, String>,
    preview_truncated: bool,
) -> SourceDecl {
    let source_path = source
        .file
        .as_deref()
        .or(source.path.as_deref())
        .unwrap_or("")
        .to_string();
    let kind = source.kind.clone().unwrap_or_else(|| {
        if source_path.ends_with(".xlsx") || source_path.ends_with(".xls") {
            "xlsx".to_string()
        } else {
            "csv".to_string()
        }
    });
    let meta = serde_json::json!({
        "lazy": {
            "preview_rows": source_preview_rows(source),
            "default_page_size": source_page_size(source),
            "max_page_size": source_max_page_size(source),
            "truncated": preview_truncated,
        },
        "sheet": source.sheet,
        "header_row": source.header_row.unwrap_or(1),
        "table": source.table,
        "query": source.query,
        "connection": source.connection,
        "normalize": normalize,
    });
    SourceDecl {
        kind,
        path: source_path,
        sheet: source.sheet.clone(),
        header_row: source.header_row,
        preview_rows: source.preview_rows,
        page_size: source.page_size,
        max_page_size: source.max_page_size,
        table: source.table.clone(),
        query: source.query.clone(),
        connection: source.connection.clone(),
        content: serde_json::to_string(&meta).ok(),
    }
}

fn load_legacy_db_rows(
    app_root: &Path,
    source: &LegacySourceDecl,
    preview_rows: usize,
) -> Result<Vec<Value>> {
    use rusqlite::{types::ValueRef, Connection};
    let dsn = source
        .connection
        .clone()
        .or_else(|| source.file.clone())
        .or_else(|| source.path.clone())
        .ok_or_else(|| anyhow!("db source missing connection/path"))?;
    let db_path = dsn
        .strip_prefix("sqlite://")
        .map(ToString::to_string)
        .unwrap_or(dsn);
    let db_path = if Path::new(&db_path).is_absolute() {
        Path::new(&db_path).to_path_buf()
    } else {
        app_root.join(db_path)
    };
    let conn = Connection::open(&db_path)
        .with_context(|| format!("failed to open sqlite db {}", db_path.display()))?;
    let base_sql = if let Some(query) = source.query.as_deref().filter(|v| !v.trim().is_empty()) {
        format!("SELECT * FROM ({query})")
    } else if let Some(table) = source.table.as_deref().filter(|v| !v.trim().is_empty()) {
        format!("SELECT * FROM \"{}\"", table.replace('"', "\"\""))
    } else {
        return Err(anyhow!("db source needs table or query"));
    };
    let sql = format!("{base_sql} LIMIT {}", preview_rows.max(1));
    let mut stmt = conn.prepare(&sql)?;
    let column_names = stmt
        .column_names()
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let rows = stmt
        .query_map([], |row| {
            let mut map = serde_json::Map::new();
            for (idx, key) in column_names.iter().enumerate() {
                let value = match row.get_ref(idx)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(v) => serde_json::json!(v),
                    ValueRef::Real(v) => serde_json::json!(v),
                    ValueRef::Text(v) => Value::String(String::from_utf8_lossy(v).to_string()),
                    ValueRef::Blob(v) => Value::String(format!("<blob:{} bytes>", v.len())),
                };
                map.insert(key.clone(), value);
            }
            Ok::<_, rusqlite::Error>(Value::Object(map))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn apply_legacy_normalize(rows: Vec<Value>, normalize: &BTreeMap<String, String>) -> Vec<Value> {
    rows.into_iter()
        .map(|row| {
            let mut out = row.as_object().cloned().unwrap_or_default();
            for (source, target) in normalize {
                if source == target {
                    continue;
                }
                if let Some(value) = out.remove(source) {
                    out.insert(target.clone(), value);
                }
            }
            Value::Object(out)
        })
        .collect()
}

pub(super) fn materialize_metric_packs(
    resources: &[LoadedResource],
    packs: &[LegacyMetricPackDecl],
) -> Result<Vec<LoadedResource>> {
    let mut datasets = BTreeMap::<String, DatasetView>::new();
    for resource in resources {
        if let Some(dataset) = &resource.dataset {
            datasets.insert(resource.id.clone(), dataset.clone());
        }
    }

    let mut compiled = Vec::new();
    for pack in packs {
        let metrics = materialize_legacy_metric_map(&pack.metrics, &[], &datasets)
            .with_context(|| format!("failed to compile metric_pack `{}`", pack.metric_pack.id))?;
        let dataset = DatasetView {
            id: pack.metric_pack.id.clone(),
            title: pack.metric_pack.purpose.clone(),
            purpose: pack.metric_pack.purpose.clone(),
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: format!("legacy.metric_pack:{}", pack.metric_pack.id),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics,
            runtime_metric_defs: pack.metrics.clone(),
        };
        datasets.insert(pack.metric_pack.id.clone(), dataset.clone());
        compiled.push(LoadedResource {
            id: pack.metric_pack.id.clone(),
            kind: "dataset".to_string(),
            title: pack.metric_pack.purpose.clone(),
            document: None,
            dataset: Some(dataset),
        });
    }
    Ok(compiled)
}

pub(super) fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, MetricContract>> {
    if let Some(ids) = metric_ids {
        let selected = ids
            .iter()
            .filter_map(|id| metric_defs.get(id).cloned().map(|value| (id.clone(), value)))
            .collect::<BTreeMap<_, _>>();
        return materialize_legacy_metric_map(&selected, base_rows, datasets);
    }
    materialize_legacy_metric_map(metric_defs, base_rows, datasets)
}

fn materialize_legacy_metric_map(
    decls: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<BTreeMap<String, MetricContract>> {
    let mut metrics = BTreeMap::new();
    for (metric_id, raw) in decls {
        let Some(map) = raw.as_object() else {
            continue;
        };
        let shape_name = map.get("shape").and_then(Value::as_str).unwrap_or_else(|| {
            if map.get("values").is_some() {
                "scalar_map"
            } else {
                "dataframe"
            }
        });
        let shape = match shape_name {
            "scalar_map" | "scalar" => MetricShape::Scalar,
            "series" => MetricShape::Series,
            "table" => MetricShape::Table,
            _ => MetricShape::Dataframe,
        };
        let schema = map
            .get("schema")
            .and_then(|value| serde_json::from_value::<Vec<ColumnSchema>>(value.clone()).ok())
            .unwrap_or_default();
        let value = if let Some(values) = map.get("values").and_then(Value::as_object) {
            let mut out = serde_json::Map::new();
            for (entry_key, entry_value) in values {
                let resolved = eval_scalar_value(entry_value, base_rows, datasets)
                    .with_context(|| format!("legacy metric `{metric_id}` field `{entry_key}`"))?;
                out.insert(entry_key.clone(), resolved);
            }
            Value::Object(out)
        } else if let Some(rowset) = map
            .get("series")
            .or_else(|| map.get("list"))
            .or_else(|| map.get("value"))
        {
            if let Ok(rows) = eval_rowset(rowset, datasets) {
                Value::Array(rows)
            } else {
                eval_scalar_value(rowset, base_rows, datasets).unwrap_or_else(|_| rowset.clone())
            }
        } else {
            Value::Null
        };
        metrics.insert(
            metric_id.clone(),
            MetricContract {
                id: metric_id.clone(),
                label: map
                    .get("label")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                purpose: None,
                shape,
                schema,
                dataset: map
                    .get("dataset")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                transforms: map
                    .get("transforms")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .map(|item| DataTransform {
                                transform_type: item
                                    .get("type")
                                    .and_then(Value::as_str)
                                    .unwrap_or("legacy")
                                    .to_string(),
                                config: item.clone(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                value,
            },
        );
    }
    Ok(metrics)
}
