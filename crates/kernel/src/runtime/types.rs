use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeClockState {
    #[serde(default)]
    pub current_time: f64,
    #[serde(default = "default_time_unit")]
    pub time_unit: String,
    #[serde(default)]
    pub paused: bool,
    #[serde(default = "default_time_rate")]
    pub rate: f64,
    #[serde(default)]
    pub countdown_remaining: f64,
}

impl Default for RuntimeClockState {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            time_unit: default_time_unit(),
            paused: false,
            rate: default_time_rate(),
            countdown_remaining: 0.0,
        }
    }
}

fn default_time_unit() -> String {
    "second".to_string()
}

fn default_time_rate() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeIntent {
    pub kind: String,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTraceItem {
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub seed: u64,
    pub phase: String,
    pub result: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub countdown: i64,
    #[serde(default)]
    pub clock: RuntimeClockState,
    #[serde(default)]
    pub placements: BTreeMap<String, String>,
    #[serde(default)]
    pub inventory: Vec<String>,
    #[serde(default)]
    pub statuses: BTreeMap<String, String>,
    #[serde(default)]
    pub flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub timeline: Vec<String>,
    #[serde(default)]
    pub trace_events: Vec<RuntimeTraceItem>,
    #[serde(default)]
    pub subject_timers: Vec<RuntimeSubjectTimerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEntityView {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub flags: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSubjectTimerState {
    pub id: String,
    pub subject_ref: String,
    pub timer_kind: String,
    pub started_at: f64,
    pub due_at: f64,
    #[serde(default)]
    pub interval: Option<f64>,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default)]
    pub cancel_when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCellView {
    pub id: String,
    #[serde(default)]
    pub surface_kind: Option<String>,
    #[serde(default)]
    pub flammable: Option<bool>,
    #[serde(default)]
    pub walkable: Option<bool>,
    #[serde(default)]
    pub occupiable: Option<bool>,
    #[serde(default)]
    pub hazard_state: Option<String>,
    #[serde(default)]
    pub hazard_timer_remaining: Option<f64>,
    #[serde(default)]
    pub hazard_timer_seconds: Option<i64>,
    #[serde(default)]
    pub interaction_target: Option<String>,
    #[serde(default)]
    pub clickable: bool,
    #[serde(default)]
    pub key_target: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entities: Vec<RuntimeEntityView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSceneView {
    pub scene_id: String,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub phase: String,
    pub result: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub outcome_state: String,
    #[serde(default)]
    pub outcome_message: Option<String>,
    pub countdown: i64,
    #[serde(default)]
    pub current_time: f64,
    #[serde(default = "default_time_unit")]
    pub time_unit: String,
    #[serde(default)]
    pub clock_paused: bool,
    #[serde(default = "default_time_rate")]
    pub time_rate: f64,
    #[serde(default)]
    pub inventory: Vec<String>,
    #[serde(default)]
    pub entities: Vec<RuntimeEntityView>,
    #[serde(default)]
    pub cells: Vec<RuntimeCellView>,
    #[serde(default)]
    pub subject_timers: Vec<RuntimeSubjectTimerState>,
    #[serde(default)]
    pub available_actions: Vec<String>,
    #[serde(default)]
    pub start_label: Option<String>,
}
