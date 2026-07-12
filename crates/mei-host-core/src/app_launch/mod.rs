//! App Launch Config: per-app launch files under `apps/{app}/launch/*.json`.
//!
//! Disk files are the sole source of truth for how an app runs. CLI only selects
//! which file to use — never overrides fields.

mod paths;
mod service;
mod types;

pub use paths::{
    app_launch_dir, app_runtime_root, default_launch_path, ensure_app_launch_dir,
    launch_config_path, resolve_launch_path,
};
pub use service::{
    ensure_default_launch_config, list_launch_configs, read_launch_config, resolve_default_launch,
    write_launch_config, AppLaunchError,
};
pub use types::{AppLaunchConfig, AppLaunchDocument, AppLaunchSummary, SCHEMA_APP_LAUNCH_V1};
