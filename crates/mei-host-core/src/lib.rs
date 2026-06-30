//! Shared host types for mei-host-shell and plugins.

mod config;
mod context;
mod plugin;
mod report;
mod slot;
mod workspace_stock;

pub use config::{load_app_config, AppConfig, PlugEndpoint, PlugsSection, RuntimeSection, WarmupPolicyRef};
pub use context::{load_app_config_for_ctx, resolve_bundle_path, HostContext};
pub use plugin::{DsPlugin, MaterializeRequest, MaterializeResult, Plugin};
pub use report::ImportReport;
pub use slot::{CacheLayersReady, EvalSlotDescriptor};
pub use workspace_stock::{
    ensure_workspace_stock_materialized, materialize_workspace_stock, workspace_stock_revision,
    MaterializeDirReport, MaterializeReport,
};
