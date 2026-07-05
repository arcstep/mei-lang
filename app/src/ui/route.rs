#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRouteMode {
    App,
    /// 布局审阅：结构 / section 深度（`/apps/{id}/layout`）。
    Layout,
    /// 原型审阅：静态内容稿（`/apps/{id}/prototype`）。
    Prototype,
    /// 演说宿主：单 scene 步进、全屏与讲述控制（兼容旧 `presentation` / `slides` 路由）。
    Run,
    /// Copilot 演说宿主：presentation 步进、工具条、气泡与 cockpit 动作编排。
    Copilot,
    /// 兼容旧 `/apps/build/...` 路由（重定向到 layout）。
    Build,
    Config,
    Upload,
    Runtime,
}

impl UiRouteMode {
    pub fn from_slug(value: &str) -> Self {
        match value {
            "app" | "access" | "access-only" | "access_only" => Self::App,
            "layout" => Self::Layout,
            "prototype" => Self::Prototype,
            "run" | "presentation" | "slides" => Self::Run,
            "copilot" | "speaker" => Self::Copilot,
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
            Self::Layout => "layout",
            Self::Prototype => "prototype",
            Self::Run => "run",
            Self::Copilot => "copilot",
            Self::Build => "build",
            Self::Config => "config",
            Self::Upload => "upload",
            Self::Runtime => "runtime",
        }
    }

    /// App-id-first canonical surface (`/apps/{id}/{surface}`).
    pub fn is_app_surface(self) -> bool {
        matches!(self, Self::App | Self::Layout | Self::Prototype)
    }

    pub fn app_surface_href(self, app_path: &str) -> String {
        format!(
            "/apps/{}/{}",
            app_path.trim_start_matches('/'),
            self.slug()
        )
    }

    /// 技术/历史显示名（兼容旧文案与内部日志）。
    pub fn label(self) -> &'static str {
        match self {
            Self::App => "访问",
            Self::Layout => "布局",
            Self::Prototype => "原型",
            Self::Run => "演说",
            Self::Copilot => "Copilot",
            Self::Build => "构建",
            Self::Config => "配置",
            Self::Upload => "上传",
            Self::Runtime => "运行",
        }
    }

    /// 顶栏与产品导航层显示名（不改内部 slug）。
    pub fn product_label(self) -> &'static str {
        match self {
            Self::App => "应用",
            Self::Layout => "布局",
            Self::Prototype => "原型",
            Self::Run | Self::Copilot => "演说",
            Self::Build => "开发",
            Self::Config => "配置",
            Self::Upload => "上传",
            Self::Runtime => "运行",
        }
    }

    /// 是否在顶栏 mode-tabs 中作为一级产品面展示。
    pub fn is_topbar_product_tab(self) -> bool {
        matches!(
            self,
            Self::App | Self::Layout | Self::Prototype | Self::Build | Self::Config | Self::Upload | Self::Runtime
        )
    }

    pub fn is_app(self) -> bool {
        self == Self::App
    }

    pub fn is_access_like(self) -> bool {
        matches!(self, Self::App | Self::Run | Self::Copilot)
    }

    pub fn is_run_like(self) -> bool {
        self == Self::Run
    }

    pub fn is_copilot_like(self) -> bool {
        self == Self::Copilot
    }

    /// 兼容旧命名。
    pub fn is_speaker_like(self) -> bool {
        self.is_copilot_like()
    }

    pub fn is_build(self) -> bool {
        matches!(self, Self::Build | Self::Layout | Self::Prototype)
    }

    pub fn uses_workspace_tree(self) -> bool {
        matches!(self, Self::Build | Self::Layout | Self::Prototype | Self::Runtime)
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
    fn copilot_and_speaker_slug_map_to_copilot_mode() {
        assert_eq!(UiRouteMode::from_slug("copilot"), UiRouteMode::Copilot);
        assert_eq!(UiRouteMode::from_slug("speaker"), UiRouteMode::Copilot);
    }

    #[test]
    fn product_label_maps_to_ia_names() {
        assert_eq!(UiRouteMode::App.product_label(), "应用");
        assert_eq!(UiRouteMode::Build.product_label(), "开发");
        assert_eq!(UiRouteMode::Runtime.product_label(), "运行");
        assert_eq!(UiRouteMode::Upload.product_label(), "上传");
        assert_eq!(UiRouteMode::Run.product_label(), "演说");
    }
}
