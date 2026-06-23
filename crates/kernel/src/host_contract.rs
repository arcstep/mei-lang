use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const HOST_RUNTIME_PROTOCOL_SCHEMA: &str = "mei-host-runtime-protocol-v1";
pub const HOST_RUNTIME_CONTRACT_SCHEMA: &str = "mei-host-runtime-contract-v1";
pub const HOST_REQUIREMENTS_SCHEMA: &str = "mei-host-requirements-v1";

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionKind {
    RuntimeCapability,
    Callback,
    Projection,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HostExtensionDescriptor {
    pub id: &'static str,
    pub kind: HostExtensionKind,
    pub description: &'static str,
    pub consumer: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_auth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_browser_context: Option<bool>,
}

const HOST_EXTENSION_REGISTRY: [HostExtensionDescriptor; 6] = [
    HostExtensionDescriptor {
        id: "rows_query(scene_qualified)",
        kind: HostExtensionKind::RuntimeCapability,
        description: "Scene-qualified dataset row query exposed by the host runtime.",
        consumer: "mei-host-web",
        requires_auth: None,
        requires_browser_context: Some(true),
    },
    HostExtensionDescriptor {
        id: "metric_query(scene_qualified)",
        kind: HostExtensionKind::RuntimeCapability,
        description: "Scene-qualified aggregate metric query exposed by the host runtime.",
        consumer: "mei-host-web",
        requires_auth: None,
        requires_browser_context: Some(true),
    },
    HostExtensionDescriptor {
        id: "metric_batch_query(scene_qualified)",
        kind: HostExtensionKind::RuntimeCapability,
        description: "Scene-qualified batch aggregate metric query exposed by the host runtime.",
        consumer: "mei-host-web",
        requires_auth: None,
        requires_browser_context: Some(true),
    },
    HostExtensionDescriptor {
        id: "browser_context_binding",
        kind: HostExtensionKind::Callback,
        description:
            "Merge browser query_state / tab hints into bounded eval scope before answering.",
        consumer: "mei-host-web",
        requires_auth: None,
        requires_browser_context: Some(true),
    },
    HostExtensionDescriptor {
        id: "resource_visibility_binding",
        kind: HostExtensionKind::Callback,
        description: "Enforce route/resource visibility before exposing access-side query tools.",
        consumer: "mei-host-web",
        requires_auth: Some(true),
        requires_browser_context: None,
    },
    HostExtensionDescriptor {
        id: "presentation_projection",
        kind: HostExtensionKind::Projection,
        description:
            "Access-like presentation shell over exported scene routes inside mei-host-web.",
        consumer: "mei-host-web",
        requires_auth: Some(true),
        requires_browser_context: None,
    },
];

pub fn host_extension_registry() -> &'static [HostExtensionDescriptor] {
    &HOST_EXTENSION_REGISTRY
}

pub fn host_runtime_capabilities_catalog() -> Vec<&'static str> {
    HOST_EXTENSION_REGISTRY
        .iter()
        .filter(|item| item.kind == HostExtensionKind::RuntimeCapability)
        .map(|item| item.id)
        .collect()
}

pub fn host_extension_registry_descriptor() -> Value {
    json!({
        "schema_version": HOST_REQUIREMENTS_SCHEMA,
        "extensions": HOST_EXTENSION_REGISTRY,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct HostRequirementsDescriptor {
    pub schema_version: &'static str,
    pub consumer_id: String,
    pub host_surface: String,
    pub auth_required: bool,
    pub browser_context_required: bool,
    pub projections: Vec<String>,
    pub registered_extensions: Vec<&'static str>,
}

pub fn host_requirements_for_consumer(consumer_id: &str) -> Option<HostRequirementsDescriptor> {
    let normalized = consumer_id.trim();
    if normalized != "mei-host-web" {
        return None;
    }
    let registered_extensions = HOST_EXTENSION_REGISTRY
        .iter()
        .filter(|item| item.consumer == normalized)
        .map(|item| item.id)
        .collect::<Vec<_>>();
    Some(HostRequirementsDescriptor {
        schema_version: HOST_REQUIREMENTS_SCHEMA,
        consumer_id: normalized.to_string(),
        host_surface: HostSurface::AuthoringHost.as_slug().to_string(),
        auth_required: true,
        browser_context_required: true,
        projections: vec!["access_shell".to_string(), "presentation_shell".to_string()],
        registered_extensions,
    })
}

pub fn host_requirements_descriptor(consumer_id: &str) -> Option<Value> {
    host_requirements_for_consumer(consumer_id).map(|item| {
        json!({
            "schema_version": item.schema_version,
            "consumer_id": item.consumer_id,
            "host_surface": item.host_surface,
            "auth_required": item.auth_required,
            "browser_context_required": item.browser_context_required,
            "projections": item.projections,
            "registered_extensions": item.registered_extensions,
        })
    })
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
        "runtime_capabilities": host_runtime_capabilities_catalog(),
        "registered_extensions": host_extension_registry_descriptor(),
        "errors": {
            "access_scene_path_invalid": 404,
            "access_scene_not_found": 404,
            "access_scene_not_exported": 403
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        host_extension_registry, host_requirements_for_consumer, host_runtime_capabilities_catalog,
        host_runtime_contract_descriptor, HostExtensionKind, HostSurface,
    };

    #[test]
    fn host_runtime_contract_descriptor_contains_core_fields() {
        let v = host_runtime_contract_descriptor();
        assert_eq!(v["protocol_schema"], "mei-host-runtime-protocol-v1");
        assert_eq!(v["host_replaceability"]["non_replaceable"][0], "compile");
        assert_eq!(v["errors"]["access_scene_not_exported"], 403);
        assert!(v["registered_extensions"]["extensions"].is_array());
    }

    #[test]
    fn host_runtime_capabilities_catalog_is_non_empty() {
        let caps = host_runtime_capabilities_catalog();
        assert!(caps.contains(&"rows_query(scene_qualified)"));
    }

    #[test]
    fn host_extension_registry_contains_runtime_and_callback_entries() {
        let registry = host_extension_registry();
        assert!(registry
            .iter()
            .any(|item| item.kind == HostExtensionKind::RuntimeCapability));
        assert!(registry
            .iter()
            .any(|item| item.kind == HostExtensionKind::Callback));
        assert!(registry
            .iter()
            .any(|item| item.id == "presentation_projection"));
    }

    #[test]
    fn host_requirements_for_web_consumer_lists_projections() {
        let requirements = host_requirements_for_consumer("mei-host-web").expect("requirements");
        assert!(requirements
            .projections
            .contains(&"presentation_shell".to_string()));
        assert!(!requirements.registered_extensions.is_empty());
    }

    #[test]
    fn host_surface_from_flag_maps_access_only() {
        assert_eq!(
            HostSurface::from_host_surface_flag("access-only"),
            HostSurface::AccessOnlyHost
        );
    }
}
