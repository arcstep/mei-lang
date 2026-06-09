pub(crate) mod agent_runtime;
pub(crate) mod auth;
pub(crate) mod build_info;
pub(crate) mod cli;
pub(crate) mod gis_config;
pub(crate) mod http;
pub(crate) mod mei_agent;
pub(crate) mod resource_tool_bridge;
mod runtime_entry;

pub use runtime_entry::{run_cli_for_flavor, BinaryFlavor};
pub(crate) use runtime_entry::{AppError, AppState, SessionContextSnapshot};
#[cfg(test)]
pub(crate) use runtime_entry::test_support;
