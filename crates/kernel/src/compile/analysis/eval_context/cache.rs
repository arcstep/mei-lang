use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;


use super::scope::{RequestDag, RuntimeMetricEvalScope};

#[derive(Debug, Clone)]
pub(super) enum CachedEvalValue {
    Rowset(Vec<Value>),
    Scalar(Value),
}

#[derive(Debug, Clone)]
struct CachedEvalNode {
    expires_at: Instant,
    value: CachedEvalValue,
}

const EVAL_NODE_CACHE_TTL_MS: u64 = 15_000;
const MAX_EVAL_NODE_CACHE_ENTRIES: usize = 1024;

fn eval_node_cache() -> &'static Mutex<BTreeMap<String, CachedEvalNode>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedEvalNode>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn eval_node_cache_ttl() -> Duration {
    Duration::from_millis(EVAL_NODE_CACHE_TTL_MS)
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
    let Ok(mut cache) = eval_node_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.value.clone())
}

pub(crate) fn store_cached_eval_node(key: &str, value: CachedEvalValue) {
    let Ok(mut cache) = eval_node_cache().lock() else {
        return;
    };
    cache.retain(|_, entry| entry.expires_at > Instant::now());
    if cache.len() >= MAX_EVAL_NODE_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(
        key.to_string(),
        CachedEvalNode {
            expires_at: Instant::now() + eval_node_cache_ttl(),
            value,
        },
    );
}

pub(crate) fn clear_eval_node_cache() -> usize {
    let Ok(mut cache) = eval_node_cache().lock() else {
        return 0;
    };
    let removed = cache.len();
    cache.clear();
    removed
}

#[derive(Debug, Default)]
pub(crate) struct EvalContext {
    pub(crate) scope: RuntimeMetricEvalScope,
    /// Runtime metric defs available for `{"__ref":"metric"}` rowset resolution.
    pub(crate) metric_defs: BTreeMap<String, Value>,
    /// Rowsets materialized by metric id during this request pass.
    pub(crate) resolved_metric_rowsets: BTreeMap<String, Vec<Value>>,
    pub(crate) rowset_cache: BTreeMap<String, Vec<Value>>,
    pub(crate) scalar_cache: BTreeMap<String, Value>,
    // This records the request-scoped execution DAG. It must not be confused
    // with `AnalysisGraph`, which is semantic and compile-derived.
    pub(crate) request_dag: RequestDag,
    pub(crate) request_cache_hits: u64,
    pub(crate) eval_node_cache_hits: u64,
    pub(crate) eval_node_cache_misses: u64,
    pub(crate) eval_stack: Vec<String>,
    pub(crate) in_progress: BTreeSet<String>,
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

pub(crate) fn expr_cache_key(prefix: &str, scope: &RuntimeMetricEvalScope, expr: &Value) -> Option<String> {
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
