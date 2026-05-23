
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::layout::LayoutDecl;
use super::ui::UiNodeDecl;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// 可视化标题槽（`title=text(...)` / `title=component(...)`）；编译归一后并入 `blocks` 的 `head` 区。
    #[serde(default)]
    pub head: Option<Box<UiNodeDecl>>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub layout: Option<LayoutDecl>,
    #[serde(default)]
    pub blocks: Vec<UiNodeDecl>,
    #[serde(default)]
    pub props: Value,
    /// head 子容器视觉 props（与 `head` 标题槽内容区分）。
    #[serde(default)]
    pub head_props: Value,
    /// body 子容器视觉 props。
    #[serde(default)]
    pub body_props: Value,
    /// Authoring-only：`panel(base = panel_ref(...))` 克隆源；编译归一后清除。
    #[serde(default)]
    pub base: Option<Value>,
}
