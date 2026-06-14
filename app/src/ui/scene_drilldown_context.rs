use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;
use serde_json::json;

pub(crate) fn scene_drilldown_context_json(compiled: &CompiledApp) -> String {
    serde_json::to_string(&json!({
        "scene_local_nav_by_target": compiled.scene_local_nav_by_target,
        "scene_bindings_by_id": compiled.scene_bindings_by_id,
        "scene_examples_by_id": compiled.scene_examples_by_id,
        "scene_projection_assembly_by_id": compiled.scene_projection_assembly_by_id,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn scene_drilldown_context_script(compiled: &CompiledApp) -> AnyView {
    let payload = scene_drilldown_context_json(compiled);
    view! {
        <script
            id="mei-scene-drilldown-context"
            type="application/json"
            inner_html=payload
        ></script>
    }
    .into_any()
}
