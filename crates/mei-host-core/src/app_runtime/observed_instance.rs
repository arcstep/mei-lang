use serde::{Deserialize, Serialize};

use super::DesiredState;

/// Runtime-observed instance status. Not persisted into git-tracked state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedInstance {
    pub instance_id: String,
    pub spec_ref: String,
    pub observed_at_ms: u64,
    pub phase: InstancePhase,
    pub desired_state: DesiredState,
    pub reachable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Whether an instance token is present; never expose the token itself.
    pub token_present: bool,
    pub health: InstanceHealth,
    #[serde(default)]
    pub revisions: InstanceRevisions,
    #[serde(default)]
    pub protected_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default)]
    pub resource: InstanceResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstancePhase {
    Queued,
    Building,
    Launching,
    Importing,
    Snapshotting,
    Warming,
    Ready,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceHealth {
    pub process: String,
    /// plug-ds / registry readiness as a simple status string.
    #[serde(rename = "plugDs")]
    pub plug_ds: String,
    pub warmup: String,
    pub bootstrap: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRevisions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_generation: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceResource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn observed_instance_roundtrip() {
        let observed = ObservedInstance {
            instance_id: "inst-1".to_string(),
            spec_ref: "sha256:abc".to_string(),
            observed_at_ms: 42,
            phase: InstancePhase::Ready,
            desired_state: DesiredState::Running,
            reachable: true,
            endpoint: Some("http://127.0.0.1:9".to_string()),
            token_present: true,
            health: InstanceHealth {
                process: "ok".to_string(),
                plug_ds: "ok".to_string(),
                warmup: "ready".to_string(),
                bootstrap: "ok".to_string(),
            },
            revisions: InstanceRevisions {
                registry_revision: Some("reg-1".to_string()),
                client_revision: None,
                data_generation: Some("WS-1".to_string()),
            },
            protected_reasons: vec!["active-route".to_string()],
            last_error: None,
            resource: InstanceResource {
                rss_bytes: Some(1024),
                generation: Some("WS-1".to_string()),
            },
        };
        let value = serde_json::to_value(&observed).expect("serialize");
        assert_eq!(
            value.get("health").and_then(|h| h.get("plugDs")),
            Some(&json!("ok"))
        );
        let back: ObservedInstance = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, observed);
    }
}
