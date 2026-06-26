use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use crate::mei_config::{write_mei_config, MeiConfig, MEI_CONFIG_FILENAME};
use crate::WorkspaceNode;

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

#[test]
fn load_component_assets_resolves_pack_path_and_preview() {
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("package root");
    let assets = load_component_assets(package_root.as_path()).expect("load assets");
    let asset = assets.get("chart.donut").expect("chart.donut");
    assert_eq!(asset.pack_path, "chart/echarts");
    assert!(
        asset
            .preview_mei
            .as_deref()
            .is_some_and(|path| path.ends_with("stock/components/chart/echarts/previews/chart.donut.mei")),
        "preview path missing for chart.donut"
    );
    let missing = audit_component_preview_coverage(package_root.as_path()).expect("audit");
    assert!(
        missing.is_empty(),
        "package stock should cover all manifest previews, missing: {missing:?}"
    );
}

fn flatten_paths(nodes: &[WorkspaceNode]) -> Vec<String> {
    let mut out = Vec::new();
    for node in nodes {
        out.push(node.path.clone());
        out.extend(flatten_paths(&node.children));
    }
    out
}
