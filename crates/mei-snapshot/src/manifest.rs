use serde::{Deserialize, Serialize};

pub const FORMAT_NAME: &str = "mei-snapshot";
pub const FORMAT_VERSION_V1: u32 = 1;
pub const FORMAT_VERSION_V2: u32 = 2;
/// Back-compat alias: legacy callers treated "current" as v1.
pub const FORMAT_VERSION: u32 = FORMAT_VERSION_V1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataModeHint {
    Eval,
    Fixture,
    Static,
}

impl DataModeHint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eval => "eval",
            Self::Fixture => "fixture",
            Self::Static => "static",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFileEntry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// Per-app entry inside a portable (v2) snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotAppEntry {
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_scene: Option<String>,
    pub data_mode_hint: DataModeHint,
    /// Path inside the zip, e.g. `apps/zhifa/exchange/zhifa.meibundle`.
    pub bundle_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    pub format: String,
    pub format_version: u32,
    /// Primary / first app id (v1 sole app; v2 primary for quick display).
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_scene: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    pub data_mode_hint: DataModeHint,
    pub created_at: String,
    pub files: Vec<ManifestFileEntry>,
    /// v2: multi-app collection. Empty for v1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<SnapshotAppEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_stock_revision: Option<String>,
    /// Relative path to resources.json when present (v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources_path: Option<String>,
}

impl SnapshotManifest {
    pub fn is_v2(&self) -> bool {
        self.format_version >= FORMAT_VERSION_V2
    }

    pub fn app_entries(&self) -> Vec<SnapshotAppEntry> {
        if !self.apps.is_empty() {
            return self.apps.clone();
        }
        // v1 synthetic entry
        let bundle_path = self
            .files
            .iter()
            .find(|f| f.path.starts_with("exchange/") && f.path.ends_with(".meibundle"))
            .map(|f| f.path.clone())
            .unwrap_or_else(|| format!("exchange/{}.meibundle", self.app_id));
        vec![SnapshotAppEntry {
            app_id: self.app_id.clone(),
            default_scene: self.default_scene.clone(),
            data_mode_hint: self.data_mode_hint,
            bundle_path,
            compiler_version: self.compiler_version.clone(),
        }]
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.format != FORMAT_NAME {
            anyhow::bail!(
                "unsupported snapshot format {:?}; expected {:?}",
                self.format,
                FORMAT_NAME
            );
        }
        if self.format_version != FORMAT_VERSION_V1 && self.format_version != FORMAT_VERSION_V2 {
            anyhow::bail!(
                "unsupported snapshot formatVersion {}; expected {} or {}",
                self.format_version,
                FORMAT_VERSION_V1,
                FORMAT_VERSION_V2
            );
        }
        if self.app_id.trim().is_empty() {
            anyhow::bail!("manifest.appId is empty");
        }
        if self.files.is_empty() {
            anyhow::bail!("manifest.files is empty");
        }

        if self.format_version == FORMAT_VERSION_V1 {
            let has_bundle = self
                .files
                .iter()
                .any(|f| f.path.starts_with("exchange/") && f.path.ends_with(".meibundle"));
            if !has_bundle {
                anyhow::bail!("manifest.files must include at least one exchange/*.meibundle");
            }
            if !self.apps.is_empty() {
                anyhow::bail!("v1 manifest must not include apps[]");
            }
        } else {
            if self.apps.is_empty() {
                anyhow::bail!("v2 manifest.apps must not be empty");
            }
            for app in &self.apps {
                if app.app_id.trim().is_empty() {
                    anyhow::bail!("v2 apps[].appId is empty");
                }
                if !app.bundle_path.ends_with(".meibundle") {
                    anyhow::bail!(
                        "v2 apps[{}].bundlePath must end with .meibundle",
                        app.app_id
                    );
                }
                let present = self.files.iter().any(|f| f.path == app.bundle_path);
                if !present {
                    anyhow::bail!(
                        "v2 apps[{}].bundlePath {} missing from files[]",
                        app.app_id,
                        app.bundle_path
                    );
                }
            }
        }
        Ok(())
    }
}
