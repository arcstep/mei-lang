use mei_lang_app::UiRouteMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeCoords {
    pub app_id: String,
    pub mode: UiMode,
    pub scene_id: String,
    pub target_file: String,
}

impl ScopeCoords {
    pub fn new(app_id: impl Into<String>, mode: UiMode, scene_id: impl Into<String>, target_file: impl Into<String>) -> Self {
        let target_file = target_file.into();
        let target_file = if target_file.ends_with(".mei") {
            mei_lang_kernel::canonical_app_source_rel_path(target_file.as_str())
        } else {
            target_file
        };
        Self {
            app_id: app_id.into(),
            mode,
            scene_id: scene_id.into(),
            target_file,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    Build,
    App,
    Run,
    #[serde(alias = "speaker")]
    Copilot,
    Config,
    Upload,
    Other,
}

impl UiMode {
    pub fn from_route_mode(route_mode: UiRouteMode) -> Self {
        match route_mode {
            UiRouteMode::Build => Self::Build,
            UiRouteMode::App => Self::App,
            UiRouteMode::Run => Self::Run,
            UiRouteMode::Copilot => Self::Copilot,
            UiRouteMode::Config => Self::Config,
            UiRouteMode::Upload => Self::Upload,
            UiRouteMode::Runtime => Self::Build,
        }
    }

    pub fn default_navigation_key(self) -> &'static str {
        match self {
            Self::Build => "default_build",
            Self::App | Self::Run | Self::Copilot => "default_access",
            _ => "default_access",
        }
    }

    pub fn scene_navigation_key(self, scene_id: &str) -> String {
        match self {
            Self::Build => format!("build:{scene_id}"),
            Self::App | Self::Run | Self::Copilot => format!("access:{scene_id}"),
            _ => format!("access:{scene_id}"),
        }
    }
}
