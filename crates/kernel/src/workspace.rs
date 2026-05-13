use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::model::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};

pub fn discover_apps(source_root: &Path) -> Result<Vec<WorkspaceAppMeta>> {
    let mut apps = Vec::new();
    for entry in fs::read_dir(source_root)
        .with_context(|| format!("failed to read source root {}", source_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with('_') {
            continue;
        }
        let main_path = path.join("main.mei");
        if !main_path.exists() {
            continue;
        }
        apps.push(WorkspaceAppMeta {
            id: name.to_string(),
            title: name.to_string(),
            root: path.to_string_lossy().to_string(),
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
    let manifest_path = source_root.join("_components/manifest.json");
    if !manifest_path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: ComponentManifestFile =
        serde_json::from_str(&raw).context("failed to parse component manifest")?;
    Ok(manifest
        .components
        .into_iter()
        .map(|(key, entry)| {
            let asset = ComponentAsset {
                key: key.clone(),
                tag: entry.tag,
                script: entry.script,
            };
            (key, asset)
        })
        .collect())
}
