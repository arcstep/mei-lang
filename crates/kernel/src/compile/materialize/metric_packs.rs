use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::model::{
    ColumnSchema, DataTransform, DatasetView, LoadedResource, MetricContract, MetricShape, SourceDecl,
};

use super::super::{
    analysis::{rowset::eval_rowset, scalar::eval_scalar_value},
    decls::LegacyMetricPackDecl,
};

pub(crate) fn materialize_metric_packs(
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
pub(crate) fn materialize_legacy_metric_map(
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
                unit: map
                    .get("unit")
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
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
