use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Context, Result};
use csv::StringRecord;
use serde_json::{json, Value};

use crate::{
    eval::evaluate_mei_file,
    model::{
        AppDecl, BlockDecl, CompiledApp, ComponentAsset, DatasetDecl, DatasetSourceDecl,
        DatasetView, Diagnostic, EntryDecl, FrameDecl, PanelDecl, RulesDecl, SceneContract,
        SceneDecl, Severity,
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
    let app_decl = app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let entry = app_decl
        .entries
        .first()
        .cloned()
        .unwrap_or(EntryDecl {
            id: Some("home".to_string()),
            frame: "main.mei".to_string(),
            title: None,
        });
    let entry_path = app_root.join(&entry.frame);
    let entry_decls = evaluate_mei_file(&entry_path)?;

    let mut frame: Option<FrameDecl> = None;
    let mut blocks: Vec<BlockDecl> = Vec::new();
    let mut datasets: Vec<DatasetDecl> = Vec::new();
    let mut scene: Option<SceneDecl> = None;
    let mut world: Option<crate::model::WorldDecl> = None;
    let mut rules: Option<RulesDecl> = None;
    let mut panels: Vec<PanelDecl> = Vec::new();

    if let Some(values) = entry_decls.as_array() {
        for value in values {
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                continue;
            };
            match kind {
                "frame" => frame = Some(serde_json::from_value(value.clone())?),
                "block" => blocks.push(serde_json::from_value(value.clone())?),
                "dataset" => datasets.push(serde_json::from_value(value.clone())?),
                "scene" => scene = Some(serde_json::from_value(value.clone())?),
                "world" => world = Some(serde_json::from_value(value.clone())?),
                "rules" => rules = Some(serde_json::from_value(value.clone())?),
                "panel" => panels.push(serde_json::from_value(value.clone())?),
                "app" => {}
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(entry.frame.clone()),
                }),
            }
        }
    }

    let dataset_views = datasets
        .iter()
        .map(|dataset| load_dataset_view(app_root, dataset))
        .collect::<Result<Vec<_>>>()?;

    let asset_map = load_component_assets(source_root)?;
    let mut asset_keys = BTreeSet::new();
    for block in &blocks {
        asset_keys.insert(block.use_key.clone());
    }
    let component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect::<Vec<ComponentAsset>>();

    if frame.is_none() && scene.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_root_surface".to_string(),
            message: "entry file must declare either frame(...) or scene(...)".to_string(),
            source_path: Some(entry.frame.clone()),
        });
    }

    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    let scene_contract = scene.map(|scene_decl| SceneContract {
        scene: scene_decl,
        world,
        rules,
        frame: frame.clone(),
        panels,
    });

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        entry_target: entry.frame.clone(),
        file_tree: source_tree(app_root)?,
        frame,
        blocks,
        datasets: dataset_views,
        scene_contract,
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

fn load_dataset_view(app_root: &Path, dataset: &DatasetDecl) -> Result<DatasetView> {
    let path = app_root.join(&dataset.source.path);
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
        id: dataset.id.clone(),
        title: dataset.title.clone(),
        columns,
        rows,
        source: DatasetSourceDecl {
            source_kind: dataset.source.kind.clone(),
            path: dataset.source.path.clone(),
        },
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
