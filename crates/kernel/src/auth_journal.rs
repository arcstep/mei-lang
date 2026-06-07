//! 宿主认证写操作审计 journal（workspace 级，避免记录敏感明文）。

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mei_config::AUTH_JOURNAL_REL_PATH;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthJournal {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub entries: Vec<AuthJournalEntry>,
}

impl Default for AuthJournal {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthJournalEntry {
    pub revision: u64,
    pub action: String,
    pub actor: String,
    pub at_ms: u128,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub patch: Value,
}

impl AuthJournal {
    pub fn load(source_root: &Path) -> Self {
        let path = auth_journal_path(source_root);
        if !path.is_file() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, source_root: &Path) -> Result<()> {
        let path = auth_journal_path(source_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create auth journal dir {}", parent.display())
            })?;
        }
        let raw = serde_json::to_string_pretty(self).context("failed to serialize auth journal")?;
        fs::write(&path, raw)
            .with_context(|| format!("failed to write auth journal {}", path.display()))
    }

    pub fn append(
        &mut self,
        action: impl Into<String>,
        actor: impl Into<String>,
        summary: impl Into<String>,
        patch: Value,
    ) -> AuthJournalEntry {
        self.revision = self.revision.saturating_add(1);
        let entry = AuthJournalEntry {
            revision: self.revision,
            action: action.into(),
            actor: actor.into(),
            at_ms: unix_timestamp_ms(),
            summary: summary.into(),
            patch,
        };
        self.entries.push(entry.clone());
        entry
    }
}

pub fn auth_journal_path(source_root: &Path) -> PathBuf {
    source_root.join(AUTH_JOURNAL_REL_PATH)
}

pub fn append_auth_journal_entry(
    source_root: &Path,
    action: &str,
    actor: &str,
    summary: &str,
    patch: Value,
) -> Result<AuthJournalEntry> {
    let mut journal = AuthJournal::load(source_root);
    let entry = journal.append(action, actor, summary, patch);
    journal.save(source_root)?;
    Ok(entry)
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
