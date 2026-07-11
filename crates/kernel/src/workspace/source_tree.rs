use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::mei_config::{resolve_app_entry_main, APP_CONFIG_FILENAME, MEI_CONFIG_FILENAME};
use crate::model::WorkspaceNode;

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

fn mei_body_declares_fragment_surface(body: &str) -> bool {
    for line in body.lines() {
        let head = line.split('#').next().unwrap_or("").trim_start();
        if head.starts_with("frame(")
            || head.starts_with("frame (")
            || head.starts_with("panel(")
            || head.starts_with("panel (")
            || head.starts_with("world(")
            || head.starts_with("world (")
        {
            return true;
        }
    }
    false
}

fn mei_file_kind(root: &Path, relative: &str, file_name: &str) -> Option<String> {
    if !file_name.ends_with(".mei") {
        return None;
    }
    if file_name.ends_with(".board.mei") || file_name.ends_with(".page.mei") {
        return Some("board".into());
    }
    if file_name.ends_with(".world.mei") {
        return Some("world".into());
    }
    let entry_main = resolve_app_entry_main(root);
    if relative == entry_main || file_name.eq_ignore_ascii_case("main.mei") {
        return Some("main".into());
    }
    let path = root.join(relative);
    let Ok(body) = fs::read_to_string(&path) else {
        return Some("mei".into());
    };
    if mei_body_declares_scene(&body) {
        return Some("scene".into());
    }
    if mei_body_declares_fragment_surface(&body) {
        return Some("fragment".into());
    }
    Some("mei".into())
}

fn should_include_source_tree_file(relative: &str) -> bool {
    if relative == MEI_CONFIG_FILENAME || relative == APP_CONFIG_FILENAME {
        return true;
    }
    !relative
        .split('/')
        .any(|seg| !seg.is_empty() && seg.starts_with('.'))
}

/// 同一 stem 的 Mei 胶囊变体排序：scene `.mei` → `.page/.board.mei` → `.world.mei`。
fn mei_capsule_variant_rank(file_name: &str) -> u8 {
    if file_name.ends_with(".world.mei") {
        2
    } else if file_name.ends_with(".board.mei") || file_name.ends_with(".page.mei") {
        1
    } else if file_name.ends_with(".mei") {
        0
    } else {
        3
    }
}

fn mei_sort_stem(file_name: &str) -> &str {
    if let Some(stem) = file_name.strip_suffix(".world.mei") {
        return stem;
    }
    if let Some(stem) = file_name.strip_suffix(".page.mei") {
        return stem;
    }
    if let Some(stem) = file_name.strip_suffix(".board.mei") {
        return stem;
    }
    if let Some(stem) = file_name.strip_suffix(".mei") {
        return stem;
    }
    file_name
}

fn source_tree_node_cmp(left: &WorkspaceNode, right: &WorkspaceNode) -> std::cmp::Ordering {
    match (left.kind.as_str(), right.kind.as_str()) {
        ("dir", "file") => std::cmp::Ordering::Less,
        ("file", "dir") => std::cmp::Ordering::Greater,
        _ => {
            let stem_cmp =
                mei_sort_stem(left.name.as_str()).cmp(mei_sort_stem(right.name.as_str()));
            if stem_cmp != std::cmp::Ordering::Equal {
                return stem_cmp;
            }
            let rank_cmp = mei_capsule_variant_rank(left.name.as_str())
                .cmp(&mei_capsule_variant_rank(right.name.as_str()));
            if rank_cmp != std::cmp::Ordering::Equal {
                return rank_cmp;
            }
            left.name.cmp(&right.name)
        }
    }
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
        if !should_include_source_tree_file(&relative) {
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
                scene_export_id: None,
                world_dataset_id: None,
                world_metric_id: None,
                explain_block_id: None,
                semantic_label: None,
                children: Vec::new(),
            });
        } else {
            let mei_kind = mei_file_kind(root, &relative, &name);
            by_parent.entry(parent).or_default().push(WorkspaceNode {
                name,
                path: relative,
                kind: "file".to_string(),
                mei_kind,
                scene_export_id: None,
                world_dataset_id: None,
                world_metric_id: None,
                explain_block_id: None,
                semantic_label: None,
                children: Vec::new(),
            });
        }
    }

    fn build(
        path: &str,
        by_parent: &mut BTreeMap<String, Vec<WorkspaceNode>>,
    ) -> Vec<WorkspaceNode> {
        let mut nodes = by_parent.remove(path).unwrap_or_default();
        nodes.sort_by(source_tree_node_cmp);
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
