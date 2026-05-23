//! `dataset_query` / `dataset_metric` 的 `WorldScope` 合并校验。

use crate::http::scene_api::WorldScope;

use crate::mei_agent::resource_tools::{AgentResourceScope, ResourceVisibility};

use super::paths::norm_workspace_rel;

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
    let m_target = norm_opt_string(merged_target)
        .ok_or_else(|| "dataset scope: merged target_file is empty".to_string())?;
    let app = app_id.unwrap_or("").trim();
    if app.is_empty() {
        return Err("dataset scope: app_id is required for reachability checks".to_string());
    }
    norm_workspace_rel(&m_target, app)
        .ok_or_else(|| format!("dataset scope: invalid merged target_file `{m_target}`"))
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
