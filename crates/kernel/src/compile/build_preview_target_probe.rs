#[cfg(test)]
mod tests {
    use crate::compile::build_experience::preview_target_from_build_node_with_app;
    use crate::model::BuildNodeId;
    use crate::CompiledApp;

    #[test]
    fn ws_hello_home_artifact_resolves_component_authoring_without_template_index() {
        let Some(ws) = (|| {
            let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
            let path = std::path::PathBuf::from(raw.trim());
            if path.as_os_str().is_empty() || !path.is_dir() {
                return None;
            }
            Some(path.canonicalize().unwrap_or(path))
        })() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        let artifact = ws.join(
            "apps/hello/build/active/artifacts/compiled_app/compiled_app__default-scene__default-target.json",
        );
        if !artifact.is_file() {
            eprintln!("skip: hello compiled_app artifact missing under MEI_TEST_WORKSPACE");
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
