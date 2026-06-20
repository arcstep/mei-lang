use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_kernel::{build_runtime_eval_plan, DatasetView, EvalPlan, RuntimeMetricEvalScope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{eval_node_cache_key, metric_scope_cache_key, RuntimeMetricWorkset};

const EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-workset-artifact-v1";
const EVAL_PLAN_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-plan-artifact-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedWorksetArtifact {
    schema_version: String,
    owner_resource_id: String,
    requested_metric_ids: Vec<String>,
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
    semantic_revision_key: String,
    scope_key: String,
    dependency_revision_key: String,
    eval_plan: EvalPlan,
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
    app_root.join(".mei").join("eval-artifacts")
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

fn eval_plan_semantic_revision_key(
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

fn workset_artifact_path(app_root: &Path, owner_resource_id: &str, requested_metric_ids: &[String]) -> PathBuf {
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

fn read_json_artifact<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let artifact = serde_json::from_str::<T>(
        &fs::read_to_string(path).with_context(|| format!("read eval artifact {}", path.display()))?,
    )
    .with_context(|| format!("parse eval artifact {}", path.display()))?;
    Ok(Some(artifact))
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
    if let Some(artifact) = read_json_artifact::<PersistedWorksetArtifact>(&path)? {
        if artifact.schema_version == EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION
            && artifact.semantic_revision_key == semantic_revision_key
        {
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
    let inner =
        crate::metric_cache_key::runtime_metric_workset(owner_resource_id, requested_metric_ids, dataset);
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
    if let Some(artifact) = read_json_artifact::<PersistedEvalPlanArtifact>(&path)? {
        if artifact.schema_version == EVAL_PLAN_ARTIFACT_SCHEMA_VERSION
            && artifact.dependency_revision_key == scope.dependency_revision_key
            && artifact.semantic_revision_key == semantic_revision_key
        {
            return Ok((artifact.eval_plan, started.elapsed().as_millis() as u64, true));
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

pub(crate) fn clear_eval_artifact_store(app_root: &Path) -> usize {
    let root = eval_artifact_root(app_root);
    if !root.exists() {
        return 0;
    }
    let count = count_files_recursively(&root);
    let _ = fs::remove_dir_all(&root);
    count
}

pub(crate) fn eval_artifact_hydrate_dataset_ids(metric_defs: &BTreeMap<String, Value>) -> BTreeSet<String> {
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
