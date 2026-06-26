use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
