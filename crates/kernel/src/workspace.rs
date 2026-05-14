use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::model::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};

pub fn discover_apps(source_root: &Path) -> Result<Vec<WorkspaceAppMeta>> {
    let mut apps = Vec::new();
    let walker = WalkDir::new(source_root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            !name.starts_with('_') && !name.starts_with('.')
        });
    for entry in walker.filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_file() || entry.file_name() != "main.mei" {
            continue;
        }
        let Some(app_root) = entry.path().parent() else {
            continue;
        };
        let Ok(relative) = app_root.strip_prefix(source_root) else {
            continue;
        };
        let id = relative.to_string_lossy().replace('\\', "/");
        if id.is_empty() {
            continue;
        }
        apps.push(WorkspaceAppMeta {
            id: id.clone(),
            title: id,
            root: app_root.to_string_lossy().to_string(),
        });
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
}

pub fn read_source_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub fn source_tree(root: &Path) -> Result<Vec<WorkspaceNode>> {
    let mut by_parent: BTreeMap<String, Vec<WorkspaceNode>> = BTreeMap::new();
    let mut dirs: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with('.') {
            continue;
        }
        let parent = path
            .parent()
            .and_then(|value| value.strip_prefix(root).ok())
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type().is_dir() {
            dirs.push(path.to_path_buf());
            by_parent.entry(parent).or_default().push(WorkspaceNode {
                name,
                path: relative,
                kind: "dir".to_string(),
                children: Vec::new(),
            });
        } else {
            by_parent.entry(parent).or_default().push(WorkspaceNode {
                name,
                path: relative,
                kind: "file".to_string(),
                children: Vec::new(),
            });
        }
    }

    fn build(
        path: &str,
        by_parent: &mut BTreeMap<String, Vec<WorkspaceNode>>,
    ) -> Vec<WorkspaceNode> {
        let mut nodes = by_parent.remove(path).unwrap_or_default();
        nodes.sort_by(
            |left, right| match (left.kind.as_str(), right.kind.as_str()) {
                ("dir", "file") => std::cmp::Ordering::Less,
                ("file", "dir") => std::cmp::Ordering::Greater,
                _ => left.name.cmp(&right.name),
            },
        );
        for node in &mut nodes {
            if node.kind == "dir" {
                node.children = build(&node.path, by_parent);
            }
        }
        nodes
    }

    let _ = dirs;
    Ok(build("", &mut by_parent))
}

#[derive(Debug, Deserialize)]
struct ComponentManifestFile {
    components: BTreeMap<String, ComponentManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ComponentManifestEntry {
    tag: String,
    script: String,
}

pub fn load_component_assets(source_root: &Path) -> Result<BTreeMap<String, ComponentAsset>> {
    let components_root = if source_root.join("_components").exists() {
        source_root.join("_components")
    } else if source_root
        .parent()
        .is_some_and(|parent| parent.join("_components").exists())
    {
        source_root
            .parent()
            .map(|parent| parent.join("_components"))
            .expect("parent existence checked above")
    } else {
        source_root.join("_components")
    };
    if !components_root.exists() {
        return Ok(BTreeMap::new());
    }
    let mut manifests = WalkDir::new(&components_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == "manifest.json")
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    manifests.sort();

    let mut assets = BTreeMap::new();
    for manifest_path in manifests {
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest: ComponentManifestFile = serde_json::from_str(&raw).with_context(|| {
            format!(
                "failed to parse component manifest {}",
                manifest_path.display()
            )
        })?;
        let manifest_dir = manifest_path.parent().unwrap_or(&components_root);
        for (key, entry) in manifest.components {
            let script_path =
                normalize_component_script_path(&components_root, manifest_dir, &entry.script)
                    .with_context(|| format!("failed to resolve script for component `{key}`"))?;
            let asset = ComponentAsset {
                key: key.clone(),
                tag: entry.tag,
                script: script_path,
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
