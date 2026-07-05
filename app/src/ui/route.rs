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
            "build" | "manage" => Self::Layout,
            "config" => Self::Config,
            "upload" => Self::Upload,
            "runtime" => Self::Runtime,
            _ => Self::Layout,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Layout => "layout",
            Self::Prototype => "prototype",
            Self::Run => "run",
            Self::Copilot => "copilot",
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
        super::view_routing::app_surface_href(app_path, self)
    }

    /// 技术/历史显示名（兼容旧文案与内部日志）。
    pub fn label(self) -> &'static str {
        match self {
            Self::App => "访问",
            Self::Layout => "布局",
            Self::Prototype => "原型",
            Self::Run => "演说",
            Self::Copilot => "Copilot",
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
            Self::Config => "配置",
            Self::Upload => "上传",
            Self::Runtime => "运行",
        }
    }

    /// 是否在顶栏 mode-tabs 中作为一级产品面展示。
    pub fn is_topbar_product_tab(self) -> bool {
        matches!(
            self,
            Self::App | Self::Layout | Self::Prototype | Self::Config | Self::Upload | Self::Runtime
        )
    }

    pub fn is_app(self) -> bool {
        self == Self::App
    }

    pub fn is_access_like(self) -> bool {
        self == Self::App
    }

    /// 独立演说宿主路由（`/apps/run|copilot/*`）已退役；演说在 app surface 上以 action 执行。
    pub fn is_legacy_presentation_host(self) -> bool {
        matches!(self, Self::Run | Self::Copilot)
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

    /// 布局 / 原型工作区（原 build/manage 语义）。
    pub fn is_workspace(self) -> bool {
        matches!(self, Self::Layout | Self::Prototype)
    }

    /// 兼容旧调用点。
    pub fn is_build(self) -> bool {
        self.is_workspace()
    }

    pub fn uses_workspace_tree(self) -> bool {
        matches!(self, Self::Layout | Self::Prototype | Self::Runtime)
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
    fn legacy_build_manage_slugs_map_to_layout() {
        assert_eq!(UiRouteMode::from_slug("build"), UiRouteMode::Layout);
        assert_eq!(UiRouteMode::from_slug("manage"), UiRouteMode::Layout);
    }

    #[test]
    fn access_like_is_app_only() {
        assert!(UiRouteMode::App.is_access_like());
        assert!(!UiRouteMode::Run.is_access_like());
        assert!(!UiRouteMode::Copilot.is_access_like());
    }

    #[test]
    fn product_label_maps_to_ia_names() {
        assert_eq!(UiRouteMode::App.product_label(), "应用");
        assert_eq!(UiRouteMode::Layout.product_label(), "布局");
        assert_eq!(UiRouteMode::Runtime.product_label(), "运行");
        assert_eq!(UiRouteMode::Upload.product_label(), "上传");
        assert_eq!(UiRouteMode::Run.product_label(), "演说");
    }
}
