use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryDecl {
    pub id: Option<String>,
    pub frame: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDecl {
    pub kind: String,
    pub id: String,
    pub title: Option<String>,
    #[serde(default)]
    pub entries: Vec<EntryDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutDecl {
    #[serde(rename = "type")]
    pub layout_type: String,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub columns: Option<Vec<String>>,
    #[serde(default)]
    pub rows: Option<Vec<String>>,
    #[serde(default)]
    pub areas: Option<Vec<Vec<String>>>,
    #[serde(default)]
    pub gap: Option<String>,
    #[serde(default)]
    pub padding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameDecl {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub layout: Option<LayoutDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDecl {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub source: SourceDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetSourceDecl {
    pub source_kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDecl {
    pub kind: String,
    pub use_key: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub data_ref: Option<String>,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub scene_kind: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub start_label: Option<String>,
}

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldDecl {
    pub kind: String,
    #[serde(default)]
    pub grid: Option<WorldGridDecl>,
    #[serde(default)]
    pub entities: Vec<EntityDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldGridDecl {
    pub rows: u32,
    pub cols: u32,
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
pub struct RuleOutcomeDecl {
    #[serde(default)]
    pub success: Option<String>,
    #[serde(default)]
    pub fail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesDecl {
    pub kind: String,
    #[serde(default)]
    pub start: Option<RuleStartDecl>,
    #[serde(default)]
    pub interactions: Vec<RuleClickDecl>,
    #[serde(default)]
    pub timer: Option<RuleTimerDecl>,
    #[serde(default)]
    pub outcome: Option<RuleOutcomeDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub area: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneContract {
    pub scene: SceneDecl,
    #[serde(default)]
    pub world: Option<WorldDecl>,
    #[serde(default)]
    pub rules: Option<RulesDecl>,
    #[serde(default)]
    pub frame: Option<FrameDecl>,
    #[serde(default)]
    pub panels: Vec<PanelDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetView {
    pub id: String,
    pub title: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub source: DatasetSourceDecl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentAsset {
    pub key: String,
    pub tag: String,
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceAppMeta {
    pub id: String,
    pub title: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceNode {
    pub name: String,
    pub path: String,
    pub kind: String,
    #[serde(default)]
    pub children: Vec<WorkspaceNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledApp {
    pub app_id: String,
    pub title: String,
    pub app_root: String,
    pub entry_target: String,
    pub file_tree: Vec<WorkspaceNode>,
    #[serde(default)]
    pub frame: Option<FrameDecl>,
    #[serde(default)]
    pub blocks: Vec<BlockDecl>,
    #[serde(default)]
    pub datasets: Vec<DatasetView>,
    #[serde(default)]
    pub scene_contract: Option<SceneContract>,
    #[serde(default)]
    pub component_assets: Vec<ComponentAsset>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

