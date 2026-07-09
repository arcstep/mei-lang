use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::layout::LayoutDecl;
use super::ui::UiTreeNode;

/// Scene shell / projection zone declared on `panel(slot = panel_slot(...))`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PanelSlotDecl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepts: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(
        default,
        rename = "selection_from",
        skip_serializing_if = "Option::is_none"
    )]
    pub selection_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiNodeDecl {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// 可视化标题槽（`title=text(...)` / `title=component(...)`）；编译归一后并入 `blocks` 的 `head` 区。
    #[serde(default)]
    pub head: Option<Box<UiTreeNode>>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub layout: Option<LayoutDecl>,
    #[serde(default)]
    pub blocks: Vec<UiTreeNode>,
    /// Projection shell zone (`panel_slot`); primary source for compile/runtime shell inference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<PanelSlotDecl>,
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
    /// 编译归一：`panel_ref` 来源 capsule 路径；该 panel 内 `*_ref` 走私有 import scope。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_scope: Option<String>,
}
