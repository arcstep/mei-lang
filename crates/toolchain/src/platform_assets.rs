use std::fs;
use std::path::{Path, PathBuf};

use mei_lang_kernel::{stock_components_source, stock_templates_source};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

pub const PLATFORM_ASSET_SCHEMA_VERSION: &str = "mei-platform-assets-v1";

#[derive(Debug, Clone, Serialize)]
pub struct PlatformAssetCatalogDescriptor {
    pub schema_version: String,
    pub package_root: String,
    pub registration_model: Vec<String>,
    pub component_packs: Vec<ComponentPackDescriptor>,
    pub template_packs: Vec<TemplatePackDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthoringSupportDescriptor {
    pub summary: String,
    pub knowledge_asset_ids: Vec<String>,
    pub recommended_example_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentPackDescriptor {
    pub id: String,
    pub asset_kind: String,
    pub source_dir_rel: String,
    pub manifest_file_rel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme_file_rel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoring_support: Option<AuthoringSupportDescriptor>,
    pub component_count: usize,
    pub component_ids: Vec<String>,
    pub component_exports: Vec<ComponentExportDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentExportDescriptor {
    pub id: String,
    pub tag: String,
    pub script_rel: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplatePackDescriptor {
    pub id: String,
    pub asset_kind: String,
    pub source_dir_rel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme_file_rel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authoring_support: Option<AuthoringSupportDescriptor>,
    pub template_file_count: usize,
    pub template_files: Vec<String>,
    pub asset_dirs: Vec<String>,
}

fn default_package_root() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    raw.canonicalize().unwrap_or(raw)
}

fn rel_to_package_root(package_root: &Path, path: &Path) -> String {
    path.strip_prefix(package_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_json_value(path: &Path) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn readme_for_pack(pack_dir: &Path, boundary: &Path) -> Option<String> {
    let mut current = Some(pack_dir);
    while let Some(dir) = current {
        let candidate = dir.join("README.md");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().replace('\\', "/"));
        }
        if dir == boundary {
            break;
        }
        current = dir.parent();
    }
    None
}

fn authoring_support(summary: &str, knowledge_asset_ids: &[&str], example_ids: &[&str]) -> AuthoringSupportDescriptor {
    AuthoringSupportDescriptor {
        summary: summary.to_string(),
        knowledge_asset_ids: knowledge_asset_ids.iter().map(|item| (*item).to_string()).collect(),
        recommended_example_ids: example_ids.iter().map(|item| (*item).to_string()).collect(),
    }
}

fn component_pack_authoring_support(pack_id: &str) -> Option<AuthoringSupportDescriptor> {
    match pack_id {
        "chart/echarts" => Some(authoring_support(
            "Public chart authoring surface. Prefer the common `data + mapping` contract before renderer-specific knobs.",
            &["component_contracts", "chart_components_guide", "dsl_contracts"],
            &["example_chart_baseline", "example_filter_reactivity"],
        )),
        "cockpit" => Some(authoring_support(
            "Cockpit-specific renderers and skins. Combine with cockpit template shells instead of inventing a cockpit-only DSL.",
            &["component_contracts", "cockpit_components_guide", "cockpit_template_index", "template_contracts", "dsl_contracts"],
            &["example_cockpit_panel", "example_template_clone", "example_data_table_runtime", "example_frame_layout_advanced"],
        )),
        "dataset" => Some(authoring_support(
            "Shared dataset/table/filter/query-state runtime components.",
            &["component_contracts", "dataset_components_guide", "dsl_contracts", "workspace_config_reference"],
            &["example_dataset_baseline", "example_filter_reactivity", "example_data_table_runtime", "example_metric_page_baseline", "example_upload_dataset_baseline"],
        )),
        "doc" => Some(authoring_support(
            "Minimal document rendering surface for markdown panels.",
            &["component_contracts", "dsl_contracts"],
            &["example_dataset_baseline"],
        )),
        "map/maplibre" => Some(authoring_support(
            "Standalone GIS map surface driven by `mapSpec`.",
            &["component_contracts", "cockpit_template_index", "template_contracts", "dsl_contracts"],
            &["example_map_baseline"],
        )),
        "mei" => Some(authoring_support(
            "Built-in text projection surface for plain/html content or metric slot rendering.",
            &["component_contracts", "dsl_contracts"],
            &["example_template_clone", "example_frame_layout_advanced"],
        )),
        "sim" => Some(authoring_support(
            "Simulation scene component pack. Start from the current scene contract via `scene_ref(\"self\")`.",
            &["component_contracts", "dsl_contracts"],
            &["example_sim_baseline"],
        )),
        _ => None,
    }
}

fn template_pack_authoring_support(pack_id: &str) -> Option<AuthoringSupportDescriptor> {
    match pack_id {
        "cockpit" => Some(authoring_support(
            "Public cockpit shell and metric-card template pack for standalone workspaces.",
            &["cockpit_template_index", "template_contracts", "cockpit_components_guide", "dsl_contracts"],
            &["example_cockpit_panel", "example_template_clone", "example_map_baseline", "example_frame_layout_advanced"],
        )),
        _ => None,
    }
}

fn component_pack_descriptors(package_root: &Path) -> Vec<ComponentPackDescriptor> {
    let components_root = stock_components_source(package_root);
    if !components_root.is_dir() {
        return Vec::new();
    }
    let mut packs = WalkDir::new(&components_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && entry.file_name() == "manifest.json")
        .filter_map(|entry| {
            let manifest_path = entry.path().to_path_buf();
            let manifest_value = read_json_value(&manifest_path)?;
            let pack_dir = manifest_path.parent()?;
            let pack_rel = pack_dir
                .strip_prefix(&components_root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            let components = manifest_value
                .get("components")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let mut component_ids = components.keys().cloned().collect::<Vec<_>>();
            component_ids.sort();
            let mut component_exports = component_ids
                .iter()
                .map(|component_id| {
                    let item = components.get(component_id).and_then(Value::as_object);
                    let tag = item
                        .and_then(|item| item.get("tag"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default()
                        .to_string();
                    let script = item
                        .and_then(|item| item.get("script"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default();
                    let script_rel = if script.is_empty() {
                        pack_rel.clone()
                    } else if pack_rel.is_empty() {
                        script.to_string()
                    } else {
                        format!("{pack_rel}/{script}")
                    };
                    ComponentExportDescriptor {
                        id: component_id.clone(),
                        tag,
                        script_rel,
                    }
                })
                .collect::<Vec<_>>();
            component_exports.sort_by(|left, right| left.id.cmp(&right.id));
            Some(ComponentPackDescriptor {
                id: if pack_rel.is_empty() {
                    "root".to_string()
                } else {
                    pack_rel.clone()
                },
                asset_kind: "component_pack".to_string(),
                source_dir_rel: rel_to_package_root(package_root, pack_dir),
                manifest_file_rel: rel_to_package_root(package_root, &manifest_path),
                readme_file_rel: readme_for_pack(pack_dir, &components_root)
                    .map(|path| rel_to_package_root(package_root, Path::new(&path))),
                authoring_support: component_pack_authoring_support(if pack_rel.is_empty() {
                    "root"
                } else {
                    pack_rel.as_str()
                }),
                component_count: component_ids.len(),
                component_ids,
                component_exports,
            })
        })
        .collect::<Vec<_>>();
    packs.sort_by(|left, right| left.id.cmp(&right.id));
    packs
}

fn collect_template_files(pack_dir: &Path) -> Vec<String> {
    let mut files = WalkDir::new(pack_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("mei"))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(pack_dir)
                .ok()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn collect_asset_dirs(pack_dir: &Path) -> Vec<String> {
    let mut dirs = WalkDir::new(pack_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .filter_map(|entry| {
            let rel = entry.path().strip_prefix(pack_dir).ok()?;
            if rel.as_os_str().is_empty() {
                return None;
            }
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            rel_str.contains("assets").then_some(rel_str)
        })
        .collect::<Vec<_>>();
    dirs.sort();
    dirs.dedup();
    dirs
}

fn template_pack_descriptors(package_root: &Path) -> Vec<TemplatePackDescriptor> {
    let templates_root = stock_templates_source(package_root);
    if !templates_root.is_dir() {
        return Vec::new();
    }
    let mut packs = fs::read_dir(&templates_root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let id = path.file_name()?.to_string_lossy().to_string();
            if id.starts_with('.') {
                return None;
            }
            let template_files = collect_template_files(&path);
            let asset_dirs = collect_asset_dirs(&path);
            Some(TemplatePackDescriptor {
                id,
                asset_kind: "template_pack".to_string(),
                source_dir_rel: rel_to_package_root(package_root, &path),
                readme_file_rel: path
                    .join("README.md")
                    .is_file()
                    .then(|| rel_to_package_root(package_root, &path.join("README.md"))),
                authoring_support: template_pack_authoring_support(
                    path.file_name()?.to_string_lossy().as_ref(),
                ),
                template_file_count: template_files.len(),
                template_files,
                asset_dirs,
            })
        })
        .collect::<Vec<_>>();
    packs.sort_by(|left, right| left.id.cmp(&right.id));
    packs
}

pub fn platform_asset_catalog_descriptor() -> PlatformAssetCatalogDescriptor {
    platform_asset_catalog_descriptor_for_package_root(default_package_root().as_path())
}

pub fn platform_asset_catalog_descriptor_for_package_root(
    package_root: &Path,
) -> PlatformAssetCatalogDescriptor {
    PlatformAssetCatalogDescriptor {
        schema_version: PLATFORM_ASSET_SCHEMA_VERSION.to_string(),
        package_root: package_root.display().to_string(),
        registration_model: vec![
            "component_packs_register_via_stock_components_manifest".to_string(),
            "template_packs_register_via_stock_templates_directory".to_string(),
            "host_extensions_must_register_before_export".to_string(),
        ],
        component_packs: component_pack_descriptors(package_root),
        template_packs: template_pack_descriptors(package_root),
    }
}
