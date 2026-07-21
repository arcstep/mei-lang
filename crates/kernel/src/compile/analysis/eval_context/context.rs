use super::cache::{
    eval_node_cache_enabled, expr_cache_key, store_cached_eval_node, take_cached_eval_node,
    CachedEvalValue, EvalContext,
};
use super::scope::{EvalNodeKind, RequestDag, RequestDagMetrics, RuntimeMetricEvalScope};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::Value;

impl EvalContext {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_scope(scope: RuntimeMetricEvalScope) -> Self {
        Self::with_scope_and_metric_defs(scope, BTreeMap::new())
    }

    pub(crate) fn with_scope_and_metric_defs(
        scope: RuntimeMetricEvalScope,
        metric_defs: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            scope,
            metric_defs,
            resolved_metric_rowsets: BTreeMap::new(),
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

    pub(crate) fn store_resolved_metric_rowset(&mut self, metric_id: &str, rows: &[Value]) {
        if !metric_id.trim().is_empty() {
            self.resolved_metric_rowsets
                .insert(metric_id.to_string(), Arc::new(rows.to_vec()));
        }
    }

    pub(crate) fn resolved_metric_rowset(&self, metric_id: &str) -> Option<Arc<Vec<Value>>> {
        self.resolved_metric_rowsets.get(metric_id).cloned()
    }

    pub(crate) fn metric_def(&self, metric_id: &str) -> Option<&Value> {
        self.metric_defs.get(metric_id)
    }

    fn register_node_access(&mut self, key: &str, hit: bool) {
        if let Some(parent) = self.eval_stack.last() {
            if parent != key {
                self.request_dag
                    .edges
                    .insert((parent.clone(), key.to_string()));
            }
        }
        let stats = self.request_dag.nodes.entry(key.to_string()).or_default();
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

    pub(crate) fn rowset(&self, expr: &Value) -> Option<Arc<Vec<Value>>> {
        let key = expr_cache_key("rowset", &self.scope, expr)?;
        self.rowset_cache.get(&key).cloned()
    }

    pub(crate) fn store_rowset(&mut self, expr: &Value, rows: &[Value]) {
        if let Some(key) = expr_cache_key("rowset", &self.scope, expr) {
            self.rowset_cache.insert(key, Arc::new(rows.to_vec()));
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

    pub(crate) fn cached_rowset(&mut self, expr: &Value) -> Option<Arc<Vec<Value>>> {
        let key = self.rowset_key(expr)?;
        if let Some(rows) = self.rowset(expr) {
            self.request_cache_hits += 1;
            self.register_node_access(&key, true);
            return Some(rows);
        }
        // Rowsets are request working sets and are never retained globally.
        self.eval_node_cache_misses += u64::from(eval_node_cache_enabled());
        None
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
