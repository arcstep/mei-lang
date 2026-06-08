mod contracts;
mod expand;
mod explain;
mod graph_build;
mod util;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

use crate::model::AnalysisGraph;

use self::contracts::build_analysis_contracts_from_expanded;
use self::expand::expand_metric_def;
use self::graph_build::build_analysis_graph_from_expanded;

/// Expand authored/runtime metric defs into the runtime-authoritative metric
/// definition map.
///
/// This step lowers explain-scope local objects into scoped metric ids so that
/// runtime evaluation, cache identity, and semantic graph construction all
/// share the same canonical metric space.
pub(crate) fn expand_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let mut expanded = BTreeMap::new();
    for (metric_id, raw) in metric_defs {
        expand_metric_def(metric_id, raw, &mut expanded);
    }
    expanded
}

pub(crate) fn build_analysis_artifacts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> (
    BTreeMap<String, Value>,
    AnalysisGraph,
    BTreeMap<String, Value>,
) {
    let expanded = expand_runtime_metric_defs(metric_defs);
    let graph = build_analysis_graph_from_expanded(&expanded, root_dataset_id);
    let contracts = build_analysis_contracts_from_expanded(&expanded, &graph, root_dataset_id);
    (expanded, graph, contracts)
}

/// Build the compile-derived semantic analysis graph.
///
/// This graph is the current semantic DAG artifact. It should not be confused
/// with request-time evaluation dependencies recorded by `RequestDag`.
pub(crate) fn build_analysis_graph(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> AnalysisGraph {
    let expanded = expand_runtime_metric_defs(metric_defs);
    build_analysis_graph_from_expanded(&expanded, root_dataset_id)
}

/// Build consumer projection contracts from expanded runtime metric defs plus
/// the semantic analysis graph.
///
/// These contracts are for consumers such as drilldown/popup and are not a
/// semantic or runtime-evaluation source of truth.
pub(crate) fn build_analysis_contracts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> BTreeMap<String, Value> {
    let expanded = expand_runtime_metric_defs(metric_defs);
    let graph = build_analysis_graph_from_expanded(&expanded, root_dataset_id);
    build_analysis_contracts_from_expanded(&expanded, &graph, root_dataset_id)
}

/// Select the metric workset implied by semantic analysis closure.
///
/// This walks compile-derived semantic edges to discover reachable metric
/// nodes. It is intentionally narrower than the request eval DAG: it selects
/// metric defs to consider for evaluation but does not express execution order
/// or expression dependencies.
pub(crate) fn analysis_closure_metric_ids(
    graph: &AnalysisGraph,
    focus_ids: &[String],
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    for focus_id in focus_ids {
        let focus_id = focus_id.trim();
        if focus_id.is_empty() {
            continue;
        }
        if visited.insert(focus_id.to_string()) {
            queue.push_back(focus_id.to_string());
        }
    }
    while let Some(node_id) = queue.pop_front() {
        for edge in &graph.edges {
            if edge.from != node_id {
                continue;
            }
            let Some(target) = graph.nodes.get(&edge.to) else {
                continue;
            };
            if !edge.participates_in_default_closure() {
                continue;
            }
            if !target.participates_in_metric_closure() {
                continue;
            }
            if visited.insert(edge.to.clone()) {
                queue.push_back(edge.to.clone());
            }
        }
    }
    visited.into_iter().collect()
}
