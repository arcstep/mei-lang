use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::mei_config::{
    resolve_apps_root, resolve_components_root, stock_path_excluded, StockCatalogKind,
};
use crate::model::ComponentAsset;

#[derive(Debug, Deserialize)]
struct ComponentManifestFile {
    components: BTreeMap<String, ComponentManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ComponentManifestEntry {
    tag: String,
    script: String,
    #[serde(default)]
    preview: Option<String>,
}

/// Load workspace-shared stock components (`stock/components`).
pub fn load_component_assets(source_root: &Path) -> Result<BTreeMap<String, ComponentAsset>> {
    let components_root = resolve_components_root(source_root);
    load_component_assets_from_root(source_root, &components_root, true, None)
}

/// Merge workspace stock + optional `apps/{app_id}/stock/components`.
/// App-local keys override workspace keys on collision.
pub fn load_component_assets_for_app(
    source_root: &Path,
    app_id: &str,
) -> Result<BTreeMap<String, ComponentAsset>> {
    let mut assets = load_component_assets(source_root)?;
    let app_components = resolve_apps_root(source_root)
        .join(app_id)
        .join("stock/components");
    if !app_components.is_dir() {
        return Ok(assets);
    }
    let app_assets =
        load_component_assets_from_root(source_root, &app_components, false, Some(app_id))?;
    for (key, asset) in app_assets {
        assets.insert(key, asset);
    }
    Ok(assets)
}

fn load_component_assets_from_root(
    source_root: &Path,
    components_root: &Path,
    apply_stock_exclusions: bool,
    app_id: Option<&str>,
) -> Result<BTreeMap<String, ComponentAsset>> {
    if !components_root.exists() {
        return Ok(BTreeMap::new());
    }
    let mut manifests = WalkDir::new(components_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "manifest.json")
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    manifests.sort();

    let mut assets = BTreeMap::new();
    for manifest_path in manifests {
        let rel = manifest_path
            .strip_prefix(components_root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if apply_stock_exclusions
            && stock_path_excluded(source_root, StockCatalogKind::Components, rel.as_str())
        {
            continue;
        }
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest: ComponentManifestFile = serde_json::from_str(&raw).with_context(|| {
            format!(
                "failed to parse component manifest {}",
                manifest_path.display()
            )
        })?;
        let manifest_dir = manifest_path.parent().unwrap_or(components_root);
        let pack_path = manifest_dir
            .strip_prefix(components_root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        for (key, entry) in manifest.components {
            let script_path = if app_id.is_some() {
                normalize_app_component_script_path(source_root, manifest_dir, &entry.script)
                    .with_context(|| format!("failed to resolve script for component `{key}`"))?
            } else {
                normalize_component_script_path(components_root, manifest_dir, &entry.script)
                    .with_context(|| format!("failed to resolve script for component `{key}`"))?
            };
            let preview_mei = resolve_component_preview_workspace_path(
                source_root,
                manifest_dir,
                key.as_str(),
                &entry,
            );
            let asset = ComponentAsset {
                key: key.clone(),
                tag: entry.tag,
                script: script_path,
                pack_path: pack_path.clone(),
                preview_mei,
            };
            if assets.insert(key.clone(), asset).is_some() {
                bail!(
                    "duplicate component key `{key}` found while loading {}",
                    manifest_path.display()
                );
            }
        }
    }
    Ok(assets)
}

fn resolve_component_preview_workspace_path(
    source_root: &Path,
    manifest_dir: &Path,
    use_key: &str,
    entry: &ComponentManifestEntry,
) -> Option<String> {
    let preview_abs = if let Some(rel) = entry
        .preview
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        manifest_dir.join(rel)
    } else {
        manifest_dir.join("previews").join(format!("{use_key}.mei"))
    };
    if !preview_abs.is_file() {
        return None;
    }
    workspace_relative_path(source_root, preview_abs.as_path())
}

fn workspace_relative_path(source_root: &Path, abs: &Path) -> Option<String> {
    abs.strip_prefix(source_root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| !rel.is_empty())
}

/// Manifest use_keys missing a resolvable pack-local preview scene.
pub fn audit_component_preview_coverage(source_root: &Path) -> Result<Vec<String>> {
    let assets = load_component_assets(source_root)?;
    let mut missing = assets
        .values()
        .filter(|asset| asset.preview_mei.is_none())
        .map(|asset| asset.key.clone())
        .collect::<Vec<_>>();
    missing.sort();
    Ok(missing)
}

fn normalize_component_script_path(
    components_root: &Path,
    manifest_dir: &Path,
    script: &str,
) -> Result<String> {
    let resolved = manifest_dir.join(script);
    let relative = resolved.strip_prefix(components_root).with_context(|| {
        format!(
            "script path `{}` escapes _components root",
            resolved.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// App-local scripts are served as workspace-relative paths under `/workspace-components/`.
fn normalize_app_component_script_path(
    source_root: &Path,
    manifest_dir: &Path,
    script: &str,
) -> Result<String> {
    let resolved = manifest_dir.join(script);
    if let Some(rel) = workspace_relative_path(source_root, resolved.as_path()) {
        return Ok(rel);
    }
    let source_abs = fs::canonicalize(source_root).unwrap_or_else(|_| source_root.to_path_buf());
    let resolved_abs = if resolved.exists() {
        fs::canonicalize(&resolved).unwrap_or(resolved)
    } else {
        resolved
    };
    let relative = resolved_abs.strip_prefix(&source_abs).with_context(|| {
        format!(
            "app component script `{}` is outside workspace root {}",
            resolved_abs.display(),
            source_root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod app_local_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_app_local_wubi_practice_when_present() {
        let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-demo-v2");
        if !ws
            .join("apps/wubi/stock/components/wubi/manifest.json")
            .is_file()
        {
            eprintln!("skip: ws-demo-v2 wubi app stock not present");
            return;
        }
        let assets = load_component_assets_for_app(ws.as_path(), "wubi").expect("load");
        let practice = assets.get("wubi.practice").expect("wubi.practice registered");
        assert_eq!(practice.tag, "mei-wubi-practice");
        assert!(
            practice
                .script
                .contains("apps/wubi/stock/components/wubi/practice.js"),
            "script={}",
            practice.script
        );
    }
}
