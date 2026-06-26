use super::*;
use std::fs;

#[test]
fn build_id_contains_toolchain_version() {
    let id = generate_build_id("2026.6.1-abc1234");
    assert!(id.ends_with("2026.6.1-abc1234"));
    assert!(id.contains('T'));
}

#[test]
fn links_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
    fs::write(ws.join("workspace.json"), r#"{"schemaVersion":2}"#).expect("write");
    let mut links = LinksState::default();
    links.build.candidate = Some("20260625T120000-dev".into());
    write_links_state(ws, &links).expect("write");
    let loaded = read_links_state(ws).expect("read");
    assert_eq!(loaded.build.candidate.as_deref(), Some("20260625T120000-dev"));
}
