use mei_host_core::{HostContext, InstancePhase, InstanceRevisions};
use mei_host_graph::WarmupTier;
use mei_lang_datasets::configure_metric_response_cache_ttl_ms;
use mei_lang_kernel::{load_mei_config_for_app, RuntimeMode};
use mei_plug_ds::{collect_warmup_targets, run_warmup_targets_with_tier};

use crate::state::AppRuntimeServeState;

/// Boot sequence: launching → importing → (warming) → ready | failed.
pub fn bootstrap_runtime(state: &AppRuntimeServeState) -> anyhow::Result<()> {
    state.set_phase(InstancePhase::Launching);

    let app_config = load_mei_config_for_app(state.host.app_root().as_path(), None);
    configure_metric_response_cache_ttl_ms(app_config.runtime.server_eval_cache.ttl_ms);

    state.set_phase(InstancePhase::Importing);
    ensure_registry_materialized(&state.host)?;

    let revisions = collect_revisions(&state.host);
    state.set_revisions(revisions);

    if should_warm(&state.spec.config_snapshot.runtime_plan.default_mode) {
        state.set_phase(InstancePhase::Warming);
        if let Err(error) = run_hot_warmup(&state.host) {
            tracing::warn!(
                app_id = %state.app_id(),
                error = %error,
                "warmup failed; continuing to ready"
            );
        }
    }

    state.set_phase(InstancePhase::Ready);
    Ok(())
}

fn should_warm(mode: &RuntimeMode) -> bool {
    matches!(mode, RuntimeMode::Hot)
}

fn run_hot_warmup(ctx: &HostContext) -> anyhow::Result<()> {
    let targets = collect_warmup_targets(ctx, Some("home"))?;
    if targets.is_empty() {
        return Ok(());
    }
    let _report = run_warmup_targets_with_tier(ctx, &targets, WarmupTier::Disk)?;
    Ok(())
}

pub fn ensure_registry_materialized(ctx: &HostContext) -> anyhow::Result<()> {
    let mcg_path =
        mei_host_graph::mcg_registry_path(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    if mcg_path.is_file() {
        let registry = mei_host_graph::McgRegistryWriter::load(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
        );
        if !registry.nodes.is_empty() {
            return Ok(());
        }
    }
    let bundle_path = ctx.bundle_path();
    if !bundle_path.is_file() {
        tracing::warn!(
            app_id = %ctx.app_id,
            "MCG registry missing and bundle not found; continuing without import"
        );
        return Ok(());
    }
    tracing::info!(
        bundle = %mei_host_core::path_for_log(ctx.workspace_root.as_path(), bundle_path.as_path()),
        "auto-importing meibundle before app-runtime ready"
    );
    mei_host_graph::import_bundle(
        ctx,
        &mei_host_graph::ImportOptions {
            bundle_path: Some(bundle_path),
        },
    )?;
    Ok(())
}

fn collect_revisions(ctx: &HostContext) -> InstanceRevisions {
    let registry =
        mei_host_graph::McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let registry_revision = {
        let rev = registry.registry_revision.trim();
        if rev.is_empty() {
            None
        } else {
            Some(rev.to_string())
        }
    };
    let app_root = ctx.app_root();
    let data_generation =
        mei_lang_kernel::load_cache_generation(app_root.as_path(), &ctx.app_id).data_generation;
    let data_generation = {
        let trimmed = data_generation.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    let client_revision = mei_host_graph::read_client_bootstrap(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        "home",
    )
    .map(|manifest| manifest.client_revision)
    .filter(|value| !value.trim().is_empty());

    InstanceRevisions {
        registry_revision,
        client_revision,
        data_generation,
    }
}
