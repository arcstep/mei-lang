//! Export scope tree + path selection helpers for portable snapshots.

use std::fs;
use std::path::Path;

use serde::Serialize;

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".mei-cache",
    ".DS_Store",
];

/// Workspace-relative folder node for the Viewer export tree.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTreeNode {
    pub path: String,
    pub name: String,
    pub default_checked: bool,
    pub children: Vec<ExportTreeNode>,
}

/// Stock (default unchecked) + apps (default checked) folder trees.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportScopeTree {
    pub stock: Vec<ExportTreeNode>,
    pub apps: Vec<ExportTreeNode>,
}

/// Normalize a workspace-relative path to forward-slash form without `./` or trailing `/`.
pub fn normalize_rel_path(raw: &str) -> String {
    let mut s = raw.trim().replace('\\', "/");
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    while s.starts_with('/') {
        s = s[1..].to_string();
    }
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// True when `rel` is the selected folder itself or a descendant of any selected folder.
pub fn path_is_selected(selected: &[String], rel: &str) -> bool {
    let rel = normalize_rel_path(rel);
    if rel.is_empty() {
        return false;
    }
    selected.iter().any(|raw| {
        let p = normalize_rel_path(raw);
        if p.is_empty() {
            return false;
        }
        rel == p || rel.starts_with(&format!("{p}/"))
    })
}

/// App ids implied by selected paths under `apps/<id>/…`.
pub fn app_ids_from_selection(selected: &[String]) -> Vec<String> {
    let mut ids = Vec::new();
    for raw in selected {
        let p = normalize_rel_path(raw);
        let Some(rest) = p.strip_prefix("apps/") else {
            continue;
        };
        let id = rest.split('/').next().unwrap_or("");
        if id.is_empty() {
            continue;
        }
        if !ids.iter().any(|x| x == id) {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    ids
}

/// Build folder trees for stock (default off) and apps (default on).
pub fn list_export_scope_tree(workspace: &Path) -> anyhow::Result<ExportScopeTree> {
    let stock = walk_folder_tree(&workspace.join("stock"), "stock", false)?;
    let apps = walk_folder_tree(&workspace.join("apps"), "apps", true)?;
    Ok(ExportScopeTree { stock, apps })
}

fn walk_folder_tree(
    root: &Path,
    prefix: &str,
    default_checked: bool,
) -> anyhow::Result<Vec<ExportTreeNode>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    walk_children(root, prefix, default_checked, 0)
}

fn walk_children(
    dir: &Path,
    prefix: &str,
    default_checked: bool,
    depth: usize,
) -> anyhow::Result<Vec<ExportTreeNode>> {
    if depth > 24 {
        return Ok(Vec::new());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && !SKIP_DIR_NAMES.contains(&name.as_str())
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = format!("{prefix}/{name}");
        let children = walk_children(&entry.path(), &path, default_checked, depth + 1)?;
        nodes.push(ExportTreeNode {
            path,
            name,
            default_checked,
            children,
        });
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_covers_descendants() {
        let selected = vec!["apps/zhifa/upload".into(), "stock/gis".into()];
        assert!(path_is_selected(&selected, "apps/zhifa/upload"));
        assert!(path_is_selected(&selected, "apps/zhifa/upload/videos/a.mp4"));
        assert!(!path_is_selected(&selected, "apps/zhifa/assets"));
        assert!(path_is_selected(&selected, "stock/gis/tiles/x.mbtiles"));
        assert!(!path_is_selected(&selected, "stock/components"));
    }

    #[test]
    fn app_ids_from_nested_selection() {
        let selected = vec![
            "apps/zhifa/upload".into(),
            "apps/other/assets".into(),
            "stock/gis".into(),
        ];
        assert_eq!(
            app_ids_from_selection(&selected),
            vec!["other".to_string(), "zhifa".to_string()]
        );
    }
}
