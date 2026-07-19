//! Negative conformance: diagnostic codes + file-level source_path.

use std::fs;
use std::path::PathBuf;

use mei_host_graph::assemble_scope_from_registry;
use mei_lang_kernel::{
    build_runtime_warmup_manifest, take_warmup_build_diagnostics, Diagnostic, Severity,
};
use mei_test_support::{
    conformance_workspace, ensure_imported, mei_lang_root, APP_DIAG_FILTER_KEY,
    APP_DIAG_GRID_TRACK, APP_DIAG_LINK_PARAM, APP_DIAG_LINK_TARGET, APP_DIAG_UNKNOWN_COMPONENT,
    APP_DIAG_WARMUP_FOCUS,
};

fn assert_has_diag(diags: &[Diagnostic], code: &str, source_substr: &str) {
    let matched: Vec<_> = diags.iter().filter(|d| d.code == code).collect();
    assert!(
        !matched.is_empty(),
        "expected diagnostic code `{code}`, got: {:?}",
        diags
            .iter()
            .map(|d| format!("{}:{:?}", d.code, d.source_path))
            .collect::<Vec<_>>()
    );
    assert!(
        matched.iter().any(|d| {
            d.severity == Severity::Error
                && d.source_path
                    .as_deref()
                    .is_some_and(|p| p.contains(source_substr))
        }),
        "expected `{code}` with source_path containing `{source_substr}`, got: {:?}",
        matched
            .iter()
            .map(|d| format!("{:?} {:?}", d.severity, d.source_path))
            .collect::<Vec<_>>()
    );
}

fn assemble_diags(app_id: &str) -> Vec<Diagnostic> {
    let workspace = ensure_imported(app_id);
    let outcome = assemble_scope_from_registry(workspace.as_path(), app_id, "home")
        .expect("assemble")
        .expect("home outcome");
    outcome.compiled.diagnostics
}

#[test]
fn conformance_diag_link_target_missing() {
    let diags = assemble_diags(APP_DIAG_LINK_TARGET);
    assert_has_diag(&diags, "link_decl_target_missing", "links.mei");
}

#[test]
fn conformance_diag_link_param() {
    let diags = assemble_diags(APP_DIAG_LINK_PARAM);
    assert_has_diag(&diags, "link_decl_param_missing", "links.mei");
    assert_has_diag(&diags, "link_decl_param_type_mismatch", "links.mei");
}

#[test]
fn conformance_diag_grid_track_unresolved() {
    let diags = assemble_diags(APP_DIAG_GRID_TRACK);
    assert_has_diag(&diags, "grid_track_unresolved", "plane.mei");
}

#[test]
fn conformance_diag_unknown_component() {
    let diags = assemble_diags(APP_DIAG_UNKNOWN_COMPONENT);
    assert_has_diag(&diags, "unknown_component", "section.mei");
}

#[test]
fn conformance_diag_filter_key_mismatch() {
    let diags = assemble_diags(APP_DIAG_FILTER_KEY);
    assert_has_diag(
        &diags,
        "row_drilldown_filter_key_mismatch",
        "plane-analytics.mei",
    );
}

#[test]
fn conformance_diag_warmup_focus_not_found() {
    // Isolated workspace so a missing focus does not poison shared conformance warmup.
    let package = mei_lang_root();
    let fixture_app = package
        .join("tests/fixtures/ws-conformance/apps")
        .join(APP_DIAG_WARMUP_FOCUS);
    assert!(
        fixture_app.is_dir(),
        "missing fixture app {}",
        fixture_app.display()
    );
    let dest = std::env::temp_dir().join(format!(
        "mei-ws-diag-warmup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let _ = fs::remove_dir_all(&dest);
    fs::create_dir_all(dest.join("apps")).expect("apps dir");
    copy_dir_recursive(&fixture_app, &dest.join("apps").join(APP_DIAG_WARMUP_FOCUS));
    fs::write(
        dest.join("workspace.json"),
        serde_json::json!({
            "schemaVersion": 2,
            "workspace": { "id": "ws-diag-warmup", "defaultApp": APP_DIAG_WARMUP_FOCUS },
            "paths": { "apps": "apps", "components": "stock/components", "templates": "stock/templates" },
            "warmup": {
                "enabled": true,
                "apps": {
                    APP_DIAG_WARMUP_FOCUS: {
                        "hotScenes": ["home"],
                        "focuses": ["src/warmup/missing-focus.mei"]
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("workspace.json");
    let _ = take_warmup_build_diagnostics();
    let err = build_runtime_warmup_manifest(dest.as_path()).expect_err("missing focus must fail");
    assert!(
        err.to_string().contains("warmup_focus_not_found"),
        "unexpected error: {err:#}"
    );
    let diags = take_warmup_build_diagnostics();
    assert_has_diag(&diags, "warmup_focus_not_found", "missing-focus.mei");
    let _ = fs::remove_dir_all(&dest);
    // Keep shared conformance fixture discoverable (must exist).
    let _shared: PathBuf = conformance_workspace();
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).unwrap_or_else(|e| panic!("create {}: {e}", dst.display()));
    for entry in fs::read_dir(src).unwrap_or_else(|e| panic!("read {}: {e}", src.display())) {
        let entry = entry.expect("entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap_or_else(|e| {
                panic!("copy {} -> {}: {e}", path.display(), target.display());
            });
        }
    }
}
