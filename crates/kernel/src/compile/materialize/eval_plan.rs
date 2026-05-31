use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::compile::analysis::eval_context::{RequestDagMetrics, RuntimeMetricEvalScope};
use crate::model::DatasetView;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlan {
    #[serde(default)]
    pub scope: EvalPlanScope,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub nodes: BTreeMap<String, EvalPlanNode>,
    #[serde(default)]
    pub edges: Vec<EvalPlanEdge>,
}

impl EvalPlan {
    pub fn node_count_by_kind(&self, kind: EvalPlanNodeKind) -> usize {
        self.nodes
            .values()
            .filter(|node| node.kind == kind)
            .count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlanScope {
    pub base_dataset_id: String,
    pub scene_id: String,
    pub target: String,
    pub search: String,
    pub filters_fingerprint: String,
    pub dependency_revision_key: String,
}

impl From<&RuntimeMetricEvalScope> for EvalPlanScope {
    fn from(scope: &RuntimeMetricEvalScope) -> Self {
        Self {
            base_dataset_id: scope.base_dataset_id.clone(),
            scene_id: scope.scene_id.clone(),
            target: scope.target.clone(),
            search: scope.search.clone(),
            filters_fingerprint: scope.filters_fingerprint.clone(),
            dependency_revision_key: scope.dependency_revision_key.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvalPlanNodeKind {
    #[default]
    Unknown,
    MetricEval,
    Rowset,
    ScalarExpr,
    Hydrate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlanNode {
    pub id: String,
    #[serde(default)]
    pub kind: EvalPlanNodeKind,
    #[serde(default)]
    pub metric_id: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub expr_fingerprint: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvalPlanEdgeKind {
    #[default]
    DependsOn,
    Hydrates,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalPlanEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub kind: EvalPlanEdgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeMetricEvalReport {
    #[serde(default)]
    pub eval_plan: EvalPlan,
    #[serde(default)]
    pub request_dag_metrics: RequestDagMetrics,
}

pub(crate) fn build_eval_plan(
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: Option<&[String]>,
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> EvalPlan {
    let targets = selected_metric_ids(metric_defs, metric_ids);
    let mut nodes = BTreeMap::<String, EvalPlanNode>::new();
    let mut edges = BTreeSet::<(String, String, EvalPlanEdgeKind)>::new();
    for metric_id in &targets {
        let metric_node_id = metric_plan_node_id(metric_id);
        nodes.entry(metric_node_id.clone()).or_insert(EvalPlanNode {
            id: metric_node_id.clone(),
            kind: EvalPlanNodeKind::MetricEval,
            metric_id: Some(metric_id.clone()),
            dataset_id: None,
            expr_fingerprint: None,
            label: Some(metric_id.clone()),
        });
        let Some(raw) = metric_defs.get(metric_id).and_then(Value::as_object) else {
            continue;
        };
        if let Some(dataset_id) = first_non_empty_string(raw, &["dataset", "dataset_id"]) {
            add_hydrate_dependency(&metric_node_id, &dataset_id, scope, datasets, &mut nodes, &mut edges);
        }
        if let Some(values) = raw.get("values").and_then(Value::as_object) {
            for (field, expr) in values {
                visit_expr(
                    expr,
                    &metric_node_id,
                    Some(metric_id.as_str()),
                    Some(field.as_str()),
                    scope,
                    datasets,
                    &mut nodes,
                    &mut edges,
                );
            }
        } else if let Some(expr) = raw.get("series").or_else(|| raw.get("list")).or_else(|| raw.get("value"))
        {
            visit_expr(
                expr,
                &metric_node_id,
                Some(metric_id.as_str()),
                None,
                scope,
                datasets,
                &mut nodes,
                &mut edges,
            );
        }
    }
    EvalPlan {
        scope: EvalPlanScope::from(scope),
        targets,
        nodes,
        edges: edges
            .into_iter()
            .map(|(from, to, kind)| EvalPlanEdge { from, to, kind })
            .collect(),
    }
}

fn selected_metric_ids(metric_defs: &BTreeMap<String, Value>, metric_ids: Option<&[String]>) -> Vec<String> {
    if let Some(ids) = metric_ids {
        let selected = ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .filter(|metric_id| metric_defs.contains_key(*metric_id))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if !selected.is_empty() {
            return selected;
        }
    }
    metric_defs.keys().cloned().collect()
}

fn visit_expr(
    expr: &Value,
    parent_node_id: &str,
    metric_id: Option<&str>,
    field_label: Option<&str>,
    scope: &RuntimeMetricEvalScope,
    datasets: &BTreeMap<String, DatasetView>,
    nodes: &mut BTreeMap<String, EvalPlanNode>,
    edges: &mut BTreeSet<(String, String, EvalPlanEdgeKind)>,
) {
    match expr {
        Value::Array(items) => {
            for item in items {
                visit_expr(item, parent_node_id, metric_id, field_label, scope, datasets, nodes, edges);
            }
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                let dataset_id = first_non_empty_string(map, &["from_dataset", "id"]);
                let node_id = expr_plan_node_id(EvalPlanNodeKind::Rowset, expr);
                nodes.entry(node_id.clone()).or_insert(EvalPlanNode {
                    id: node_id.clone(),
                    kind: EvalPlanNodeKind::Rowset,
                    metric_id: metric_id.map(str::to_string),
                    dataset_id: dataset_id.clone(),
                    expr_fingerprint: Some(expr_fingerprint(expr)),
                    label: dataset_id.clone().or_else(|| field_label.map(str::to_string)),
                });
                edges.insert((
                    parent_node_id.to_string(),
                    node_id.clone(),
                    EvalPlanEdgeKind::DependsOn,
                ));
                if let Some(dataset_id) = dataset_id {
                    add_hydrate_dependency(&node_id, &dataset_id, scope, datasets, nodes, edges);
                }
                return;
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                let plan_kind = analysis_expr_plan_kind(
                    map.get("type").and_then(Value::as_str).unwrap_or_default(),
                );
                let node_id = expr_plan_node_id(plan_kind, expr);
                nodes.entry(node_id.clone()).or_insert(EvalPlanNode {
                    id: node_id.clone(),
                    kind: plan_kind,
                    metric_id: metric_id.map(str::to_string),
                    dataset_id: None,
                    expr_fingerprint: Some(expr_fingerprint(expr)),
                    label: map
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| field_label.map(str::to_string)),
                });
                edges.insert((
                    parent_node_id.to_string(),
                    node_id.clone(),
                    EvalPlanEdgeKind::DependsOn,
                ));
                if let Some(dataset_id) = dataset_dependency_from_analysis_expr(map) {
                    if let Some(node) = nodes.get_mut(&node_id) {
                        if node.dataset_id.is_none() {
                            node.dataset_id = Some(dataset_id.clone());
                        }
                    }
                    add_hydrate_dependency(&node_id, &dataset_id, scope, datasets, nodes, edges);
                }
                for child in map.values() {
                    visit_expr(child, &node_id, metric_id, None, scope, datasets, nodes, edges);
                }
                return;
            }
            for child in map.values() {
                visit_expr(child, parent_node_id, metric_id, field_label, scope, datasets, nodes, edges);
            }
        }
        _ => {}
    }
}

fn add_hydrate_dependency(
    parent_node_id: &str,
    dataset_id: &str,
    scope: &RuntimeMetricEvalScope,
    datasets: &BTreeMap<String, DatasetView>,
    nodes: &mut BTreeMap<String, EvalPlanNode>,
    edges: &mut BTreeSet<(String, String, EvalPlanEdgeKind)>,
) {
    if !dataset_requires_hydration(dataset_id, scope, datasets) {
        return;
    }
    let hydrate_node_id = hydrate_plan_node_id(dataset_id);
    nodes.entry(hydrate_node_id.clone()).or_insert(EvalPlanNode {
        id: hydrate_node_id.clone(),
        kind: EvalPlanNodeKind::Hydrate,
        metric_id: None,
        dataset_id: Some(dataset_id.to_string()),
        expr_fingerprint: None,
        label: Some(dataset_id.to_string()),
    });
    edges.insert((
        parent_node_id.to_string(),
        hydrate_node_id,
        EvalPlanEdgeKind::Hydrates,
    ));
}

fn metric_plan_node_id(metric_id: &str) -> String {
    format!("metric:{metric_id}")
}

fn hydrate_plan_node_id(dataset_id: &str) -> String {
    format!("hydrate:{}", dataset_id.trim())
}

fn expr_plan_node_id(kind: EvalPlanNodeKind, expr: &Value) -> String {
    let prefix = match kind {
        EvalPlanNodeKind::Rowset => "rowset",
        EvalPlanNodeKind::ScalarExpr => "scalar",
        EvalPlanNodeKind::MetricEval => "metric",
        EvalPlanNodeKind::Hydrate => "hydrate",
        EvalPlanNodeKind::Unknown => "expr",
    };
    format!("{prefix}:{}", expr_fingerprint(expr))
}

fn expr_fingerprint(expr: &Value) -> String {
    let serialized = serde_json::to_string(&canonicalize_expr_value(expr)).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serialized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn canonicalize_expr_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_expr_value).collect()),
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

fn analysis_expr_plan_kind(analysis_type: &str) -> EvalPlanNodeKind {
    match analysis_type.trim() {
        "count"
        | "sum"
        | "avg"
        | "min"
        | "max"
        | "median"
        | "unique_count"
        | "item_count"
        | "ratio"
        | "percent"
        | "sum_first_number"
        | "sum_rowset_counts"
        | "number"
        | "lit"
        | "mom"
        | "yoy" => EvalPlanNodeKind::ScalarExpr,
        _ => EvalPlanNodeKind::Rowset,
    }
}

fn dataset_dependency_from_analysis_expr(map: &serde_json::Map<String, Value>) -> Option<String> {
    if map.get("type").and_then(Value::as_str) == Some("rows") {
        return first_non_empty_string(map, &["dataset"]);
    }
    None
}

fn dataset_requires_hydration(
    dataset_id: &str,
    scope: &RuntimeMetricEvalScope,
    datasets: &BTreeMap<String, DatasetView>,
) -> bool {
    let normalized = dataset_id.trim();
    if normalized.is_empty() || normalized == scope.base_dataset_id.trim() {
        return false;
    }
    let Some(dataset) = lookup_dataset_view(datasets, normalized) else {
        return false;
    };
    let path = dataset.source.path.trim();
    if path.is_empty() || path.starts_with("dataset_view:") {
        return false;
    }
    let kind = dataset.source.kind.trim();
    if kind.eq_ignore_ascii_case("derived") || kind.eq_ignore_ascii_case("world_metrics") {
        return false;
    }
    true
}

fn lookup_dataset_view<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    datasets
        .get(normalized)
        .or_else(|| datasets.get(dataset_id))
        .or_else(|| {
            datasets.iter().find_map(|(key, dataset)| {
                (dataset.id == normalized
                    || key.ends_with(&format!("::{normalized}"))
                    || key.ends_with(&format!("/{normalized}")))
                .then_some(dataset)
            })
        })
}

fn first_non_empty_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::{build_eval_plan, EvalPlanNodeKind};
    use crate::compile::analysis::eval_context::RuntimeMetricEvalScope;
    use crate::model::{DatasetView, QueryState, SourceDecl};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn dataset(id: &str, kind: &str, path: &str) -> DatasetView {
        DatasetView {
            id: id.to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: kind.to_string(),
                path: path.to_string(),
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
            runtime_analysis_contracts: Default::default(),
        }
    }

    #[test]
    fn build_eval_plan_tracks_metric_rowset_scalar_and_hydrate_nodes() {
        let defs = BTreeMap::from([(
            "sales_total".to_string(),
            json!({
                "key": "sales_total",
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "count",
                        "rowset": {
                            "__kind": "analysis_expr",
                            "type": "rows",
                            "dataset": "warning_detail"
                        }
                    }
                }
            }),
        )]);
        let datasets = BTreeMap::from([
            ("warning_list".to_string(), dataset("warning_list", "derived", "dataset_view:warning_list")),
            ("warning_detail".to_string(), dataset("warning_detail", "xlsx", "upload/detail.xlsx")),
        ]);
        let scope = RuntimeMetricEvalScope {
            base_dataset_id: "warning_list".to_string(),
            scene_id: "home".to_string(),
            target: "scenes/home.mei".to_string(),
            search: String::new(),
            query_state: QueryState::default(),
            filter_intents: Vec::new(),
            dimension_bindings: Vec::new(),
            filters_fingerprint: "{}".to_string(),
            dependency_revision_key: "deps=v1".to_string(),
        };
        let plan = build_eval_plan(&defs, Some(&["sales_total".to_string()]), &datasets, &scope);
        assert_eq!(plan.targets, vec!["sales_total".to_string()]);
        assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::MetricEval), 1);
        assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::ScalarExpr), 1);
        assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::Rowset), 1);
        assert_eq!(plan.node_count_by_kind(EvalPlanNodeKind::Hydrate), 1);
        assert!(
            plan.edges
                .iter()
                .any(|edge| edge.from == "metric:sales_total" && edge.kind == super::EvalPlanEdgeKind::DependsOn)
        );
    }
}
