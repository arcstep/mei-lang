use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::layout::FrameDecl;
use super::ui::{SceneDecl, ThemeDecl};
use super::ui_node::UiNodeDecl;
use super::world::{FlowDecl, WorldDecl};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneContract {
    pub scene: SceneDecl,
    #[serde(default)]
    pub themes: Vec<ThemeDecl>,
    /// 当前 scene 的只读共享参数，编译期先合并自定义 theme.shared 与 scene.shared；
    /// 预览阶段再叠加 builtin preset shared 默认值。
    #[serde(default)]
    pub shared: Value,
    #[serde(default)]
    pub world: Option<WorldDecl>,
    #[serde(default)]
    pub flow: Option<FlowDecl>,
    #[serde(default)]
    pub frame: Option<FrameDecl>,
    #[serde(default)]
    pub panels: Vec<UiNodeDecl>,
}
