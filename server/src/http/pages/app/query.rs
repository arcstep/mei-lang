use mei_lang_app::UiRouteMode;

/// 应用视图 URL 中 `/scene/<id>` 的固定标记（`scene_id` 为单段，可含百分号编码）。
pub(crate) const ACCESS_SCENE_PATH_MARK: &str = "/scene/";

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct AppQuery {
    /// 构建/上传视图：当前打开的源码/资源路径（相对 app 根）。兼容旧链接 `target=`。
    /// 应用视图禁止携带脚本 `file`：若出现则 307 重定向到剥离 `file`/`target` 后的 URL。
    #[serde(default, alias = "target")]
    pub file: Option<String>,
    pub scene: Option<String>,
    pub tab: Option<String>,
    /// 构建视图调试页：编译诊断范围。`all` = 全部诊断；缺省或其它 = 当前文件。
    pub diag_filter: Option<String>,
    pub world_metric: Option<String>,
    pub world_dataset: Option<String>,
    pub explain: Option<String>,
    /// 构建视图 canonical 节点坐标（BuildNodeId 编码）。
    pub node: Option<String>,
    /// 构建视图 exec tab scope：warmup / empty / last_request / custom。
    pub scope: Option<String>,
    /// 构建视图预览细粒度锚点（scene-block 编码）；与 `node` 独立，不强制改左树 selection。
    pub focus: Option<String>,
    /// Stock catalog facet when `_stock-catalog` has split topbar entries: `components` | `templates`.
    pub catalog: Option<String>,
    /// Component pack or template folder within the catalog facet.
    pub pack: Option<String>,
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

fn projection_base_path(route_mode: UiRouteMode, app_id: &str) -> String {
    let mode = match route_mode {
        UiRouteMode::App => "app",
        UiRouteMode::Presentation => "presentation",
        _ => "app",
    };
    format!("/apps/{mode}/{}", app_id.trim_start_matches('/'))
}

fn projection_query_parts(
    route_mode: UiRouteMode,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> Vec<String> {
    if route_mode != UiRouteMode::App {
        return Vec::new();
    }
    let mut parts = Vec::new();
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", percent_encode_query_component(t)));
    }
    if let Some(c) = chrome.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("chrome={}", percent_encode_query_component(c)));
    }
    parts
}

/// 从 `/apps/app/<app_path>/scene/<scene_id>` 或 `/apps/presentation/<app_path>/scene/<scene_id>` 形态解析 app 与 scene。
pub(crate) fn parse_access_scene_path(raw_app_path: &str) -> Result<Option<(String, String)>, ()> {
    let raw = raw_app_path.trim_start_matches('/');
    if !raw.contains(ACCESS_SCENE_PATH_MARK) {
        return Ok(None);
    }
    let Some(idx) = raw.find(ACCESS_SCENE_PATH_MARK) else {
        return Ok(None);
    };
    let app = raw[..idx].trim_end_matches('/').to_string();
    let scene = raw[idx + ACCESS_SCENE_PATH_MARK.len()..]
        .trim_matches('/')
        .to_string();
    if app.is_empty() || scene.is_empty() || scene.contains('/') {
        return Err(());
    }
    Ok(Some((app, scene)))
}

pub(crate) fn scene_projection_canonical_location(
    route_mode: UiRouteMode,
    app_id: &str,
    scene_id: &str,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> String {
    let sid = scene_id.trim();
    let mut out = format!(
        "{}{ACCESS_SCENE_PATH_MARK}{}",
        projection_base_path(route_mode, app_id),
        percent_encode_query_component(sid)
    );
    let parts = projection_query_parts(route_mode, tab, chrome);
    if !parts.is_empty() {
        out.push('?');
        out.push_str(&parts.join("&"));
    }
    out
}

/// 应用视图 canonical：`/apps/app/<app>/scene/<scene_id>?tab=…&chrome=…`
pub(crate) fn access_canonical_location(
    app_id: &str,
    scene_id: &str,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> String {
    scene_projection_canonical_location(UiRouteMode::App, app_id, scene_id, tab, chrome)
}

pub(crate) fn scene_projection_sanitized_redirect_location(
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
) -> String {
    if let Some(scene) = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return scene_projection_canonical_location(
            route_mode,
            app_id,
            scene,
            query.tab.as_deref(),
            query.chrome.as_deref(),
        );
    }
    let parts = projection_query_parts(route_mode, query.tab.as_deref(), query.chrome.as_deref());
    let base = projection_base_path(route_mode, app_id);
    if parts.is_empty() {
        base
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}

/// 应用视图允许的 query：`tab`、`chrome`（不含脚本 `file`/`target`）。
pub(crate) fn access_sanitized_redirect_location(app_id: &str, query: &AppQuery) -> String {
    scene_projection_sanitized_redirect_location(UiRouteMode::App, app_id, query)
}

pub(crate) fn presentation_sanitized_redirect_location(app_id: &str, query: &AppQuery) -> String {
    scene_projection_sanitized_redirect_location(UiRouteMode::Presentation, app_id, query)
}

fn build_query_suffix(query: &AppQuery) -> String {
    build_query_suffix_with_options(query, BuildQuerySuffixOptions::default())
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BuildQuerySuffixOptions {
    pub include_node: bool,
    pub include_scope: bool,
    pub include_focus: bool,
}

pub(crate) fn build_query_suffix_with_options(
    query: &AppQuery,
    options: BuildQuerySuffixOptions,
) -> String {
    let mut parts = Vec::new();
    if let Some(file) = query
        .file
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("file={}", percent_encode_query_component(file)));
    }
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
    if let Some(filter) = query
        .diag_filter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!(
            "diag_filter={}",
            percent_encode_query_component(filter)
        ));
    }
    for (key, value) in [
        ("world_metric", query.world_metric.as_deref()),
        ("world_dataset", query.world_dataset.as_deref()),
        ("explain", query.explain.as_deref()),
    ] {
        if let Some(value) = value.map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("{key}={}", percent_encode_query_component(value)));
        }
    }
    if options.include_node {
        if let Some(node) = query.node.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("node={}", percent_encode_query_component(node)));
        }
    }
    if options.include_scope {
        if let Some(scope) = query.scope.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("scope={}", percent_encode_query_component(scope)));
        }
    }
    if options.include_focus {
        if let Some(focus) = query.focus.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("focus={}", percent_encode_query_component(focus)));
        }
    }
    if let Some(catalog) = query
        .catalog
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("catalog={}", percent_encode_query_component(catalog)));
    }
    if let Some(pack) = query
        .pack
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(format!("pack={}", percent_encode_query_component(pack)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// 旧 `/apps/access/...` → `/apps/app/...`
pub(crate) fn legacy_access_redirect_location(
    app_id_raw: &str,
    query: &AppQuery,
) -> Option<String> {
    let (app_id, scene) = match parse_access_scene_path(app_id_raw) {
        Ok(Some((app, scene))) => (app, Some(scene)),
        Ok(None) => (app_id_raw.trim_start_matches('/').to_string(), None),
        Err(()) => return None,
    };
    if let Some(scene_id) = scene {
        return Some(access_canonical_location(
            &app_id,
            &scene_id,
            query.tab.as_deref(),
            query.chrome.as_deref(),
        ));
    }
    Some(access_sanitized_redirect_location(&app_id, query))
}

/// 旧 `/apps/manage/...` → `/apps/build/...` 或 `/apps/config/...`
pub(crate) fn legacy_manage_redirect_location(app_id: &str, query: &AppQuery) -> String {
    let app = app_id.trim_start_matches('/');
    if query
        .file
        .as_deref()
        .map(str::trim)
        .is_some_and(|f| f == ".mei-config.json")
    {
        return format!("/apps/config/{app}");
    }
    format!("/apps/build/{app}{}", build_query_suffix(query))
}
