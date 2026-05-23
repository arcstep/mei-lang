use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::resource::ResourceDecl;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDecl {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub spawns: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub flags: Value,
    /// Authoring-only：`entity(base = entity_ref(...))` 克隆源；编译归一后清除。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldCellDecl {
    pub id: String,
    #[serde(default)]
    pub row: Option<u32>,
    #[serde(default)]
    pub col: Option<u32>,
    #[serde(default)]
    pub surface_kind: Option<String>,
    #[serde(default)]
    pub flammable: Option<bool>,
    #[serde(default)]
    pub walkable: Option<bool>,
    #[serde(default)]
    pub occupiable: Option<bool>,
    #[serde(default)]
    pub capacity: Option<u32>,
    #[serde(default)]
    pub hazard_state: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldDecl {
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub topology: Option<WorldGridDecl>,
    #[serde(default)]
    pub resources: Vec<ResourceDecl>,
    /// 一等 dataset 集合（`world(datasets=[...])` / `world.add_dataset(...)` 账本归一）。
    #[serde(default)]
    pub datasets: Vec<ResourceDecl>,
    /// 一等 world metric 集合（`world(metrics=[...])` / `world.add_metric(...)` 账本归一）。
    #[serde(default)]
    pub metrics: Vec<Value>,
    /// 一等 metric_pack 集合（兼容旧写法）。
    #[serde(default)]
    pub metric_packs: Vec<ResourceDecl>,
    #[serde(default)]
    pub entities: Vec<EntityDecl>,
    /// Authoring-only：`world(base = world_ref(...))` 克隆源；编译归一后清除。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldGridDecl {
    pub rows: u32,
    pub cols: u32,
    #[serde(default)]
    pub cells: Vec<WorldCellDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleStartDecl {
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub action_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleRequireDecl {
    #[serde(rename = "type")]
    pub require_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEffectDecl {
    #[serde(rename = "type")]
    pub effect_type: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub effects: Vec<RuleEffectDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleClickDecl {
    pub target: String,
    #[serde(default)]
    pub require: Option<RuleRequireDecl>,
    pub effect: RuleEffectDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTimerDecl {
    pub seconds: u32,
    pub on_timeout: RuleEffectDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSubjectTimerDecl {
    #[serde(default)]
    pub id: Option<String>,
    pub subject_ref: String,
    #[serde(rename = "type")]
    pub timer_kind: String,
    pub delay_seconds: f64,
    #[serde(default)]
    pub interval_seconds: Option<f64>,
    #[serde(default)]
    pub repeat: bool,
    pub on_timeout: RuleEffectDecl,
    #[serde(default)]
    pub cancel_when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleOutcomeDecl {
    #[serde(default)]
    pub success: Option<String>,
    #[serde(default)]
    pub fail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowDecl {
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub start: Option<RuleStartDecl>,
    #[serde(default)]
    pub interactions: Vec<RuleClickDecl>,
    #[serde(default)]
    pub timer: Option<RuleTimerDecl>,
    #[serde(default)]
    pub subject_timers: Vec<RuleSubjectTimerDecl>,
    #[serde(default)]
    pub outcome: Option<RuleOutcomeDecl>,
    /// Authoring-only：`flow(base = flow_ref(...))` 克隆源；编译归一后清除。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
}
