mod auth_journal;
mod cache_generation;
mod catalog_app;
mod compile;
mod compile_scope_filter;
mod compile_semantics;
mod config_refs;
mod eval;
mod geojson;
mod host_contract;
mod mei_config;
mod model;
mod ops_journal;
mod runtime;
mod runtime_dev_eval;
mod runtime_resource_index;
mod source_version;
mod theme_tokens;
mod typed_refs;
mod warmup_manifest;
mod warmup_t2_page_autogen;
mod workspace;
mod workspace_profile;

pub use cache_generation::{
    bump_cache_generation, cache_generation_path, is_file_source_dataset, load_cache_generation,
    resolve_app_data_generation, save_cache_generation, CacheGenerationRecord,
    SourceGenerationRecord, CACHE_GENERATION_REL, CACHE_GENERATION_SCHEMA_VERSION,
    DEFAULT_DATABASE_TTL_MS,
};
pub use catalog_app::{
    catalog_app_needs_sync, catalog_scene_route_for_build_node, catalog_scene_routes_from_app_root,
    collect_stock_catalog_routes, discover_stock_catalog_packs, is_catalog_build_app,
    render_stock_catalog_main_mei, stock_catalog_app_root, StockCatalogPackDiscovery,
    StockCatalogRouteEntry, StockCatalogRouteKind,
};
pub use compile::layout_budget::{
    hierarchy_spacing_defaults, padding_profile_css, HierarchySpacingDefaults, HIERARCHY_PX_1,
    HIERARCHY_REGION_OUTER, HIERARCHY_SECTION_OUTER, HIERARCHY_SPACE_1,
};
pub use compile::{
    access_parquet_import_required, block_instance_id, build_experience_index,
    build_experience_path, build_overview_backing, build_preview_panel_scope,
    build_preview_ui_scope, build_reachability_tree, build_runtime_analysis_contracts,
    build_runtime_analysis_graph, build_runtime_eval_plan, build_runtime_metric_artifacts,
    build_ui_layout_index, cached_load_xlsx_table_snapshot,
    capsule_path_from_namespaced_resource_id, catalog_preview_target_for_build_node,
    clear_runtime_compile_caches, clear_runtime_eval_node_cache, coerce_calendar_columns_in_rows,
    coerce_row_to_schema, coerce_rows_to_schema, compile_app, compile_app_from_root,
    compile_app_from_root_with_options, compile_app_from_root_with_options_and_revision,
    compile_app_with_options, compile_app_with_options_and_revision, compile_coordinate_for_node,
    compile_revision_plan_from_root_with_options, compile_revision_token_from_root_with_options,
    compile_scene_from_build_node, compile_scene_from_build_node_with_app,
    component_authoring_example_workspace_path, data_snapshot_import_manifest_path,
    data_snapshot_store_root, dataset_materialize_cache_epoch, dataset_materialize_cache_hit_count,
    default_build_node_for_compiled, enrich_reachability_tree_compile_coords,
    enrich_runtime_page_instance_projection_slots, evaluate_runtime_metric_defs,
    evaluate_runtime_metric_defs_with_plan_and_dag, evaluate_runtime_metric_defs_with_scope,
    evaluate_runtime_metric_defs_with_scope_and_dag, experience_layout_hint,
    experience_mount_chain, filter_reachability_roots_for_stock_catalog,
    filter_roots_for_tree_mode, format_calendar_date_value, format_experience_path,
    format_ui_scope_agent_context, format_ui_scope_technical_detail,
    imported_capsule_path_from_world_metrics_resource_id, is_stock_catalog_facet_root,
    load_xlsx_table_snapshot, local_dataset_id_from_namespaced_token,
    materialize_fill_section_derived_heights, materialize_layout_budget_px,
    materialize_xlsx_column_headers, merge_ui_structure_root, normalize_panel_slots,
    normalize_panel_slots_with_options, panel_resolved_has_head, parquet_sidecar_write_allowed,
    parquet_snapshot_path, preview_target_from_build_node,
    preview_target_from_build_node_with_app, publish_xlsx_data_snapshots_for_paths,
    read_data_snapshot_import_manifest, resolve_build_node_context, resolve_build_preview_scope,
    resolve_build_preview_scope_for_ssr, resolve_data_snapshot_import_entry,
    resolve_default_scene_from_root, resolve_scene_ids_from_root, app_declares_scene_from_root,
    resolve_layout_budgets, resolve_layout_budgets_with_options, resolve_metric_contract_key, resolve_runtime_metric_def_key,
    runtime_analysis_closure_metric_ids, runtime_eval_node_cache_enabled,
    scene_contract_contains_use_key, scene_payload_cache_epoch, source_file_content_signature,
    try_get_cached_xlsx_table_snapshot, try_load_xlsx_parquet_snapshot,
    ui_scope_annotation_for_preview_panel, ui_scope_annotation_for_preview_path,
    ui_scope_for_block, validate_layout_budget_policy, validate_layout_budget_policy_with_options,
    write_data_snapshot_import_manifest, write_xlsx_parquet_snapshot, BuildCompileCoordinate,
    BuildNodeContext, BuildPreviewKind, CompileAppArtifacts, CompileOptions, CompileRevisionPlan,
    CompileWatchedFile, DataSnapshotImportEntry, DataSnapshotImportManifest, EvalPlan,
    EvalPlanEdge, EvalPlanEdgeKind, EvalPlanNode, EvalPlanNodeKind, EvalPlanScope,
    LayoutBudgetValidateOptions, ReachabilityTreeNode, ReachabilityTreeRoot, RequestDagMetrics,
    RuntimeMetricEvalReport, RuntimeMetricEvalScope, TableSnapshot, TableSnapshotKey,
    UiScopeBlockAnnotation, UiScopePanelAnnotation, DATA_SNAPSHOT_IMPORT_MANIFEST_SCHEMA_VERSION,
    DATA_SNAPSHOT_SCHEMA_VERSION,
};
pub use compile_scope_filter::{
    compile_scope_entry_allowed, compile_scope_scene_id_allowed, compile_scope_target_allowed,
};
pub use compile_semantics::COMPILE_SEMANTICS_GENERATION;

pub use auth_journal::{
    append_auth_journal_entry, auth_journal_path, AuthJournal, AuthJournalEntry,
};
pub use config_refs::{
    config_ref_to_json, decode_config_ref_value, decode_theme_ref_token, is_config_ref_source,
    ops_source_entry_to_decl, parse_config_ref_path, source_decl_from_value, theme_ref_token,
    walk_value_for_config_refs, ConfigRefExpr, ConfigRefKind, ConfigRefResolver,
    CONFIG_REF_SOURCE_KIND, THEME_REF_PREFIX,
};
pub use eval::{describe_dsl, evaluate_mei_file, evaluate_mei_source};
pub use geojson::{parse_geojson_rows, rows_from_geojson_value};
pub use host_contract::{
    host_extension_registry, host_extension_registry_descriptor, host_protocol_descriptor,
    host_requirements_descriptor, host_requirements_for_consumer,
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor, HostExtensionDescriptor,
    HostExtensionKind, HostRequirementsDescriptor, HostSurface, HOST_REQUIREMENTS_SCHEMA,
    HOST_RUNTIME_CONTRACT_SCHEMA, HOST_RUNTIME_PROTOCOL_SCHEMA,
};
pub use mei_config::{
    app_build_store_dir, app_env_build_dir, app_env_dir, app_env_var_dir, app_mei_config_path,
    app_source_rel_path_lookup_keys, app_var_store_dir, apply_toolchain_store_symlinks,
    attach_build_generation, begin_prebuild_generation, begin_prebuild_generation_with_hint,
    canonical_app_source_rel_path, clean_env_generations, clear_prebuild_build_root_override,
    default_scene_assembly_key, finalize_and_promote_build, finish_prebuild_generation,
    generate_build_id, is_app_mei_source_rel, is_dev_toolchain_alias, is_region_structure_mei_path,
    is_section_structure_mei_path, is_stock_catalog_app, is_stock_catalog_app_for_root,
    is_v2_app_root, load_mei_config_for_app, load_workspace_auth_bundle, load_workspace_config,
    mei_config_compile_revision_digest, merge_ops_section, merge_theme_layout_draft_into_theme,
    merge_theme_layout_overlay, migrate_apps_to_env_layout, migrate_build_var_store_to_env,
    migrate_flat_build_to_store, migrate_legacy_app_mei, migrate_legacy_workspace_mei,
    migrate_legacy_workspace_runtime_dir, normalize_env_generation_id,
    normalize_stock_relative_path, ops_theme_layout_revision_digest, ops_themes_revision_digest,
    prepare_dev_build_generation, prepare_dev_build_generation_with_hint, promote_build,
    read_build_manifest, read_links_state, record_toolchain_install_links, replace_env_generation,
    resolve_active_build_id, resolve_active_build_identity, resolve_active_build_identity_for_app,
    resolve_active_build_identity_with_hint, resolve_app_build_generation_from_current,
    resolve_app_build_root, resolve_app_build_store_root, resolve_app_data_snapshot_root,
    resolve_app_entry_main, resolve_app_eval_cache_root, resolve_app_id, resolve_app_main_path,
    resolve_app_mei_file_path, resolve_app_mei_store_root, resolve_app_registry_root,
    resolve_app_root, resolve_app_src_root, resolve_app_var_root, resolve_apps_root,
    resolve_authoring_root, resolve_build_footer_label, resolve_build_footer_label_with_hint,
    resolve_build_generation_for_prebuild, resolve_components_root, resolve_deploy_root,
    resolve_dev_toolchain_version, resolve_env_generation_id,
    resolve_env_generation_id_for_prebuild, resolve_env_version, resolve_live_ops_theme_value,
    resolve_mei_config_path, resolve_scene_assembly_rel, resolve_stock_root,
    resolve_templates_root, resolve_toolchain_root, resolve_toolchain_version,
    resolve_toolchain_version_with_hint, resolve_version_display_identity,
    resolve_version_display_identity_for_app, resolve_version_display_identity_with_hint,
    resolve_workspace_app_build_generations, resolve_workspace_cache_root,
    resolve_workspace_default_app_id, resolve_workspace_footer_label,
    resolve_workspace_footer_label_with_hint, resolve_workspace_graph_root,
    resolve_workspace_logs_root, resolve_workspace_platform_root, resolve_workspace_runtime_root,
    resolve_workspace_shell_theme, resolve_workspace_source_root_from_app_root,
    resolve_workspace_version, restore_prebuild_build_root_override, rollback_build,
    set_mei_package_root, set_prebuild_build_root_override, snapshot_prebuild_build_root_override,
    stock_authoring_source, stock_catalog_app_config, stock_catalog_app_id, stock_catalog_enabled,
    stock_components_source, stock_path_excluded, stock_templates_source,
    theme_layout_overlay_keys, toolchain_store_dir, validate_workspace_shell_theme,
    workspace_auth_config_path, workspace_auth_host_id, workspace_auth_state_dir,
    workspace_config_path, write_build_manifest, write_links_state, write_mei_config,
    write_workspace_auth_bundle, write_workspace_config, AccessAiExternalConfig, AppEntryConfig,
    AppFeaturesConfig, AppPathsConfig, AuthKeyPairConfig, AuthUserConfig, BuildGenerationSpec,
    BuildLinks, BuildManifest, CleanEnvEntry, CleanEnvPolicy, CleanEnvReport,
    ClientBootstrapConfig, CompileScopeFilterConfig, DiscoverConfig, FileCacheConfig,
    FileCacheSettings, LinksState, MeiConfig, MemoryWarmupConfig, MigrateEnvReport,
    OpsBasemapEntry, OpsConfig, OpsConfigPatch, OpsSourceEntry, PrebuildGeneration, RuntimeConfig,
    RuntimeMode, RuntimePlan, RuntimePlanApp, RuntimePlanTarget, RuntimeWarmupApp,
    RuntimeWarmupDatasetRequest, RuntimeWarmupManifest, RuntimeWarmupXlsxSource, SmartWarmupConfig,
    StockCatalogKind, ToolchainLinks, VersionDisplayIdentity, WorkspaceAuthBundle,
    WorkspaceAuthConfig, WorkspaceBuildConfig, WorkspaceBuildGenerationConfig,
    WorkspaceComplianceConfig, WorkspaceConfig, WorkspaceDeployDevEvalConfig, WorkspaceHostState,
    WorkspaceOpsConfig, WorkspacePathsConfig, WorkspaceProfile, WorkspaceStockBootstrapConfig,
    WorkspaceStockCatalogAppConfig, WorkspaceStockCatalogConfig, WorkspaceStockCatalogKindConfig,
    WorkspaceStockConfig, WorkspaceStockPreviewConfig, WorkspaceStockSourceEntry,
    WorkspaceToolchainConfig, WorkspaceWarmupAppConfig, WorkspaceWarmupConfig,
    WorkspaceWarmupDatasetConfig, WorkspaceWarmupXlsxConfig, APP_BUILD_STORE_REL,
    APP_CONFIG_FILENAME, APP_ENV_REL, APP_VAR_STORE_REL, AUTH_JOURNAL_REL_PATH,
    BUILD_MANIFEST_FILENAME, BUILD_MANIFEST_SCHEMA, DEFAULT_APPS_REL, DEFAULT_APP_ENTRY_MAIN,
    DEFAULT_HOST_STATE_ID, DEFAULT_STOCK_AUTHORING_REL, DEFAULT_STOCK_CATALOG_APP_ID,
    DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL, DEPLOY_LINKS_REL,
    DEV_TOOLCHAIN_ALIAS, DEV_TOOLCHAIN_VERSION, LEGACY_AUTH_JOURNAL_REL_PATH,
    LEGACY_WORKSPACE_AGENT_DB_REL, LEGACY_WORKSPACE_SNAPSHOT_DIR_REL,
    LEGACY_WORKSPACE_SNAPSHOT_GIT_REL, LINKS_STATE_SCHEMA, MEI_CONFIG_FILENAME,
    MEI_WORKSPACE_CONFIG_FILENAME, OPS_JOURNAL_REL_PATH, OPS_OBJECT_KINDS,
    PREBUILD_COMPILE_INDEX_REL, PREBUILD_DIR_REL, PREBUILD_LAST_BUILD_SUMMARY_REL,
    PRE_LOCAL_AUTH_JOURNAL_REL_PATH, TOOLCHAIN_ACTIVE_REL, TOOLCHAIN_STORE_REL,
    WORKSPACE_AGENT_DB_REL, WORKSPACE_AGENT_LOCAL_DIR_REL, WORKSPACE_AUTH_DIR_REL,
    WORKSPACE_CONFIG_FILENAME, WORKSPACE_HOSTS_DIR_REL, WORKSPACE_HOST_STATE_SCHEMA_VERSION,
    WORKSPACE_LOCAL_DIR_REL, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL, WORKSPACE_SNAPSHOT_DIR_REL,
    WORKSPACE_SNAPSHOT_GIT_REL,
};
pub use model::{
    resolve_build_view_query, tab_visible_for_node, tabs_for_node_kind, ui_role_depth_rank,
    ui_role_within_max_depth, AnalysisEdge, AnalysisGraph, AnalysisNode, BlockDecl, BuildExecScope,
    BuildNodeId, BuildNodeKind, BuildViewTab, ColumnSchema, CompiledApp, CompiledSceneRoute,
    ComponentAsset, DataMode, DataModeCeiling, DataRef, DataTransform, DatasetSourceRef,
    DatasetView, Diagnostic, DimensionBinding, FilterIntent, FilterIntentSource, FilterOperator,
    FlowDecl, FrameDecl, is_stage_registry_candidate, LayoutBudgetManifest,
    LayoutBudgetManifestEntry, LayoutDecl, LegacyBuildQuery, LoadedResource, MetricContract,
    MetricPackContract, MetricRef, MetricShape, PanelRefEmbedDecl, ProvenanceAnchor, QueryState,
    QueryTimeRange, ResolvedBuildViewQuery, ResourceDecl, ReviewProjection, RuleClickDecl,
    RuleEffectDecl, RuleOutcomeDecl, RuleRequireDecl, RuleStartDecl, RuleSubjectTimerDecl,
    RuleTimerDecl, SceneContract, SceneDecl, SemanticEdgeKind, SemanticNodeKind, Severity,
    SourceDecl, StageDescriptor, StageId, StageProfile, StageProgram, StageProgramIndex,
    StageProgramSummary, StageRegistry, StageRoute, StageSlideInput, StageSurface, StageUnit,
    StageUnitKind, SurfacePreviewPolicy, T2PageFileEntry, ThemeDecl, UiBudgetSummary,
    UiLayoutIndex, UiNodeDecl, ContentCapability, ContentCapabilityId, ContentCapabilityKind,
    FillDownPolicy, NarrationCatalog, NarrationCue, NarrationCueTarget, NarrationTrack,
    ProfileLayoutPolicy, SceneSlotModule, SceneSlotModuleId, SemanticSlotDecl, SizeAxisPolicy,
    SlotCardinality, bind_programs_to_abi, compute_narration_digest, compute_structure_digest,
    diagnose_slot_missing, profile_layout_policy_digest, project_abi,
    validate_abi_against_programs, apply_cockpit_stage_decl, CockpitFillDecl, CockpitStageDecl,
    CockpitStepDecl, AbiProjection, AbiProjectionInput,
    UiScopeNode, UiScopeRole, UiSourceAnchor, UiTreeNode, WorkspaceAppMeta, WorkspaceNode,
    WorldCellDecl, WorldMetricLedgerEntry, WorldSemanticDataset, WorldSemanticExplainBlock,
    WorldSemanticFileIndex, WorldSemanticMetric,
};
pub use ops_journal::{apply_ops_patch_with_journal, journal_path, OpsJournal, OpsJournalEntry};
pub use runtime::{
    initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step, RuntimeIntent,
    RuntimeSceneView, RuntimeState, RuntimeSubjectTimerState, RuntimeTraceItem,
};
pub use runtime_dev_eval::{
    runtime_plan_env_vars, runtime_plan_requires_warm, RuntimeDevEvalDecision, RuntimeDevEvalGate,
    RuntimeDevEvalProfile,
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
#[allow(deprecated)]
pub use theme_tokens::{
    is_font_scale_key, is_literal_color, is_literal_font_size, is_literal_gradient,
    validate_frame_token_refs, validate_panel_token_refs, validate_scene_theme_value_from_ops,
    validate_shell_theme_value, validate_theme_decl, validate_theme_layout_value,
    validate_theme_value_from_ops,
};
pub use typed_refs::{
    decode_binding_value, decode_ref_value, ref_to_json, BindingValue, RefExpr, RefKind,
    SceneLocator, SceneRegistry,
};
pub use warmup_manifest::{
    build_runtime_warmup_manifest, enrich_runtime_warmup_app, resolve_runtime_warmup_manifest,
    WORKSPACE_RUNTIME_WARMUP_MANIFEST_SCHEMA_VERSION,
};
pub use warmup_t2_page_autogen::{
    discover_board_warmup_suggestions, merge_workspace_and_board_warmup_requests,
    SuggestedWarmupDatasetRequest,
};
pub use workspace::{
    audit_component_preview_coverage, discover_apps, discover_apps_with_config,
    discover_build_apps, load_component_assets, read_source_file, source_tree,
};
pub use workspace_profile::{
    RuntimePlanAppDryRun, RuntimePlanReferenceCheck, WorkspaceProfileDocument,
    WorkspaceProfileDryRun, WorkspaceProfileError, WorkspaceProfileService,
    WorkspaceProfileSummary, WorkspaceProfileValidation, WorkspaceProfileValidationIssue,
};

/// Stable revision token derived from kernel sources at compile time.
pub fn platform_source_revision() -> &'static str {
    env!("MEI_KERNEL_SOURCE_REVISION")
}
