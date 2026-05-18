/// 访问态 URL 中 `/scene/<id>` 的固定标记（`scene_id` 为单段，可含百分号编码）。
pub(crate) const ACCESS_SCENE_PATH_MARK: &str = "/scene/";

#[derive(Debug, serde::Deserialize)]
pub struct AppQuery {
    /// 仅管理态：当前打开的源码/资源路径（相对 app 根）。兼容旧链接 `target=`。
    /// 访问态禁止携带：若出现则 307 重定向到剥离 `file`/`target` 后的 URL（发布面不得深链内部路径）。
    #[serde(default, alias = "target")]
    pub file: Option<String>,
    pub scene: Option<String>,
    pub tab: Option<String>,
    pub chrome: Option<String>,
}

fn percent_encode_query_component(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b));
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 从 `/apps/access/<app_path>/scene/<scene_id>` 形态解析 app 与 scene。
/// `scene_id` 必须为单路径段（不含未编码的 `/`）；路径不含 `/scene/` 时返回 `None`。
/// 含 `/scene/` 但格式非法时返回 `Some(Err)` 以便与「普通 app 路径」区分。
pub(crate) fn parse_access_scene_path(
    raw_app_path: &str,
) -> Result<Option<(String, String)>, ()> {
    let raw = raw_app_path.trim_start_matches('/');
    if !raw.contains(ACCESS_SCENE_PATH_MARK) {
        return Ok(None);
    }
    let Some(idx) = raw.find(ACCESS_SCENE_PATH_MARK) else {
        return Ok(None);
    };
    let app = raw[..idx].trim_end_matches('/').to_string();
    let scene = raw[idx + ACCESS_SCENE_PATH_MARK.len()..].trim_matches('/').to_string();
    if app.is_empty() || scene.is_empty() || scene.contains('/') {
        return Err(());
    }
    Ok(Some((app, scene)))
}

/// 访问态 canonical：`/apps/access/<app>/scene/<scene_id>?tab=…&chrome=…`（不再用 `?scene=` 作为主定位）。
pub(crate) fn access_canonical_location(
    app_id: &str,
    scene_id: &str,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> String {
    let sid = scene_id.trim();
    let mut out = format!(
        "/apps/access/{}{ACCESS_SCENE_PATH_MARK}{}",
        app_id.trim_start_matches('/'),
        percent_encode_query_component(sid)
    );
    let mut parts = Vec::new();
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", percent_encode_query_component(t)));
    }
    if let Some(c) = chrome.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("chrome={}", percent_encode_query_component(c)));
    }
    if !parts.is_empty() {
        out.push('?');
        out.push_str(&parts.join("&"));
    }
    out
}

/// 访问态允许的 query：`tab`、`chrome`（不含 `file`/`target`；`scene` 已收口到 path）。
pub(crate) fn access_sanitized_redirect_location(app_id: &str, query: &AppQuery) -> String {
    if let Some(scene) = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return access_canonical_location(app_id, scene, query.tab.as_deref(), query.chrome.as_deref());
    }
    let mut parts = Vec::new();
    if let Some(tab) = query
        .tab
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("tab={}", percent_encode_query_component(tab)));
    }
    if let Some(chrome) = query
        .chrome
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("chrome={}", percent_encode_query_component(chrome)));
    }
    if parts.is_empty() {
        format!("/apps/access/{app_id}")
    } else {
        format!("/apps/access/{app_id}?{}", parts.join("&"))
    }
}
