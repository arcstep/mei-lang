//! `read_file` 业务白名单与 resource world 工具前置校验。

use crate::mei_agent::resource_tools::{AgentResourceScope, ResourceVisibility};

use super::paths::{norm_rel_for_read_compare, paths_match_workspace_rel};

/// `resource_list` / `resource_get` / `resource_runtime_peek` 与 `/world` 对齐的前置校验。
pub(crate) fn resource_world_tools_precheck(scope: &AgentResourceScope) -> Result<(), String> {
    match scope.resource_visibility {
        ResourceVisibility::LocalOnly => Err(
            "scope_denied: resource_list/resource_get/resource_runtime_peek are blocked under resource_visibility=local_only (aligned with /world)"
                .to_string(),
        ),
        _ => {
            if scope.world_injection_allowed_ids.is_none() {
                Err(
                    "scope_denied: missing world snapshot; cannot match world asset ids against inventory reachability"
                        .to_string(),
                )
            } else {
                Ok(())
            }
        }
    }
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
        ResourceVisibility::AllowDirectRefs => scope.direct_ref_paths.contains(&rel_cmp),
        ResourceVisibility::AllowSceneReachable => scope.scene_reachable_paths.contains(&rel_cmp),
    }
}
