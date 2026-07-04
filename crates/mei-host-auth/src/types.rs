use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use super::runtime::normalize_id;

/// 宿主启动时的认证策略：默认 `disabled`；`serve --auth` 时设为 `required`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthEnforcement {
    Required,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthRole {
    Guest,
    Admin,
    Super,
}

impl AuthRole {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthRole::Guest => "guest",
            AuthRole::Admin => "admin",
            AuthRole::Super => "super",
        }
    }

    pub fn from_slug(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "guest" => Some(Self::Guest),
            "admin" => Some(Self::Admin),
            "super" => Some(Self::Super),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub profile: String,
    pub role: String,
    #[serde(default)]
    pub app_allowlist: Vec<String>,
    #[serde(default)]
    pub app_denylist: Vec<String>,
    #[serde(default)]
    pub scene_allowlist: BTreeMap<String, Vec<String>>,
    pub iat: usize,
    pub exp: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthPrincipal {
    pub username: String,
    pub profile: String,
    pub role: AuthRole,
    pub app_allowlist: BTreeSet<String>,
    pub app_denylist: BTreeSet<String>,
    pub scene_allowlist: BTreeMap<String, BTreeSet<String>>,
    /// JWT `exp` claim (unix seconds); 0 when unknown.
    pub session_exp: usize,
}

impl AuthPrincipal {
    pub fn from_claims(claims: &AuthClaims) -> Self {
        let role = AuthRole::from_slug(&claims.role).unwrap_or(AuthRole::Guest);
        let app_allowlist = claims
            .app_allowlist
            .iter()
            .map(|value| normalize_id(value))
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>();
        let app_denylist = claims
            .app_denylist
            .iter()
            .map(|value| normalize_id(value))
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>();
        let mut scene_allowlist = BTreeMap::new();
        for (app, scenes) in &claims.scene_allowlist {
            let app_id = normalize_id(app);
            if app_id.is_empty() {
                continue;
            }
            let allowed = scenes
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<BTreeSet<_>>();
            if !allowed.is_empty() {
                scene_allowlist.insert(app_id, allowed);
            }
        }
        Self {
            username: claims.sub.clone(),
            profile: claims.profile.clone(),
            role,
            app_allowlist,
            app_denylist,
            scene_allowlist,
            session_exp: claims.exp,
        }
    }

    pub fn role_slug(&self) -> &'static str {
        self.role.as_str()
    }

    /// 配置/上传视图与对应写 API（admin、super）。
    pub fn can_use_config_upload_surface(&self) -> bool {
        matches!(self.role, AuthRole::Admin | AuthRole::Super)
    }

    /// 构建视图路由（仅 super）。
    pub fn can_use_build_surface(&self) -> bool {
        matches!(self.role, AuthRole::Super)
    }

    /// 访问侧 Agent（问答/会话/上下文预览）；guest/admin/super 均可用。
    pub fn can_use_access_agent_api(&self) -> bool {
        true
    }

    pub fn can_manage_sensitive_api(&self) -> bool {
        matches!(self.role, AuthRole::Super)
    }

    /// 与页面 `data-mei-auth-capabilities` 及 `authorize_path` 共用真源。
    pub fn capabilities(&self) -> mei_lang_app::HostCapabilities {
        mei_lang_app::HostCapabilities {
            access_view: true,
            config_upload: self.can_use_config_upload_surface(),
            build_view: self.can_use_build_surface(),
            access_agent: self.can_use_access_agent_api(),
            authoring_agent: self.can_use_build_surface(),
            agent_control: self.can_manage_sensitive_api(),
            runtime_components: true,
        }
    }

    pub fn can_access_host_route_mode(&self, mode: &str) -> bool {
        match mode {
            "app" | "access" | "access-only" | "run" | "presentation" | "slides"
            | "copilot" | "speaker" => true,
            "upload" | "config" => self.can_use_config_upload_surface(),
            "build" | "manage" | "runtime" => self.can_use_build_surface(),
            _ => false,
        }
    }

    pub fn can_access_app(&self, app_id: &str) -> bool {
        if !matches!(self.role, AuthRole::Guest) {
            return true;
        }
        let app_id = normalize_id(app_id);
        if self.app_denylist.contains(&app_id) {
            return false;
        }
        if self.app_allowlist.is_empty() {
            return true;
        }
        self.app_allowlist.contains(&app_id)
    }

    pub fn can_access_scene(&self, app_id: &str, scene_id: &str) -> bool {
        if !matches!(self.role, AuthRole::Guest) {
            return true;
        }
        if !self.can_access_app(app_id) {
            return false;
        }
        match self.scene_allowlist.get(&normalize_id(app_id)) {
            Some(allowed) => allowed.contains(scene_id.trim()),
            None => true,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AuthUserRecord {
    pub(crate) username: String,
    pub(crate) profile: String,
    pub(crate) password_hash: String,
    pub(crate) role: AuthRole,
    pub(crate) app_allowlist: Vec<String>,
    pub(crate) app_denylist: Vec<String>,
    pub(crate) scene_allowlist: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AuthRuntime {
    /// 密钥与用户齐备，可完成登录与 JWT 签发。
    pub enabled: bool,
    pub config_path: PathBuf,
    pub cookie_name: String,
    pub jwt_ttl_seconds: u64,
    pub jwt_secret: String,
    pub public_key_pem: String,
    pub private_key_pem: String,
    pub(crate) users: HashMap<String, AuthUserRecord>,
}
