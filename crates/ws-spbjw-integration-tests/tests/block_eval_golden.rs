//! Golden block-eval tests for ws-spbjw home embedded world_metrics capsules.

use mei_lang_server::{block_eval, materialize_worksets, BlockEvalRequest, PrebuildMode};
use ws_spbjw_integration_tests::source_root;

fn home_wm_owner(scene_file: &str) -> String {
    format!("__world_metrics__::scenes/{scene_file}::metrics")
}

#[test]
fn t_block_eval_home_wm_03() {
    let source_root = source_root();
    let owner = home_wm_owner("03-指标体系.mei");
    let report = materialize_worksets(
        source_root.as_path(),
        "zhifa",
        Some("home"),
        Some("src/scenes/home.mei"),
        owner.as_str(),
        &[],
        PrebuildMode::Build,
    )
    .expect("block materialize_worksets");
    assert!(
        report.ok,
        "home wm 03 eval failed: {}",
        report.error_chain.as_deref().unwrap_or("unknown")
    );
}

#[test]
fn t_block_eval_home_wm_05() {
    let source_root = source_root();
    let owner = home_wm_owner("05-监督预警.mei");
    let report = block_eval(BlockEvalRequest {
        source_root: source_root.clone(),
        app_id: "zhifa".to_string(),
        scene_id: Some("home".to_string()),
        target_file: Some("src/scenes/home.mei".to_string()),
        owner_resource_id: owner,
        metric_ids: vec![],
    })
    .expect("block eval request");
    assert!(
        report.ok,
        "home wm 05 eval failed: {}",
        report.error_chain.as_deref().unwrap_or("unknown")
    );
}

#[test]
fn t_block_eval_home_wm_08() {
    let source_root = source_root();
    let owner = home_wm_owner("08-监督成效.mei");
    let report = materialize_worksets(
        source_root.as_path(),
        "zhifa",
        Some("home"),
        Some("src/scenes/home.mei"),
        owner.as_str(),
        &["effectiveness_transfer_clue_count".to_string()],
        PrebuildMode::Build,
    )
    .expect("block materialize_worksets");
    assert!(
        report.ok,
        "home wm 08 eval failed: {}",
        report.error_chain.as_deref().unwrap_or("unknown")
    );
}

#[test]
fn t_block_eval_qunfu_home_zero_warning_regression() {
    let source_root = source_root();
    let report = materialize_worksets(
        source_root.as_path(),
        "qunfu",
        None,
        Some("src/scenes/home.mei"),
        "__world_metrics__",
        &[],
        PrebuildMode::Build,
    )
    .expect("qunfu home block eval");
    assert!(
        report.ok,
        "qunfu home wm eval failed: {}",
        report.error_chain.as_deref().unwrap_or("unknown")
    );
}

#[test]
fn t_block_eval_card_board_warning_list() {
    let source_root = source_root();
    let report = materialize_worksets(
        source_root.as_path(),
        "qunfu",
        None,
        Some("src/scenes/_shared/warning-detail.card.board.mei"),
        "warning_list",
        &[],
        PrebuildMode::Build,
    )
    .expect("qunfu warning_list card board block eval");
    assert!(
        report.ok,
        "qunfu warning_list card board eval failed: {}",
        report.error_chain.as_deref().unwrap_or("unknown")
    );
}
