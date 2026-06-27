use super::*;

#[test]
fn prebuild_dataframe_page_sizes_default_single_collect_all() {
    assert_eq!(prebuild_dataframe_page_sizes(), &[PREBUILD_DATAFRAME_CANONICAL_PAGE_SIZE]);
    let options = prebuild_dataframe_query_options(PREBUILD_DATAFRAME_CANONICAL_PAGE_SIZE);
    assert!(options.collect_all);
    assert_eq!(options.page_size, PREBUILD_DATAFRAME_CANONICAL_PAGE_SIZE);
}

#[test]
fn discover_navigation_only_does_not_enqueue_pending() {
    let session = Mutex::new(PrebuildCompileSession {
        discover_enqueue_compile: false,
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
        Path::new("/tmp/ws"),
        "demo",
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

    assert!(pending.is_empty());
    assert_eq!(prepared_outcomes.len(), 1);
}

#[test]
fn prebuild_dataframe_metric_selector_keeps_dataframe_metric() {
    let metric_defs = BTreeMap::from([(
        "warnings_realtime_cockpit_table".to_string(),
        serde_json::json!({
            "id": "warnings_realtime_cockpit_table",
            "shape": "dataframe"
        }),
    )]);
    assert_eq!(
        prebuild_dataframe_metric_selector(&metric_defs, "warnings_realtime_cockpit_table"),
        "warnings_realtime_cockpit_table"
    );
}

#[test]
fn unique_prepared_outcomes_prefers_scene_scoped_compile_scope() {
    let home_scope = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: None,
    };
    let default_scope = CompileScope::default_scope();
    let prepared = vec![
        PreparedCompileOutcome {
            scope: default_scope,
            outcome: test_outcome("home", "scenes/home.mei"),
        },
        PreparedCompileOutcome {
            scope: home_scope,
            outcome: test_outcome("home", "scenes/home.mei"),
        },
    ];
    let unique = unique_prepared_outcomes_for_artifacts(&prepared);
    assert_eq!(unique.len(), 1);
    assert_eq!(unique[0].scope.requested_scene_id.as_deref(), Some("home"));
}

#[test]
fn observed_count_replays_reports_without_dup_prepared_outcomes() {
    let scope = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("scenes/home.mei".to_string()),
    };
    let outcome = test_outcome("home", "scenes/home.mei");
    let session = Mutex::new(PrebuildCompileSession::default());
    let mut seen_scopes = BTreeSet::new();
    let mut pending = std::collections::VecDeque::new();
    let mut prepared_outcomes = Vec::new();
    let mut compile_reports = Vec::new();

    record_prebuild_scope_compile_with_discovered(
        Path::new("/tmp/ws"),
        "demo",
        &session,
        &scope,
        &outcome,
        Some(&[]),
        3,
        &mut seen_scopes,
        &mut pending,
        &mut prepared_outcomes,
        &mut compile_reports,
    );

    assert_eq!(compile_reports.len(), 3);
    assert_eq!(prepared_outcomes.len(), 1);
    assert!(pending.is_empty());
}

#[test]
fn compile_index_observed_count_comes_from_reports_not_prepared_duplicates() {
    let scope = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("scenes/home.mei".to_string()),
    };
    let prepared_outcomes = vec![PreparedCompileOutcome {
        scope: scope.clone(),
        outcome: test_outcome("home", "scenes/home.mei"),
    }];
    let compile_reports = vec![
        scope_report_from_outcome(&scope, &test_outcome("home", "scenes/home.mei")),
        scope_report_from_outcome(&scope, &test_outcome("home", "scenes/home.mei")),
        scope_report_from_outcome(&scope, &test_outcome("home", "scenes/home.mei")),
    ];

    let index = build_prebuild_compile_index(
        Path::new("/tmp/ws"),
        "demo",
        &prepared_outcomes,
        &compile_reports,
    );
    let entry = index
        .entries_by_scope_key
        .get(&scope.key())
        .expect("compile index entry");

    assert_eq!(entry.observed_count, 3);
}

#[test]
fn filter_board_discovered_scopes_expands_once_per_board_file() {
    let mut session = PrebuildCompileSession::default();
    let parent = CompileScope {
        requested_scene_id: None,
        requested_target_file: Some("scenes/a.board.mei".to_string()),
    };
    let discovered = vec![
        CompileScope {
            requested_scene_id: Some("s1".to_string()),
            requested_target_file: Some("scenes/a.board.mei".to_string()),
        },
        CompileScope {
            requested_scene_id: Some("s2".to_string()),
            requested_target_file: Some("scenes/a.board.mei".to_string()),
        },
    ];
    let first = session.filter_board_discovered_scopes(&parent, &discovered);
    assert_eq!(first.len(), 2);
    let second = session.filter_board_discovered_scopes(&parent, &discovered);
    assert!(second.is_empty());
}

#[test]
fn alias_scope_does_not_expand_discover_queue() {
    let representative = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("scenes/home.mei".to_string()),
    };
    let alias = CompileScope {
        requested_scene_id: Some("home".to_string()),
        requested_target_file: Some("src/scenes/home.mei".to_string()),
    };
    let outcome = test_outcome("home", "scenes/home.mei");
    let session = Mutex::new(PrebuildCompileSession::default());
    let mut seen_scopes = BTreeSet::new();
    let mut pending = std::collections::VecDeque::new();
    let mut prepared_outcomes = Vec::new();
    let mut compile_reports = Vec::new();

    record_prebuild_scope_compile_with_discovered(
        Path::new("/tmp/ws"),
        "demo",
        &session,
        &representative,
        &outcome,
        None,
        1,
        &mut seen_scopes,
        &mut pending,
        &mut prepared_outcomes,
        &mut compile_reports,
    );
    let pending_after_rep = pending.len();

    record_prebuild_scope_compile_with_discovered(
        Path::new("/tmp/ws"),
        "demo",
        &session,
        &alias,
        &outcome,
        Some(&[]),
        1,
        &mut seen_scopes,
        &mut pending,
        &mut prepared_outcomes,
        &mut compile_reports,
    );

    assert_eq!(pending.len(), pending_after_rep);
    assert_eq!(prepared_outcomes.len(), 2);
}

#[test]
fn filter_hot_only_discovered_keeps_hot_scenes_and_target_only() {
    let session = PrebuildCompileSession {
        hot_only_scene_ids: Some(BTreeSet::from(["home".to_string()])),
        ..PrebuildCompileSession::default()
    };
    let filtered = session.filter_hot_only_discovered(vec![
        CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        },
        CompileScope {
            requested_scene_id: Some("details".to_string()),
            requested_target_file: Some("scenes/details.mei".to_string()),
        },
        CompileScope {
            requested_scene_id: None,
            requested_target_file: Some("scenes/focus.mei".to_string()),
        },
    ]);
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().any(|scope| scope.key() == "home|scenes/home.mei"));
    assert!(filtered.iter().any(|scope| scope.key() == "|scenes/focus.mei"));
}
