use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    collect_stock_catalog_routes, render_stock_catalog_main_mei, stock_catalog_app_config,
    stock_catalog_app_root, RuntimeWarmupApp, RuntimeWarmupManifest, APP_CONFIG_FILENAME,
    WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL, WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION,
};
use serde::Serialize;

const CATALOG_APP_CONFIG_JSON: &str = r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" }
}
"#;

#[derive(Debug, Clone, Serialize)]
pub struct SyncStockCatalogAppReport {
    pub app_root: String,
    pub route_count: usize,
    pub main_mei_written: bool,
    pub warmup_manifest_updated: bool,
    pub routes: Vec<mei_lang_kernel::StockCatalogRouteEntry>,
}

pub fn sync_stock_catalog_app(source_root: &Path) -> Result<SyncStockCatalogAppReport> {
    let routes = collect_stock_catalog_routes(source_root)?;
    let app_root = stock_catalog_app_root(source_root);
    fs::create_dir_all(app_root.join("src")).context("create catalog app src dir")?;
    let main_mei = render_stock_catalog_main_mei(source_root, routes.as_slice())?;
    let main_path = app_root.join("src/main.mei");
    let main_mei_written = !main_path.is_file() || fs::read_to_string(&main_path)? != main_mei;
    fs::write(&main_path, main_mei).context("write catalog app main.mei")?;
    fs::write(app_root.join(APP_CONFIG_FILENAME), CATALOG_APP_CONFIG_JSON)
        .context("write catalog app.config.json")?;
    let warmup_manifest_updated = merge_catalog_warmup_manifest(source_root, routes.as_slice())?;
    Ok(SyncStockCatalogAppReport {
        app_root: app_root.display().to_string(),
        route_count: routes.len(),
        main_mei_written,
        warmup_manifest_updated,
        routes,
    })
}

fn merge_catalog_warmup_manifest(
    source_root: &Path,
    routes: &[mei_lang_kernel::StockCatalogRouteEntry],
) -> Result<bool> {
    let manifest_path = source_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL);
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).context("create runtime/platform dir")?;
    }
    let mut manifest = if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read warmup manifest {}", manifest_path.display()))?;
        serde_json::from_str::<RuntimeWarmupManifest>(&raw)
            .with_context(|| format!("parse warmup manifest {}", manifest_path.display()))?
    } else {
        RuntimeWarmupManifest {
            schema_version: WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION.to_string(),
            enabled: true,
            apps: Vec::new(),
        }
    };
    if manifest.schema_version.trim().is_empty() {
        manifest.schema_version = WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION.to_string();
    }
    manifest.enabled = true;
    let app_id = stock_catalog_app_config(source_root).id;
    let scene_ids = routes
        .iter()
        .map(|route| route.route_id.clone())
        .collect::<Vec<_>>();
    let default_stage = scene_ids.first().cloned();
    let entry = RuntimeWarmupApp {
        app_id: app_id.clone(),
        default_scene: default_stage,
        hot_scenes: scene_ids.clone(),
        scenes: scene_ids,
        focuses: Vec::new(),
        datasets: Vec::new(),
        xlsx_sources: Vec::new(),
        compile_scope: None,
    };
    if let Some(existing) = manifest.apps.iter_mut().find(|app| app.app_id == app_id) {
        let changed = existing != &entry;
        *existing = entry;
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).context("serialize warmup manifest")?,
        )
        .context("write warmup manifest")?;
        return Ok(changed);
    }
    manifest.apps.push(entry);
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).context("serialize warmup manifest")?,
    )
    .context("write warmup manifest")?;
    Ok(true)
}
