use std::path::Path;

use mei_host_core::HostContext;
use mei_host_graph::{collect_eval_frontier, load_block_artifact, GraphNodeKind, McgRegistryWriter};
use mei_lang_kernel::{load_mei_config_for_app};
use serde_json::Value;
use std::collections::BTreeSet;

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
    let configured_scopes: BTreeSet<String> = load_mei_config_for_app(app_root.as_path(), None)
        .runtime
        .client_bootstrap
        .map(|cfg| cfg.scopes.into_iter().collect())
        .unwrap_or_default();
    let mut targets = Vec::new();

    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::WarmupPolicy)
    {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(app_root.as_path(), pref)? else {
            continue;
        };
        let payload: Value = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let scope_key = extract_scope_key(&payload).unwrap_or_else(|| "home".to_string());
        if policy_filter != "all" && scope_key != policy_filter {
            let allowed_by_config =
                policy_filter == "home" && configured_scopes.contains(&scope_key);
            if !allowed_by_config {
                continue;
            }
        }
        if let Some(slots) = payload.get("slots").and_then(Value::as_array) {
            for (idx, slot) in slots.iter().enumerate() {
                if let Some(target) = parse_workset_slot(&scope_key, idx, slot) {
                    targets.push(target);
                }
            }
        }
    }
    expand_board_scope_frontier_targets(ctx, &mut targets, &configured_scopes)?;
    Ok(targets)
}

fn expand_board_scope_frontier_targets(
    ctx: &HostContext,
    targets: &mut Vec<WarmupTarget>,
    configured_scopes: &BTreeSet<String>,
) -> anyhow::Result<()> {
    let mut known: BTreeSet<String> = targets
        .iter()
        .map(|target| {
            format!(
                "{}|{}|{}",
                target.scope_key,
                target.workset_id,
                target.metric_ids.join(",")
            )
        })
        .collect();
    for scope in configured_scopes {
        if scope == "home" {
            continue;
        }
        let metrics = collect_eval_frontier(ctx, scope.as_str())?;
        if metrics.is_empty() {
            continue;
        }
        for target in frontier_targets_from_metrics(scope.as_str(), &metrics) {
            let key = format!(
                "{}|{}|{}",
                target.scope_key,
                target.workset_id,
                target.metric_ids.join(",")
            );
            if known.insert(key) {
                targets.push(target);
            }
        }
    }
    Ok(())
}

fn extract_scope_key(payload: &Value) -> Option<String> {
    payload.get("scope").and_then(|scope| {
        if let Some(args) = scope.get("__args").and_then(Value::as_object) {
            args.get("arg0")
                .and_then(|v| v.as_str())
                .map(str::to_string)
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

pub fn collect_all_t2_page_scenes(source_root: &Path, app_id: &str) -> Vec<String> {
    mei_host_graph::collect_all_t2_page_scenes(source_root, app_id)
}

pub fn frontier_targets_from_metrics(
    _scope_key: &str,
    metrics: &[mei_host_graph::FrontierMetric],
) -> Vec<WarmupTarget> {
    let mut grouped: std::collections::BTreeMap<(String, String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for metric in metrics {
        grouped
            .entry((
                metric.scope_key.clone(),
                metric.owner_resource_id.clone(),
                metric.bundle_key.clone(),
            ))
            .or_default()
            .push(metric.metric_id.clone());
    }
    grouped
        .into_iter()
        .enumerate()
        .map(|(idx, ((scope_key, owner, bundle_key), mut metric_ids))| {
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
