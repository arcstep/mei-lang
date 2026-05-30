mod compile;
mod eval;
mod geojson;
mod model;
mod runtime;
mod runtime_resource_index;
mod source_version;
mod typed_refs;
mod workspace;

pub use compile::{
    clear_runtime_compile_caches, clear_runtime_eval_node_cache, compile_app, compile_app_from_root,
    compile_app_from_root_with_options, compile_app_with_options,
    compile_revision_plan_from_root_with_options, compile_revision_token_from_root_with_options,
    cached_load_xlsx_table_snapshot,
    try_get_cached_xlsx_table_snapshot,
    TableSnapshot, TableSnapshotKey,
    dataset_materialize_cache_epoch, evaluate_runtime_metric_defs,
    evaluate_runtime_metric_defs_with_scope, evaluate_runtime_metric_defs_with_scope_and_dag,
    build_runtime_analysis_contracts, build_runtime_analysis_graph, materialize_xlsx_column_headers,
    panel_resolved_has_head, resolve_default_scene_from_root, resolve_runtime_metric_def_key,
    runtime_analysis_closure_metric_ids, runtime_eval_node_cache_enabled, RequestDagMetrics,
    RuntimeMetricEvalScope,
    scene_payload_cache_epoch, CompileOptions, CompileRevisionPlan, CompileWatchedFile,
};

pub use eval::{describe_dsl, evaluate_mei_file, evaluate_mei_source};
pub use geojson::{parse_geojson_rows, rows_from_geojson_value};
pub use model::{
    AnalysisEdge, AnalysisGraph, AnalysisNode, BlockDecl, ColumnSchema, CompiledApp,
    CompiledSceneRoute, ComponentAsset, DataRef, DataTransform, DatasetSourceRef, DatasetView,
    Diagnostic, FlowDecl, FrameDecl, LayoutDecl, LoadedResource, MetricContract, MetricPackContract,
    MetricRef, MetricShape, PanelDecl,
    PanelRefEmbedDecl, ResourceDecl, RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl,
    RuleRequireDecl, RuleStartDecl, RuleSubjectTimerDecl, RuleTimerDecl, SceneContract, SceneDecl,
    Severity, SourceDecl, ThemeDecl, UiNodeDecl, WorkspaceAppMeta, WorkspaceNode, WorldCellDecl,
    WorldMetricLedgerEntry,
};
pub use runtime::{
    initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step, RuntimeIntent,
    RuntimeSceneView, RuntimeState, RuntimeSubjectTimerState, RuntimeTraceItem,
};
pub use runtime_resource_index::{
    build_runtime_resource_index, build_runtime_resource_map, is_forbidden_legacy_resource_id,
    locate_dataset_resource, resolve_dataset_resource_id, resolve_dataset_selector_value,
    RuntimeResourceIndex, RuntimeResourceResolveError,
};
pub use source_version::{
    compare_version_tokens, parse_versioned_upload_file_name, read_upload_registry,
    register_upload_version, resolve_versioned_source_identifier, resolve_versioned_source_path,
    write_upload_registry, ParsedVersionedUploadFile, UploadAliasRecord, UploadRegistry,
    UploadVersionRecord,
};
pub use typed_refs::{
    decode_binding_value, decode_ref_value, ref_to_json, BindingValue, RefExpr, RefKind,
    SceneLocator, SceneRegistry,
};
pub use workspace::{discover_apps, load_component_assets, read_source_file, source_tree};
