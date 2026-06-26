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
        Self {
            app_id: app_id.into(),
            mode,
            scene_id: scene_id.into(),
            target_file: target_file.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    Build,
    App,
    Presentation,
    Config,
    Upload,
    Other,
}

impl UiMode {
    pub fn from_route_mode(route_mode: UiRouteMode) -> Self {
        match route_mode {
            UiRouteMode::Build => Self::Build,
            UiRouteMode::App => Self::App,
            UiRouteMode::Presentation => Self::Presentation,
            UiRouteMode::Config => Self::Config,
            UiRouteMode::Upload => Self::Upload,
            UiRouteMode::Runtime => Self::Build,
        }
    }

    pub fn default_navigation_key(self) -> &'static str {
        match self {
            Self::Build => "default_build",
            Self::App | Self::Presentation => "default_access",
            _ => "default_access",
        }
    }

    pub fn scene_navigation_key(self, scene_id: &str) -> String {
        match self {
            Self::Build => format!("build:{scene_id}"),
            Self::App | Self::Presentation => format!("access:{scene_id}"),
            _ => format!("access:{scene_id}"),
        }
    }
}
