use crate::http::scene_api::types::WorldScope;

pub(super) fn normalize_asset_kind(kind: Option<&str>) -> String {
    match kind.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if value == "entity" => "entity".to_string(),
        Some(value) if value == "resource" => "resource".to_string(),
        Some(value) if value == "cell" => "cell".to_string(),
        _ => "all".to_string(),
    }
}

pub(super) fn normalize_limit(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

pub(super) fn normalize_scope_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

pub(super) fn normalize_world_scope(scope: Option<&WorldScope>) -> WorldScope {
    WorldScope {
        scene_id: normalize_scope_field(scope.and_then(|item| item.scene_id.as_deref())),
        target_file: normalize_scope_field(scope.and_then(|item| item.target_file.as_deref())),
    }
}

/// 将请求里的 `target_file` 规范为「相对 app 根」的 `.mei` 路径（供 preview 编译与磁盘探测）。
/// 允许传入 workspace 相对路径 `{app_id}/data/...` 或仅用 `data/...`。
pub(crate) fn app_relative_mei_for_preview(app_id: &str, target_file: &str) -> Option<String> {
    let mut t = normalize_path(target_file);
    if !t.ends_with(".mei") {
        return None;
    }
    let prefix = format!("{}/", app_id.trim_end_matches('/'));
    if t.starts_with(&prefix) {
        t = t[prefix.len()..].to_string();
    }
    if t.is_empty() {
        return None;
    }
    Some(t)
}

pub(super) fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}
