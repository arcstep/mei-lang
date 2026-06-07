mod compile;
mod compile_semantics;
mod config_refs;
mod eval;
mod geojson;
mod host_contract;
mod mei_config;
mod model;
mod ops_journal;
mod runtime;
mod runtime_resource_index;
mod source_version;
mod typed_refs;
mod workspace;

pub use compile::{
    build_runtime_analysis_contracts, build_runtime_analysis_graph,
    cached_load_xlsx_table_snapshot, capsule_path_from_namespaced_resource_id,
    clear_runtime_compile_caches, clear_runtime_eval_node_cache, coerce_rows_to_schema,
    compile_app, compile_app_from_root, compile_app_from_root_with_options,
    compile_app_with_options, compile_revision_plan_from_root_with_options,
    compile_revision_token_from_root_with_options, dataset_materialize_cache_epoch,
    build_runtime_eval_plan, evaluate_runtime_metric_defs, evaluate_runtime_metric_defs_with_scope,
    evaluate_runtime_metric_defs_with_scope_and_dag,
    imported_capsule_path_from_world_metrics_resource_id, local_dataset_id_from_namespaced_token,
    materialize_xlsx_column_headers, panel_resolved_has_head, resolve_default_scene_from_root,
    resolve_runtime_metric_def_key, runtime_analysis_closure_metric_ids,
    runtime_eval_node_cache_enabled, scene_payload_cache_epoch, try_get_cached_xlsx_table_snapshot,
    CompileOptions, CompileRevisionPlan, CompileWatchedFile, EvalPlan, EvalPlanEdge,
    EvalPlanEdgeKind, EvalPlanNode, EvalPlanNodeKind, EvalPlanScope, RequestDagMetrics,
    RuntimeMetricEvalReport, RuntimeMetricEvalScope, TableSnapshot, TableSnapshotKey,
};
pub use compile_semantics::COMPILE_SEMANTICS_GENERATION;

pub use eval::{describe_dsl, evaluate_mei_file, evaluate_mei_source};
pub use geojson::{parse_geojson_rows, rows_from_geojson_value};
pub use host_contract::{
    host_protocol_descriptor, host_runtime_capabilities_catalog, host_runtime_contract_descriptor,
    HostSurface, HOST_RUNTIME_CONTRACT_SCHEMA, HOST_RUNTIME_PROTOCOL_SCHEMA,
};
pub use model::{
    AnalysisEdge, AnalysisGraph, AnalysisNode, BlockDecl, ColumnSchema, CompiledApp,
    CompiledSceneRoute, ComponentAsset, DataRef, DataTransform, DatasetSourceRef, DatasetView,
    Diagnostic, DimensionBinding, FilterIntent, FilterIntentSource, FilterOperator, FlowDecl,
    FrameDecl, LayoutDecl, LoadedResource, MetricContract, MetricPackContract, MetricRef,
    MetricShape, PanelDecl, PanelRefEmbedDecl, QueryState, QueryTimeRange, ResourceDecl,
    RuleClickDecl, RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl, RuleStartDecl,
    RuleSubjectTimerDecl, RuleTimerDecl, SceneContract, SceneDecl, SemanticEdgeKind,
    SemanticNodeKind, Severity, SourceDecl, ThemeDecl, UiNodeDecl, WorkspaceAppMeta, WorkspaceNode,
    WorldCellDecl, WorldMetricLedgerEntry,
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
pub use config_refs::{
    config_ref_to_json, decode_config_ref_value, decode_theme_ref_token, is_config_ref_source,
    ops_source_entry_to_decl, parse_config_ref_path, source_decl_from_value, theme_ref_token,
    walk_value_for_config_refs, ConfigRefExpr, ConfigRefKind, ConfigRefResolver,
    CONFIG_REF_SOURCE_KIND, THEME_REF_PREFIX,
};
pub use mei_config::{
    app_mei_config_path, load_mei_config_for_app, load_workspace_config, merge_ops_section,
    resolve_app_entry_main, resolve_app_main_path, resolve_mei_config_path, workspace_config_path,
    write_mei_config, AppEntryConfig, AppFeaturesConfig, AppPathsConfig, DiscoverConfig,
    FileCacheConfig, FileCacheSettings, MeiConfig, OpsBasemapEntry, OpsConfig, OpsConfigPatch,
    OpsSourceEntry, RuntimeConfig, WorkspaceConfig, DEFAULT_APP_ENTRY_MAIN, MEI_CONFIG_FILENAME,
    MEI_WORKSPACE_CONFIG_FILENAME, OPS_JOURNAL_REL_PATH, OPS_OBJECT_KINDS,
};
pub use ops_journal::{apply_ops_patch_with_journal, journal_path, OpsJournal, OpsJournalEntry};
pub use typed_refs::{
    decode_binding_value, decode_ref_value, ref_to_json, BindingValue, RefExpr, RefKind,
    SceneLocator, SceneRegistry,
};
pub use workspace::{discover_apps, load_component_assets, read_source_file, source_tree};
