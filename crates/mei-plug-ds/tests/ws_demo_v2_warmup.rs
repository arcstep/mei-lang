//! Integration test: ws-demo-v2 warmup + MRG memory tier.

use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{import_bundle, ImportOptions, WarmupTier};
use mei_plug_ds::{
    collect_warmup_targets, eval_metric_ids, load_compiled_for_warmup, run_warmup_targets_with_tier,
};

static INIT: Once = Once::new();

fn ws_demo_v2_root() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

fn bundle_path(workspace: &std::path::Path) -> PathBuf {
    workspace.join("apps/data-demo/build/active/exchange/data-demo.meibundle")
}

fn skip_if_data_demo_missing() -> Option<PathBuf> {
    let workspace = ws_demo_v2_root()?;
    if !workspace.join("apps/data-demo").is_dir() {
        return None;
    }
    Some(workspace)
}

fn ensure_imported() -> Option<PathBuf> {
    let workspace = skip_if_data_demo_missing()?;
    INIT.call_once(|| {
        if !bundle_path(workspace.as_path()).is_file() {
            return;
        }
        let ctx = HostContext::new(workspace.clone(), "data-demo");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle_path(workspace.as_path())),
            },
        )
        .expect("import bundle");
    });
    if !bundle_path(workspace.as_path()).is_file() {
        return None;
    }
    Some(workspace)
}

#[test]
fn ws_demo_v2_warmup_tier_all_populates_mrg_and_memory_hit() {
    let Some(workspace) = ensure_imported() else {
        return;
    };
    let ctx = HostContext::new(workspace.clone(), "data-demo".to_string());
    let targets = collect_warmup_targets(&ctx, Some("home")).expect("warmup targets");
    assert!(
        !targets.is_empty(),
        "home warmup policy should define targets"
    );
    let report =
        run_warmup_targets_with_tier(&ctx, &targets, WarmupTier::All).expect("warmup tier all");
    assert!(report.slot_count > 0, "warmup should register MRG slots");
    let status =
        mei_host_graph::mrg_status_json(workspace.as_path(), "data-demo").expect("mrg status");
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
    let manifest = mei_host_graph::read_client_bootstrap(workspace.as_path(), "data-demo", "home")
        .expect("home client-bootstrap manifest");
    assert!(
        manifest.metrics.len() >= 40,
        "home manifest should cover critical metrics, got {}",
        manifest.metrics.len()
    );
    let manifest_path = ctx.app_root().join("var/active/client-bootstrap/home.json");
    assert!(
        manifest_path.is_file(),
        "client-bootstrap manifest file should exist at {}",
        manifest_path.display()
    );
    let bootstrap_dir = ctx.app_root().join("var/active/client-bootstrap");
    let bootstrap_files = std::fs::read_dir(&bootstrap_dir)
        .expect("read bootstrap dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count();
    assert!(
        bootstrap_files >= 2,
        "multi-scope warmup should emit neighbor bootstrap manifests, got {} in {}",
        bootstrap_files,
        bootstrap_dir.display()
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

    let embed_status =
        mei_host_graph::bootstrap_embed_status(workspace.as_path(), "data-demo", "home");
    assert!(
        embed_status.allowed,
        "home bootstrap embed should be allowed after warmup: {:?}",
        embed_status
    );
}
