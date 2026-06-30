use serde::{Deserialize, Serialize};

pub(crate) const STOCK_MANIFEST_FILENAME: &str = "STOCK.json";
pub(crate) const STOCK_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub(crate) const BOOTSTRAP_SOURCE_PLATFORM_DEFAULT: &str = "platform-default";

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
