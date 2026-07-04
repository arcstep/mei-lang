//! Shared host types for mei-host-shell and plugins.

mod config;
mod context;
mod draft_session;
mod layout_tuning_draft;
mod log_path;
mod plugin;
mod report;
mod slot;
mod workspace_stock;

pub use config::{load_app_config, AppConfig, PlugEndpoint, PlugsSection, RuntimeSection, WarmupPolicyRef};
pub use context::{load_app_config_for_ctx, resolve_bundle_path, HostContext};
pub use draft_session::{
    layout_tuning_draft_storage_key, resolve_draft_session_id, DRAFT_SESSION_COOKIE,
    DRAFT_SESSION_HEADER,
};
pub use layout_tuning_draft::{
    layout_tuning_draft, merge_layout_tuning_overlay, set_layout_tuning_draft,
};
pub use log_path::{dir_tree_bytes, format_bytes_human, log_timestamp_rfc3339, path_for_log};
pub use plugin::{DsPlugin, MaterializeRequest, MaterializeResult, Plugin};
pub use report::ImportReport;
pub use slot::{CacheLayersReady, EvalSlotDescriptor};
pub use workspace_stock::{
    ensure_workspace_stock_materialized, materialize_workspace_stock, workspace_stock_revision,
    MaterializeDirReport, MaterializeReport,
};
