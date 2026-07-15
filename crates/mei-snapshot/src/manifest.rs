use serde::{Deserialize, Serialize};

pub const FORMAT_NAME: &str = "mei-snapshot";
pub const FORMAT_VERSION: u32 = 1;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    pub format: String,
    pub format_version: u32,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_scene: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    pub data_mode_hint: DataModeHint,
    pub created_at: String,
    pub files: Vec<ManifestFileEntry>,
}

impl SnapshotManifest {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.format != FORMAT_NAME {
            anyhow::bail!(
                "unsupported snapshot format {:?}; expected {:?}",
                self.format,
                FORMAT_NAME
            );
        }
        if self.format_version != FORMAT_VERSION {
            anyhow::bail!(
                "unsupported snapshot formatVersion {}; expected {}",
                self.format_version,
                FORMAT_VERSION
            );
        }
        if self.app_id.trim().is_empty() {
            anyhow::bail!("manifest.appId is empty");
        }
        if self.files.is_empty() {
            anyhow::bail!("manifest.files is empty");
        }
        let has_bundle = self
            .files
            .iter()
            .any(|f| f.path.starts_with("exchange/") && f.path.ends_with(".meibundle"));
        if !has_bundle {
            anyhow::bail!("manifest.files must include at least one exchange/*.meibundle");
        }
        Ok(())
    }
}
