//! L2/L3 缓存命中与失效测试 — 共享 fixture。

use std::path::Path;

pub(super) use std::fs;
pub(super) use crate::compile::{
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
pub(super) use crate::eval::evaluate_mei_file;
pub(super) use crate::model::CompiledSceneRoute;
pub(super) use crate::{
    compile_app_from_root_with_options, compile_revision_plan_from_root_with_options,
    compile_revision_token_from_root_with_options, CompileOptions,
};

pub(super) fn write_spbjw_like_app(root: &Path) {
    fs::create_dir_all(root.join("src/scenes/layouts")).unwrap();
    fs::create_dir_all(root.join("src/scenes/child")).unwrap();
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(
        root.join("src/main.mei"),
        r#"app(id = "cache-test", default_scene = "left", scene = scene_ref(scene_file = "scenes/layouts/left.mei"))"#,
    )
    .unwrap();
    fs::write(
        root.join("src/scenes/layouts/left.mei"),
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
        root.join("src/scenes/child/page.mei"),
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

pub(super) fn write_multi_route_app(root: &Path) {
    fs::create_dir_all(root.join("src/scenes")).unwrap();
    fs::write(
        root.join("src/main.mei"),
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
        root.join("src/scenes/left.mei"),
        r#"
scene(id = "left")
world()
frame()
"#,
    )
    .unwrap();
    fs::write(
        root.join("src/scenes/right.mei"),
        r#"
scene(id = "right")
world()
frame()
"#,
    )
    .unwrap();
}
