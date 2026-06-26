use super::helpers::*;

#[test]
fn dependency_fingerprint_changes_when_transitive_file_content_changes() {
    clear_file_content_hash_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-dep-hash-{}", std::process::id()));
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
    let graph = DependencyGraph::build(&root, &app_decls, &routes);
    let first = graph
        .dependency_fingerprint_for_target(&root, &app_decls, "scenes/layouts/left.mei")
        .expect("first fingerprint");
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(
        root.join("scenes/child/page.mei"),
        r#"
scene(id = "child")
world()
world.add_dataset(
    id = "child_ds",
    source = ds.csv("data/sample.csv"),
    schema = [ds.column("name", "string")]
)
frame()
frame.add_panel(id = "child_panel", area = "main", blocks = [text("changed")])
"#,
    )
    .unwrap();
    let second = graph
        .dependency_fingerprint_for_target(&root, &app_decls, "scenes/layouts/left.mei")
        .expect("second fingerprint");
    assert_ne!(
        first, second,
        "transitive file content change should change fingerprint"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_revision_token_ignores_unrelated_scene_changes() {
    let root = std::env::temp_dir().join(format!("mei-revision-token-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_multi_route_app(&root);
    let options = CompileOptions {
        scene: Some("left".to_string()),
        preview_target: None,
    };
    let first =
        compile_revision_token_from_root_with_options(&root, &root, &options).expect("first token");
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(
        root.join("scenes/right.mei"),
        r#"
scene(id = "right")
world()
frame()
frame.add_panel(id = "right_panel", area = "main", blocks = [text("changed")])
"#,
    )
    .unwrap();
    let second = compile_revision_token_from_root_with_options(&root, &root, &options)
        .expect("second token");
    assert_eq!(
        first, second,
        "selected scene revision token should ignore unrelated route changes"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_revision_plan_tracks_only_relevant_watch_files() {
    let root = std::env::temp_dir().join(format!("mei-revision-plan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_multi_route_app(&root);
    let options = CompileOptions {
        scene: Some("left".to_string()),
        preview_target: None,
    };
    let plan = compile_revision_plan_from_root_with_options(&root, &root, &options)
        .expect("revision plan");
    let watched: Vec<&str> = plan
        .watched_files
        .iter()
        .map(|item| item.rel_path.as_str())
        .collect();
    assert!(
        watched.contains(&"main.mei"),
        "revision plan should always watch main.mei"
    );
    assert!(
        watched.contains(&"scenes/left.mei"),
        "selected route should stay in watch set"
    );
    assert!(
        !watched.contains(&"scenes/right.mei"),
        "unrelated route should not enter focused watch set"
    );
    let _ = fs::remove_dir_all(&root);
}

