//! Process-safe development eval gate shared by host and data APIs.
//!
//! The host-shell owns its richer startup profile, while independently spawned
//! server / plug-ds processes resolve the same contract from environment or the
//! workspace config. This keeps frozen metric enforcement out of process-local
//! `OnceLock` state.

use std::path::Path;

use crate::{RuntimeMode, RuntimePlan, RuntimePlanApp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDevEvalProfile {
    Full,
    Static,
    Scoped,
}

impl RuntimeDevEvalProfile {
    fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "static" | "off" | "none" => Self::Static,
            "scoped" | "scope" => Self::Scoped,
            _ => Self::Full,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Static => "static",
            Self::Scoped => "scoped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDevEvalGate {
    pub profile: RuntimeDevEvalProfile,
    pub eval_scopes: Vec<String>,
    runtime_plan: Option<RuntimePlan>,
    app_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDevEvalDecision {
    pub accepted: bool,
    pub reason: &'static str,
}

impl Default for RuntimeDevEvalGate {
    fn default() -> Self {
        Self {
            profile: RuntimeDevEvalProfile::Full,
            eval_scopes: Vec::new(),
            runtime_plan: None,
            app_id: "*".to_string(),
        }
    }
}

impl RuntimeDevEvalGate {
    pub fn resolve(workspace_root: &Path) -> Self {
        Self::resolve_for_app(workspace_root, "*")
    }

    pub fn resolve_for_app(workspace_root: &Path, app_id: &str) -> Self {
        let profile_env = std::env::var("MEI_DEV_EVAL_PROFILE").ok();
        let scopes_env = std::env::var("MEI_EVAL_SCOPE").ok();
        let env_configured = profile_env
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || scopes_env
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
        if env_configured {
            return Self {
                profile: RuntimeDevEvalProfile::parse(profile_env.as_deref().unwrap_or("full")),
                eval_scopes: parse_scope_list(scopes_env.as_deref().unwrap_or("")),
                runtime_plan: None,
                app_id: app_id.to_string(),
            };
        }

        let explicit_workspace_config = std::env::var("MEI_WORKSPACE_CONFIG")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if explicit_workspace_config {
            let workspace = crate::load_workspace_config(workspace_root);
            if workspace.deploy.runtime_plan.is_some() {
                return Self::from_runtime_plan(workspace.deploy.effective_runtime_plan(), app_id);
            }
            return Self::from_legacy_config(&workspace.deploy.dev_eval, app_id);
        }

        if let Some(plan) = load_applied_runtime_plan(workspace_root) {
            return Self::from_runtime_plan(plan, app_id);
        }

        let workspace = crate::load_workspace_config(workspace_root);
        if workspace.deploy.runtime_plan.is_some() {
            return Self::from_runtime_plan(workspace.deploy.effective_runtime_plan(), app_id);
        }
        let config = &workspace.deploy.dev_eval;
        Self::from_legacy_config(config, app_id)
    }

    fn from_legacy_config(config: &crate::WorkspaceDeployDevEvalConfig, app_id: &str) -> Self {
        if config.is_empty() {
            return Self {
                app_id: app_id.to_string(),
                ..Self::default()
            };
        }
        let eval_scopes = if config.eval_scopes.is_empty() {
            normalize_scope_list(&config.scopes)
        } else {
            normalize_scope_list(&config.eval_scopes)
        };
        Self {
            profile: RuntimeDevEvalProfile::parse(config.profile.as_deref().unwrap_or("full")),
            eval_scopes,
            runtime_plan: None,
            app_id: app_id.to_string(),
        }
    }

    pub fn decide_scope(&self, preview_scope: Option<&str>) -> RuntimeDevEvalDecision {
        if let Some(plan) = self.runtime_plan.as_ref() {
            let mode = runtime_mode_for_scope(plan, self.app_id.as_str(), preview_scope);
            return RuntimeDevEvalDecision {
                accepted: mode != RuntimeMode::Frozen,
                reason: match mode {
                    RuntimeMode::Hot => "runtime_plan_hot",
                    RuntimeMode::Lazy => "runtime_plan_lazy",
                    RuntimeMode::Frozen => "runtime_plan_frozen",
                },
            };
        }
        match self.profile {
            RuntimeDevEvalProfile::Full => RuntimeDevEvalDecision {
                accepted: true,
                reason: "profile_full",
            },
            RuntimeDevEvalProfile::Static => RuntimeDevEvalDecision {
                accepted: false,
                reason: "profile_static",
            },
            RuntimeDevEvalProfile::Scoped => {
                let Some(scope) = preview_scope
                    .map(str::trim)
                    .filter(|scope| !scope.is_empty())
                else {
                    return RuntimeDevEvalDecision {
                        accepted: false,
                        reason: "missing_preview_scope",
                    };
                };
                if scope_matches_any(scope, &self.eval_scopes) {
                    RuntimeDevEvalDecision {
                        accepted: true,
                        reason: "scope_allowed",
                    }
                } else {
                    RuntimeDevEvalDecision {
                        accepted: false,
                        reason: "scope_frozen",
                    }
                }
            }
        }
    }

    pub fn decide_metric(
        &self,
        metric_id: Option<&str>,
        preview_scope: Option<&str>,
    ) -> RuntimeDevEvalDecision {
        let Some(plan) = self.runtime_plan.as_ref() else {
            return self.decide_scope(preview_scope);
        };
        let app = effective_app_plan(plan, self.app_id.as_str());
        if let Some(mode) = metric_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .and_then(|id| app.and_then(|app| app.metric_overrides.get(id)))
        {
            let accepted = match mode {
                RuntimeMode::Hot => true,
                RuntimeMode::Lazy => {
                    runtime_mode_for_scope(plan, self.app_id.as_str(), preview_scope)
                        == RuntimeMode::Lazy
                }
                RuntimeMode::Frozen => false,
            };
            return RuntimeDevEvalDecision {
                accepted,
                reason: match mode {
                    RuntimeMode::Hot => "runtime_metric_hot",
                    RuntimeMode::Lazy if accepted => "runtime_metric_lazy_explicit",
                    RuntimeMode::Lazy => "runtime_metric_lazy_deferred",
                    RuntimeMode::Frozen => "runtime_metric_frozen",
                },
            };
        }
        self.decide_scope(preview_scope)
    }

    fn from_runtime_plan(plan: RuntimePlan, app_id: &str) -> Self {
        let profile = match plan.default_mode {
            RuntimeMode::Hot => RuntimeDevEvalProfile::Full,
            RuntimeMode::Lazy => RuntimeDevEvalProfile::Scoped,
            RuntimeMode::Frozen => RuntimeDevEvalProfile::Static,
        };
        let eval_scopes = effective_app_plan(&plan, app_id)
            .into_iter()
            .flat_map(|app| app.targets.iter())
            .filter(|target| target.mode != RuntimeMode::Frozen)
            .map(|target| normalize_scope(target.scope.as_str()))
            .collect();
        Self {
            profile,
            eval_scopes,
            runtime_plan: Some(plan),
            app_id: app_id.to_string(),
        }
    }
}

fn load_applied_runtime_plan(workspace_root: &Path) -> Option<RuntimePlan> {
    let path = workspace_root.join("deploy/state/host-control.json");
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    serde_json::from_value(value.get("runtimePlan")?.clone()).ok()
}

fn effective_app_plan<'a>(plan: &'a RuntimePlan, app_id: &str) -> Option<&'a RuntimePlanApp> {
    plan.apps.get(app_id).or_else(|| plan.apps.get("*"))
}

fn runtime_mode_for_scope(
    plan: &RuntimePlan,
    app_id: &str,
    preview_scope: Option<&str>,
) -> RuntimeMode {
    let Some(scope) = preview_scope
        .map(normalize_scope)
        .filter(|scope| !scope.is_empty())
    else {
        return plan.default_mode;
    };
    let mut selected = (0usize, plan.default_mode);
    if let Some(app) = effective_app_plan(plan, app_id) {
        for target in &app.targets {
            let prefix = normalize_scope(target.scope.as_str());
            if prefix == "*" || scope == prefix || scope.starts_with(&format!("{prefix}/")) {
                if prefix.len() >= selected.0 {
                    selected = (prefix.len(), target.mode);
                }
            }
        }
    }
    selected.1
}

fn normalize_scope(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .strip_prefix("content:")
        .or_else(|| value.trim().trim_matches('/').strip_prefix("scope:"))
        .unwrap_or_else(|| value.trim().trim_matches('/'))
        .trim_matches('/')
        .to_string()
}

fn normalize_scope_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| normalize_scope(value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_scope_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(normalize_scope)
        .filter(|scope| !scope.is_empty())
        .collect()
}

fn scope_matches_any(preview_scope: &str, prefixes: &[String]) -> bool {
    let scope = normalize_scope(preview_scope);
    if scope.is_empty() {
        return false;
    }
    prefixes.iter().any(|prefix| {
        let prefix = normalize_scope(prefix);
        prefix == "*" || scope == prefix || scope.starts_with(&format!("{prefix}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_gate_accepts_selected_descendants_and_rejects_siblings() {
        let gate = RuntimeDevEvalGate {
            profile: RuntimeDevEvalProfile::Scoped,
            eval_scopes: vec!["home/t1/r-right-rail/s-warning".to_string()],
            runtime_plan: None,
            app_id: "demo".to_string(),
        };
        assert!(
            gate.decide_scope(Some("home/t1/r-right-rail/s-warning/value"))
                .accepted
        );
        let frozen = gate.decide_scope(Some("home/t1/r-right-rail/s-enforcement"));
        assert!(!frozen.accepted);
        assert_eq!(frozen.reason, "scope_frozen");
    }

    #[test]
    fn scoped_gate_fails_closed_without_request_scope() {
        let gate = RuntimeDevEvalGate {
            profile: RuntimeDevEvalProfile::Scoped,
            eval_scopes: vec!["home/t1/r-right-rail/s-warning".to_string()],
            runtime_plan: None,
            app_id: "demo".to_string(),
        };
        let decision = gate.decide_scope(None);
        assert!(!decision.accepted);
        assert_eq!(decision.reason, "missing_preview_scope");
    }

    #[test]
    fn metric_overrides_distinguish_hot_lazy_and_frozen_requests() {
        let app = RuntimePlanApp {
            targets: vec![
                crate::RuntimePlanTarget {
                    scope: "home/warning".to_string(),
                    mode: RuntimeMode::Hot,
                },
                crate::RuntimePlanTarget {
                    scope: "warning_analytics".to_string(),
                    mode: RuntimeMode::Lazy,
                },
            ],
            metric_overrides: [
                ("warnings_count".to_string(), RuntimeMode::Hot),
                ("models_count".to_string(), RuntimeMode::Lazy),
                ("items_count".to_string(), RuntimeMode::Frozen),
            ]
            .into_iter()
            .collect(),
        };
        let gate = RuntimeDevEvalGate::from_runtime_plan(
            RuntimePlan {
                default_mode: RuntimeMode::Frozen,
                apps: [("demo".to_string(), app)].into_iter().collect(),
            },
            "demo",
        );

        assert!(
            gate.decide_metric(Some("warnings_count"), Some("home/warning"))
                .accepted
        );
        let deferred = gate.decide_metric(Some("models_count"), Some("home/warning"));
        assert!(!deferred.accepted);
        assert_eq!(deferred.reason, "runtime_metric_lazy_deferred");
        assert!(
            gate.decide_metric(Some("models_count"), Some("warning_analytics"))
                .accepted
        );
        assert!(
            !gate
                .decide_metric(Some("items_count"), Some("warning_analytics"))
                .accepted
        );
    }
}
