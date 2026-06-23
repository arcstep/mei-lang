#![recursion_limit = "256"]

mod access_query;
mod analysis_contract;
mod artifact_store;
mod capability_catalog;
mod compile_report;
mod compile_service;
mod editor_runtime;
mod export;
mod knowledge_bundle;
mod observation;
mod platform_assets;
mod publish_data_snapshots;
mod runtime_sim;
mod semantic_summary;
mod types;
mod workspace_stock;
mod workspace_summary;
mod world;

pub use access_query::{
    query_world_dataset, query_world_dataset_metrics, RESOURCE_QUERY_SCHEMA_VERSION,
};
pub use artifact_store::{
    toolchain_artifact_store_root, ArtifactStoreManifest, ArtifactStoreWriteResult,
    ArtifactWriteContext, TOOLCHAIN_ARTIFACT_STORE_VERSION,
};
pub use capability_catalog::{
    access_host_bound_query_tools, access_host_bound_tool_descriptors,
    access_host_bound_tool_names, access_profile_descriptor, ai_profile_descriptor,
    ai_profile_policy_lines, author_profile_descriptor, capability_catalog_descriptor,
    capability_catalog_descriptor_for_package_root, capability_catalog_descriptor_for_workspace_root,
    mcp_surface_descriptor, mcp_surface_descriptor_for_workspace_root, meilang_access_skill_package,
    meilang_author_skill_package, AiProfileDescriptor, SkillPackageDescriptor,
    CAPABILITY_CATALOG_SCHEMA_VERSION, MCP_SURFACE_SCHEMA_VERSION,
};
pub use compile_report::{compile_report, CompileReport};
pub use compile_service::{
    clear_compile_cache_for_app, clear_compiled_app_artifacts_for_app, compile_app_with_cache,
    compile_app_with_cache_shared, compile_cache_key, env_flag_enabled, inspect_source_layout, is_compile_inflight,
    load_compile_artifact_only, load_compile_artifact_only_shared, peek_compile_cache,
    peek_compile_cache_hit, peek_compile_cache_hit_shared, peek_compile_cache_shared,
    recent_compile_failure, resolve_components_root, CompileWithCacheFailure,
    CompileWithCacheOutcome, CompileWithCacheOutcomeShared, LayoutCheck, PeekCompileCacheHit,
    PeekCompileCacheHitShared, SourceLayoutInspection, SourceLayoutRoots,
};
pub use editor_runtime::{
    doctor_editor_runtime_for_package_root, doctor_editor_runtime_for_workspace_root,
    editor_runtime_descriptor_for_package_root, install_editor_runtime_support_files,
    scaffold_editor_runtime_tooling, workspace_runtime_manifest_for_package_root,
    workspace_runtime_status_for_workspace_root, workspace_runtime_version_descriptor,
    EditorRuntimeCheck, EditorRuntimeDescriptor, EditorRuntimeDoctorReport,
    EditorRuntimeInstallReport, EditorRuntimePathDescriptor, EditorRuntimeScaffoldFile,
    EditorRuntimeScaffoldReport, EditorRuntimeTemplateDescriptor, InstalledRuntimeDescriptor,
    RuntimeCompatibilityDescriptor, RuntimeManifestArtifactDescriptor,
    RuntimeManifestContentDescriptor, RuntimeManifestProvenance, RuntimeSourceRevision,
    WorkspaceRuntimeManifest, WorkspaceRuntimeStatusReport, WorkspaceRuntimeVersionDescriptor,
    EDITOR_RUNTIME_SCHEMA_VERSION, RUNTIME_BUNDLE_SCHEMA_VERSION,
    WORKSPACE_RUNTIME_MANIFEST_SCHEMA_VERSION, WORKSPACE_RUNTIME_VERSION_SCHEMA_VERSION,
};
pub use export::{
    build_eval_plan_markdown, export_analysis_contracts, export_eval_plan,
    export_inventory_snapshot, export_runtime_trace, export_semantic_dag,
    format_eval_plan_markdown, format_semantic_graph_markdown, HeadlessArtifactEnvelope,
    HeadlessArtifactKind, HeadlessExportOptions, HEADLESS_EXPORT_SCHEMA_VERSION,
};
pub use knowledge_bundle::{
    export_knowledge_bundle_for_package_root, export_knowledge_bundle_for_workspace_root,
    knowledge_bundle_descriptor_for_package_root, KnowledgeAssetContent,
    KnowledgeAssetDescriptor, KnowledgeBundleDescriptor, KNOWLEDGE_BUNDLE_SCHEMA_VERSION,
};
pub use observation::{CompileObservation, EvalObservation, ExposureManifest};
pub use platform_assets::{
    platform_asset_catalog_descriptor, platform_asset_catalog_descriptor_for_package_root,
    ComponentExportDescriptor, ComponentPackDescriptor, PlatformAssetCatalogDescriptor,
    TemplatePackDescriptor, PLATFORM_ASSET_SCHEMA_VERSION,
};
pub use publish_data_snapshots::{publish_data_snapshots, PublishDataSnapshotsReport};
pub use runtime_sim::{runtime_sim_step, RuntimeSimStepResult};
pub use types::{
    ResourceInventoryItem, ResourceInventorySnapshot, ResourceQueryToolSpec, WorkspaceAppSummary,
    WorkspaceSummary, WorldAssetGetResponse, WorldAssetListItem, WorldAssetListResponse,
    WorldBusinessEntitySummary, WorldBusinessResourceSummary, WorldBusinessSummary,
    WorldContextSnapshot, WorldRuntimeBundle, WorldRuntimePeekResponse, WorldRuntimeSummary,
    WorldScope, WorldSnapshotSummary,
};
pub use workspace_stock::{
    create_app_skeleton, init_workspace_profile, materialize_workspace_stock, MaterializeReport,
};
pub use workspace_summary::build_workspace_summary;
pub use world::{
    build_world_business_summary, build_world_context_snapshot, default_resource_query_tools,
    load_world_runtime_bundle, query_world_asset, query_world_assets, query_world_runtime,
};
