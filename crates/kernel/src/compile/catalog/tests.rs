use std::fs;

use super::*;

#[test]
fn inactive_filter_compiles_no_catalog_files() {
    let filter = DatasetCatalogFilter::default();
    assert!(!filter.is_active());
    let root = std::env::temp_dir().join(format!("mei-catalog-inactive-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("main.mei"),
        r#"app(id = "t") scene = scene_ref(scene_file = "scenes/a.mei")"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/a.mei"),
        r#"scene(id="a") world() frame()"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/b.mei"),
        r#"scene(id="b") world() frame()"#,
    )
    .unwrap();
    let out = compile_dataset_catalog_resources(
        &root,
        &root,
        &serde_json::json!([]),
        &BTreeMap::new(),
        &filter,
    );
    assert!(out.is_empty());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn build_filter_never_returns_none_and_expands_panel_ref() {
    let root = std::env::temp_dir().join(format!("mei-catalog-embed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("scenes/layouts")).unwrap();
    fs::write(
        root.join("scenes/layouts/left.mei"),
        r#"
scene(id = "left")
world()
frame(
    panels = [
        panel_ref(id = "child_panel", scene_file = "scenes/child.mei"),
    ],
)
"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/child.mei"),
        r#"
scene(id = "child")
world()
world.add_dataset(id = "child_ds", source = ds.csv("x.csv"), schema = [ds.column("a", "string")])
frame()
frame.add_panel(id = "child_panel", area = "auto", blocks = [])
"#,
    )
    .unwrap();
    let filter = build_dataset_catalog_filter(&root, Some("scenes/layouts/left.mei"), &[]);
    assert!(filter.is_active());
    assert!(filter.dataset_paths.contains("scenes/layouts/left.mei"));
    assert!(filter.dataset_paths.contains("scenes/child.mei"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn extract_from_dataset_tokens_parses_world_id_and_path() {
    let src = r#"
            metric_ref("a", from_dataset = "typical_cases")
            metric_ref("b", from_dataset = "data/dataset/典型案例/监督典型案例.mei")
        "#;
    let tokens = super::scan::extract_from_dataset_tokens(src);
    assert!(tokens.contains(&"typical_cases".to_string()));
    assert!(tokens.iter().any(|t| t.contains("监督典型案例.mei")));
}

#[test]
fn extract_metric_ref_tokens_supports_positional_and_named_id() {
    let src = r#"
            component("x", props = {"metric": metric_ref("sales_total")})
            component("x", props = {"metric": metric_ref(id = "alerts_total", from_dataset = "warning_view")})
        "#;
    let tokens = super::scan::extract_metric_ref_tokens(src);
    assert!(tokens.contains(&("sales_total".to_string(), None)));
    assert!(tokens.contains(&(
        "alerts_total".to_string(),
        Some("warning_view".to_string())
    )));
}
