mod prelude;
mod types;
mod profiles;
mod catalog;
mod mcp_surface;
mod access_mcp_surface;
mod access_host;

pub(crate) use mcp_surface::*;
pub(crate) use access_mcp_surface::*;

pub use types::*;
pub use profiles::{
    access_profile_descriptor, ai_profile_descriptor, ai_profile_policy_lines,
    author_profile_descriptor, meilang_access_skill_package, meilang_author_skill_package,
};
pub use catalog::{
    capability_catalog_descriptor, capability_catalog_descriptor_for_package_root,
    capability_catalog_descriptor_for_workspace_root,
};
pub use mcp_surface::{
    mcp_surface_descriptor, mcp_surface_descriptor_for_workspace_root,
};
pub use access_host::{
    access_host_bound_query_tools, access_host_bound_tool_descriptors,
    access_host_bound_tool_names,
};
