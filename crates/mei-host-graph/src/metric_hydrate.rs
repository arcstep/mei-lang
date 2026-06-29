use std::collections::BTreeMap;
use std::path::Path;

use mei_host_core::load_app_config;
use mei_lang_kernel::{
    build_runtime_metric_artifacts, ops_source_entry_to_decl, ColumnSchema, DatasetView,
    LoadedResource, OpsSourceEntry, SourceDecl,
};
use serde_json::Value;

pub fn load_metric_resources_hydrated(
    app_root: &Path,
    registry: &crate::mcg::registry::McgRegistry,
) -> anyhow::Result<Vec<LoadedResource>> {
    let ops_sources = load_app_ops_sources(app_root)?;
    let mut resources = Vec::new();
    for node in registry.nodes_of_kind(crate::types::GraphNodeKind::MetricDefBundle) {
        let Some(owner) = node.owner_resource_id.clone() else {
            continue;
        };
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = crate::import::load_block_artifact(app_root, pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let bundle_rel = owner
            .strip_prefix("__world_metrics__::")
            .unwrap_or(owner.as_str());
        let bundle_source_path = app_root.join("src/data").join(bundle_rel);
        let bundle_constants = std::fs::read_to_string(bundle_source_path.as_path())
            .map(|content| crate::v2_bundle_constants::parse_bundle_constants_from_source(&content))
            .unwrap_or_default();
        let raw_runtime_metric_defs = {
            let bundle_datasets = payload
                .get("datasets")
                .and_then(Value::as_array)
                .map(|items| items.as_slice())
                .unwrap_or(&[]);
            let ctx =
                crate::v2_metric_lower::V2MetricLowerContext::from_bundle_datasets(bundle_datasets);
            let raw_defs = extract_runtime_metric_defs(&payload);
            let resolved_defs = raw_defs
                .into_iter()
                .map(|(id, metric)| {
                    (
                        id,
                        crate::v2_bundle_constants::resolve_v2_constants(
                            &metric,
                            &bundle_constants,
                        ),
                    )
                })
                .collect();
            crate::v2_metric_lower::lower_v2_runtime_metric_defs(resolved_defs, &ctx)
        };
        let (runtime_metric_defs, runtime_analysis_graph, runtime_analysis_contracts) =
            build_runtime_metric_artifacts(&raw_runtime_metric_defs, owner.as_str());
        resources.push(LoadedResource {
            id: owner.clone(),
            kind: "world_metrics".to_string(),
            title: None,
            document: None,
            dataset: Some(build_owner_dataset_view(
                &payload,
                runtime_metric_defs,
                runtime_analysis_graph,
                runtime_analysis_contracts,
            )),
        });
        for dataset in extract_bundle_datasets(&payload, &ops_sources) {
            if resources.iter().any(|resource| resource.id == dataset.id) {
                continue;
            }
            resources.push(LoadedResource {
                id: dataset.id.clone(),
                kind: "dataset".to_string(),
                title: None,
                document: None,
                dataset: Some(dataset),
            });
        }
    }
    Ok(resources)
}

fn load_app_ops_sources(app_root: &Path) -> anyhow::Result<BTreeMap<String, OpsSourceEntry>> {
    let config = load_app_config(app_root)?;
    let mut sources = BTreeMap::new();
    for (key, value) in config.ops.sources {
        if let Ok(entry) = serde_json::from_value::<OpsSourceEntry>(value) {
            sources.insert(key, entry);
        }
    }
    Ok(sources)
}

fn build_owner_dataset_view(
    payload: &Value,
    runtime_metric_defs: BTreeMap<String, Value>,
    runtime_analysis_graph: mei_lang_kernel::AnalysisGraph,
    runtime_analysis_contracts: BTreeMap<String, Value>,
) -> DatasetView {
    DatasetView {
        id: payload
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("metrics")
            .to_string(),
        title: None,
        purpose: None,
        schema: Vec::new(),
        stage_schema: Vec::new(),
        columns: Vec::new(),
        rows: Vec::new(),
        source: SourceDecl {
            kind: "world_metrics".to_string(),
            path: String::new(),
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
        metrics: BTreeMap::new(),
        runtime_metric_defs,
        runtime_analysis_graph,
        runtime_analysis_contracts,
    }
}

fn extract_bundle_datasets(
    payload: &Value,
    ops_sources: &BTreeMap<String, OpsSourceEntry>,
) -> Vec<DatasetView> {
    let Some(array) = payload.get("datasets").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut datasets = Vec::new();
    for item in array {
        let Some(view) = lower_bundle_dataset(item, ops_sources) else {
            continue;
        };
        datasets.push(view);
    }
    datasets
}

fn lower_bundle_dataset(
    value: &Value,
    ops_sources: &BTreeMap<String, OpsSourceEntry>,
) -> Option<DatasetView> {
    let args = v2_call_args(value)?;
    let id = args.get("id").and_then(Value::as_str)?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let source = enrich_file_backed_source(
        args.get("source")
            .and_then(|source| resolve_bundle_source(source, ops_sources))
            .unwrap_or_else(empty_source_decl),
    );
    let schema = lower_bundle_schema(args.get("schema"));
    Some(DatasetView {
        id,
        title: None,
        purpose: None,
        schema,
        stage_schema: Vec::new(),
        columns: Vec::new(),
        rows: Vec::new(),
        source,
        sources: Vec::new(),
        metrics: BTreeMap::new(),
        runtime_metric_defs: BTreeMap::new(),
        runtime_analysis_graph: Default::default(),
        runtime_analysis_contracts: Default::default(),
    })
}

fn resolve_bundle_source(
    value: &Value,
    ops_sources: &BTreeMap<String, OpsSourceEntry>,
) -> Option<SourceDecl> {
    if let Some(args) = value.get("__args").and_then(Value::as_object) {
        if value.get("__ref").and_then(Value::as_str) == Some("source_ref") {
            let key = args.get("arg0").and_then(Value::as_str)?;
            let entry = ops_sources.get(key)?;
            return Some(enrich_file_backed_source(ops_source_entry_to_decl(entry)));
        }
    }
    None
}

fn lower_bundle_schema(value: Option<&Value>) -> Vec<ColumnSchema> {
    let Some(array) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut schema = Vec::new();
    for item in array {
        let Some(args) = v2_call_args(item) else {
            continue;
        };
        let name = args
            .get("arg0")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        schema.push(ColumnSchema {
            name: name.clone(),
            type_name: args
                .get("arg1")
                .and_then(Value::as_str)
                .unwrap_or("string")
                .to_string(),
            source: args
                .get("source")
                .and_then(Value::as_str)
                .map(str::to_string),
            optional: args
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            unit: None,
        });
    }
    schema
}

fn extract_runtime_metric_defs(payload: &Value) -> BTreeMap<String, Value> {
    let mut defs = BTreeMap::new();
    if let Some(metrics) = payload.get("metrics").and_then(Value::as_array) {
        for metric in metrics {
            if metric.get("__call").is_some() {
                if let Some(args) = metric.get("__args").and_then(Value::as_object) {
                    if let Some(id) = args.get("id").and_then(Value::as_str) {
                        defs.insert(id.to_string(), metric.clone());
                    }
                }
            } else if let Some(id) = metric.get("id").and_then(Value::as_str) {
                defs.insert(id.to_string(), metric.clone());
            }
        }
    }
    defs
}

fn v2_call_args(value: &Value) -> Option<&Value> {
    value.get("__args")
}

fn empty_source_decl() -> SourceDecl {
    SourceDecl {
        kind: String::new(),
        path: String::new(),
        sheet: None,
        header_row: None,
        preview_rows: None,
        page_size: None,
        max_page_size: None,
        table: None,
        query: None,
        connection: None,
        content: None,
    }
}

fn enrich_file_backed_source(source: SourceDecl) -> SourceDecl {
    let path = source.path.trim();
    if path.is_empty() {
        return source;
    }
    if source
        .content
        .as_deref()
        .is_some_and(|content| !content.trim().is_empty())
    {
        return source;
    }
    let kind = source.kind.trim().to_ascii_lowercase();
    if !matches!(kind.as_str(), "xlsx" | "xls" | "csv" | "json" | "geojson") {
        return source;
    }
    let meta = serde_json::json!({
        "lazy": {
            "default_page_size": source.page_size.unwrap_or(20),
            "max_page_size": source.max_page_size.unwrap_or(1000),
        },
        "sheet": source.sheet,
        "header_row": source.header_row.unwrap_or(1),
        "table": source.table,
        "query": source.query,
        "connection": source.connection,
    });
    SourceDecl {
        content: serde_json::to_string(&meta).ok(),
        ..source
    }
}
