use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::model::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};

#[derive(Debug, Default, Deserialize)]
struct MeiConfigDiscover {
    #[serde(default)]
    skip_directories: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MeiConfigDisk {
    #[serde(default)]
    discover: MeiConfigDiscover,
}

fn segment_discover_skip_dirs(segment_root: &Path) -> HashSet<String> {
    let mut out: HashSet<String> = ["node_modules", ".git", "target", "dist"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let path = segment_root.join(".mei-config.json");
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<MeiConfigDisk>(&raw) {
            for d in cfg.discover.skip_directories {
                let t = d.trim().trim_matches('/').replace('\\', "/");
                if !t.is_empty() && !t.contains('/') {
                    out.insert(t);
                }
            }
        }
    }
    out
}

/// 仅在 `source_root` 的**一级子目录**下递归发现应用（不会把 `source_root/main.mei` 当作应用，
/// 也不会从 `spbjw/` 扫描到 `examples/` 等兄弟目录）。
pub fn discover_apps(source_root: &Path) -> Result<Vec<WorkspaceAppMeta>> {
    let mut apps = Vec::new();
    if !source_root.is_dir() {
        bail!(
            "discover_apps: source_root `{}` is not a directory",
            source_root.display()
        );
    }
    for child in fs::read_dir(source_root)
        .with_context(|| format!("discover_apps: read_dir {}", source_root.display()))?
    {
        let child = child.context("discover_apps: read_dir entry")?;
        let name = child.file_name().to_string_lossy().to_string();
        if !child.file_type().context("discover_apps: file_type")?.is_dir() {
            continue;
        }
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        let child_root = child.path();
        let skip_dirs = segment_discover_skip_dirs(&child_root);
        let skip = skip_dirs.clone();
        let walker = WalkDir::new(&child_root)
            .min_depth(1)
            .into_iter()
            .filter_entry(move |entry| {
                if !entry.file_type().is_dir() {
                    return true;
                }
                let seg = entry.file_name().to_string_lossy();
                if seg.starts_with('_') || seg.starts_with('.') {
                    return false;
                }
                !skip.contains(seg.as_ref())
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
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
}

pub fn read_source_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn mei_body_declares_scene(body: &str) -> bool {
    for line in body.lines() {
        let head = line.split('#').next().unwrap_or("").trim_start();
        if head.starts_with("scene(") || head.starts_with("scene (") {
            return true;
        }
    }
    false
}

fn mei_file_kind(root: &Path, relative: &str, file_name: &str) -> Option<String> {
    if !file_name.ends_with(".mei") {
        return None;
    }
    if file_name.eq_ignore_ascii_case("main.mei") {
        return Some("main".into());
    }
    let path = root.join(relative);
    let Ok(body) = fs::read_to_string(&path) else {
        return Some("mei".into());
    };
    if mei_body_declares_scene(&body) {
        return Some("scene".into());
    }
    Some("mei".into())
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
        if relative
            .split('/')
            .any(|seg| !seg.is_empty() && seg.starts_with('.'))
        {
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
                mei_kind: None,
                children: Vec::new(),
            });
        } else {
            let mei_kind = mei_file_kind(root, &relative, &name);
            by_parent.entry(parent).or_default().push(WorkspaceNode {
                name,
                path: relative,
                kind: "file".to_string(),
                mei_kind,
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
