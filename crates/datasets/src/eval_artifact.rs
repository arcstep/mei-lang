use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    build_runtime_eval_plan, resolve_app_eval_cache_root, DatasetView, EvalPlan, MetricContract,
    RuntimeMetricEvalScope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::util::read_json_artifact_lenient;
use crate::{eval_node_cache_key, metric_scope_cache_key, RuntimeMetricWorkset};

const EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-workset-artifact-v2";
const EVAL_PLAN_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-plan-artifact-v2";
const EVAL_METRIC_NODE_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-metric-node-artifact-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedWorksetArtifact {
    schema_version: String,
    owner_resource_id: String,
    requested_metric_ids: Vec<String>,
    /// Added in v2; missing in legacy v1 artifacts and treated as stale on read.
    #[serde(default)]
    semantic_revision_key: String,
    closure_metric_ids: Vec<String>,
    eval_metric_ids: Option<Vec<String>>,
    defs_for_hydrate: BTreeMap<String, Value>,
    generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEvalPlanArtifact {
    schema_version: String,
    owner_resource_id: String,
    requested_metric_ids: Vec<String>,
    #[serde(default)]
    semantic_revision_key: String,
    scope_key: String,
    dependency_revision_key: String,
    eval_plan: EvalPlan,
    generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEvalMetricNodeArtifact {
    schema_version: String,
    owner_resource_id: String,
    node_id: String,
    metric_id: String,
    semantic_revision_key: String,
    scope_key: String,
    dependency_revision_key: String,
    metric: MetricContract,
    generated_at_ms: u64,
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_key(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn eval_artifact_root(app_root: &Path) -> PathBuf {
    resolve_app_eval_cache_root(app_root)
}

fn dataset_semantic_revision_key(owner_resource_id: &str, dataset: &DatasetView) -> String {
    let payload = serde_json::json!({
        "owner_resource_id": owner_resource_id.trim(),
        "runtime_metric_defs": dataset.runtime_metric_defs,
        "runtime_analysis_graph": dataset.runtime_analysis_graph,
        "runtime_analysis_contracts": dataset.runtime_analysis_contracts,
    });
    hash_key(&serde_json::to_string(&payload).unwrap_or_default())
}

pub(crate) fn eval_plan_semantic_revision_key(
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    let payload = serde_json::json!({
        "owner_resource_id": owner_resource_id.trim(),
        "requested_metric_ids": requested_metric_ids,
        "metric_defs": metric_defs,
    });
    hash_key(&serde_json::to_string(&payload).unwrap_or_default())
}

fn eval_metric_node_artifact_path(
    app_root: &Path,
    owner_resource_id: &str,
    node_id: &str,
    scope: &RuntimeMetricEvalScope,
) -> PathBuf {
    let key = format!(
        "metric_node|owner={}|node={}|scope={}",
        owner_resource_id.trim(),
        node_id.trim(),
        eval_node_cache_key("metric_node", scope)
    );
    eval_artifact_root(app_root)
        .join("node-metric")
        .join(format!("{}.json", hash_key(&key)))
}

fn workset_artifact_path(
    app_root: &Path,
    owner_resource_id: &str,
    requested_metric_ids: &[String],
) -> PathBuf {
    let key = format!(
        "workset|owner={}|metrics={}",
        owner_resource_id.trim(),
        metric_scope_cache_key(requested_metric_ids)
    );
    eval_artifact_root(app_root)
        .join("workset")
        .join(format!("{}.json", hash_key(&key)))
}

fn eval_plan_artifact_path(
    app_root: &Path,
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    scope: &RuntimeMetricEvalScope,
) -> PathBuf {
    let key = format!(
        "plan|owner={}|metrics={}|scope={}",
        owner_resource_id.trim(),
        metric_scope_cache_key(requested_metric_ids),
        eval_node_cache_key("eval_plan", scope)
    );
    eval_artifact_root(app_root)
        .join("plan")
        .join(format!("{}.json", hash_key(&key)))
}

fn workset_artifact_is_current(
    artifact: &PersistedWorksetArtifact,
    semantic_revision_key: &str,
) -> bool {
    artifact.schema_version == EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION
        && !artifact.semantic_revision_key.is_empty()
        && artifact.semantic_revision_key == semantic_revision_key
}

fn eval_plan_artifact_is_current(
    artifact: &PersistedEvalPlanArtifact,
    semantic_revision_key: &str,
    dependency_revision_key: &str,
) -> bool {
    artifact.schema_version == EVAL_PLAN_ARTIFACT_SCHEMA_VERSION
        && !artifact.semantic_revision_key.is_empty()
        && artifact.dependency_revision_key == dependency_revision_key
        && artifact.semantic_revision_key == semantic_revision_key
}

fn write_json_artifact<T: Serialize>(path: &Path, artifact: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create eval artifact dir {}", parent.display()))?;
    }
    fs::write(path, serde_json::to_string_pretty(artifact)?)
        .with_context(|| format!("write eval artifact {}", path.display()))?;
    Ok(())
}

pub(crate) fn load_or_build_runtime_metric_workset_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    dataset: &DatasetView,
) -> Result<(RuntimeMetricWorkset, u64, bool)> {
    let started = Instant::now();
    let path = workset_artifact_path(app_root, owner_resource_id, requested_metric_ids);
    let semantic_revision_key = dataset_semantic_revision_key(owner_resource_id, dataset);
    if let Some(artifact) =
        read_json_artifact_lenient::<PersistedWorksetArtifact>(&path, "workset")?
    {
        if workset_artifact_is_current(&artifact, &semantic_revision_key) {
            return Ok((
                RuntimeMetricWorkset {
                    closure_metric_ids: artifact.closure_metric_ids,
                    eval_metric_ids: artifact.eval_metric_ids,
                    defs_for_hydrate: artifact.defs_for_hydrate,
                },
                started.elapsed().as_millis() as u64,
                true,
            ));
        }
    }
    let inner = crate::metric_cache_key::runtime_metric_workset(
        owner_resource_id,
        requested_metric_ids,
        dataset,
    );
    let workset = RuntimeMetricWorkset {
        closure_metric_ids: inner.closure_metric_ids,
        eval_metric_ids: inner.eval_metric_ids,
        defs_for_hydrate: inner.defs_for_hydrate,
    };
    write_json_artifact(
        &path,
        &PersistedWorksetArtifact {
            schema_version: EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION.to_string(),
            owner_resource_id: owner_resource_id.to_string(),
            requested_metric_ids: requested_metric_ids.to_vec(),
            semantic_revision_key,
            closure_metric_ids: workset.closure_metric_ids.clone(),
            eval_metric_ids: workset.eval_metric_ids.clone(),
            defs_for_hydrate: workset.defs_for_hydrate.clone(),
            generated_at_ms: now_epoch_ms(),
        },
    )?;
    Ok((workset, started.elapsed().as_millis() as u64, false))
}

pub(crate) fn load_or_build_eval_plan_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    metric_defs: &BTreeMap<String, Value>,
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> Result<(EvalPlan, u64, bool)> {
    let started = Instant::now();
    let path = eval_plan_artifact_path(app_root, owner_resource_id, requested_metric_ids, scope);
    let semantic_revision_key =
        eval_plan_semantic_revision_key(owner_resource_id, requested_metric_ids, metric_defs);
    if let Some(artifact) = read_json_artifact_lenient::<PersistedEvalPlanArtifact>(&path, "plan")?
    {
        if eval_plan_artifact_is_current(
            &artifact,
            &semantic_revision_key,
            scope.dependency_revision_key.as_str(),
        ) {
            return Ok((
                artifact.eval_plan,
                started.elapsed().as_millis() as u64,
                true,
            ));
        }
    }
    let selected_metric_ids = if requested_metric_ids.is_empty() {
        None
    } else {
        Some(requested_metric_ids)
    };
    let eval_plan = build_runtime_eval_plan(metric_defs, selected_metric_ids, datasets, scope);
    write_json_artifact(
        &path,
        &PersistedEvalPlanArtifact {
            schema_version: EVAL_PLAN_ARTIFACT_SCHEMA_VERSION.to_string(),
            owner_resource_id: owner_resource_id.to_string(),
            requested_metric_ids: requested_metric_ids.to_vec(),
            semantic_revision_key,
            scope_key: eval_node_cache_key("eval_plan", scope),
            dependency_revision_key: scope.dependency_revision_key.clone(),
            eval_plan: eval_plan.clone(),
            generated_at_ms: now_epoch_ms(),
        },
    )?;
    Ok((eval_plan, started.elapsed().as_millis() as u64, false))
}

pub(crate) fn load_eval_metric_node_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    node_id: &str,
    metric_id: &str,
    semantic_revision_key: &str,
    scope: &RuntimeMetricEvalScope,
) -> Result<Option<(MetricContract, u64)>> {
    let started = Instant::now();
    let path = eval_metric_node_artifact_path(app_root, owner_resource_id, node_id, scope);
    let Some(artifact) =
        read_json_artifact_lenient::<PersistedEvalMetricNodeArtifact>(&path, "metric-node")?
    else {
        return Ok(None);
    };
    if artifact.schema_version != EVAL_METRIC_NODE_ARTIFACT_SCHEMA_VERSION
        || artifact.owner_resource_id != owner_resource_id
        || artifact.node_id != node_id
        || artifact.metric_id != metric_id
        || artifact.semantic_revision_key != semantic_revision_key
        || artifact.scope_key != eval_node_cache_key("metric_node", scope)
        || artifact.dependency_revision_key != scope.dependency_revision_key
    {
        return Ok(None);
    }
    Ok(Some((
        artifact.metric,
        started.elapsed().as_millis() as u64,
    )))
}

pub(crate) fn store_eval_metric_node_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    node_id: &str,
    metric_id: &str,
    semantic_revision_key: &str,
    scope: &RuntimeMetricEvalScope,
    metric: &MetricContract,
) -> Result<()> {
    let path = eval_metric_node_artifact_path(app_root, owner_resource_id, node_id, scope);
    write_json_artifact(
        &path,
        &PersistedEvalMetricNodeArtifact {
            schema_version: EVAL_METRIC_NODE_ARTIFACT_SCHEMA_VERSION.to_string(),
            owner_resource_id: owner_resource_id.to_string(),
            node_id: node_id.to_string(),
            metric_id: metric_id.to_string(),
            semantic_revision_key: semantic_revision_key.to_string(),
            scope_key: eval_node_cache_key("metric_node", scope),
            dependency_revision_key: scope.dependency_revision_key.clone(),
            metric: metric.clone(),
            generated_at_ms: now_epoch_ms(),
        },
    )
}

pub(crate) fn clear_eval_artifact_store(app_root: &Path) -> usize {
    let root = eval_artifact_root(app_root);
    if !root.exists() {
        return 0;
    }
    let count = count_files_recursively(&root);
    let _ = fs::remove_dir_all(&root);
    count
}

pub(crate) fn eval_artifact_hydrate_dataset_ids(
    metric_defs: &BTreeMap<String, Value>,
) -> BTreeSet<String> {
    crate::metric_hydrate::collect_dataset_ids_from_metric_defs(metric_defs)
}

fn count_files_recursively(path: &Path) -> usize {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|child| {
            if child.is_file() {
                1
            } else if child.is_dir() {
                count_files_recursively(&child)
            } else {
                0
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mei_lang_kernel::SourceDecl;

    use super::*;

    fn minimal_dataset() -> DatasetView {
        DatasetView {
            id: "test-dataset".to_string(),
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
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: BTreeMap::new(),
        }
    }

    fn temp_app_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mei-eval-artifact-{name}-{}-{}",
            std::process::id(),
            now_epoch_ms()
        ))
    }

    fn workset_artifact_file(app_root: &Path) -> PathBuf {
        fs::read_dir(
            mei_lang_kernel::resolve_app_eval_cache_root(app_root).join("workset"),
        )
            .expect("workset dir")
            .next()
            .expect("workset artifact")
            .expect("workset entry")
            .path()
    }

    #[test]
    fn corrupt_workset_artifact_is_rebuilt_without_error() {
        let app_root = temp_app_root("corrupt");
        let _ = fs::remove_dir_all(&app_root);
        let dataset = minimal_dataset();
        let owner = "owner::metrics";
        let metrics = vec!["metric-a".to_string()];

        load_or_build_runtime_metric_workset_artifact(&app_root, owner, &metrics, &dataset)
            .expect("initial build");
        let path = workset_artifact_file(&app_root);
        fs::write(&path, "{not-json").expect("write corrupt artifact");

        let (_, _, hit) =
            load_or_build_runtime_metric_workset_artifact(&app_root, owner, &metrics, &dataset)
                .expect("corrupt artifact must not fail request");
        assert!(!hit);

        let content = fs::read_to_string(&path).expect("rebuilt artifact");
        assert!(content.contains(EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION));
        assert!(content.contains("semantic_revision_key"));

        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn legacy_v1_workset_artifact_is_rebuilt_without_error() {
        let app_root = temp_app_root("legacy-v1");
        let _ = fs::remove_dir_all(&app_root);
        let dataset = minimal_dataset();
        let owner = "owner::metrics";
        let metrics = vec!["metric-a".to_string()];

        load_or_build_runtime_metric_workset_artifact(&app_root, owner, &metrics, &dataset)
            .expect("initial build");
        let path = workset_artifact_file(&app_root);
        fs::write(
            &path,
            r#"{
  "schema_version": "mei-eval-workset-artifact-v1",
  "owner_resource_id": "owner::metrics",
  "requested_metric_ids": ["metric-a"],
  "closure_metric_ids": ["metric-a"],
  "eval_metric_ids": ["metric-a"],
  "defs_for_hydrate": {},
  "generated_at_ms": 1
}"#,
        )
        .expect("write legacy artifact");

        let (_, _, hit) =
            load_or_build_runtime_metric_workset_artifact(&app_root, owner, &metrics, &dataset)
                .expect("legacy artifact must not fail request");
        assert!(!hit);

        let content = fs::read_to_string(&path).expect("rebuilt artifact");
        assert!(content.contains(EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION));
        assert!(content.contains("semantic_revision_key"));

        let _ = fs::remove_dir_all(&app_root);
    }
}
