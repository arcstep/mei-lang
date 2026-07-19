use std::fs;
use std::path::{Path, PathBuf};

use mei_lang_kernel::{
    discover_app_admin_resources, AdminDiscoverOutcome, AdminRegistryProjection,
};
use mei_test_support::{mei_lang_root, APP_ADMIN_MEI};

fn fixture_copy() -> tempfile::TempDir {
    let source = mei_lang_root()
        .join("tests/fixtures/ws-conformance/apps")
        .join(APP_ADMIN_MEI);
    assert!(source.is_dir(), "missing fixture {}", source.display());
    let temp = tempfile::tempdir().expect("temp fixture root");
    copy_dir_recursive(&source, temp.path());
    temp
}

fn projection(root: &Path) -> AdminRegistryProjection {
    match discover_app_admin_resources(root, APP_ADMIN_MEI) {
        AdminDiscoverOutcome::Ok(projection) => projection,
        outcome => panic!("Admin discovery failed: {outcome:?}"),
    }
}

fn replace(path: PathBuf, before: &str, after: &str) {
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert!(
        source.contains(before),
        "{} does not contain mutation marker `{before}`",
        path.display()
    );
    fs::write(&path, source.replacen(before, after, 1))
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

#[test]
fn conformance_admin_digests_are_orthogonal_on_fixture_copies() {
    let governance = fixture_copy();
    let before = projection(governance.path());
    replace(
        governance.path().join("src/admin/demo/overview.mdx"),
        "audit: true",
        "audit: false",
    );
    let after = projection(governance.path());
    assert_ne!(before.admin_registry_digest, after.admin_registry_digest);
    assert_eq!(before.page_structure_digest, after.page_structure_digest);

    let help = fixture_copy();
    let before = projection(help.path());
    replace(
        help.path().join("src/admin/demo/overview.mdx"),
        "这里维护 fixture 的单位资料",
        "这里维护 fixture 的组织资料",
    );
    let after = projection(help.path());
    assert_eq!(before.admin_registry_digest, after.admin_registry_digest);
    assert_ne!(before.page_structure_digest, after.page_structure_digest);

    let scene = fixture_copy();
    let before = projection(scene.path());
    replace(
        scene.path().join("src/scene/admin/demo/overview.mei"),
        "Admin v2 conformance page",
        "Admin v2 conformance page changed",
    );
    let after = projection(scene.path());
    assert_eq!(before.admin_registry_digest, after.admin_registry_digest);
    assert_ne!(before.page_structure_digest, after.page_structure_digest);

    let data = fixture_copy();
    let before = projection(data.path());
    replace(
        data.path().join("src/data/admin/demo/overview.mei"),
        "admin.demo.organization\")",
        "admin.demo.organization.v2\")",
    );
    let after = projection(data.path());
    assert_eq!(before.admin_registry_digest, after.admin_registry_digest);
    assert_ne!(before.page_structure_digest, after.page_structure_digest);

    let narration = fixture_copy();
    let before = projection(narration.path());
    let track = narration.path().join("src/narration/admin.track.mdx");
    fs::create_dir_all(track.parent().expect("track parent")).expect("create narration dir");
    fs::write(
        track,
        "---\nid: admin-demo\ntitle: Admin demo\nscope: app\n---\n\nTrack prose.\n",
    )
    .expect("write optional track");
    let after = projection(narration.path());
    assert_eq!(before.admin_registry_digest, after.admin_registry_digest);
    assert_eq!(before.page_structure_digest, after.page_structure_digest);
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap_or_else(|error| panic!("create {}: {error}", dst.display()));
    for entry in fs::read_dir(src).unwrap_or_else(|error| panic!("read {}: {error}", src.display()))
    {
        let entry = entry.expect("fixture entry");
        let source = entry.path();
        let target = dst.join(entry.file_name());
        if source.is_dir() {
            copy_dir_recursive(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap_or_else(|error| {
                panic!("copy {} to {}: {error}", source.display(), target.display())
            });
        }
    }
}
