mod binaries;
mod descriptor;
mod doctor;
mod install;
mod io;
mod layout;
mod paths;
mod prelude;
mod render;
mod scaffold;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use binaries::*;
pub(crate) use install::*;
pub(crate) use io::*;
pub(crate) use layout::*;
pub(crate) use paths::*;
pub(crate) use render::*;

pub use descriptor::{
    doctor_editor_runtime_for_package_root, editor_runtime_descriptor_for_package_root,
    workspace_runtime_manifest_for_package_root, workspace_runtime_version_descriptor,
};
pub use doctor::{
    doctor_editor_runtime_for_workspace_root, workspace_runtime_status_for_workspace_root,
};
pub use install::{ensure_workspace_author_skill_package, install_editor_runtime_support_files};
pub use scaffold::scaffold_editor_runtime_tooling;
pub use types::*;
