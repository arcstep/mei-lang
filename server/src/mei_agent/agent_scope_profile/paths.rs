//! Workspace 相对路径规范化，与 `read_file` sanitize 后的形式对齐。

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

pub(crate) fn norm_rel_for_read_compare(rel: &str, app_id: Option<&str>) -> String {
    let rel = rel.trim().replace('\\', "/").trim_start_matches('/').to_string();
    let Some(app) = app_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return rel;
    };
    norm_workspace_rel(&rel, app).unwrap_or(rel)
}

pub(crate) fn paths_match_workspace_rel(rel: &str, target: &str, app_id: Option<&str>) -> bool {
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
