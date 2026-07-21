use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{
    evaluate_runtime_metric_defs_with_plan_and_dag, DatasetView, EvalPlanNodeKind, MetricContract,
    RuntimeMetricEvalReport, RuntimeMetricEvalScope,
};
use serde_json::Value;

use super::eval_artifact::{
    eval_plan_semantic_revision_key, load_eval_metric_node_pack, load_or_build_eval_plan_artifact,
    store_eval_metric_node_pack,
};
use super::l1_project::{
    metric_contract_eligible_for_node_pack, metric_id_eligible_for_node_pack, L1PinPolicy,
};

pub(crate) struct EvalPlanExecutionOutcome {
    pub metrics_map: BTreeMap<String, MetricContract>,
    pub eval_report: RuntimeMetricEvalReport,
    pub eval_artifact_load_ms: u64,
    pub eval_artifact_hit: bool,
    pub eval_node_artifact_load_ms: u64,
    pub eval_node_artifact_hits: u64,
    pub eval_node_artifact_stores: u64,
}

pub(crate) fn execute_runtime_eval_plan_artifacts(
    app_root: &Path,
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    metric_defs: &BTreeMap<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    base_rows: &[Value],
    scope: &RuntimeMetricEvalScope,
    enable_node_artifact_cache: bool,
) -> Result<EvalPlanExecutionOutcome> {
    let (persisted_eval_plan, eval_artifact_load_ms, eval_artifact_hit) =
        load_or_build_eval_plan_artifact(
            app_root,
            owner_resource_id,
            requested_metric_ids,
            metric_defs,
            datasets,
            scope,
        )?;
    let semantic_revision_key =
        eval_plan_semantic_revision_key(owner_resource_id, requested_metric_ids, metric_defs);
    let mut cached_metrics = BTreeMap::new();
    let mut eval_node_artifact_load_ms = 0u64;
    let mut eval_node_artifact_hits = 0u64;
    // Node-pack only caches L1-shaped nodes (no __scalar_rowset__ / oversized).
    let cacheable_metric_ids: Vec<String> = if enable_node_artifact_cache {
        persisted_eval_plan
            .nodes
            .values()
            .filter(|node| node.kind == EvalPlanNodeKind::MetricEval)
            .filter_map(|node| node.metric_id.clone())
            .filter(|metric_id| metric_id_eligible_for_node_pack(metric_id.as_str()))
            .collect()
    } else {
        Vec::new()
    };
    let expected_metric_nodes = cacheable_metric_ids.len();
    if enable_node_artifact_cache {
        let (pack_metrics, load_ms, hit_count) = load_eval_metric_node_pack(
            app_root,
            owner_resource_id,
            semantic_revision_key.as_str(),
            scope,
        )?;
        eval_node_artifact_load_ms = load_ms;
        eval_node_artifact_hits = hit_count as u64;
        cached_metrics = pack_metrics;
    }
    let (metrics_map, mut eval_report) = evaluate_runtime_metric_defs_with_plan_and_dag(
        metric_defs,
        base_rows,
        datasets,
        scope,
        &persisted_eval_plan,
        &cached_metrics,
    )?;
    eval_report.eval_plan = persisted_eval_plan.clone();
    let mut eval_node_artifact_stores = 0u64;
    if enable_node_artifact_cache {
        let pin_policy = L1PinPolicy::default();
        let full_hit = expected_metric_nodes > 0
            && cacheable_metric_ids
                .iter()
                .all(|metric_id| cached_metrics.contains_key(metric_id));
        let mut to_store = BTreeMap::new();
        if !full_hit {
            for node in persisted_eval_plan.nodes.values() {
                if node.kind != EvalPlanNodeKind::MetricEval {
                    continue;
                }
                let Some(metric_id) = node.metric_id.as_deref() else {
                    continue;
                };
                if cached_metrics.contains_key(metric_id) {
                    continue;
                }
                let Some(metric) = metrics_map.get(metric_id) else {
                    continue;
                };
                if !metric_contract_eligible_for_node_pack(metric_id, metric, &pin_policy) {
                    continue;
                }
                to_store.insert(node.id.clone(), (metric_id.to_string(), metric.clone()));
            }
        }
        eval_node_artifact_stores = store_eval_metric_node_pack(
            app_root,
            owner_resource_id,
            semantic_revision_key.as_str(),
            scope,
            &to_store,
            full_hit,
        )?;
    }
    Ok(EvalPlanExecutionOutcome {
        metrics_map,
        eval_report,
        eval_artifact_load_ms,
        eval_artifact_hit,
        eval_node_artifact_load_ms,
        eval_node_artifact_hits,
        eval_node_artifact_stores,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use mei_lang_kernel::{QueryState, SourceDecl};
    use serde_json::json;

    use super::*;

    fn temp_app_root(name: &str) -> PathBuf {
        let now_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|dur| dur.as_millis() as u64)
            .unwrap_or(0);
        let app_root = std::env::temp_dir().join(format!(
            "mei-eval-execute-{name}-{}-{}",
            std::process::id(),
            now_epoch_ms
        ));
        let env_dir = app_root.join("env").join("WS-20260713.0");
        let build_dir = env_dir.join("build");
        let _ = fs::create_dir_all(&build_dir);
        let _ = fs::create_dir_all(env_dir.join("var"));
        let current = app_root.join("env").join("current");
        let _ = fs::remove_file(&current);
        let _ = fs::remove_dir_all(&current);
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink("WS-20260713.0", &current);
        }
        mei_lang_kernel::set_prebuild_build_root_override(&app_root, Some(&build_dir));
        app_root
    }

    fn owner_dataset(metric_defs: &BTreeMap<String, Value>) -> DatasetView {
        DatasetView {
            id: "owner::metrics".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "inline".to_string(),
                path: String::new(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: metric_defs.clone(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: BTreeMap::new(),
        }
    }

    #[test]
    fn eval_plan_execution_reuses_metric_node_artifacts() {
        let app_root = temp_app_root("node-artifact");
        let _ = fs::remove_dir_all(&app_root);
        let metric_defs = BTreeMap::from([(
            "metric-a".to_string(),
            json!({
                "id": "metric-a",
                "shape": "scalar",
                "values": {
                    "count": {
                        "__kind": "analysis_expr",
                        "type": "count"
                    }
                }
            }),
        )]);
        let dataset = owner_dataset(&metric_defs);
        let datasets = BTreeMap::from([(dataset.id.clone(), dataset.clone())]);
        let base_rows = vec![json!({"id": 1}), json!({"id": 2})];
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: dataset.id.clone(),
            query_state: QueryState::default(),
            ..RuntimeMetricEvalScope::default()
        };
        let request = vec!["metric-a".to_string()];

        let cold = execute_runtime_eval_plan_artifacts(
            &app_root,
            dataset.id.as_str(),
            &request,
            &metric_defs,
            &datasets,
            &base_rows,
            &scope,
            true,
        )
        .expect("cold execution");
        assert_eq!(cold.eval_node_artifact_hits, 0);
        assert!(cold.eval_node_artifact_stores > 0);
        assert!(cold.metrics_map.contains_key("metric-a"));

        let warm = execute_runtime_eval_plan_artifacts(
            &app_root,
            dataset.id.as_str(),
            &request,
            &metric_defs,
            &datasets,
            &base_rows,
            &scope,
            true,
        )
        .expect("warm execution");
        assert!(warm.eval_node_artifact_hits > 0);
        assert_eq!(
            warm.eval_node_artifact_stores, 0,
            "full node pack hit must skip rewrite"
        );
        assert!(warm.metrics_map.contains_key("metric-a"));

        mei_lang_kernel::clear_prebuild_build_root_override();
        let _ = fs::remove_dir_all(&app_root);
    }
}
