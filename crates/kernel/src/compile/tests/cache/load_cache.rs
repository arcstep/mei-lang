use super::helpers::*;

#[test]
fn decl_file_cache_hits_on_repeated_external_loads() {
    clear_decl_file_cache_for_tests();
    let root = std::env::temp_dir().join(format!("mei-decl-cache-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_spbjw_like_app(&root);
    let options = CompileOptions {
        scene: Some("left".to_string()),
        preview_target: None,
            ..Default::default()
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

#[test]
fn xlsx_table_snapshot_singleflight_deduplicates_parallel_cold_loads() {
    use std::sync::Arc;
    use std::thread;

    clear_materialize_cache_for_tests();
    let source =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-spbjw/zhifa");
    if !source.join("upload/8.行政处罚结果清单.xlsx").is_file() {
        eprintln!("skip xlsx singleflight test: zhifa fixture missing");
        return;
    }
    let root = std::env::temp_dir().join(format!("mei-xlsx-sf-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("upload")).unwrap();
    fs::copy(
        source.join("upload/8.行政处罚结果清单.xlsx"),
        root.join("upload/demo.xlsx"),
    )
    .unwrap();

    let app_root = Arc::new(root.clone());
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let app_root = Arc::clone(&app_root);
            thread::spawn(move || {
                crate::compile::cached_load_xlsx_table_snapshot(
                    app_root.as_path(),
                    "upload/demo.xlsx",
                    None,
                    1,
                )
                .expect("parallel xlsx load")
            })
        })
        .collect();
    let mut row_counts = Vec::new();
    for handle in handles {
        let (snapshot, _) = handle.join().expect("thread join");
        row_counts.push(snapshot.rows.len());
    }
    assert!(
        row_counts.windows(2).all(|pair| pair[0] == pair[1]),
        "all parallel loads should share the same snapshot rows"
    );
    let _ = fs::remove_dir_all(&root);
}
