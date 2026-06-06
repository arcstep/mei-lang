//! 宿主 ops 写操作审计 journal（revision / rollback 接缝）。

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mei_config::{write_mei_config, MeiConfig, OPS_JOURNAL_REL_PATH};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpsJournal {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub entries: Vec<OpsJournalEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsJournalEntry {
    pub revision: u64,
    pub action: String,
    pub actor: String,
    pub at_ms: u128,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub patch: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_of: Option<u64>,
}

impl OpsJournal {
    pub fn load(app_root: &Path) -> Self {
        let path = journal_path(app_root);
        if !path.is_file() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app_root: &Path) -> Result<()> {
        let path = journal_path(app_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create ops journal dir {}", parent.display())
            })?;
        }
        let raw = serde_json::to_string_pretty(self).context("failed to serialize ops journal")?;
        fs::write(&path, raw)
            .with_context(|| format!("failed to write ops journal {}", path.display()))
    }

    pub fn append(
        &mut self,
        action: impl Into<String>,
        actor: impl Into<String>,
        summary: impl Into<String>,
        patch: Value,
        rollback_of: Option<u64>,
    ) -> OpsJournalEntry {
        self.revision = self.revision.saturating_add(1);
        let entry = OpsJournalEntry {
            revision: self.revision,
            action: action.into(),
            actor: actor.into(),
            at_ms: unix_timestamp_ms(),
            summary: summary.into(),
            patch,
            rollback_of,
        };
        self.entries.push(entry.clone());
        entry
    }
}

pub fn journal_path(app_root: &Path) -> PathBuf {
    app_root.join(OPS_JOURNAL_REL_PATH)
}

pub fn apply_ops_patch_with_journal(
    app_root: &Path,
    config_path: &Path,
    actor: &str,
    summary: &str,
    patch: &crate::mei_config::OpsConfigPatch,
) -> Result<(MeiConfig, OpsJournalEntry)> {
    let mut config = MeiConfig::load_or_default(config_path);
    crate::mei_config::merge_ops_section(&mut config, patch)?;
    write_mei_config(config_path, &config)?;
    let mut journal = OpsJournal::load(app_root);
    let entry = journal.append(
        "ops.patch",
        actor,
        summary,
        serde_json::to_value(patch).unwrap_or(Value::Null),
        None,
    );
    journal.save(app_root)?;
    Ok((config, entry))
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
