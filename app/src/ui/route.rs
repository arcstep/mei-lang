#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRouteMode {
    App,
    Build,
    Config,
    Upload,
}

impl UiRouteMode {
    pub fn from_slug(value: &str) -> Self {
        match value {
            "app" | "access" | "run" | "access-only" | "access_only" => Self::App,
            "build" | "manage" => Self::Build,
            "config" => Self::Config,
            "upload" => Self::Upload,
            _ => Self::Build,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Build => "build",
            Self::Config => "config",
            Self::Upload => "upload",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::App => "访问",
            Self::Build => "构建",
            Self::Config => "配置",
            Self::Upload => "上传",
        }
    }

    pub fn is_app(self) -> bool {
        self == Self::App
    }

    pub fn is_build(self) -> bool {
        self == Self::Build
    }

    pub fn uses_workspace_tree(self) -> bool {
        self == Self::Build
    }

    pub fn uses_full_page_navigation(self) -> bool {
        matches!(self, Self::Config | Self::Upload)
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
}
