//! 宿主认证写操作审计 journal（workspace 级，避免记录敏感明文）。

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mei_config::{
    AUTH_JOURNAL_REL_PATH, LEGACY_AUTH_JOURNAL_REL_PATH, PRE_LOCAL_AUTH_JOURNAL_REL_PATH,
};

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
        let _ = migrate_legacy_auth_journal(source_root);
        let path = resolve_auth_journal_read_path(source_root);
        if !path.is_file() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, source_root: &Path) -> Result<()> {
        migrate_legacy_auth_journal(source_root)?;
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

fn legacy_auth_journal_paths(source_root: &Path) -> [PathBuf; 2] {
    [
        source_root.join(LEGACY_AUTH_JOURNAL_REL_PATH),
        source_root.join(PRE_LOCAL_AUTH_JOURNAL_REL_PATH),
    ]
}

fn resolve_auth_journal_read_path(source_root: &Path) -> PathBuf {
    let modern = auth_journal_path(source_root);
    if modern.is_file() {
        return modern;
    }
    for legacy in legacy_auth_journal_paths(source_root) {
        if legacy.is_file() {
            return legacy;
        }
    }
    modern
}

fn migrate_legacy_auth_journal(source_root: &Path) -> Result<()> {
    let modern = auth_journal_path(source_root);
    if modern.exists() {
        return Ok(());
    }
    for legacy in legacy_auth_journal_paths(source_root) {
        if !legacy.is_file() {
            continue;
        }
        if let Some(parent) = modern.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create auth dir {}", parent.display()))?;
        }
        move_file(&legacy, &modern)?;
        if let Some(parent) = legacy.parent() {
            let _ = fs::remove_dir(parent);
        }
        return Ok(());
    }
    Ok(())
}

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            fs::copy(source, destination).with_context(|| {
                format!(
                    "failed to copy auth journal {} -> {} after rename error: {rename_error}",
                    source.display(),
                    destination.display()
                )
            })?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove legacy auth journal {}", source.display()))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let unique = format!(
                "{prefix}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn load_migrates_previous_local_auth_journal_path() {
        let temp = TempDirGuard::new("mei-auth-journal");
        let legacy = temp.path.join(LEGACY_AUTH_JOURNAL_REL_PATH);
        fs::create_dir_all(legacy.parent().expect("legacy parent")).expect("create legacy parent");
        fs::write(
            &legacy,
            r#"{"schemaVersion":1,"revision":3,"entries":[{"revision":3,"action":"bootstrap","actor":"tester","at_ms":1,"summary":"ok","patch":{}}]}"#,
        )
        .expect("write legacy journal");

        let journal = AuthJournal::load(&temp.path);

        assert_eq!(journal.revision, 3);
        assert_eq!(journal.entries.len(), 1);
        assert!(temp.path.join(AUTH_JOURNAL_REL_PATH).is_file());
        assert!(!legacy.exists());
    }
}
