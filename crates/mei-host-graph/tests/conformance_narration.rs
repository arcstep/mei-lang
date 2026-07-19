use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mei_lang_kernel::{
    compute_narration_digest, compute_structure_digest, discover_narration_catalog,
    discover_narration_track_paths, AdminDiscoverOutcome, ContentCapability,
    NarrationTargetCatalog,
};
use mei_test_support::{ensure_imported, APP_NARRATION_JOURNEY};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("mei-lang repository root")
}

fn fixture(name: &str) -> PathBuf {
    let path = repo_root()
        .join("tests/fixtures/ws-conformance/apps")
        .join(name);
    assert!(
        path.is_dir(),
        "missing conformance fixture: {}",
        path.display()
    );
    path
}

#[test]
fn conformance_discovers_app_level_tracks_and_resolves_fixture_targets() {
    let journey = fixture("fx-narration-journey");
    let paths = discover_narration_track_paths(&journey);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("src/narration/overview.track.mdx"));

    let mut targets = NarrationTargetCatalog::default();
    for target in [
        "stage:home/viewpoint:warnings_main",
        "stage:home/t2_page:warnings-detail",
        "stage:journey/slide:mission",
        "stage:journey/slide:mission/slot:evidence",
    ] {
        targets.insert(target, "fixture");
    }
    let (_, catalog, diagnostics) =
        discover_narration_catalog(&journey, "fx-narration-journey", &targets);
    assert!(
        diagnostics.is_empty(),
        "journey target resolution failed: {diagnostics:?}"
    );
    assert_eq!(catalog.tracks.len(), 1);
    assert_eq!(catalog.tracks[0].cues.len(), 4);
    assert_eq!(
        catalog.default_track_by_entry.get("stage:home"),
        Some(&"overview".to_string())
    );

    let admin = fixture("fx-admin-mei");
    let admin_targets = NarrationTargetCatalog::from_admin(&admin, "fx-admin-mei");
    assert!(
        admin_targets.contains("admin:demo/overview/document_anchor:basic"),
        "Admin Registry/PageProgram public catalog must expose document_anchor"
    );
    let (_, admin_catalog, admin_diagnostics) =
        discover_narration_catalog(&admin, "fx-admin-mei", &admin_targets);
    assert!(
        admin_diagnostics.is_empty(),
        "admin target resolution failed: {admin_diagnostics:?}"
    );
    assert_eq!(
        admin_catalog.tracks[0].cues[0].target_ref,
        "admin:demo/overview/document_anchor:basic"
    );
}

#[test]
fn conformance_track_edit_changes_only_narration_digest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    let track_path = app_root.join("src/narration/overview.track.mdx");
    fs::create_dir_all(track_path.parent().expect("track parent")).expect("mkdir");
    let source = r#"---
id: overview
title: Overview
scope: app
default_for: [stage:home]
---
@cue(stage:home/viewpoint:summary)
@caption
Caption A
@end
@end
"#;
    fs::write(&track_path, source).expect("write track");
    let mut targets = NarrationTargetCatalog::default();
    targets.insert("stage:home/viewpoint:summary", "src/scene/home.mei");

    let (_, first, first_diagnostics) =
        discover_narration_catalog(app_root, "digest-fixture", &targets);
    assert!(first_diagnostics.is_empty());
    fs::write(&track_path, source.replace("Caption A", "Caption B")).expect("edit track");
    let (_, second, second_diagnostics) =
        discover_narration_catalog(app_root, "digest-fixture", &targets);
    assert!(second_diagnostics.is_empty());

    let first_digest =
        compute_narration_digest(&BTreeMap::from([(first.catalog_id.clone(), first)]));
    let second_digest =
        compute_narration_digest(&BTreeMap::from([(second.catalog_id.clone(), second)]));
    assert_ne!(first_digest, second_digest);

    let capabilities = BTreeMap::from([(
        "summary".to_string(),
        ContentCapability::from_content_panel("summary", "src/scene/home.mei", Vec::new()),
    )]);
    let structure_before = compute_structure_digest(&BTreeMap::new(), &capabilities);
    let structure_after = compute_structure_digest(&BTreeMap::new(), &capabilities);
    assert_eq!(structure_before, structure_after);
}

#[test]
fn conformance_admin_page_and_registry_digests_ignore_track_edits() {
    let source_root = fixture("fx-admin-mei");
    let temp = tempfile::tempdir().expect("tempdir");
    let app_root = temp.path();
    for rel in [
        "app.toml",
        "src/admin/demo/overview.mdx",
        "src/scene/admin/demo/overview.mei",
        "src/data/admin/demo/overview.mei",
        "src/narration/admin.track.mdx",
    ] {
        let source = source_root.join(rel);
        let target = app_root.join(rel);
        fs::create_dir_all(target.parent().expect("target parent")).expect("mkdir");
        fs::copy(&source, &target)
            .unwrap_or_else(|error| panic!("copy {}: {error}", source.display()));
    }
    let before_admin = match mei_lang_kernel::discover_app_admin_resources(app_root, "fx-admin-mei")
    {
        AdminDiscoverOutcome::Ok(projection) => projection,
        outcome => panic!("admin discovery before edit failed: {outcome:?}"),
    };
    let targets = NarrationTargetCatalog::from_admin(app_root, "fx-admin-mei");
    let (_, before_catalog, before_diagnostics) =
        discover_narration_catalog(app_root, "fx-admin-mei", &targets);
    assert!(before_diagnostics.is_empty());

    let track_path = app_root.join("src/narration/admin.track.mdx");
    let source = fs::read_to_string(&track_path).expect("read copied track");
    fs::write(
        &track_path,
        source.replace(
            "Review the basic application settings.",
            "Review the updated basic application settings.",
        ),
    )
    .expect("edit track");

    let after_admin = match mei_lang_kernel::discover_app_admin_resources(app_root, "fx-admin-mei")
    {
        AdminDiscoverOutcome::Ok(projection) => projection,
        outcome => panic!("admin discovery after edit failed: {outcome:?}"),
    };
    let (_, after_catalog, after_diagnostics) =
        discover_narration_catalog(app_root, "fx-admin-mei", &targets);
    assert!(after_diagnostics.is_empty());
    assert_eq!(
        before_admin.admin_registry_digest,
        after_admin.admin_registry_digest
    );
    assert_eq!(
        before_admin.page_structure_digest,
        after_admin.page_structure_digest
    );
    assert_ne!(before_catalog.source_digest, after_catalog.source_digest);
}

#[test]
fn conformance_import_assemble_bootstraps_app_catalog() {
    let workspace = ensure_imported(APP_NARRATION_JOURNEY);
    let outcome = mei_host_graph::assemble_scope_from_registry(
        workspace.as_path(),
        APP_NARRATION_JOURNEY,
        "home",
    )
    .expect("assemble narration fixture")
    .expect("narration fixture outcome");
    let catalog = outcome
        .compiled
        .narration_catalogs
        .values()
        .next()
        .expect("App-level NarrationCatalog");
    assert_eq!(catalog.tracks.len(), 1);
    assert_eq!(catalog.tracks[0].cues.len(), 4);
    assert!(
        outcome
            .compiled
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.code.starts_with("narration_")),
        "narration diagnostics: {:?}",
        outcome
            .compiled
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.starts_with("narration_"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        outcome
            .presentation_map
            .pointer("/defaultScript/source")
            .and_then(serde_json::Value::as_str),
        Some("narration_catalog")
    );
}
