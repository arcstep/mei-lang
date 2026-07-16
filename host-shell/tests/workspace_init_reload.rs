//! workspace init + reload integration tests.

use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

fn host_shell_bin() -> std::path::PathBuf {
    std::env::var("CARGO_BIN_EXE_mei-host-shell")
        .map(std::path::PathBuf::from)
        .expect("CARGO_BIN_EXE_mei-host-shell")
}

#[test]
fn workspace_init_creates_config_and_stock() {
    let temp = tempdir().expect("tempdir");
    let dir = temp.path().join("ws-test");
    std::fs::create_dir_all(&dir).expect("mkdir");

    let status = Command::new(host_shell_bin())
        .args([
            "workspace",
            "init",
            "--dir",
            dir.to_str().expect("path"),
            "--id",
            "ws-test",
            "--label",
            "Test WS",
            "--app",
            "demo",
        ])
        .status()
        .expect("spawn");
    assert!(status.success(), "workspace init failed");

    assert!(dir.join("workspace.json").is_file());
    assert!(dir.join("mei.lang.json").is_file());
    assert!(dir.join("stock/components").is_dir());
    assert!(dir.join("apps/demo/app.toml").is_file());
    assert!(dir.join("apps/demo/src/stage/home.stage.mdx").is_file());
}

fn ws_demo_v2() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

#[test]
fn reload_imports_ws_demo_v2_when_bundle_exists() {
    let workspace = match ws_demo_v2() {
        Some(ws) => ws,
        None => return,
    };
    if !workspace.join("apps/zhifa").is_dir() {
        return;
    }
    let bundle = workspace.join("apps/zhifa/build/active/exchange/zhifa.meibundle");
    if !bundle.is_file() {
        return;
    }
    let output = Command::new(host_shell_bin())
        .args([
            "reload",
            "--workspace",
            workspace.to_str().expect("path"),
            "--app",
            "zhifa",
            "--bundle",
            bundle.to_str().expect("bundle"),
            "--json",
        ])
        .output()
        .expect("spawn reload");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("reload json stdout");
    assert_eq!(json.get("accepted").and_then(|v| v.as_bool()), Some(true));
}
