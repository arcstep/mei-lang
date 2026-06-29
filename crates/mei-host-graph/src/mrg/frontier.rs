use std::collections::BTreeMap;

use mei_host_core::HostContext;
use mei_lang_kernel::CompiledApp;
use serde_json::Value;

use crate::assemble_scope_from_registry;
use crate::load_block_artifact;
use crate::mcg::registry::McgRegistryWriter;
use crate::types::GraphNodeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierMetric {
    pub scope_key: String,
    pub metric_id: String,
    pub owner_resource_id: String,
    pub bundle_key: String,
}

pub fn collect_eval_frontier(
    ctx: &HostContext,
    scope_key: &str,
) -> anyhow::Result<Vec<FrontierMetric>> {
    let outcome =
        assemble_scope_from_registry(ctx.workspace_root.as_path(), ctx.app_id.as_str(), scope_key)?
            .ok_or_else(|| anyhow::anyhow!("scene `{scope_key}` not assembled"))?;
    Ok(collect_metrics_from_compiled(scope_key, &outcome.compiled))
}

pub fn collect_eval_frontier_with_hops(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<Vec<FrontierMetric>> {
    let mut metrics = collect_eval_frontier(ctx, scope_key)?;
    if hops == 0 {
        return Ok(metrics);
    }
    let linked = linked_board_scenes_for_scope(ctx, scope_key, hops)?;
    for board_scene in linked {
        let mut board_metrics = collect_eval_frontier(ctx, board_scene.as_str())?;
        metrics.append(&mut board_metrics);
    }
    dedupe_frontier(metrics)
}

fn collect_metrics_from_compiled(scope_key: &str, compiled: &CompiledApp) -> Vec<FrontierMetric> {
    let mut out = Vec::new();
    if let Some(contract) = compiled.scene_contract.as_ref() {
        for panel in &contract.panels {
            if let Ok(value) = serde_json::to_value(&panel.blocks) {
                walk_value_for_metrics(scope_key, &value, compiled, &mut out);
            }
            if let Some(head) = panel.head.as_ref() {
                if let Ok(value) = serde_json::to_value(head.as_ref()) {
                    walk_value_for_metrics(scope_key, &value, compiled, &mut out);
                }
            }
        }
    }
    dedupe_frontier(out).unwrap_or_default()
}

fn walk_value_for_metrics(
    scope_key: &str,
    value: &Value,
    compiled: &CompiledApp,
    out: &mut Vec<FrontierMetric>,
) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("metric_ref") {
                if let Some(metric_id) = map
                    .get("__args")
                    .and_then(|args| args.get("arg0"))
                    .and_then(Value::as_str)
                {
                    let bundle_key = map
                        .get("__args")
                        .and_then(|args| args.get("bundle"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if let Some((owner, bundle)) =
                        resolve_metric_owner(compiled, metric_id, bundle_key)
                    {
                        out.push(FrontierMetric {
                            scope_key: scope_key.to_string(),
                            metric_id: metric_id.to_string(),
                            owner_resource_id: owner,
                            bundle_key: bundle,
                        });
                    }
                }
            }
            for entry in map.values() {
                walk_value_for_metrics(scope_key, entry, compiled, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_value_for_metrics(scope_key, item, compiled, out);
            }
        }
        _ => {}
    }
}

fn resolve_metric_owner(
    compiled: &CompiledApp,
    metric_id: &str,
    bundle_key: &str,
) -> Option<(String, String)> {
    if !bundle_key.is_empty() {
        return Some((
            format!("__world_metrics__::{bundle_key}"),
            bundle_key.to_string(),
        ));
    }
    for resource in &compiled.resources {
        if let Some(dataset) = resource.dataset.as_ref() {
            if dataset.runtime_metric_defs.contains_key(metric_id) {
                return Some((resource.id.clone(), resource.id.clone()));
            }
        }
    }
    None
}

fn direct_linked_board_scenes(ctx: &HostContext, scope_key: &str) -> anyhow::Result<Vec<String>> {
    let registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let mut scenes = Vec::new();
    for node in registry.nodes_of_kind(GraphNodeKind::Navigation) {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(ctx.app_root().as_path(), pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let from_scope = payload
            .get("from_scope")
            .or_else(|| payload.get("scope"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if from_scope != scope_key && !node.id.key.contains(scope_key) {
            continue;
        }
        if let Some(board) = payload
            .get("board")
            .or_else(|| payload.get("target_scene"))
            .and_then(Value::as_str)
        {
            scenes.push(board.to_string());
        } else if let Some(scene) = payload.get("scene").and_then(Value::as_str) {
            scenes.push(scene.to_string());
        }
    }
    scenes.sort();
    scenes.dedup();
    Ok(scenes)
}

pub fn linked_board_scenes_for_scope(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<Vec<String>> {
    if hops == 0 {
        return Ok(Vec::new());
    }
    let mut seen = std::collections::BTreeSet::from([scope_key.to_string()]);
    let mut frontier = vec![scope_key.to_string()];
    let mut out = Vec::new();
    for _ in 0..hops {
        let mut next = Vec::new();
        for scope in frontier {
            for linked in direct_linked_board_scenes(ctx, scope.as_str())? {
                if seen.insert(linked.clone()) {
                    out.push(linked.clone());
                    next.push(linked);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    Ok(out)
}

pub fn record_navigation_edges_for_scope(
    ctx: &HostContext,
    scope_key: &str,
    hops: usize,
) -> anyhow::Result<usize> {
    if hops == 0 {
        return Ok(0);
    }
    let linked = direct_linked_board_scenes(ctx, scope_key)?;
    let mut registry = crate::mrg::registry::MrgRegistryWriter::load(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
    );
    let mut added = 0usize;
    for scene in linked {
        added +=
            crate::mrg::warmup::record_navigation_edge(&mut registry, scope_key, scene.as_str());
    }
    if added > 0 {
        registry.finalize();
        crate::mrg::registry::MrgRegistryWriter::save(ctx.workspace_root.as_path(), &registry)?;
    }
    Ok(added)
}

fn dedupe_frontier(metrics: Vec<FrontierMetric>) -> anyhow::Result<Vec<FrontierMetric>> {
    let mut map = BTreeMap::new();
    for metric in metrics {
        map.insert(
            (
                metric.scope_key.clone(),
                metric.metric_id.clone(),
                metric.owner_resource_id.clone(),
            ),
            metric,
        );
    }
    Ok(map.into_values().collect())
}
