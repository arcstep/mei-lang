use super::helpers::*;

#[test]
fn dependency_graph_cache_hits_on_repeated_build() {
    clear_dependency_graph_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-graph-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let app_decls = evaluate_mei_file(&root.join("main.mei")).expect("eval main");
    let routes = vec![CompiledSceneRoute {
        scene_id: "left".to_string(),
        frame_id: None,
        target_file: "scenes/layouts/left.mei".to_string(),
        kind: "file_ref".to_string(),
        title: None,
        is_default: true,
        access_export: true,
    }];
    let before = dependency_graph_cache_metrics_snapshot();
    let first = DependencyGraph::build_cached(&root, &app_decls, &routes);
    let after_first = dependency_graph_cache_metrics_snapshot();
    let second = DependencyGraph::build_cached(&root, &app_decls, &routes);
    let after_second = dependency_graph_cache_metrics_snapshot();
    assert!(first
        .dependent_targets_for_file("scenes/child/page.mei")
        .contains("scenes/layouts/left.mei"));
    assert_eq!(
        second.dependent_targets_for_file("scenes/child/page.mei"),
        first.dependent_targets_for_file("scenes/child/page.mei")
    );
    assert!(
        after_first.1 > before.1,
        "first build should miss graph cache"
    );
    assert!(
        after_second.0 > after_first.0,
        "second build should hit graph cache"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn l2_cache_key_changes_when_dependency_fingerprint_changes() {
    let root = std::env::temp_dir().join(format!("mei-l2-key-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let key_a = scene_payload_cache_key(
        &root,
        &root,
        "scenes/layouts/left.mei",
        None,
        Some("dep-a@1|dep-b@2"),
    )
    .expect("cache key a");
    let key_b = scene_payload_cache_key(
        &root,
        &root,
        "scenes/layouts/left.mei",
        None,
        Some("dep-a@1|dep-b@3"),
    )
    .expect("cache key b");
    assert_ne!(
        key_a, key_b,
        "dependency fingerprint should affect L2 cache key"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn l2_cache_key_changes_when_scene_selector_changes() {
    let root = std::env::temp_dir().join(format!("mei-l2-scene-key-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let key_a = scene_payload_cache_key(
        &root,
        &root,
        "scenes/layouts/left.mei",
        Some("overview"),
        Some("dep-a@1"),
    )
    .expect("cache key a");
    let key_b = scene_payload_cache_key(
        &root,
        &root,
        "scenes/layouts/left.mei",
        Some("detail"),
        Some("dep-a@1"),
    )
    .expect("cache key b");
    assert_ne!(key_a, key_b, "scene selector should affect L2 cache key");
    let _ = fs::remove_dir_all(&root);
}

