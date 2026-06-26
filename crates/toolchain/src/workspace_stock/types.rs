use serde::{Deserialize, Serialize};

pub(crate) const STOCK_MANIFEST_FILENAME: &str = "STOCK.json";
pub(crate) const STOCK_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub(crate) const BOOTSTRAP_SOURCE_PLATFORM_DEFAULT: &str = "platform-default";
pub(crate) const LEGACY_STOCK_DIR: &str = ".stock";

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeReport {
    pub source_root: String,
    pub components: MaterializeDirReport,
    pub templates: MaterializeDirReport,
    pub authoring: MaterializeDirReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeDirReport {
    pub from: String,
    pub to: String,
    pub copied_files: usize,
    pub skipped_files: usize,
    pub overwritten_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockTreeFingerprint {
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    #[serde(skip_serializing_if = "Option::is_none", rename = "pathsHash")]
    pub paths_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "materializedAt")]
    pub materialized_at: String,
    #[serde(rename = "bootstrapSource")]
    pub bootstrap_source: String,
    #[serde(rename = "packageRoot")]
    pub package_root: String,
    pub components: StockTreeFingerprint,
    pub templates: StockTreeFingerprint,
    pub authoring: StockTreeFingerprint,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockDoctorReport {
    pub ok: bool,
    pub missing_trees: Vec<String>,
    pub orphan_paths: Vec<String>,
    pub manifest_drift: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_component_previews: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalog_app_drift: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrateWorkspaceStockPathsReport {
    pub renamed_legacy_stock: bool,
    pub updated_example_files: Vec<String>,
}
