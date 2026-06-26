use super::*;

#[test]
fn discovered_compile_scopes_do_not_expand_target_only_scope_into_route_aliases() {
    let scope = CompileScope {
        requested_scene_id: None,
        requested_target_file: Some("scenes/popup.board.mei".to_string()),
    };
    let mut outcome = test_outcome("popup_scene", "scenes/popup.board.mei");
    let compiled = Arc::make_mut(&mut outcome.compiled);
    compiled.scene_routes.push(CompiledSceneRoute {
        scene_id: "popup_scene".to_string(),
        frame_id: None,
        target_file: "scenes/popup.board.mei".to_string(),
        kind: "board".to_string(),
        title: None,
        is_default: false,
        access_export: true,
    });
    compiled.scene_routes.push(CompiledSceneRoute {
        scene_id: "popup_scene_duplicate".to_string(),
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
    assert!(discovered.contains("popup_scene|"));
    assert!(discovered.contains("popup_scene|scenes/popup.board.mei"));
    assert!(!discovered.contains("popup_scene_duplicate|scenes/popup.board.mei"));
}

#[test]
fn discovered_compile_scopes_expand_board_target_without_active_scene() {
    let scope = CompileScope {
        requested_scene_id: None,
        requested_target_file: Some("scenes/01-elements.board.mei".to_string()),
    };
    let mut outcome = test_outcome("", "scenes/01-elements.board.mei");
    {
        let compiled = Arc::make_mut(&mut outcome.compiled);
        compiled.active_scene = None;
        compiled.build_board_index.boards.insert(
            "scenes/01-elements.board.mei#key_enterprises_analytics_board".to_string(),
            BoardFileEntry {
                board_file: "scenes/01-elements.board.mei".to_string(),
                scene_id: "key_enterprises_analytics_board".to_string(),
                label: "Key enterprises".to_string(),
                ..Default::default()
            },
        );
        compiled.build_board_index.boards.insert(
            "scenes/01-elements.board.mei#enforcement_units_analytics_board".to_string(),
            BoardFileEntry {
                board_file: "scenes/01-elements.board.mei".to_string(),
                scene_id: "enforcement_units_analytics_board".to_string(),
                label: "Enforcement units".to_string(),
                ..Default::default()
            },
        );
    }

    let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
        .into_iter()
        .map(|scope| scope.key())
        .collect::<BTreeSet<_>>();
    assert!(discovered.contains(
        "key_enterprises_analytics_board|scenes/01-elements.board.mei"
    ));
    assert!(discovered.contains(
        "enforcement_units_analytics_board|scenes/01-elements.board.mei"
    ));
}

#[test]
fn focus_targets_from_warmup_datasets_extracts_scene_paths() {
    let app = RuntimeWarmupApp {
        app_id: "demo".to_string(),
        default_scene: None,
        hot_scenes: Vec::new(),
        scenes: Vec::new(),
        focuses: Vec::new(),
        datasets: vec![RuntimeWarmupDatasetRequest {
            scene_id: Some("home".to_string()),
            focus: None,
            dataset_id: "__world_metrics__::scenes/05-监督预警.mei::metrics".to_string(),
            priority: None,
            metric_id: None,
            metric_ids: Vec::new(),
        }],
        xlsx_sources: Vec::new(),
    };
    assert_eq!(
        focus_targets_from_warmup_datasets(&app),
        vec!["scenes/05-监督预警.mei".to_string()]
    );
}

#[test]
fn requested_dataframe_metric_ids_respects_explicit_metric_list() {
    let mut resource = test_dataset_resource("demo_ds");
    let dataset = resource.dataset.as_mut().expect("dataset");
    dataset
        .runtime_metric_defs
        .insert("table_a".to_string(), json!({"shape":"dataframe"}));
    dataset
        .runtime_metric_defs
        .insert("table_b".to_string(), json!({"shape":"dataframe"}));
    dataset.runtime_analysis_contracts.insert(
        "demo".to_string(),
        json!({
            "table_metric_id": "table_a",
            "detail_table_metric_id": "table_b"
        }),
    );

    let requested =
        requested_dataframe_metric_ids(dataset, &[String::from("table_a::__scalar_rowset__")]);
    assert_eq!(requested, vec!["table_a::__scalar_rowset__".to_string()]);

    let all_requested = requested_dataframe_metric_ids(dataset, &[]);
    assert!(all_requested.contains(&"table_a".to_string()));
    assert!(all_requested.contains(&"table_b".to_string()));
}

#[test]
fn warmup_request_scope_uses_dataset_selector_target_when_focus_missing() {
    let request = RuntimeWarmupDatasetRequest {
        scene_id: Some("home".to_string()),
        focus: None,
        dataset_id: "__world_metrics__::scenes/10-地图.mei::metrics".to_string(),
        priority: None,
        metric_id: None,
        metric_ids: Vec::new(),
    };
    let scope = warmup_request_scope(&request);
    assert_eq!(scope.requested_scene_id.as_deref(), Some("home"));
    assert_eq!(
        scope.requested_target_file.as_deref(),
        Some("scenes/10-地图.mei")
    );
}

