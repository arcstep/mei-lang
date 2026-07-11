use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mei_host_graph::{
    list_scope_routes, mrg_status_json, McgRegistryWriter, MrgRegistry, MrgRegistryWriter,
    ScopeRoute,
};
use mei_lang_kernel::{
    load_cache_generation, resolve_active_build_identity,
    resolve_app_build_generation_from_current, resolve_app_build_root,
    resolve_app_data_snapshot_root, resolve_app_eval_cache_root, resolve_app_root,
    resolve_runtime_warmup_manifest, ReachabilityTreeNode, ReachabilityTreeRoot,
    PREBUILD_COMPILE_INDEX_REL, PREBUILD_LAST_BUILD_SUMMARY_REL,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::build_ops::build_status_aggregate;
use crate::cache_diagnostics::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled, locked_cache_env_overrides,
};
use crate::state::ShellState;

#[derive(Debug, Clone)]
struct RuntimeRouteEntry {
    node_id: String,
    scope_key: String,
    scene_id: String,
    url: String,
    assembly_key: String,
}

#[derive(Debug, Clone, Default)]
struct ScopeSummary {
    scope_key: String,
    route_node_ids: Vec<String>,
    route_count: usize,
    access_url: Option<String>,
    slot_count: usize,
    ready_slots: usize,
    stale_slots: usize,
    failed_slots: usize,
    dirty_slots: usize,
    client_eligible_slots: usize,
    route_duplicate_count: usize,
    workset_ids: BTreeSet<String>,
}

#[derive(Debug, Deserialize, Default)]
struct PersistedCompileIndex {
    #[serde(rename = "generated_at_ms", alias = "generatedAtMs")]
    generated_at_ms: u64,
    #[serde(default)]
    entries: Vec<Value>,
}

#[derive(Debug, Deserialize, Default)]
struct PersistedLastBuildSummary {
    #[serde(default, rename = "appId")]
    app_id: String,
    #[serde(default, rename = "recordedAtMs")]
    recorded_at_ms: u64,
    #[serde(default, rename = "peakRssBytes")]
    peak_rss_bytes: u64,
    #[serde(default, rename = "currentRssBytes")]
    current_rss_bytes: Option<u64>,
    #[serde(default, rename = "compileIndexHits")]
    compile_index_hits: usize,
    #[serde(default, rename = "compileIndexMisses")]
    compile_index_misses: usize,
    #[serde(default, rename = "compileIndexStaleEntries")]
    compile_index_stale_entries: usize,
    #[serde(default, rename = "mrgEvalSkips")]
    mrg_eval_skips: usize,
    #[serde(default, rename = "dataframeEvalSkips")]
    dataframe_eval_skips: usize,
}

pub fn build_runtime_snapshot(shell: &ShellState, app_id: &str) -> Value {
    let workspace = shell.ctx.workspace_root.as_path();
    let app_root = resolve_app_root(workspace, app_id);
    let build_root = resolve_app_build_root(app_root.as_path());
    let ops = build_status_aggregate(shell);
    let identity = resolve_active_build_identity(workspace);
    let mcg = McgRegistryWriter::load(workspace, app_id);
    let mrg = MrgRegistryWriter::load(workspace, app_id);
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace, app_id);
    let mrg_status = mrg_status_json(workspace, app_id).unwrap_or_else(|_| json!({}));
    let scope_routes = list_scope_routes(workspace, app_id).unwrap_or_default();

    let registry_ready = !mcg.nodes.is_empty();
    let is_default_app = app_id == shell.ctx.app_id.as_str();
    let access_ready = if is_default_app {
        shell.imported || registry_ready
    } else {
        registry_ready
    };
    let warmup_ready = is_default_app && shell.warmed_up;
    let phase = if !access_ready {
        "starting"
    } else if warmup_ready {
        "ready"
    } else {
        "bound"
    };

    let route_entries = build_route_entries(&scope_routes);
    let route_values = route_entries
        .iter()
        .map(route_entry_to_json)
        .collect::<Vec<_>>();
    let route_duplicate_count = count_duplicate_routes(&scope_routes);
    let orphan_url_count = scope_routes
        .iter()
        .filter(|route| route.url.trim().is_empty())
        .count();

    let slot_values = build_slot_values(&mrg);
    let scope_summaries = build_scope_summaries(
        app_id,
        &route_entries,
        &slot_values,
        access_ready,
        warmup_ready,
    );
    let scope_values = scope_summaries
        .iter()
        .map(|scope| scope_summary_to_json(workspace, app_id, scope, access_ready, warmup_ready))
        .collect::<Vec<_>>();
    let default_scope = scope_summaries
        .first()
        .map(|scope| scope.scope_key.clone())
        .unwrap_or_else(|| "home".to_string());
    let default_scope_summary = scope_summaries
        .iter()
        .find(|scope| scope.scope_key == default_scope)
        .cloned();

    let ready_slots = scope_summaries
        .iter()
        .map(|scope| scope.ready_slots)
        .sum::<usize>();
    let stale_slots = scope_summaries
        .iter()
        .map(|scope| scope.stale_slots)
        .sum::<usize>();
    let failed_slots = scope_summaries
        .iter()
        .map(|scope| scope.failed_slots)
        .sum::<usize>();
    let dirty_slot_count = scope_summaries
        .iter()
        .map(|scope| scope.dirty_slots)
        .sum::<usize>();
    let dirty_scopes = scope_summaries
        .iter()
        .filter(|scope| scope.dirty_slots > 0)
        .map(|scope| scope.scope_key.clone())
        .collect::<Vec<_>>();
    let degraded_scopes = scope_summaries
        .iter()
        .filter(|scope| scope.stale_slots > 0 || scope.failed_slots > 0 || scope.route_count == 0)
        .map(|scope| scope.scope_key.clone())
        .collect::<Vec<_>>();
    let failed_slot_values = slot_values
        .iter()
        .filter(|slot| slot.get("state").and_then(Value::as_str) == Some("failed"))
        .cloned()
        .collect::<Vec<_>>();

    let stale_ratio = if mrg.slots.is_empty() {
        0.0
    } else {
        stale_slots as f64 / mrg.slots.len() as f64
    };
    let data_ready = warmup_ready && failed_slots == 0 && dirty_slot_count == 0;
    let mut blockers = Vec::new();
    if route_entries.is_empty() {
        blockers.push("L2:navigation missing".to_string());
    }
    if !access_ready {
        blockers.push("L3:assembly not imported".to_string());
    }
    if !warmup_ready {
        blockers.push("L4:warmup not completed".to_string());
    }
    if failed_slots > 0 {
        blockers.push(format!("L4:failed slots={failed_slots}"));
    }
    if stale_slots > 0 {
        blockers.push(format!("L4:stale slots={stale_slots}"));
    }

    let compile_index = load_compile_index_meta(app_root.as_path());
    let last_build_summary = load_last_build_summary(app_root.as_path(), app_id);
    let env_current = resolve_app_build_generation_from_current(app_root.as_path()).ok();
    let build_diag = build_diagnostics_json(
        workspace,
        app_root.as_path(),
        identity.meilang_version.as_str(),
        identity.build_generation.as_str(),
        identity.workspace_version.as_str(),
        env_current,
        compile_index.as_ref(),
        last_build_summary.as_ref(),
    );
    let prebuild_summary = build_prebuild_summary(
        warmup_ready,
        resolve_runtime_warmup_manifest(workspace)
            .ok()
            .flatten()
            .map(|_| "standard".to_string()),
        last_build_summary.as_ref(),
    );
    let disk_diag = scan_disk(app_root.as_path(), build_root.as_path());
    let eval_diag = scan_eval(app_root.as_path());
    let content_store = scan_content_store(app_root.as_path());
    let data_generation = load_cache_generation(app_root.as_path(), app_id).data_generation;

    let eval_pack_embed =
        mei_host_graph::bootstrap_embed_status(workspace, app_id, default_scope.as_str());
    let delivery_class_counts =
        mei_host_graph::delivery_class_counts_for_scope(workspace, app_id, default_scope.as_str());
    let warmup_last_run = mei_host_graph::warmup_last_run_json(app_root.as_path());
    let eval_pack = json!({
        "warmupLastRun": warmup_last_run,
        "deliveryClassCounts": delivery_class_counts,
        "evalPackMissReason": if eval_pack_embed.allowed { Value::Null } else { json!(eval_pack_embed.reason) },
        "bootstrapEmbed": {
            "allowed": eval_pack_embed.allowed,
            "reason": eval_pack_embed.reason,
            "metricCount": eval_pack_embed.metric_count,
            "clientRevision": eval_pack_embed.client_revision,
            "expectedRevision": eval_pack_embed.expected_revision,
        },
    });

    let hot_scopes = mrg_status
        .get("hotScopes")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let bootstrap_manifest_count = mrg_status
        .get("bootstrapManifestCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let bootstrap_scopes = mrg_status
        .get("bootstrapScopes")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let slots_by_tier = mrg_status
        .get("slotsByTier")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let telemetry = mrg_status
        .get("telemetry")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let diagnostics = json!({
        "disk": disk_diag.clone(),
        "eval": eval_diag.clone(),
        "mcg": {
            "nodeCount": mcg.nodes.len(),
            "scenePayloadNodes": mcg.nodes.iter().filter(|node| node.id.kind.slug() == "scene_payload").count(),
            "metricDefBundleNodes": mcg.nodes.iter().filter(|node| node.id.kind.slug() == "metric_def_bundle").count(),
            "appSkeletonPresent": mcg.nodes.iter().any(|node| node.id.kind.slug() == "app_skeleton" && node.payload_ref.is_some()),
            "registryRevision": mcg.registry_revision,
        },
        "mrg": {
            "slotCount": mrg.slots.len(),
            "readySlots": ready_slots,
            "staleSlots": stale_slots,
            "failedSlots": failed_slots,
            "staleRatio": stale_ratio,
            "navigationNodeCount": route_entries.len(),
            "navigationDuplicateKeys": route_duplicate_count,
            "navigationOrphanUrls": orphan_url_count,
        },
        "cache": {
            "accessSlimArtifacts": access_slim_artifacts_enabled(),
            "canonicalArtifactPersist": canonical_artifact_persist_enabled(),
            "graphRegistryDedup": true,
            "lockedOn": true,
            "envOverrides": locked_cache_env_overrides(),
        },
        "build": build_diag.clone(),
        "contentStore": content_store.clone(),
        "scopeGateSweep": {
            "l2Miss": if route_entries.is_empty() { 1 } else { 0 },
            "l3Fail": if access_ready { 0 } else { 1 },
            "l4Stale": if data_ready { 0 } else { 1 },
            "degradedScopes": degraded_scopes,
        },
        "prebuildPlan": {
            "planSource": if dirty_slot_count == 0 { "manifest_override" } else { "mrg_dirty" },
            "dirtySlotCount": dirty_slot_count,
            "mrgEvalSkips": build_diag.get("mrgEvalSkips").cloned().unwrap_or(Value::Null),
        },
        "failedSlots": failed_slot_values.clone(),
        "alerts": build_alerts(route_duplicate_count, orphan_url_count, stale_ratio, failed_slots, dirty_slot_count),
    });

    let roots = build_management_roots(
        &route_entries,
        &scope_summaries,
        route_duplicate_count,
        dirty_scopes.as_slice(),
        failed_slots,
        slot_values.len(),
    );

    json!({
        "appId": app_id,
        "hostShellMgmt": true,
        "roots": roots,
        "scopeRoutes": scope_routes_to_json(&scope_routes),
        "slots": Value::Array(slot_values.clone()),
        "ops": ops,
        "mrgStatus": mrg_status,
        "host": {
            "phase": phase,
            "appPhase": phase,
            "accessReady": access_ready,
            "scopeGateReady": access_ready,
            "warmupReady": warmup_ready,
            "scopeGateMode": "host-shell-lightweight",
            "gateL2Miss": if route_entries.is_empty() { Some(1usize) } else { Some(0usize) },
            "gateL3Fail": if access_ready { Some(0usize) } else { Some(1usize) },
            "gateL4Stale": if data_ready { Some(0usize) } else { Some(1usize) },
            "lastBuildTotalMs": Value::Null,
            "lastBuildCompileMs": Value::Null,
            "lastBuildWarmupMs": Value::Null,
        },
        "prebuild": prebuild_summary,
        "navigation": {
            "routes": route_values,
            "routeCount": route_entries.len(),
            "scopeCount": scope_summaries.len(),
            "duplicateRouteCount": route_duplicate_count,
            "orphanUrlCount": orphan_url_count,
            "bootstrapManifestCount": bootstrap_manifest_count,
            "bootstrapScopes": bootstrap_scopes.clone(),
            "note": "运行视图的 route 来自 host-shell 当前可见入口索引；同一 scope 可能对应多条入口。"
        },
        "scopeGate": {
            "defaultScope": default_scope,
            "accessReady": access_ready,
            "shellReady": access_ready,
            "dataReady": data_ready,
            "blockers": blockers,
            "degradedScopes": scope_summaries.iter().filter(|scope| !scope_is_ready(scope, access_ready, warmup_ready)).map(|scope| scope.scope_key.clone()).collect::<Vec<_>>(),
            "scopeGateSweep": diagnostics.get("scopeGateSweep").cloned().unwrap_or_else(|| json!({})),
            "selectedScopeSummary": default_scope_summary
                .as_ref()
                .map(|scope| scope_summary_to_json(workspace, app_id, scope, access_ready, warmup_ready))
                .unwrap_or_else(|| json!({})),
            "note": "host-shell 当前展示的是轻量 gate 摘要，用于解释入口、装配与物化是否可用；更深的 parity 仍以 CLI/readiness 为准。"
        },
        "warmup": {
            "planSource": if dirty_slot_count == 0 { "manifest_override" } else { "mrg_dirty" },
            "dirtySlotCount": dirty_slot_count,
            "dirtyScopes": dirty_scopes,
            "hotScopes": hot_scopes,
            "bootstrapManifestCount": bootstrap_manifest_count,
            "bootstrapScopes": bootstrap_scopes.clone(),
            "mrgEvalSkips": build_diag.get("mrgEvalSkips").cloned().unwrap_or(Value::Null),
            "note": "warmup 关注哪些 scope/workset/slot 需要推进；不等同于 compile/import 是否完成。"
        },
        "mrg": {
            "schemaVersion": mrg.schema_version,
            "registryRevision": mrg.registry_revision,
            "updatedAtMs": mrg.updated_at_ms,
            "edgeCount": mrg.edges.len(),
            "slotCount": mrg.slots.len(),
            "slotsByTier": slots_by_tier,
            "telemetry": telemetry,
            "slots": slot_values,
            "scopes": scope_values.clone(),
            "failedSlots": failed_slot_values.clone(),
            "edges": serde_json::to_value(&mrg.edges).unwrap_or_else(|_| json!([])),
            "note": "MRG 负责当前 scope 下的 materialization 结果与 tier 就绪状态。"
        },
        "cache": {
            "dataGeneration": data_generation,
            "build": build_diag.clone(),
            "disk": disk_diag,
            "eval": eval_diag,
            "contentStore": content_store,
            "flags": {
                "accessSlimArtifacts": access_slim_artifacts_enabled(),
                "canonicalArtifactPersist": canonical_artifact_persist_enabled(),
                "graphRegistryDedup": true,
                "envOverrides": locked_cache_env_overrides(),
            },
            "clientTier": {
                "bootstrapManifestCount": bootstrap_manifest_count,
                "bootstrapScopes": bootstrap_scopes,
                "note": "clientEligible/clientRevision 表示可写入 client bootstrap 的资格与失效键，不代表当前浏览器会话已命中缓存。"
            }
        },
        "scopes": scope_values,
        "evalPack": eval_pack,
        "diagnostics": diagnostics,
    })
}

pub fn management_roots_from_snapshot(snapshot: &Value) -> Vec<ReachabilityTreeRoot> {
    snapshot
        .get("roots")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

fn scope_routes_to_json(routes: &[ScopeRoute]) -> Value {
    Value::Array(
        routes
            .iter()
            .map(|route| {
                json!({
                    "sceneId": route.scene_id,
                    "url": route.url,
                    "assemblyKey": route.assembly_key,
                })
            })
            .collect(),
    )
}

fn build_route_entries(routes: &[ScopeRoute]) -> Vec<RuntimeRouteEntry> {
    routes
        .iter()
        .map(|route| {
            let scene_id = normalized_scope_key(route.scene_id.as_str());
            let route_key = stable_short_id(&format!("{}|{}", route.url, route.assembly_key));
            RuntimeRouteEntry {
                node_id: format!("nav:route:{scene_id}:{route_key}"),
                scope_key: scene_id.clone(),
                scene_id,
                url: route.url.clone(),
                assembly_key: route.assembly_key.clone(),
            }
        })
        .collect()
}

fn route_entry_to_json(route: &RuntimeRouteEntry) -> Value {
    json!({
        "nodeId": route.node_id,
        "sceneId": route.scene_id,
        "scopeKey": route.scope_key,
        "url": route.url,
        "assemblyKey": route.assembly_key,
        "label": format!("route · {}", route.scene_id),
    })
}

fn build_slot_values(registry: &MrgRegistry) -> Vec<Value> {
    registry
        .slots
        .iter()
        .map(|slot| {
            let mut value = serde_json::to_value(slot).unwrap_or_else(|_| json!({}));
            if let Some(map) = value.as_object_mut() {
                map.insert(
                    "nodeId".to_string(),
                    json!(slot_node_id(
                        slot.slot_id.node.key.as_str(),
                        slot.slot_id.scope_key.as_str()
                    )),
                );
                map.insert(
                    "scopeKey".to_string(),
                    json!(slot.slot_id.scope_key.clone()),
                );
                map.insert("nodeKey".to_string(), json!(slot.slot_id.node.key.clone()));
                map.insert("nodeKind".to_string(), json!(slot.slot_id.node.kind.slug()));
            }
            value
        })
        .collect()
}

fn build_scope_summaries(
    app_id: &str,
    routes: &[RuntimeRouteEntry],
    slots: &[Value],
    access_ready: bool,
    warmup_ready: bool,
) -> Vec<ScopeSummary> {
    let mut summaries = BTreeMap::<String, ScopeSummary>::new();
    for route in routes {
        let summary = summaries
            .entry(route.scope_key.clone())
            .or_insert_with(|| ScopeSummary {
                scope_key: route.scope_key.clone(),
                ..Default::default()
            });
        summary.route_count += 1;
        summary.route_node_ids.push(route.node_id.clone());
        if summary.access_url.is_none() && !route.url.trim().is_empty() {
            summary.access_url = Some(route.url.clone());
        }
    }
    for slot in slots {
        let scope_key = slot
            .get("scopeKey")
            .and_then(Value::as_str)
            .unwrap_or("home")
            .to_string();
        let summary = summaries
            .entry(scope_key.clone())
            .or_insert_with(|| ScopeSummary {
                scope_key,
                ..Default::default()
            });
        summary.slot_count += 1;
        if slot
            .get("clientEligible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            summary.client_eligible_slots += 1;
        }
        if let Some(workset_id) = slot.get("worksetId").and_then(Value::as_str) {
            if !workset_id.trim().is_empty() {
                summary.workset_ids.insert(workset_id.to_string());
            }
        }
        match slot
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "ready" => summary.ready_slots += 1,
            "stale" => {
                summary.stale_slots += 1;
                summary.dirty_slots += 1;
            }
            "failed" => {
                summary.failed_slots += 1;
                summary.dirty_slots += 1;
            }
            "missing" | "warming" => summary.dirty_slots += 1,
            _ => {}
        }
    }
    for summary in summaries.values_mut() {
        if summary.route_count > 1 {
            summary.route_duplicate_count = summary.route_count - 1;
        }
        if summary.access_url.is_none() {
            summary.access_url = Some(format!("/apps/{}/{}", app_id, summary.scope_key));
        }
        if !access_ready {
            summary.dirty_slots = summary.dirty_slots.max(summary.slot_count);
        } else if !warmup_ready && summary.slot_count > 0 {
            summary.dirty_slots = summary
                .dirty_slots
                .max(summary.slot_count.saturating_sub(summary.ready_slots));
        }
    }
    summaries.into_values().collect()
}

fn scope_summary_to_json(
    workspace: &Path,
    app_id: &str,
    scope: &ScopeSummary,
    access_ready: bool,
    warmup_ready: bool,
) -> Value {
    let blockers = scope_blockers(scope, access_ready, warmup_ready);
    let bootstrap_embed =
        mei_host_graph::bootstrap_embed_status(workspace, app_id, scope.scope_key.as_str());
    json!({
        "nodeId": format!("scope:{}", scope.scope_key),
        "scopeKey": scope.scope_key,
        "routeCount": scope.route_count,
        "routeDuplicateCount": scope.route_duplicate_count,
        "slotCount": scope.slot_count,
        "readySlots": scope.ready_slots,
        "staleSlots": scope.stale_slots,
        "failedSlots": scope.failed_slots,
        "dirtySlots": scope.dirty_slots,
        "clientEligibleSlots": scope.client_eligible_slots,
        "accessUrl": scope.access_url,
        "routeNodeIds": scope.route_node_ids,
        "worksetIds": scope.workset_ids.iter().cloned().collect::<Vec<_>>(),
        "blockers": blockers,
        "bootstrapEmbed": {
            "allowed": bootstrap_embed.allowed,
            "reason": bootstrap_embed.reason,
            "metricCount": bootstrap_embed.metric_count,
            "clientRevision": bootstrap_embed.client_revision,
            "expectedRevision": bootstrap_embed.expected_revision,
        },
    })
}

fn build_management_roots(
    routes: &[RuntimeRouteEntry],
    scopes: &[ScopeSummary],
    duplicate_route_count: usize,
    dirty_scopes: &[String],
    failed_slot_count: usize,
    slot_count: usize,
) -> Vec<ReachabilityTreeRoot> {
    let host_root = ReachabilityTreeRoot {
        group: "host_ops".to_string(),
        label: "Host / Ops".to_string(),
        default_open: true,
        children: vec![
            ReachabilityTreeNode {
                node_id: "ops:overview".to_string(),
                id: "ops:overview".to_string(),
                kind: "host_ops".to_string(),
                label: "运行状态".to_string(),
                badges: vec!["ops".to_string()],
                ..Default::default()
            },
            ReachabilityTreeNode {
                node_id: "ops:versions".to_string(),
                id: "ops:versions".to_string(),
                kind: "host_version".to_string(),
                label: "运行版本".to_string(),
                badges: Vec::new(),
                ..Default::default()
            },
        ],
    };

    let navigation_children = scopes
        .iter()
        .map(|scope| {
            let route_children = routes
                .iter()
                .filter(|route| route.scope_key == scope.scope_key)
                .map(|route| ReachabilityTreeNode {
                    node_id: route.node_id.clone(),
                    id: route.node_id.clone(),
                    kind: "nav_route".to_string(),
                    label: format!("route · {}", route.scene_id),
                    badges: vec!["入口".to_string()],
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            ReachabilityTreeNode {
                node_id: format!("scope:{}", scope.scope_key),
                id: format!("scope:{}", scope.scope_key),
                kind: "scope_summary".to_string(),
                label: format!("scope · {}", scope.scope_key),
                badges: vec![
                    format!("routes={}", scope.route_count),
                    format!("slots={}", scope.slot_count),
                ],
                children: route_children,
                compile_scene: scope.scope_key.clone(),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    let navigation_root = ReachabilityTreeRoot {
        group: "runtime_navigation".to_string(),
        label: "Navigation".to_string(),
        default_open: true,
        children: std::iter::once(ReachabilityTreeNode {
            node_id: "nav:summary".to_string(),
            id: "nav:summary".to_string(),
            kind: "nav_summary".to_string(),
            label: "入口总览".to_string(),
            badges: if duplicate_route_count > 0 {
                vec![format!("dup={duplicate_route_count}")]
            } else {
                Vec::new()
            },
            ..Default::default()
        })
        .chain(navigation_children)
        .collect(),
    };

    let gate_root = ReachabilityTreeRoot {
        group: "runtime_scope_gate".to_string(),
        label: "Scope Gate".to_string(),
        default_open: true,
        children: vec![ReachabilityTreeNode {
            node_id: "gate:summary".to_string(),
            id: "gate:summary".to_string(),
            kind: "gate_summary".to_string(),
            label: "就绪判定".to_string(),
            badges: Vec::new(),
            ..Default::default()
        }],
    };

    let warmup_root = ReachabilityTreeRoot {
        group: "runtime_warmup".to_string(),
        label: "Warmup".to_string(),
        default_open: true,
        children: std::iter::once(ReachabilityTreeNode {
            node_id: "warmup:summary".to_string(),
            id: "warmup:summary".to_string(),
            kind: "warmup_summary".to_string(),
            label: "预热计划".to_string(),
            badges: if dirty_scopes.is_empty() {
                vec!["clean".to_string()]
            } else {
                vec![format!("dirty={}", dirty_scopes.len())]
            },
            ..Default::default()
        })
        .chain(dirty_scopes.iter().map(|scope_key| ReachabilityTreeNode {
            node_id: format!("warmup:scope:{scope_key}"),
            id: format!("warmup:scope:{scope_key}"),
            kind: "warmup_scope".to_string(),
            label: format!("scope · {scope_key}"),
            badges: vec!["dirty".to_string()],
            ..Default::default()
        }))
        .collect(),
    };

    let mrg_root = ReachabilityTreeRoot {
        group: "runtime_mrg".to_string(),
        label: "MRG".to_string(),
        default_open: true,
        children: vec![
            ReachabilityTreeNode {
                node_id: "mrg:summary".to_string(),
                id: "mrg:summary".to_string(),
                kind: "mrg_summary".to_string(),
                label: "MRG 总览".to_string(),
                badges: vec![slot_count.to_string()],
                ..Default::default()
            },
            ReachabilityTreeNode {
                node_id: "mrg:failed".to_string(),
                id: "mrg:failed".to_string(),
                kind: "mrg_failed".to_string(),
                label: "失败 slots".to_string(),
                badges: vec![failed_slot_count.to_string()],
                ..Default::default()
            },
            ReachabilityTreeNode {
                node_id: "mrg:slots".to_string(),
                id: "mrg:slots".to_string(),
                kind: "mrg_slots".to_string(),
                label: "全部 slots".to_string(),
                badges: vec![slot_count.to_string()],
                ..Default::default()
            },
        ],
    };

    let cache_root = ReachabilityTreeRoot {
        group: "runtime_cache".to_string(),
        label: "Cache / Artifact".to_string(),
        default_open: true,
        children: vec![ReachabilityTreeNode {
            node_id: "cache:summary".to_string(),
            id: "cache:summary".to_string(),
            kind: "cache_summary".to_string(),
            label: "缓存与产物".to_string(),
            badges: Vec::new(),
            ..Default::default()
        }],
    };

    let diagnostics_root = ReachabilityTreeRoot {
        group: "runtime_diagnostics".to_string(),
        label: "Diagnostics".to_string(),
        default_open: true,
        children: vec![ReachabilityTreeNode {
            node_id: "diag:summary".to_string(),
            id: "diag:summary".to_string(),
            kind: "diag_summary".to_string(),
            label: "诊断与告警".to_string(),
            badges: Vec::new(),
            ..Default::default()
        }],
    };

    vec![
        host_root,
        navigation_root,
        gate_root,
        warmup_root,
        mrg_root,
        cache_root,
        diagnostics_root,
    ]
}

fn build_diagnostics_json(
    workspace: &Path,
    app_root: &Path,
    meilang_version: &str,
    build_generation: &str,
    workspace_version: &str,
    env_active: Option<String>,
    compile_index: Option<&PersistedCompileIndex>,
    last_build_summary: Option<&PersistedLastBuildSummary>,
) -> Value {
    json!({
        "source": if last_build_summary.is_some() { "last-build-summary" } else { "none" },
        "reportPath": if last_build_summary.is_some() {
            Some(app_root.join(PREBUILD_LAST_BUILD_SUMMARY_REL).display().to_string())
        } else {
            None::<String>
        },
        "recordedAtMs": last_build_summary.map(|summary| summary.recorded_at_ms),
        "peakRssBytes": last_build_summary.map(|summary| summary.peak_rss_bytes),
        "currentRssBytes": last_build_summary.and_then(|summary| summary.current_rss_bytes),
        "compileIndexHits": last_build_summary.map(|summary| summary.compile_index_hits),
        "compileIndexMisses": last_build_summary.map(|summary| summary.compile_index_misses),
        "compileIndexStaleEntries": last_build_summary.map(|summary| summary.compile_index_stale_entries),
        "mrgEvalSkips": last_build_summary.map(|summary| summary.mrg_eval_skips),
        "dataframeEvalSkips": last_build_summary.map(|summary| summary.dataframe_eval_skips),
        "compileIndexEntries": compile_index.map(|index| index.entries.len()),
        "compileIndexGeneratedAtMs": compile_index.map(|index| index.generated_at_ms),
        "meilangVersion": meilang_version,
        "buildGeneration": build_generation,
        "toolchainVersion": meilang_version,
        "workspaceVersion": workspace_version,
        "envCurrent": env_active,
        "warmupManifestPresent": resolve_runtime_warmup_manifest(workspace).ok().flatten().is_some(),
    })
}

fn build_prebuild_summary(
    warmup_ready: bool,
    scope_profile: Option<String>,
    last_build_summary: Option<&PersistedLastBuildSummary>,
) -> Value {
    json!({
        "ok": warmup_ready,
        "scopeProfile": scope_profile,
        "totalWallMs": Value::Null,
        "compileScopesMs": Value::Null,
        "scopeArtifactsMs": Value::Null,
        "peakRssBytes": last_build_summary.map(|summary| summary.peak_rss_bytes),
        "currentRssBytes": last_build_summary.and_then(|summary| summary.current_rss_bytes),
        "compileScopeCount": Value::Null,
        "realCompileCount": Value::Null,
        "cacheHitCount": last_build_summary.map(|summary| summary.compile_index_hits),
        "expansionRatio": Value::Null,
        "reportAge": last_build_summary.map(|summary| format_age(summary.recorded_at_ms)),
        "inSucceededApps": warmup_ready,
    })
}

fn scan_disk(app_root: &Path, build_root: &Path) -> Value {
    let manifest_dir = build_root.join("manifests/compiled_app");
    let artifact_dir = build_root.join("artifacts/compiled_app");
    let (manifest_count, manifest_bytes) = dir_stats(manifest_dir.as_path());
    let (artifact_count, artifact_bytes) = dir_stats(artifact_dir.as_path());
    let scene_payload_root = build_root.join("store/content/scene_payload");
    let (scene_payload_file_count, scene_payload_bytes) = dir_stats(scene_payload_root.as_path());
    let eval_root = resolve_app_eval_cache_root(app_root);
    let (eval_artifact_file_count, eval_artifact_bytes) = dir_stats(eval_root.as_path());
    let graph_root = build_root.join("registry");
    let (_, graph_bytes) = dir_stats(graph_root.as_path());
    let (_, data_snapshots_bytes) = dir_stats(resolve_app_data_snapshot_root(app_root).as_path());
    let (_, prebuild_bytes) = dir_stats(app_root.join("prebuild").as_path());
    json!({
        "compiledAppFileCount": manifest_count + artifact_count,
        "compiledAppBytes": manifest_bytes + artifact_bytes,
        "scenePayloadFileCount": scene_payload_file_count,
        "scenePayloadBytes": scene_payload_bytes,
        "evalArtifactFileCount": eval_artifact_file_count,
        "evalArtifactBytes": eval_artifact_bytes,
        "graphBytes": graph_bytes,
        "dataSnapshotsBytes": data_snapshots_bytes,
        "prebuildBytes": prebuild_bytes,
        "appRootBytes": dir_stats(app_root).1,
    })
}

fn scan_eval(app_root: &Path) -> Value {
    let eval_root = resolve_app_eval_cache_root(app_root);
    let (eval_total_files, eval_total_bytes) = dir_stats(eval_root.as_path());
    let response_dir = eval_root.join("metric-response");
    let dataframe_dir = eval_root.join("metric-dataframe");
    let (metric_response_files, metric_response_bytes) = dir_stats(response_dir.as_path());
    let (metric_dataframe_files, metric_dataframe_bytes) = dir_stats(dataframe_dir.as_path());
    json!({
        "metricResponseFiles": metric_response_files,
        "metricResponseBytes": metric_response_bytes,
        "metricDataframeFiles": metric_dataframe_files,
        "metricDataframeBytes": metric_dataframe_bytes,
        "evalTotalFiles": eval_total_files,
        "evalTotalBytes": eval_total_bytes,
    })
}

fn scan_content_store(app_root: &Path) -> Value {
    let root = resolve_app_build_root(app_root).join("store/content");
    let mut files_by_kind = BTreeMap::new();
    let mut bytes = 0u64;
    if root.is_dir() {
        if let Ok(kinds) = fs::read_dir(&root) {
            for kind_entry in kinds.flatten() {
                if !kind_entry.path().is_dir() {
                    continue;
                }
                let kind = kind_entry.file_name().to_string_lossy().to_string();
                let (count, kind_bytes) = dir_stats(kind_entry.path().as_path());
                files_by_kind.insert(kind, count);
                bytes += kind_bytes;
            }
        }
    }
    json!({
        "bytes": bytes,
        "filesByKind": files_by_kind,
        "orphanCount": 0usize,
    })
}

fn load_compile_index_meta(app_root: &Path) -> Option<PersistedCompileIndex> {
    let path = resolve_app_build_root(app_root).join(PREBUILD_COMPILE_INDEX_REL);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<PersistedCompileIndex>(&raw).ok()
}

fn load_last_build_summary(app_root: &Path, app_id: &str) -> Option<PersistedLastBuildSummary> {
    let path = app_root.join(PREBUILD_LAST_BUILD_SUMMARY_REL);
    let raw = fs::read_to_string(path).ok()?;
    let summary = serde_json::from_str::<PersistedLastBuildSummary>(&raw).ok()?;
    if summary.app_id.is_empty() || summary.app_id == app_id {
        Some(summary)
    } else {
        None
    }
}

fn build_alerts(
    duplicate_routes: usize,
    orphan_urls: usize,
    stale_ratio: f64,
    failed_slots: usize,
    dirty_slots: usize,
) -> Vec<String> {
    let mut alerts = Vec::new();
    if duplicate_routes > 0 {
        alerts.push(format!(
            "入口重复：发现 {duplicate_routes} 个 scope 拥有多条 route。"
        ));
    }
    if orphan_urls > 0 {
        alerts.push(format!(
            "入口缺失 URL：{orphan_urls} 条 route 没有有效访问地址。"
        ));
    }
    if stale_ratio > 0.10 {
        alerts.push(format!(
            "MRG stale ratio {:.0}% 超过 10% 观察阈值。",
            stale_ratio * 100.0
        ));
    }
    if failed_slots > 0 {
        alerts.push(format!(
            "存在 {failed_slots} 个 failed slot，建议结合 block/layer CLI 排障。"
        ));
    }
    if dirty_slots > 0 {
        alerts.push(format!("当前仍有 {dirty_slots} 个 dirty slot 未完成物化。"));
    }
    alerts
}

fn scope_blockers(scope: &ScopeSummary, access_ready: bool, warmup_ready: bool) -> Vec<String> {
    let mut blockers = Vec::new();
    if scope.route_count == 0 {
        blockers.push("L2:navigation missing".to_string());
    }
    if !access_ready {
        blockers.push("L3:assembly not imported".to_string());
    }
    if scope.failed_slots > 0 {
        blockers.push(format!("L4:failed slots={}", scope.failed_slots));
    }
    if scope.stale_slots > 0 {
        blockers.push(format!("L4:stale slots={}", scope.stale_slots));
    }
    if !warmup_ready && scope.slot_count > 0 {
        blockers.push("L4:warmup not completed".to_string());
    }
    blockers
}

fn scope_is_ready(scope: &ScopeSummary, access_ready: bool, warmup_ready: bool) -> bool {
    scope_blockers(scope, access_ready, warmup_ready).is_empty()
}

fn slot_node_id(node_key: &str, scope_key: &str) -> String {
    format!("mrg:slot:{node_key}@{scope_key}")
}

fn normalized_scope_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "home".to_string()
    } else {
        trimmed.to_string()
    }
}

fn count_duplicate_routes(routes: &[ScopeRoute]) -> usize {
    let mut counts = BTreeMap::<String, usize>::new();
    for route in routes {
        let scope_key = normalized_scope_key(route.scene_id.as_str());
        *counts.entry(scope_key).or_insert(0) += 1;
    }
    counts
        .into_values()
        .map(|count| count.saturating_sub(1))
        .sum()
}

fn stable_short_id(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:08x}", hasher.finish())
}

fn dir_stats(root: &Path) -> (usize, u64) {
    if !root.is_dir() {
        return (0, 0);
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    walk_dir(root, &mut files, &mut bytes);
    (files, bytes)
}

fn walk_dir(path: &Path, files: &mut usize, bytes: &mut u64) {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            walk_dir(entry_path.as_path(), files, bytes);
        } else if entry_path.is_file() {
            *files += 1;
            if let Ok(meta) = entry.metadata() {
                *bytes += meta.len();
            }
        }
    }
}

fn format_age(recorded_at_ms: u64) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(recorded_at_ms);
    let delta = now_ms.saturating_sub(recorded_at_ms);
    if delta < 1_000 {
        "just now".to_string()
    } else if delta < 60_000 {
        format!("{}s ago", delta / 1_000)
    } else if delta < 3_600_000 {
        format!("{}m ago", delta / 60_000)
    } else if delta < 86_400_000 {
        format!("{}h ago", delta / 3_600_000)
    } else {
        format!("{}d ago", delta / 86_400_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_roots_include_runtime_groups() {
        let routes = build_route_entries(&[ScopeRoute {
            scene_id: "home".to_string(),
            url: "/apps/demo/home".to_string(),
            assembly_key: "home@src/scene/home/assembly.mei".to_string(),
        }]);
        let scopes = vec![ScopeSummary {
            scope_key: "home".to_string(),
            route_count: 1,
            slot_count: 3,
            ..Default::default()
        }];
        let roots = build_management_roots(&routes, &scopes, 0, &[], 1, 3);
        assert_eq!(roots.len(), 7);
        assert_eq!(roots[0].children[0].node_id, "ops:overview");
        assert_eq!(roots[1].children[0].node_id, "nav:summary");
        assert_eq!(roots[2].children[0].node_id, "gate:summary");
        assert_eq!(roots[3].children[0].node_id, "warmup:summary");
        assert_eq!(roots[4].children[0].node_id, "mrg:summary");
    }
}
