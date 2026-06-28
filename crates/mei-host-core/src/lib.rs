//! Shared host types for mei-host-shell and plugins.

mod config;
mod context;
mod plugin;
mod report;
mod slot;

pub use config::{load_app_config, AppConfig, PlugEndpoint, PlugsSection, RuntimeSection, WarmupPolicyRef};
pub use context::{load_app_config_for_ctx, resolve_bundle_path, HostContext};
pub use plugin::{DsPlugin, MaterializeRequest, MaterializeResult, Plugin};
pub use report::ImportReport;
pub use slot::{CacheLayersReady, EvalSlotDescriptor};
