//! Runtime 消费链：将 world resource id、scene 路由路径、兼容 `from_dataset` 路径与 typed ref
//! 归一为 canonical `LoadedResource.id`，供 preview SSR 与 server dataset/metric API 共用。

use std::collections::BTreeMap;

use crate::model::{CompiledApp, LoadedResource};
use crate::typed_refs::{decode_ref_value, normalize_rel_path, RefKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeResourceResolveError {
    EmptySelector,
    ForbiddenLegacyId,
    NotFound { selector: String },
    Ambiguous { selector: String },
    NotDataset { resource_id: String },
}

impl std::fmt::Display for RuntimeResourceResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySelector => write!(f, "dataset selector is required"),
            Self::ForbiddenLegacyId => {
                write!(
                    f,
                    "dataset selector must be an explicit stable world resource id"
                )
            }
            Self::NotFound { selector } => {
                write!(
                    f,
                    "dataset `{selector}` not found in active scene resources"
                )
            }
            Self::Ambiguous { selector } => {
                write!(
                    f,
                    "dataset `{selector}` is ambiguous across scene resources"
                )
            }
            Self::NotDataset { resource_id } => {
                write!(f, "resource `{resource_id}` is not a dataset")
            }
        }
    }
}

impl std::error::Error for RuntimeResourceResolveError {}

/// 查找键 → canonical resource id。
#[derive(Debug, Clone, Default)]
pub struct RuntimeResourceIndex {
    aliases: BTreeMap<String, String>,
}

impl RuntimeResourceIndex {
    pub fn canonical_id(&self, selector: &str) -> Option<&str> {
        let key = normalize_rel_path(selector);
        if key.is_empty() {
            return None;
        }
        if self.aliases.contains_key(&key) {
            return self.aliases.get(&key).map(|s| s.as_str());
        }
        None
    }
}

pub fn build_runtime_resource_index(compiled: &CompiledApp) -> RuntimeResourceIndex {
    let mut aliases = BTreeMap::new();

    for resource in &compiled.resources {
        let id = resource.id.trim();
        if id.is_empty() {
            continue;
        }
        register_alias(&mut aliases, id, id);
        if let Some(dataset) = resource.dataset.as_ref() {
            let source_path = normalize_rel_path(&dataset.source.path);
            if !source_path.is_empty() {
                register_alias(&mut aliases, &source_path, id);
            }
        }
    }

    for route in &compiled.scene_routes {
        let target = normalize_rel_path(&route.target_file);
        if target.is_empty() {
            continue;
        }
        let scene_id = route.scene_id.trim();
        if !scene_id.is_empty() && compiled.resources.iter().any(|r| r.id == scene_id) {
            register_alias(&mut aliases, &target, scene_id);
        }
        let stem = scene_stem_from_path(&target);
        if !stem.is_empty() && compiled.resources.iter().any(|r| r.id == stem) {
            register_alias(&mut aliases, &target, &stem);
            register_alias(&mut aliases, &stem, &stem);
        }
    }

    for resource in &compiled.resources {
        let id = resource.id.as_str();
        if resource.dataset.is_some() {
            register_alias(&mut aliases, id, id);
        }
    }

    RuntimeResourceIndex { aliases }
}

pub fn build_runtime_resource_map(compiled: &CompiledApp) -> BTreeMap<String, LoadedResource> {
    let index = build_runtime_resource_index(compiled);
    let mut map = compiled
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.clone()))
        .collect::<BTreeMap<_, _>>();

    for (alias, canonical) in &index.aliases {
        if let Some(resource) = map.get(canonical).cloned() {
            map.insert(alias.clone(), resource);
        }
    }
    map
}

pub fn resolve_dataset_resource_id(
    compiled: &CompiledApp,
    selector: &str,
    index: Option<&RuntimeResourceIndex>,
) -> Result<String, RuntimeResourceResolveError> {
    let index = match index {
        Some(index) => index.clone(),
        None => build_runtime_resource_index(compiled),
    };
    resolve_dataset_resource_id_with_index(compiled, selector, &index)
}

fn resolve_dataset_resource_id_with_index(
    compiled: &CompiledApp,
    selector: &str,
    index: &RuntimeResourceIndex,
) -> Result<String, RuntimeResourceResolveError> {
    let key = normalize_rel_path(selector);
    if key.is_empty() {
        return Err(RuntimeResourceResolveError::EmptySelector);
    }

    if let Some(canonical) = index.canonical_id(&key) {
        return Ok(canonical.to_string());
    }

    if is_forbidden_legacy_resource_id(&key) {
        return Err(RuntimeResourceResolveError::ForbiddenLegacyId);
    }

    let direct_matches: Vec<_> = compiled
        .resources
        .iter()
        .filter(|resource| resource.id == key)
        .collect();
    match direct_matches.len() {
        0 => Err(RuntimeResourceResolveError::NotFound {
            selector: key.to_string(),
        }),
        1 => Ok(direct_matches[0].id.clone()),
        _ => Err(RuntimeResourceResolveError::Ambiguous {
            selector: key.to_string(),
        }),
    }
}

pub fn locate_dataset_resource<'a>(
    compiled: &'a CompiledApp,
    selector: &str,
) -> Result<&'a LoadedResource, RuntimeResourceResolveError> {
    let index = build_runtime_resource_index(compiled);
    let canonical = resolve_dataset_resource_id_with_index(compiled, selector, &index)?;
    let matches: Vec<_> = compiled
        .resources
        .iter()
        .filter(|resource| resource.id == canonical)
        .collect();
    match matches.len() {
        0 => Err(RuntimeResourceResolveError::NotFound {
            selector: canonical,
        }),
        1 => {
            let resource = matches[0];
            if resource.dataset.is_none() {
                return Err(RuntimeResourceResolveError::NotDataset {
                    resource_id: canonical,
                });
            }
            Ok(resource)
        }
        _ => Err(RuntimeResourceResolveError::Ambiguous {
            selector: canonical,
        }),
    }
}

/// 从 JSON ref 值或字符串 selector 解析 canonical dataset id。
pub fn resolve_dataset_selector_value(
    compiled: &CompiledApp,
    value: &serde_json::Value,
    index: &RuntimeResourceIndex,
) -> Option<String> {
    if let Some(expr) = decode_ref_value(value) {
        if matches!(
            expr.kind,
            RefKind::World | RefKind::Scene | RefKind::Flow | RefKind::Frame | RefKind::Panel
        ) {
            return None;
        }
        let selector = expr
            .id
            .as_deref()
            .or(expr.locator.scene_file.as_deref())
            .or(expr.locator.scene_id.as_deref())?;
        return resolve_dataset_resource_id_with_index(compiled, selector, index).ok();
    }
    if let Some(s) = value.as_str() {
        return resolve_dataset_resource_id_with_index(compiled, s, index).ok();
    }
    if let Some(map) = value.as_object() {
        if let Some(from) = map
            .get("from_dataset")
            .or_else(|| map.get("from"))
            .and_then(|v| v.as_str())
        {
            return resolve_dataset_resource_id_with_index(compiled, from, index).ok();
        }
        if let Some(id) = map.get("id").and_then(|v| v.as_str()) {
            return resolve_dataset_resource_id_with_index(compiled, id, index).ok();
        }
    }
    None
}

fn register_alias(aliases: &mut BTreeMap<String, String>, key: &str, canonical: &str) {
    let key = normalize_rel_path(key);
    let canonical = canonical.trim();
    if key.is_empty() || canonical.is_empty() {
        return;
    }
    aliases.insert(key, canonical.to_string());
}

fn scene_stem_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

pub fn is_forbidden_legacy_resource_id(id: &str) -> bool {
    let trimmed = id.trim();
    trimmed == "__source_path__" || trimmed.ends_with(".mei")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CompiledApp, DatasetView, LoadedResource, SourceDecl};
    use serde_json::json;

    fn sample_dataset_resource(id: &str) -> LoadedResource {
        LoadedResource {
            id: id.to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: id.to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["a".to_string()],
                rows: vec![json!({"a": 1})],
                source: SourceDecl {
                    kind: "csv".to_string(),
                    path: format!("data/{id}.csv"),
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
                metrics: Default::default(),
                runtime_metric_defs: Default::default(),
            }),
        }
    }

    fn sample_compiled() -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            resources: vec![
                sample_dataset_resource("warning_list"),
                sample_dataset_resource("home"),
            ],
            world_metrics: BTreeMap::new(),
            scene_routes: vec![crate::model::CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: None,
                is_default: true,
                access_export: true,
            }],
            app_root: ".".to_string(),
            title: "demo".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn resolves_resource_id_directly() {
        let compiled = sample_compiled();
        let id = resolve_dataset_resource_id(&compiled, "warning_list", None).expect("id");
        assert_eq!(id, "warning_list");
    }

    #[test]
    fn resolves_route_target_file_alias() {
        let compiled = sample_compiled();
        let id = resolve_dataset_resource_id(&compiled, "scenes/home.mei", None).expect("alias");
        assert_eq!(id, "home");
    }

    #[test]
    fn resolves_typed_dataset_ref_json() {
        let compiled = sample_compiled();
        let index = build_runtime_resource_index(&compiled);
        let value = json!({"__ref": "dataset", "id": "warning_list"});
        let id = resolve_dataset_selector_value(&compiled, &value, &index).expect("typed");
        assert_eq!(id, "warning_list");
    }
}
