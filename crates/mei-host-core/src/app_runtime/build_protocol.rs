//! Build Worker request/result protocol (Host ↔ one-shot build subprocess).

use serde::{Deserialize, Serialize};

pub const SCHEMA_BUILD_REQUEST_V1: &str = "mei-build-request-v1";
pub const SCHEMA_BUILD_RESULT_V1: &str = "mei-build-result-v1";

/// One-shot Build Worker input. Host writes this to a temp file and spawns the worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildRequest {
    pub schema_version: String,
    pub profile_id: String,
    pub profile_revision: String,
    pub profile_file: String,
    pub apps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain_hint: Option<String>,
    /// Reserved: optional compile scope filter (worker may ignore until implemented).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile_scope: Option<String>,
    /// When set, attach this generation instead of allocating a new one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired_generation: Option<String>,
}

impl BuildRequest {
    pub fn new(
        profile_id: impl Into<String>,
        profile_revision: impl Into<String>,
        profile_file: impl Into<String>,
        apps: Vec<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_BUILD_REQUEST_V1.to_string(),
            profile_id: profile_id.into(),
            profile_revision: profile_revision.into(),
            profile_file: profile_file.into(),
            apps,
            toolchain_hint: None,
            compile_scope: None,
            desired_generation: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_BUILD_REQUEST_V1 {
            return Err(format!(
                "unsupported BuildRequest schemaVersion: {}",
                self.schema_version
            ));
        }
        if self.profile_id.trim().is_empty() {
            return Err("profileId must be non-empty".to_string());
        }
        if self.profile_revision.trim().is_empty() {
            return Err("profileRevision must be non-empty".to_string());
        }
        if self.apps.is_empty() {
            return Err("apps must be non-empty".to_string());
        }
        Ok(())
    }
}

/// Per-app artifact summary returned by the Build Worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildAppArtifact {
    pub app_id: String,
    pub bundle_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_digest: Option<String>,
}

/// Timed phase report for Host ops aggregation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildPhaseReport {
    pub name: String,
    pub ok: bool,
    pub ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Build Worker stdout / `--output` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildResult {
    pub schema_version: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    #[serde(default)]
    pub apps: Vec<BuildAppArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub phases: Vec<BuildPhaseReport>,
}

impl BuildResult {
    pub fn success(generation: impl Into<String>, apps: Vec<BuildAppArtifact>) -> Self {
        Self {
            schema_version: SCHEMA_BUILD_RESULT_V1.to_string(),
            ok: true,
            generation: Some(generation.into()),
            apps,
            error: None,
            phases: Vec::new(),
        }
    }

    pub fn failure(error: impl Into<String>, phases: Vec<BuildPhaseReport>) -> Self {
        Self {
            schema_version: SCHEMA_BUILD_RESULT_V1.to_string(),
            ok: false,
            generation: None,
            apps: Vec::new(),
            error: Some(error.into()),
            phases,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_request_roundtrip() {
        let request = BuildRequest {
            schema_version: SCHEMA_BUILD_REQUEST_V1.to_string(),
            profile_id: "local".to_string(),
            profile_revision: "r1".to_string(),
            profile_file: "configs/local.json".to_string(),
            apps: vec!["mini-data".to_string()],
            toolchain_hint: None,
            compile_scope: None,
            desired_generation: None,
        };
        let value = serde_json::to_value(&request).expect("serialize");
        assert_eq!(value["schemaVersion"], SCHEMA_BUILD_REQUEST_V1);
        let back: BuildRequest = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, request);
        assert!(back.validate().is_ok());
    }

    #[test]
    fn build_request_denies_unknown_fields() {
        let err = serde_json::from_value::<BuildRequest>(json!({
            "schemaVersion": SCHEMA_BUILD_REQUEST_V1,
            "profileId": "local",
            "profileRevision": "r1",
            "profileFile": "configs/local.json",
            "apps": ["mini-data"],
            "unexpected": true
        }));
        assert!(err.is_err());
    }

    #[test]
    fn build_result_roundtrip() {
        let result = BuildResult {
            schema_version: SCHEMA_BUILD_RESULT_V1.to_string(),
            ok: true,
            generation: Some("WS-20260712.1".to_string()),
            apps: vec![BuildAppArtifact {
                app_id: "mini-data".to_string(),
                bundle_path: "apps/mini-data/env/WS-20260712.1".to_string(),
                digest: Some("abc".to_string()),
                config_digest: Some("r1".to_string()),
            }],
            error: None,
            phases: vec![BuildPhaseReport {
                name: "compiling".to_string(),
                ok: true,
                ms: 12,
                message: None,
            }],
        };
        let value = serde_json::to_value(&result).expect("serialize");
        let back: BuildResult = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, result);
    }

    #[test]
    fn build_result_failure_shape() {
        let result = BuildResult::failure("compile failed", Vec::new());
        assert!(!result.ok);
        assert_eq!(result.error.as_deref(), Some("compile failed"));
        assert!(result.generation.is_none());
    }
}
