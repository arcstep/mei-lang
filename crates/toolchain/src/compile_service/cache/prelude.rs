//! Shared imports for compile cache submodules.

pub(crate) use std::collections::{BTreeMap, HashMap};
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::sync::Arc;
pub(crate) use std::time::{Instant, UNIX_EPOCH};

pub(crate) use anyhow;
pub(crate) use mei_lang_kernel::{
    compile_app_with_options, compile_app_with_options_and_revision, resolve_app_root, CompileOptions, CompileWatchedFile, CompiledApp, COMPILE_SEMANTICS_GENERATION,
};
pub(crate) use mei_lang_kernel::resolve_components_root as kernel_resolve_components_root;
pub(crate) use serde_json::Value;

pub(crate) use crate::artifact_store::{
    compiled_app_manifest_identity, read_artifact_manifest, read_json_artifact,
    write_json_artifact, ArtifactStoreManifest, ArtifactWatchedFile, ArtifactWriteContext,
};
pub(crate) use crate::types::WorldScope;

pub(crate) use super::access_slim::{
    access_slim_artifacts_enabled, canonical_artifact_persist_enabled, content_store_preferred,
    should_persist_compiled_app_artifact, slim_compiled_app_for_access,
    strip_loaded_compiled_app_for_access,
};

pub(crate) use super::types::*;
pub(crate) use super::singleflight::env_flag_enabled;
