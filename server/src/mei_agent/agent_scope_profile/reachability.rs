//! 由 world 快照或请求回退构造的可达路径集合。

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::agent_runtime::bridge::BridgePromptRequest;
use crate::http::scene_api::WorldContextSnapshot;

use super::paths::norm_workspace_rel;

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

    /// 无快照时的保守回退：优先 scene 锚点，再纳入 source-focus `target_file`。
    pub(crate) fn fallback_from_request_target(
        request: &BridgePromptRequest,
        app_id: &str,
    ) -> Self {
        let mut direct = HashSet::new();
        let mut scene = HashSet::new();
        let app = app_id.trim();
        if let Some(raw) = request
            .scene_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let pseudo = format!("{app}/scenes/{raw}.mei");
            if let Some(p) = norm_workspace_rel(&pseudo, app) {
                scene.insert(p);
            }
        }
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
