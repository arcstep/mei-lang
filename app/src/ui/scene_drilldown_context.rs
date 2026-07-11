use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;
use serde_json::{json, Value};

use super::preview::host_runtime_capabilities_value;

pub fn scene_drilldown_context_json_for_host_ssr(
    compiled: &CompiledApp,
    preview_scene_id: Option<&str>,
) -> String {
    let mut assembly = compiled.scene_projection_assembly_by_id.clone();
    if let Some(scene_id) = preview_scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(Value::Object(entry)) = assembly.get_mut(scene_id) {
            entry.remove("panels");
        }
    }
    serde_json::to_string(&json!({
        "scene_local_nav_by_target": compiled.scene_local_nav_by_target,
        "scene_bindings_by_id": compiled.scene_bindings_by_id,
        "scene_examples_by_id": compiled.scene_examples_by_id,
        "scene_projection_assembly_by_id": assembly,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

pub fn host_runtime_capabilities_json(app_path: &str, data_mode: Option<&str>) -> String {
    serde_json::to_string(&host_runtime_capabilities_value(app_path, data_mode))
        .unwrap_or_else(|_| "{}".to_string())
}

pub fn scene_drilldown_artifact_public_url(app_id: &str, scene_id: &str) -> String {
    format!("/api/host/scene-drilldown-context?app={app_id}&scene={scene_id}")
}

fn html_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

/// Thin-shell head: drilldown via API meta; runtime capabilities stay inline (~2KB).
pub fn render_host_ssr_bootstrap_head_revision_only(
    _compiled: &CompiledApp,
    app_path: &str,
    app_id: &str,
    preview_scene_id: Option<&str>,
    data_mode: Option<&str>,
) -> String {
    let artifact_url =
        scene_drilldown_artifact_public_url(app_id, preview_scene_id.unwrap_or("home"));
    let runtime_payload = host_runtime_capabilities_json(app_path, data_mode);
    let scope = html_escape_attr(preview_scene_id.unwrap_or("home"));
    let app_attr = html_escape_attr(app_id);
    let url_attr = html_escape_attr(artifact_url.as_str());
    format!(
        concat!(
            r#"<meta name="mei-drilldown-inlined" content="0" />"#,
            r#"<meta name="mei-drilldown-scope" content="{scope}" />"#,
            r#"<meta name="mei-drilldown-app-id" content="{app_attr}" />"#,
            r#"<meta name="mei-drilldown-artifact-url" content="{url_attr}" />"#,
            r#"<script id="mei-host-runtime-capabilities" type="application/json">"#,
            "{runtime_payload}",
            r#"</script>"#
        ),
        scope = scope,
        app_attr = app_attr,
        url_attr = url_attr,
        runtime_payload = runtime_payload,
    )
}

pub(crate) fn host_ssr_bootstrap_scripts(
    compiled: &CompiledApp,
    app_path: &str,
    preview_scene_id: Option<&str>,
    data_mode: Option<&str>,
) -> AnyView {
    let drilldown_payload = scene_drilldown_context_json_for_host_ssr(compiled, preview_scene_id);
    let runtime_payload = host_runtime_capabilities_json(app_path, data_mode);
    view! {
        <script
            id="mei-scene-drilldown-context"
            type="application/json"
            inner_html=drilldown_payload
        ></script>
        <script
            id="mei-host-runtime-capabilities"
            type="application/json"
            inner_html=runtime_payload
        ></script>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeMap;

    fn sample_assembly() -> BTreeMap<String, Value> {
        BTreeMap::from([
            (
                "home".to_string(),
                json!({
                    "scene_id": "home",
                    "panels": [{"id": "heavy"}],
                    "bindings": {},
                }),
            ),
            (
                "detail_board".to_string(),
                json!({
                    "scene_id": "detail_board",
                    "panels": [{"id": "light"}],
                }),
            ),
        ])
    }

    #[test]
    fn host_ssr_drilldown_context_strips_preview_scene_panels_only() {
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            scene_routes: Vec::new(),
            app_root: ".".to_string(),
            title: "demo".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: sample_assembly(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        };
        let payload: Value = serde_json::from_str(&scene_drilldown_context_json_for_host_ssr(
            &compiled,
            Some("home"),
        ))
        .expect("valid json");
        let assembly = payload
            .get("scene_projection_assembly_by_id")
            .and_then(Value::as_object)
            .expect("assembly map");
        assert!(assembly.get("home").and_then(|v| v.get("panels")).is_none());
        assert!(assembly
            .get("detail_board")
            .and_then(|v| v.get("panels"))
            .is_some());
    }
}
