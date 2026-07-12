//! Shared host types for mei-host-shell and plugins.

mod app_runtime;
mod config;
mod context;
mod draft_session;
mod log_path;
mod plugin;
mod report;
mod slot;
mod workspace_stock;

pub use app_runtime::{
    host_control_path, instance_bootstrap_dir, instance_eval_cache_dir, instance_logs_dir,
    instance_meta_dir, instance_mrg_disk_dir, instance_mrg_memory_dir, instance_runtime_root,
    instance_spec_path, instance_var_dir, list_instance_runtime_ids, partition_cache_key,
    pinned_generation_root, read_host_control_state, read_instance_spec, write_host_control_state,
    write_if_revision_matches, write_instance_spec, ActiveProfileRef, AppRuntimeState,
    BuildAppArtifact, BuildPhaseReport, BuildRequest, BuildResult, BundleRef, CachePartitionKey,
    ConfigSnapshot, DesiredInstance, DesiredState, HostControlConflict, HostControlState,
    InstanceHealth, InstancePhase, InstanceResource, InstanceRevisions, InstanceSpec,
    LastSuccessfulApply, LaunchManifest, ObservedInstance, RouteBinding, RuntimeContext,
    HEADER_APP_ID, HEADER_GENERATION, HEADER_INSTANCE_ID, HEADER_INSTANCE_TOKEN, HEADER_PRINCIPAL,
    HEADER_SPEC_DIGEST, SCHEMA_BUILD_REQUEST_V1, SCHEMA_BUILD_RESULT_V1, SCHEMA_HOST_CONTROL_V1,
    SCHEMA_HOST_CONTROL_V2, SCHEMA_INSTANCE_SPEC_V1, SCHEMA_LAUNCH_MANIFEST_V1,
};
pub use config::{
    load_app_config, AppConfig, PlugEndpoint, PlugsSection, RuntimeSection, WarmupPolicyRef,
};
pub use context::{load_app_config_for_ctx, resolve_bundle_path, HostContext};
pub use draft_session::{resolve_draft_session_id, DRAFT_SESSION_COOKIE, DRAFT_SESSION_HEADER};
pub use log_path::{dir_tree_bytes, format_bytes_human, log_timestamp_rfc3339, path_for_log};
pub use plugin::{DsPlugin, MaterializeRequest, MaterializeResult, Plugin};
pub use report::ImportReport;
pub use slot::{CacheLayersReady, EvalSlotDescriptor};
pub use workspace_stock::{
    ensure_workspace_stock_materialized, materialize_workspace_stock, workspace_stock_revision,
    MaterializeDirReport, MaterializeReport,
};
