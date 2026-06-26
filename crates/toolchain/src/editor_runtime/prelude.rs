//! Shared imports for editor_runtime submodules.

pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::process::Command;

pub(crate) use anyhow::{Context, Result};
pub(crate) use chrono::{SecondsFormat, Utc};
pub(crate) use serde_json::Value;
pub(crate) use walkdir::WalkDir;

pub(crate) use mei_lang_kernel::{
    apply_toolchain_store_symlinks, build_runtime_warmup_manifest, record_toolchain_install_links,
    resolve_toolchain_root, resolve_workspace_runtime_root, toolchain_store_dir,
    RuntimeWarmupManifest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};

pub(crate) use crate::capability_catalog::CAPABILITY_CATALOG_SCHEMA_VERSION;
pub(crate) use crate::{knowledge_bundle::package_root_hint, knowledge_bundle_descriptor_for_package_root};

pub(crate) use super::types::*;
