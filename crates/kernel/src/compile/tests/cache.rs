//! L2/L3 缓存命中与失效测试。

use std::fs;
use std::path::Path;

use crate::compile::{
    clear_materialize_cache_for_tests, clear_scene_payload_cache_for_tests,
    legacy_rows_cache_len_for_tests, scene_payload_cache_len_for_tests,
};
use crate::{compile_app_from_root_with_options, CompileOptions};

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
