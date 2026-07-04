//! Authoring examples strict compile gate (mei-compiler subprocess).

use std::path::PathBuf;
use std::process::Command;

fn ws_demo_v2_root() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .ok()?;
    if root.join("workspace.json").is_file() {
        Some(root)
    } else {
        None
    }
}

fn mei_lang_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("mei-lang root")
}

#[test]
fn author_frame_layout_advanced_mei_compiler_strict_compile_succeeds() {
    let Some(ws_root) = ws_demo_v2_root() else {
        return;
    };
    if !ws_root
        .join("stock/authoring/examples/frame-layout-advanced.mei")
        .is_file()
    {
        return;
    }
    let status = Command::new("cargo")
        .current_dir(mei_lang_root())
        .args([
            "run",
            "-p",
            "mei-compiler",
            "--",
            "compile",
            "--workspace",
            ws_root.to_str().expect("workspace"),
            "--app",
            "_author-frame-smoke",
        ])
        .status()
        .expect("spawn mei-compiler");
    assert!(status.success(), "_author-frame-smoke compile failed");
}
