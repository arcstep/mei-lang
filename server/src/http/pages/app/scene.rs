use mei_lang_kernel::CompiledApp;

/// 若 URL `scene` 在应用路由表中不存在（编译已回退并带 `unknown_scene` 警告），用 `compiled.active_scene` 生成管理壳链接，避免把无效 id 写进 href。
pub(super) fn manage_scene_for_render(
    compiled: &CompiledApp,
    query_scene: Option<&str>,
) -> Option<String> {
    let q = query_scene?.trim();
    if q.is_empty() {
        return None;
    }
    if compiled.scene_routes.iter().any(|r| r.scene_id == q) {
        return Some(q.to_string());
    }
    compiled.active_scene.clone()
}

/// 给定已解析的 scene id（或 `None`），返回该场景路由对应的主文件路径；无匹配时回退 `active_target_file`。
pub(super) fn default_file_for_scene(compiled: &CompiledApp, scene_id: Option<&str>) -> String {
    let sid = scene_id.unwrap_or("").trim();
    if sid.is_empty() {
        return compiled.active_target_file.clone();
    }
    compiled
        .scene_routes
        .iter()
        .find(|r| r.scene_id == sid)
        .map(|r| r.target_file.clone())
        .unwrap_or_else(|| compiled.active_target_file.clone())
}

/// 若目标文件本身就是某条 scene route 的主文件，则返回该 route 的 scene id。
pub(super) fn canonical_scene_for_target(
    compiled: &CompiledApp,
    target_file: Option<&str>,
) -> Option<String> {
    let target_file = target_file?.trim();
    if target_file.is_empty() {
        return None;
    }
    compiled
        .scene_routes
        .iter()
        .find(|r| r.target_file == target_file)
        .map(|r| r.scene_id.clone())
}
