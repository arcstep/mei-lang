//! Per-app App Runtime process: embedded DS + Access thin shell + view/eval data plane.

mod access;
mod auth;
mod cli;
mod host_data;
pub mod http;
mod lifecycle;
mod serve;
mod state;

pub use cli::{Cli, Command, ServeArgs};
pub use http::{registered_route_paths, router};
pub use serve::run_serve;
pub use state::{AppRuntimeServeState, ReadySnapshot, SharedRuntimeState};
