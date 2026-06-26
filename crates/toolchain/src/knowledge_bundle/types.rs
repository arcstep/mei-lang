use std::path::Path;

use serde::Serialize;

pub const KNOWLEDGE_BUNDLE_SCHEMA_VERSION: &str = "mei-knowledge-bundle-v1";

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeAssetDescriptor {
    pub id: String,
    pub surface: String,
    pub topic: String,
    pub kind: String,
    pub title: String,
    pub relative_path: String,
    pub install_relative_path: String,
    pub summary: String,
    pub injection_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeBundleDescriptor {
    pub schema_version: String,
    pub bundle_id: String,
    pub surface: String,
    pub package_root: String,
    pub install_dir_rel: String,
    pub primary_entry_ids: Vec<String>,
    pub available_topics: Vec<String>,
    pub assets: Vec<KnowledgeAssetDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeAssetContent {
    pub descriptor: KnowledgeAssetDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct AssetSeed {
    pub(crate) id: &'static str,
    pub(crate) topic: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) title: &'static str,
    pub(crate) rel_path: &'static str,
    pub(crate) install_rel_path: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) injection_roles: &'static [&'static str],
}

pub(crate) fn normalize_surface(surface: &str) -> Option<&'static str> {
    match surface.trim().to_ascii_lowercase().as_str() {
        "author" => Some("author"),
        "access" => Some("access"),
        _ => None,
    }
}

pub(crate) fn package_root_hint(package_root: &Path) -> String {
    let leaf = package_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mei-package");
    if package_root.join("Cargo.toml").is_file() {
        format!("source-tree:{leaf}")
    } else if package_root.ends_with(Path::new("share/mei")) {
        "installed-layout:share/mei".to_string()
    } else {
        format!("package-layout:{leaf}")
    }
}

