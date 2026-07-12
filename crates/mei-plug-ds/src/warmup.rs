use std::path::Path;

use mei_host_core::HostContext;
use mei_host_graph::{
    collect_eval_frontier, load_block_artifact, GraphNodeKind, McgRegistryWriter,
};
use mei_lang_kernel::load_mei_config_for_app;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct WarmupTarget {
    pub scope_key: String,
    pub workset_id: String,
    pub owner_resource_id: String,
    pub bundle_key: String,
    pub metric_ids: Vec<String>,
    /// Optional section-level `preview_scope` annotation from the workset
    /// (`preview_scope = "home/t1/r-right-rail/s-warning"`). Used by 0535 dev
    /// `MEI_WARMUP_SCOPE` to filter which worksets participate in prebuild warmup.
    pub preview_scope: Option<String>,
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
    let warmup_filter = WarmupScopeFilter::from_env();
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
                if let Some(mut target) = parse_workset_slot(&scope_key, idx, slot) {
                    let (accepted, reason) = warmup_filter.filter_target(&mut target);
                    tracing::info!(
                        profile = warmup_filter.profile.as_str(),
                        warmup_scopes = %warmup_filter.warmup_scopes.join(","),
                        warmup_metrics = %warmup_filter.warmup_metrics.join(","),
                        workset_id = %target.workset_id,
                        scope = target.preview_scope.as_deref().unwrap_or("-"),
                        metric_ids = %target.metric_ids.join(","),
                        accepted,
                        reason,
                        "warmup workset scope decision"
                    );
                    if accepted {
                        targets.push(target);
                    }
                }
            }
        }
    }
    expand_board_scope_frontier_targets(ctx, &mut targets, &configured_scopes, &warmup_filter)?;
    Ok(targets)
}

/// 0535 dev warmup scope filter: `MEI_DEV_EVAL_PROFILE` + `MEI_WARMUP_SCOPE`.
/// - `full` (default): allow all.
/// - `static`: deny all (skip warmup).
/// - `scoped`: allow only worksets whose `preview_scope` matches a prefix in
///   `MEI_WARMUP_SCOPE`. Worksets without `preview_scope` are denied.
#[derive(Debug, Clone)]
pub struct WarmupScopeFilter {
    pub profile: String,
    pub warmup_scopes: Vec<String>,
    pub warmup_metrics: Vec<String>,
}

impl Default for WarmupScopeFilter {
    fn default() -> Self {
        Self {
            profile: "full".to_string(),
            warmup_scopes: Vec::new(),
            warmup_metrics: Vec::new(),
        }
    }
}

impl WarmupScopeFilter {
    pub fn from_env() -> Self {
        let profile = std::env::var("MEI_DEV_EVAL_PROFILE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let warmup_scopes = std::env::var("MEI_WARMUP_SCOPE")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| part.trim_matches('/').to_string())
            .collect::<Vec<_>>();
        let warmup_metrics = std::env::var("MEI_WARMUP_METRICS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        Self {
            profile,
            warmup_scopes,
            warmup_metrics,
        }
    }

    pub fn decide_target(&self, target: &WarmupTarget) -> (bool, &'static str) {
        match self.profile.as_str() {
            "static" | "off" | "none" => (false, "profile_static"),
            "scoped" => {
                if target
                    .preview_scope
                    .as_ref()
                    .is_some_and(|scope| scope_prefix_matches(scope, &self.warmup_scopes))
                {
                    (true, "scope_allowed")
                } else if target
                    .metric_ids
                    .iter()
                    .any(|metric_id| self.warmup_metrics.contains(metric_id))
                {
                    (true, "metric_allowed")
                } else if self.warmup_scopes.is_empty() && self.warmup_metrics.is_empty() {
                    (false, "empty_warmup_plan")
                } else if target.preview_scope.is_none() {
                    (false, "missing_preview_scope")
                } else {
                    (false, "scope_frozen")
                }
            }
            _ => (true, "profile_full"), // full / default
        }
    }

    pub fn filter_target(&self, target: &mut WarmupTarget) -> (bool, &'static str) {
        let decision = self.decide_target(target);
        if self.profile == "scoped" && decision.0 && !self.warmup_metrics.is_empty() {
            target
                .metric_ids
                .retain(|metric_id| self.warmup_metrics.contains(metric_id));
            if target.metric_ids.is_empty() {
                return (false, "metrics_frozen");
            }
            return (true, "hot_metrics_allowed");
        }
        decision
    }

    pub fn allows_scope(&self, scope_key: &str) -> bool {
        match self.profile.as_str() {
            "static" | "off" | "none" => false,
            "scoped" => {
                if self.warmup_scopes.is_empty() {
                    return false;
                }
                scope_prefix_matches(scope_key, &self.warmup_scopes)
            }
            _ => true,
        }
    }
}

fn scope_prefix_matches(scope: &str, prefixes: &[String]) -> bool {
    let scope = scope.trim().trim_matches('/');
    if scope.is_empty() {
        return false;
    }
    prefixes.iter().any(|prefix| {
        let prefix = prefix.trim().trim_matches('/');
        if prefix.is_empty() || prefix == "*" {
            return true;
        }
        scope == prefix || scope.starts_with(&format!("{prefix}/"))
    })
}

fn expand_board_scope_frontier_targets(
    ctx: &HostContext,
    targets: &mut Vec<WarmupTarget>,
    configured_scopes: &BTreeSet<String>,
    warmup_filter: &WarmupScopeFilter,
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
        if !warmup_filter.allows_scope(scope) {
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
    let preview_scope = args
        .get("preview_scope")
        .and_then(Value::as_str)
        .map(str::to_string);
    if bundle_key.is_empty() || metric_ids.is_empty() {
        return None;
    }
    Some(WarmupTarget {
        scope_key: scope_key.to_string(),
        workset_id: format!("workset:{scope_key}:{idx}"),
        owner_resource_id: format!("__world_metrics__::{bundle_key}"),
        bundle_key,
        metric_ids,
        preview_scope,
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
                preview_scope: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(scope: &str, metrics: &[&str]) -> WarmupTarget {
        WarmupTarget {
            scope_key: "home".to_string(),
            workset_id: format!("workset:{scope}"),
            owner_resource_id: format!("owner:{scope}"),
            bundle_key: format!("bundle:{scope}"),
            metric_ids: metrics.iter().map(|metric| (*metric).to_string()).collect(),
            preview_scope: Some(scope.to_string()),
        }
    }

    #[test]
    fn scoped_golden_accepts_only_warning_workset_with_three_slots() {
        let filter = WarmupScopeFilter {
            profile: "scoped".to_string(),
            warmup_scopes: vec!["home/t1/r-right-rail/s-warning".to_string()],
            warmup_metrics: Vec::new(),
        };
        let targets = [
            target(
                "home/t1/r-right-rail/s-warning",
                &[
                    "supervision_items_count",
                    "supervision_models_count",
                    "warnings_count",
                ],
            ),
            target(
                "home/t1/r-right-rail/s-enforcement",
                &["enforcement_objects_count"],
            ),
        ];
        let accepted = targets
            .iter()
            .filter(|target| filter.decide_target(target).0)
            .collect::<Vec<_>>();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].metric_ids.len(), 3);
        assert_eq!(filter.decide_target(&targets[1]), (false, "scope_frozen"));
    }

    #[test]
    fn scoped_metric_override_keeps_only_hot_metrics() {
        let filter = WarmupScopeFilter {
            profile: "scoped".to_string(),
            warmup_scopes: Vec::new(),
            warmup_metrics: vec!["hot_metric".to_string()],
        };
        let mut candidate = target("home/frozen", &["hot_metric", "lazy_metric"]);
        assert_eq!(
            filter.filter_target(&mut candidate),
            (true, "hot_metrics_allowed")
        );
        assert_eq!(candidate.metric_ids, vec!["hot_metric"]);
    }

    #[test]
    fn scoped_metric_override_refines_an_allowed_hot_scope() {
        let filter = WarmupScopeFilter {
            profile: "scoped".to_string(),
            warmup_scopes: vec!["home/t1/r-right-rail/s-warning".to_string()],
            warmup_metrics: vec!["warnings_count".to_string()],
        };
        let mut candidate = target(
            "home/t1/r-right-rail/s-warning",
            &[
                "warnings_count",
                "supervision_models_count",
                "supervision_items_count",
            ],
        );
        assert_eq!(
            filter.filter_target(&mut candidate),
            (true, "hot_metrics_allowed")
        );
        assert_eq!(candidate.metric_ids, vec!["warnings_count"]);
    }
}
