use super::*;
use std::fs;
use std::path::Path;

fn write_ws(root: &Path, ws_version: &str) {
    fs::write(
        root.join("workspace.json"),
        format!(
            r#"{{"schemaVersion":2,"workspace":{{"version":"{ws_version}"}}}}"#
        ),
    )
    .expect("write workspace.json");
}

#[test]
fn format_env_generation_id_composes_toolchain_and_workspace() {
    assert_eq!(
        format_env_generation_id("2.0.1", "20260228"),
        "2.0.1-ws20260228"
    );
    assert_eq!(
        format_env_generation_id("latest", "WS20260228"),
        "latest-ws20260228"
    );
}

#[test]
fn resolve_toolchain_segment_maps_dev_alias_to_latest() {
    assert_eq!(resolve_toolchain_segment(DEV_TOOLCHAIN_ALIAS), "latest");
    assert_eq!(resolve_toolchain_segment("2026.6.1-abc1234"), "2026.6.1-abc1234");
}

#[test]
fn resolve_env_generation_id_uses_workspace_json_version() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    write_ws(ws, "20260228");
    let mut links = LinksState::default();
    links.toolchain.active = Some("2.0.1".into());
    write_links_state(ws, &links).expect("write links");
    assert_eq!(resolve_env_generation_id(ws), "2.0.1-ws20260228");
}

#[test]
fn resolve_toolchain_version_prefers_workspace_pin_over_stale_links() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    fs::write(
        ws.join("workspace.json"),
        r#"{"schemaVersion":2,"workspace":{"version":"20260628"},"toolchain":{"pin":"2.0.2"}}"#,
    )
    .expect("write workspace.json");
    let mut links = LinksState::default();
    links.toolchain.active = Some("2.0.1".into());
    write_links_state(ws, &links).expect("write links");
    assert_eq!(resolve_toolchain_version(ws), "2.0.2");
    assert_eq!(resolve_env_generation_id(ws), "2.0.2-ws20260628");
}

#[test]
fn resolve_toolchain_version_prefers_cli_hint_over_stale_links_without_pin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    write_ws(ws, "20260628");
    let mut links = LinksState::default();
    links.toolchain.active = Some("2.0.1".into());
    write_links_state(ws, &links).expect("write links");
    assert_eq!(
        resolve_toolchain_version_with_hint(ws, Some("2.0.2")),
        "2.0.2"
    );
    assert_eq!(
        format_env_generation_id("2.0.2", "20260628"),
        "2.0.2-ws20260628"
    );
}

#[test]
fn normalize_env_generation_id_upgrades_legacy_toolchain_only_ids() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    write_ws(ws, "20260228");
    assert_eq!(
        normalize_env_generation_id(ws, "latest"),
        "latest-ws20260228"
    );
    assert_eq!(
        normalize_env_generation_id(ws, "20260627T232726-latest"),
        "latest-ws20260228"
    );
    assert_eq!(
        normalize_env_generation_id(ws, "2.0.1-ws20260228"),
        "2.0.1-ws20260228"
    );
}

#[test]
fn merge_build_content_store_copies_missing_blobs_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = tmp.path().join("apps/demo");
    let from = app_env_build_dir(&app, "a-ws1");
    let to = app_env_build_dir(&app, "b-ws1");
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
    write_ws(ws, "1");
    let app = ws.join("apps/zhifa");
    fs::create_dir_all(app_env_build_dir(&app, "old-ws1").join("store/content/scene_payload"))
        .expect("mkdir old");
    fs::create_dir_all(app_env_build_dir(&app, "new-ws1").join("store/content/scene_payload"))
        .expect("mkdir new");
    fs::write(
        app_env_build_dir(&app, "old-ws1").join("store/content/scene_payload/old-only.json"),
        br#"{"old":true}"#,
    )
    .expect("write old blob");
    fs::write(
        app_env_build_dir(&app, "new-ws1").join("store/content/scene_payload/new-only.json"),
        br#"{"new":true}"#,
    )
    .expect("write new blob");
    let mut links = LinksState::default();
    links.build.active = Some("new-ws1".into());
    write_links_state(ws, &links).expect("write links");
    let promoted = promote_build(ws, Some("new-ws1")).expect("promote");
    assert_eq!(promoted, "new-ws1");
    assert!(
        app_env_build_dir(&app, "new-ws1")
            .join("store/content/scene_payload/old-only.json")
            .is_file(),
        "promote should union historical CAS into active env build"
    );
}

#[test]
fn attach_build_generation_creates_active_symlinks_to_env() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    write_ws(ws, "20260228");
    fs::create_dir_all(ws.join("apps/demo/src")).expect("mkdir app");
    fs::write(
        ws.join("apps/demo/app.config.json"),
        r#"{"schemaVersion":1,"entry":{"main":"main.mei"}}"#,
    )
    .expect("write app config");
    let mut links = LinksState::default();
    links.toolchain.active = Some(DEV_TOOLCHAIN_ALIAS.into());
    write_links_state(ws, &links).expect("write links");
    let gen = prepare_dev_build_generation(ws, &[String::from("demo")]).expect("prepare");
    assert_eq!(gen.env_version, "latest-ws20260228");
    let active = ws.join("apps/demo/build/active");
    assert!(active.is_symlink(), "build/active should be symlink");
    assert!(app_env_build_dir(&ws.join("apps/demo"), "latest-ws20260228").is_dir());
}

#[test]
fn replace_env_generation_reuses_same_composite_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    write_ws(ws, "20260228");
    fs::create_dir_all(ws.join("apps/demo/src")).expect("mkdir app");
    fs::write(
        ws.join("apps/demo/app.config.json"),
        r#"{"schemaVersion":1,"entry":{"main":"main.mei"}}"#,
    )
    .expect("write app config");
    let mut links = LinksState::default();
    links.toolchain.active = Some(DEV_TOOLCHAIN_ALIAS.into());
    write_links_state(ws, &links).expect("write links");
    let first = prepare_dev_build_generation(ws, &[String::from("demo")]).expect("prepare1");
    fs::write(
        app_env_build_dir(&ws.join("apps/demo"), "latest-ws20260228").join("marker.txt"),
        b"1",
    )
    .expect("marker");
    let second = prepare_dev_build_generation(ws, &[String::from("demo")]).expect("prepare2");
    assert_eq!(first.env_version, second.env_version);
    let env_dirs: Vec<_> = fs::read_dir(app_env_root(&ws.join("apps/demo")))
        .expect("read env")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(env_dirs, vec!["latest-ws20260228".to_string()]);
    assert!(
        !app_env_build_dir(&ws.join("apps/demo"), "latest-ws20260228")
            .join("marker.txt")
            .exists(),
        "replace should wipe prior build tree"
    );
}

#[test]
fn migrate_build_var_store_to_env_moves_legacy_layout() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    write_ws(ws, "20260228");
    let app = ws.join("apps/demo");
    let legacy_id = "20260628T120000-latest";
    fs::create_dir_all(app.join("build/store").join(legacy_id).join("exchange")).expect("mkdir");
    fs::write(
        app.join("build/store")
            .join(legacy_id)
            .join("exchange/demo.meibundle"),
        b"bundle",
    )
    .expect("write");
    let report = migrate_build_var_store_to_env(ws, app.as_path()).expect("migrate");
    assert_eq!(report.migrated_build_dirs, 1);
    assert!(
        !app.join("build/store").exists(),
        "legacy build/store should be removed after migrate"
    );
    assert!(
        app_env_build_dir(&app, "latest-ws20260228")
            .join("exchange/demo.meibundle")
            .is_file()
    );
}

#[test]
fn upgrade_non_composite_env_dir_during_migrate() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    write_ws(ws, "20260228");
    let app = ws.join("apps/demo");
    fs::create_dir_all(app_env_build_dir(&app, "latest").join("exchange")).expect("mkdir");
    fs::write(
        app_env_build_dir(&app, "latest")
            .join("exchange/demo.meibundle"),
        b"x",
    )
    .expect("write");
    let report = migrate_build_var_store_to_env(ws, app.as_path()).expect("migrate");
    assert!(report.upgraded_env_dirs.iter().any(|s| s.contains("latest")));
    assert!(app_env_build_dir(&app, "latest-ws20260228")
        .join("exchange/demo.meibundle")
        .is_file());
    assert!(!app_env_dir(&app, "latest").exists());
}

#[test]
fn migrate_flat_build_to_store_moves_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = tmp.path().join("apps/demo");
    fs::create_dir_all(app.join("build/active/exchange")).expect("mkdir");
    fs::write(app.join("build/active/exchange/demo.meibundle"), b"bundle").expect("write");
    let env_version = "2.0.1-ws20260228";
    assert!(migrate_flat_build_to_store(app.as_path(), env_version).expect("migrate"));
    assert!(app.join("build/active").is_symlink());
    assert!(
        app_env_build_dir(&app, env_version)
            .join("exchange/demo.meibundle")
            .is_file()
    );
}

#[test]
fn clean_env_generations_respects_links_protected_vers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    write_ws(ws, "20260228");
    fs::create_dir_all(ws.join("apps/demo/src")).expect("mkdir app");
    fs::write(
        ws.join("apps/demo/app.config.json"),
        r#"{"schemaVersion":1,"entry":{"main":"main.mei"}}"#,
    )
    .expect("write app config");
    fs::create_dir_all(app_env_dir(&ws.join("apps/demo"), "latest-ws20260228")).expect("mkdir");
    fs::create_dir_all(app_env_dir(&ws.join("apps/demo"), "2.0.1-ws20260301")).expect("mkdir");
    let mut links = LinksState::default();
    links.build.active = Some("latest-ws20260228".into());
    write_links_state(ws, &links).expect("write links");
    let report = clean_env_generations(
        ws,
        &[String::from("demo")],
        &CleanEnvPolicy { dry_run: true },
    )
    .expect("clean");
    assert!(report.removed.iter().any(|l| l.contains("2.0.1-ws20260301")));
    assert!(report.retained.iter().any(|l| l.contains("latest-ws20260228")));
}

#[test]
fn resolve_build_footer_label_shows_composite_active_build() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
    write_ws(ws, "20260228");
    let mut links = LinksState::default();
    links.toolchain.active = Some("2.0.1".into());
    links.build.active = Some("2.0.1-ws20260228".into());
    write_links_state(ws, &links).expect("write");
    let label = resolve_build_footer_label(ws);
    assert!(label.contains("MeiLang 2.0.1"));
    assert!(label.contains("WS 20260228"));
    assert!(label.contains("build 2.0.1-ws20260228"));
}

#[test]
fn links_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
    write_ws(ws, "1");
    let mut links = LinksState::default();
    links.build.candidate = Some("latest-ws1".into());
    write_links_state(ws, &links).expect("write");
    let loaded = read_links_state(ws).expect("read");
    assert_eq!(loaded.build.candidate.as_deref(), Some("latest-ws1"));
}
