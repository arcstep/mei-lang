//! Shared imports for host_api submodules.
pub(crate) use std::collections::{BTreeMap, BTreeSet};
pub(crate) use std::io::{IsTerminal, Write};
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::{Mutex, OnceLock};
pub(crate) use std::time::Instant;
pub(crate) use anyhow::{anyhow, Result};
pub(crate) use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use crate::{
    http::compile_cache::{
        compile_app_with_cache, compile_outcome_from_shared, resolve_runtime_compile_shared,
        CompileWithCacheOutcome, RuntimeAccessPolicies,
    },
    http::startup_run,
    prebuild::{
        run_prebuild, PrebuildAppReport, PrebuildDiagnosticsReport,
        PrebuildMode, PrebuildOptions, PrebuildReport, PrebuildScopeProfile, PrebuildWarningReport,
    },
    AppState,
};
pub(crate) use mei_lang_kernel::{
    CompileOptions, CompiledApp, Severity,
};
pub(crate) use mei_lang_toolchain::resolve_components_root;
