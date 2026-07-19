//! Host Admin entries use the same v2 discovery path as app-authored entries.
//!
//! There is deliberately no Rust `AdminResourceSpec` fallback. A Host-owned
//! authoring root can be discovered by the caller and merged as a normal
//! `AdminRegistryProjection`.

use mei_lang_kernel::{AdminRegistryProjection, ADMIN_RESOURCE_API_VERSION};

pub fn merge_host_builtins(
    app_id: &str,
    projection: Option<AdminRegistryProjection>,
) -> AdminRegistryProjection {
    projection.unwrap_or_else(|| AdminRegistryProjection {
        app_id: app_id.to_string(),
        api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
        admin_registry_digest: String::new(),
        page_structure_digest: String::new(),
        resources: Vec::new(),
    })
}
