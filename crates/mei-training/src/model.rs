use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEDULER_ID: &str = "sm2-v0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemPhase {
    New,
    Learning,
    Review,
    Relearning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerItemState {
    pub phase: ItemPhase,
    /// Unix millis UTC.
    pub due_at: i64,
    pub interval_days: f64,
    pub ease: f64,
    pub learning_step: u32,
    pub reps: u32,
    pub lapses: u32,
    pub introduced: bool,
    #[serde(default)]
    pub suspended: bool,
}

impl LearnerItemState {
    pub fn fresh_unintroduced() -> Self {
        Self {
            phase: ItemPhase::New,
            due_at: 0,
            interval_days: 0.0,
            ease: 2.5,
            learning_step: 0,
            reps: 0,
            lapses: 0,
            introduced: false,
            suspended: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearnerStateFile {
    pub schema_version: u32,
    pub items: BTreeMap<String, LearnerItemState>,
    /// Calendar day (UTC `YYYY-MM-DD`) for which `new_introduced_today` applies.
    #[serde(default)]
    pub new_day: String,
    #[serde(default)]
    pub new_introduced_today: u32,
    /// Open intro pool for char-to-code: `d1` or `d2`.
    #[serde(default = "default_char_pool")]
    pub char_pool: String,
}

fn default_char_pool() -> String {
    "d1".to_string()
}

impl LearnerStateFile {
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            items: BTreeMap::new(),
            new_day: String::new(),
            new_introduced_today: 0,
            char_pool: default_char_pool(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLogEntry {
    pub ts: DateTime<Utc>,
    pub item_id: String,
    pub learner_id: String,
    pub rating: Rating,
    pub correct: bool,
    pub latency_ms: u64,
    pub phase_before: ItemPhase,
    pub phase_after: ItemPhase,
    pub due_before: i64,
    pub due_after: i64,
    pub scheduler: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFile {
    pub schema_version: u32,
    pub scheduler: String,
    pub app_id: String,
    pub learner_id: String,
    pub updated_at: DateTime<Utc>,
}

impl MetaFile {
    pub fn new(app_id: &str, learner_id: &str) -> Self {
        Self {
            schema_version: 1,
            scheduler: SCHEDULER_ID.to_string(),
            app_id: app_id.to_string(),
            learner_id: learner_id.to_string(),
            updated_at: Utc::now(),
        }
    }
}
