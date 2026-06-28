use std::path::Path;

use mei_host_core::HostContext;
use mei_host_graph::{
    load_block_artifact, GraphNodeKind, McgRegistryWriter,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct WarmupTarget {
    pub scope_key: String,
    pub workset_id: String,
    pub owner_resource_id: String,
    pub bundle_key: String,
    pub metric_ids: Vec<String>,
}

pub fn collect_warmup_targets(
    ctx: &HostContext,
    policy: Option<&str>,
) -> anyhow::Result<Vec<WarmupTarget>> {
    let registry = McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    let app_root = ctx.app_root();
    let policy_filter = policy.unwrap_or("home");
    let mut targets = Vec::new();

    for node in registry.nodes.iter().filter(|n| n.id.kind == GraphNodeKind::WarmupPolicy) {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(app_root.as_path(), pref)? else {
            continue;
        };
        let payload: Value = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let scope_key = extract_scope_key(&payload).unwrap_or_else(|| "home".to_string());
        if policy_filter != "all" && scope_key != policy_filter {
            continue;
        }
        if let Some(slots) = payload.get("slots").and_then(Value::as_array) {
            for (idx, slot) in slots.iter().enumerate() {
                if let Some(target) = parse_workset_slot(&scope_key, idx, slot) {
                    targets.push(target);
                }
            }
        }
    }
    Ok(targets)
}

fn extract_scope_key(payload: &Value) -> Option<String> {
    payload.get("scope").and_then(|scope| {
        if let Some(args) = scope.get("__args").and_then(Value::as_object) {
            args.get("arg0").and_then(|v| v.as_str()).map(str::to_string)
        } else {
            scope.as_str().map(str::to_string)
        }
    })
}

fn parse_workset_slot(scope_key: &str, idx: usize, slot: &Value) -> Option<WarmupTarget> {
    let args = slot.get("__args").and_then(Value::as_object)?;
    let bundle_key = args
        .get("bundle")
        .and_then(|b| b.get("__args"))
        .and_then(|a| a.get("arg0"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let metric_ids = args
        .get("metrics")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if bundle_key.is_empty() || metric_ids.is_empty() {
        return None;
    }
    Some(WarmupTarget {
        scope_key: scope_key.to_string(),
        workset_id: format!("workset:{scope_key}:{idx}"),
        owner_resource_id: format!("__world_metrics__::{bundle_key}"),
        bundle_key,
        metric_ids,
    })
}

pub fn collect_all_board_scenes(source_root: &Path, app_id: &str) -> Vec<String> {
    mei_host_graph::collect_all_board_scenes(source_root, app_id)
}

pub fn frontier_targets_from_metrics(
    scope_key: &str,
    metrics: &[mei_host_graph::FrontierMetric],
) -> Vec<WarmupTarget> {
    let mut grouped: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for metric in metrics {
        grouped
            .entry((metric.owner_resource_id.clone(), metric.bundle_key.clone()))
            .or_default()
            .push(metric.metric_id.clone());
    }
    grouped
        .into_iter()
        .enumerate()
        .map(|(idx, ((owner, bundle_key), mut metric_ids))| {
            metric_ids.sort();
            metric_ids.dedup();
            WarmupTarget {
                scope_key: scope_key.to_string(),
                workset_id: format!("frontier:{scope_key}:{idx}"),
                owner_resource_id: owner,
                bundle_key,
                metric_ids,
            }
        })
        .collect()
}
