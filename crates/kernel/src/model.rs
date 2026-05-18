use std::collections::BTreeMap;

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
pub struct AppDecl {
    pub kind: String,
    pub id: String,
    pub title: Option<String>,
    #[serde(default)]
    pub default_scene: Option<String>,
    #[serde(default)]
    pub scene: Option<Value>,
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
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub layout: Option<LayoutDecl>,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDecl {
    #[serde(alias = "__source")]
    pub kind: String,
    #[serde(default)]
    #[serde(alias = "file")]
    pub path: String,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default)]
    pub header_row: Option<i64>,
    #[serde(default)]
    pub preview_rows: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
    #[serde(default)]
    pub max_page_size: Option<i64>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub connection: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDecl {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub source: Option<SourceDecl>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub dataset: Option<Value>,
    #[serde(default)]
    pub metrics: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub filters: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDecl {
    #[serde(default = "default_block_kind")]
    pub kind: String,
    pub use_key: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UiNodeDecl {
    Panel(PanelDecl),
    Block(BlockDecl),
}

fn default_block_kind() -> String {
    "block".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub world: Option<Value>,
    #[serde(default)]
    pub flow: Option<String>,
    #[serde(default)]
    pub frame: Option<Value>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub state: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub frame: Value,
    #[serde(default)]
    pub panel: Value,
    #[serde(default)]
    pub panel_bare: Value,
    #[serde(default)]
    pub heading: Value,
    #[serde(default)]
    pub font: Value,
    #[serde(default)]
    pub tokens: Value,
    /// 组件级默认配置（如 `dataset_table.cell_preview_max_chars`），由预览 `_mei.components` 下发。
    #[serde(default)]
    pub components: Value,
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
    #[serde(default)]
    pub entities: Vec<EntityDecl>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub layout: Option<LayoutDecl>,
    #[serde(default)]
    pub blocks: Vec<UiNodeDecl>,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneContract {
    pub scene: SceneDecl,
    #[serde(default)]
    pub themes: Vec<ThemeDecl>,
    #[serde(default)]
    pub world: Option<WorldDecl>,
    #[serde(default)]
    pub flow: Option<FlowDecl>,
    #[serde(default)]
    pub frame: Option<FrameDecl>,
    #[serde(default)]
    pub panels: Vec<PanelDecl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetView {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub stage_schema: Vec<ColumnSchema>,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub source: SourceDecl,
    #[serde(default)]
    pub sources: Vec<DatasetSourceRef>,
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricContract>,
    #[serde(skip, default)]
    pub runtime_metric_defs: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricShape {
    Scalar,
    Series,
    Table,
    Dataframe,
}

fn default_metric_shape() -> MetricShape {
    MetricShape::Dataframe
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricContract {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default = "default_metric_shape")]
    pub shape: MetricShape,
    #[serde(default)]
    pub schema: Vec<ColumnSchema>,
    #[serde(default)]
    pub dataset: Option<String>,
    #[serde(default)]
    pub transforms: Vec<DataTransform>,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetSourceRef {
    pub id: String,
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTransform {
    #[serde(rename = "type")]
    pub transform_type: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRef {
    pub id: String,
    #[serde(default)]
    pub from_dataset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRef {
    pub id: String,
    #[serde(default)]
    pub from_dataset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPackContract {
    pub id: String,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedResource {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub document: Option<String>,
    #[serde(default)]
    pub dataset: Option<DatasetView>,
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
    /// `.mei` 语义：`main` / `scene` / `mei`（普通脚本）；非 mei 文件为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mei_kind: Option<String>,
    #[serde(default)]
    pub children: Vec<WorkspaceNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompiledSceneRoute {
    pub scene_id: String,
    #[serde(default)]
    pub frame_id: Option<String>,
    pub target_file: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledApp {
    pub app_id: String,
    pub title: String,
    pub app_root: String,
    #[serde(default)]
    pub scene_routes: Vec<CompiledSceneRoute>,
    #[serde(default)]
    pub active_scene: Option<String>,
    pub active_target_file: String,
    pub file_tree: Vec<WorkspaceNode>,
    #[serde(default)]
    pub scene_contract: Option<SceneContract>,
    #[serde(default)]
    pub resources: Vec<LoadedResource>,
    #[serde(default)]
    pub component_assets: Vec<ComponentAsset>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}
