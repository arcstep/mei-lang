//! ws-demo-v2 compile smoke test (MeiLang 2.0 graph path).

use mei_graph::compile_v2_app;

fn ws_demo_v2_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../workspaces/ws-demo-v2")
}

#[test]
fn demo_v2_compiles_graph_blocks() {
    let workspace = ws_demo_v2_root();
    if !workspace.join("apps/data-demo/src/app.mei").is_file() {
        eprintln!("skip: ws-demo-v2 not present at {}", workspace.display());
        return;
    }
    let outcome = compile_v2_app(&workspace, "data-demo").expect("compile-v2 data-demo");
    assert_eq!(outcome.syntax_version, "2.0.0");
    assert_eq!(outcome.files.len(), 26, "expected 26 v2 author files");
    assert!(
        outcome.blocks.len() >= 40,
        "expected many graph blocks, got {}",
        outcome.blocks.len()
    );
    assert!(
        outcome
            .blocks
            .iter()
            .any(|b| b.block_id == "app_skeleton:data-demo"),
        "missing app_skeleton block"
    );
}
