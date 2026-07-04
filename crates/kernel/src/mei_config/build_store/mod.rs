mod types;
mod prebuild_override;
mod build_generation;
mod env_paths;
mod paths;
mod lifecycle;
mod migrate;
mod env_clean;

#[cfg(test)]
mod tests;

pub use build_generation::{
    resolve_build_generation_for_prebuild, resolve_version_display_identity,
    resolve_version_display_identity_for_app, resolve_version_display_identity_with_hint,
    BuildGenerationSpec, VersionDisplayIdentity,
};
pub use env_clean::{
    clean_env_generations, migrate_apps_to_env_layout, migrate_build_var_store_to_env,
    resolve_build_footer_label, resolve_build_footer_label_with_hint,
    resolve_workspace_footer_label, resolve_workspace_footer_label_with_hint,
    CleanEnvPolicy, CleanEnvReport, MigrateEnvReport,
};
pub use env_paths::*;
pub use types::*;
pub use prebuild_override::{
    clear_prebuild_build_root_override, restore_prebuild_build_root_override,
    set_prebuild_build_root_override, snapshot_prebuild_build_root_override,
};
pub use paths::*;
pub use lifecycle::*;
pub use migrate::*;
