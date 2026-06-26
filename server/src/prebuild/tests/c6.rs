use super::*;

#[test]
fn clear_runtime_maps_drops_compile_session_indexes() {
    let mut session = PrebuildCompileSession::default();
    let default_scope = CompileScope::default_scope();
    let home_scope = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("scenes/home.mei".to_string()),
    };
    let outcome = test_outcome("home", "scenes/home.mei");

    session.register(
        Path::new("/tmp/ws"),
        "demo",
        &default_scope,
        outcome.clone(),
    );
    session.note_scope_alias(&home_scope, &outcome);

    assert!(!session.by_scope_key.is_empty());
    assert!(!session.by_compile_cache_key.is_empty());
    assert!(!session.by_identity.is_empty());

    session.clear_runtime_maps();

    assert!(session.by_scope_key.is_empty());
    assert!(session.by_compile_cache_key.is_empty());
    assert!(session.by_identity.is_empty());
}

#[test]
fn warmup_request_matches_active_scene_without_exact_scope_key() {
    let request = AggregatedWarmupRequest {
        scope: CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: None,
        },
        dataset_id: "penalty_result_dashboard_ds".to_string(),
        priority: WarmupRequestPriority::Critical,
        metric_ids: vec!["penalties_total_count::__scalar_rowset__".to_string()],
    };
    let mut outcome = test_outcome("home", "scenes/home.mei");
    let mut resource = test_dataset_resource("penalty_result_dashboard_ds");
    resource.dataset.as_mut().expect("dataset").runtime_metric_defs.insert(
        "penalties_total_count::__scalar_rowset__".to_string(),
        json!({"shape": "scalar_map"}),
    );
    Arc::make_mut(&mut outcome.compiled).resources.push(resource);
    assert!(warmup_request_matches_outcome(&request, &outcome));
    assert_eq!(
        matching_warmup_requests_for_outcome(&[request], &outcome).len(),
        1
    );
}

#[test]
fn warmup_request_does_not_match_outcome_without_dataset_resource() {
    let request = AggregatedWarmupRequest {
        scope: CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/10-地图.mei".to_string()),
        },
        dataset_id: "__world_metrics__::scenes/10-地图.mei::metrics".to_string(),
        priority: WarmupRequestPriority::Critical,
        metric_ids: Vec::new(),
    };
    let outcome = test_outcome("home", "scenes/10-地图.mei");
    assert!(!warmup_request_matches_outcome(&request, &outcome));
    assert!(
        matching_warmup_requests_for_outcome(&[request], &outcome).is_empty()
    );
}

#[test]
fn parallel_runner_preserves_input_order() {
    let values = run_limited_parallel_ordered(vec![1, 2, 3, 4], 4, |value| value * 10);
    assert_eq!(values, vec![10, 20, 30, 40]);
}
