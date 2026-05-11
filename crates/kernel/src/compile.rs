use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Context, Result};
use csv::StringRecord;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    eval::evaluate_mei_file,
    model::{
        AppDecl, ColumnSchema, CompiledApp, ComponentAsset, DatasetView, Diagnostic, EntryDecl,
        FlowDecl, FrameDecl, LoadedResource, MetricContract, MetricShape, PanelDecl, ResourceDecl,
        SceneContract, SceneDecl, Severity, SourceDecl,
    },
    workspace::{load_component_assets, source_tree},
};

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    let app_root = source_root.join(app_id);
    compile_app_from_root(source_root, &app_root)
}

pub fn compile_app_from_root(source_root: &Path, app_root: &Path) -> Result<CompiledApp> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let (entry_target, entry_decls) =
        resolve_scene_source(app_root, &app_main, &app_decl, &app_decls, &mut diagnostics)?;

    let mut frame: Option<FrameDecl> = None;
    let mut scene: Option<SceneDecl> = None;
    let mut world: Option<crate::model::WorldDecl> = None;
    let mut flow: Option<FlowDecl> = None;
    let mut panels: Vec<PanelDecl> = Vec::new();
    let mut dataset_views: Vec<DatasetViewDecl> = Vec::new();

    if let Some(values) = entry_decls.as_array() {
        for value in values {
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                continue;
            };
            match kind {
                "frame" => frame = Some(serde_json::from_value(value.clone())?),
                "scene" => scene = Some(serde_json::from_value(value.clone())?),
                "world" => world = Some(serde_json::from_value(value.clone())?),
                "flow" => flow = Some(serde_json::from_value(value.clone())?),
                "panel" => panels.push(serde_json::from_value(value.clone())?),
                "dataset_view" => match serde_json::from_value::<DatasetViewDecl>(value.clone()) {
                    Ok(decl) => dataset_views.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_dataset_view_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(entry_target.clone()),
                    }),
                },
                "app" | "app_scene_ref" => {}
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(entry_target.clone()),
                }),
            }
        }
    }

    let asset_map = load_component_assets(source_root)?;
    let mut asset_keys = BTreeSet::new();
    for panel in &panels {
        for block in &panel.blocks {
            asset_keys.insert(block.use_key.clone());
        }
    }
    let component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect::<Vec<ComponentAsset>>();

    if scene.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "entry file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(entry_target.clone()),
        });
    }

    if frame.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "missing_frame".to_string(),
            message: "scene entry should declare frame(...) to define UI layout".to_string(),
            source_path: Some(entry_target.clone()),
        });
    }

    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    let mut resources = match world.as_ref() {
        Some(world_decl) => load_resources(app_root, &world_decl.resources)?,
        None => Vec::new(),
    };
    if !dataset_views.is_empty() {
        let derived = materialize_dataset_views(&resources, &dataset_views)?;
        for resource in derived {
            if let Some(index) = resources.iter().position(|item| item.id == resource.id) {
                resources[index] = resource;
            } else {
                resources.push(resource);
            }
        }
    }

    let scene_contract = scene.map(|scene_decl| SceneContract {
        scene: scene_decl,
        world,
        flow,
        frame: frame.clone(),
        panels,
    });

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        entry_target,
        file_tree: source_tree(app_root)?,
        scene_contract,
        resources,
        component_assets,
        diagnostics,
    })
}

fn decode_app_decl(path: &Path, raw: &Value) -> (Option<AppDecl>, Vec<Diagnostic>) {
    let mut app_decl = None;
    let mut diagnostics = Vec::new();
    if let Some(values) = raw.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("app") {
                match serde_json::from_value::<AppDecl>(value.clone()) {
                    Ok(decl) => app_decl = Some(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_app_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(path.to_string_lossy().to_string()),
                    }),
                }
            }
        }
    }
    (app_decl, diagnostics)
}

#[derive(Debug, Clone, Deserialize)]
struct SceneFileRefDecl {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DatasetViewDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub rowset: Option<Value>,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub metrics: Vec<MetricDecl>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricDecl {
    pub kind: String,
    pub metric_type: String,
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    #[serde(default)]
    pub value: Option<Value>,
}

fn resolve_scene_source(
    app_root: &Path,
    app_main: &Path,
    app_decl: &AppDecl,
    app_decls: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(String, Value)> {
    if let Some(entry) = app_decl.entries.first().cloned() {
        let entry_target = entry_source(&entry).unwrap_or_else(|| "main.mei".to_string());
        let entry_path = app_root.join(&entry_target);
        return Ok((entry_target, evaluate_mei_file(&entry_path)?));
    }

    if let Some(default_scene) = app_decl.default_scene.as_deref() {
        if let Some(target) = resolve_default_scene_target(app_decls, default_scene) {
            if target == "main.mei" {
                return Ok((target, app_decls.clone()));
            }
            let entry_path = app_root.join(&target);
            return Ok((target, evaluate_mei_file(&entry_path)?));
        }

        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_default_scene".to_string(),
            message: format!(
                "default_scene `{default_scene}` did not match an inline scene or app.add_scene(scene_file_ref(...))"
            ),
            source_path: Some(app_main.to_string_lossy().to_string()),
        });
    }

    if has_inline_scene(app_decls, None) {
        return Ok(("main.mei".to_string(), app_decls.clone()));
    }

    if let Some(target) = first_scene_file_ref_target(app_decls) {
        let entry_path = app_root.join(&target);
        return Ok((target, evaluate_mei_file(&entry_path)?));
    }

    Ok(("main.mei".to_string(), app_decls.clone()))
}

fn resolve_default_scene_target(raw: &Value, default_scene: &str) -> Option<String> {
    if has_inline_scene(raw, Some(default_scene)) {
        return Some("main.mei".to_string());
    }

    scene_file_ref_target(raw, default_scene)
}

fn has_inline_scene(raw: &Value, scene_id: Option<&str>) -> bool {
    raw.as_array().is_some_and(|values| {
        values.iter().any(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("scene") {
                return false;
            }
            match scene_id {
                Some(expected) => value.get("id").and_then(Value::as_str) == Some(expected),
                None => true,
            }
        })
    })
}

fn scene_file_ref_target(raw: &Value, scene_id: &str) -> Option<String> {
    raw.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
                return None;
            }
            let scene_ref = serde_json::from_value::<SceneFileRefDecl>(
                value.get("scene").cloned().unwrap_or(Value::Null),
            )
            .ok()?;
            if scene_ref.kind != "scene_file_ref" {
                return None;
            }
            if scene_ref.id.as_deref() == Some(scene_id)
                || (scene_ref.id.is_none() && scene_name_from_path(&scene_ref.path) == scene_id)
            {
                return Some(scene_ref.path);
            }
            None
        })
    })
}

fn first_scene_file_ref_target(raw: &Value) -> Option<String> {
    raw.as_array().and_then(|values| {
        values.iter().find_map(|value| {
            if value.get("kind").and_then(Value::as_str) != Some("app_scene_ref") {
                return None;
            }
            let scene_ref = serde_json::from_value::<SceneFileRefDecl>(
                value.get("scene").cloned().unwrap_or(Value::Null),
            )
            .ok()?;
            if scene_ref.kind == "scene_file_ref" {
                Some(scene_ref.path)
            } else {
                None
            }
        })
    })
}

fn scene_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn entry_source(entry: &EntryDecl) -> Option<String> {
    entry
        .scene
        .clone()
        .or_else(|| entry.frame.clone())
}

fn load_resources(app_root: &Path, resources: &[ResourceDecl]) -> Result<Vec<LoadedResource>> {
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
    let columns = headers.iter().map(|value| value.to_string()).collect::<Vec<_>>();
    let rows = reader
        .records()
        .map(|record| {
            let record = record.context("failed to read csv row")?;
            Ok::<_, anyhow::Error>(csv_record_to_json(&headers, &record))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DatasetView {
        id: resource.id.clone(),
        title: resource.title.clone(),
        schema: infer_schema_from_rows(&rows),
        columns,
        rows,
        source: source.clone(),
        metrics: BTreeMap::new(),
    })
}

fn csv_record_to_json(headers: &StringRecord, record: &StringRecord) -> Value {
    let mut out = BTreeMap::new();
    for (idx, header) in headers.iter().enumerate() {
        let value = record.get(idx).unwrap_or_default();
        out.insert(header.to_string(), Value::String(value.to_string()));
    }
    json!(out)
}

fn materialize_dataset_views(
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
            schema,
            columns,
            rows,
            source: SourceDecl {
                kind: "derived".to_string(),
                path: format!("dataset_view:{}", decl.id),
                content: None,
            },
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
                shape,
                schema,
                value,
            },
        );
    }
    Ok(metrics)
}

fn eval_rowset(expr: &Value, datasets: &BTreeMap<String, DatasetView>) -> Result<Vec<Value>> {
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
        "group_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("group_by expression missing rowset"))?;
            let rows = eval_rowset(rowset_expr, datasets)?;
            let group_field = map
                .get("by")
                .or_else(|| map.get("field"))
                .and_then(Value::as_str)
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

fn aggregate_group_rows(
    rows: &[Value],
    group_field: &str,
    value_field: Option<&str>,
    agg: &str,
    limit: Option<usize>,
) -> Vec<Value> {
    let mut grouped = BTreeMap::<String, Vec<f64>>::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows {
        let label = row_string(row, group_field);
        *counts.entry(label.clone()).or_insert(0) += 1;
        if let Some(field) = value_field {
            if let Some(number) = row_number(row, field) {
                grouped.entry(label).or_default().push(number);
            } else {
                grouped.entry(label).or_default();
            }
        } else {
            grouped.entry(label).or_default();
        }
    }
    let mut out = Vec::new();
    for (label, numbers) in grouped {
        let value = match agg {
            "sum" => numbers.iter().sum::<f64>(),
            "avg" => {
                if numbers.is_empty() {
                    0.0
                } else {
                    numbers.iter().sum::<f64>() / numbers.len() as f64
                }
            }
            "min" => numbers.into_iter().reduce(f64::min).unwrap_or(0.0),
            "max" => numbers.into_iter().reduce(f64::max).unwrap_or(0.0),
            _ => counts.get(&label).copied().unwrap_or(0) as f64,
        };
        out.push(json!({
            "label": label,
            "value": value,
        }));
    }
    if let Some(limit) = limit {
        out.truncate(limit);
    }
    out
}

fn summarize_rows(rows: &[Value], agg: &str, value_field: &str) -> Vec<Value> {
    if agg == "count" {
        return vec![json!({ "value": rows.len() })];
    }
    let values = rows
        .iter()
        .filter_map(|row| row_number(row, value_field))
        .collect::<Vec<_>>();
    let value = match agg {
        "sum" => values.iter().sum::<f64>(),
        "avg" => {
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        }
        "min" => values.into_iter().reduce(f64::min).unwrap_or(0.0),
        "max" => values.into_iter().reduce(f64::max).unwrap_or(0.0),
        _ => 0.0,
    };
    vec![json!({ "value": value })]
}

fn select_fields(row: &Value, fields: &[String]) -> Value {
    let Some(object) = row.as_object() else {
        return row.clone();
    };
    let mut out = serde_json::Map::new();
    for field in fields {
        if let Some(value) = object.get(field) {
            out.insert(field.clone(), value.clone());
        }
    }
    Value::Object(out)
}

fn rename_fields(row: &Value, mapping: &serde_json::Map<String, Value>) -> Value {
    let Some(object) = row.as_object() else {
        return row.clone();
    };
    let mut out = serde_json::Map::new();
    for (key, value) in object {
        let renamed = mapping
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(key)
            .to_string();
        out.insert(renamed, value.clone());
    }
    Value::Object(out)
}

fn mutate_row(row: &Value, updates: &serde_json::Map<String, Value>) -> Value {
    let mut out = row.as_object().cloned().unwrap_or_default();
    for (key, expr) in updates {
        out.insert(key.clone(), eval_row_value(expr, &out));
    }
    Value::Object(out)
}

fn eval_row_value(expr: &Value, row: &serde_json::Map<String, Value>) -> Value {
    if let Some(analysis) = expr.as_object() {
        if analysis.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
            let analysis_type = analysis.get("type").and_then(Value::as_str).unwrap_or("");
            return match analysis_type {
                "lit" => analysis.get("value").cloned().unwrap_or(Value::Null),
                "col" => analysis
                    .get("field")
                    .and_then(Value::as_str)
                    .and_then(|field| row.get(field).cloned())
                    .unwrap_or(Value::Null),
                _ => expr.clone(),
            };
        }
    }
    expr.clone()
}

fn predicate_matches(row: &Value, predicate: &Value) -> bool {
    let Some(object) = predicate.as_object() else {
        return true;
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return true;
    }
    let analysis_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    match analysis_type {
        "eq" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").cloned().unwrap_or(Value::Null);
            row_value(row, field).cloned().unwrap_or(Value::Null) == expected
        }
        _ => true,
    }
}

fn eval_scalar_value(
    expr: &Value,
    base_rows: &[Value],
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<Value> {
    let Some(object) = expr.as_object() else {
        return Ok(expr.clone());
    };
    if object.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return Ok(expr.clone());
    }
    let analysis_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("analysis expression missing type"))?;
    match analysis_type {
        "count" => {
            let rows = match object.get("rowset") {
                Some(rowset) => eval_rowset(rowset, datasets)?,
                None => base_rows.to_vec(),
            };
            Ok(json!(rows.len()))
        }
        "sum" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            Ok(json!(values.iter().sum::<f64>()))
        }
        "avg" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            let value = if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            };
            Ok(json!(value))
        }
        "number" => {
            let values = eval_numeric_values(Some(expr), datasets)?;
            Ok(Value::Array(values.into_iter().map(|value| json!(value)).collect()))
        }
        "lit" => Ok(object.get("value").cloned().unwrap_or(Value::Null)),
        _ => Ok(expr.clone()),
    }
}

fn eval_numeric_values(
    expr: Option<&Value>,
    datasets: &BTreeMap<String, DatasetView>,
) -> Result<Vec<f64>> {
    let Some(expr) = expr else {
        return Ok(Vec::new());
    };
    if let Some(number) = parse_number(expr) {
        return Ok(vec![number]);
    }
    match expr {
        Value::Array(items) => Ok(items.iter().filter_map(parse_number).collect()),
        Value::Object(map) => {
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("number")
            {
                let rowset_expr = map
                    .get("rowset")
                    .ok_or_else(|| anyhow!("number expression missing rowset"))?;
                let field = map
                    .get("field")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("number expression missing field"))?;
                return Ok(eval_rowset(rowset_expr, datasets)?
                    .iter()
                    .filter_map(|row| row_number(row, field))
                    .collect());
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("lit")
            {
                return Ok(map
                    .get("value")
                    .and_then(parse_number)
                    .into_iter()
                    .collect::<Vec<_>>());
            }
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}

fn infer_columns(rows: &[Value]) -> Vec<String> {
    let mut fields = BTreeSet::new();
    for row in rows {
        if let Some(object) = row.as_object() {
            for key in object.keys() {
                fields.insert(key.clone());
            }
        }
    }
    fields.into_iter().collect()
}

fn infer_schema_from_rows(rows: &[Value]) -> Vec<ColumnSchema> {
    infer_columns(rows)
        .into_iter()
        .map(|name| ColumnSchema {
            name: name.clone(),
            type_name: infer_column_type(rows, &name),
            source: None,
            optional: false,
            unit: None,
        })
        .collect()
}

fn infer_column_type(rows: &[Value], field: &str) -> String {
    for row in rows {
        let Some(value) = row_value(row, field) else {
            continue;
        };
        return match value {
            Value::Bool(_) => "boolean".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::String(raw) => {
                if raw.parse::<f64>().is_ok() {
                    "number".to_string()
                } else {
                    "string".to_string()
                }
            }
            Value::Array(_) => "object".to_string(),
            Value::Object(_) => "object".to_string(),
            Value::Null => "string".to_string(),
        };
    }
    "string".to_string()
}

fn row_value<'a>(row: &'a Value, field: &str) -> Option<&'a Value> {
    row.as_object().and_then(|object| object.get(field))
}

fn row_string(row: &Value, field: &str) -> String {
    row_value(row, field)
        .map(|value| match value {
            Value::String(raw) => raw.clone(),
            Value::Number(raw) => raw.to_string(),
            Value::Bool(raw) => raw.to_string(),
            _ => value.to_string(),
        })
        .unwrap_or_default()
}

fn row_number(row: &Value, field: &str) -> Option<f64> {
    row_value(row, field).and_then(parse_number)
}

fn parse_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::compile_app_from_root;
    use crate::MetricShape;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mei-lang-kernel-{name}-{nonce}"))
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, content).expect("write file");
    }

    #[test]
    fn compile_supports_inline_default_scene_authoring() {
        let root = temp_root("inline-default-scene");
        let app_root = root.join("demo");
        write_file(
            &app_root.join("main.mei"),
            r#"
app(
    id = "demo",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
    ],
)
"#,
        );

        let compiled = compile_app_from_root(&root, &app_root).expect("compile inline scene app");
        assert_eq!(compiled.entry_target, "main.mei");
        let contract = compiled.scene_contract.expect("scene contract");
        assert_eq!(contract.scene.id, "home");
        assert_eq!(contract.world.expect("world").resources.len(), 1);
        assert_eq!(contract.panels.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn compile_supports_scene_file_ref_authoring() {
        let root = temp_root("scene-file-ref");
        let app_root = root.join("fire");
        write_file(
            &app_root.join("main.mei"),
            r#"
app(
    id = "fire",
    default_scene = "room_fire_click",
)

app.add_scene(
    scene_file_ref("home.mei", id = "room_fire_click"),
)
"#,
        );
        write_file(
            &app_root.join("home.mei"),
            r#"
app.add_scene(
    id = "room_fire_click",
    profile = "simulation",
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "status",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", content = "hello"),
    ],
)
"#,
        );

        let compiled = compile_app_from_root(&root, &app_root).expect("compile external scene app");
        assert_eq!(compiled.entry_target, "home.mei");
        let contract = compiled.scene_contract.expect("scene contract");
        assert_eq!(contract.scene.id, "room_fire_click");
        assert_eq!(contract.panels.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn compile_materializes_dataset_view_and_metrics() {
        let root = temp_root("dataset-view-metrics");
        let app_root = root.join("analytics");
        write_file(
            &app_root.join("main.mei"),
            r#"
app(
    id = "analytics",
    default_scene = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_world(
    resources = [
        resource(
            id = "sales_data",
            kind = "dataset",
            source = ds.csv(path = "data/sales.csv"),
        ),
    ],
)

rows = ds.data_ref("sales_data")

ds.dataset_view(
    id = "sales_metrics",
    title = "销售指标视图",
    rowset = rows,
    schema = [
        ds.column("label", "string"),
        ds.column("value", "number", unit = "元"),
        ds.column("unit", "string"),
    ],
    metrics = [
        ds.scalar_map(
            id = "overview",
            schema = [
                ds.column("total_rows", "number"),
                ds.column("total_value", "number"),
                ds.column("avg_value", "number"),
            ],
            values = {
                "total_rows": ds.count(rows),
                "total_value": ds.sum(ds.number(rows, "value")),
                "avg_value": ds.avg(ds.number(rows, "value")),
            },
        ),
        ds.dataframe(
            id = "ranking",
            schema = [
                ds.column("label", "string"),
                ds.column("value", "number"),
            ],
            value = ds.group_by(rows, by = "label", value = "value", agg = "sum"),
        ),
    ],
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "table",
    area = "auto",
    blocks = [
        component("dataset.table", area = "auto", props = {"data": ds.data_ref("sales_metrics")}),
    ],
)
"#,
        );
        write_file(
            &app_root.join("data/sales.csv"),
            "label,value,unit\nA,100,元\nB,200,元\nC,300,元\n",
        );

        let compiled = compile_app_from_root(&root, &app_root).expect("compile dataset view app");
        let view_resource = compiled
            .resources
            .iter()
            .find(|resource| resource.id == "sales_metrics")
            .expect("derived dataset view resource");
        let dataset = view_resource.dataset.as_ref().expect("dataset view payload");
        assert_eq!(dataset.rows.len(), 3);
        assert_eq!(dataset.columns, vec!["label", "value", "unit"]);
        let overview = dataset
            .metrics
            .get("overview")
            .expect("scalar metric should exist");
        assert_eq!(overview.shape, MetricShape::Scalar);
        assert!(overview.value.get("total_rows").is_some());
        let ranking = dataset
            .metrics
            .get("ranking")
            .expect("dataframe metric should exist");
        assert_eq!(ranking.shape, MetricShape::Dataframe);
        assert!(ranking.value.as_array().is_some());

        let _ = fs::remove_dir_all(&root);
    }
}
