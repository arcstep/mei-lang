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
        AppDecl, ColumnSchema, CompiledApp, ComponentAsset, DataTransform, DatasetView, Diagnostic,
        EntryDecl, FlowDecl, FrameDecl, LoadedResource, MetricContract, MetricShape, PanelDecl,
        ResourceDecl, SceneContract, SceneDecl, Severity, SourceDecl,
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
    let mut legacy_datasets: Vec<LegacyDatasetDecl> = Vec::new();
    let mut legacy_metric_packs: Vec<LegacyMetricPackDecl> = Vec::new();

    if let Some(values) = entry_decls.as_array() {
        for value in values {
            if value.get("dataset").is_some() && value.get("schema_version").is_some() {
                match serde_json::from_value::<LegacyDatasetDecl>(value.clone()) {
                    Ok(decl) => legacy_datasets.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_legacy_dataset_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(entry_target.clone()),
                    }),
                }
                continue;
            }
            if value.get("metric_pack").is_some() && value.get("schema_version").is_some() {
                match serde_json::from_value::<LegacyMetricPackDecl>(value.clone()) {
                    Ok(decl) => legacy_metric_packs.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_metric_pack_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(entry_target.clone()),
                    }),
                }
                continue;
            }
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
    if !legacy_datasets.is_empty() {
        let derived = materialize_legacy_datasets(app_root, &resources, &legacy_datasets)?;
        for resource in derived {
            if let Some(index) = resources.iter().position(|item| item.id == resource.id) {
                resources[index] = resource;
            } else {
                resources.push(resource);
            }
        }
    }
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
    if !legacy_metric_packs.is_empty() {
        let derived = materialize_metric_packs(&resources, &legacy_metric_packs)?;
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

#[derive(Debug, Clone, Deserialize)]
struct LegacyDatasetNodeDecl {
    pub key: String,
    pub kind: String,
    #[serde(default)]
    pub columns: Vec<ColumnSchema>,
    #[serde(default)]
    pub normalize: BTreeMap<String, String>,
    #[serde(default)]
    pub rowset: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct LegacySourceDecl {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyDatasetDecl {
    #[serde(default)]
    pub data_ref: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub source: LegacySourceDecl,
    pub dataset: LegacyDatasetNodeDecl,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyMetricPackDecl {
    pub metric_pack: LegacyMetricPackMetaDecl,
    #[serde(default)]
    pub metrics: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyMetricPackMetaDecl {
    pub id: String,
    #[serde(default)]
    pub purpose: Option<String>,
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
        purpose: None,
        schema: infer_schema_from_rows(&rows),
        stage_schema: Vec::new(),
        columns,
        rows,
        source: source.clone(),
        sources: Vec::new(),
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

fn materialize_legacy_datasets(
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
        compiled.push(LoadedResource {
            id: dataset_id,
            kind: "dataset".to_string(),
            title: decl.title.clone(),
            document: None,
            dataset: Some(dataset),
        });
    }
    Ok(compiled)
}

fn load_legacy_rows_from_source(app_root: &Path, source: &LegacySourceDecl) -> Result<Vec<Value>> {
    let source_kind = source.kind.as_deref().unwrap_or("csv");
    let source_path = source.file.as_deref().or(source.path.as_deref()).unwrap_or("");
    if source_path.is_empty() {
        return Ok(Vec::new());
    }
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
        other => Err(anyhow!("unsupported legacy dataset source kind `{other}`")),
    }
}

fn apply_legacy_normalize(
    rows: Vec<Value>,
    normalize: &BTreeMap<String, String>,
) -> Vec<Value> {
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

fn materialize_metric_packs(
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
        let shape_name = map
            .get("shape")
            .and_then(Value::as_str)
            .unwrap_or_else(|| if map.get("values").is_some() { "scalar_map" } else { "dataframe" });
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
        } else if let Some(rowset) = map.get("series").or_else(|| map.get("list")).or_else(|| map.get("value")) {
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
                label: map.get("label").and_then(Value::as_str).map(ToString::to_string),
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
        "rows" => {
            let dataset_id = map
                .get("dataset")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("rows expression missing dataset"))?;
            let normalized = dataset_id
                .strip_prefix("dataset.")
                .unwrap_or(dataset_id)
                .to_string();
            let dataset = datasets
                .get(&normalized)
                .or_else(|| datasets.get(dataset_id))
                .ok_or_else(|| anyhow!("unknown dataset `{dataset_id}`"))?;
            Ok(dataset.rows.clone())
        }
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
        "sort_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("sort_by expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("sort_by expression missing field"))?;
            let order = map.get("order").and_then(Value::as_str).unwrap_or("asc");
            let mut rows = eval_rowset(rowset_expr, datasets)?;
            sort_rows_by_field(&mut rows, field, order);
            Ok(rows)
        }
        "reorder" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("reorder expression missing rowset"))?;
            let fields = map
                .get("fields")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("reorder expression missing fields"))?
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            Ok(eval_rowset(rowset_expr, datasets)?
                .into_iter()
                .map(|row| reorder_fields(&row, &fields))
                .collect())
        }
        "stage" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("stage expression missing rowset"))?;
            Ok(eval_rowset(rowset_expr, datasets)?)
        }
        "first_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("first_by expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("first_by expression missing field"))?;
            let rows = eval_rowset(rowset_expr, datasets)?;
            Ok(first_rows_by_field(&rows, field))
        }
        "distinct_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("distinct_by expression missing rowset"))?;
            let fields = map
                .get("fields")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("distinct_by expression missing fields"))?
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let rows = eval_rowset(rowset_expr, datasets)?;
            Ok(distinct_rows_by_fields(&rows, &fields))
        }
        "group_by" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("group_by expression missing rowset"))?;
            let rows = eval_rowset(rowset_expr, datasets)?;
            let group_field = map
                .get("by")
                .and_then(Value::as_str)
                .or_else(|| {
                    map.get("fields")
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(Value::as_str)
                })
                .or_else(|| map.get("field").and_then(Value::as_str))
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
                .or_else(|| map.get("grouped"))
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
                .or_else(|| map.get("date_field"))
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
        "table_rows" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("table_rows expression missing rowset"))?;
            Ok(eval_rowset(rowset_expr, datasets)?)
        }
        "split_text" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("split_text expression missing rowset"))?;
            let field = map
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("split_text expression missing field"))?;
            let delimiter = map.get("delimiter").and_then(Value::as_str).unwrap_or("、");
            let mut out = Vec::new();
            for row in eval_rowset(rowset_expr, datasets)? {
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
        "lookup_value" => {
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
            for row in eval_rowset(lookup_rowset_expr, datasets)? {
                let key = row_string(&row, lookup_field);
                let value = row_value(&row, value_field).cloned().unwrap_or(Value::Null);
                lookup.insert(key, value);
            }
            let mut out = Vec::new();
            for row in eval_rowset(rowset_expr, datasets)? {
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
        "latest_days" | "latest_months" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("{analysis_type} expression missing rowset"))?;
            let mut rows = eval_rowset(rowset_expr, datasets)?;
            let limit = if analysis_type == "latest_days" {
                map.get("days").and_then(Value::as_u64).unwrap_or(rows.len() as u64) as usize
            } else {
                map.get("months").and_then(Value::as_u64).unwrap_or(rows.len() as u64) as usize
            };
            if rows.len() > limit {
                rows = rows.split_off(rows.len() - limit);
            }
            Ok(rows)
        }
        "bucket_date" => {
            let rowset_expr = map
                .get("rowset")
                .ok_or_else(|| anyhow!("bucket_date expression missing rowset"))?;
            Ok(eval_rowset(rowset_expr, datasets)?)
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

fn reorder_fields(row: &Value, fields: &[String]) -> Value {
    let Some(object) = row.as_object() else {
        return row.clone();
    };
    let mut out = serde_json::Map::new();
    for field in fields {
        if let Some(value) = object.get(field) {
            out.insert(field.clone(), value.clone());
        }
    }
    for (key, value) in object {
        if !out.contains_key(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    Value::Object(out)
}

fn sort_rows_by_field(rows: &mut [Value], field: &str, order: &str) {
    rows.sort_by(|left, right| {
        let l = row_value(left, field).cloned().unwrap_or(Value::Null);
        let r = row_value(right, field).cloned().unwrap_or(Value::Null);
        compare_json_values(&l, &r)
    });
    if order.eq_ignore_ascii_case("desc") {
        rows.reverse();
    }
}

fn compare_json_values(left: &Value, right: &Value) -> std::cmp::Ordering {
    if let (Some(l), Some(r)) = (parse_number(left), parse_number(right)) {
        return l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal);
    }
    left.to_string().cmp(&right.to_string())
}

fn first_rows_by_field(rows: &[Value], field: &str) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for row in rows {
        let key = row_string(row, field);
        if seen.insert(key) {
            out.push(row.clone());
        }
    }
    out
}

fn distinct_rows_by_fields(rows: &[Value], fields: &[String]) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for row in rows {
        let key = fields
            .iter()
            .map(|field| row_value(row, field).cloned().unwrap_or(Value::Null).to_string())
            .collect::<Vec<_>>()
            .join("\u{1f}");
        if seen.insert(key) {
            out.push(row.clone());
        }
    }
    out
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
                "number" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    row.get(field)
                        .and_then(parse_number)
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null)
                }
                "text" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    row.get(field)
                        .map(|value| Value::String(value.as_str().unwrap_or(&value.to_string()).to_string()))
                        .unwrap_or(Value::Null)
                }
                "extract_number" => {
                    let field = analysis.get("field").and_then(Value::as_str).unwrap_or("");
                    let text = row.get(field).and_then(Value::as_str).unwrap_or_default();
                    let extracted = text
                        .chars()
                        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
                        .collect::<String>();
                    extracted
                        .parse::<f64>()
                        .ok()
                        .map(|value| json!(value))
                        .unwrap_or(Value::Null)
                }
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
        "ne" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").cloned().unwrap_or(Value::Null);
            row_value(row, field).cloned().unwrap_or(Value::Null) != expected
        }
        "gt" | "gte" | "lt" | "lte" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object
                .get("value")
                .and_then(parse_number)
                .unwrap_or(f64::NAN);
            let actual = row_value(row, field).and_then(parse_number).unwrap_or(f64::NAN);
            match analysis_type {
                "gt" => actual > expected,
                "gte" => actual >= expected,
                "lt" => actual < expected,
                _ => actual <= expected,
            }
        }
        "between" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let lower = object.get("lower").and_then(parse_number).unwrap_or(f64::MIN);
            let upper = object.get("upper").and_then(parse_number).unwrap_or(f64::MAX);
            let actual = row_value(row, field).and_then(parse_number).unwrap_or(f64::NAN);
            actual >= lower && actual <= upper
        }
        "in_values" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let actual = row_value(row, field).cloned().unwrap_or(Value::Null);
            object
                .get("values")
                .and_then(Value::as_array)
                .map(|items| items.iter().any(|item| item == &actual))
                .unwrap_or(false)
        }
        "not_empty" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            !row_string(row, field).trim().is_empty()
        }
        "contains" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("value").and_then(Value::as_str).unwrap_or("");
            row_string(row, field).contains(expected)
        }
        "matches" => {
            let field = object.get("field").and_then(Value::as_str).unwrap_or("");
            let expected = object.get("pattern").and_then(Value::as_str).unwrap_or("");
            row_string(row, field).contains(expected)
        }
        "and" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| items.iter().all(|item| predicate_matches(row, item)))
            .unwrap_or(true),
        "or" => object
            .get("predicates")
            .and_then(Value::as_array)
            .map(|items| items.iter().any(|item| predicate_matches(row, item)))
            .unwrap_or(true),
        "not" => !predicate_matches(row, object.get("predicate").unwrap_or(&Value::Null)),
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
        "min" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            Ok(json!(values.into_iter().reduce(f64::min).unwrap_or(0.0)))
        }
        "max" => {
            let values = eval_numeric_values(object.get("value"), datasets)?;
            Ok(json!(values.into_iter().reduce(f64::max).unwrap_or(0.0)))
        }
        "median" => {
            let mut values = eval_numeric_values(object.get("value"), datasets)?;
            values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
            if values.is_empty() {
                return Ok(json!(0.0));
            }
            let middle = values.len() / 2;
            let median = if values.len() % 2 == 0 {
                (values[middle - 1] + values[middle]) / 2.0
            } else {
                values[middle]
            };
            Ok(json!(median))
        }
        "unique_count" => {
            let Some(value_expr) = object.get("value") else {
                return Ok(json!(0));
            };
            let unique = match value_expr {
                Value::Array(items) => items.iter().map(Value::to_string).collect::<BTreeSet<_>>().len(),
                Value::Object(map) => {
                    if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                        && map.get("type").and_then(Value::as_str) == Some("text")
                    {
                        let source = map.get("source").or_else(|| map.get("rowset")).unwrap_or(&Value::Null);
                        let field = map.get("field").and_then(Value::as_str).unwrap_or("");
                        eval_rowset(source, datasets)?
                            .iter()
                            .map(|row| row_string(row, field))
                            .collect::<BTreeSet<_>>()
                            .len()
                    } else {
                        0
                    }
                }
                _ => 0,
            };
            Ok(json!(unique))
        }
        "item_count" => {
            let Some(value_expr) = object.get("value") else {
                return Ok(json!(0));
            };
            let count = match value_expr {
                Value::Array(items) => items.len(),
                _ => eval_rowset(value_expr, datasets).map(|rows| rows.len()).unwrap_or(0),
            };
            Ok(json!(count))
        }
        "ratio" => {
            let numerator = eval_scalar_value(
                object.get("numerator").unwrap_or(&Value::Null),
                base_rows,
                datasets,
            )?
            .as_f64()
            .unwrap_or(0.0);
            let denominator = eval_scalar_value(
                object.get("denominator").unwrap_or(&Value::Null),
                base_rows,
                datasets,
            )?
            .as_f64()
            .unwrap_or(0.0);
            if denominator.abs() < f64::EPSILON {
                Ok(json!(0.0))
            } else {
                Ok(json!(numerator / denominator))
            }
        }
        "percent" => {
            let rows = object
                .get("rowset")
                .map(|rowset| eval_rowset(rowset, datasets))
                .transpose()?
                .unwrap_or_else(|| base_rows.to_vec());
            let matched = object
                .get("predicate")
                .map(|predicate| rows.iter().filter(|row| predicate_matches(row, predicate)).count())
                .unwrap_or(rows.len());
            if rows.is_empty() {
                Ok(json!(0.0))
            } else {
                Ok(json!(matched as f64 / rows.len() as f64))
            }
        }
        "sum_first_number" => {
            let rows = base_rows;
            let fields = object
                .get("fields")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut total = 0.0;
            for row in rows {
                for field in &fields {
                    if let Some(name) = field.as_str() {
                        if let Some(number) = row_number(row, name) {
                            total += number;
                            break;
                        }
                    }
                }
            }
            Ok(json!(total))
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
                    .or_else(|| map.get("source"))
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

    fn repo_examples_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .canonicalize()
            .expect("resolve examples root")
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

    #[test]
    fn compile_examples_regressions() {
        let examples = repo_examples_root();
        for app_id in ["02-dataset", "03-cockpit", "05-chart"] {
            let app_root = examples.join(app_id);
            let compiled = compile_app_from_root(&examples, &app_root)
                .unwrap_or_else(|error| panic!("compile {app_id} failed: {error}"));
            assert!(
                compiled
                    .diagnostics
                    .iter()
                    .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
                "example {app_id} should not produce error diagnostics"
            );
            assert!(
                compiled.scene_contract.is_some(),
                "example {app_id} should contain scene contract"
            );
        }
    }
}
