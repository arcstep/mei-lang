use std::collections::HashSet;

use mei_bundle::{
    compute_workspace_digest, exchange_from_outcome, read_bundle, write_bundle_from_outcome,
};
use mei_graph::compile_app;

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
    let outcome = compile_app(&workspace, "data-demo").expect("compile data-demo");
    assert_eq!(outcome.syntax_version, "2.0.1");
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

#[test]
fn demo_v2_meibundle_roundtrip_and_size() {
    let workspace = ws_demo_v2_root();
    if !workspace.join("apps/data-demo/src/app.mei").is_file() {
        eprintln!("skip: ws-demo-v2 not present at {}", workspace.display());
        return;
    }
    let outcome = compile_app(&workspace, "data-demo").expect("compile data-demo");
    let exchange = exchange_from_outcome(&outcome);

    let indexed_ids: HashSet<_> = exchange
        .sources
        .iter()
        .flat_map(|s| s.block_ids.iter().cloned())
        .collect();
    assert_eq!(
        indexed_ids.len(),
        exchange.blocks.len(),
        "sources must cover every block_id exactly once"
    );
    for block in &exchange.blocks {
        assert!(indexed_ids.contains(&block.block_id));
    }

    let digest = compute_workspace_digest(&workspace, "data-demo", "stock/templates");
    let dir = std::env::temp_dir().join("mei-compiler-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let bundle_path = dir.join("data-demo.meibundle");

    let write_stats = write_bundle_from_outcome(
        &outcome,
        digest.as_str(),
        "2.0.1",
        bundle_path.as_path(),
    )
    .expect("write bundle");

    assert!(
        write_stats.bundle_bytes < write_stats.blocks_json_bytes / 2,
        "bundle should be smaller than half of compact blocks json (got bundle {} vs json {})",
        write_stats.bundle_bytes,
        write_stats.blocks_json_bytes
    );

    let (manifest, blocks) = read_bundle(bundle_path.as_path()).expect("read bundle");
    assert_eq!(manifest.block_count, exchange.blocks.len());
    assert_eq!(blocks.len(), exchange.blocks.len());

    let written_ids: HashSet<_> = blocks.iter().map(|b| b.block_id.clone()).collect();
    let original_ids: HashSet<_> = exchange.blocks.iter().map(|b| b.block_id.clone()).collect();
    assert_eq!(written_ids, original_ids);

    let _ = std::fs::remove_file(bundle_path);
}
