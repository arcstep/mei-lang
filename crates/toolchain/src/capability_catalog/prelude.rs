//! Shared imports for capability_catalog submodules.

pub(crate) use std::path::Path;

pub(crate) use mei_lang_kernel::{
    host_extension_registry_descriptor, host_requirements_descriptor,
    host_runtime_capabilities_catalog, host_runtime_contract_descriptor,
};
pub(crate) use serde_json::{json, Value};

pub(crate) use crate::knowledge_bundle::knowledge_bundle_descriptor_for_package_root;
pub(crate) use crate::platform_assets::{
    platform_asset_catalog_descriptor_for_package_root,
    platform_asset_catalog_descriptor_for_workspace_root,
};
pub(crate) use crate::types::ResourceQueryToolSpec;

pub(crate) use super::types::*;
