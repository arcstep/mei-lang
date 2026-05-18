//! Agent 请求的业务 scope 规范化：`resource_visibility`、dataset 参数覆盖校验、`read_file` 业务白名单、
//! 以及基于 world inventory 的 direct refs / scene reachable 可达集。

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::agent_runtime::bridge::BridgePromptRequest;
use crate::http::scene_api::{WorldContextSnapshot, WorldScope};

use super::mode_policy::{AgentMode, AgentModePolicy, RouteMode};
use super::resource_tools::{AgentResourceScope, ResourceVisibility};

/// 由 `WorldContextSnapshot.resource_inventory` 解析出的可达路径集合（workspace 相对路径，已规范化）。
#[derive(Debug, Clone, Default)]
pub(crate) struct ScopeReachabilitySets {
    pub direct_ref_paths: HashSet<String>,
    pub scene_reachable_paths: HashSet<String>,
}

impl ScopeReachabilitySets {
    /// 从 world 上下文快照构建：direct = active_target + `related_to_target` 的条目；scene = 所有带 `source_path` 的条目 + active_target。
    pub(crate) fn from_world_snapshot(snapshot: &WorldContextSnapshot, app_id: &str) -> Self {
        let mut direct = HashSet::new();
        let mut scene = HashSet::new();
        let app = app_id.trim();
        if let Some(p) = norm_workspace_rel(&snapshot.active_target_file, app) {
            direct.insert(p.clone());
            scene.insert(p);
        }
        for item in &snapshot.resource_inventory.items {
            if let Some(ref sp) = item.source_path {
                if let Some(norm) = norm_workspace_rel(sp, app) {
                    scene.insert(norm.clone());
                    if item.related_to_target {
                        direct.insert(norm);
                    }
                }
            }
        }
        Self {
            direct_ref_paths: direct,
            scene_reachable_paths: scene,
        }
    }

    /// 无快照时的保守回退：仅将请求中的 `target_file` 纳入 direct/scene（若可规范化）。
    pub(crate) fn fallback_from_request_target(request: &BridgePromptRequest, app_id: &str) -> Self {
        let mut direct = HashSet::new();
        let mut scene = HashSet::new();
        let app = app_id.trim();
        if let Some(raw) = request
            .target_file
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Some(p) = norm_workspace_rel(raw, app) {
                direct.insert(p.clone());
                scene.insert(p);
            }
        }
        Self {
            direct_ref_paths: direct,
            scene_reachable_paths: scene,
        }
    }

    pub(crate) fn digest_short(&self) -> String {
        let mut keys: Vec<_> = self.scene_reachable_paths.iter().cloned().collect();
        keys.sort();
        let mut h = DefaultHasher::new();
        keys.hash(&mut h);
        format!("{:016x}", h.finish())
    }

    pub(crate) fn to_arc_pair(self) -> (Arc<HashSet<String>>, Arc<HashSet<String>>) {
        (
            Arc::new(self.direct_ref_paths),
            Arc::new(self.scene_reachable_paths),
        )
    }
}

/// 统一规范化 workspace 相对路径，便于集合匹配（与 `read_file` sanitize 后的形式对齐）。
pub(crate) fn norm_workspace_rel(path: &str, app_id: &str) -> Option<String> {
    let mut p = path
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if p.is_empty() {
        return None;
    }
    let app = app_id.trim();
    if !app.is_empty() {
        let pref = format!("{app}/");
        if p == app {
            return Some(p);
        }
        if p.starts_with(&pref) {
            return Some(p);
        }
        // 无 app 前缀的相对路径（常见于 DSL 内 `source.path`）：视为位于该 app 目录下
        p = format!("{app}/{p}");
    }
    Some(p)
}

fn norm_rel_for_read_compare(rel: &str, app_id: Option<&str>) -> String {
    let rel = rel.trim().replace('\\', "/").trim_start_matches('/').to_string();
    let Some(app) = app_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return rel;
    };
    norm_workspace_rel(&rel, app).unwrap_or(rel)
}

/// 根据路由与模式选择默认的资源可见策略（可被请求体显式覆盖）。
pub(crate) fn default_resource_visibility(policy: AgentModePolicy) -> ResourceVisibility {
    match (policy.route_mode, policy.mode) {
        (RouteMode::Access, AgentMode::Ask) => ResourceVisibility::AllowSceneReachable,
        (RouteMode::Manage, AgentMode::Ask) => ResourceVisibility::AllowDirectRefs,
        (RouteMode::Manage, AgentMode::Build) => ResourceVisibility::AllowDirectRefs,
        (RouteMode::Access, AgentMode::Build) => ResourceVisibility::LocalOnly,
    }
}

/// 解析并收敛 `resource_visibility`：未知值回退到默认值。
pub(crate) fn resolve_resource_visibility(
    request: &BridgePromptRequest,
    policy: AgentModePolicy,
) -> ResourceVisibility {
    ResourceVisibility::parse(request.resource_visibility.as_deref())
        .unwrap_or_else(|| default_resource_visibility(policy))
}

/// 从 HTTP 请求构造 Native 侧资源 scope（无 inventory 时可达集为空，read_file 在非 local 模式下会偏保守）。
pub(crate) fn agent_resource_scope_from_request(
    request: &BridgePromptRequest,
    policy: AgentModePolicy,
) -> AgentResourceScope {
    let vis = resolve_resource_visibility(request, policy);
    let app_id = request.app_id.as_deref().unwrap_or("").trim();
    let reach = if app_id.is_empty() {
        ScopeReachabilitySets::default()
    } else {
        ScopeReachabilitySets::fallback_from_request_target(request, app_id)
    };
    let (d, s) = reach.to_arc_pair();
    AgentResourceScope {
        scene_id: request.scene_id.clone(),
        target_file: request.target_file.clone(),
        resource_visibility: vis,
        direct_ref_paths: d,
        scene_reachable_paths: s,
    }
}

/// 结合 world 快照构造完整执行期 scope（推荐路径：由 HTTP 分发层在 `send_prompt` 前构建）。
pub(crate) fn agent_resource_scope_from_request_with_snapshot(
    request: &BridgePromptRequest,
    policy: AgentModePolicy,
    snapshot: Option<&WorldContextSnapshot>,
    app_id: &str,
) -> AgentResourceScope {
    let vis = resolve_resource_visibility(request, policy);
    let reach = match snapshot {
        Some(snap) => ScopeReachabilitySets::from_world_snapshot(snap, app_id),
        None => ScopeReachabilitySets::fallback_from_request_target(request, app_id),
    };
    let (d, s) = reach.to_arc_pair();
    AgentResourceScope {
        scene_id: request.scene_id.clone(),
        target_file: request.target_file.clone(),
        resource_visibility: vis,
        direct_ref_paths: d,
        scene_reachable_paths: s,
    }
}

fn paths_match_workspace_rel(rel: &str, target: &str, app_id: Option<&str>) -> bool {
    let rel = rel.replace('\\', "/").trim_start_matches('/').to_string();
    let target = target.replace('\\', "/").trim_start_matches('/').to_string();
    if rel == target {
        return true;
    }
    if let Some(app) = app_id.map(str::trim).filter(|s| !s.is_empty()) {
        let app = app.replace('\\', "/").trim_start_matches('/').to_string();
        let prefixed = format!("{app}/{target}");
        if rel == prefixed {
            return true;
        }
    }
    false
}

/// `read_file` 在 workspace sanitize 之后、磁盘读取之前的业务白名单。
pub(crate) fn read_file_allowed_for_agent(
    rel: &str,
    app_id: Option<&str>,
    scope: &AgentResourceScope,
) -> bool {
    let rel_cmp = norm_rel_for_read_compare(rel, app_id);
    let vis = scope.resource_visibility;
    let target = scope.target_file.as_deref().unwrap_or("").trim();
    if target.is_empty() && vis == ResourceVisibility::LocalOnly {
        return false;
    }
    match vis {
        ResourceVisibility::LocalOnly => paths_match_workspace_rel(&rel_cmp, target, app_id),
        ResourceVisibility::AllowDirectRefs => {
            scope.direct_ref_paths.contains(&rel_cmp)
        }
        ResourceVisibility::AllowSceneReachable => scope.scene_reachable_paths.contains(&rel_cmp),
    }
}

fn norm_opt_string(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn merged_target_norm(
    merged_target: &Option<String>,
    app_id: Option<&str>,
) -> Result<String, String> {
    let m_target = norm_opt_string(merged_target).ok_or_else(|| {
        "dataset scope: merged target_file is empty".to_string()
    })?;
    let app = app_id.unwrap_or("").trim();
    if app.is_empty() {
        return Err("dataset scope: app_id is required for reachability checks".to_string());
    }
    norm_workspace_rel(&m_target, app).ok_or_else(|| {
        format!("dataset scope: invalid merged target_file `{m_target}`")
    })
}

/// 校验 `dataset_query` / `dataset_metric` 合并后的 `WorldScope` 是否仍满足 `resource_visibility` 与可达集。
pub(crate) fn validate_dataset_world_scope_merge(
    base: &WorldScope,
    merged: &WorldScope,
    vis: ResourceVisibility,
    scope: Option<&AgentResourceScope>,
    app_id: Option<&str>,
) -> Result<(), String> {
    let b_scene = norm_opt_string(&base.scene_id);
    let b_target = norm_opt_string(&base.target_file);
    let m_scene = norm_opt_string(&merged.scene_id);
    let m_target = norm_opt_string(&merged.target_file);

    match vis {
        ResourceVisibility::LocalOnly => {
            if b_scene != m_scene {
                return Err(format!(
                    "dataset scope: scene_id override not allowed in `{}` visibility (base={b_scene:?}, merged={m_scene:?})",
                    vis.as_slug()
                ));
            }
            if b_target != m_target {
                return Err(format!(
                    "dataset scope: target_file override not allowed in `{}` visibility (base={b_target:?}, merged={m_target:?})",
                    vis.as_slug()
                ));
            }
            Ok(())
        }
        ResourceVisibility::AllowDirectRefs | ResourceVisibility::AllowSceneReachable => {
            if b_scene.is_some() && m_scene != b_scene {
                return Err(
                    "dataset scope: scene_id must match the current request scope for this visibility"
                        .to_string(),
                );
            }
            if b_scene.is_none() && m_scene.is_some() {
                return Err(
                    "dataset scope: cannot introduce scene_id when the request baseline has none"
                        .to_string(),
                );
            }
            let merged_norm = merged_target_norm(&merged.target_file, app_id)?;
            let Some(rs) = scope else {
                return Err("dataset scope: internal scope snapshot missing".to_string());
            };
            let allowed = if vis == ResourceVisibility::AllowDirectRefs {
                rs.direct_ref_paths.contains(&merged_norm)
            } else {
                rs.scene_reachable_paths.contains(&merged_norm)
            };
            if !allowed {
                return Err(format!(
                    "dataset scope: merged target_file `{}` is not in the current `{}` reachability set for this scene/target",
                    merged_norm,
                    vis.as_slug()
                ));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::bridge::BridgePromptRequest;

    fn policy_access_ask() -> AgentModePolicy {
        AgentModePolicy {
            mode: AgentMode::Ask,
            route_mode: RouteMode::Access,
        }
    }

    fn policy_manage_build() -> AgentModePolicy {
        AgentModePolicy {
            mode: AgentMode::Build,
            route_mode: RouteMode::Manage,
        }
    }

    #[test]
    fn default_visibility_follows_route_and_mode() {
        assert_eq!(
            default_resource_visibility(policy_access_ask()),
            ResourceVisibility::AllowSceneReachable
        );
        assert_eq!(
            default_resource_visibility(policy_manage_build()),
            ResourceVisibility::AllowDirectRefs
        );
    }

    #[test]
    fn resolve_visibility_from_request_field() {
        let mut req = BridgePromptRequest {
            text: String::new(),
            app_id: Some("demo".into()),
            scene_id: None,
            target_file: None,
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("access".into()),
            agent: None,
            model: None,
            resource_visibility: Some("local_only".into()),
        };
        let vis = resolve_resource_visibility(&req, policy_access_ask());
        assert_eq!(vis, ResourceVisibility::LocalOnly);

        req.resource_visibility = Some("ALLOW_SCENE_REACHABLE".into());
        let vis2 = resolve_resource_visibility(&req, policy_access_ask());
        assert_eq!(vis2, ResourceVisibility::AllowSceneReachable);
    }

    #[test]
    fn read_file_local_only_matches_prefixed_target() {
        let scope = AgentResourceScope {
            scene_id: None,
            target_file: Some("main.mei".into()),
            resource_visibility: ResourceVisibility::LocalOnly,
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(HashSet::new()),
        };
        assert!(read_file_allowed_for_agent("demo/main.mei", Some("demo"), &scope));
        assert!(!read_file_allowed_for_agent("demo/other.mei", Some("demo"), &scope));
    }

    #[test]
    fn read_file_allow_direct_requires_membership() {
        let mut direct = HashSet::new();
        direct.insert("demo/data/x.mei".to_string());
        direct.insert("demo/main.mei".to_string());
        let scope = AgentResourceScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/main.mei".into()),
            resource_visibility: ResourceVisibility::AllowDirectRefs,
            direct_ref_paths: Arc::new(direct),
            scene_reachable_paths: Arc::new(HashSet::new()),
        };
        assert!(read_file_allowed_for_agent("demo/data/x.mei", Some("demo"), &scope));
        assert!(!read_file_allowed_for_agent("demo/unlisted.mei", Some("demo"), &scope));
        assert!(!read_file_allowed_for_agent("otherapp/x.mei", Some("demo"), &scope));
    }

    #[test]
    fn read_file_scene_reachable_uses_scene_set() {
        let mut scene = HashSet::new();
        scene.insert("demo/panels/a.mei".to_string());
        let scope = AgentResourceScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/main.mei".into()),
            resource_visibility: ResourceVisibility::AllowSceneReachable,
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(scene),
        };
        assert!(read_file_allowed_for_agent("demo/panels/a.mei", Some("demo"), &scope));
        assert!(!read_file_allowed_for_agent("demo/main.mei", Some("demo"), &scope));
    }

    #[test]
    fn dataset_merge_local_only_must_match() {
        let base = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("a.mei".into()),
        };
        let ok = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("a.mei".into()),
        };
        assert!(
            validate_dataset_world_scope_merge(
                &base,
                &ok,
                ResourceVisibility::LocalOnly,
                None,
                Some("demo")
            )
            .is_ok()
        );

        let bad = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("b.mei".into()),
        };
        assert!(
            validate_dataset_world_scope_merge(
                &base,
                &bad,
                ResourceVisibility::LocalOnly,
                None,
                Some("demo")
            )
            .is_err()
        );
    }

    #[test]
    fn dataset_merge_allow_refs_requires_reachability() {
        let base = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/a.mei".into()),
        };
        let merged = WorldScope {
            scene_id: Some("s1".into()),
            target_file: Some("demo/b.mei".into()),
        };
        let mut direct = HashSet::new();
        direct.insert("demo/b.mei".to_string());
        let scope = AgentResourceScope {
            scene_id: base.scene_id.clone(),
            target_file: base.target_file.clone(),
            resource_visibility: ResourceVisibility::AllowDirectRefs,
            direct_ref_paths: Arc::new(direct),
            scene_reachable_paths: Arc::new(HashSet::new()),
        };
        assert!(
            validate_dataset_world_scope_merge(
                &base,
                &merged,
                ResourceVisibility::AllowDirectRefs,
                Some(&scope),
                Some("demo")
            )
            .is_ok()
        );

        let bad_scope = AgentResourceScope {
            scene_id: base.scene_id.clone(),
            target_file: base.target_file.clone(),
            resource_visibility: ResourceVisibility::AllowDirectRefs,
            direct_ref_paths: Arc::new(HashSet::new()),
            scene_reachable_paths: Arc::new(HashSet::new()),
        };
        assert!(
            validate_dataset_world_scope_merge(
                &base,
                &merged,
                ResourceVisibility::AllowDirectRefs,
                Some(&bad_scope),
                Some("demo")
            )
            .is_err()
        );

        let bad_scene = WorldScope {
            scene_id: Some("s2".into()),
            target_file: Some("demo/b.mei".into()),
        };
        assert!(
            validate_dataset_world_scope_merge(
                &base,
                &bad_scene,
                ResourceVisibility::AllowDirectRefs,
                Some(&scope),
                Some("demo")
            )
            .is_err()
        );
    }
}
