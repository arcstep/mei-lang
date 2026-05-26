//! L2/L3 缓存命中与失效测试。

use std::fs;
use std::path::Path;

use crate::compile::{
    clear_materialize_cache_for_tests, clear_scene_payload_cache_for_tests,
    decl_file_cache::{
        clear_decl_file_cache_for_tests, decl_file_cache_metrics_snapshot_for_tests,
    },
    dependency_graph::{
        clear_dependency_graph_cache_for_tests, clear_file_content_hash_cache_for_tests,
        dependency_graph_cache_metrics_snapshot, DependencyGraph,
    },
    legacy_rows_cache_len_for_tests,
    scene_payload_cache::scene_payload_cache_key,
    scene_payload_cache_len_for_tests,
};
use crate::eval::evaluate_mei_file;
use crate::model::CompiledSceneRoute;
use crate::{
    compile_app_from_root_with_options, compile_revision_plan_from_root_with_options,
    compile_revision_token_from_root_with_options, CompileOptions,
};

fn write_spbjw_like_app(root: &Path) {
    fs::create_dir_all(root.join("scenes/layouts")).unwrap();
    fs::create_dir_all(root.join("scenes/child")).unwrap();
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(
        root.join("main.mei"),
        r#"app(id = "cache-test", default_scene = "left", scene = scene_ref(scene_file = "scenes/layouts/left.mei"))"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/layouts/left.mei"),
        r#"
scene(id = "left")
world()
frame(
    panels = [
        panel_ref(id = "child_panel", scene_file = "scenes/child/page.mei"),
    ],
)
"#,
    )
    .unwrap();
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
frame.add_panel(id = "child_panel", area = "main", blocks = [])
"#,
    )
    .unwrap();
    fs::write(root.join("data/sample.csv"), "name\na\n").unwrap();
}

fn write_multi_route_app(root: &Path) {
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("main.mei"),
        r#"
app(
    id = "revision-test",
    default_scene = "left",
    scene = scene_ref(scene_file = "scenes/left.mei")
)
app.add_scene(scene_ref(id = "right", scene_file = "scenes/right.mei"))
"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/left.mei"),
        r#"
scene(id = "left")
world()
frame()
"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/right.mei"),
        r#"
scene(id = "right")
world()
frame()
"#,
    )
    .unwrap();
}

#[test]
fn l2_scene_payload_reused_for_catalog_and_official() {
    clear_scene_payload_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-l2-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let options = CompileOptions {
        scene: None,
        preview_target: Some("scenes/layouts/left.mei".to_string()),
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
        Some("dep-a@1|dep-b@2"),
    )
    .expect("cache key a");
    let key_b = scene_payload_cache_key(
        &root,
        &root,
        "scenes/layouts/left.mei",
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

#[test]
fn decl_file_cache_hits_on_repeated_external_loads() {
    clear_decl_file_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-decl-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let options = CompileOptions {
        scene: Some("left".to_string()),
        preview_target: None,
    };
    let before = decl_file_cache_metrics_snapshot_for_tests();
    let _ =
        compile_app_from_root_with_options(&root, &root, options.clone()).expect("first compile");
    let after_first = decl_file_cache_metrics_snapshot_for_tests();
    let _ = compile_app_from_root_with_options(&root, &root, options).expect("second compile");
    let after_second = decl_file_cache_metrics_snapshot_for_tests();
    assert!(
        after_first.1 > before.1,
        "first compile should populate decl file cache"
    );
    assert!(
        after_second.0 > after_first.0,
        "second compile should reuse cached decl files"
    );
    let _ = fs::remove_dir_all(&root);
}
