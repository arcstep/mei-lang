use crate::agent_runtime::bridge::BridgePromptRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentMode {
    Ask,
    Build,
}

impl AgentMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            AgentMode::Ask => "ask",
            AgentMode::Build => "build",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteMode {
    Manage,
    Access,
}

impl RouteMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RouteMode::Manage => "manage",
            RouteMode::Access => "access",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentModePolicy {
    pub mode: AgentMode,
    pub route_mode: RouteMode,
}

fn normalize_route_mode(value: Option<&str>) -> RouteMode {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("access") | Some("run") => RouteMode::Access,
        _ => RouteMode::Manage,
    }
}

fn normalize_mode(value: Option<&str>) -> Option<AgentMode> {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("ask") | Some("plan") => Some(AgentMode::Ask),
        Some("build") => Some(AgentMode::Build),
        _ => None,
    }
}

impl AgentModePolicy {
    pub(crate) fn from_request(request: &BridgePromptRequest) -> Self {
        let route_mode = normalize_route_mode(request.route_mode.as_deref());
        let mode = normalize_mode(request.mode.as_deref())
            .or_else(|| normalize_mode(request.agent.as_deref()))
            .unwrap_or_else(|| match route_mode {
                RouteMode::Access => AgentMode::Ask,
                RouteMode::Manage => AgentMode::Build,
            });
        Self { mode, route_mode }
    }

    pub(crate) fn validate(self) -> Result<(), String> {
        if self.route_mode == RouteMode::Access && self.mode == AgentMode::Build {
            return Err("access 页面不允许 build 模式，请切换到 ask".to_string());
        }
        Ok(())
    }

    pub(crate) fn apply_to_request(self, request: &mut BridgePromptRequest) {
        request.mode = Some(self.mode.as_str().to_string());
        request.route_mode = Some(self.route_mode.as_str().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentMode, AgentModePolicy, RouteMode};
    use crate::agent_runtime::bridge::BridgePromptRequest;

    fn request(
        mode: Option<&str>,
        route_mode: Option<&str>,
        agent: Option<&str>,
    ) -> BridgePromptRequest {
        BridgePromptRequest {
            text: String::new(),
            app_id: None,
            scene_id: None,
            target_file: None,
            system: None,
            mode: mode.map(str::to_string),
            route_mode: route_mode.map(str::to_string),
            agent: agent.map(str::to_string),
            model: None,
        }
    }

    #[test]
    fn plan_agent_maps_to_ask_mode() {
        let req = request(None, Some("access"), Some("plan"));
        let policy = AgentModePolicy::from_request(&req);
        assert_eq!(policy.mode, AgentMode::Ask);
        assert_eq!(policy.route_mode, RouteMode::Access);
    }

    #[test]
    fn access_route_rejects_build_mode() {
        let req = request(Some("build"), Some("access"), None);
        let policy = AgentModePolicy::from_request(&req);
        assert!(policy.validate().is_err());
    }

    #[test]
    fn defaults_follow_route_mode() {
        let access = AgentModePolicy::from_request(&request(None, Some("access"), None));
        let manage = AgentModePolicy::from_request(&request(None, Some("manage"), None));
        assert_eq!(access.mode, AgentMode::Ask);
        assert_eq!(manage.mode, AgentMode::Build);
    }
}
