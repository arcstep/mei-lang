use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;
use serde_json::{json, Value};

use super::preview::host_runtime_capabilities_value;

pub(crate) fn scene_drilldown_context_json_for_host_ssr(
    compiled: &CompiledApp,
    preview_scene_id: Option<&str>,
) -> String {
    let mut assembly = compiled.scene_projection_assembly_by_id.clone();
    if let Some(scene_id) = preview_scene_id.map(str::trim).filter(|value| !value.is_empty()) {
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

pub(crate) fn host_runtime_capabilities_json(app_path: &str) -> String {
    serde_json::to_string(&host_runtime_capabilities_value(app_path))
        .unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn host_ssr_bootstrap_scripts(
    compiled: &CompiledApp,
    app_path: &str,
    preview_scene_id: Option<&str>,
) -> AnyView {
    let drilldown_payload = scene_drilldown_context_json_for_host_ssr(compiled, preview_scene_id);
    let runtime_payload = host_runtime_capabilities_json(app_path);
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
    use std::collections::BTreeMap;
    use serde_json::Value;

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
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let payload: Value =
            serde_json::from_str(&scene_drilldown_context_json_for_host_ssr(&compiled, Some("home")))
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
