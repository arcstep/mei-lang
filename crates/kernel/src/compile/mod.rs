use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

mod analysis;
mod app_compile;
mod app_decl;
mod authoring_eval;
mod catalog;
mod decl_file_cache;
mod decls;
mod dependency_graph;
mod discover_routes;
mod entry_payload;
mod load_external;
mod loaders;
mod materialize;
mod materialize_cache;
mod mutations;
mod panel_normalize;
mod projection_assembly;
mod resources;
mod route_compile;
mod scene;
mod scene_binding;
mod scene_payload_cache;
mod source_tree_enrich;
mod source_tree_world;
mod shards;
mod ui_data_policy;

pub use analysis::dates::{
    coerce_calendar_columns_in_rows, coerce_row_to_schema, coerce_rows_to_schema,
    format_calendar_date_value,
};
pub use loaders::{load_xlsx_table_snapshot, materialize_xlsx_column_headers};

pub use app_compile::{
    compile_app, compile_app_from_root, compile_app_from_root_with_options,
    compile_app_from_root_with_options_and_revision, compile_app_with_options,
    compile_app_with_options_and_revision, compile_revision_plan_from_root_with_options,
    compile_revision_token_from_root_with_options, resolve_default_scene_from_root,
    CompileAppArtifacts,
};
pub use discover_routes::{CompileOptions, CompileRevisionPlan, CompileWatchedFile};

pub use materialize_cache::cached_load_xlsx_table_snapshot;
pub use materialize_cache::dataset_materialize_cache_epoch;
pub use materialize_cache::try_get_cached_xlsx_table_snapshot;
pub use materialize_cache::TableSnapshot;
pub use materialize_cache::TableSnapshotKey;
pub use panel_normalize::panel_resolved_has_head;
pub use scene_payload_cache::scene_payload_cache_epoch;

pub fn clear_runtime_compile_caches() {
    materialize_cache::clear_materialize_cache();
    scene_payload_cache::clear_scene_payload_cache();
    catalog::clear_dataset_catalog_index_cache();
    decl_file_cache::clear_decl_file_cache();
    dependency_graph::clear_dependency_graph_cache();
    dependency_graph::clear_file_content_hash_cache();
    clear_runtime_eval_node_cache();
}

pub fn clear_runtime_eval_node_cache() -> usize {
    analysis::eval_context::clear_eval_node_cache()
}

#[cfg(test)]
pub(crate) use materialize_cache::{
    clear_materialize_cache_for_tests, legacy_rows_cache_len_for_tests,
};
#[cfg(test)]
pub(crate) use scene_payload_cache::{
    clear_scene_payload_cache_for_tests, scene_payload_cache_len_for_tests,
};

pub fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, crate::model::MetricContract>> {
    materialize::evaluate_runtime_metric_defs(metric_defs, base_rows, datasets, metric_ids)
}

pub fn evaluate_runtime_metric_defs_with_scope(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &analysis::eval_context::RuntimeMetricEvalScope,
) -> Result<BTreeMap<String, crate::model::MetricContract>> {
    materialize::evaluate_runtime_metric_defs_with_scope(
        metric_defs,
        base_rows,
        datasets,
        metric_ids,
        scope,
    )
}

pub fn evaluate_runtime_metric_defs_with_scope_and_dag(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &analysis::eval_context::RuntimeMetricEvalScope,
) -> Result<(
    BTreeMap<String, crate::model::MetricContract>,
    materialize::RuntimeMetricEvalReport,
)> {
    materialize::evaluate_runtime_metric_defs_with_scope_and_dag(
        metric_defs,
        base_rows,
        datasets,
        metric_ids,
        scope,
    )
}

pub fn build_runtime_analysis_graph(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> crate::model::AnalysisGraph {
    materialize::build_analysis_graph(metric_defs, root_dataset_id)
}

pub fn build_runtime_analysis_contracts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> BTreeMap<String, Value> {
    materialize::build_analysis_contracts(metric_defs, root_dataset_id)
}

pub fn build_runtime_eval_plan(
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: Option<&[String]>,
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    scope: &analysis::eval_context::RuntimeMetricEvalScope,
) -> EvalPlan {
    materialize::build_runtime_eval_plan(metric_defs, metric_ids, datasets, scope)
}

pub fn runtime_analysis_closure_metric_ids(
    graph: &crate::model::AnalysisGraph,
    focus_ids: &[String],
) -> Vec<String> {
    materialize::analysis_closure_metric_ids(graph, focus_ids)
}

pub use analysis::eval_context::{
    runtime_eval_node_cache_enabled, RequestDagMetrics, RuntimeMetricEvalScope,
};
pub use materialize::{
    capsule_path_from_namespaced_resource_id, imported_capsule_path_from_world_metrics_resource_id,
    local_dataset_id_from_namespaced_token, resolve_runtime_metric_def_key, EvalPlan, EvalPlanEdge,
    EvalPlanEdgeKind, EvalPlanNode, EvalPlanNodeKind, EvalPlanScope, RuntimeMetricEvalReport,
};

#[cfg(test)]
mod tests;
