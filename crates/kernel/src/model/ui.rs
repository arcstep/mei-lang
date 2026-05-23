use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::layout::LayoutDecl;
use super::panel::PanelDecl;

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
    if value.get("use_key").is_some() || value.get("kind").and_then(Value::as_str) == Some("block")
    {
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
    pub tokens: Value,
    /// 组件级默认配置（如 `dataset_table.cell_preview_max_chars`），由预览 `_mei.components` 下发。
    #[serde(default)]
    pub components: Value,
}
