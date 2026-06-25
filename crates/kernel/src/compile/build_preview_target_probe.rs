#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::compile::build_experience::preview_target_from_build_node_with_app;
    use crate::model::BuildNodeId;
    use crate::CompiledApp;

    #[test]
    fn ws_hello_home_artifact_resolves_component_authoring_without_template_index() {
        let artifact = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("workspaces/ws-hello/apps/hello/build/active/artifacts/compiled_app/compiled_app__default-scene__default-target.json");
        if !artifact.is_file() {
            return;
        }
        let text = std::fs::read_to_string(&artifact).expect("artifact json");
        let value: serde_json::Value = serde_json::from_str(&text).expect("parse artifact");
        let compiled: CompiledApp =
            serde_json::from_value(value.get("compiled").cloned().expect("compiled field"))
                .expect("deserialize compiled app");
        assert!(
            compiled.build_template_index.templates.is_empty(),
            "fixture expects empty template index on home artifact"
        );
        let node = BuildNodeId::component("chart.area");
        let preview = preview_target_from_build_node_with_app(&node, Some(&compiled))
            .expect("component authoring preview target");
        assert!(
            preview.contains("chart-baseline.mei"),
            "expected chart baseline example, got {preview}"
        );
    }
}
