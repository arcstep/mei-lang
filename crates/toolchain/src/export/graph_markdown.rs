use std::collections::{BTreeMap, BTreeSet, VecDeque};

use mei_lang_kernel::{
    build_runtime_eval_plan, AnalysisGraph, DatasetView, EvalPlan, RuntimeMetricEvalScope,
};

pub fn format_semantic_graph_markdown(graph: &AnalysisGraph, focus_metric: Option<&str>) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "- nodes: {} · edges: {}\n",
        graph.nodes.len(),
        graph.edges.len()
    ));
    let invariants = graph.validate_invariants();
    if invariants.is_empty() {
        md.push_str("- invariants: none\n");
    } else {
        md.push_str("- invariants:\n");
        for item in invariants.iter().take(8) {
            md.push_str(&format!("  - {item}\n"));
        }
    }
    if let Some(focus) = focus_metric.map(str::trim).filter(|value| !value.is_empty()) {
        md.push_str(&format!("- focus: `{focus}`\n"));
        let closure = closure_neighbors(graph, focus);
        if !closure.is_empty() {
            md.push_str("- neighborhood:\n");
            for id in closure.iter().take(12) {
                md.push_str(&format!("  - `{id}`\n"));
            }
        }
    }
    md
}

pub fn format_eval_plan_markdown(plan: &EvalPlan) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "- nodes: {} · edges: {}\n",
        plan.nodes.len(),
        plan.edges.len()
    ));
    md.push_str(&format!("- targets: `{}`\n", plan.targets.join("`, `")));
    for (id, node) in plan.nodes.iter().take(12) {
        let inbound = plan
            .edges
            .iter()
            .filter(|edge| edge.to == *id)
            .count();
        md.push_str(&format!(
            "  - `{id}` kind={:?} inbound_edges={inbound}\n",
            node.kind,
        ));
    }
    md
}

pub fn build_eval_plan_markdown(
    metric_defs: &BTreeMap<String, serde_json::Value>,
    metric_id: &str,
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> String {
    let plan = build_runtime_eval_plan(
        metric_defs,
        Some(&[metric_id.to_string()]),
        datasets,
        scope,
    );
    format_eval_plan_markdown(&plan)
}

fn closure_neighbors(graph: &AnalysisGraph, focus: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([focus.to_string()]);
    let mut out = Vec::new();
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if current != focus {
            out.push(current.clone());
        }
        for edge in &graph.edges {
            if edge.from == current && seen.insert(edge.to.clone()) {
                queue.push_back(edge.to.clone());
            }
            if edge.to == current && seen.insert(edge.from.clone()) {
                queue.push_back(edge.from.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::AnalysisGraph;

    #[test]
    fn semantic_markdown_includes_counts() {
        let md = format_semantic_graph_markdown(&AnalysisGraph::default(), None);
        assert!(md.contains("nodes:"));
    }
}
