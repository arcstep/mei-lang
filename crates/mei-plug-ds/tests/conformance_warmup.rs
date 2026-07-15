//! Platform conformance: warmup + MRG memory tier on fx-data.

use mei_host_core::HostContext;
use mei_host_graph::WarmupTier;
use mei_plug_ds::{
    collect_warmup_targets, eval_metric_ids, load_compiled_for_warmup, run_warmup_targets_with_tier,
};
use mei_test_support::{ensure_imported, APP_DATA};

#[test]
fn conformance_warmup_mrg_memory() {
    let workspace = ensure_imported(APP_DATA);
    let ctx = HostContext::new(workspace.clone(), APP_DATA.to_string());
    let targets = collect_warmup_targets(&ctx, Some("home")).expect("warmup targets");
    assert!(
        !targets.is_empty(),
        "home warmup policy should define at least one target"
    );
    let report =
        run_warmup_targets_with_tier(&ctx, &targets, WarmupTier::All).expect("warmup tier all");
    assert!(report.slot_count > 0, "warmup should register MRG slots");
    let status =
        mei_host_graph::mrg_status_json(workspace.as_path(), APP_DATA).expect("mrg status");
    let memory_resident = status
        .get("slotsByTier")
        .and_then(|value| value.get("memoryResident"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    assert!(
        memory_resident > 0,
        "tier all should pin at least one memory resident slot"
    );
    assert!(
        report.client_manifest_written,
        "tier all should write client-bootstrap manifest"
    );
    let manifest = mei_host_graph::read_client_bootstrap(workspace.as_path(), APP_DATA, "home")
        .expect("home client-bootstrap manifest");
    assert!(
        !manifest.metrics.is_empty(),
        "home manifest should cover at least one metric"
    );

    let target = &targets[0];
    let (compiled, compile_revision) =
        load_compiled_for_warmup(&ctx, target.scope_key.as_str()).expect("compiled");
    let outcome = eval_metric_ids(
        &ctx,
        &compiled,
        compile_revision.as_str(),
        target.scope_key.as_str(),
        target.owner_resource_id.as_str(),
        target.workset_id.as_str(),
        target.bundle_key.as_str(),
        &target.metric_ids,
    )
    .expect("second eval");
    assert!(
        outcome.artifact_hit,
        "second eval should hit in-memory metric response cache"
    );
}
