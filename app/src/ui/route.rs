#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRouteMode {
    Manage,
    Access,
}

impl UiRouteMode {
    pub fn from_slug(value: &str) -> Self {
        match value {
            "access" | "run" => Self::Access,
            _ => Self::Manage,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Manage => "manage",
            Self::Access => "access",
        }
    }
}
