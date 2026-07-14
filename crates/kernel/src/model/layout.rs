use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDecl {
    pub kind: String,
    pub id: String,
    pub title: Option<String>,
    /// Default product Stage id (Phase 9: replaces `default_scene`).
    #[serde(default, alias = "default_scene")]
    pub default_stage: Option<String>,
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
