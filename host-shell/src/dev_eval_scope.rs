//! Development-only selective warmup / eval scope (0535).
//!
//! Production default is `full`. `static` / `scoped` shorten ACCESS READY and
//! limit dynamic evaluation to selected preview_scope prefixes.
//!
//! 0535 双集合：
//! - `warmup_scopes`：允许启动 / rewarm 预热的前缀（空 = 跳过启动 warmup）
//! - `eval_scopes`：允许客户端动态求值的前缀（空 = 全部 placeholder）
//! 向后兼容：仅设 `scopes` 时作为 `eval_scopes`，且不参与 warmup。

use serde_json::{json, Value};

use mei_lang_kernel::{RuntimeMode, RuntimePlan, RuntimePlanApp};

/// Process-level development eval profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevEvalProfile {
    #[default]
    Full,
    Static,
    Scoped,
}

impl DevEvalProfile {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "full" | "default" => Some(Self::Full),
            "static" | "off" | "none" => Some(Self::Static),
            "scoped" | "scope" => Some(Self::Scoped),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Static => "static",
            Self::Scoped => "scoped",
        }
    }

    pub fn skips_startup_warmup(self) -> bool {
        !matches!(self, Self::Full)
    }

    pub fn skips_access_bootstrap_gate(self) -> bool {
        !matches!(self, Self::Full)
    }

    pub fn skips_plug_ds_startup(self) -> bool {
        matches!(self, Self::Static)
    }

    pub fn allows_rewarm(self) -> bool {
        matches!(self, Self::Full)
    }

    pub fn forces_static_ceiling(self) -> bool {
        matches!(self, Self::Static)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DevEvalConfig {
    pub profile: DevEvalProfile,
    /// 允许预热的 scope 前缀（`scoped` 时；空 = 跳过启动 warmup）
    pub warmup_scopes: Vec<String>,
    /// 允许动态求值的 scope 前缀（`scoped` 时；空 = 全部 placeholder）
    pub eval_scopes: Vec<String>,
    runtime_plan: Option<RuntimePlan>,
    runtime_app_id: Option<String>,
}

impl DevEvalConfig {
    /// 旧式单集合入口（CLI `--eval-scope` / `MEI_EVAL_SCOPE`）。
    /// 设定的是 eval 集合；warmup 集合留空（scoped 下跳过启动 warmup）。
    pub fn from_env_and_args(
        profile_arg: Option<&str>,
        eval_scope_arg: Option<&str>,
        warmup_scope_arg: Option<&str>,
    ) -> Self {
        let profile_raw = profile_arg
            .map(str::to_string)
            .or_else(|| std::env::var("MEI_DEV_EVAL_PROFILE").ok())
            .unwrap_or_default();
        let profile = DevEvalProfile::parse(profile_raw.as_str()).unwrap_or_default();
        let eval_raw = eval_scope_arg
            .map(str::to_string)
            .or_else(|| std::env::var("MEI_EVAL_SCOPE").ok())
            .unwrap_or_default();
        let eval_scopes = parse_scope_list(eval_raw.as_str());
        let warmup_raw = warmup_scope_arg
            .map(str::to_string)
            .or_else(|| std::env::var("MEI_WARMUP_SCOPE").ok())
            .unwrap_or_default();
        let warmup_scopes = parse_scope_list(warmup_raw.as_str());
        let config = Self {
            profile,
            warmup_scopes,
            eval_scopes,
            runtime_plan: None,
            runtime_app_id: None,
        };
        if config.profile == DevEvalProfile::Scoped
            && config.eval_scopes.is_empty()
            && config.warmup_scopes.is_empty()
        {
            tracing::warn!(
                "MEI_DEV_EVAL_PROFILE=scoped but both MEI_EVAL_SCOPE and MEI_WARMUP_SCOPE are empty; \
                 treating as static placeholders"
            );
        }
        config
    }

    /// CLI/env 优先；未设置时回退到 `workspace.json` / `MEI_WORKSPACE_CONFIG` 的 `deploy.devEval`。
    pub fn resolve(
        workspace_root: &std::path::Path,
        profile_arg: Option<&str>,
        eval_scope_arg: Option<&str>,
        warmup_scope_arg: Option<&str>,
    ) -> Self {
        let from_cli_env = Self::from_env_and_args(profile_arg, eval_scope_arg, warmup_scope_arg);
        let cli_or_env_set = profile_arg
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
            || std::env::var("MEI_DEV_EVAL_PROFILE")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            || eval_scope_arg
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            || std::env::var("MEI_EVAL_SCOPE")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            || warmup_scope_arg
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            || std::env::var("MEI_WARMUP_SCOPE")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
        if cli_or_env_set {
            return from_cli_env;
        }
        if let Some(from_ws) = Self::from_workspace_config(workspace_root) {
            return from_ws;
        }
        from_cli_env
    }

    pub fn from_workspace_config(workspace_root: &std::path::Path) -> Option<Self> {
        let cfg = mei_lang_kernel::load_workspace_config(workspace_root);
        let dev = &cfg.deploy.dev_eval;
        if dev.is_empty() {
            return None;
        }
        let profile = DevEvalProfile::parse(dev.profile.as_deref().unwrap_or("full"))
            .unwrap_or(DevEvalProfile::Full);
        let warmup_scopes = normalize_scope_list(&dev.warmup_scopes);
        // eval_scopes 优先；回退到 legacy `scopes`
        let eval_scopes = if dev.eval_scopes.is_empty() {
            normalize_scope_list(&dev.scopes)
        } else {
            normalize_scope_list(&dev.eval_scopes)
        };
        Some(Self {
            profile,
            warmup_scopes,
            eval_scopes,
            runtime_plan: None,
            runtime_app_id: None,
        })
    }

    pub fn client_payload(&self) -> Value {
        json!({
            "profile": self.profile.slug(),
            "warmupScopes": self.warmup_scopes,
            "evalScopes": self.eval_scopes,
            "fill": "placeholder",
            "runtimePlan": self.runtime_plan,
            "appId": self.runtime_app_id,
        })
    }

    /// 客户端 bind / eval-pack 是否允许该 scope 动态求值。
    pub fn allows_eval_scope(&self, preview_scope: &str) -> bool {
        if self.runtime_plan.is_some() {
            return self.runtime_mode_for_scope(preview_scope) != RuntimeMode::Frozen;
        }
        match self.profile {
            DevEvalProfile::Full => true,
            DevEvalProfile::Static => false,
            DevEvalProfile::Scoped => {
                if self.eval_scopes.is_empty() {
                    return false;
                }
                scope_matches_any(preview_scope, &self.eval_scopes)
            }
        }
    }

    /// 启动 / rewarm 是否允许对该 scope 预热。
    pub fn allows_warmup_scope(&self, preview_scope: &str) -> bool {
        if self.runtime_plan.is_some() {
            return self.runtime_mode_for_scope(preview_scope) == RuntimeMode::Hot;
        }
        match self.profile {
            DevEvalProfile::Full => true,
            DevEvalProfile::Static => false,
            DevEvalProfile::Scoped => {
                if self.warmup_scopes.is_empty() {
                    return false;
                }
                scope_matches_any(preview_scope, &self.warmup_scopes)
            }
        }
    }

    pub fn allows_rewarm(&self) -> bool {
        self.runtime_plan
            .as_ref()
            .map(|plan| {
                plan.default_mode == RuntimeMode::Hot
                    || effective_app_plan(plan, self.runtime_app_id.as_deref().unwrap_or("*"))
                        .is_some_and(|app| {
                            app.targets
                                .iter()
                                .any(|target| target.mode == RuntimeMode::Hot)
                                || app
                                    .metric_overrides
                                    .values()
                                    .any(|mode| *mode == RuntimeMode::Hot)
                        })
            })
            .unwrap_or_else(|| self.profile.allows_rewarm())
    }

    fn runtime_mode_for_scope(&self, preview_scope: &str) -> RuntimeMode {
        let Some(plan) = self.runtime_plan.as_ref() else {
            return RuntimeMode::Hot;
        };
        let app_id = self.runtime_app_id.as_deref().unwrap_or("*");
        let mut selected = (0usize, plan.default_mode);
        if let Some(app) = effective_app_plan(plan, app_id) {
            let normalized = normalize_scope(preview_scope);
            for target in &app.targets {
                let prefix = normalize_scope(target.scope.as_str());
                if prefix == "*"
                    || normalized == prefix
                    || normalized.starts_with(&format!("{prefix}/"))
                {
                    let specificity = prefix.len();
                    if specificity >= selected.0 {
                        selected = (specificity, target.mode);
                    }
                }
            }
        }
        selected.1
    }
}

fn effective_app_plan<'a>(plan: &'a RuntimePlan, app_id: &str) -> Option<&'a RuntimePlanApp> {
    plan.apps.get(app_id).or_else(|| plan.apps.get("*"))
}

fn dev_eval_from_runtime_plan(plan: &RuntimePlan, app_id: &str) -> DevEvalConfig {
    let apps = if app_id == "*" {
        plan.apps.values().collect::<Vec<_>>()
    } else {
        effective_app_plan(plan, app_id)
            .into_iter()
            .collect::<Vec<_>>()
    };
    let warmup_scopes = apps
        .iter()
        .copied()
        .flat_map(|app| app.targets.iter())
        .filter(|target| target.mode == RuntimeMode::Hot)
        .map(|target| normalize_scope(target.scope.as_str()))
        .collect();
    let eval_scopes = apps
        .iter()
        .copied()
        .flat_map(|app| app.targets.iter())
        .filter(|target| target.mode != RuntimeMode::Frozen)
        .map(|target| normalize_scope(target.scope.as_str()))
        .collect();
    let has_non_frozen = plan.default_mode != RuntimeMode::Frozen
        || apps.iter().any(|app| {
            app.targets
                .iter()
                .any(|target| target.mode != RuntimeMode::Frozen)
                || app
                    .metric_overrides
                    .values()
                    .any(|mode| *mode != RuntimeMode::Frozen)
        });
    let profile = if plan.default_mode == RuntimeMode::Hot {
        DevEvalProfile::Full
    } else if has_non_frozen {
        DevEvalProfile::Scoped
    } else {
        DevEvalProfile::Static
    };
    DevEvalConfig {
        profile,
        warmup_scopes,
        eval_scopes,
        runtime_plan: Some(plan.clone()),
        runtime_app_id: Some(app_id.to_string()),
    }
}

fn normalize_scope_list(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|value| normalize_scope(value))
        .filter(|value| !value.is_empty())
        .collect()
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

fn parse_scope_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(normalize_scope)
        .filter(|part| !part.is_empty())
        .collect()
}

/// Prefix match: selected node and descendants. Exact segment boundary.
pub fn scope_matches_any(preview_scope: &str, prefixes: &[String]) -> bool {
    let scope = preview_scope.trim().trim_matches('/');
    if scope.is_empty() {
        return false;
    }
    // scene:default and empty structural keys stay unbound under scoped/static.
    if scope == "scene:default" {
        return prefixes.iter().any(|p| p == "scene:default" || p == "*");
    }
    prefixes.iter().any(|prefix| {
        let prefix = prefix.trim().trim_matches('/');
        if prefix.is_empty() || prefix == "*" {
            return true;
        }
        scope == prefix
            || scope.starts_with(&format!("{prefix}/"))
            || scope.strip_prefix("content:").is_some_and(|rest| {
                let rest = rest.trim_matches('/');
                rest == prefix || rest.starts_with(&format!("{prefix}/"))
            })
            || scope.strip_prefix("scope:").is_some_and(|rest| {
                let rest = rest.trim_matches('/');
                rest == prefix || rest.starts_with(&format!("{prefix}/"))
            })
    })
}

#[derive(Debug, Clone, Default)]
struct InstalledConfig {
    base: DevEvalConfig,
    runtime_plan: Option<RuntimePlan>,
}

static INSTALLED: std::sync::OnceLock<std::sync::RwLock<InstalledConfig>> =
    std::sync::OnceLock::new();

fn installed() -> &'static std::sync::RwLock<InstalledConfig> {
    INSTALLED.get_or_init(|| std::sync::RwLock::new(InstalledConfig::default()))
}

pub fn install(config: DevEvalConfig) {
    if config.profile != DevEvalProfile::Full {
        tracing::info!(
            profile = config.profile.slug(),
            warmup_scopes = %config.warmup_scopes.join(","),
            eval_scopes = %config.eval_scopes.join(","),
            "dev eval profile active (non-production workflow)"
        );
    }
    let mut guard = installed().write().expect("dev eval config lock");
    guard.base = config;
}

pub fn install_runtime_plan(plan: RuntimePlan) {
    let mut guard = installed().write().expect("dev eval config lock");
    guard.runtime_plan = Some(plan);
}

pub fn applied_runtime_plan(workspace_root: &std::path::Path) -> Option<RuntimePlan> {
    let path = workspace_root.join("deploy/state/host-control.json");
    let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    serde_json::from_value(value.get("runtimePlan")?.clone()).ok()
}

pub fn current() -> DevEvalConfig {
    current_for_app("*")
}

pub fn current_for_app(app_id: &str) -> DevEvalConfig {
    let guard = installed().read().expect("dev eval config lock");
    guard
        .runtime_plan
        .as_ref()
        .map(|plan| dev_eval_from_runtime_plan(plan, app_id))
        .unwrap_or_else(|| guard.base.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_prefix_matches_descendants() {
        let prefixes = vec!["home/t1/r-right-rail".to_string()];
        assert!(scope_matches_any("home/t1/r-right-rail", &prefixes));
        assert!(scope_matches_any(
            "home/t1/r-right-rail/s-warning",
            &prefixes
        ));
        assert!(scope_matches_any(
            "scope:home/t1/r-right-rail/s-warning",
            &prefixes
        ));
        assert!(!scope_matches_any("home/t1/r-header", &prefixes));
        assert!(!scope_matches_any("home/t0/r-map-stage", &prefixes));
    }

    #[test]
    fn section_scope_excludes_sibling() {
        let prefixes = vec!["home/t1/r-right-rail/s-warning".to_string()];
        assert!(scope_matches_any(
            "home/t1/r-right-rail/s-warning",
            &prefixes
        ));
        assert!(!scope_matches_any(
            "home/t1/r-right-rail/s-enforcement",
            &prefixes
        ));
    }

    #[test]
    fn static_profile_denies_all_scopes() {
        let config = DevEvalConfig {
            profile: DevEvalProfile::Static,
            warmup_scopes: vec!["home/t1".to_string()],
            eval_scopes: vec!["home/t1".to_string()],
            runtime_plan: None,
            runtime_app_id: None,
        };
        assert!(!config.allows_eval_scope("home/t1/r-right-rail"));
        assert!(!config.allows_warmup_scope("home/t1/r-right-rail"));
    }

    #[test]
    fn scoped_split_warmup_eval_independent() {
        let config = DevEvalConfig {
            profile: DevEvalProfile::Scoped,
            warmup_scopes: vec!["home/t1/r-right-rail/s-warning".to_string()],
            eval_scopes: vec![
                "home/t1/r-right-rail/s-warning".to_string(),
                "home/t2/r-warnings".to_string(),
            ],
            runtime_plan: None,
            runtime_app_id: None,
        };
        // warning: warmup + eval
        assert!(config.allows_warmup_scope("home/t1/r-right-rail/s-warning"));
        assert!(config.allows_eval_scope("home/t1/r-right-rail/s-warning"));
        // T2 warnings: eval only, no warmup
        assert!(!config.allows_warmup_scope("home/t2/r-warnings"));
        assert!(config.allows_eval_scope("home/t2/r-warnings"));
        // enforcement: neither
        assert!(!config.allows_warmup_scope("home/t1/r-right-rail/s-enforcement"));
        assert!(!config.allows_eval_scope("home/t1/r-right-rail/s-enforcement"));
    }

    #[test]
    fn resolve_reads_deploy_dev_eval_from_workspace_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::write(
            root.join("workspace.json"),
            r#"{
              "schemaVersion": 2,
              "workspace": {"id":"t","version":"1"},
              "deploy": {
                "devEval": {
                  "profile": "scoped",
                  "warmupScopes": ["home/t1/r-right-rail/s-warning"],
                  "evalScopes": ["home/t1/r-right-rail/s-warning", "home/t2/r-warnings"]
                }
              }
            }"#,
        )
        .expect("write config");
        let config = DevEvalConfig::resolve(root, None, None, None);
        assert_eq!(config.profile, DevEvalProfile::Scoped);
        assert_eq!(
            config.warmup_scopes,
            vec!["home/t1/r-right-rail/s-warning".to_string()]
        );
        assert_eq!(
            config.eval_scopes,
            vec![
                "home/t1/r-right-rail/s-warning".to_string(),
                "home/t2/r-warnings".to_string()
            ]
        );
    }

    #[test]
    fn resolve_legacy_scopes_falls_back_to_eval_scopes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::write(
            root.join("workspace.json"),
            r#"{
              "schemaVersion": 2,
              "workspace": {"id":"t","version":"1"},
              "deploy": {
                "devEval": {
                  "profile": "scoped",
                  "scopes": ["home/t1/r-right-rail"]
                }
              }
            }"#,
        )
        .expect("write config");
        let config = DevEvalConfig::resolve(root, None, None, None);
        assert_eq!(config.profile, DevEvalProfile::Scoped);
        assert!(config.warmup_scopes.is_empty());
        assert_eq!(config.eval_scopes, vec!["home/t1/r-right-rail".to_string()]);
    }

    #[test]
    fn runtime_plan_global_view_aggregates_configured_apps() {
        let plan: RuntimePlan = serde_json::from_value(json!({
            "defaultMode": "frozen",
            "apps": {
                "mini-data": {
                    "targets": [
                        {"scope": "home/warning", "mode": "hot"},
                        {"scope": "warning_analytics", "mode": "lazy"}
                    ],
                    "metricOverrides": {}
                }
            }
        }))
        .expect("runtime plan");
        let config = dev_eval_from_runtime_plan(&plan, "*");
        assert_eq!(config.profile, DevEvalProfile::Scoped);
        assert_eq!(config.warmup_scopes, vec!["home/warning".to_string()]);
        assert!(config
            .eval_scopes
            .contains(&"warning_analytics".to_string()));
    }
}
