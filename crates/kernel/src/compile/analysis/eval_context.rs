use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{DimensionBinding, FilterIntent, QueryState};

#[derive(Debug, Clone, Default)]
pub struct RuntimeMetricEvalScope {
    /// Base dataset whose current filtered rows act as the root rowset for this
    /// evaluation pass.
    pub base_dataset_id: String,
    /// Scene locator dimensions belong to evaluation identity, not semantic
    /// graph identity.
    pub scene_id: String,
    pub target: String,
    pub search: String,
    /// Shared runtime query state carried into evaluation. This is a host/runtime
    /// context object, not semantic DAG state.
    pub query_state: QueryState,
    /// Filter intents normalized from query state or interaction inputs.
    pub filter_intents: Vec<FilterIntent>,
    /// Resolved bindings from semantic filter dimensions to concrete dataset
    /// fields for this evaluation pass.
    pub dimension_bindings: Vec<DimensionBinding>,
    /// Normalized filter identity for cache keys and request-scoped memoization.
    /// This is evaluation context, not semantic DAG state.
    pub filters_fingerprint: String,
    pub dependency_revision_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvalNodeKind {
    Rowset,
    Scalar,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EvalNodeStats {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RequestDag {
    /// Evaluation nodes visited in a single request scope.
    pub nodes: BTreeMap<String, EvalNodeStats>,
    /// Request-time execution dependencies between evaluation nodes.
    pub edges: BTreeSet<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestDagMetrics {
    pub nodes: usize,
    pub edges: usize,
    pub hits: u64,
    pub misses: u64,
    pub request_cache_hits: u64,
    pub eval_node_cache_hits: u64,
    pub eval_node_cache_misses: u64,
}

#[derive(Debug, Clone)]
enum CachedEvalValue {
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

fn eval_node_cache_enabled() -> bool {
    let raw = std::env::var("MEI_ENABLE_EVAL_NODE_CACHE")
        .ok()
        .or_else(|| std::env::var("MEI_PERF_ENABLE").ok())
        .unwrap_or_default();
    raw.split(',')
        .map(str::trim)
        .any(|flag| flag.eq_ignore_ascii_case("1") || flag.eq_ignore_ascii_case("eval_node_cache"))
}

pub fn runtime_eval_node_cache_enabled() -> bool {
    eval_node_cache_enabled()
}

fn take_cached_eval_node(key: &str) -> Option<CachedEvalValue> {
    let Ok(mut cache) = eval_node_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.value.clone())
}

fn store_cached_eval_node(key: &str, value: CachedEvalValue) {
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
    scope: RuntimeMetricEvalScope,
    rowset_cache: BTreeMap<String, Vec<Value>>,
    scalar_cache: BTreeMap<String, Value>,
    // This records the request-scoped execution DAG. It must not be confused
    // with `AnalysisGraph`, which is semantic and compile-derived.
    request_dag: RequestDag,
    request_cache_hits: u64,
    eval_node_cache_hits: u64,
    eval_node_cache_misses: u64,
    eval_stack: Vec<String>,
    in_progress: BTreeSet<String>,
}

fn scope_cache_key(scope: &RuntimeMetricEvalScope) -> String {
    format!(
        "base={}|scene={}|target={}|search={}|filters={}|deps={}",
        scope.base_dataset_id,
        scope.scene_id,
        scope.target,
        scope.search,
        scope.filters_fingerprint,
        scope.dependency_revision_key
    )
}

fn canonicalize_expr_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            Value::Array(items.iter().map(canonicalize_expr_value).collect::<Vec<_>>())
        }
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

fn expr_cache_key(prefix: &str, scope: &RuntimeMetricEvalScope, expr: &Value) -> Option<String> {
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

impl EvalContext {
    pub(crate) fn with_scope(scope: RuntimeMetricEvalScope) -> Self {
        Self {
            scope,
            rowset_cache: BTreeMap::new(),
            scalar_cache: BTreeMap::new(),
            request_dag: RequestDag::default(),
            request_cache_hits: 0,
            eval_node_cache_hits: 0,
            eval_node_cache_misses: 0,
            eval_stack: Vec::new(),
            in_progress: BTreeSet::new(),
        }
    }

    fn register_node_access(&mut self, key: &str, hit: bool) {
        if let Some(parent) = self.eval_stack.last() {
            if parent != key {
                self.request_dag
                    .edges
                    .insert((parent.clone(), key.to_string()));
            }
        }
        let stats = self
            .request_dag
            .nodes
            .entry(key.to_string())
            .or_default();
        if hit {
            stats.hits += 1;
        } else {
            stats.misses += 1;
        }
    }

    pub(crate) fn rowset_key(&self, expr: &Value) -> Option<String> {
        expr_cache_key("rowset", &self.scope, expr)
    }

    pub(crate) fn scalar_key(&self, expr: &Value) -> Option<String> {
        expr_cache_key("scalar", &self.scope, expr)
    }

    pub(crate) fn begin_eval_node(&mut self, key: &str) -> Result<()> {
        self.register_node_access(key, false);
        if self.in_progress.contains(key) {
            return Err(anyhow!(
                "cyclic_eval_dependency: node `{key}` re-entered while in progress"
            ));
        }
        self.in_progress.insert(key.to_string());
        self.eval_stack.push(key.to_string());
        Ok(())
    }

    pub(crate) fn finish_eval_node(&mut self, key: &str) {
        self.in_progress.remove(key);
        if self.eval_stack.last().is_some_and(|value| value == key) {
            self.eval_stack.pop();
            return;
        }
        if let Some(pos) = self.eval_stack.iter().rposition(|value| value == key) {
            self.eval_stack.remove(pos);
        }
    }

    pub(crate) fn with_eval_node<T>(
        &mut self,
        key: &str,
        _kind: EvalNodeKind,
        compute: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.begin_eval_node(key)?;
        let outcome = compute(self);
        self.finish_eval_node(key);
        outcome
    }

    pub(crate) fn request_dag_metrics(&self) -> RequestDagMetrics {
        let (hits, misses) = self
            .request_dag
            .nodes
            .values()
            .fold((0u64, 0u64), |(acc_hits, acc_misses), stats| {
                (acc_hits + stats.hits, acc_misses + stats.misses)
            });
        RequestDagMetrics {
            nodes: self.request_dag.nodes.len(),
            edges: self.request_dag.edges.len(),
            hits,
            misses,
            request_cache_hits: self.request_cache_hits,
            eval_node_cache_hits: self.eval_node_cache_hits,
            eval_node_cache_misses: self.eval_node_cache_misses,
        }
    }

    pub(crate) fn rowset(&self, expr: &Value) -> Option<Vec<Value>> {
        let key = expr_cache_key("rowset", &self.scope, expr)?;
        self.rowset_cache.get(&key).cloned()
    }

    pub(crate) fn store_rowset(&mut self, expr: &Value, rows: &[Value]) {
        if let Some(key) = expr_cache_key("rowset", &self.scope, expr) {
            self.rowset_cache.insert(key.clone(), rows.to_vec());
            if eval_node_cache_enabled() {
                store_cached_eval_node(&key, CachedEvalValue::Rowset(rows.to_vec()));
            }
        }
    }

    pub(crate) fn scalar(&self, expr: &Value) -> Option<Value> {
        let key = expr_cache_key("scalar", &self.scope, expr)?;
        self.scalar_cache.get(&key).cloned()
    }

    pub(crate) fn store_scalar(&mut self, expr: &Value, value: &Value) {
        if let Some(key) = expr_cache_key("scalar", &self.scope, expr) {
            self.scalar_cache.insert(key.clone(), value.clone());
            if eval_node_cache_enabled() {
                store_cached_eval_node(&key, CachedEvalValue::Scalar(value.clone()));
            }
        }
    }

    pub(crate) fn cached_rowset(&mut self, expr: &Value) -> Option<Vec<Value>> {
        let key = self.rowset_key(expr)?;
        if let Some(rows) = self.rowset(expr) {
            self.request_cache_hits += 1;
            self.register_node_access(&key, true);
            return Some(rows);
        }
        if !eval_node_cache_enabled() {
            return None;
        }
        let Some(CachedEvalValue::Rowset(rows)) = take_cached_eval_node(&key) else {
            self.eval_node_cache_misses += 1;
            return None;
        };
        self.rowset_cache.insert(key.clone(), rows.clone());
        self.eval_node_cache_hits += 1;
        self.register_node_access(&key, true);
        Some(rows)
    }

    pub(crate) fn cached_scalar(&mut self, expr: &Value) -> Option<Value> {
        let key = self.scalar_key(expr)?;
        if let Some(value) = self.scalar(expr) {
            self.request_cache_hits += 1;
            self.register_node_access(&key, true);
            return Some(value);
        }
        if !eval_node_cache_enabled() {
            return None;
        }
        let Some(CachedEvalValue::Scalar(value)) = take_cached_eval_node(&key) else {
            self.eval_node_cache_misses += 1;
            return None;
        };
        self.scalar_cache.insert(key.clone(), value.clone());
        self.eval_node_cache_hits += 1;
        self.register_node_access(&key, true);
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{EvalContext, RuntimeMetricEvalScope};
    use crate::model::QueryState;
    use serde_json::json;

    #[test]
    fn eval_context_keys_include_scope_fingerprint() {
        let expr = json!({"__kind":"analysis_expr","type":"count"});
        let mut left = EvalContext::with_scope(RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: String::new(),
            query_state: QueryState::default(),
            filter_intents: Vec::new(),
            dimension_bindings: Vec::new(),
            filters_fingerprint: "{\"status\":\"待办\"}".to_string(),
            dependency_revision_key: "deps=a".to_string(),
        });
        let mut right = EvalContext::with_scope(RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: String::new(),
            query_state: QueryState::default(),
            filter_intents: Vec::new(),
            dimension_bindings: Vec::new(),
            filters_fingerprint: "{\"status\":\"已办\"}".to_string(),
            dependency_revision_key: "deps=a".to_string(),
        });
        left.store_scalar(&expr, &json!(3));
        right.store_scalar(&expr, &json!(4));
        assert_eq!(left.scalar(&expr), Some(json!(3)));
        assert_eq!(right.scalar(&expr), Some(json!(4)));
    }

    #[test]
    fn eval_context_cycle_guard_rejects_reentry() {
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: String::new(),
            query_state: QueryState::default(),
            filter_intents: Vec::new(),
            dimension_bindings: Vec::new(),
            filters_fingerprint: "{}".to_string(),
            dependency_revision_key: "deps=a".to_string(),
        };
        let mut ctx = EvalContext::with_scope(scope);
        let expr = json!({"__kind":"analysis_expr","type":"count"});
        let key = ctx.rowset_key(&expr).expect("rowset key");
        ctx.begin_eval_node(&key)
            .expect("first begin should pass");
        let err = ctx
            .begin_eval_node(&key)
            .expect_err("reentry should fail");
        assert!(err.to_string().contains("cyclic_eval_dependency"));
    }

    #[test]
    fn eval_context_canonicalizes_expr_key_order() {
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: String::new(),
            query_state: QueryState::default(),
            filter_intents: Vec::new(),
            dimension_bindings: Vec::new(),
            filters_fingerprint: "{}".to_string(),
            dependency_revision_key: "deps=a".to_string(),
        };
        let mut ctx = EvalContext::with_scope(scope);
        let left = json!({"__kind":"analysis_expr","type":"count","rowset":{"b":2,"a":1}});
        let right = json!({"type":"count","__kind":"analysis_expr","rowset":{"a":1,"b":2}});
        ctx.store_scalar(&left, &json!(9));
        assert_eq!(ctx.cached_scalar(&right), Some(json!(9)));
    }

    #[test]
    fn request_dag_metrics_track_nested_eval_edges_and_request_hits() {
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: String::new(),
            query_state: QueryState::default(),
            filter_intents: Vec::new(),
            dimension_bindings: Vec::new(),
            filters_fingerprint: "{\"status\":\"待办\"}".to_string(),
            dependency_revision_key: "deps=a".to_string(),
        };
        let mut ctx = EvalContext::with_scope(scope);
        let parent = json!({"__kind":"analysis_expr","type":"count","name":"parent"});
        let child = json!({"__kind":"analysis_expr","type":"where","name":"child"});
        ctx.store_rowset(&child, &[json!({"id": 1})]);
        ctx.with_eval_node(
            &ctx.scalar_key(&parent).expect("parent scalar key"),
            super::EvalNodeKind::Scalar,
            |ctx| {
                assert_eq!(ctx.cached_rowset(&child).unwrap_or_default().len(), 1);
                Ok(json!(1))
            },
        )
        .expect("nested eval should succeed");
        let metrics = ctx.request_dag_metrics();
        assert_eq!(metrics.nodes, 2, "parent scalar node and child rowset node");
        assert_eq!(metrics.edges, 1, "parent should depend on child");
        assert_eq!(metrics.request_cache_hits, 1, "child should hit request cache");
    }
}
