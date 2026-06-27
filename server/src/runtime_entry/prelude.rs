//! Shared imports for runtime_entry submodules.
pub(crate) use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
pub(crate) use anyhow::{Context, Result};
pub(crate) use axum::{
    body::Body,
    http::StatusCode,
    http::{HeaderName, HeaderValue, Method, Request, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
pub(crate) use clap::Parser;
pub(crate) use http_body_util::BodyExt;
pub(crate) use mei_lang_kernel::{set_mei_package_root, HostSurface};
pub(crate) use tracing::Instrument;
pub(crate) use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub(crate) use crate::cli::args::{AgentRuntimeArgs, Cli, Command, HostCommand, ServeArgs};
pub(crate) use crate::cli::util::{
    print_cli_version_if_requested, resolve_cargo_package_root, resolve_cli_source_root,
    resolve_package_root, resolve_source_root_arg,
};
pub(crate) use crate::cli::{
    agent_command, block_command, compile_or_check_command, diagnostics_command,
    editor_runtime_command, export_command, graph_command, host_command, inspect_command,
    knowledge_command, layer_command, mcp_command, prebuild_command, query_command,
    readiness_command, runtime_command, scope_command, warmup_command, workspace_command,
};
