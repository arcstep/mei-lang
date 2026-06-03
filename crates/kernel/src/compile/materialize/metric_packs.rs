use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::model::{
    ColumnSchema, DataTransform, DatasetView, LoadedResource, MetricContract, MetricShape,
    SourceDecl,
};

use super::super::{
    analysis::{
        eval_context::{EvalContext, RequestDagMetrics, RuntimeMetricEvalScope},
        rowset::eval_rowset_with_ctx,
        scalar::eval_scalar_value_with_ctx,
    },
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
        let (runtime_metric_defs, runtime_analysis_graph, runtime_analysis_contracts) =
            super::build_analysis_artifacts(&pack.metrics, &pack.metric_pack.id);
        let metrics = materialize_legacy_metric_map(&runtime_metric_defs, &[], &datasets)
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
            runtime_metric_defs,
            runtime_analysis_graph,
            runtime_analysis_contracts,
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
    materialize_legacy_metric_map_with_scope(
        decls,
        base_rows,
        datasets,
        &RuntimeMetricEvalScope::default(),
    )
}

pub(crate) fn materialize_legacy_metric_map_with_scope(
    decls: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> Result<BTreeMap<String, MetricContract>> {
    Ok(materialize_legacy_metric_map_with_scope_and_dag(decls, base_rows, datasets, scope)?.0)
}

pub(crate) fn materialize_legacy_metric_map_with_scope_and_dag(
    decls: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> Result<(BTreeMap<String, MetricContract>, RequestDagMetrics)> {
    let mut metrics = BTreeMap::new();
    let mut eval_ctx = EvalContext::with_scope(scope.clone());
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
                let resolved =
                    eval_scalar_value_with_ctx(entry_value, base_rows, datasets, &mut eval_ctx)
                        .with_context(|| {
                            format!("legacy metric `{metric_id}` field `{entry_key}`")
                        })?;
                out.insert(entry_key.clone(), resolved);
            }
            Value::Object(out)
        } else if let Some(rowset) = map
            .get("series")
            .or_else(|| map.get("list"))
            .or_else(|| map.get("value"))
        {
            if let Ok(rows) = eval_rowset_with_ctx(rowset, datasets, &mut eval_ctx) {
                Value::Array(rows)
            } else {
                eval_scalar_value_with_ctx(rowset, base_rows, datasets, &mut eval_ctx)
                    .unwrap_or_else(|_| rowset.clone())
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
                value_format: map.get("value_format").cloned(),
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
    let dag_metrics = eval_ctx.request_dag_metrics();
    Ok((metrics, dag_metrics))
}
