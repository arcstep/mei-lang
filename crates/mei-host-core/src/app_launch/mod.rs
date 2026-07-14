//! App Launch Config: single file `apps/{app}/launch.json` (Phase 8.5).
//!
//! Disk file is the sole Git-true source for how an app runs. Ephemeral
//! hot/lazy/frozen overlays never write back to this file.

mod paths;
mod service;
mod types;

pub use paths::{
    app_launch_dir, app_runtime_root, default_launch_path, ensure_app_launch_dir,
    launch_config_path, launch_json_path, resolve_launch_path,
};
pub use service::{
    ensure_default_launch_config, list_launch_configs, read_launch_config, resolve_default_launch,
    write_launch_config, AppLaunchError,
};
pub use types::{AppLaunchConfig, AppLaunchDocument, AppLaunchSummary, SCHEMA_APP_LAUNCH_V1};
