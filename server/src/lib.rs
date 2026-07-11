pub(crate) mod agent_runtime;
pub(crate) mod auth;
pub(crate) mod block;
pub(crate) mod build_info;
pub(crate) mod cli;
pub(crate) mod diagnostics;
pub(crate) mod gis_config;
pub(crate) mod graph;
pub(crate) mod http;
pub(crate) mod mei_agent;
pub(crate) mod prebuild;
pub(crate) mod prebuild_fingerprint;
pub(crate) mod readiness;
pub(crate) mod resource_tool_bridge;
mod runtime_entry;

pub use block::{block_eval, materialize_worksets, BlockEvalRequest};
pub use prebuild::PrebuildMode;
#[cfg(test)]
pub(crate) use runtime_entry::test_support;
pub use runtime_entry::{run_cli_for_flavor, BinaryFlavor};
pub(crate) use runtime_entry::{AppError, AppState, SessionContextSnapshot};
