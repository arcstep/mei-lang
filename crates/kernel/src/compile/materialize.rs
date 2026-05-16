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
        DatasetViewDecl, LegacyDatasetDecl, LegacyMetricPackDecl, LegacySourceDecl, MetricDecl,
    },
    loaders::load_legacy_xlsx_rows,
    resources::csv_record_to_json,
};

/// `source_mei_rel`：定义该批 legacy dataset 的 `.mei` 相对路径（如 `data/dataset/foo.mei`）。
/// 当 `ds.dataset` 未写 `id`/`key` 时，数据集会注册为 `__source_path__`，与 `ds.data_ref("...同一路径...")`
/// 对齐需要把同一份 `DatasetView` 再挂到该路径 id 上。
pub(super) fn materialize_legacy_datasets(
    app_root: &Path,
    resources: &[LoadedResource],
    decls: &[LegacyDatasetDecl],
    source_mei_rel: Option<&str>,
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
        let mut rows = if decl.dataset.kind == "dataframe" {
            load_legacy_rows_from_source(app_root, &decl.source)?
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
            source: SourceDecl {
                kind: "derived".to_string(),
                path: format!("legacy.dataset:{dataset_id}"),
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
        };
        datasets.insert(dataset_id.clone(), dataset_stub.clone());
        if dataset_id == "__source_path__" {
            if let Some(rel) = source_mei_rel.map(str::trim).filter(|s| !s.is_empty()) {
                datasets.insert(rel.to_string(), dataset_stub);
            }
        }
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
            source: SourceDecl {
                kind: "derived".to_string(),
                path: format!("legacy.dataset:{dataset_id}"),
                content: None,
            },
            sources: Vec::new(),
            metrics,
        };
        datasets.insert(dataset_id.clone(), dataset.clone());
        if dataset_id == "__source_path__" {
            if let Some(rel) = source_mei_rel.map(str::trim).filter(|s| !s.is_empty()) {
                datasets.insert(rel.to_string(), dataset.clone());
            }
        }
        compiled.push(LoadedResource {
            id: dataset_id.clone(),
            kind: "dataset".to_string(),
            title: decl.title.clone(),
            document: None,
            dataset: Some(dataset.clone()),
        });
        if dataset_id == "__source_path__" {
            if let Some(rel) = source_mei_rel.map(str::trim).filter(|s| !s.is_empty()) {
                compiled.push(LoadedResource {
                    id: rel.to_string(),
                    kind: "dataset".to_string(),
                    title: decl.title.clone(),
                    document: None,
                    dataset: Some(dataset),
                });
            }
        }
    }
    Ok(compiled)
}

fn load_legacy_rows_from_source(app_root: &Path, source: &LegacySourceDecl) -> Result<Vec<Value>> {
    let source_path = source
        .file
        .as_deref()
        .or(source.path.as_deref())
        .unwrap_or("");
    if source_path.is_empty() {
        return Ok(Vec::new());
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
            reader
                .records()
                .map(|record| {
                    let record = record.context("failed to read csv row")?;
                    Ok::<_, anyhow::Error>(csv_record_to_json(&headers, &record))
                })
                .collect::<Result<Vec<_>>>()
        }
        "json" => {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read json dataset {}", path.display()))?;
            let json: Value = serde_json::from_str(&raw)
                .with_context(|| format!("invalid json dataset {}", path.display()))?;
            Ok(json.as_array().cloned().unwrap_or_default())
        }
        "xlsx" => {
            let header_row = source.header_row.unwrap_or(1).max(1) as usize;
            load_legacy_xlsx_rows(&path, source.sheet.as_deref(), header_row)
                .with_context(|| format!("failed to read xlsx dataset {}", path.display()))
        }
        other => Err(anyhow!("unsupported legacy dataset source kind `{other}`")),
    }
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
                content: None,
            },
            sources: Vec::new(),
            metrics,
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

pub(super) fn materialize_dataset_views(
    resources: &[LoadedResource],
    decls: &[DatasetViewDecl],
) -> Result<Vec<LoadedResource>> {
    let mut datasets = BTreeMap::<String, DatasetView>::new();
    for resource in resources {
        if let Some(dataset) = &resource.dataset {
            datasets.insert(resource.id.clone(), dataset.clone());
        }
    }

    let mut compiled = Vec::new();
    for decl in decls {
        if decl.kind != "dataset_view" {
            continue;
        }
        let rows = match &decl.rowset {
            Some(rowset) => eval_rowset(rowset, &datasets)
                .with_context(|| format!("failed to materialize rowset for `{}`", decl.id))?,
            None => datasets
                .get(&decl.id)
                .map(|dataset| dataset.rows.clone())
                .unwrap_or_default(),
        };
        let schema = if decl.schema.is_empty() {
            infer_schema_from_rows(&rows)
        } else {
            decl.schema.clone()
        };
        let columns = if schema.is_empty() {
            infer_columns(&rows)
        } else {
            schema.iter().map(|column| column.name.clone()).collect()
        };
        let metrics = materialize_metrics(&decl.metrics, &rows, &datasets)
            .with_context(|| format!("failed to compile metrics for `{}`", decl.id))?;
        let dataset = DatasetView {
            id: decl.id.clone(),
            title: decl.title.clone(),
            purpose: None,
            schema,
            stage_schema: Vec::new(),
            columns,
            rows,
            source: SourceDecl {
                kind: "derived".to_string(),
                path: format!("dataset_view:{}", decl.id),
                content: None,
            },
            sources: Vec::new(),
            metrics,
        };
        datasets.insert(decl.id.clone(), dataset.clone());
        compiled.push(LoadedResource {
            id: decl.id.clone(),
            kind: "dataset".to_string(),
            title: decl.title.clone(),
            document: None,
            dataset: Some(dataset),
        });
    }
    Ok(compiled)
}

fn materialize_metrics(
    decls: &[MetricDecl],
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<BTreeMap<String, MetricContract>> {
    let mut metrics = BTreeMap::new();
    for decl in decls {
        if decl.kind != "metric" {
            continue;
        }
        let (shape, schema, value) = match decl.metric_type.as_str() {
            "scalar_map" => {
                let mut values = serde_json::Map::new();
                for (key, expr) in &decl.values {
                    values.insert(
                        key.clone(),
                        eval_scalar_value(expr, base_rows, datasets)
                            .with_context(|| format!("metric `{}` field `{key}`", decl.id))?,
                    );
                }
                let schema = if decl.schema.is_empty() {
                    values
                        .keys()
                        .map(|key| ColumnSchema {
                            name: key.clone(),
                            type_name: "number".to_string(),
                            source: None,
                            optional: false,
                            unit: None,
                        })
                        .collect()
                } else {
                    decl.schema.clone()
                };
                (MetricShape::Scalar, schema, Value::Object(values))
            }
            "dataframe" | "series" | "table" => {
                let rows = match &decl.value {
                    Some(expr) => eval_rowset(expr, datasets)?,
                    None => base_rows.to_vec(),
                };
                let shape = match decl.metric_type.as_str() {
                    "series" => MetricShape::Series,
                    "table" => MetricShape::Table,
                    _ => MetricShape::Dataframe,
                };
                let schema = if decl.schema.is_empty() {
                    infer_schema_from_rows(&rows)
                } else {
                    decl.schema.clone()
                };
                (shape, schema, Value::Array(rows))
            }
            other => {
                return Err(anyhow!(
                    "unsupported metric_type `{other}` for metric `{}`",
                    decl.id
                ));
            }
        };
        metrics.insert(
            decl.id.clone(),
            MetricContract {
                id: decl.id.clone(),
                label: decl.label.clone(),
                purpose: None,
                shape,
                schema,
                dataset: None,
                transforms: Vec::new(),
                value,
            },
        );
    }
    Ok(metrics)
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
