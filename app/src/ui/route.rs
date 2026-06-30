#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRouteMode {
    App,
    /// 单 scene 脱离宿主控制壳独立运行（原 `presentation` / `slides` 路由）。
    Run,
    /// 演说模式：tour 步进、助手壳、文案与 cockpit 动作编排。
    Speaker,
    Build,
    Config,
    Upload,
    Runtime,
}

impl UiRouteMode {
    pub fn from_slug(value: &str) -> Self {
        match value {
            "app" | "access" | "access-only" | "access_only" => Self::App,
            "run" | "presentation" | "slides" => Self::Run,
            "speaker" => Self::Speaker,
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
            Self::Run => "run",
            Self::Speaker => "speaker",
            Self::Build => "build",
            Self::Config => "config",
            Self::Upload => "upload",
            Self::Runtime => "runtime",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::App => "访问",
            Self::Run => "独立运行",
            Self::Speaker => "演说",
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
        matches!(self, Self::App | Self::Run | Self::Speaker)
    }

    pub fn is_run_like(self) -> bool {
        self == Self::Run
    }

    pub fn is_speaker_like(self) -> bool {
        self == Self::Speaker
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
    fn run_aliases_map_to_run_mode() {
        assert_eq!(UiRouteMode::from_slug("run"), UiRouteMode::Run);
        assert_eq!(UiRouteMode::from_slug("presentation"), UiRouteMode::Run);
        assert_eq!(UiRouteMode::from_slug("slides"), UiRouteMode::Run);
    }

    #[test]
    fn speaker_slug_maps_to_speaker_mode() {
        assert_eq!(UiRouteMode::from_slug("speaker"), UiRouteMode::Speaker);
    }
}
