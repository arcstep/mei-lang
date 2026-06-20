use mei_lang_kernel::{
    resolve_build_view_query, BuildExecScope, BuildNodeId, BuildViewTab, LegacyBuildQuery,
    ResolvedBuildViewQuery,
};

use super::UiRouteMode;

pub const OPS_CONFIG_TARGET: &str = ".mei-config.json";

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WorldSemanticQuery<'a> {
    pub world_metric: Option<&'a str>,
    pub world_dataset: Option<&'a str>,
    pub explain: Option<&'a str>,
}

impl WorldSemanticQuery<'_> {
    pub(crate) fn has_selection(self) -> bool {
        self.world_metric
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
            || self.world_dataset
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
    }
}

pub(crate) use mei_lang_kernel::BuildViewTab as ManageViewTab;

pub(crate) fn is_ops_config_target(target: &str) -> bool {
    target.trim() == OPS_CONFIG_TARGET
}

pub(crate) fn encode_query_value(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b))
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

pub(crate) fn resolve_build_query(
    node: Option<&str>,
    scope: Option<&str>,
    tab: Option<&str>,
    file: Option<&str>,
    scene: Option<&str>,
    world_metric: Option<&str>,
    world_dataset: Option<&str>,
    explain: Option<&str>,
) -> Option<ResolvedBuildViewQuery> {
    resolve_build_view_query(
        node,
        scope,
        tab,
        &LegacyBuildQuery {
            file: file.map(str::to_string),
            scene: scene.map(str::to_string),
            world_metric: world_metric.map(str::to_string),
            world_dataset: world_dataset.map(str::to_string),
            explain: explain.map(str::to_string),
            tab: tab.map(str::to_string),
        },
    )
}

pub(crate) fn build_node_href(
    app_path: &str,
    node: &BuildNodeId,
    tab: BuildViewTab,
    scope: BuildExecScope,
) -> String {
    let query = build_node_query_parts(node, tab, scope);
    if query.is_empty() {
        format!("/apps/build/{app_path}")
    } else {
        format!("/apps/build/{app_path}?{}", query.join("&"))
    }
}

pub(crate) fn build_node_query_parts(
    node: &BuildNodeId,
    tab: BuildViewTab,
    scope: BuildExecScope,
) -> Vec<String> {
    let mut query = vec![format!("node={}", encode_query_value(&node.encode()))];
    if tab != node.default_tab() {
        query.push(format!("tab={}", encode_query_value(tab.slug())));
    }
    if scope != BuildExecScope::Warmup {
        query.push(format!("scope={}", encode_query_value(scope.slug())));
    }
    query
}

/// Legacy wrapper for statusbar / diagnostics links.
pub(crate) fn manage_tab_href(
    app_path: &str,
    file_param: Option<&str>,
    selected_target: &str,
    _script_target: bool,
    tab: BuildViewTab,
    _diag_filter: Option<&str>,
    selected_scene: Option<&str>,
    semantic: WorldSemanticQuery<'_>,
) -> String {
    let resolved = resolve_build_query(
        None,
        None,
        Some(tab.slug()),
        file_param.or(Some(selected_target)),
        selected_scene,
        semantic.world_metric,
        semantic.world_dataset,
        semantic.explain,
    );
    if let Some(resolved) = resolved {
        return build_node_href(app_path, &resolved.node, resolved.tab, resolved.scope);
    }
    build_preview_href(
        app_path,
        file_param.or(Some(selected_target)),
        selected_scene,
        Some(tab.slug()),
        None,
        semantic,
    )
}

/// 构建视图预览链接（legacy + canonical node）。
pub(crate) fn build_preview_href(
    app_path: &str,
    file: Option<&str>,
    scene: Option<&str>,
    tab: Option<&str>,
    diag_filter: Option<&str>,
    semantic: WorldSemanticQuery<'_>,
) -> String {
    if let Some(resolved) = resolve_build_query(
        None,
        None,
        tab,
        file,
        scene,
        semantic.world_metric,
        semantic.world_dataset,
        semantic.explain,
    ) {
        return build_node_href(app_path, &resolved.node, resolved.tab, resolved.scope);
    }
    let query = build_preview_query_parts(file, scene, tab, diag_filter, semantic);
    if query.is_empty() {
        format!("/apps/build/{app_path}")
    } else {
        format!("/apps/build/{app_path}?{}", query.join("&"))
    }
}

pub(crate) fn build_preview_query_parts(
    file: Option<&str>,
    scene: Option<&str>,
    tab: Option<&str>,
    diag_filter: Option<&str>,
    semantic: WorldSemanticQuery<'_>,
) -> Vec<String> {
    if let Some(resolved) = resolve_build_query(
        None,
        None,
        tab,
        file,
        scene,
        semantic.world_metric,
        semantic.world_dataset,
        semantic.explain,
    ) {
        return build_node_query_parts(&resolved.node, resolved.tab, resolved.scope);
    }
    let mut query = Vec::new();
    if let Some(f) = file.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("file={}", encode_query_value(f)));
    }
    if let Some(sc) = scene.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("scene={}", encode_query_value(sc)));
    }
    if let Some(metric) = semantic
        .world_metric
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        query.push(format!("world_metric={}", encode_query_value(metric)));
    }
    if let Some(dataset) = semantic
        .world_dataset
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        query.push(format!("world_dataset={}", encode_query_value(dataset)));
    }
    if let Some(explain) = semantic.explain.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("explain={}", encode_query_value(explain)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("tab={}", encode_query_value(t)));
    }
    if let Some(filter) = diag_filter.map(str::trim).filter(|s| !s.is_empty()) {
        if filter.eq_ignore_ascii_case("all") {
            query.push("diag_filter=all".to_string());
        }
    }
    query
}

/// 访问态 canonical 路径后缀：`/scene/<id>?tab=…&chrome=…`（`scene_id` 经 `encode_query_value` 编码）。
pub(crate) fn access_scene_route_suffix(
    selected_scene: Option<&str>,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(sc) = selected_scene.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("/scene/");
        out.push_str(&encode_query_value(sc));
    }
    let mut q = Vec::new();
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        q.push(format!("tab={}", encode_query_value(t)));
    }
    if let Some(c) = chrome.map(str::trim).filter(|s| !s.is_empty()) {
        q.push(format!("chrome={}", encode_query_value(c)));
    }
    if !q.is_empty() {
        out.push('?');
        out.push_str(&q.join("&"));
    }
    out
}

pub(crate) fn access_scene_query(selected_scene: Option<&str>) -> String {
    access_scene_route_suffix(selected_scene, None, None)
}

#[allow(dead_code)]
pub(crate) fn route_query(
    route_mode: UiRouteMode,
    selected_scene: Option<&str>,
    _preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> String {
    if route_mode.uses_scene_route() {
        access_scene_route_suffix(selected_scene, active_tab, None)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_world_metric_resolves_to_node_href() {
        let href = build_preview_href(
            "zhifa",
            Some("metrics.world.mei"),
            None,
            Some("preview"),
            None,
            WorldSemanticQuery {
                world_metric: Some("total"),
                world_dataset: None,
                explain: None,
            },
        );
        assert!(href.contains("node=world-metric"));
        assert!(href.contains("tab=preview"));
    }
}
