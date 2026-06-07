use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const HOST_RUNTIME_PROTOCOL_SCHEMA: &str = "mei-host-runtime-protocol-v1";
pub const HOST_RUNTIME_CONTRACT_SCHEMA: &str = "mei-host-runtime-contract-v1";

const HOST_RUNTIME_CAPABILITIES: [&str; 2] =
    ["rows_query(scene_qualified)", "metric_query(scene_qualified)"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostSurface {
    AuthoringHost,
    AccessHost,
    AccessOnlyHost,
}

impl HostSurface {
    pub fn as_slug(self) -> &'static str {
        match self {
            HostSurface::AuthoringHost => "authoring_host",
            HostSurface::AccessHost => "access_host",
            HostSurface::AccessOnlyHost => "access_only_host",
        }
    }

    pub fn from_host_surface_flag(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "access-only" | "access_only" | "access" => HostSurface::AccessOnlyHost,
            "access_host" => HostSurface::AccessHost,
            _ => HostSurface::AuthoringHost,
        }
    }
}

pub fn host_runtime_capabilities_catalog() -> &'static [&'static str] {
    &HOST_RUNTIME_CAPABILITIES
}

pub fn host_protocol_descriptor(surface: HostSurface, route_mode: &str, mode: &str) -> Value {
    json!({
        "schema": HOST_RUNTIME_PROTOCOL_SCHEMA,
        "surface": surface.as_slug(),
        "route_mode": route_mode,
        "mode": mode,
    })
}

pub fn host_runtime_contract_descriptor() -> Value {
    json!({
        "schema_version": HOST_RUNTIME_CONTRACT_SCHEMA,
        "protocol_schema": HOST_RUNTIME_PROTOCOL_SCHEMA,
        "host_replaceability": {
            "replaceable": [
                "host_coordinates",
                "route_mode",
                "mode_policy_binding",
                "resource_visibility_binding",
                "browser_context_binding",
                "runtime_capabilities_exposure",
                "access_error_semantics"
            ],
            "non_replaceable": [
                "compile",
                "lowering",
                "scene_world_contract",
                "runtime_objects"
            ]
        },
        "runtime_capabilities": HOST_RUNTIME_CAPABILITIES,
        "errors": {
            "access_scene_path_invalid": 404,
            "access_scene_not_found": 404,
            "access_scene_not_exported": 403
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{host_runtime_contract_descriptor, host_runtime_capabilities_catalog, HostSurface};

    #[test]
    fn host_runtime_contract_descriptor_contains_core_fields() {
        let v = host_runtime_contract_descriptor();
        assert_eq!(v["protocol_schema"], "mei-host-runtime-protocol-v1");
        assert_eq!(
            v["host_replaceability"]["non_replaceable"][0],
            "compile"
        );
        assert_eq!(v["errors"]["access_scene_not_exported"], 403);
    }

    #[test]
    fn host_runtime_capabilities_catalog_is_non_empty() {
        let caps = host_runtime_capabilities_catalog();
        assert!(caps.contains(&"rows_query(scene_qualified)"));
    }

    #[test]
    fn host_surface_from_flag_maps_access_only() {
        assert_eq!(
            HostSurface::from_host_surface_flag("access-only"),
            HostSurface::AccessOnlyHost
        );
    }
}
