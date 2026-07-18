//! Phase B admin platform fixture smoke (kernel-level; HTTP covered in host unit tests).

use mei_lang_kernel::{
    discover_app_admin_resources, get_config_record, put_config_record, AdminDiscoverOutcome,
};

fn fixture_app_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/admin-loop-app")
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

#[test]
fn admin_loop_fixture_discovers_organization() {
    let root = fixture_app_root();
    assert!(
        root.join("app.toml").is_file(),
        "missing admin-loop-app fixture at {}",
        root.display()
    );
    match discover_app_admin_resources(&root, "admin-loop-app") {
        AdminDiscoverOutcome::Ok(proj) => {
            assert!(proj
                .resources
                .iter()
                .any(|r| r.resource_id == "organization"));
        }
        other => panic!("expected Ok discovery, got {other:?}"),
    }
}

#[test]
fn admin_loop_config_record_round_trip_in_temp_copy() {
    let src = fixture_app_root();
    let dir = tempfile::tempdir().unwrap();
    let app_root = dir.path().join("admin-loop-app");
    copy_dir(&src, &app_root);

    let path = "admin/data/organization.json";
    let first = put_config_record(
        &app_root,
        path,
        0,
        serde_json::json!({"name": "测试单位", "contact": "张三"}),
        "test",
        "admin-loop-app",
        "organization",
        "c1",
    )
    .unwrap();
    assert_eq!(first.revision, 1);
    let loaded = get_config_record(&app_root, path).unwrap();
    assert_eq!(loaded.data["name"], "测试单位");

    let conflict = put_config_record(
        &app_root,
        path,
        0,
        serde_json::json!({"name": "覆盖"}),
        "test",
        "admin-loop-app",
        "organization",
        "c2",
    );
    assert!(conflict.is_err());
}
