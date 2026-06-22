mod asset_shell;
mod diagnostics;

pub(crate) use asset_shell::is_static_workspace_asset_target;
pub(crate) use diagnostics::{
    blocking_errors_for_preview, is_world_capsule_target, normalize_diagnostic_source,
    normalize_target_path, world_capsule_companion_scene,
};
