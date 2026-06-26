#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRouteMode {
    App,
    Presentation,
    Build,
    Config,
    Upload,
    Runtime,
}

impl UiRouteMode {
    pub fn from_slug(value: &str) -> Self {
        match value {
            "app" | "access" | "run" | "access-only" | "access_only" => Self::App,
            "presentation" | "slides" => Self::Presentation,
            "build" | "manage" => Self::Build,
            "config" => Self::Config,
            "upload" => Self::Upload,
            "runtime" => Self::Runtime,
            _ => Self::Build,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Presentation => "presentation",
            Self::Build => "build",
            Self::Config => "config",
            Self::Upload => "upload",
            Self::Runtime => "runtime",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::App => "访问",
            Self::Presentation => "演示",
            Self::Build => "构建",
            Self::Config => "配置",
            Self::Upload => "上传",
            Self::Runtime => "运行",
        }
    }

    pub fn is_app(self) -> bool {
        self == Self::App
    }

    pub fn is_access_like(self) -> bool {
        matches!(self, Self::App | Self::Presentation)
    }

    pub fn is_build(self) -> bool {
        self == Self::Build
    }

    pub fn uses_workspace_tree(self) -> bool {
        matches!(self, Self::Build | Self::Runtime)
    }

    pub fn uses_full_page_navigation(self) -> bool {
        matches!(self, Self::Config | Self::Upload | Self::Runtime)
    }

    pub fn uses_scene_route(self) -> bool {
        self.is_access_like()
    }
}

#[cfg(test)]
mod tests {
    use super::UiRouteMode;

    #[test]
    fn access_only_slug_maps_to_app_mode() {
        assert_eq!(UiRouteMode::from_slug("access-only"), UiRouteMode::App);
        assert_eq!(UiRouteMode::from_slug("access_only"), UiRouteMode::App);
    }

    #[test]
    fn presentation_aliases_map_to_presentation_mode() {
        assert_eq!(
            UiRouteMode::from_slug("presentation"),
            UiRouteMode::Presentation
        );
        assert_eq!(UiRouteMode::from_slug("slides"), UiRouteMode::Presentation);
    }
}
