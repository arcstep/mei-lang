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
    #[serde(default)]
    pub align: Option<String>,
    #[serde(default)]
    pub justify: Option<String>,
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
    /// Authoring-only：`frame(base = frame_ref(...))` 克隆源；编译归一后清除。
    #[serde(default)]
    pub base: Option<Value>,
    /// Owner 槽位：`frame(panels=[panel_ref(...), panel(...)])` 归一后的 panel 集合。
    #[serde(default)]
    pub panels: Vec<Value>,
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
    /// Authoring-only：`resource(base = *_ref(...))` 克隆源；编译归一后清除。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
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
    /// Authoring-only：`component(base = component_ref(...))` 克隆源；编译归一后清除。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Value>,
    #[serde(default)]
    pub layout: Option<LayoutDecl>,
    #[serde(default)]
    pub blocks: Vec<Value>,
    #[serde(default)]
    pub component: Option<Value>,
    #[serde(default)]
    pub placement: Option<Value>,
    #[serde(default)]
    pub interactions: Vec<Value>,
    #[serde(default)]
    pub lifecycle: Option<Value>,
    #[serde(default)]
    pub constraints: Option<Value>,
    #[serde(default)]
    pub data: Option<Value>,
}

/// Legacy block embed IR (panel_ref+area removed); kept for serde compat and error surfacing only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelRefEmbedDecl {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub area: Option<String>,
    pub scene_file: String,
    #[serde(default)]
    pub render_policy: Option<String>,
    #[serde(default)]
    pub data: Option<Value>,
    /// Set when decoding legacy `panel_capsule_ref` / `frame_ref` block embed shapes.
    #[serde(default, skip_serializing)]
    pub compat_source: Option<String>,
}

#[derive(Debug, Clone)]
pub enum UiNodeDecl {
    Panel(PanelDecl),
    Block(BlockDecl),
    PanelRefEmbed(PanelRefEmbedDecl),
}

impl Serialize for UiNodeDecl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            UiNodeDecl::Panel(panel) => panel.serialize(serializer),
            UiNodeDecl::Block(block) => block.serialize(serializer),
            UiNodeDecl::PanelRefEmbed(embed) => {
                let component = serde_json::json!({
                    "id": embed.id,
                    "title": embed.title,
                    "area": embed.area,
                    "block_kind": "panel_ref",
                    "scene_file": embed.scene_file,
                    "render_policy": embed.render_policy,
                    "data": embed.data,
                });
                serde_json::json!({ "component": component }).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for UiNodeDecl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        deserialize_ui_node_value(value).map_err(serde::de::Error::custom)
    }
}

pub fn deserialize_ui_node_value(value: Value) -> Result<UiNodeDecl, String> {
    if value.get("kind").and_then(Value::as_str) == Some("panel") {
        return serde_json::from_value::<PanelDecl>(value)
            .map(UiNodeDecl::Panel)
            .map_err(|error| error.to_string());
    }
    if value.get("use_key").is_some() || value.get("kind").and_then(Value::as_str) == Some("block") {
        return serde_json::from_value::<BlockDecl>(value)
            .map(UiNodeDecl::Block)
            .map_err(|error| error.to_string());
    }
    if let Some(component) = value.get("component") {
        let block_kind = component.get("block_kind").and_then(Value::as_str);
        if block_kind == Some("panel_ref") {
            return Err(
                "panel_ref_embed_removed: panel_ref only references external panels in frame.panels; \
                 block embed with `area` is no longer supported"
                    .to_string(),
            );
        }
        if matches!(block_kind, Some("panel_capsule_ref") | Some("frame_ref")) {
            let compat_source = match block_kind {
                Some("panel_capsule_ref") => Some("panel_capsule_ref".to_string()),
                Some("frame_ref") => Some("frame_ref".to_string()),
                _ => None,
            };
            let scene_file = component
                .get("scene_file")
                .or_else(|| component.get("frame_ref"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    "legacy panel embed missing component.scene_file path".to_string()
                })?;
            return Ok(UiNodeDecl::PanelRefEmbed(PanelRefEmbedDecl {
                id: component
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                title: component
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                area: component
                    .get("area")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                scene_file: scene_file.to_string(),
                render_policy: component
                    .get("render_policy")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                data: component.get("data").cloned(),
                compat_source,
            }));
        }
    }
    Err("data did not match any variant of untagged enum UiNodeDecl".to_string())
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
    pub flow: Option<Value>,
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
    /// Access 态是否允许导出该 scene（默认 true，保持兼容）。
    #[serde(default = "default_access_export")]
    pub access_export: bool,
}

fn default_access_export() -> bool {
    true
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
    /// 一等 metric_pack 集合。
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
    /// Authoring-only：`panel(base = panel_ref(...))` 克隆源；编译归一后清除。
    #[serde(default)]
    pub base: Option<Value>,
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
    /// `ds.scalar_map(..., unit = "...")` 等声明的展示单位，供指标卡等 UI 与数值分列展示。
    #[serde(default)]
    pub unit: Option<String>,
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
    #[serde(default = "default_access_export")]
    pub access_export: bool,
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
