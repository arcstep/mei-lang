//! World inventory 可达分层与 `/world` 注入 id 集合。

use std::collections::HashSet;

use crate::http::scene_api::{ResourceInventoryItem, WorldContextSnapshot};
use crate::mei_agent::resource_tools::{AgentResourceScope, ResourceVisibility};

use super::paths::norm_workspace_rel;

/// 资源 inventory 条目的可达分层，供前端展示与 `/world` 注入裁剪对齐。
pub(crate) fn resource_inventory_reach_tier(
    item: &ResourceInventoryItem,
    rs: &AgentResourceScope,
    app_id: &str,
) -> &'static str {
    if let Some(sp) = item
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Some(p) = norm_workspace_rel(sp, app_id) {
            if rs.direct_ref_paths.contains(&p) {
                return "direct";
            }
            if rs.scene_reachable_paths.contains(&p) {
                return "scene";
            }
        }
    }
    "other"
}

/// `/world` 与工具链对齐：仅允许把当前 `resource_visibility` 下可达的 inventory 条目注入模型。
pub(crate) fn world_injection_inventory_item_allowed(
    item: &ResourceInventoryItem,
    vis: ResourceVisibility,
    rs: &AgentResourceScope,
    app_id: &str,
) -> bool {
    match vis {
        ResourceVisibility::LocalOnly => false,
        ResourceVisibility::AllowDirectRefs => {
            resource_inventory_reach_tier(item, rs, app_id) == "direct"
        }
        ResourceVisibility::AllowSceneReachable => {
            matches!(
                resource_inventory_reach_tier(item, rs, app_id),
                "direct" | "scene"
            )
        }
    }
}

/// 当前请求下允许出现在 `/world` 注入 JSON 中的 inventory `id` 集合（由可达规则推导）。
pub(crate) fn allowed_world_injection_inventory_ids(
    snapshot: &WorldContextSnapshot,
    vis: ResourceVisibility,
    rs: &AgentResourceScope,
    app_id: &str,
) -> HashSet<String> {
    snapshot
        .resource_inventory
        .items
        .iter()
        .filter(|it| world_injection_inventory_item_allowed(it, vis, rs, app_id))
        .map(|it| it.id.clone())
        .collect()
}
