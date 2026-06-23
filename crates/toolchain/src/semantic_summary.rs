use mei_lang_kernel::{CompiledApp, SceneContract};

#[derive(Debug, Clone)]
pub(crate) struct SemanticSignals {
    pub app_kind: String,
    pub semantic_tags: Vec<String>,
    pub business_explanation: String,
    pub scene_profile: Option<String>,
    pub scene_summary: Option<String>,
    pub scene_goal: Option<String>,
    pub panel_count: usize,
    pub flow_interaction_count: usize,
    pub flow_subject_timer_count: usize,
    pub has_timer: bool,
    pub frame_layout_type: Option<String>,
    pub route_count: usize,
    pub loaded_resource_count: usize,
    pub dataset_resource_count: usize,
    pub component_asset_count: usize,
    pub world_has_topology: bool,
}

fn push_tag(tags: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if !tags.iter().any(|item| item == &value) {
        tags.push(value);
    }
}

fn has_component_prefix(compiled: &CompiledApp, prefix: &str) -> bool {
    compiled
        .component_assets
        .iter()
        .any(|asset| asset.key.starts_with(prefix) || asset.tag.contains(prefix))
}

pub(crate) fn summarize_compiled_app_semantics(compiled: &CompiledApp) -> SemanticSignals {
    let contract: Option<&SceneContract> = compiled.scene_contract.as_ref();
    let scene_profile = contract
        .and_then(|contract| contract.scene.profile.as_deref())
        .map(str::to_string);
    let scene_summary = contract
        .and_then(|contract| contract.scene.summary.as_deref())
        .map(str::to_string);
    let scene_goal = contract
        .and_then(|contract| contract.scene.goal.as_deref())
        .map(str::to_string);
    let panel_count = contract.map(|contract| contract.panels.len()).unwrap_or(0);
    let flow_interaction_count = contract
        .and_then(|contract| contract.flow.as_ref())
        .map(|flow| flow.interactions.len())
        .unwrap_or(0);
    let flow_subject_timer_count = contract
        .and_then(|contract| contract.flow.as_ref())
        .map(|flow| flow.subject_timers.len())
        .unwrap_or(0);
    let has_timer = contract
        .and_then(|contract| contract.flow.as_ref())
        .is_some_and(|flow| flow.timer.is_some());
    let frame_layout_type = contract
        .and_then(|contract| contract.frame.as_ref())
        .and_then(|frame| frame.layout.as_ref())
        .map(|layout| layout.layout_type.clone());
    let world_has_topology = contract
        .and_then(|contract| contract.world.as_ref())
        .is_some_and(|world| world.topology.is_some());

    let route_count = compiled.scene_routes.len();
    let loaded_resource_count = compiled.resources.len();
    let dataset_resource_count = compiled
        .resources
        .iter()
        .filter(|resource| resource.dataset.is_some())
        .count();
    let document_resource_count = compiled
        .resources
        .iter()
        .filter(|resource| resource.document.is_some())
        .count();
    let component_asset_count = compiled.component_assets.len();
    let has_chart_components = has_component_prefix(compiled, "chart");
    let has_map_components = has_component_prefix(compiled, "map");

    let app_kind = if world_has_topology
        || flow_interaction_count > 0
        || flow_subject_timer_count > 0
        || has_timer
    {
        "simulation_app"
    } else if has_map_components {
        "geospatial_app"
    } else if has_chart_components || dataset_resource_count > 0 {
        "analytics_app"
    } else if route_count > 1 || panel_count >= 4 {
        "dashboard_app"
    } else if document_resource_count > 0 && dataset_resource_count == 0 {
        "document_app"
    } else {
        "scene_app"
    }
    .to_string();

    let mut semantic_tags = Vec::new();
    push_tag(&mut semantic_tags, format!("app_kind:{app_kind}"));
    if let Some(profile) = scene_profile.as_deref() {
        push_tag(&mut semantic_tags, format!("scene_profile:{profile}"));
    }
    if route_count > 1 {
        push_tag(&mut semantic_tags, "multi_route");
    }
    if dataset_resource_count > 0 {
        push_tag(&mut semantic_tags, "has_dataset_resources");
    }
    if document_resource_count > 0 {
        push_tag(&mut semantic_tags, "has_document_resources");
    }
    if has_chart_components {
        push_tag(&mut semantic_tags, "uses_chart_components");
    }
    if has_map_components {
        push_tag(&mut semantic_tags, "uses_map_components");
    }
    if world_has_topology {
        push_tag(&mut semantic_tags, "world_topology_grid");
    }
    if flow_interaction_count > 0 {
        push_tag(&mut semantic_tags, "has_click_interactions");
    }
    if flow_subject_timer_count > 0 || has_timer {
        push_tag(&mut semantic_tags, "has_timers");
    }
    if panel_count >= 4 {
        push_tag(&mut semantic_tags, "multi_panel_surface");
    }
    if component_asset_count > 0 {
        push_tag(&mut semantic_tags, "uses_platform_components");
    }

    let mut parts = vec![format!(
        "`{}` 更接近 `{}`，当前 active_scene=`{}`，routes={}",
        compiled.title,
        app_kind,
        compiled.active_scene.as_deref().unwrap_or("-"),
        route_count
    )];
    if let Some(profile) = scene_profile.as_deref() {
        parts.push(format!("scene profile=`{profile}`"));
    }
    if panel_count > 0 {
        parts.push(format!("panel_count={panel_count}"));
    }
    if dataset_resource_count > 0 || loaded_resource_count > 0 {
        parts.push(format!(
            "loaded_resources={} (datasets={})",
            loaded_resource_count, dataset_resource_count
        ));
    }
    if component_asset_count > 0 {
        parts.push(format!("component_assets={component_asset_count}"));
    }
    if world_has_topology {
        parts.push("world 带 topology/grid".to_string());
    }
    if flow_interaction_count > 0 || flow_subject_timer_count > 0 || has_timer {
        parts.push(format!(
            "flow(interactions={}, subject_timers={}, timer={})",
            flow_interaction_count, flow_subject_timer_count, has_timer
        ));
    }
    if let Some(layout_type) = frame_layout_type.as_deref() {
        parts.push(format!("frame_layout={layout_type}"));
    }
    let business_explanation = format!("{}。", parts.join("，"));

    SemanticSignals {
        app_kind,
        semantic_tags,
        business_explanation,
        scene_profile,
        scene_summary,
        scene_goal,
        panel_count,
        flow_interaction_count,
        flow_subject_timer_count,
        has_timer,
        frame_layout_type,
        route_count,
        loaded_resource_count,
        dataset_resource_count,
        component_asset_count,
        world_has_topology,
    }
}
