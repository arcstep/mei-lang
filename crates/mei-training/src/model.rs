use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEDULER_ID: &str = "sm2-v0";
pub const LEARNER_SCHEMA_VERSION: u32 = 2;

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

/// Per-item mastery ladder (difficulty). Orthogonal to Pack curriculum scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LadderStage {
    #[default]
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl LadderStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "l0",
            Self::L1 => "l1",
            Self::L2 => "l2",
            Self::L3 => "l3",
            Self::L4 => "l4",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::L0 => 0,
            Self::L1 => 1,
            Self::L2 => 2,
            Self::L3 => 3,
            Self::L4 => 4,
        }
    }

    pub fn at_least(self, other: Self) -> bool {
        self.rank() >= other.rank()
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "l0" | "0" => Some(Self::L0),
            "l1" | "1" => Some(Self::L1),
            "l2" | "2" => Some(Self::L2),
            "l3" | "3" => Some(Self::L3),
            "l4" | "4" => Some(Self::L4),
            _ => None,
        }
    }
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
    #[serde(default)]
    pub ladder_stage: LadderStage,
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
            ladder_stage: LadderStage::L0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerStateFile {
    pub schema_version: u32,
    pub items: BTreeMap<String, LearnerItemState>,
    /// Calendar day (UTC `YYYY-MM-DD`) for which `new_introduced_today` applies.
    #[serde(default)]
    pub new_day: String,
    #[serde(default)]
    pub new_introduced_today: u32,
    /// Legacy intro pool (`d1`/`d2`); retained for migration only.
    #[serde(default = "default_char_pool")]
    pub char_pool: String,
    /// Currently unlocked curriculum packs (new may only come from these).
    #[serde(default)]
    pub open_packs: Vec<String>,
    #[serde(default)]
    pub active_pack_id: String,
    /// Matrix focus: which pack to introduce from / prefer for practice.
    #[serde(default = "default_focus_pack")]
    pub focus_pack_id: String,
    /// Matrix focus: L1/L2/L3 target for this session.
    #[serde(default = "default_focus_target")]
    pub focus_target: LadderStage,
}

fn default_char_pool() -> String {
    "d1".to_string()
}

fn default_focus_pack() -> String {
    "pack-a".to_string()
}

fn default_focus_target() -> LadderStage {
    LadderStage::L1
}

impl LearnerStateFile {
    pub fn new() -> Self {
        Self {
            schema_version: LEARNER_SCHEMA_VERSION,
            items: BTreeMap::new(),
            new_day: String::new(),
            new_introduced_today: 0,
            char_pool: default_char_pool(),
            open_packs: vec!["pack-a".into()],
            active_pack_id: "pack-a".into(),
            focus_pack_id: default_focus_pack(),
            focus_target: default_focus_target(),
        }
    }
}

impl Default for LearnerStateFile {
    fn default() -> Self {
        Self::new()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ladder_after: Option<String>,
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
            schema_version: LEARNER_SCHEMA_VERSION,
            scheduler: SCHEDULER_ID.to_string(),
            app_id: app_id.to_string(),
            learner_id: learner_id.to_string(),
            updated_at: Utc::now(),
        }
    }
}
