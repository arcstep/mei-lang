use mei_lang_kernel::CompiledApp;

pub fn load_compiled_for_warmup(
    scope_key: &str,
    owner_resource_id: &str,
    bundle_key: &str,
) -> anyhow::Result<CompiledApp> {
    let _ = (owner_resource_id, bundle_key);
    Ok(CompiledApp {
        app_id: "warmup".to_string(),
        title: "warmup".to_string(),
        app_root: String::new(),
        active_scene: Some(scope_key.to_string()),
        active_target_file: String::new(),
        scene_routes: Vec::new(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: Vec::new(),
        world_metrics: Default::default(),
        world_semantic_by_file: Default::default(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
    })
}

pub fn eval_metric_ids(
    _compiled: &CompiledApp,
    metric_ids: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    Ok(metric_ids
        .iter()
        .map(|id| (id.clone(), format!("placeholder:{id}")))
        .collect())
}
