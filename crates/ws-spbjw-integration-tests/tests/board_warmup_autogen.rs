use mei_lang_kernel::{build_runtime_warmup_manifest, resolve_app_root};
use ws_spbjw_integration_tests::{source_root, zhifa_app_root};

#[test]
fn runtime_warmup_manifest_includes_board_derived_penalty_analytics() {
    let source_root = source_root();
    let manifest = build_runtime_warmup_manifest(source_root.as_path())
        .expect("build runtime warmup manifest");
    let zhifa = manifest
        .apps
        .iter()
        .find(|app| app.app_id == "zhifa")
        .expect("zhifa app in manifest");
    assert!(
        zhifa.datasets.iter().any(|entry| {
            entry.scene_id.as_deref() == Some("penalty_total_analytics_board")
                && entry.dataset_id == "penalty_result_dashboard_ds"
        }),
        "expected board autogen entry for penalty_total_analytics_board, got: {:?}",
        zhifa
            .datasets
            .iter()
            .map(|entry| (entry.scene_id.as_deref(), entry.dataset_id.as_str()))
            .collect::<Vec<_>>()
    );
    let _app_root = zhifa_app_root();
    let _ = resolve_app_root(source_root.as_path(), "zhifa");
}
