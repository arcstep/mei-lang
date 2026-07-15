use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::layout::LayoutDecl;
use super::ui_node::UiNodeDecl;

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
pub enum UiTreeNode {
    Panel(UiNodeDecl),
    Block(BlockDecl),
    PanelRefEmbed(PanelRefEmbedDecl),
}

impl Serialize for UiTreeNode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            UiTreeNode::Panel(panel) => panel.serialize(serializer),
            UiTreeNode::Block(block) => block.serialize(serializer),
            UiTreeNode::PanelRefEmbed(embed) => {
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

impl<'de> Deserialize<'de> for UiTreeNode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        deserialize_ui_node_value(value).map_err(serde::de::Error::custom)
    }
}

pub fn deserialize_ui_node_value(value: Value) -> Result<UiTreeNode, String> {
    if value.get("kind").and_then(Value::as_str) == Some("panel") {
        return serde_json::from_value::<UiNodeDecl>(value)
            .map(UiTreeNode::Panel)
            .map_err(|error| error.to_string());
    }
    if value.get("use_key").is_some() || value.get("kind").and_then(Value::as_str) == Some("block")
    {
        return serde_json::from_value::<BlockDecl>(value)
            .map(UiTreeNode::Block)
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
            return Ok(UiTreeNode::PanelRefEmbed(PanelRefEmbedDecl {
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
    Err("data did not match any variant of untagged enum UiTreeNode".to_string())
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
    /// Scene 级只读共享参数；供 theme/components/props 通过 `shared_ref(...)` 消费。
    #[serde(default)]
    pub shared: Value,
    /// Scene 级局部导航约定（如 tabs/sub-nav），由宿主按 scene contract 消费。
    #[serde(default)]
    pub local_nav: Value,
    /// Scene 级显式输入声明；供 caller 通过 link.params / route params 传值。
    /// `accepts` 作为作者态别名，强调“参数化目标”的心智。
    #[serde(default, alias = "accepts")]
    pub params: Value,
    /// Scene / board 的可选能力声明；用于表达运行时能力而非强制作者分型。
    #[serde(default)]
    pub capabilities: Value,
    /// Openable T2 page-plane ids from `scene.t2_pages` (0335); not always-on panels.
    #[serde(default)]
    pub t2_pages: Vec<String>,
    /// Scene 级装配默认绑定；用于把可复用 scene 壳接到本地 world 资源。
    #[serde(default)]
    pub bindings: Value,
    /// 参数化 scene 的预览示例；Manage 可在无外部 caller 时选用示例装配。
    #[serde(default)]
    pub examples: Value,
    /// Access 态是否允许导出该 scene（默认 true，保持兼容）。
    #[serde(default = "default_access_export")]
    pub access_export: bool,
}

fn default_access_export() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneExportDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub scene: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameExportDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub frame: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelExportDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub panel: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentExportDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub block: Value,
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
    pub panel_head: Value,
    #[serde(default)]
    pub panel_body: Value,
    /// 兼容：合并时迁入 `panel_head`。
    #[serde(default)]
    pub heading: Value,
    #[serde(default)]
    pub font: Value,
    #[serde(default)]
    pub metric_label: Value,
    #[serde(default)]
    pub metric_value: Value,
    #[serde(default)]
    pub metric_unit: Value,
    #[serde(default)]
    pub metric_desc: Value,
    #[serde(default)]
    pub metric_sub_label: Value,
    #[serde(default)]
    pub metric_sub_value: Value,
    #[serde(default)]
    pub metric_sub_unit: Value,
    #[serde(default)]
    pub chart_title: Value,
    #[serde(default)]
    pub chart_label: Value,
    #[serde(default)]
    pub table_head: Value,
    #[serde(default)]
    pub table_body: Value,
    #[serde(default)]
    pub filter_panel: Value,
    #[serde(default)]
    pub tokens: Value,
    /// 与 CSS tokens 分轨的只读共享参数默认值。
    #[serde(default)]
    pub shared: Value,
    /// 组件级默认配置（如 `dataset_table.cell_preview_max_chars`），由预览 `_mei.components` 下发。
    #[serde(default)]
    pub components: Value,
}
