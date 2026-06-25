use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::mei_config::{
    is_v2_app_root, load_workspace_config, resolve_app_entry_main, resolve_apps_root,
    resolve_components_root, stock_path_excluded, StockCatalogKind, APP_CONFIG_FILENAME,
    MEI_CONFIG_FILENAME,
};
use crate::model::{ComponentAsset, WorkspaceAppMeta, WorkspaceNode};

fn segment_discover_skip_dirs(segment_root: &Path) -> HashSet<String> {
    let mut out: HashSet<String> = ["node_modules", ".git", "target", "dist"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let cfg = load_workspace_config(segment_root);
    for d in cfg.discover_skip_directories() {
        out.insert(d);
    }
    out
}

fn push_discovered_app(
    app_root: &Path,
    _source_root: &Path,
    apps: &mut Vec<WorkspaceAppMeta>,
) -> Result<()> {
    let id = app_root
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("discover_apps: app root has no directory name"))?
        .to_string();
    apps.push(WorkspaceAppMeta {
        id: id.clone(),
        title: id,
        root: app_root.to_string_lossy().to_string(),
    });
    Ok(())
}

/// 在 `root` 下发现 mei 应用（v2：`app.config.json` 或 `src/main.mei`）。
fn discover_apps_under(
    root: &Path,
    source_root: &Path,
    skip_dirs: &HashSet<String>,
    apps: &mut Vec<WorkspaceAppMeta>,
) -> Result<()> {
    if is_v2_app_root(root) {
        push_discovered_app(root, source_root, apps)?;
        return Ok(());
    }
    if !root.is_dir() {
        return Ok(());
    }
    for child in
        fs::read_dir(root).with_context(|| format!("discover_apps: read_dir {}", root.display()))?
    {
        let child = child.context("discover_apps: read_dir entry")?;
        if !child
            .file_type()
            .context("discover_apps: file_type")?
            .is_dir()
        {
            continue;
        }
        let name = child.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') || skip_dirs.contains(&name) {
            continue;
        }
        discover_apps_under(&child.path(), source_root, skip_dirs, apps)?;
    }
    Ok(())
}

/// 在 `{workspace}/apps/` 下发现应用。
pub fn discover_apps(source_root: &Path) -> Result<Vec<WorkspaceAppMeta>> {
    let mut apps = Vec::new();
    if !source_root.is_dir() {
        bail!(
            "discover_apps: source_root `{}` is not a directory",
            source_root.display()
        );
    }
    let apps_root = resolve_apps_root(source_root);
    if !apps_root.is_dir() {
        return Ok(apps);
    }
    let skip_dirs = segment_discover_skip_dirs(source_root);
    for child in fs::read_dir(&apps_root)
        .with_context(|| format!("discover_apps: read_dir {}", apps_root.display()))?
    {
        let child = child.context("discover_apps: read_dir entry")?;
        if !child
            .file_type()
            .context("discover_apps: file_type")?
            .is_dir()
        {
            continue;
        }
        let name = child.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') || skip_dirs.contains(&name) {
            continue;
        }
        discover_apps_under(&child.path(), source_root, &skip_dirs, &mut apps)?;
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
    if file_name.ends_with(".board.mei") {
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

/// 同一 stem 的 Mei 胶囊变体排序：scene `.mei` → `.board.mei` → `.world.mei`。
fn mei_capsule_variant_rank(file_name: &str) -> u8 {
    if file_name.ends_with(".world.mei") {
        2
    } else if file_name.ends_with(".board.mei") {
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
    let components_root = resolve_components_root(source_root);
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
        let rel = manifest_path
            .strip_prefix(&components_root)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if stock_path_excluded(source_root, StockCatalogKind::Components, rel.as_str()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::{write_mei_config, MeiConfig};

    fn temp_test_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mei_kernel_test_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("temp test root");
        dir
    }

    fn write_main_mei(dir: &Path, app_id: &str) {
        fs::create_dir_all(dir).expect("mkdir app dir");
        let body = format!(
            r#"app(id="{app_id}")
scene(id="home", target="home.mei")
"#
        );
        fs::write(dir.join("main.mei"), body).expect("write main.mei");
        fs::write(dir.join("home.mei"), "frame()").expect("write home.mei");
    }

    #[test]
    fn discover_prefers_mei_config_over_nested_main() {
        let root = temp_test_root("discover_config");
        let segment = root.join("demo");
        fs::create_dir_all(&segment).expect("mkdir segment");
        let app = segment.join("myapp");
        fs::create_dir_all(app.join("nested")).expect("mkdir");
        write_mei_config(&app.join(MEI_CONFIG_FILENAME), &MeiConfig::default())
            .expect("write config");
        write_main_mei(&app.join("nested"), "nested-app");
        write_main_mei(&segment.join("legacy"), "legacy-app");

        let apps = discover_apps(&root).expect("discover");
        let ids: Vec<_> = apps.iter().map(|app| app.id.as_str()).collect();
        assert!(ids.contains(&"demo/myapp"));
        assert!(!ids.iter().any(|id| id.contains("nested")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_falls_back_to_main_mei_without_config() {
        let root = temp_test_root("discover_main");
        let segment = root.join("examples");
        write_main_mei(&segment.join("core/foo"), "foo");

        let apps = discover_apps(&root).expect("discover");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "examples/core/foo");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn source_tree_includes_root_mei_config_only() {
        let root = temp_test_root("source_tree");
        fs::write(root.join(MEI_CONFIG_FILENAME), "{}").expect("root config");
        fs::create_dir_all(root.join("sub")).expect("mkdir sub");
        fs::write(root.join("sub/.mei-config.json"), "{}").expect("nested config");
        fs::write(root.join("visible.txt"), "ok").expect("visible");

        let nodes = source_tree(&root).expect("tree");
        let paths: Vec<_> = flatten_paths(&nodes);
        assert!(paths.contains(&".mei-config.json".to_string()));
        assert!(!paths.iter().any(|p| p.contains("sub/.mei-config")));
        assert!(paths.contains(&"visible.txt".to_string()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn source_tree_orders_scene_board_world_variants_by_stem() {
        let root = temp_test_root("source_tree_capsule_sort");
        fs::create_dir_all(root.join("scenes")).expect("mkdir scenes");
        for name in [
            "01-执法要素.board.mei",
            "01-执法要素.mei",
            "01-执法要素.world.mei",
            "02-其他.mei",
        ] {
            fs::write(root.join("scenes").join(name), "// stub").expect("write mei");
        }

        let nodes = source_tree(&root).expect("tree");
        let scenes = nodes
            .iter()
            .find(|node| node.path == "scenes")
            .map(|node| node.children.as_slice())
            .unwrap_or_else(|| panic!("missing scenes dir: {:?}", nodes));
        let names: Vec<_> = scenes
            .iter()
            .filter(|node| node.kind == "file")
            .map(|node| node.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "01-执法要素.mei",
                "01-执法要素.board.mei",
                "01-执法要素.world.mei",
                "02-其他.mei",
            ]
        );
        let board = scenes
            .iter()
            .find(|node| node.name == "01-执法要素.board.mei")
            .expect("board capsule");
        let world = scenes
            .iter()
            .find(|node| node.name == "01-执法要素.world.mei")
            .expect("world capsule");
        assert_eq!(board.mei_kind.as_deref(), Some("board"));
        assert_eq!(world.mei_kind.as_deref(), Some("world"));
        let _ = fs::remove_dir_all(&root);
    }

    fn flatten_paths(nodes: &[WorkspaceNode]) -> Vec<String> {
        let mut out = Vec::new();
        for node in nodes {
            out.push(node.path.clone());
            out.extend(flatten_paths(&node.children));
        }
        out
    }
}
