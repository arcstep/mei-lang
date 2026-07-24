use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use moka::sync::Cache;
use serde_json::Value;

use super::scope::{RequestDag, RuntimeMetricEvalScope};

#[derive(Debug, Clone)]
pub(super) enum CachedEvalValue {
    Scalar(Value),
}

const EVAL_NODE_CACHE_TTL_MS: u64 = 15_000;
const EVAL_NODE_CACHE_MAX_BYTES: u64 = 8 * 1024 * 1024;
const EVAL_NODE_MAX_VALUE_BYTES: usize = 256 * 1024;

fn eval_node_cache() -> &'static Cache<String, Value> {
    static CACHE: OnceLock<Cache<String, Value>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(EVAL_NODE_CACHE_MAX_BYTES)
            .weigher(|_key: &String, value: &Value| {
                serde_json::to_vec(value)
                    .map(|bytes| bytes.len().clamp(1, u32::MAX as usize) as u32)
                    .unwrap_or(128)
            })
            .time_to_live(Duration::from_millis(EVAL_NODE_CACHE_TTL_MS))
            .build()
    })
}

pub(crate) fn eval_node_cache_enabled() -> bool {
    if let Ok(raw) = std::env::var("MEI_ENABLE_EVAL_NODE_CACHE") {
        let normalized = raw.trim();
        if normalized.is_empty() {
            return true;
        }
        return matches!(
            normalized.to_ascii_lowercase().as_str(),
            "1" | "true" | "on" | "yes" | "eval_node_cache"
        );
    }
    true
}

pub fn runtime_eval_node_cache_enabled() -> bool {
    eval_node_cache_enabled()
}

pub(crate) fn take_cached_eval_node(key: &str) -> Option<CachedEvalValue> {
    eval_node_cache().get(key).map(CachedEvalValue::Scalar)
}

pub(crate) fn store_cached_eval_node(key: &str, value: CachedEvalValue) {
    let CachedEvalValue::Scalar(value) = value;
    let bytes = serde_json::to_vec(&value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX);
    if bytes > EVAL_NODE_MAX_VALUE_BYTES {
        return;
    }
    eval_node_cache().insert(key.to_string(), value);
}

pub(crate) fn clear_eval_node_cache() -> usize {
    let cache = eval_node_cache();
    let removed = cache.entry_count() as usize;
    cache.invalidate_all();
    cache.run_pending_tasks();
    removed
}

/// Max nested `with_eval_node` frames (symmetric with `pipeline_sql` `depth > 32`).
pub(crate) const MAX_METRIC_EVAL_DEPTH: usize = 32;

#[derive(Debug)]
pub(crate) struct EvalContext {
    pub(crate) scope: RuntimeMetricEvalScope,
    /// Runtime metric defs available for `{"__ref":"metric"}` rowset resolution.
    pub(crate) metric_defs: BTreeMap<String, Value>,
    /// Rowsets materialized by metric id during this request pass.
    pub(crate) resolved_metric_rowsets: BTreeMap<String, Arc<Vec<Value>>>,
    pub(crate) rowset_cache: BTreeMap<String, Arc<Vec<Value>>>,
    pub(crate) scalar_cache: BTreeMap<String, Value>,
    // This records the request-scoped execution DAG. It must not be confused
    // with `AnalysisGraph`, which is semantic and compile-derived.
    pub(crate) request_dag: RequestDag,
    pub(crate) request_cache_hits: u64,
    pub(crate) eval_node_cache_hits: u64,
    pub(crate) eval_node_cache_misses: u64,
    pub(crate) eval_stack: Vec<String>,
    pub(crate) in_progress: BTreeSet<String>,
    /// Caps `eval_stack` growth; overridable in tests (default [`MAX_METRIC_EVAL_DEPTH`]).
    pub(crate) max_eval_depth: usize,
}

impl Default for EvalContext {
    fn default() -> Self {
        Self {
            scope: RuntimeMetricEvalScope::default(),
            metric_defs: BTreeMap::new(),
            resolved_metric_rowsets: BTreeMap::new(),
            rowset_cache: BTreeMap::new(),
            scalar_cache: BTreeMap::new(),
            request_dag: RequestDag::default(),
            request_cache_hits: 0,
            eval_node_cache_hits: 0,
            eval_node_cache_misses: 0,
            eval_stack: Vec::new(),
            in_progress: BTreeSet::new(),
            max_eval_depth: MAX_METRIC_EVAL_DEPTH,
        }
    }
}

fn scope_cache_key(scope: &RuntimeMetricEvalScope) -> String {
    format!(
        "base={}|scene={}|target={}|search={}|filters={}|filter_intents={}|dimension_bindings={}|group={}|time_range={}|deps={}",
        scope.base_dataset_id,
        scope.scene_id,
        scope.target,
        scope.search,
        scope.filters_fingerprint,
        serde_json::to_string(&scope.filter_intents).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&scope.dimension_bindings).unwrap_or_else(|_| "[]".to_string()),
        scope.query_state.group_identity_key(),
        scope.query_state.time_range_identity_key(),
        scope.dependency_revision_key
    )
}

fn canonicalize_expr_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(canonicalize_expr_value)
                .collect::<Vec<_>>(),
        ),
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            for key in map.keys().cloned().collect::<BTreeSet<_>>() {
                if let Some(item) = map.get(&key) {
                    sorted.insert(key, canonicalize_expr_value(item));
                }
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

pub(crate) fn expr_cache_key(
    prefix: &str,
    scope: &RuntimeMetricEvalScope,
    expr: &Value,
) -> Option<String> {
    let serialized = serde_json::to_string(&canonicalize_expr_value(expr)).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serialized.hash(&mut hasher);
    let expr_hash = format!("{:016x}", hasher.finish());
    let identity_hint = expr_identity_hint(expr).unwrap_or_else(|| prefix.to_string());
    Some(format!(
        "{prefix}|{}|hint={identity_hint}|expr_hash={expr_hash}",
        scope_cache_key(scope)
    ))
}

fn expr_identity_hint(expr: &Value) -> Option<String> {
    let map = expr.as_object()?;
    for key in [
        "analysis_node_id",
        "analysis_parent_metric_id",
        "key",
        "id",
        "dataset",
        "dataset_id",
        "type",
    ] {
        let value = map
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        return Some(value.to_string());
    }
    None
}
