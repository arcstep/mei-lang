//! Scene-qualified compile options and dataset/metric lookup for runtime APIs.

mod render;
mod route_parse;

#[allow(unused_imports)]
pub use render::{
    expected_scene_id_for_runtime_lookup, locate_dataset_resource, resolved_scene_context,
    ResolvedSceneContext,
};
#[allow(unused_imports)]
pub use route_parse::{
    compile_options_from_coords, strict_dataset_query_mode_contract, strict_runtime_query_contract,
    strict_scene_query_coords, SceneQueryCoords,
};

#[cfg(test)]
mod tests {
    use super::{
        locate_dataset_resource, strict_dataset_query_mode_contract, strict_runtime_query_contract,
        strict_scene_query_coords, SceneQueryCoords,
    };
    use mei_lang_kernel::{
        CompiledApp, CompiledSceneRoute, DatasetView, FilterIntent, FilterIntentSource,
        LoadedResource, QueryState, SourceDecl,
    };
    use serde_json::json;

    fn sample_dataset_resource(id: &str) -> LoadedResource {
        LoadedResource {
            id: id.to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: id.to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["a".to_string()],
                rows: vec![json!({"a": 1})],
                source: SourceDecl {
                    kind: "csv".to_string(),
                    path: format!("data/{id}.csv"),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: Default::default(),
                runtime_metric_defs: Default::default(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        }
    }

    fn sample_compiled() -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            resources: vec![
                sample_dataset_resource("warning_list"),
                sample_dataset_resource("home"),
            ],
            world_metrics: std::collections::BTreeMap::new(),
            world_semantic_by_file: std::collections::BTreeMap::new(),
            scene_routes: vec![CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: None,
                is_default: true,
                access_export: true,
            }],
            app_root: ".".to_string(),
            title: "demo".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: std::collections::BTreeMap::new(),
            scene_bindings_by_id: std::collections::BTreeMap::new(),
            scene_examples_by_id: std::collections::BTreeMap::new(),
            scene_projection_assembly_by_id: std::collections::BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn strict_scene_query_coords_rejects_target_only_requests() {
        let error =
            strict_scene_query_coords(None, Some("scenes/home.mei".to_string()), "dataset query")
                .expect_err("target-only requests should fail");
        let debug = format!("{error:?}");
        assert!(
            debug.contains("requires `scene_id`"),
            "unexpected error: {debug}"
        );
    }

    #[test]
    fn strict_scene_query_coords_accepts_scene_first_requests() {
        let coords = strict_scene_query_coords(
            Some("home".to_string()),
            Some("scenes/home.mei".to_string()),
            "dataset query",
        )
        .expect("scene-qualified requests should pass");
        assert_eq!(coords.scene_id.as_deref(), Some("home"));
        assert_eq!(coords.target.as_deref(), Some("scenes/home.mei"));
    }

    #[test]
    fn strict_runtime_query_contract_rejects_filters_without_query_state() {
        let mut filters = std::collections::BTreeMap::new();
        filters.insert("region".to_string(), "east".to_string());
        let error = strict_runtime_query_contract(&filters, None, None, &[], "dataset query")
            .expect_err("filters without query_state should fail");
        let debug = format!("{error:?}");
        assert!(
            debug.contains("requires `query_state`"),
            "unexpected error: {debug}"
        );
    }

    #[test]
    fn strict_runtime_query_contract_rejects_filter_intents_without_query_state() {
        let error = strict_runtime_query_contract(
            &std::collections::BTreeMap::new(),
            None,
            None,
            &[FilterIntent {
                dimension: "region".to_string(),
                operator: mei_lang_kernel::FilterOperator::Eq,
                value: "east".to_string(),
                source: FilterIntentSource::QueryState,
            }],
            "dataset query",
        )
        .expect_err("filter intents without query_state should fail");
        let debug = format!("{error:?}");
        assert!(
            debug.contains("requires `query_state`"),
            "unexpected error: {debug}"
        );
    }

    #[test]
    fn strict_runtime_query_contract_rejects_conflicting_filters() {
        let mut top_level_filters = std::collections::BTreeMap::new();
        top_level_filters.insert("region".to_string(), "east".to_string());
        let mut state_filters = std::collections::BTreeMap::new();
        state_filters.insert("region".to_string(), "west".to_string());
        let error = strict_runtime_query_contract(
            &top_level_filters,
            None,
            Some(&QueryState {
                filters: state_filters,
                search: None,
                group: Vec::new(),
                time_range: None,
            }),
            &[],
            "metric query",
        )
        .expect_err("conflicting filters should fail");
        let debug = format!("{error:?}");
        assert!(
            debug.contains("conflicting `filters`"),
            "unexpected error: {debug}"
        );
    }

    #[test]
    fn strict_dataset_query_mode_contract_rejects_filter_intents_for_plain_rows() {
        let error = strict_dataset_query_mode_contract(
            None,
            &[FilterIntent {
                dimension: "region".to_string(),
                operator: mei_lang_kernel::FilterOperator::Eq,
                value: "east".to_string(),
                source: FilterIntentSource::QueryState,
            }],
        )
        .expect_err("plain dataset rows should reject filter_intents");
        let debug = format!("{error:?}");
        assert!(
            debug.contains("plain dataset row queries do not accept `filter_intents`"),
            "unexpected error: {debug}"
        );
    }

    #[test]
    fn strict_dataset_query_mode_contract_allows_metric_dataframe_filter_intents() {
        strict_dataset_query_mode_contract(
            Some("summary_metric"),
            &[FilterIntent {
                dimension: "region".to_string(),
                operator: mei_lang_kernel::FilterOperator::Eq,
                value: "east".to_string(),
                source: FilterIntentSource::QueryState,
            }],
        )
        .expect("metric dataframe path should keep filter_intents");
    }

    #[test]
    fn locate_dataset_accepts_route_target_alias() {
        let compiled = sample_compiled();
        let resource = locate_dataset_resource(&compiled, "scenes/home.mei", None).expect("alias");
        assert_eq!(resource.id, "home");
    }

    #[test]
    fn locate_dataset_accepts_canonical_resource_id() {
        let compiled = sample_compiled();
        let coords = SceneQueryCoords::from_parts(Some("home".to_string()), None);
        let resource =
            locate_dataset_resource(&compiled, "warning_list", Some(&coords)).expect("id");
        assert_eq!(resource.id, "warning_list");
    }

    #[test]
    fn locate_dataset_allows_host_scene_id_when_target_matches_active_capsule() {
        let mut compiled = sample_compiled();
        compiled.active_scene = Some("enforcement_elements".to_string());
        compiled.active_target_file = "scenes/1_执法要素/执法要素.mei".to_string();
        compiled.resources.push(LoadedResource {
            id: "enforcement_units".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: "enforcement_units".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["a".to_string()],
                rows: vec![json!({"a": 1})],
                source: SourceDecl {
                    kind: "csv".to_string(),
                    path: "data/enforcement_units.csv".to_string(),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: Default::default(),
                runtime_metric_defs: Default::default(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        });
        let coords = SceneQueryCoords::from_parts(
            Some("home".to_string()),
            Some("scenes/1_执法要素/执法要素.mei".to_string()),
        );
        let resource = locate_dataset_resource(&compiled, "enforcement_units", Some(&coords))
            .expect("capsule");
        assert_eq!(resource.id, "enforcement_units");
    }
}
