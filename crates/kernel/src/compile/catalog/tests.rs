use std::fs;

use super::*;
use crate::compile::dependency_graph::DependencyGraph;
use crate::eval::evaluate_mei_file;
use crate::model::CompiledSceneRoute;

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
        &DependencyGraph::default(),
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
        root.join("main.mei"),
        r#"app(id = "t", default_scene = "left", scene = scene_ref(scene_file = "scenes/layouts/left.mei"))"#,
    )
    .unwrap();
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
    let app_decls = evaluate_mei_file(&root.join("main.mei")).expect("eval main");
    let graph = DependencyGraph::build(
        &root,
        &app_decls,
        &[CompiledSceneRoute {
            scene_id: "left".to_string(),
            frame_id: None,
            target_file: "scenes/layouts/left.mei".to_string(),
            kind: "file_ref".to_string(),
            title: None,
            is_default: true,
            access_export: true,
        }],
    );
    let filter =
        build_dataset_catalog_filter(&root, &app_decls, &graph, Some("scenes/layouts/left.mei"));
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
    assert!(tokens.contains(&("alerts_total".to_string(), Some("warning_view".to_string()))));
}

#[test]
fn resolve_catalog_compile_rels_uses_resource_and_metric_indexes() {
    clear_dataset_catalog_index_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-catalog-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("data/dataset")).unwrap();
    fs::write(
        root.join("data/dataset/alpha.mei"),
        r#"
scene(id = "alpha")
world()
world.add_dataset(id = "alpha_ds", source = ds.csv("alpha.csv"), schema = [ds.column("a", "string")])
world.add_metric(
    metric(
        id = "alpha_total",
        title = "Alpha Total",
        expr = "1"
    )
)
frame()
"#,
    )
    .unwrap();
    fs::write(
        root.join("data/dataset/beta.mei"),
        r#"
scene(id = "beta")
world()
world.add_dataset(id = "beta_ds", source = ds.csv("beta.csv"), schema = [ds.column("b", "string")])
frame()
"#,
    )
    .unwrap();

    let mut filter = DatasetCatalogFilter::default();
    filter.resource_ids.insert("alpha_ds".to_string());
    filter.metric_ids.insert("alpha_total".to_string());
    let rels = super::scan::resolve_dataset_catalog_compile_rels(&root, &filter);
    assert_eq!(rels, vec!["data/dataset/alpha.mei".to_string()]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dataset_catalog_index_cache_hits_on_repeated_resolution() {
    clear_dataset_catalog_index_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-catalog-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("data/dataset")).unwrap();
    fs::write(
        root.join("data/dataset/alpha.mei"),
        r#"
scene(id = "alpha")
world()
world.add_dataset(id = "alpha_ds", source = ds.csv("alpha.csv"), schema = [ds.column("a", "string")])
frame()
"#,
    )
    .unwrap();
    let mut filter = DatasetCatalogFilter::default();
    filter.resource_ids.insert("alpha_ds".to_string());

    let before = dataset_catalog_index_cache_metrics_snapshot();
    let first = super::scan::resolve_dataset_catalog_compile_rels(&root, &filter);
    let after_first = dataset_catalog_index_cache_metrics_snapshot();
    let second = super::scan::resolve_dataset_catalog_compile_rels(&root, &filter);
    let after_second = dataset_catalog_index_cache_metrics_snapshot();

    assert_eq!(first, vec!["data/dataset/alpha.mei".to_string()]);
    assert_eq!(second, first);
    assert!(
        after_first.1 > before.1,
        "first resolution should miss cache"
    );
    assert!(
        after_second.0 > after_first.0,
        "second resolution should hit cache"
    );
    let _ = fs::remove_dir_all(&root);
}
