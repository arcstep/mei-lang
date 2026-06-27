use super::*;

#[test]
fn hot_only_compile_scopes_skip_deferred_dataset_closure() {
    let app = RuntimeWarmupApp {
        app_id: "demo".to_string(),
        default_scene: Some("home".to_string()),
        hot_scenes: vec!["dashboard".to_string()],
        scenes: vec!["details".to_string()],
        focuses: vec!["main.mei".to_string()],
        datasets: vec![
            RuntimeWarmupDatasetRequest {
                scene_id: Some("dashboard".to_string()),
                focus: Some("main.mei".to_string()),
                dataset_id: "hot_ds".to_string(),
                priority: None,
                metric_id: None,
                metric_ids: Vec::new(),
            },
            RuntimeWarmupDatasetRequest {
                scene_id: Some("details".to_string()),
                focus: Some("scenes/details.mei".to_string()),
                dataset_id: "deferred_ds".to_string(),
                priority: None,
                metric_id: None,
                metric_ids: Vec::new(),
            },
        ],
        xlsx_sources: Vec::new(),
    };
    let scope_keys = compile_scopes_for_app(&app, PrebuildScopeProfile::HotOnly)
        .into_iter()
        .map(|scope| scope.key())
        .collect::<BTreeSet<_>>();

    assert!(scope_keys.contains("|"));
    assert!(scope_keys.contains("home|"));
    assert!(scope_keys.contains("dashboard|"));
    assert!(scope_keys.contains("|main.mei"));
    assert!(scope_keys.contains("dashboard|main.mei"));
    assert!(!scope_keys.contains("details|"));
    assert!(!scope_keys.contains("details|scenes/details.mei"));
}

#[test]
fn hot_only_warmup_requests_keep_hot_scoped_datasets() {
    let app = RuntimeWarmupApp {
        app_id: "demo".to_string(),
        default_scene: Some("home".to_string()),
        hot_scenes: vec!["dashboard".to_string()],
        scenes: vec!["details".to_string()],
        focuses: vec!["main.mei".to_string()],
        datasets: vec![
            RuntimeWarmupDatasetRequest {
                scene_id: Some("dashboard".to_string()),
                focus: Some("main.mei".to_string()),
                dataset_id: "hot_ds".to_string(),
                priority: None,
                metric_id: Some("metric_a".to_string()),
                metric_ids: Vec::new(),
            },
            RuntimeWarmupDatasetRequest {
                scene_id: Some("details".to_string()),
                focus: Some("scenes/details.mei".to_string()),
                dataset_id: "deferred_ds".to_string(),
                priority: None,
                metric_id: Some("metric_b".to_string()),
                metric_ids: Vec::new(),
            },
        ],
        xlsx_sources: Vec::new(),
    };
    let requests = aggregate_warmup_requests(&app, PrebuildScopeProfile::HotOnly);

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].dataset_id, "hot_ds");
    assert_eq!(requests[0].scope.key(), "dashboard|main.mei");
}

#[test]
fn warmup_scope_batches_group_multiple_requests_by_scope() {
    let request_a = AggregatedWarmupRequest {
        scope: CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        },
        dataset_id: "ds_a".to_string(),
        priority: WarmupRequestPriority::Critical,
        metric_ids: vec!["metric_a".to_string()],
    };
    let request_b = AggregatedWarmupRequest {
        scope: CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        },
        dataset_id: "ds_b".to_string(),
        priority: WarmupRequestPriority::Critical,
        metric_ids: vec!["metric_b".to_string()],
    };
    let request_c = AggregatedWarmupRequest {
        scope: CompileScope {
            requested_scene_id: Some("details".to_string()),
            requested_target_file: Some("scenes/details.mei".to_string()),
        },
        dataset_id: "ds_c".to_string(),
        priority: WarmupRequestPriority::Deferred,
        metric_ids: vec!["metric_c".to_string()],
    };

    let grouped = group_warmup_requests_by_scope(&[&request_a, &request_b, &request_c])
        .into_iter()
        .map(|batch| (batch.scope.key(), batch.requests.len()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(grouped.len(), 2);
    assert_eq!(grouped.get("home|scenes/home.mei"), Some(&2));
    assert_eq!(grouped.get("details|scenes/details.mei"), Some(&1));
}

#[test]
fn discovered_compile_scopes_do_not_cross_bind_target_specific_scope_with_overlays() {
    let scope = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("scenes/home.mei".to_string()),
    };
    let mut outcome = test_outcome("home", "scenes/home.mei");
    let compiled = Arc::make_mut(&mut outcome.compiled);
    compiled.scene_local_nav_by_target.insert(
        "scenes/popup.board.mei".to_string(),
        json!({"scene_file":"scenes/popup.board.mei"}),
    );
    compiled.scene_routes.push(CompiledSceneRoute {
        scene_id: "popup_scene".to_string(),
        frame_id: None,
        target_file: "scenes/popup.board.mei".to_string(),
        kind: "board".to_string(),
        title: None,
        is_default: false,
        access_export: true,
    });

    let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
        .into_iter()
        .map(|scope| scope.key())
        .collect::<BTreeSet<_>>();
    assert!(discovered.contains("home|"));
    assert!(discovered.contains("home|scenes/home.mei"));
    assert!(!discovered.contains("home|scenes/popup.board.mei"));
    assert!(!discovered.contains("popup_scene|scenes/popup.board.mei"));
}

#[test]
fn discovered_compile_scopes_keep_default_scope_explicit_only() {
    let scope = CompileScope::default_scope();
    let mut outcome = test_outcome("home", "scenes/home.mei");
    let compiled = Arc::make_mut(&mut outcome.compiled);
    compiled.scene_local_nav_by_target.insert(
        "scenes/popup.board.mei".to_string(),
        json!({"scene_file":"scenes/popup.board.mei"}),
    );
    compiled.scene_routes.push(CompiledSceneRoute {
        scene_id: "popup_scene".to_string(),
        frame_id: None,
        target_file: "scenes/popup.board.mei".to_string(),
        kind: "board".to_string(),
        title: None,
        is_default: false,
        access_export: true,
    });

    let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
        .into_iter()
        .map(|scope| scope.key())
        .collect::<BTreeSet<_>>();
    assert!(discovered.contains("home|"));
    assert!(discovered.contains("home|scenes/home.mei"));
    assert_eq!(discovered.len(), 2);
    assert!(!discovered.contains("home|scenes/popup.board.mei"));
    assert!(!discovered.contains("popup_scene|scenes/popup.board.mei"));
}

#[test]
fn hot_only_skips_deferred_pending() {
    let session = Mutex::new(PrebuildCompileSession {
        hot_only_scene_ids: Some(BTreeSet::from(["home".to_string()])),
        skip_discover: false,
        ..PrebuildCompileSession::default()
    });
    let scope = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("scenes/home.mei".to_string()),
    };
    let outcome = test_outcome("home", "scenes/home.mei");
    let mut seen_scopes = BTreeSet::new();
    let mut pending = std::collections::VecDeque::new();
    let mut prepared_outcomes = Vec::new();
    let mut compile_reports = Vec::new();

    record_prebuild_scope_compile_with_discovered(
        &session,
        &scope,
        &outcome,
        None,
        1,
        &mut seen_scopes,
        &mut pending,
        &mut prepared_outcomes,
        &mut compile_reports,
    );

    assert!(
        pending.iter().all(|candidate| {
            candidate
                .requested_scene_id
                .as_deref()
                .is_none_or(|scene| scene == "home")
        }),
        "hot-only discover must not enqueue non-hot scenes"
    );
}

