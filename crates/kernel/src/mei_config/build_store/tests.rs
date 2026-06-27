use super::*;
use std::fs;

#[test]
fn build_id_contains_toolchain_version() {
    let id = generate_build_id("2026.6.1-abc1234");
    assert!(id.ends_with("2026.6.1-abc1234"));
    assert!(id.contains('T'));
}

#[test]
fn merge_build_content_store_copies_missing_blobs_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = tmp.path().join("apps/demo");
    let from = app.join("build/store/build-a");
    let to = app.join("build/store/build-b");
    fs::create_dir_all(from.join("store/content/scene_payload")).expect("mkdir from");
    fs::create_dir_all(to.join("store/content/scene_payload")).expect("mkdir to");
    fs::write(
        from.join("store/content/scene_payload/aaa.json"),
        br#"{"schemaVersion":"x"}"#,
    )
    .expect("write from");
    fs::write(
        to.join("store/content/scene_payload/bbb.json"),
        br#"{"schemaVersion":"y"}"#,
    )
    .expect("write to");
    let stats = merge_build_content_store(from.as_path(), to.as_path()).expect("merge");
    assert_eq!(stats.copied_files, 1);
    assert!(to.join("store/content/scene_payload/aaa.json").is_file());
    assert!(to.join("store/content/scene_payload/bbb.json").is_file());
    let stats_again = merge_build_content_store(from.as_path(), to.as_path()).expect("merge");
    assert_eq!(stats_again.copied_files, 0);
    assert_eq!(stats_again.skipped_existing, 1);
}

#[test]
fn promote_build_unions_historical_cas_into_active_target() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    fs::write(ws.join("workspace.json"), r#"{"schemaVersion":2}"#).expect("write ws");
    fs::create_dir_all(ws.join("apps/zhifa/build/store/build-old/store/content/scene_payload"))
        .expect("mkdir old");
    fs::create_dir_all(ws.join("apps/zhifa/build/store/build-new/store/content/scene_payload"))
        .expect("mkdir new");
    fs::write(
        ws.join("apps/zhifa/build/store/build-old/store/content/scene_payload/old-only.json"),
        br#"{"old":true}"#,
    )
    .expect("write old blob");
    fs::write(
        ws.join("apps/zhifa/build/store/build-new/store/content/scene_payload/new-only.json"),
        br#"{"new":true}"#,
    )
    .expect("write new blob");
    let mut links = LinksState::default();
    links.build.active = Some("build-new".into());
    write_links_state(ws, &links).expect("write links");
    let promoted = promote_build(ws, Some("build-new")).expect("promote");
    assert_eq!(promoted, "build-new");
    assert!(
        ws.join("apps/zhifa/build/store/build-new/store/content/scene_payload/old-only.json").is_file(),
        "promote should union historical CAS into active build store"
    );
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
