use serde::{Deserialize, Serialize};

use super::layout::FrameDecl;
use super::panel::PanelDecl;
use super::ui::{SceneDecl, ThemeDecl};
use super::world::{FlowDecl, WorldDecl};

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
