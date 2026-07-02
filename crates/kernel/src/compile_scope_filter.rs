use crate::canonical_app_source_rel_path;
use crate::mei_config::match_path_glob;
use crate::mei_config::CompileScopeFilterConfig;

fn normalize_target_path(target: &str) -> String {
    canonical_app_source_rel_path(target.trim())
}

fn normalize_glob_pattern(pattern: &str) -> String {
    let pattern = pattern.trim().replace('\\', "/");
    if pattern.is_empty()
        || pattern.starts_with("src/")
        || pattern.starts_with("**/")
        || pattern.starts_with("../../../")
    {
        return pattern;
    }
    if pattern.starts_with("scenes/") || pattern == "main.mei" || pattern.starts_with("main.mei") {
        return format!("src/{pattern}");
    }
    pattern
}

fn matches_any_glob(patterns: &[String], value: &str) -> bool {
    patterns
        .iter()
        .map(|pattern| normalize_glob_pattern(pattern.as_str()))
        .filter(|pattern| !pattern.is_empty())
        .any(|pattern| match_path_glob(pattern.as_str(), value))
}

pub fn compile_scope_target_allowed(
    config: &CompileScopeFilterConfig,
    target: &str,
) -> bool {
    let normalized = normalize_target_path(target);
    if normalized.is_empty() {
        return true;
    }
    if matches_any_glob(config.exclude_targets.as_slice(), normalized.as_str()) {
        return false;
    }
    if config.include_targets.is_empty() {
        return true;
    }
    matches_any_glob(config.include_targets.as_slice(), normalized.as_str())
}

pub fn compile_scope_scene_id_allowed(
    config: &CompileScopeFilterConfig,
    scene_id: &str,
) -> bool {
    let scene_id = scene_id.trim();
    if scene_id.is_empty() {
        return true;
    }
    !matches_any_glob(config.exclude_scene_ids.as_slice(), scene_id)
}

/// Default route scope（无 target）始终允许；有 target/scene 时按 compileScope 过滤。
pub fn compile_scope_entry_allowed(
    config: Option<&CompileScopeFilterConfig>,
    scene_id: Option<&str>,
    target_file: Option<&str>,
) -> bool {
    let Some(config) = config.filter(|cfg| cfg.is_active()) else {
        return true;
    };
    if let Some(scene_id) = scene_id.map(str::trim).filter(|value| !value.is_empty()) {
        if !compile_scope_scene_id_allowed(config, scene_id) {
            return false;
        }
    }
    if let Some(target) = target_file.map(str::trim).filter(|value| !value.is_empty()) {
        return compile_scope_target_allowed(config, target);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_only() -> CompileScopeFilterConfig {
        CompileScopeFilterConfig {
            exclude_targets: vec!["**/*.page.mei".to_string()],
            skip_discover: Some(true),
            skip_t2_page_autogen_focus: Some(true),
            ..CompileScopeFilterConfig::default()
        }
    }

    #[test]
    fn exclude_t2_page_targets() {
        let cfg = home_only();
        assert!(compile_scope_target_allowed(&cfg, "scenes/home.mei"));
        assert!(!compile_scope_target_allowed(&cfg, "scenes/05-监督预警.page.mei"));
        assert!(!compile_scope_target_allowed(
            &cfg,
            "scenes/_shared/warning-detail.detail.page.mei"
        ));
    }

    #[test]
    fn include_targets_whitelist() {
        let cfg = CompileScopeFilterConfig {
            include_targets: vec![
                "scenes/home.mei".to_string(),
                "scenes/layout-*.mei".to_string(),
            ],
            ..CompileScopeFilterConfig::default()
        };
        assert!(compile_scope_target_allowed(&cfg, "scenes/home.mei"));
        assert!(compile_scope_target_allowed(&cfg, "scenes/layout-中栏.mei"));
        assert!(!compile_scope_target_allowed(&cfg, "scenes/05-监督预警.page.mei"));
    }

    #[test]
    fn exclude_scene_ids() {
        let cfg = CompileScopeFilterConfig {
            exclude_scene_ids: vec!["*_analytics_page".to_string()],
            ..CompileScopeFilterConfig::default()
        };
        assert!(compile_scope_scene_id_allowed(&cfg, "home"));
        assert!(!compile_scope_scene_id_allowed(&cfg, "issue_pending_analytics_page"));
    }
}
