//! Build overview layoutTuning diff via enriched assemble.

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{assemble_scope_from_registry, import_bundle, ImportOptions};
use mei_lang_kernel::{
    build_ui_layout_index, format_layout_tuning_diff, load_mei_config_for_app, BuildNodeId,
    UiScopeRole,
};

static INIT: Once = Once::new();

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn ensure_pretty_panels_imported() -> PathBuf {
    let workspace = ws_demo_v2();
    INIT.call_once(|| {
        let bundle = workspace.join("apps/pretty-panels/env/current/build/exchange/pretty-panels.meibundle");
        let ctx = HostContext::new(workspace.clone(), "pretty-panels");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import pretty-panels bundle");
    });
    workspace
}

#[test]
fn pretty_panels_enriched_assemble_reports_layout_tuning_padding_diff() {
    let workspace = ensure_pretty_panels_imported();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "pretty-panels", "home")
        .expect("assemble")
        .expect("home outcome");
    let mut compiled = outcome.compiled;
    assert!(
        !compiled.ui_layout_index.nodes.is_empty(),
        "assemble should populate ui_layout_index before diff"
    );
    let node = BuildNodeId::ui_scope("home", "home/T1/left_rail/enforcement");
    let entry = compiled
        .ui_layout_index
        .lookup(&node)
        .expect("ui scope entry for enforcement section");
    assert_eq!(entry.role, UiScopeRole::Section);
    assert_eq!(entry.preview_scope, "left_rail/enforcement");
    let config = load_mei_config_for_app(
        std::path::Path::new(compiled.app_root.as_str()),
        Some(workspace.as_path()),
    );
    let diff = format_layout_tuning_diff(
        entry.preview_scope.as_str(),
        entry.budget.as_ref(),
        config.ops.layout_tuning.as_ref(),
    )
    .expect("layout tuning diff");
    assert!(
        diff.contains("padding_profile"),
        "diff should mention padding_profile: {diff}"
    );
    assert!(
        diff.contains("compact"),
        "diff should include config padding profile: {diff}"
    );
    let ui = build_ui_layout_index(&compiled);
    assert!(
        ui.index
            .lookup(&node)
            .is_some_and(|n| n.preview_scope == "left_rail/enforcement"),
        "rebuilt index should stay aligned with assemble index"
    );
}
