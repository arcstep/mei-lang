use super::*;

#[test]
fn aggregate_warmup_requests_derive_target_file_from_dataset_selector() {
    let app = RuntimeWarmupApp {
        app_id: "demo".to_string(),
        default_scene: Some("home".to_string()),
        hot_scenes: vec!["home".to_string()],
        scenes: Vec::new(),
        focuses: vec!["main.mei".to_string()],
        datasets: vec![RuntimeWarmupDatasetRequest {
            scene_id: Some("home".to_string()),
            focus: None,
            dataset_id: "__world_metrics__::scenes/10-地图.mei::metrics".to_string(),
            priority: None,
            metric_id: None,
            metric_ids: Vec::new(),
        }],
        xlsx_sources: Vec::new(),
    };
    let requests = aggregate_warmup_requests(&app, PrebuildScopeProfile::Full);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].scope.key(), "home|scenes/10-地图.mei");
}

#[test]
fn prebuild_warning_classifies_dataset_locate_failure() {
    let warning = build_prebuild_warning(
        "warmup_critical",
        Some("home"),
        Some("scenes/10-地图.mei"),
        None,
        None,
        None,
        None,
        "locate warmup dataset `__world_metrics__::scenes/10-地图.mei::metrics`".to_string(),
    );
    assert_eq!(warning.category, "warmup_dataset_locate_failed");
    assert_eq!(
        warning.dataset_selector.as_deref(),
        Some("__world_metrics__::scenes/10-地图.mei::metrics")
    );
    assert_eq!(warning.scene_id.as_deref(), Some("home"));
    assert_eq!(warning.target_file.as_deref(), Some("scenes/10-地图.mei"));
}

#[test]
fn requested_metric_ids_merge_scalar_and_list_fields() {
    let request = RuntimeWarmupDatasetRequest {
        scene_id: Some("home".to_string()),
        focus: None,
        dataset_id: "demo_ds".to_string(),
        priority: None,
        metric_id: Some("total".to_string()),
        metric_ids: vec!["delta".to_string(), "total".to_string()],
    };

    assert_eq!(
        requested_metric_ids(&request),
        vec!["delta".to_string(), "total".to_string()]
    );
}

#[test]
fn hot_only_warmup_requests_respect_explicit_deferred_priority() {
    let app = RuntimeWarmupApp {
        app_id: "demo".to_string(),
        default_scene: Some("home".to_string()),
        hot_scenes: vec!["home".to_string()],
        scenes: vec!["home".to_string()],
        focuses: vec!["main.mei".to_string()],
        datasets: vec![
            RuntimeWarmupDatasetRequest {
                scene_id: Some("home".to_string()),
                focus: Some("main.mei".to_string()),
                dataset_id: "critical_ds".to_string(),
                priority: Some("critical".to_string()),
                metric_id: Some("metric_a".to_string()),
                metric_ids: Vec::new(),
            },
            RuntimeWarmupDatasetRequest {
                scene_id: Some("home".to_string()),
                focus: Some("main.mei".to_string()),
                dataset_id: "heavy_ds".to_string(),
                priority: Some("deferred".to_string()),
                metric_id: Some("metric_b".to_string()),
                metric_ids: Vec::new(),
            },
        ],
        xlsx_sources: Vec::new(),
    };
    let requests = aggregate_warmup_requests(&app, PrebuildScopeProfile::HotOnly);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].dataset_id, "critical_ds");
    assert_eq!(requests[0].priority, WarmupRequestPriority::Critical);
}

#[test]
fn prebuild_dataframe_metric_selector_rewrites_scalar_metric_to_rowset() {
    let metric_defs = BTreeMap::from([(
        "scenes/01-执法要素.mei::enforcement_items_count".to_string(),
        serde_json::json!({
            "id": "scenes/01-执法要素.mei::enforcement_items_count",
            "shape": "scalar_map"
        }),
    )]);
    assert_eq!(
        prebuild_dataframe_metric_selector(
            &metric_defs,
            "scenes/01-执法要素.mei::enforcement_items_count"
        ),
        "scenes/01-执法要素.mei::enforcement_items_count::__scalar_rowset__"
    );
}

