#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::CompiledApp;

    fn compiled_with_scene(active_scene: Option<&str>) -> CompiledApp {
        CompiledApp {
            app_id: "zhifa".to_string(),
            title: "zhifa".to_string(),
            app_root: "/tmp/zhifa".to_string(),
            scene_routes: Vec::new(),
            active_scene: active_scene.map(str::to_string),
            active_target_file: "scenes/home.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
        }
    }

    #[test]
    fn artifact_matches_compile_scene_request_requires_bound_scene() {
        let options = CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        };
        assert!(artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(Some("home"))
        ));
        assert!(!artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(None)
        ));
        assert!(!artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(Some("other"))
        ));
    }

    #[test]
    fn artifact_matches_compile_scene_request_allows_full_app_lookup() {
        let options = CompileOptions::default();
        assert!(artifact_matches_compile_scene_request(
            &options,
            &compiled_with_scene(None)
        ));
    }

    #[test]
    fn artifact_matches_compile_scene_request_rejects_hydrated_export_board_from_parent_scene() {
        let options = CompileOptions {
            scene: Some("ai_warning_cockpit_board".to_string()),
            preview_target: Some("scenes/02-行政检查.board.mei".to_string()),
        };
        let mut compiled = compiled_with_scene(Some("home"));
        compiled.active_target_file = "scenes/02-行政检查.board.mei".to_string();
        compiled.scene_projection_assembly_by_id.insert(
            "ai_warning_cockpit_board".to_string(),
            Value::Object(Default::default()),
        );
        assert!(!artifact_matches_compile_scene_request(&options, &compiled));
    }

    #[test]
    fn artifact_matches_compile_scene_request_accepts_dedicated_export_board_compile() {
        let options = CompileOptions {
            scene: Some("ai_warning_cockpit_board".to_string()),
            preview_target: Some("scenes/02-行政检查.board.mei".to_string()),
        };
        let mut compiled = compiled_with_scene(Some("ai_warning_cockpit_board"));
        compiled.active_target_file = "scenes/02-行政检查.board.mei".to_string();
        assert!(artifact_matches_compile_scene_request(&options, &compiled));
    }

    #[test]
    fn artifact_matches_compile_scene_request_rejects_hydrated_parent_without_active_scene() {
        let options = CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        };
        let mut compiled = compiled_with_scene(None);
        compiled.active_target_file = "scenes/01-执法要素.mei".to_string();
        compiled.scene_projection_assembly_by_id.insert(
            "home".to_string(),
            serde_json::Value::Object(Default::default()),
        );
        assert!(!artifact_matches_compile_scene_request(&options, &compiled));
    }

    #[test]
    fn artifact_matches_compile_scene_request_rejects_wrong_active_target() {
        let options = CompileOptions {
            scene: Some("enforcement_units_analytics_board".to_string()),
            preview_target: Some("scenes/01-执法要素.board.mei".to_string()),
        };
        let mut compiled = compiled_with_scene(Some("home"));
        compiled.active_target_file = "scenes/home.mei".to_string();
        compiled.scene_projection_assembly_by_id.insert(
            "enforcement_units_analytics_board".to_string(),
            Value::Object(Default::default()),
        );

        assert!(!artifact_matches_compile_scene_request(&options, &compiled));
    }
}
