use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use mei_lang_kernel::{
    build_runtime_eval_plan, resolve_app_eval_cache_root, DatasetView, EvalPlan, MetricContract,
    RuntimeMetricEvalScope,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::eval_cache_io_stats::{
    record_artifact_write, record_node_pack_load, record_node_pack_store,
    record_node_pack_store_skipped_full_hit,
};
use crate::l1_project::{metric_contract_eligible_for_node_pack, L1PinPolicy};
use crate::{eval_node_cache_key, metric_scope_cache_key, RuntimeMetricWorkset};

const EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-workset-artifact-v2";
const EVAL_PLAN_ARTIFACT_SCHEMA_VERSION: &str = "mei-eval-plan-artifact-v2";
const EVAL_METRIC_NODE_PACK_SCHEMA_VERSION: &str = "mei-eval-metric-node-pack-v1";
const WORKSET_KIND: &str = "eval-workset";
const PLAN_KIND: &str = "eval-plan";
const NODE_PACK_KIND: &str = "eval-node-pack";

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
struct PersistedEvalMetricNodeEntry {
    metric_id: String,
    metric: MetricContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEvalMetricNodePack {
    schema_version: String,
    owner_resource_id: String,
    semantic_revision_key: String,
    scope_key: String,
    dependency_revision_key: String,
    /// node_id → metric entry
    nodes: BTreeMap<String, PersistedEvalMetricNodeEntry>,
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

fn eval_metric_node_pack_key(owner_resource_id: &str, scope: &RuntimeMetricEvalScope) -> String {
    format!(
        "metric_node_pack|owner={}|scope={}",
        owner_resource_id.trim(),
        eval_node_cache_key("metric_node", scope)
    )
}

fn workset_artifact_key(owner_resource_id: &str, requested_metric_ids: &[String]) -> String {
    format!(
        "workset|owner={}|metrics={}",
        owner_resource_id.trim(),
        metric_scope_cache_key(requested_metric_ids)
    )
}

fn eval_plan_artifact_key(
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    scope: &RuntimeMetricEvalScope,
) -> String {
    format!(
        "plan|owner={}|metrics={}|scope={}",
        owner_resource_id.trim(),
        metric_scope_cache_key(requested_metric_ids),
        eval_node_cache_key("eval_plan", scope)
    )
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

pub(crate) fn load_or_build_runtime_metric_workset_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    requested_metric_ids: &[String],
    dataset: &DatasetView,
) -> Result<(RuntimeMetricWorkset, u64, bool)> {
    let started = Instant::now();
    let key = workset_artifact_key(owner_resource_id, requested_metric_ids);
    let semantic_revision_key = dataset_semantic_revision_key(owner_resource_id, dataset);
    if let Some(artifact) = crate::load_small_artifact::<PersistedWorksetArtifact>(
        app_root,
        WORKSET_KIND,
        key.as_str(),
    )? {
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
    let bytes = crate::store_small_artifact(
        app_root,
        WORKSET_KIND,
        key.as_str(),
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
    record_artifact_write(bytes as u64);
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
    let key = eval_plan_artifact_key(owner_resource_id, requested_metric_ids, scope);
    let semantic_revision_key =
        eval_plan_semantic_revision_key(owner_resource_id, requested_metric_ids, metric_defs);
    if let Some(artifact) =
        crate::load_small_artifact::<PersistedEvalPlanArtifact>(app_root, PLAN_KIND, key.as_str())?
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
    let bytes = crate::store_small_artifact(
        app_root,
        PLAN_KIND,
        key.as_str(),
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
    record_artifact_write(bytes as u64);
    Ok((eval_plan, started.elapsed().as_millis() as u64, false))
}

pub(crate) fn load_eval_metric_node_pack(
    app_root: &Path,
    owner_resource_id: &str,
    semantic_revision_key: &str,
    scope: &RuntimeMetricEvalScope,
) -> Result<(BTreeMap<String, MetricContract>, u64, usize)> {
    let started = Instant::now();
    let key = eval_metric_node_pack_key(owner_resource_id, scope);
    let Some(artifact) = crate::load_small_artifact::<PersistedEvalMetricNodePack>(
        app_root,
        NODE_PACK_KIND,
        key.as_str(),
    )?
    else {
        return Ok((BTreeMap::new(), 0, 0));
    };
    if artifact.schema_version != EVAL_METRIC_NODE_PACK_SCHEMA_VERSION
        || artifact.owner_resource_id != owner_resource_id
        || artifact.semantic_revision_key != semantic_revision_key
        || artifact.scope_key != eval_node_cache_key("metric_node", scope)
        || artifact.dependency_revision_key != scope.dependency_revision_key
    {
        return Ok((BTreeMap::new(), 0, 0));
    }
    record_node_pack_load();
    let policy = L1PinPolicy::default();
    let mut cached = BTreeMap::new();
    for entry in artifact.nodes.values() {
        if !metric_contract_eligible_for_node_pack(entry.metric_id.as_str(), &entry.metric, &policy)
        {
            continue;
        }
        cached.insert(entry.metric_id.clone(), entry.metric.clone());
    }
    let hit_count = cached.len();
    Ok((cached, started.elapsed().as_millis() as u64, hit_count))
}

pub(crate) fn store_eval_metric_node_pack(
    app_root: &Path,
    owner_resource_id: &str,
    semantic_revision_key: &str,
    scope: &RuntimeMetricEvalScope,
    node_metrics: &BTreeMap<String, (String, MetricContract)>,
    full_hit: bool,
) -> Result<u64> {
    if full_hit {
        record_node_pack_store_skipped_full_hit();
        return Ok(0);
    }
    let policy = L1PinPolicy::default();
    let key = eval_metric_node_pack_key(owner_resource_id, scope);
    let mut nodes = BTreeMap::new();
    let mut stripped_ineligible = false;
    if let Some(existing) = crate::load_small_artifact::<PersistedEvalMetricNodePack>(
        app_root,
        NODE_PACK_KIND,
        key.as_str(),
    )? {
        if existing.schema_version == EVAL_METRIC_NODE_PACK_SCHEMA_VERSION
            && existing.owner_resource_id == owner_resource_id
            && existing.semantic_revision_key == semantic_revision_key
            && existing.scope_key == eval_node_cache_key("metric_node", scope)
            && existing.dependency_revision_key == scope.dependency_revision_key
        {
            for (node_id, entry) in existing.nodes {
                if metric_contract_eligible_for_node_pack(
                    entry.metric_id.as_str(),
                    &entry.metric,
                    &policy,
                ) {
                    nodes.insert(node_id, entry);
                } else {
                    stripped_ineligible = true;
                }
            }
        }
    }
    let mut inserted = 0usize;
    for (node_id, (metric_id, metric)) in node_metrics {
        if !metric_contract_eligible_for_node_pack(metric_id.as_str(), metric, &policy) {
            continue;
        }
        nodes.insert(
            node_id.clone(),
            PersistedEvalMetricNodeEntry {
                metric_id: metric_id.clone(),
                metric: metric.clone(),
            },
        );
        inserted += 1;
    }
    if inserted == 0 && !stripped_ineligible {
        return Ok(0);
    }
    let bytes = crate::store_small_artifact(
        app_root,
        NODE_PACK_KIND,
        key.as_str(),
        &PersistedEvalMetricNodePack {
            schema_version: EVAL_METRIC_NODE_PACK_SCHEMA_VERSION.to_string(),
            owner_resource_id: owner_resource_id.to_string(),
            semantic_revision_key: semantic_revision_key.to_string(),
            scope_key: eval_node_cache_key("metric_node", scope),
            dependency_revision_key: scope.dependency_revision_key.clone(),
            nodes,
            generated_at_ms: now_epoch_ms(),
        },
    )?;
    if bytes == 0 {
        return Ok(0);
    }
    record_artifact_write(bytes as u64);
    record_node_pack_store();
    Ok(1)
}

#[allow(dead_code)]
pub(crate) fn load_eval_metric_node_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    _node_id: &str,
    metric_id: &str,
    semantic_revision_key: &str,
    scope: &RuntimeMetricEvalScope,
) -> Result<Option<(MetricContract, u64)>> {
    let (pack, load_ms, _) =
        load_eval_metric_node_pack(app_root, owner_resource_id, semantic_revision_key, scope)?;
    Ok(pack.get(metric_id).cloned().map(|metric| (metric, load_ms)))
}

#[allow(dead_code)]
pub(crate) fn store_eval_metric_node_artifact(
    app_root: &Path,
    owner_resource_id: &str,
    node_id: &str,
    metric_id: &str,
    semantic_revision_key: &str,
    scope: &RuntimeMetricEvalScope,
    metric: &MetricContract,
) -> Result<()> {
    let mut nodes = BTreeMap::new();
    nodes.insert(node_id.to_string(), (metric_id.to_string(), metric.clone()));
    let _ = store_eval_metric_node_pack(
        app_root,
        owner_resource_id,
        semantic_revision_key,
        scope,
        &nodes,
        false,
    )?;
    Ok(())
}

pub(crate) fn clear_eval_artifact_store(app_root: &Path) -> usize {
    let root = resolve_app_eval_cache_root(app_root);
    let removed = crate::clear_small_artifacts(app_root).unwrap_or(0);
    let mut legacy_files = 0;
    for legacy in [
        "workset",
        "plan",
        "node-pack",
        "metric-response-lite",
        "metric-dataframe",
    ] {
        let path = root.join(legacy);
        legacy_files += count_files_recursively(path.as_path());
        let _ = fs::remove_dir_all(path);
    }
    removed.saturating_add(legacy_files)
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
    use std::path::PathBuf;

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
                            primary_key: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: BTreeMap::new(),
        }
    }

    fn temp_app_root(name: &str) -> PathBuf {
        let app_root = std::env::temp_dir().join(format!(
            "mei-eval-artifact-{name}-{}-{}",
            std::process::id(),
            now_epoch_ms()
        ));
        let _ = fs::remove_dir_all(&app_root);
        let env_dir = app_root.join("env").join("WS-20260720.0");
        let _ = fs::create_dir_all(env_dir.join("build"));
        let _ = fs::create_dir_all(env_dir.join("var"));
        let current = app_root.join("env").join("current");
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink("WS-20260720.0", &current);
        }
        #[cfg(not(unix))]
        {
            let _ = fs::create_dir_all(&current);
        }
        app_root
    }

    #[test]
    fn stale_workset_artifact_is_rebuilt_without_error() {
        let app_root = temp_app_root("corrupt");
        let dataset = minimal_dataset();
        let owner = "owner::metrics";
        let metrics = vec!["metric-a".to_string()];

        let key = workset_artifact_key(owner, &metrics);
        crate::store_small_artifact(
            &app_root,
            WORKSET_KIND,
            key.as_str(),
            &PersistedWorksetArtifact {
                schema_version: "stale".to_string(),
                owner_resource_id: owner.to_string(),
                requested_metric_ids: metrics.clone(),
                semantic_revision_key: "stale".to_string(),
                closure_metric_ids: Vec::new(),
                eval_metric_ids: None,
                defs_for_hydrate: BTreeMap::new(),
                generated_at_ms: 1,
            },
        )
        .expect("store stale");

        let (_, _, hit) =
            load_or_build_runtime_metric_workset_artifact(&app_root, owner, &metrics, &dataset)
                .expect("stale artifact must not fail request");
        assert!(!hit);
        let rebuilt: PersistedWorksetArtifact =
            crate::load_small_artifact(&app_root, WORKSET_KIND, key.as_str())
                .expect("load rebuilt")
                .expect("present");
        assert_eq!(rebuilt.schema_version, EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION);
        assert!(!rebuilt.semantic_revision_key.is_empty());

        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn node_pack_store_and_load_skip_scalar_rowset() {
        use mei_lang_kernel::{MetricShape, QueryState};

        let app_root = temp_app_root("node-pack-rowset");
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "owner::metrics".to_string(),
            query_state: QueryState::default(),
            ..RuntimeMetricEvalScope::default()
        };
        let owner = "owner::metrics";
        let semantic = "sem-test";
        let mut to_store = BTreeMap::new();
        to_store.insert(
            "n-kpi".to_string(),
            (
                "kpi_count".to_string(),
                MetricContract {
                    id: "kpi_count".to_string(),
                    label: None,
                    unit: None,
                    purpose: None,
                    shape: MetricShape::Scalar,
                    schema: Vec::new(),
                    value: serde_json::json!({"value": 3}),
                    value_format: None,
                    dataset: None,
                    transforms: Vec::new(),
                },
            ),
        );
        to_store.insert(
            "n-rowset".to_string(),
            (
                "kpi_count::__scalar_rowset__".to_string(),
                MetricContract {
                    id: "kpi_count::__scalar_rowset__".to_string(),
                    label: None,
                    unit: None,
                    purpose: None,
                    shape: MetricShape::Dataframe,
                    schema: Vec::new(),
                    value: serde_json::json!([{"a": 1}, {"a": 2}, {"a": 3}]),
                    value_format: None,
                    dataset: None,
                    transforms: Vec::new(),
                },
            ),
        );
        let wrote =
            store_eval_metric_node_pack(&app_root, owner, semantic, &scope, &to_store, false)
                .expect("store");
        assert_eq!(wrote, 1);

        let key = eval_metric_node_pack_key(owner, &scope);
        let persisted: PersistedEvalMetricNodePack =
            crate::load_small_artifact(&app_root, NODE_PACK_KIND, key.as_str())
                .expect("read pack")
                .expect("pack");
        assert_eq!(persisted.nodes.len(), 1);
        assert!(persisted
            .nodes
            .values()
            .all(|entry| !entry.metric_id.contains("__scalar_rowset__")));

        let (loaded, _, hits) =
            load_eval_metric_node_pack(&app_root, owner, semantic, &scope).expect("load");
        assert_eq!(hits, 1);
        assert!(loaded.contains_key("kpi_count"));
        assert!(!loaded.contains_key("kpi_count::__scalar_rowset__"));

        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn node_pack_load_filters_legacy_scalar_rowset_entries() {
        use mei_lang_kernel::{MetricShape, QueryState};

        let app_root = temp_app_root("node-pack-legacy");
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "owner::metrics".to_string(),
            query_state: QueryState::default(),
            ..RuntimeMetricEvalScope::default()
        };
        let owner = "owner::metrics";
        let semantic = "sem-legacy";
        let key = eval_metric_node_pack_key(owner, &scope);
        let pack = PersistedEvalMetricNodePack {
            schema_version: EVAL_METRIC_NODE_PACK_SCHEMA_VERSION.to_string(),
            owner_resource_id: owner.to_string(),
            semantic_revision_key: semantic.to_string(),
            scope_key: eval_node_cache_key("metric_node", &scope),
            dependency_revision_key: scope.dependency_revision_key.clone(),
            nodes: BTreeMap::from([(
                "n-rowset".to_string(),
                PersistedEvalMetricNodeEntry {
                    metric_id: "kpi_count::__scalar_rowset__".to_string(),
                    metric: MetricContract {
                        id: "kpi_count::__scalar_rowset__".to_string(),
                        label: None,
                        unit: None,
                        purpose: None,
                        shape: MetricShape::Dataframe,
                        schema: Vec::new(),
                        value: serde_json::json!([{"x": 1}]),
                        value_format: None,
                        dataset: None,
                        transforms: Vec::new(),
                    },
                },
            )]),
            generated_at_ms: 1,
        };
        crate::store_small_artifact(&app_root, NODE_PACK_KIND, key.as_str(), &pack)
            .expect("write legacy");

        let (loaded, _, hits) =
            load_eval_metric_node_pack(&app_root, owner, semantic, &scope).expect("load");
        assert_eq!(hits, 0);
        assert!(loaded.is_empty());

        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn legacy_v1_workset_artifact_is_rebuilt_without_error() {
        let app_root = temp_app_root("legacy-v1");
        let dataset = minimal_dataset();
        let owner = "owner::metrics";
        let metrics = vec!["metric-a".to_string()];

        let key = workset_artifact_key(owner, &metrics);
        crate::store_small_artifact(
            &app_root,
            WORKSET_KIND,
            key.as_str(),
            &serde_json::json!({
                "schema_version": "mei-eval-workset-artifact-v1",
                "owner_resource_id": owner,
                "requested_metric_ids": metrics,
                "closure_metric_ids": ["metric-a"],
                "eval_metric_ids": ["metric-a"],
                "defs_for_hydrate": {},
                "generated_at_ms": 1
            }),
        )
        .expect("store legacy artifact");

        let (_, _, hit) =
            load_or_build_runtime_metric_workset_artifact(&app_root, owner, &metrics, &dataset)
                .expect("legacy artifact must not fail request");
        assert!(!hit);

        let rebuilt: PersistedWorksetArtifact =
            crate::load_small_artifact(&app_root, WORKSET_KIND, key.as_str())
                .expect("load rebuilt")
                .expect("present");
        assert_eq!(rebuilt.schema_version, EVAL_WORKSET_ARTIFACT_SCHEMA_VERSION);
        assert!(!rebuilt.semantic_revision_key.is_empty());

        let _ = fs::remove_dir_all(&app_root);
    }
}
