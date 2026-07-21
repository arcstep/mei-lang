use mei_host_core::{HostContext, InstancePhase, InstanceRevisions};
use mei_host_graph::WarmupTier;
use mei_lang_datasets::{
    configure_metric_response_cache_ttl_ms, metric_response_result_artifact_exists,
};
use mei_lang_kernel::{load_mei_config_for_app, runtime_plan_requires_warm};
use mei_plug_ds::{
    apply_memory_warmup_pin_policy, collect_warmup_targets_for_scopes_with_filter,
    hydrate_existing_l1_slots, run_warmup_targets_with_tier, WarmupScopeFilter,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::state::AppRuntimeServeState;

/// Boot sequence: launching → importing → (warming) → ready | failed.
pub fn bootstrap_runtime(state: &AppRuntimeServeState) -> anyhow::Result<()> {
    state.set_phase(InstancePhase::Launching);

    let app_config = load_mei_config_for_app(state.host.app_root().as_path(), None);
    configure_metric_response_cache_ttl_ms(app_config.runtime.server_eval_cache.ttl_ms);
    apply_memory_warmup_pin_policy(&app_config.runtime.memory_warmup.clone().unwrap_or_default());

    state.set_phase(InstancePhase::Importing);
    ensure_registry_materialized(&state.host)?;

    let revisions = collect_revisions(&state.host);
    state.set_revisions(revisions);

    let plan = &state.spec.config_snapshot.runtime_plan;
    let warmup_enabled = launch_warmup_enabled(&state.spec.config_snapshot.warmup);
    if runtime_plan_requires_warm(plan, state.app_id()) && warmup_enabled {
        state.set_phase(InstancePhase::Warming);
        match run_hot_warmup(state) {
            Ok(report) => {
                tracing::info!(
                    app_id = %state.app_id(),
                    hydrated = report.memory_hydrated,
                    targets = report.target_count,
                    unique_worksets = report.unique_workset_count,
                    elapsed_ms = report.elapsed_ms,
                    "hot warmup hydrate completed"
                );
            }
            Err(error) => {
                let message = format!("warmup failed: {error}");
                tracing::error!(
                    app_id = %state.app_id(),
                    error = %error,
                    "warmup failed; refusing ready until target manifest is hydrated"
                );
                state.set_failed(message);
                return Err(error);
            }
        }
    }

    let access_prime_started = std::time::Instant::now();
    crate::access::prime_access_html(state);
    tracing::info!(
        app_id = %state.app_id(),
        elapsed_ms = access_prime_started.elapsed().as_millis() as u64,
        "access thin-shell cache primed"
    );
    state.set_phase(InstancePhase::Ready);
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct HotWarmupReport {
    memory_hydrated: usize,
    target_count: usize,
    unique_workset_count: usize,
    elapsed_ms: u64,
}

fn run_hot_warmup(state: &AppRuntimeServeState) -> anyhow::Result<HotWarmupReport> {
    let started = std::time::Instant::now();
    let ctx = &state.host;
    let scenes = launch_hot_scenes(&state.spec.config_snapshot.warmup, state.app_id());
    let required_datasets =
        launch_required_datasets(&state.spec.config_snapshot.warmup, state.app_id());
    // Use THIS candidate instance's runtime plan. Active disk pointer still points at the
    // previous instance until cutover, so resolve_for_app would wrongly apply lazy/scoped.
    let warmup_filter = WarmupScopeFilter::from_runtime_plan(
        &state.spec.config_snapshot.runtime_plan,
        state.app_id(),
    );
    let targets = collect_warmup_targets_for_scopes_with_filter(
        ctx,
        Some(scenes.as_slice()),
        &warmup_filter,
    )?;
    if targets.is_empty() {
        anyhow::bail!(
            "no warmup targets for hotScenes={:?}; check launch warmup manifest",
            scenes
        );
    }

    let app_root = ctx.app_root();
    let mut memory_hydrated = 0usize;
    for scope in &scenes {
        memory_hydrated += hydrate_existing_l1_slots(ctx, scope)?;
    }

    // Pack-first: if prebuild already left MRG slots + disk packs for hot scenes,
    // never re-run Disk compute just because this process L1 is empty. Optionally
    // lite→L1 already happened above; client bootstrap stays on disk.
    let disk_packs_ready =
        validate_required_dataset_readiness(ctx, &scenes, &required_datasets)?.is_empty();

    if memory_hydrated == 0 && !disk_packs_ready {
        tracing::warn!(
            app_id = %ctx.app_id,
            targets = targets.len(),
            "runtime L1 empty and disk packs incomplete; running Memory tier (disk+l1)"
        );
        let report = run_warmup_targets_with_tier(ctx, &targets, WarmupTier::Memory)?;
        memory_hydrated = report.memory_hydrated;
        let _ = mei_host_graph::write_warmup_last_run(
            app_root.as_path(),
            &mei_host_graph::WarmupLastRunRecord {
                policy: scenes.join(","),
                at_ms: mei_host_graph::warmup_last_run_time_ms(),
                eval_compute: report.eval_compute_count,
                cache_hit: report.eval_cache_hit_count,
                disk_hit: report.disk_artifact_hit_count,
                l1_hit: report.l1_cache_hit_count,
                slot_count: report.slot_count,
                elapsed_ms: report.elapsed_ms,
                disk_tier_ms: report.disk_tier_ms,
                memory_tier_ms: report.memory_tier_ms,
                client_tier_ms: report.client_tier_ms,
                disk_bytes: report.disk_bytes,
                target_count: report.target_count,
                unique_content_hash_count: report.unique_content_hash_count,
                rss_before_bytes: report.rss_before_bytes,
                rss_after_bytes: report.rss_after_bytes,
                cpu_user_ms: report.cpu_user_ms,
                cpu_system_ms: report.cpu_system_ms,
                io_read_ops: report.io_read_ops,
                io_read_bytes: report.io_read_bytes,
                io_write_ops: report.io_write_ops,
                io_write_bytes: report.io_write_bytes,
                content_hash_dedupe_skips: report.content_hash_dedupe_skips,
                node_pack_loads: report.node_pack_loads,
                node_pack_stores: report.node_pack_stores,
                node_pack_store_skipped_full_hit: report.node_pack_store_skipped_full_hit,
                tier: "memory".to_string(),
                memory_hydrated: report.memory_hydrated,
                memory_pinned_bytes: report.memory_pinned_bytes,
                rowset_skipped: report.rowset_skipped,
                oversized_skipped: report.oversized_skipped,
                projected_metric_count: report.projected_metric_count,
                lite_hydrated: report.lite_hydrated,
                lite_bytes: report.lite_bytes,
                full_artifact_loads: report.full_artifact_loads,
                lite_backfill: report.lite_backfill,
            },
        );
    } else {
        let tier_label = if memory_hydrated > 0 {
            "memory-hydrate"
        } else {
            "pack-first-skip-disk"
        };
        if memory_hydrated == 0 {
            tracing::info!(
                app_id = %ctx.app_id,
                targets = targets.len(),
                "disk packs ready for hot scenes; skipping Disk recompute (pack-first)"
            );
        }
        let lite_io = mei_lang_datasets::take_lite_artifact_io_stats();
        let _ = mei_host_graph::write_warmup_last_run(
            app_root.as_path(),
            &mei_host_graph::WarmupLastRunRecord {
                policy: scenes.join(","),
                at_ms: mei_host_graph::warmup_last_run_time_ms(),
                eval_compute: 0,
                cache_hit: 0,
                disk_hit: memory_hydrated.max(if disk_packs_ready { targets.len() } else { 0 }),
                l1_hit: memory_hydrated,
                slot_count: memory_hydrated,
                elapsed_ms: started.elapsed().as_millis() as u64,
                tier: tier_label.to_string(),
                memory_hydrated,
                target_count: targets.len(),
                memory_pinned_bytes: mei_lang_datasets::memory_pinned_bytes() as u64,
                lite_hydrated: lite_io.lite_hydrated,
                lite_bytes: lite_io.lite_bytes,
                full_artifact_loads: lite_io.full_artifact_loads,
                lite_backfill: lite_io.lite_backfill,
                ..Default::default()
            },
        );
    }

    let not_ready = validate_required_dataset_readiness(ctx, &scenes, &required_datasets)?;
    if !not_ready.is_empty() {
        anyhow::bail!(
            "required launch targets not ready: {}",
            not_ready.join("; ")
        );
    }

    Ok(HotWarmupReport {
        memory_hydrated,
        target_count: targets.len(),
        unique_workset_count: targets
            .iter()
            .map(|target| target.workset_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn launch_warmup_enabled(warmup: &Option<Value>) -> bool {
    warmup
        .as_ref()
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn launch_hot_scenes(warmup: &Option<Value>, app_id: &str) -> Vec<String> {
    let scenes = warmup
        .as_ref()
        .and_then(|value| value.get("apps"))
        .and_then(|apps| apps.get(app_id))
        .and_then(|app| app.get("hotScenes"))
        .and_then(Value::as_array)
        .map(|scenes| {
            scenes
                .iter()
                .filter_map(|scene| scene.as_str())
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if scenes.is_empty() {
        vec!["home".to_string()]
    } else {
        scenes
    }
}

#[derive(Debug, Clone)]
struct RequiredDataset {
    scene_id: String,
    dataset_id: String,
    metric_ids: Vec<String>,
}

fn launch_required_datasets(warmup: &Option<Value>, app_id: &str) -> Vec<RequiredDataset> {
    warmup
        .as_ref()
        .and_then(|value| value.get("apps"))
        .and_then(|apps| apps.get(app_id))
        .and_then(|app| app.get("datasets"))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let scene_id = row.get("sceneId")?.as_str()?.trim().to_string();
                    let dataset_id = row.get("datasetId")?.as_str()?.trim().to_string();
                    let metric_ids = row
                        .get("metricIds")
                        .and_then(Value::as_array)
                        .map(|ids| {
                            ids.iter()
                                .filter_map(|id| id.as_str())
                                .map(str::trim)
                                .filter(|id| !id.is_empty())
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if scene_id.is_empty() || dataset_id.is_empty() {
                        return None;
                    }
                    Some(RequiredDataset {
                        scene_id,
                        dataset_id,
                        metric_ids,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn validate_required_dataset_readiness(
    ctx: &HostContext,
    scenes: &[String],
    required: &[RequiredDataset],
) -> anyhow::Result<Vec<String>> {
    let app_root = ctx.app_root();
    let registry =
        mei_host_graph::MrgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let mut missing = Vec::new();

    // scene → metric_id → slot indexes (metric_id = suffix after last `::`)
    let mut metrics_by_scene: BTreeMap<String, BTreeMap<String, Vec<usize>>> = BTreeMap::new();
    for (idx, slot) in registry.slots.iter().enumerate() {
        let metric_id = slot
            .slot_id
            .node
            .key
            .rsplit("::")
            .next()
            .unwrap_or(slot.slot_id.node.key.as_str())
            .to_string();
        metrics_by_scene
            .entry(slot.slot_id.scope_key.clone())
            .or_default()
            .entry(metric_id)
            .or_default()
            .push(idx);
    }

    for scope in scenes {
        if !metrics_by_scene.contains_key(scope) {
            missing.push(format!(
                "scene={scope} (no MRG slots; phase=manifest/trace)"
            ));
        }
    }

    for item in required {
        let scene_metrics = metrics_by_scene.get(&item.scene_id);
        if item.metric_ids.is_empty() {
            let matched = registry.slots.iter().any(|slot| {
                slot.slot_id.scope_key == item.scene_id
                    && (slot.owner_resource_id.contains(item.dataset_id.as_str())
                        || slot
                            .metric_def_bundle_revision
                            .contains(item.dataset_id.as_str()))
            });
            if !matched {
                missing.push(format!(
                    "scene={} dataset={} (phase=manifest; no metricIds and owner unmatched)",
                    item.scene_id, item.dataset_id
                ));
            }
            continue;
        }

        for metric_id in &item.metric_ids {
            let candidates = resolve_warmup_metric_aliases(metric_id);
            let mut found_idxs: Vec<usize> = Vec::new();
            if let Some(by_metric) = scene_metrics {
                for candidate in &candidates {
                    if let Some(idxs) = by_metric.get(candidate.as_str()) {
                        found_idxs.extend(idxs.iter().copied());
                    }
                }
            }
            if found_idxs.is_empty() {
                missing.push(format!(
                    "scene={} dataset={} metric={} aliases={:?} (phase=missing-slot)",
                    item.scene_id, item.dataset_id, metric_id, candidates
                ));
                continue;
            }
            let mut pack_ok = false;
            for idx in found_idxs {
                let Some(pref) = registry.slots[idx].payload_ref.as_ref() else {
                    continue;
                };
                if pref.kind == "metric_response"
                    && metric_response_result_artifact_exists(
                        app_root.as_path(),
                        pref.content_hash.as_str(),
                    )
                {
                    pack_ok = true;
                    break;
                }
            }
            if !pack_ok {
                missing.push(format!(
                    "scene={} dataset={} metric={} (phase=disk-pack)",
                    item.scene_id, item.dataset_id, metric_id
                ));
            }
        }
    }

    let mut seen = BTreeSet::new();
    missing.retain(|item| seen.insert(item.clone()));
    Ok(missing)
}

/// Launch / UI dashboard metric ids → static-layout (or other) warmup slot ids.
fn resolve_warmup_metric_aliases(metric_id: &str) -> Vec<String> {
    let trimmed = metric_id.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut out = vec![trimmed.to_string()];
    let alias = match trimmed {
        "park_count" => Some("enforcement_parks_count"),
        "inspections_total_count" => Some("inspection_total_count"),
        "inspections_week_count" => Some("inspection_recent_7d_count"),
        "inspections_no_violation_count" => Some("inspection_no_violation_count"),
        "penalties_total_count" => Some("penalty_total_count"),
        "penalties_week_count" => Some("penalty_recent_7d_count"),
        "administrative_reconsiderations_count" => Some("penalty_admin_review_count"),
        _ => None,
    };
    if let Some(alias) = alias {
        if alias != trimmed {
            out.push(alias.to_string());
        }
    }
    out
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
