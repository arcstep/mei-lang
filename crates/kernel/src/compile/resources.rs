use std::{collections::BTreeMap, path::Path};

use anyhow::{anyhow, Context, Result};
use csv::StringRecord;
use serde_json::{json, Value};

use crate::model::{DatasetView, LoadedResource, ResourceDecl, SourceDecl};

use super::analysis::schema::infer_schema_from_rows;
const DEFAULT_PREVIEW_ROWS: usize = 1000;
const DEFAULT_PAGE_SIZE: usize = 20;
const DEFAULT_MAX_PAGE_SIZE: usize = 1000;

pub(super) fn load_resources(
    app_root: &Path,
    resources: &[ResourceDecl],
) -> Result<Vec<LoadedResource>> {
    resources
        .iter()
        .map(|resource| load_resource(app_root, resource))
        .collect()
}

fn load_resource(app_root: &Path, resource: &ResourceDecl) -> Result<LoadedResource> {
    match resource.kind.as_str() {
        "document" => {
            let document = match (&resource.content, &resource.source) {
                (Some(content), _) => Some(content.clone()),
                (_, Some(source)) if source.kind == "markdown" => {
                    Some(load_markdown_content(app_root, source)?)
                }
                _ => None,
            };
            Ok(LoadedResource {
                id: resource.id.clone(),
                kind: resource.kind.clone(),
                title: resource.title.clone(),
                document,
                dataset: None,
            })
        }
        "dataset" => Ok(LoadedResource {
            id: resource.id.clone(),
            kind: resource.kind.clone(),
            title: resource.title.clone(),
            document: None,
            dataset: Some(load_dataset_view(app_root, resource)?),
        }),
        _ => Ok(LoadedResource {
            id: resource.id.clone(),
            kind: resource.kind.clone(),
            title: resource.title.clone(),
            document: resource.content.clone(),
            dataset: None,
        }),
    }
}

fn load_markdown_content(app_root: &Path, source: &SourceDecl) -> Result<String> {
    let path = app_root.join(&source.path);
    std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read markdown resource {}", path.display()))
}

fn load_dataset_view(app_root: &Path, resource: &ResourceDecl) -> Result<DatasetView> {
    let source = resource
        .source
        .as_ref()
        .ok_or_else(|| anyhow!("dataset resource `{}` missing source", resource.id))?;
    let path = app_root.join(&source.path);
    let mut reader = csv::Reader::from_path(&path)
        .with_context(|| format!("failed to open dataset {}", path.display()))?;
    let headers = reader
        .headers()
        .context("failed to read csv headers")?
        .clone();
    let columns = headers
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    let mut truncated = false;
    for record in reader.records() {
        let record = record.context("failed to read csv row")?;
        if rows.len() >= DEFAULT_PREVIEW_ROWS {
            truncated = true;
            break;
        }
        rows.push(csv_record_to_json(&headers, &record));
    }
    let source_meta = serde_json::json!({
        "lazy": {
            "preview_rows": DEFAULT_PREVIEW_ROWS,
            "default_page_size": DEFAULT_PAGE_SIZE,
            "max_page_size": DEFAULT_MAX_PAGE_SIZE,
            "truncated": truncated,
        },
        "header_row": 1,
        "normalize": {},
    });
    let source_with_meta = SourceDecl {
        kind: source.kind.clone(),
        path: source.path.clone(),
        sheet: source.sheet.clone(),
        header_row: source.header_row,
        preview_rows: source.preview_rows,
        page_size: source.page_size,
        max_page_size: source.max_page_size,
        table: source.table.clone(),
        query: source.query.clone(),
        connection: source.connection.clone(),
        content: serde_json::to_string(&source_meta).ok(),
    };
    Ok(DatasetView {
        id: resource.id.clone(),
        title: resource.title.clone(),
        purpose: None,
        schema: infer_schema_from_rows(&rows),
        stage_schema: Vec::new(),
        columns,
        rows,
        source: source_with_meta,
        sources: Vec::new(),
        metrics: BTreeMap::new(),
        runtime_metric_defs: BTreeMap::new(),
    })
}

pub(super) fn csv_record_to_json(headers: &StringRecord, record: &StringRecord) -> Value {
    let mut out = BTreeMap::new();
    for (idx, header) in headers.iter().enumerate() {
        let value = record.get(idx).unwrap_or_default();
        out.insert(header.to_string(), Value::String(value.to_string()));
    }
    json!(out)
}
