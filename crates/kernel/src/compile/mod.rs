use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::Value;

mod analysis;
mod app_compile;
mod app_decl;
mod authoring_eval;
mod build_board_index;
mod build_experience;
mod build_experience_index;
mod build_mcg_index;
mod build_node_context;
mod build_template_index;
mod catalog;
mod component_authoring_preview;
mod component_pack_preview;
mod data_snapshot;
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
mod reachability_tree;
mod resources;
mod route_compile;
mod rowset_engine;
mod scene;
mod scene_binding;
mod scene_payload_cache;
mod shards;
mod source_paths;
mod source_tree_enrich;
mod source_tree_world;
mod ui_data_policy;
mod xlsx_singleflight;
#[cfg(test)]
mod build_preview_target_probe;

pub use analysis::dates::{
    coerce_calendar_columns_in_rows, coerce_row_to_schema, coerce_rows_to_schema,
    format_calendar_date_value,
};
pub use data_snapshot::{
    access_parquet_import_required, data_snapshot_import_manifest_path, data_snapshot_store_root,
    parquet_sidecar_write_allowed, parquet_snapshot_path, publish_xlsx_data_snapshots_for_paths,
    read_data_snapshot_import_manifest, resolve_data_snapshot_import_entry,
    source_file_content_signature, try_load_xlsx_parquet_snapshot,
    write_data_snapshot_import_manifest, write_xlsx_parquet_snapshot, DataSnapshotImportEntry,
    DataSnapshotImportManifest, DATA_SNAPSHOT_IMPORT_MANIFEST_SCHEMA_VERSION,
    DATA_SNAPSHOT_SCHEMA_VERSION,
};
pub use loaders::{load_xlsx_table_snapshot, materialize_xlsx_column_headers};

pub use source_paths::canonicalize_compiled_app_source_paths;
pub use app_compile::{
    compile_app, compile_app_from_root, compile_app_from_root_with_options,
    compile_app_from_root_with_options_and_revision, compile_app_with_options,
    compile_app_with_options_and_revision, compile_revision_plan_from_root_with_options,
    compile_revision_token_from_root_with_options, resolve_default_scene_from_root,
    CompileAppArtifacts,
};
pub use build_board_index::build_board_index;
pub use build_experience::{
    aggregate_use_key_badges, backing_refs_from_block_props, block_instance_id,
    build_experience_path, build_overview_backing, compile_coordinate_for_node,
    compile_scene_from_build_node, compile_scene_from_build_node_with_app, experience_layout_hint,
    experience_mount_chain, format_experience_path, preview_target_from_build_node_with_app,
    preview_target_relative_to_app, BuildCompileCoordinate, BuildPreviewKind,
};
pub use build_experience_index::{build_experience_index, enrich_reachability_tree_compile_coords};
pub use build_node_context::{
    build_preview_panel_scope, catalog_preview_target_for_build_node,
    default_build_node_for_compiled, preview_target_from_build_node, resolve_build_node_context,
    BuildNodeContext,
};
pub use component_authoring_preview::{
    component_authoring_example_workspace_path, scene_contract_contains_use_key,
};
pub use build_template_index::build_template_index;
pub use discover_routes::{CompileOptions, CompileRevisionPlan, CompileWatchedFile};
pub use reachability_tree::{
    build_reachability_tree, filter_reachability_roots_for_stock_catalog,
    is_stock_catalog_facet_root, ReachabilityTreeNode,
    ReachabilityTreeRoot,
};

pub use materialize_cache::cached_load_xlsx_table_snapshot;
pub use materialize_cache::dataset_materialize_cache_epoch;
pub use materialize_cache::dataset_materialize_cache_hit_count;
pub use materialize_cache::try_get_cached_xlsx_table_snapshot;
pub use materialize_cache::TableSnapshot;
pub use materialize_cache::TableSnapshotKey;
pub use panel_normalize::{normalize_panel_slots, panel_resolved_has_head};
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
    capsule_path_from_namespaced_resource_id, evaluate_runtime_metric_defs_with_plan_and_dag,
    imported_capsule_path_from_world_metrics_resource_id, local_dataset_id_from_namespaced_token,
    resolve_metric_contract_key, resolve_runtime_metric_def_key, EvalPlan, EvalPlanEdge,
    EvalPlanEdgeKind, EvalPlanNode,
    EvalPlanNodeKind, EvalPlanScope, RuntimeMetricEvalReport,
};

#[cfg(test)]
mod tests;
