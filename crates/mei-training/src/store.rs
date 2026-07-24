use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use chrono::Utc;
use thiserror::Error;

use crate::model::{LearnerStateFile, MetaFile, ReviewLogEntry};
use crate::paths::training_learner_dir;

static LEARNER_LOCKS: OnceLock<Mutex<std::collections::HashMap<String, ()>>> = OnceLock::new();

fn learner_locks() -> &'static Mutex<std::collections::HashMap<String, ()>> {
    LEARNER_LOCKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub struct LearnerStore {
    pub dir: PathBuf,
    pub app_id: String,
    pub learner_id: String,
}

impl LearnerStore {
    pub fn open(workspace: &Path, app_id: &str, learner_id: &str) -> Self {
        Self {
            dir: training_learner_dir(workspace, app_id, learner_id),
            app_id: app_id.to_string(),
            learner_id: learner_id.to_string(),
        }
    }

    fn lock_key(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    pub fn with_lock<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let mut map = learner_locks().lock().expect("learner locks");
        map.entry(self.lock_key()).or_insert(());
        // Hold map lock for the duration — coarse but correct for single-host ASAP.
        f()
    }

    pub fn state_path(&self) -> PathBuf {
        self.dir.join("learner-state.json")
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join("review-log.jsonl")
    }

    pub fn meta_path(&self) -> PathBuf {
        self.dir.join("meta.json")
    }

    pub fn load_state(&self) -> Result<LearnerStateFile> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(LearnerStateFile::new());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save_state(&self, state: &LearnerStateFile) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("create {}", self.dir.display()))?;
        write_json_atomically(&self.state_path(), state)?;
        let mut meta = MetaFile::new(&self.app_id, &self.learner_id);
        meta.updated_at = Utc::now();
        write_json_atomically(&self.meta_path(), &meta)?;
        Ok(())
    }

    pub fn append_log(&self, entry: &ReviewLogEntry) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("create {}", self.dir.display()))?;
        let line = serde_json::to_string(entry).context("serialize ReviewLogEntry")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .with_context(|| format!("open {}", self.log_path().display()))?;
        writeln!(file, "{line}").context("append review log")?;
        Ok(())
    }
}

fn write_json_atomically<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value).context("serialize json")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ItemPhase, Rating, SCHEDULER_ID};
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_state_and_log() {
        let dir = tempdir().unwrap();
        let store = LearnerStore::open(dir.path(), "wubi", "alice");
        let mut state = LearnerStateFile::new();
        state.items.insert(
            "char:国".into(),
            crate::model::LearnerItemState::fresh_unintroduced(),
        );
        store.save_state(&state).unwrap();
        let loaded = store.load_state().unwrap();
        assert!(loaded.items.contains_key("char:国"));

        let entry = ReviewLogEntry {
            ts: Utc::now(),
            item_id: "char:国".into(),
            learner_id: "alice".into(),
            rating: Rating::Good,
            correct: true,
            latency_ms: 1200,
            phase_before: ItemPhase::Learning,
            phase_after: ItemPhase::Learning,
            due_before: 0,
            due_after: 1,
            scheduler: SCHEDULER_ID.into(),
            mode: Some("char_to_code".into()),
        };
        store.append_log(&entry).unwrap();
        let log = fs::read_to_string(store.log_path()).unwrap();
        assert!(log.contains("char:国"));
    }
}
