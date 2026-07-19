use super::helpers::*;

#[test]
fn l2_scene_payload_reused_for_catalog_and_official() {
    clear_scene_payload_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-l2-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let options = CompileOptions {
        scene: None,
        preview_target: Some("scenes/layouts/left.mei".to_string()),
            ..Default::default()
    };
    let _ =
        compile_app_from_root_with_options(&root, &root, options.clone()).expect("first compile");
    let after_first = scene_payload_cache_len_for_tests();
    assert!(after_first >= 1, "expected L2 entries after first compile");
    let _ = compile_app_from_root_with_options(&root, &root, options).expect("second compile");
    let after_second = scene_payload_cache_len_for_tests();
    assert!(
        after_second >= 1,
        "L2 cache should stay populated after repeat compile (first={after_first}, second={after_second})"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn l3_rows_cache_invalidates_on_data_file_change() {
    clear_materialize_cache_for_tests();
    clear_scene_payload_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-l3-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let options = CompileOptions {
        scene: None,
        preview_target: Some("scenes/child/page.mei".to_string()),
            ..Default::default()
    };
    let _ =
        compile_app_from_root_with_options(&root, &root, options.clone()).expect("compile child");
    let rows_after_first = legacy_rows_cache_len_for_tests();
    assert!(rows_after_first >= 1);

    let _ =
        compile_app_from_root_with_options(&root, &root, options.clone()).expect("compile again");
    assert!(
        legacy_rows_cache_len_for_tests() >= rows_after_first,
        "repeat compile should not shrink L3 rows cache"
    );

    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(root.join("data/sample.csv"), "name\nb\n").unwrap();
    clear_scene_payload_cache_for_tests();
    let _ = compile_app_from_root_with_options(&root, &root, options)
        .expect("compile after data change");
    assert!(
        legacy_rows_cache_len_for_tests() >= rows_after_first,
        "data mtime change should produce new L3 row entry"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dependency_graph_tracks_route_closure_and_dependents() {
    let root = std::env::temp_dir().join(format!("mei-dag-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let app_decls = evaluate_mei_file(&root.join("main.mei")).expect("eval main");
    let routes = vec![CompiledSceneRoute {
        scene_id: "left".to_string(),
        frame_id: None,
        target_file: "scenes/layouts/left.mei".to_string(),
        kind: "file_ref".to_string(),
        title: None,
        short_title: None,
        is_default: true,
        access_export: true,
    }];
    let graph = DependencyGraph::build(&root, &app_decls, &routes);
    let dependents = graph.dependent_targets_for_file("scenes/child/page.mei");
    assert!(
        dependents.contains("scenes/layouts/left.mei"),
        "child capsule should map back to left route target"
    );
    let fingerprint = graph
        .dependency_fingerprint_for_target(&root, &app_decls, "scenes/layouts/left.mei")
        .expect("fingerprint");
    assert!(
        fingerprint.contains("scenes/layouts/left.mei")
            && fingerprint.contains("scenes/child/page.mei"),
        "fingerprint should include transitive mei closure"
    );
    let _ = fs::remove_dir_all(&root);
}

