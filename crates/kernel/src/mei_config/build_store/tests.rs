use super::*;
use std::fs;
use std::path::Path;

fn write_ws(root: &Path, ws_version: &str) {
    fs::write(
        root.join("workspace.json"),
        format!(
            r#"{{"schemaVersion":2,"workspace":{{"version":"{ws_version}"}},"build":{{"generation":{{"dateSource":"manual","date":"{ws_version}","fixver":0}}}}}}"#
        ),
    )
    .expect("write workspace.json");
}

fn write_ws_with_default_app(root: &Path, ws_version: &str, app_id: &str) {
    fs::write(
        root.join("workspace.json"),
        format!(
            r#"{{"schemaVersion":2,"workspace":{{"version":"{ws_version}","defaultApp":"{app_id}"}},"build":{{"generation":{{"dateSource":"manual","date":"{ws_version}","fixver":0}}}}}}"#
        ),
    )
    .expect("write workspace.json");
}

fn setup_demo_app(root: &Path) {
    fs::create_dir_all(root.join("apps/demo/src")).expect("mkdir app");
    fs::write(
        root.join("apps/demo/app.config.json"),
        r#"{"schemaVersion":1,"entry":{"main":"main.mei"}}"#,
    )
    .expect("write app config");
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
    assert_eq!(resolve_env_generation_id(ws), "WS-20260228.0");
}

#[test]
fn resolve_toolchain_version_prefers_workspace_pin_over_stale_links() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    fs::write(
        ws.join("workspace.json"),
        r#"{"schemaVersion":2,"workspace":{"version":"20260628"},"toolchain":{"pin":"2.0.2"},"build":{"generation":{"dateSource":"manual","date":"20260628","fixver":0}}}"#,
    )
    .expect("write workspace.json");
    let mut links = LinksState::default();
    links.toolchain.active = Some("2.0.1".into());
    write_links_state(ws, &links).expect("write links");
    assert_eq!(resolve_toolchain_version(ws), "2.0.2");
    assert_eq!(resolve_env_generation_id(ws), "WS-20260628.0");
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
}

#[test]
fn normalize_env_generation_id_accepts_ws_tags_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    write_ws(ws, "20260228");
    assert!(normalize_env_generation_id(ws, "latest").is_err());
    assert!(normalize_env_generation_id(ws, "2.0.1-ws20260228").is_err());
    assert_eq!(
        normalize_env_generation_id(ws, "WS-20260228.1").expect("ok"),
        "WS-20260228.1"
    );
    assert_eq!(
        normalize_env_generation_id(ws, "").expect("empty uses config"),
        "WS-20260228.0"
    );
}

#[test]
fn merge_build_content_store_copies_missing_blobs_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = tmp.path().join("apps/demo");
    let from = app_env_build_dir(&app, "WS-20260201.0");
    let to = app_env_build_dir(&app, "WS-20260202.0");
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
    write_ws(ws, "20260201");
    let app = ws.join("apps/zhifa");
    fs::create_dir_all(app_env_build_dir(&app, "WS-20260201.0").join("store/content/scene_payload"))
        .expect("mkdir old");
    fs::create_dir_all(app_env_build_dir(&app, "WS-20260202.0").join("store/content/scene_payload"))
        .expect("mkdir new");
    fs::write(
        app_env_build_dir(&app, "WS-20260201.0").join("store/content/scene_payload/old-only.json"),
        br#"{"old":true}"#,
    )
    .expect("write old blob");
    fs::write(
        app_env_build_dir(&app, "WS-20260202.0").join("store/content/scene_payload/new-only.json"),
        br#"{"new":true}"#,
    )
    .expect("write new blob");
    attach_build_generation(ws, &[String::from("zhifa")], "WS-20260202.0").expect("attach current");
    let promoted = promote_build(ws, Some("WS-20260202.0")).expect("promote");
    assert_eq!(promoted, "WS-20260202.0");
    assert!(
        app_env_build_dir(&app, "WS-20260202.0")
            .join("store/content/scene_payload/old-only.json")
            .is_file(),
        "promote should union historical CAS into active env build"
    );
}

#[test]
fn attach_build_generation_creates_env_current_symlink() {
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
    assert_eq!(gen.env_version, "WS-20260228.0");
    assert_eq!(gen.build_generation, "WS-20260228.0");
    let app = ws.join("apps/demo");
    let current = app.join("env/current");
    assert!(current.is_symlink(), "env/current should be symlink");
    assert!(app_env_build_dir(&app, "WS-20260228.0").is_dir());
}

#[test]
fn replace_env_generation_reuses_same_build_generation_directory() {
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
        app_env_build_dir(&ws.join("apps/demo"), "WS-20260228.0").join("marker.txt"),
        b"1",
    )
    .expect("marker");
    let second = prepare_dev_build_generation(ws, &[String::from("demo")]).expect("prepare2");
    assert_eq!(first.env_version, second.env_version);
    let env_dirs: Vec<_> = fs::read_dir(app_env_root(&ws.join("apps/demo")))
        .expect("read env")
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name == "current" {
                None
            } else if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(env_dirs, vec!["WS-20260228.0".to_string()]);
    assert!(
        !app_env_build_dir(&ws.join("apps/demo"), "WS-20260228.0")
            .join("marker.txt")
            .exists(),
        "replace should wipe prior build tree"
    );
}

#[test]
fn migrate_build_var_store_to_env_rejects_legacy_layout() {
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
    let err = migrate_build_var_store_to_env(ws, app.as_path()).expect_err("legacy");
    assert!(err.to_string().contains("legacy build/store"));
}

#[test]
fn migrate_build_var_store_to_env_rejects_non_ws_env_dirs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    write_ws(ws, "20260228");
    let app = ws.join("apps/demo");
    fs::create_dir_all(app_env_build_dir(&app, "latest").join("exchange")).expect("mkdir");
    let err = migrate_build_var_store_to_env(ws, app.as_path()).expect_err("legacy env");
    assert!(err.to_string().contains("legacy env directories"));
}

#[test]
fn migrate_flat_build_to_store_rejects_flat_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let app = tmp.path().join("apps/demo");
    fs::create_dir_all(app.join("build/active/exchange")).expect("mkdir");
    fs::write(app.join("build/active/exchange/demo.meibundle"), b"bundle").expect("write");
    let err = migrate_flat_build_to_store(app.as_path(), "WS-20260228.0").expect_err("flat");
    assert!(err.to_string().contains("legacy flat build/active"));
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
    fs::create_dir_all(app_env_dir(&ws.join("apps/demo"), "WS-20260228.0")).expect("mkdir");
    fs::create_dir_all(app_env_dir(&ws.join("apps/demo"), "WS-20260301.0")).expect("mkdir");
    attach_build_generation(ws, &[String::from("demo")], "WS-20260228.0").expect("attach current");
    let report = clean_env_generations(
        ws,
        &[String::from("demo")],
        &CleanEnvPolicy { dry_run: true },
    )
    .expect("clean");
    assert!(report.removed.iter().any(|l| l.contains("WS-20260301.0")));
    assert!(report.retained.iter().any(|l| l.contains("WS-20260228.0")));
}

#[test]
fn resolve_build_footer_label_shows_meilang_and_build_generation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
    write_ws_with_default_app(ws, "20260228", "demo");
    setup_demo_app(ws);
    attach_build_generation(ws, &[String::from("demo")], "WS-20260228.0").expect("attach");
    let mut links = LinksState::default();
    links.toolchain.active = Some("2.0.1".into());
    write_links_state(ws, &links).expect("write");
    let label = resolve_build_footer_label(ws);
    assert!(label.contains("MeiLang 2.0.1"));
    assert!(label.contains("Build WS-20260228.0"));
}

#[test]
fn resolve_build_footer_label_ignores_deprecated_links_build_active() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
    write_ws_with_default_app(ws, "20260228", "demo");
    setup_demo_app(ws);
    attach_build_generation(ws, &[String::from("demo")], "WS-20260228.0").expect("attach");
    fs::write(
        ws.join("deploy/state/links.json"),
        r#"{"schemaVersion":"mei-workspace-links-v1","toolchain":{"active":"2.0.1"},"build":{"active":"2.0.1-ws20260228","candidate":null,"previous":null}}"#,
    )
    .expect("write legacy links");
    let label = resolve_build_footer_label(ws);
    assert!(label.contains("Build WS-20260228.0"));
    assert!(!label.contains("2.0.1-ws20260228"));
}

#[test]
fn resolve_workspace_app_build_generations_reads_env_current() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir deploy");
    write_ws_with_default_app(ws, "20260228", "demo");
    setup_demo_app(ws);
    attach_build_generation(ws, &[String::from("demo")], "WS-20260228.0").expect("attach");
    let gens = resolve_workspace_app_build_generations(ws, &[String::from("demo")]).expect("gens");
    assert_eq!(gens.get("demo").map(String::as_str), Some("WS-20260228.0"));
}

#[test]
fn links_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
    write_ws(ws, "20260201");
    let mut links = LinksState::default();
    links.build.candidate = Some("WS-20260201.0".into());
    write_links_state(ws, &links).expect("write");
    let loaded = read_links_state(ws).expect("read");
    assert_eq!(loaded.build.candidate.as_deref(), Some("WS-20260201.0"));
}
