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

/// 访问态允许的 query：`scene`、`tab`、`chrome`（不含 `file`/`target`）。
pub(crate) fn access_sanitized_redirect_location(app_id: &str, query: &AppQuery) -> String {
    let mut parts = Vec::new();
    if let Some(scene) = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("scene={}", percent_encode_query_component(scene)));
    }
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
