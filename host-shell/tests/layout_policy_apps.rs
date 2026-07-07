//! Strict layout policy compile gate for ws-demo-v2 cockpit apps.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_kernel::Severity;

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn mei_lang_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("mei-lang root")
}

fn bundle_for(app_id: &str) -> PathBuf {
    ws_demo_v2().join(format!(
        "apps/{app_id}/env/current/build/exchange/{app_id}.meibundle"
    ))
}

fn ensure_imported(app_id: &str) {
    let workspace = ws_demo_v2();
    let bundle = bundle_for(app_id);
    assert!(
        bundle.is_file(),
        "run `mei-compiler compile --workspace ws-demo-v2 --app {app_id}` first"
    );
    let ctx = HostContext::new(workspace.clone(), app_id);
    import_bundle(
        &ctx,
        &ImportOptions {
            bundle_path: Some(bundle),
        },
    )
    .unwrap_or_else(|err| panic!("import {app_id}: {err}"));
}

fn assert_home_assembles_without_layout_policy_errors(app_id: &str) {
    ensure_imported(app_id);
    let outcome = assemble_scope_from_registry(ws_demo_v2().as_path(), app_id, "home")
        .unwrap_or_else(|err| panic!("assemble {app_id}: {err}"))
        .unwrap_or_else(|| panic!("missing home outcome for {app_id}"));
    let layout_errors: Vec<_> = outcome
        .compiled
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error && d.code.starts_with("layout_policy_"))
        .collect();
    assert!(
        layout_errors.is_empty(),
        "{app_id} home layout_policy errors: {layout_errors:?}"
    );
}

#[test]
fn pretty_panels_home_strict_layout_policy_clean() {
    INIT.call_once(|| ensure_imported("pretty-panels"));
    assert_home_assembles_without_layout_policy_errors("pretty-panels");
}

#[test]
fn data_demo_home_strict_layout_policy_clean() {
    assert_home_assembles_without_layout_policy_errors("data-demo");
}

#[test]
fn mini_park_home_strict_layout_policy_clean() {
    assert_home_assembles_without_layout_policy_errors("mini-park");
}

#[test]
fn ws_demo_v2_apps_compile_with_strict_layout_policy() {
    let workspace = ws_demo_v2();
    let manifest_dir = mei_lang_root();
    for app_id in ["pretty-panels", "data-demo", "mini-park"] {
        let status = Command::new("cargo")
            .current_dir(&manifest_dir)
            .args([
                "run",
                "-p",
                "mei-compiler",
                "--",
                "compile",
                "--workspace",
                workspace.to_str().expect("workspace path"),
                "--app",
                app_id,
            ])
            .status()
            .unwrap_or_else(|err| panic!("spawn mei-compiler for {app_id}: {err}"));
        assert!(status.success(), "mei-compiler compile failed for {app_id}");
    }
}
