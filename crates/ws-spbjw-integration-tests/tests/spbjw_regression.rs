use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_preview_typical_cases_dataset_mei_has_no_missing_scene() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw with dataset mei preview");
    let contract = compiled.scene_contract.as_ref().unwrap_or_else(|| {
        panic!(
            "preview should yield scene contract, diagnostics: {:?}",
            compiled.diagnostics
        )
    });
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let path_id = "typical_cases";
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == path_id)
        .and_then(|r| r.dataset.as_ref())
        .map(|d| d.rows.len())
        .unwrap_or(0);
    assert!(
        row_count > 0,
        "expected rows from xlsx for typical_cases, got {row_count}"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|d| !matches!(d.severity, mei_lang_kernel::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_routes.iter().any(|r| {
            r.scene_id == "typical_cases" && r.target_file == "scenes/09-监督典型案例.mei"
        }),
        "expected typical_cases in app route registry for access/manage deep links, got: {:?}",
        compiled
            .scene_routes
            .iter()
            .map(|r| (r.scene_id.as_str(), r.target_file.as_str()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_select_typical_cases_scene_resolves_dataset_entry() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("typical_cases".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with typical_cases scene (access-style)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/09-监督典型案例.mei"
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("typical_cases"));
}

#[test]
fn compile_spbjw_select_enterprise_complaints_scene_resolves_dataset_entry() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("administrative_inspection".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with administrative_inspection scene (discovered route)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/02-行政检查.mei"
    );
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("administrative_inspection")
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enterprise_complaints"),
        "expected enterprise_complaints dataset on administrative_inspection scene"
    );
}

#[test]
fn compile_spbjw_preview_enforcement_whitelist_dataset_mei_has_no_missing_scene() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw enforcement elements preview (whitelist dataset)");
    let contract = compiled.scene_contract.as_ref().unwrap_or_else(|| {
        panic!(
            "preview should yield scene contract, diagnostics: {:?}",
            compiled.diagnostics
        )
    });
    assert!(
        !contract.panels.is_empty(),
        "preview needs frame.add_panel blocks; got 0 panels"
    );
    let row_count = compiled
        .resources
        .iter()
        .find(|r| r.id == "enterprise_whitelist")
        .and_then(|r| r.dataset.as_ref())
        .map(|d| d.rows.len())
        .unwrap_or(0);
    assert!(
        row_count > 0,
        "expected xlsx rows for whitelist dataset, got {row_count}"
    );
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|d| !matches!(d.severity, mei_lang_kernel::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_dataset_preview_with_wrong_scene_query_still_resolves_entry_scene() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("执法要素".to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw enforcement elements with filename-like scene query");
    assert_eq!(compiled.active_target_file, target);
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enforcement_elements")
    );
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown_scene"),
        "preview_target route should satisfy scene anchor without unknown_scene: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_dataset_preview_with_explicit_scene_and_focus_stays_preview_only() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("enforcement_elements".to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw enforcement elements scene+focus");
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enforcement_elements")
    );
    assert_eq!(compiled.active_target_file, target);
    assert!(
        !compiled
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown_scene"),
        "explicit scene+focus should not warn unknown_scene: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_preview_widget_elements_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-左栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let elapsed = started.elapsed();
    assert_eq!(compiled.active_target_file, "scenes/layout-左栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "widget elements preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "layout_left");
    assert!(
        !contract.panels.is_empty(),
        "layout left should resolve left_rail panel, got {}",
        contract.panels.len()
    );
    let left_rail = contract
        .panels
        .iter()
        .find(|p| p.id == "left_rail")
        .expect("left_rail panel from layout capsule");
    assert!(
        !left_rail.blocks.is_empty(),
        "left_rail should carry titled_shell + body blocks from panel_ref"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enforcement_units" || r.id == "administrative_inspection"),
        "layout left preview should merge datasets from embedded rail bodies"
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|resource| resource.dataset.is_some()),
        "layout left preview needs selective dataset catalog, got ids: {:?}",
        compiled
            .resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    let dataset_resources: Vec<_> = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .collect();
    assert!(
        dataset_resources.len() <= 45,
        "manage widget preview should use selective catalog, not full scan (got {}): {:?}",
        dataset_resources.len(),
        dataset_resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        elapsed.as_secs() < 9,
        "manage widget preview should not compile home + full catalog (21 xlsx), took {:?}",
        elapsed
    );
}

#[test]
fn compile_spbjw_preview_layout_center_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-中栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout center preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "layout center preview errors: {:?}",
        errors
    );
    assert_eq!(compiled.active_target_file, "scenes/layout-中栏.mei");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("layout center preview scene contract");
    assert_eq!(contract.scene.id, "layout_center");
    assert!(
        contract.panels.len() >= 2,
        "layout center should resolve indicator + realtime panel_ref slots, got {}",
        contract.panels.len()
    );
    assert!(
        contract
            .panels
            .iter()
            .any(|p| p.id == "indicator_system_stats" && !p.blocks.is_empty()),
        "indicator_system_stats should carry body blocks"
    );
    assert!(
        contract
            .panels
            .iter()
            .any(|p| p.id == "realtime_warnings_table" && !p.blocks.is_empty()),
        "realtime_warnings_table should carry body blocks"
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_list"),
        "layout center preview should materialize warning_list from realtime body"
    );
}

#[test]
fn compile_spbjw_preview_widget_metrics_system_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
        },
    )
    .expect("compile spbjw supervision warning preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "metrics widget preview errors: {:?}",
        errors
    );
    assert_eq!(compiled.active_target_file, "scenes/05-监督预警.mei");
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_models"),
        "expected warning_models dataset in resources"
    );
}

#[test]
fn spbjw_supervision_models_count_is_eighteen() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
        },
    )
    .expect("compile supervision warning preview for models count");
    let metric = compiled
        .world_metrics
        .get("supervision_models_count")
        .map(|entry| &entry.metric)
        .expect("supervision_models_count in world_metrics");
    let value = metric
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| metric.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        value, 18.0,
        "《10》按序号前缀去重应得 18 个预警模型，got {value}"
    );
}

#[test]
fn spbjw_warning_list_materializes_leading_columns_from_empty_xlsx_headers() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
        },
    )
    .expect("compile supervision warning for warning_list columns");
    let dataset = compiled
        .resources
        .iter()
        .find(|r| r.id == "warning_list")
        .and_then(|r| r.dataset.as_ref())
        .expect("warning_list dataset");
    let row = dataset
        .rows
        .iter()
        .find(|row| {
            row.get("预警ID")
                .and_then(|v| v.as_str())
                .map(|s| s.contains("YJ2025001"))
                .unwrap_or(false)
        })
        .expect("sample warning row");
    let serial = row.get("序号").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_f64().map(|number| number as i64))
    });
    assert_eq!(
        serial,
        Some(1),
        "序号列应来自 Excel A 列（表头行为空单元格）"
    );
    assert_eq!(
        row.get("监督领域").and_then(|v| v.as_str()),
        Some("行政执法"),
        "监督领域应来自 Excel B 列"
    );
}

#[test]
#[ignore = "历史数据口径：预警条数求和断言待与 Excel 源数据对齐后恢复"]
fn spbjw_warnings_count_sums_warning_entry_column() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/05-监督预警.mei".to_string()),
        },
    )
    .expect("compile supervision warning preview for warnings count");
    let metric = compiled
        .world_metrics
        .get("warnings_count")
        .map(|entry| &entry.metric)
        .expect("warnings_count in world_metrics");
    let value = metric
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| metric.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        value, 25.0,
        "《11》预警ID去重后对「预警条数」求和应为 25 条，got {value}"
    );
}

#[test]
fn compile_spbjw_preview_widget_supervision_warning_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-右栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout right preview");
    assert_eq!(compiled.active_target_file, "scenes/layout-右栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "supervision widget preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "layout_right");
    assert!(
        !contract.panels.is_empty(),
        "layout right should resolve right_rail panel, got {}",
        contract.panels.len()
    );
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "warning_list" || r.id == "supervision_matters"),
        "layout right preview should merge datasets from embedded rail bodies"
    );
    assert!(
        contract.panels.iter().any(|p| !p.blocks.is_empty()),
        "layout right panels should carry blocks from external panel lookup"
    );
    assert!(
        compiled.resources.iter().any(|r| r.dataset.is_some()),
        "layout right preview should materialize datasets from referenced panels, got: {:?}",
        compiled
            .resources
            .iter()
            .filter(|r| r.dataset.is_some())
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_layout_right_supervision_popup_has_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-右栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout right preview");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("layout right scene contract");
    let encoded = serde_json::to_string(contract).expect("encode contract");
    assert!(
        encoded.contains("projection_slots"),
        "embedded supervision warning popups should lower projection_slots, assembly keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(
        encoded.contains("layout_mode") && encoded.contains("analytics"),
        "embedded supervision popups should use analytics layout_mode"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("supervision_items_analytics_board"),
        "drilldown context should include board export assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_typical_cases_popup_lowers_list_preview_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile typical cases preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "typical cases compile errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("typical cases scene contract");
    let encoded = serde_json::to_string(contract).expect("encode contract");
    assert!(
        encoded.contains("typical_cases_detail_board"),
        "typical cases popup should target detail card board export"
    );
    assert!(
        encoded.contains("list_preview") || encoded.contains("\"layout_zone\":\"preview\""),
        "expected list_preview slots in contract"
    );
    assert!(
        encoded.contains("typical_case_card") && encoded.contains("preview_mode"),
        "preview mapping should include typical_case_card config, snippet: {}",
        &encoded[encoded.find("preview_mode").unwrap_or(0)
            ..encoded
                .len()
                .min(encoded.find("preview_mode").unwrap_or(0) + 240)]
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("typical_cases_detail_board"),
        "typical_cases_detail_board assembly should be hydrated, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let popup = contract
        .panels
        .iter()
        .flat_map(|panel| panel.blocks.iter())
        .find_map(|block| match block {
            mei_lang_kernel::UiNodeDecl::Block(decl) if decl.use_key == "cockpit.data-table" => {
                decl.props.get("popup")
            }
            _ => None,
        })
        .expect("typical cases data-table popup");
    assert!(
        popup
            .get("projection_slots")
            .and_then(Value::as_array)
            .is_some_and(|slots| !slots.is_empty()),
        "popup should include lowered projection_slots, got keys: {:?}",
        popup.as_object().map(|map| map.keys().collect::<Vec<_>>())
    );
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("typical_cases_detail_board")
        .expect("detail card board assembly");
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .expect("projection slots array");
    assert!(
        !slots
            .iter()
            .any(|slot| slot.get("component").and_then(Value::as_str) == Some("data_table")),
        "preview-only board should not include list data_table slot, got {slots:?}"
    );
    let preview_slot = slots
        .iter()
        .find(|slot| slot.get("component").and_then(Value::as_str) == Some("summary"))
        .expect("preview summary slot");
    let preview_metric_id = preview_slot
        .get("metric_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        preview_metric_id.contains("__scalar_rowset__"),
        "preview slot should query scalar rowset detail metric, got `{preview_metric_id}` slots={slots:?}"
    );
    let mapping = preview_slot.get("mapping").expect("preview mapping");
    assert_eq!(
        mapping.get("preview_mode").and_then(Value::as_str),
        Some("typical_case_card"),
        "preview slot should carry typical_case_card mapping"
    );
    assert_eq!(
        mapping.get("preview_only").and_then(Value::as_bool),
        Some(true),
        "preview mapping should be preview_only"
    );
    let typical_cases = compiled
        .resources
        .iter()
        .find_map(|resource| {
            resource
                .dataset
                .as_ref()
                .filter(|dataset| dataset.id == "typical_cases")
                .cloned()
        })
        .expect("typical_cases dataset");
    assert!(
        typical_cases
            .runtime_metric_defs
            .contains_key("case_count::__scalar_rowset__"),
        "case_count should hoist inferred scalar rowset, keys: {:?}",
        typical_cases.runtime_metric_defs.keys().collect::<Vec<_>>()
    );
    let detail_contract = typical_cases
        .runtime_analysis_contracts
        .get("case_count")
        .and_then(|contract| contract.get("tab_metrics"))
        .and_then(|tabs| tabs.get("detail"))
        .and_then(Value::as_object)
        .expect("case_count detail tab metric");
    assert_eq!(
        detail_contract.get("metric_id").and_then(Value::as_str),
        Some("case_count::__scalar_rowset__"),
        "detail tab should target scalar rowset metric"
    );
}

#[test]
fn spbjw_typical_cases_swimlane_metric_dataframe_respects_result_id_filter() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use std::collections::BTreeMap;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/09-监督典型案例.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile typical cases preview");
    let highlights = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "typical_cases",
        "case_highlights",
        Some("typical_cases"),
        Some("scenes/09-监督典型案例.mei"),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 4,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("case_highlights dataframe");
    let sample_id = highlights
        .rows
        .first()
        .and_then(|row| row.get("value"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .expect("case_highlights row with 处理结果ID value");
    let mut filters = BTreeMap::new();
    filters.insert("处理结果ID".to_string(), sample_id.to_string());
    let detail = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "typical_cases",
        "case_count::__scalar_rowset__",
        Some("typical_cases_detail_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 20,
            filters,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("typical cases detail metric dataframe");
    assert!(
        !detail.rows.is_empty(),
        "filtered detail should return at least one row"
    );
    assert!(
        detail.rows.iter().all(|row| {
            row.get("处理结果ID").and_then(Value::as_str).map(str::trim) == Some(sample_id)
        }),
        "detail rows should match selected 处理结果ID"
    );
}

#[test]
fn spbjw_home_resolves_imported_typical_cases_dataset_selector() {
    use mei_lang_kernel::locate_dataset_resource;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile spbjw home preview");
    let namespaced = "scenes/09-监督典型案例.mei::typical_cases";
    let resource_ids: Vec<_> = compiled
        .resources
        .iter()
        .filter(|resource| {
            resource
                .id
                .contains("typical_cases")
                || resource
                    .dataset
                    .as_ref()
                    .is_some_and(|dataset| dataset.id.contains("typical_cases"))
        })
        .map(|resource| resource.id.as_str())
        .collect();
    assert!(
        !resource_ids.is_empty(),
        "home preview should materialize imported typical_cases resources, got {resource_ids:?}"
    );
    let resource = locate_dataset_resource(&compiled, namespaced)
        .unwrap_or_else(|error| panic!("locate {namespaced}: {error}; resources={resource_ids:?}"));
    assert!(
        resource.dataset.is_some(),
        "typical_cases resource should expose dataset view"
    );
}

#[test]
fn spbjw_typical_cases_board_resolves_namespaced_dataset_selector() {
    use mei_lang_kernel::locate_dataset_resource;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/09-监督典型案例.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .expect("compile typical cases board preview");
    let namespaced = "scenes/09-监督典型案例.mei::typical_cases";
    let resource = locate_dataset_resource(&compiled, namespaced)
        .unwrap_or_else(|error| {
            let resource_ids: Vec<_> = compiled
                .resources
                .iter()
                .filter(|resource| resource.dataset.is_some())
                .map(|resource| resource.id.as_str())
                .collect();
            panic!("locate {namespaced}: {error}; resources={resource_ids:?}")
        });
    assert_eq!(resource.id, "typical_cases");
}

#[test]
fn compile_spbjw_layout_right_typical_cases_popup_lowers_list_preview() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-右栏.mei".to_string()),
        },
    )
    .expect("compile layout right preview");
    let encoded = serde_json::to_string(compiled.scene_contract.as_ref().expect("contract"))
        .expect("encode contract");
    assert!(
        encoded.contains("typical_cases") && encoded.contains("typical_cases_detail_board"),
        "layout right should reference typical_cases detail card board, assembly keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_preview_widget_typical_cases_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw typical cases preview");
    let elapsed = started.elapsed();
    assert_eq!(compiled.active_target_file, "scenes/09-监督典型案例.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "typical cases widget preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "typical_cases");
    assert!(
        !contract.panels[0].blocks.is_empty(),
        "typical_cases preview should render blocks"
    );
    let dataset_resources: Vec<_> = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .collect();
    assert!(
        !dataset_resources.is_empty(),
        "typical_cases preview should materialize dataset resources, got: {:?}",
        dataset_resources
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "typical_cases"),
        "missing typical_cases resource"
    );
    assert!(
        elapsed.as_secs() < 8,
        "widget preview with selective catalog should compile faster than full scan, took {:?}",
        elapsed
    );
}

#[test]
fn compile_spbjw_overview_preview_materializes_imported_metrics() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/layout-左栏.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "layout left preview errors: {:?}",
        errors
    );
    let ids: Vec<_> = compiled.resources.iter().map(|r| r.id.as_str()).collect();
    let units = compiled
        .resources
        .iter()
        .find(|r| r.id == "enforcement_units")
        .unwrap_or_else(|| panic!("expected enforcement_units in catalog, got {ids:?}"));
    let dataset = units.dataset.as_ref().expect("enforcement_units dataset");
    assert!(
        !dataset.columns.is_empty() && !dataset.rows.is_empty(),
        "imported dataset should at least materialize basic tabular payload"
    );
    assert!(
        dataset.schema.len() >= 1,
        "expected imported dataset schema to be materialized"
    );
}

#[test]
fn compile_spbjw_access_home_scene_materializes_ops_theme() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw access home");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("access home scene contract");
    assert_eq!(
        contract.themes.len(),
        1,
        "access route must inject ops theme"
    );
    assert_eq!(
        contract.themes[0].font.get("4").and_then(|v| v.as_str()),
        Some("36px")
    );
    assert_eq!(
        contract.themes[0].font.get("5").and_then(|v| v.as_str()),
        Some("24px"),
        "ops theme should define font level 5 for metric values"
    );
    assert_eq!(
        contract.themes[0]
            .metric_value
            .get("font")
            .and_then(|v| v.as_str()),
        Some("5")
    );
    let left_rail = contract
        .panels
        .iter()
        .find(|p| p.id == "left_rail_float")
        .expect("home access route should include left_rail_float");
    fn find_panel_in_nodes<'a>(
        nodes: &'a [mei_lang_kernel::UiNodeDecl],
        id: &str,
    ) -> Option<&'a mei_lang_kernel::PanelDecl> {
        for node in nodes {
            let mei_lang_kernel::UiNodeDecl::Panel(panel) = node else {
                continue;
            };
            if panel.id == id {
                return Some(panel);
            }
            if let Some(found) = find_panel_in_nodes(&panel.blocks, id) {
                return Some(found);
            }
        }
        None
    }
    let titled = find_panel_in_nodes(&left_rail.blocks, "enforcement_elements_stats")
        .expect("left rail should nest titled_shell panel");
    assert_eq!(
        titled.head_props.get("font_size").and_then(|v| v.as_str()),
        Some("30px"),
        "titled_shell heading font_size should survive panel_ref merge"
    );
    assert!(
        titled
            .head_props
            .get("background")
            .and_then(|bg| bg.get("image"))
            .and_then(|v| v.as_str())
            .is_some_and(|v| v.contains("linear-gradient")),
        "titled_shell title background should survive panel_ref merge"
    );
}

#[test]
fn compile_spbjw_preview_home_scene_succeeds() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile spbjw home preview");
    assert_eq!(compiled.active_target_file, "scenes/home.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(errors.is_empty(), "home preview errors: {:?}", errors);
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert_eq!(contract.scene.id, "home");
    let frame = contract.frame.as_ref().expect("home should have frame");
    assert!(
        frame.layout.is_some(),
        "home should expose frame grid layout"
    );
    assert!(
        contract.panels.len() >= 6,
        "home should flatten panel_ref slots into scene panels, got {}",
        contract.panels.len()
    );
    assert!(
        contract.panels.iter().any(|panel| panel.area.is_some()),
        "home should keep area bindings for scene grid panels"
    );
    let overview = contract
        .panels
        .iter()
        .find(|p| p.id == "enforcement_elements_stats");
    if let Some(panel) = overview {
        assert!(
            !panel.blocks.is_empty(),
            "home panel(base=panel_ref) should inherit blocks from external panel"
        );
    }
    let resource_ids: Vec<_> = compiled.resources.iter().map(|r| r.id.as_str()).collect();
    assert!(
        compiled
            .resources
            .iter()
            .any(|r| r.id == "enforcement_units"),
        "home preview catalog should materialize panel_ref datasets, got {resource_ids:?}"
    );
    let viewport = frame
        .props
        .get("viewport")
        .and_then(|value| value.as_object())
        .expect("home frame should declare viewport props");
    assert_eq!(
        viewport.get("design_width").and_then(|v| v.as_i64()),
        Some(1920)
    );
    assert_eq!(
        viewport.get("design_height").and_then(|v| v.as_i64()),
        Some(1080)
    );
    assert_eq!(contract.themes.len(), 1);
    assert_eq!(contract.themes[0].id, "cockpit");
    assert_eq!(
        contract.themes[0].font.get("4").and_then(|v| v.as_str()),
        Some("36px"),
        "ops.themes.cockpit font scale should materialize into scene_contract"
    );
    let frame_image = contract.themes[0]
        .frame
        .get("background")
        .and_then(|bg| bg.get("image"))
        .and_then(|v| v.as_str());
    assert!(
        frame_image.is_some_and(|v| v.contains("bg@3x.png")),
        "frame bg image ops_param should resolve at compile time, got {frame_image:?}"
    );
    let issue_metrics_owner = "__world_metrics__::scenes/07-问题办理.mei::metrics";
    let issue_metrics = compiled
        .resources
        .iter()
        .find(|r| r.id == issue_metrics_owner)
        .and_then(|r| r.dataset.as_ref())
        .expect("home should import 问题办理 world metrics resource");
    assert_eq!(
        mei_lang_kernel::resolve_runtime_metric_def_key(
            issue_metrics_owner,
            "warnings_pending_count::__scalar_rowset__",
            &issue_metrics.runtime_metric_defs,
        )
        .as_deref(),
        Some("scenes/07-问题办理.mei::warnings_pending_count::__scalar_rowset__"),
        "imported capsule metrics should hoist inferred scalar rowset for detail drilldown"
    );
    assert!(
        compiled
            .resources
            .iter()
            .filter_map(|r| r.dataset.as_ref())
            .any(|dataset| !dataset.metrics.is_empty() || !dataset.runtime_metric_defs.is_empty()),
        "home preview should materialize datasets with metrics"
    );
}

#[test]
fn compile_spbjw_preview_main_mei_keeps_inspection_and_penalty_cockpit_metrics() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("main.mei".to_string()),
        },
    )
    .expect("compile spbjw main preview");
    let datasets: Vec<_> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.as_ref())
        .collect();
    assert!(
        !datasets.is_empty(),
        "main preview should materialize datasets from cockpit scenes"
    );
    assert!(
        datasets
            .iter()
            .any(|dataset| !dataset.metrics.is_empty() || !dataset.runtime_metric_defs.is_empty()),
        "main preview should keep resolved metric payloads"
    );
}

#[test]
fn compile_spbjw_cockpit_scenes_use_generic_drilldown_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let cases: [(&str, &str, Vec<&str>); 0] = [];

    for (target, sample_metric_id, legacy_popup_files) in cases {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
        let contract = compiled
            .scene_contract
            .as_ref()
            .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
        let encoded = serde_json::to_string(contract).expect("encode scene contract");

        assert!(
            encoded.contains("drilldown-kit.mei"),
            "`{target}` should reference generic drilldown shell, got: {encoded}"
        );
        assert!(
            encoded.contains("projection_slots"),
            "`{target}` link tabs should lower to projection_slots, got: {encoded}"
        );
        assert!(
            encoded.contains(sample_metric_id),
            "`{target}` projection_slots should include `{sample_metric_id}`, got: {encoded}"
        );
        for legacy in legacy_popup_files {
            assert!(
                !encoded.contains(legacy),
                "`{target}` should not reference legacy popup `{legacy}`, got: {encoded}"
            );
        }
        assert!(
            !encoded.contains("scene_file\":\"../.stock/templates/cockpit/drilldown/metric-explain-board.mei\""),
            "`{target}` should not keep direct metric-explain-board scene_file links, got: {encoded}"
        );
    }
}

#[test]
fn compile_spbjw_enforcement_elements_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let contract = compiled
        .scene_contract
        .as_ref()
        .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
    let encoded = serde_json::to_string(contract).expect("encode scene contract");
    assert!(
        encoded.contains("enforcement_units_analytics_board"),
        "执法单位卡应引用 analytics board，got: {encoded}"
    );
    assert!(
        encoded.contains("drilldown-kit.mei"),
        "执法对象复合卡仍应保留 generic 多表下钻，got: {encoded}"
    );
    assert!(
        !encoded.contains("enforcement-units-popup.mei"),
        "不应再引用独立 popup scene 文件，got: {encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "drilldown context should hydrate enforcement units analytics assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("执法要素 direct preview should include __world_metrics__");
    let contract = dataset
        .runtime_analysis_contracts
        .get("enforcement_objects_count")
        .and_then(Value::as_object)
        .expect("enforcement_objects_count analysis contract");
    let blocks = contract
        .get("blocks")
        .and_then(Value::as_array)
        .expect("enforcement_objects_count contract blocks");
    assert!(
        blocks.len() >= 5,
        "执法对象 explain 的 5 个 dataframe 应落成 projection blocks，got {blocks:?}"
    );
}

#[test]
fn compile_spbjw_enforcement_units_shell_contract_zones_match_layout() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let board_id = "enforcement_units_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(board_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{board_id}`, keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let shell = assembly
        .get("shell_contract")
        .and_then(Value::as_object)
        .expect("enforcement units assembly should include shell_contract");
    let zone_ids: Vec<String> = shell
        .get("zones")
        .and_then(Value::as_array)
        .map(|zones| {
            zones
                .iter()
                .filter_map(|zone| zone.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !zone_ids.iter().any(|id| id == "chart"),
        "filter_detail board shell_contract should not include chart zone, got {zone_ids:?}"
    );
    assert!(
        zone_ids.iter().any(|id| id == "filter") && zone_ids.iter().any(|id| id == "detail"),
        "filter_detail board shell_contract should include filter and detail zones, got {zone_ids:?}"
    );
    let areas = shell
        .get("layout")
        .and_then(|layout| layout.get("areas"))
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .map(|row| {
            row.iter()
                .filter_map(|cell| cell.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    assert_eq!(
        areas,
        vec!["filter".to_string(), "detail".to_string()],
        "filter_detail board layout areas should be filter+detail only, got {areas:?}"
    );
}

#[test]
fn compile_spbjw_drilldown_kit_template_is_previewable() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "../.stock/templates/cockpit/drilldown/drilldown-kit.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile drilldown kit preview `{target}` failed: {error}"));
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "drilldown kit preview should have no error diagnostics: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("drilldown kit preview should yield scene contract");
    assert_eq!(contract.scene.id, "generic_drilldown_board");
}

#[test]
fn compile_spbjw_generic_drilldown_board_template_is_previewable() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "../.stock/templates/cockpit/drilldown/generic-drilldown-board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile generic drilldown preview `{target}` failed: {error}"));
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "generic drilldown preview should have no error diagnostics: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("generic drilldown preview should yield scene contract");
    assert_eq!(contract.scene.id, "generic_drilldown_board");
}

#[test]
fn compile_spbjw_analytics_drilldown_board_template_is_previewable() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "../.stock/templates/cockpit/drilldown/analytics-drilldown-board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| {
        panic!("compile analytics drilldown preview `{target}` failed: {error}")
    });
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "analytics drilldown preview should have no error diagnostics: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("analytics drilldown preview should yield scene contract");
    assert_eq!(contract.scene.id, "analytics_drilldown_board");
}

#[test]
fn compile_spbjw_supervision_warning_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/05-监督预警.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let contract = compiled
        .scene_contract
        .as_ref()
        .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
    let encoded = serde_json::to_string(contract).expect("encode scene contract");
    assert!(
        encoded.contains("supervision_items_analytics_board"),
        "items card popup should reference analytics board export, got: {encoded}"
    );
    assert!(
        encoded.contains("supervision_models_analytics_board"),
        "models card popup should reference analytics board export, got: {encoded}"
    );
    assert!(
        encoded.contains("warnings_analytics_board"),
        "warnings card popup should reference board export, got: {encoded}"
    );
    assert!(
        encoded.contains("layout_zone"),
        "analytics projection slots should include layout_zone, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_category"),
        "explicit analytics charts should reference explain block ids, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_warning_level"),
        "items analytics charts should reference warning-level explain block ids, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_model_type"),
        "models analytics charts should reference model-type explain block ids, got: {encoded}"
    );
    assert!(
        encoded.contains("chart_kind") && encoded.contains("column"),
        "analytics charts should lower configured chart kinds, got: {encoded}"
    );
    assert!(
        encoded.contains("\"top_n\":6"),
        "warnings analytics chart slots should carry configurable top_n=6, got: {encoded}"
    );
    assert!(
        encoded.contains("filter_schema"),
        "analytics link should include filter_schema, got: {encoded}"
    );
    assert!(
        encoded.contains("\"layout_zone\":\"chart\"")
            && encoded.contains("\"layout_zone\":\"detail\""),
        "analytics board assembly should lower chart/detail layout_zone aligned with scene panels, got: {encoded}"
    );
    assert!(
        encoded.contains("supervisionDomain")
            && encoded.contains("matter")
            && encoded.contains("modelName")
            && encoded.contains("month_multi_select"),
        "three analytics boards should include dataset-specific filter fields, got: {encoded}"
    );
    assert!(
        encoded.contains("layout_mode") && encoded.contains("analytics"),
        "board assembly should lower to analytics layout_mode, got: {encoded}"
    );
    assert!(
        !encoded.contains("generic_drilldown_board"),
        "supervision warning cards should no longer reference generic drilldown shell, got: {encoded}"
    );
}

#[test]
fn compile_spbjw_supervision_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/05-监督预警.board.mei";
    let scene_id = "supervision_items_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{scene_id}`, got keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        !slots.is_empty(),
        "board export preview assembly should include projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    assert!(
        assembly.get("preview_params").is_some(),
        "expected preview_params in assembly, got: {assembly:?}"
    );
    let detail_slot = slots.iter().find(|slot| {
        slot.get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == "detail")
    });
    assert_eq!(
        detail_slot.and_then(|slot| slot.get("dataset_id").and_then(Value::as_str)),
        Some("supervision_matters"),
        "analytics board detail slot should use rowset dataset, slots: {slots:?}"
    );
    let chart_slot = slots.iter().find(|slot| {
        slot.get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == "composition_by_category")
    });
    assert_eq!(
        chart_slot.and_then(|slot| slot.get("dataset_id").and_then(Value::as_str)),
        Some("supervision_matters"),
        "analytics board chart slot should use rowset dataset, slots: {slots:?}"
    );
}

#[test]
fn compile_spbjw_inspection_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.board.mei";
    let scene_id = "inspection_total_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{scene_id}`, got keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        slots.len() >= 3,
        "cockpit-wrapped inspection board should lower projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    assert!(
        assembly.get("preview_params").is_some(),
        "expected preview_params in assembly, got: {assembly:?}"
    );
}

#[test]
fn compile_spbjw_ai_warning_cockpit_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.board.mei";
    let scene_id = "ai_warning_cockpit_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{scene_id}`, got keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        slots.len() >= 4,
        "ai_warning list_preview board should lower chart+detail+preview projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    let components: Vec<&str> = slots
        .iter()
        .filter_map(|slot| {
            slot.as_object()
                .and_then(|map| map.get("component"))
                .and_then(Value::as_str)
        })
        .collect();
    assert!(
        components.iter().filter(|c| **c == "chart").count() >= 2,
        "expected at least 2 chart slots, got components: {components:?}"
    );
    assert!(
        components.iter().any(|c| *c == "data_table"),
        "expected detail table slot, got components: {components:?}"
    );
    assert!(
        components.iter().any(|c| *c == "summary"),
        "expected preview summary slot, got components: {components:?}"
    );
    assert!(
        assembly.get("preview_params").is_some(),
        "expected preview_params in assembly, got: {assembly:?}"
    );
    let shell = assembly
        .get("shell_contract")
        .and_then(Value::as_object)
        .expect("shell_contract");
    let zones = shell
        .get("zones")
        .and_then(Value::as_array)
        .expect("shell zones");
    let chart_zone = zones
        .iter()
        .find(|zone| {
            zone.as_object()
                .and_then(|map| map.get("id"))
                .and_then(Value::as_str)
                == Some("chart")
        })
        .and_then(Value::as_object)
        .expect("chart zone");
    assert_eq!(
        chart_zone.get("parent").and_then(Value::as_str),
        Some("left"),
        "chart zone should stay nested under left container, zones: {zones:?}"
    );
}

#[test]
fn compile_spbjw_home_ai_warning_cockpit_hydrated_assembly_includes_chart_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw home scene");
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("ai_warning_cockpit_board")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected hydrated ai_warning_cockpit_board assembly on home compile, keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        slots.len() >= 4,
        "home hydrated ai_warning board should include chart+detail+preview slots, got: {slots:?}"
    );
    let components: Vec<&str> = slots
        .iter()
        .filter_map(|slot| {
            slot.as_object()
                .and_then(|map| map.get("component"))
                .and_then(Value::as_str)
        })
        .collect();
    assert!(
        components.iter().filter(|c| **c == "chart").count() >= 2,
        "expected chart slots on home hydrated assembly, got components: {components:?}"
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("home scene contract");
    let encoded = serde_json::to_string(contract).expect("encode home contract");
    assert!(
        encoded.contains("ai_warning_cockpit_board"),
        "home contract should lower ai_warning popup"
    );
    assert!(
        encoded.contains("\"composition_by_source_unit\"")
            && encoded.contains("\"composition_by_warning_type\""),
        "home popup should lower chart projection slots in scene contract"
    );
}

#[test]
fn compile_spbjw_enforcement_personnel_board_preview_precompiles_single_route() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.board.mei";
    let scene_id = "enforcement_personnel_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    assert_eq!(compiled.active_scene.as_deref(), Some(scene_id));
    let routes_attempted = compiled
        .diagnostics
        .iter()
        .find_map(|diag| {
            if diag.code != "route_precompile_stats" {
                return None;
            }
            diag.message.split(',').find_map(|part| {
                part.trim()
                    .strip_prefix("routes_attempted=")
                    .and_then(|value| value.parse::<usize>().ok())
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "route_precompile_stats missing, diagnostics: {:?}",
                compiled.diagnostics
            )
        });
    assert_eq!(
        routes_attempted, 1,
        "multi-export board manage preview should precompile one route, diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key(scene_id),
        "expected assembly for `{scene_id}`, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    assert!(
        !compiled
            .scene_projection_assembly_by_id
            .contains_key("enforcement_units_analytics_board"),
        "sibling board should not be pre-warmed, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_issue_handling_board_export_preview_projection_slots_in_assembly() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/07-问题办理.board.mei";
    let scene_id = "issue_pending_analytics_board";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "expected assembly for `{scene_id}`, got keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        !slots.is_empty(),
        "issue analytics board export preview assembly should include projection_slots, assembly keys: {:?}, diagnostics: {:?}",
        assembly.keys().collect::<Vec<_>>(),
        compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
            .collect::<Vec<_>>()
    );
    let encoded = serde_json::to_string(assembly).expect("encode assembly");
    assert!(
        encoded.contains("\"layout_zone\":\"chart\"")
            && encoded.contains("\"layout_zone\":\"detail\""),
        "issue analytics board assembly should lower chart/detail layout_zone, got: {encoded}"
    );
    assert!(
        encoded.contains("layout_mode") && encoded.contains("analytics"),
        "issue analytics board assembly should lower to analytics layout_mode, got: {encoded}"
    );
}

#[test]
fn compile_spbjw_issue_handling_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/07-问题办理.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let contract = compiled
        .scene_contract
        .as_ref()
        .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
    let encoded = serde_json::to_string(contract).expect("encode scene contract");
    for metric_id in [
        "warnings_pending_count",
        "effectiveness_in_progress_count",
        "effectiveness_completed_count",
        "effectiveness_issue_verification_rate",
    ] {
        assert!(
            encoded.contains(metric_id),
            "issue handling scene contract should include metric `{metric_id}`, got: {encoded}"
        );
    }
    for board_id in [
        "issue_pending_analytics_board",
        "issue_doing_analytics_board",
        "issue_done_analytics_board",
        "issue_rate_analytics_board",
    ] {
        assert!(
            encoded.contains(board_id),
            "issue handling status cards should reference analytics board `{board_id}`, got: {encoded}"
        );
    }
    assert!(
        encoded.contains("composition_by_verified"),
        "verification rate card should reference verified status composition explain, got: {encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("issue_rate_analytics_board"),
        "drilldown context should hydrate rate analytics board assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let pending_assembly = compiled
        .scene_projection_assembly_by_id
        .get("issue_pending_analytics_board")
        .and_then(Value::as_object)
        .expect("pending analytics assembly");
    let pending_encoded = serde_json::to_string(pending_assembly).expect("encode pending assembly");
    assert!(
        pending_encoded.contains("filter_schema"),
        "issue pending analytics assembly should include filter_schema, got: {pending_encoded}"
    );
    assert!(
        pending_encoded.contains("layout_mode") && pending_encoded.contains("analytics"),
        "issue pending analytics assembly should lower to analytics layout_mode, got: {pending_encoded}"
    );
    assert!(
        encoded.contains("composition_by_category"),
        "issue analytics cards should reference composition explain blocks, got: {encoded}"
    );
    assert!(
        encoded.contains("chart_kind") && encoded.contains("column"),
        "issue analytics charts should lower configured chart kinds, got: {encoded}"
    );
    assert!(
        pending_encoded.contains("supervisionDomain")
            && pending_encoded.contains("month_multi_select"),
        "issue analytics boards should include warning_list filter fields, got: {pending_encoded}"
    );
    assert!(
        pending_encoded.contains("\"layout_zone\":\"chart\"")
            && pending_encoded.contains("\"layout_zone\":\"detail\""),
        "issue pending analytics assembly should lower chart/detail layout_zone, got: {pending_encoded}"
    );
    assert!(
        encoded.contains("issue_rate_analytics_board")
            && encoded.contains("warning_detail")
            && encoded.contains("核查情况"),
        "verification rate popup should use analytics board with warning_list detail fields, got: {encoded}"
    );
    let rate_assembly = compiled
        .scene_projection_assembly_by_id
        .get("issue_rate_analytics_board")
        .and_then(Value::as_object)
        .expect("rate analytics assembly");
    let rate_slots = rate_assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_slot = rate_slots.iter().find(|slot| {
        slot.as_object()
            .and_then(|map| map.get("layout_zone"))
            .and_then(Value::as_str)
            == Some("detail")
    });
    assert!(
        detail_slot
            .and_then(Value::as_object)
            .and_then(|slot| slot.get("explain_block_id"))
            .and_then(Value::as_str)
            == Some("detail"),
        "rate analytics detail slot should bind warning_list detail explain, slots: {rate_slots:?}"
    );
}

#[test]
fn compile_spbjw_enterprise_complaints_analytics_board_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("enterprise_complaints_analytics_board")
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "missing enterprise_complaints_analytics_board assembly, keys: {:?}",
                compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .collect::<Vec<_>>()
            )
        });
    let slots = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let detail_metric_id = slots
        .iter()
        .find(|slot| {
            slot.as_object()
                .and_then(|map| map.get("layout_zone"))
                .and_then(Value::as_str)
                == Some("detail")
        })
        .and_then(Value::as_object)
        .and_then(|slot| slot.get("metric_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .expect("detail slot metric_id");
    assert!(
        detail_metric_id.ends_with("::__scalar_rowset__"),
        "detail slot should bind scalar rowset, got `{detail_metric_id}`"
    );
    let trend_slot = slots
        .iter()
        .find(|slot| {
            slot.as_object()
                .and_then(|map| map.get("explain_block_id"))
                .and_then(Value::as_str)
                == Some("trend_by_report_time")
        })
        .and_then(Value::as_object)
        .expect("trend_by_report_time chart slot");
    assert_eq!(
        trend_slot.get("date_field").and_then(Value::as_str),
        Some("反映时间"),
        "trend slot should carry date_field for filtered client-side aggregation"
    );
}

#[test]
fn compile_spbjw_left_rail_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    for (target, board_id) in [
        (
            "scenes/01-执法要素.mei",
            "enforcement_units_analytics_board",
        ),
        ("scenes/02-行政检查.mei", "inspection_total_analytics_board"),
        (
            "scenes/03-指标体系.mei",
            "indicator_inspection_frequency_analytics_board",
        ),
        ("scenes/04-行政处罚.mei", "penalty_total_analytics_board"),
    ] {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
        let contract = compiled
            .scene_contract
            .as_ref()
            .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
        let encoded = serde_json::to_string(contract).expect("encode scene contract");
        assert!(
            encoded.contains(board_id),
            "`{target}` should reference analytics board `{board_id}`, got: {encoded}"
        );
        assert!(
            !encoded.contains("generic_drilldown_board") || target == "scenes/01-执法要素.mei",
            "`{target}` should not use generic drilldown except 执法对象，got: {encoded}"
        );
        assert!(
            compiled
                .scene_projection_assembly_by_id
                .contains_key(board_id),
            "`{target}` should hydrate assembly for `{board_id}`, keys: {:?}",
            compiled
                .scene_projection_assembly_by_id
                .keys()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_supervision_effectiveness_analytics_projection_slots() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/08-监督成效.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let contract = compiled
        .scene_contract
        .as_ref()
        .unwrap_or_else(|| panic!("`{target}` should yield scene contract"));
    let encoded = serde_json::to_string(contract).expect("encode scene contract");
    for board_id in [
        "effect_transfer_clue_analytics_board",
        "effect_filing_analytics_board",
        "effect_sanction_analytics_board",
        "effect_handled_analytics_board",
        "effect_recovered_analytics_board",
        "effect_mechanism_documents_board",
    ] {
        assert!(
            encoded.contains(board_id),
            "supervision effectiveness cards should reference analytics board `{board_id}`, got: {encoded}"
        );
    }
    assert!(
        !encoded.contains("generic_drilldown_board"),
        "supervision effectiveness cards should no longer reference generic drilldown shell, got: {encoded}"
    );
    assert!(
        encoded.contains("composition_by_category"),
        "effectiveness analytics should reference composition explain blocks, got: {encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("effect_transfer_clue_analytics_board"),
        "drilldown context should hydrate transfer clue analytics board assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let clue_assembly = compiled
        .scene_projection_assembly_by_id
        .get("effect_transfer_clue_analytics_board")
        .and_then(Value::as_object)
        .expect("transfer clue analytics assembly");
    let clue_encoded = serde_json::to_string(clue_assembly).expect("encode clue assembly");
    assert!(
        clue_encoded.contains("layout_mode") && clue_encoded.contains("analytics"),
        "transfer clue analytics assembly should lower to analytics layout_mode, got: {clue_encoded}"
    );
    assert!(
        clue_encoded.contains("supervisionDomain") && clue_encoded.contains("month_multi_select"),
        "warning_list effectiveness boards should include warning_list filter fields, got: {clue_encoded}"
    );
    let sanction_assembly = compiled
        .scene_projection_assembly_by_id
        .get("effect_sanction_analytics_board")
        .and_then(Value::as_object)
        .expect("sanction analytics assembly");
    let sanction_encoded =
        serde_json::to_string(sanction_assembly).expect("encode sanction assembly");
    assert!(
        sanction_encoded.contains("resultId") && sanction_encoded.contains("sanction"),
        "issue_result_list effectiveness boards should include result filter fields, got: {sanction_encoded}"
    );
    assert!(
        compiled
            .scene_projection_assembly_by_id
            .contains_key("effect_mechanism_documents_board"),
        "drilldown context should hydrate mechanism documents board assembly, keys: {:?}",
        compiled
            .scene_projection_assembly_by_id
            .keys()
            .collect::<Vec<_>>()
    );
    let mechanism_assembly = compiled
        .scene_projection_assembly_by_id
        .get("effect_mechanism_documents_board")
        .and_then(Value::as_object)
        .expect("mechanism documents assembly");
    let mechanism_encoded =
        serde_json::to_string(mechanism_assembly).expect("encode mechanism documents assembly");
    assert!(
        mechanism_encoded.contains("document_preview"),
        "mechanism documents board should lower document_preview mapping, got: {mechanism_encoded}"
    );
    assert!(
        mechanism_encoded.contains("layout_mode") && mechanism_encoded.contains("list_preview"),
        "mechanism documents board should lower to list_preview layout_mode, got: {mechanism_encoded}"
    );
}

#[test]
fn compile_spbjw_preview_administrative_inspection_park_metrics_succeeds() {
    use ws_spbjw_integration_tests::MetricShape;
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/02-行政检查.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw administrative inspection preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, mei_lang_kernel::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "administrative_inspection preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert!(
        !contract.panels.is_empty(),
        "expected inspection cockpit panels"
    );
    let logistics = compiled
        .resources
        .iter()
        .find(|r| r.id == "logistics_park_vector")
        .and_then(|r| r.dataset.as_ref())
        .expect("logistics_park_vector dataset");
    assert_eq!(
        logistics.rows.len(),
        3,
        "geojson FeatureCollection should yield 3 park rows"
    );
    let inspection_owner = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection_dashboard_ds")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection_dashboard_ds dataset");
    assert!(
        inspection_owner
            .runtime_metric_defs
            .contains_key("park_inspection_total_by_park"),
        "行政检查应内联分园区检查指标"
    );
    let datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let evaluated = evaluate_runtime_metric_defs(
        &inspection_owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&["park_inspection_total_by_park".to_string()]),
    )
    .expect("evaluate park_inspection_total_by_park");
    let by_park = evaluated
        .get("park_inspection_total_by_park")
        .expect("park_inspection_total_by_park");
    assert_eq!(by_park.shape, MetricShape::Dataframe);
    let by_park_rows = by_park
        .value
        .as_array()
        .or_else(|| by_park.value.get("value").and_then(|v| v.as_array()))
        .unwrap_or_else(|| panic!("dataframe rows expected, got: {}", by_park.value));
    assert!(
        !by_park_rows.is_empty(),
        "park_inspection_total_by_park should have grouped rows"
    );
    assert!(
        by_park_rows[0]
            .get("园区名称")
            .and_then(|v| v.as_str())
            .is_some(),
        "group_by should use 园区名称 field: {:?}",
        by_park_rows[0]
    );
}

#[test]
fn compile_spbjw_runtime_metric_defs_keep_drilldown_object_metadata() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let preview_targets = [
        "scenes/03-指标体系.mei",
        "scenes/07-问题办理.mei",
        "scenes/01-执法要素.mei",
        "scenes/02-行政检查.mei",
        "scenes/04-行政处罚.mei",
        "scenes/05-监督预警.mei",
        "scenes/08-监督成效.mei",
    ];
    let metric_ids = [
        "inspection_frequency_reduction_rate",
        "warnings_verification_rate",
        "effectiveness_verified_rectification_rate",
        "warnings_pending_count",
        "key_enterprises_count",
        "enforcement_items_count",
        "inspections_total_count",
        "inspections_today_count",
        "penalties_today_count",
        "administrative_reconsiderations_count",
        "penalty_revenue_growth_rate",
        "supervision_items_count",
        "effectiveness_transfer_clue_count",
    ];
    for target in preview_targets {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|_| panic!("compile {target} preview"));
        for metric_id in metric_ids {
            let metric = compiled.resources.iter().find_map(|resource| {
                let dataset = resource.dataset.as_ref()?;
                dataset.runtime_metric_defs.get(metric_id)
            });
            if let Some(metric) = metric {
                let meta = metric
                    .get("drilldown_dataset")
                    .or_else(|| metric.get("drilldown"))
                    .or_else(|| metric.get("explain"));
                if let Some(meta) = meta {
                    let is_non_empty = meta
                        .as_object()
                        .map(|value| !value.is_empty())
                        .or_else(|| meta.as_array().map(|value| !value.is_empty()))
                        .unwrap_or(false);
                    assert!(
                        is_non_empty,
                        "{metric_id} should keep non-empty explain/drilldown metadata in {target}"
                    );
                }
            }
        }
    }
}

#[test]
fn compile_spbjw_runtime_metric_defs_support_explain_list_shape() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let preview_targets = [
        (
            "scenes/08-监督成效.mei",
            "effectiveness_handled_person_times",
        ),
        (
            "scenes/07-问题办理.mei",
            "effectiveness_issue_verification_rate",
        ),
        (
            "scenes/03-指标体系.mei",
            "inspection_frequency_reduction_rate",
        ),
    ];
    for (target, metric_id) in preview_targets {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|_| panic!("compile {target} preview"));
        let explain = compiled.resources.iter().find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get(metric_id)
                .and_then(|metric| metric.get("explain"))
        });
        let explain =
            explain.unwrap_or_else(|| panic!("{metric_id} explain should exist in {target}"));
        let items = explain.as_array().unwrap_or_else(|| {
            panic!("{metric_id} explain should normalize to list in {target}: {explain:?}")
        });
        assert!(
            !items.is_empty(),
            "{metric_id} explain list should not be empty in {target}"
        );
    }
}

#[test]
fn compile_spbjw_home_preview_imported_world_metrics_align_analysis_contract_keys() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile home preview");
    let cases = [
        ("scenes/01-执法要素.mei", "enforcement_units_count"),
        (
            "scenes/08-监督成效.mei",
            "effectiveness_handled_person_times",
        ),
        (
            "scenes/03-指标体系.mei",
            "inspection_frequency_reduction_rate",
        ),
    ];
    for (capsule, local_metric_id) in cases {
        let resource_id = format!("__world_metrics__::{capsule}::metrics");
        let metric_key = format!("{capsule}::{local_metric_id}");
        let dataset = compiled
            .resources
            .iter()
            .find(|resource| resource.id == resource_id)
            .and_then(|resource| resource.dataset.as_ref())
            .unwrap_or_else(|| {
                panic!(
                    "home preview should include imported world metrics resource `{resource_id}`"
                )
            });
        assert!(
            dataset.runtime_metric_defs.contains_key(&metric_key),
            "expected runtime_metric_defs key `{metric_key}` on `{resource_id}`"
        );
        assert!(
            dataset
                .runtime_analysis_contracts
                .contains_key(&metric_key),
            "expected runtime_analysis_contracts key `{metric_key}` on `{resource_id}`, got keys: {:?}",
            dataset.runtime_analysis_contracts.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_home_embedded_map_world_metrics_materialized() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile home preview");
    let resource_id = "__world_metrics__::scenes/10-地图.mei::metrics";
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "home preview should include imported map world metrics `{resource_id}`, got: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter(|r| r.id.contains("10-地图") || r.id.contains("__world_metrics__"))
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    for metric_id in [
        "scenes/10-地图.mei::map_street_inspection_count_2025",
        "scenes/10-地图.mei::map_enterprise_poi_in_park_2025",
    ] {
        assert!(
            dataset.runtime_metric_defs.contains_key(metric_id),
            "expected `{metric_id}` on home map world metrics, keys: {:?}",
            dataset
                .runtime_metric_defs
                .keys()
                .filter(|k| k.contains("map_"))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_world_metrics_have_analysis_contracts() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("`{target}` direct preview should include __world_metrics__"));
    for metric_id in [
        "enforcement_units_count",
        "enforcement_personnel_count",
        "enforcement_items_count",
        "key_enterprises_count",
        "park_count",
        "whitelist_enterprises_count",
    ] {
        assert!(
            dataset.runtime_metric_defs.contains_key(metric_id),
            "expected runtime_metric_defs key `{metric_id}`, got: {:?}",
            dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
        );
        assert!(
            dataset.runtime_analysis_contracts.contains_key(metric_id),
            "expected runtime_analysis_contracts key `{metric_id}`, got: {:?}",
            dataset
                .runtime_analysis_contracts
                .keys()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_enforcement_elements_enforcement_units_resource_has_hydratable_source() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find_map(|resource| {
            resource
                .dataset
                .as_ref()
                .filter(|dataset| dataset.id == "enforcement_units")
                .cloned()
        })
        .unwrap_or_else(|| {
            panic!(
                "expected enforcement_units dataset, got ids: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter_map(|r| r.dataset.as_ref().map(|d| d.id.as_str()))
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(dataset.source.kind, "xlsx");
    assert!(
        dataset.source.path.contains("执法单位"),
        "unexpected source path: {}",
        dataset.source.path
    );
    assert!(
        !dataset.rows.is_empty(),
        "compile-time preview rows should not be empty"
    );
    let first = dataset.rows.first().cloned().unwrap_or_default();
    assert!(
        first.get("类别").is_some() || first.get("执法单位").is_some(),
        "expected schema-mapped row keys, got keys: {:?}",
        first.as_object().map(|m| m.keys().collect::<Vec<_>>())
    );
}

#[test]
fn spbjw_enforcement_items_count_rowset_matches_metric_value() {
    use mei_lang_datasets::{evaluate_runtime_metrics, query_metric_dataframe, DatasetQueryOptions};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let scene_id = compiled
        .active_scene
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default");
    let rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        "enforcement_items_count::__scalar_rowset__",
        Some(scene_id),
        Some(target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 16,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("enforcement_items_count rowset");
    let metric = evaluate_runtime_metrics(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        &["enforcement_items_count".to_string()],
        scene_id,
        Some(target),
        &Default::default(),
        &[],
        mei_lang_datasets::RuntimeMetricEvalMode::WithDag,
    )
    .expect("enforcement_items_count metric");
    let value = metric
        .metrics
        .iter()
        .find(|metric| metric.id == "enforcement_items_count")
        .and_then(|metric| metric.value.get("value").and_then(|value| value.as_f64()))
        .unwrap_or(0.0);
    assert_eq!(
        rowset.total as f64, value,
        "enforcement_items_count rowset total should match metric value"
    );
}

#[test]
fn spbjw_map_scene_world_metrics_can_evaluate() {
    use mei_lang_datasets::evaluate_runtime_metrics;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/10-地图.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("10-地图 preview should expose __world_metrics__"));
    let metric_id = dataset
        .runtime_metric_defs
        .keys()
        .find(|metric_id| metric_id == &&"map_park_penalty_count_2025".to_string())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "10-地图 runtime_metric_defs should include map_park_penalty_count_2025, got: {:?}",
                dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
            )
        });
    let scene_id = compiled
        .active_scene
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default");
    let metric = evaluate_runtime_metrics(
        &compiled,
        app_root.as_path(),
        "__world_metrics__",
        std::slice::from_ref(&metric_id),
        scene_id,
        Some(target),
        &Default::default(),
        &[],
        mei_lang_datasets::RuntimeMetricEvalMode::WithDag,
    )
    .expect("imported map world metric");
    let resolved = metric
        .metrics
        .iter()
        .find(|entry| entry.id == metric_id)
        .unwrap_or_else(|| panic!("expected metric `{metric_id}` in response"));
    assert!(
        resolved.value.get("value").is_some()
            || resolved.value.is_number()
            || resolved
                .value
                .as_array()
                .map(|rows| !rows.is_empty())
                .unwrap_or(false),
        "map world metric should resolve to scalar or non-empty grouped rows, got {:?}",
        resolved.value
    );
}

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_inferred_rowset_materializes_enforcement_units(
) {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("direct preview world metrics");
    let rowset_key = "enforcement_units_count::__scalar_rowset__";
    let rowset_def = dataset
        .runtime_metric_defs
        .get(rowset_key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| {
            panic!(
                "missing rowset def `{rowset_key}`, keys: {:?}",
                dataset.runtime_metric_defs.keys().collect::<Vec<_>>()
            )
        });
    eprintln!(
        "rowset def: {}",
        serde_json::to_string_pretty(rowset_def).unwrap()
    );
    let metric = dataset
        .metrics
        .get(rowset_key)
        .or_else(|| dataset.metrics.get("enforcement_units_count"));
    if let Some(m) = metric {
        eprintln!("metric value shape: {:?}", m.shape);
        eprintln!(
            "metric value: {}",
            serde_json::to_string(&m.value)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        );
    }
}

#[test]
fn compile_spbjw_home_preview_imported_enforcement_personnel_composition_tab_uses_real_rowset() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile home preview");
    let resource_id = "__world_metrics__::scenes/01-执法要素.mei::metrics";
    let metric_key = "scenes/01-执法要素.mei::enforcement_personnel_count";
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("missing `{resource_id}`"));
    let contract = dataset
        .runtime_analysis_contracts
        .get(metric_key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("missing contract `{metric_key}`"));
    let composition_metric_id = contract
        .get("tab_metrics")
        .and_then(|value| value.get("composition"))
        .and_then(|value| value.get("metric_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        composition_metric_id.ends_with("::composition_by_agency")
            || composition_metric_id.ends_with("::composition_by_rank"),
        "composition tab should bind to hoisted composition metric, got `{composition_metric_id}`"
    );
    assert!(
        !composition_metric_id.ends_with("::__scalar_rowset__"),
        "composition tab should not bind to raw scalar rowset"
    );
}

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_composition_tab_uses_rowset_not_dataset() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .expect("direct preview world metrics");
    let contract = dataset
        .runtime_analysis_contracts
        .get("enforcement_personnel_count")
        .and_then(Value::as_object)
        .expect("enforcement_personnel_count contract");
    let composition_metric_id = contract
        .get("tab_metrics")
        .and_then(|value| value.get("composition"))
        .and_then(|value| value.get("metric_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        composition_metric_id.contains("__scalar_rowset__"),
        "composition tab should bind to inferred scalar rowset, got `{composition_metric_id}`"
    );
    assert_ne!(
        composition_metric_id, "enforcement_units",
        "composition tab should not bind to raw dataset id"
    );
}

#[test]
fn compile_spbjw_runtime_metric_defs_expand_explain_scope_metric_nodes() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/08-监督成效.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|_| panic!("compile {target} preview"));
    let dataset = compiled
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .contains_key("effectiveness_handled_person_times")
                .then_some(dataset)
        })
        .expect("supervision effectiveness runtime metric defs");
    assert!(
        dataset
            .runtime_metric_defs
            .contains_key("effectiveness_handled_person_times::__scalar_rowset__"),
        "effectiveness_handled_person_times should hoist inferred scalar rowset child metric"
    );
    let contract = dataset
        .runtime_analysis_contracts
        .get("effectiveness_handled_person_times")
        .and_then(Value::as_object)
        .expect("handled analysis contract");
    let detail = contract
        .get("tab_metrics")
        .and_then(Value::as_object)
        .and_then(|tabs| tabs.get("detail"))
        .and_then(Value::as_object)
        .expect("detail tab metric");
    assert_eq!(
        detail.get("metric_id").and_then(Value::as_str),
        Some("effectiveness_handled_person_times::__scalar_rowset__")
    );
}

#[test]
fn spbjw_effectiveness_transfer_clue_and_filing_count_from_alert_tracking() {
    use std::collections::BTreeMap;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/08-监督成效.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let owner_dataset = compiled
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .contains_key("effectiveness_transfer_clue_count")
                .then_some(dataset)
        })
        .expect("08 capsule should materialize effectiveness metrics");
    let datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "effectiveness_transfer_clue_count".to_string(),
            "effectiveness_filing_count".to_string(),
            "effectiveness_mechanism_item_count".to_string(),
        ]),
    )
    .expect("evaluate supervision effectiveness clue/filing/mechanism metrics");
    let transfer = metrics
        .get("effectiveness_transfer_clue_count")
        .unwrap_or_else(|| panic!("missing metric `effectiveness_transfer_clue_count`"));
    let transfer_value = transfer
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| transfer.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        transfer_value, 4.0,
        "effectiveness_transfer_clue_count should count four 《11》 rows with 是否转问题线索=是, got {transfer_value}"
    );
    let filing = metrics
        .get("effectiveness_filing_count")
        .unwrap_or_else(|| panic!("missing metric `effectiveness_filing_count`"));
    let filing_value = filing
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| filing.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        filing_value, 4.0,
        "effectiveness_filing_count should match transfer clue count from 《11》, got {filing_value}"
    );
    let mechanism = metrics
        .get("effectiveness_mechanism_item_count")
        .unwrap_or_else(|| panic!("missing metric `effectiveness_mechanism_item_count`"));
    let mechanism_value = mechanism
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| mechanism.value.as_f64())
        .unwrap_or(f64::NAN);
    assert_eq!(
        mechanism_value, 10.0,
        "effectiveness_mechanism_item_count should dedupe 10 mechanism titles after splitting on 、 and 》《, got {mechanism_value}"
    );
}

#[test]
fn spbjw_indicator_system_calendar_year_metrics_use_inspection_xlsx_check_date() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::MetricShape;
    use ws_spbjw_integration_tests::{coerce_rows_to_schema, load_xlsx_table_snapshot};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/03-指标体系.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));

    let xlsx_path = app_root.join("upload/5.行政检查结果清单.xlsx");
    assert!(
        xlsx_path.is_file(),
        "expected inspection workbook at {}",
        xlsx_path.display()
    );
    let snapshot = load_xlsx_table_snapshot(
        &xlsx_path,
        "upload/5.行政检查结果清单.xlsx",
        Some("总表"),
        1,
        None,
    )
    .expect("load full inspection xlsx");
    assert!(
        !snapshot.rows.is_empty(),
        "inspection xlsx should contain rows"
    );

    let inspection_resource = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "administrative_inspection")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "compiled preview should expose administrative_inspection dataset; ids: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter(|r| r.dataset.is_some())
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        inspection_resource
            .schema
            .iter()
            .any(|column| column.name == "检查日期" && column.type_name == "date"),
        "administrative_inspection schema should declare 检查日期 as date: {:?}",
        inspection_resource.schema
    );

    let coerced_rows = coerce_rows_to_schema(snapshot.rows.clone(), &inspection_resource.schema);
    let count_2024 = coerced_rows
        .iter()
        .filter(|row| {
            row.get("检查日期")
                .and_then(|value| value.as_str())
                .map(|text| text.starts_with("2024"))
                .unwrap_or(false)
        })
        .count();
    let count_2025 = coerced_rows
        .iter()
        .filter(|row| {
            row.get("检查日期")
                .and_then(|value| value.as_str())
                .map(|text| text.starts_with("2025"))
                .unwrap_or(false)
        })
        .count();
    assert!(
        count_2024 > 0 && count_2025 > 0,
        "检查日期 should span 2024 and 2025 after schema coerce (2024={count_2024}, 2025={count_2025})"
    );

    let owner_dataset = compiled
        .resources
        .iter()
        .find(|resource| {
            resource.dataset.as_ref().is_some_and(|dataset| {
                dataset
                    .runtime_metric_defs
                    .contains_key("inspection_frequency_reduction_rate")
            })
        })
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("indicator metrics should be on a runtime metric owner"));

    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();

    let preview_only = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&["inspection_frequency_reduction_rate".to_string()]),
    )
    .expect("evaluate on compile-preview rows");
    let preview_value = preview_only
        .get("inspection_frequency_reduction_rate")
        .and_then(|metric| {
            metric
                .value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| metric.value.as_f64())
        })
        .unwrap_or(0.0);
    assert!(
        preview_value.is_finite() && preview_value.abs() > f64::EPSILON,
        "preview-materialized rows should already yield non-zero inspection_frequency_reduction_rate, got {preview_value}"
    );

    if let Some(dataset) = datasets.get_mut("administrative_inspection") {
        dataset.rows = coerced_rows;
    }

    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "inspection_frequency_reduction_rate".to_string(),
            "penalty_revenue_growth_rate".to_string(),
        ]),
    )
    .expect("evaluate indicator system calendar year metrics");

    let inspection_rate = metrics
        .get("inspection_frequency_reduction_rate")
        .expect("inspection_frequency_reduction_rate metric");
    assert_eq!(inspection_rate.shape, MetricShape::Scalar);
    let inspection_value = inspection_rate
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| inspection_rate.value.as_f64())
        .unwrap_or(0.0);
    assert!(
        inspection_value.is_finite() && inspection_value.abs() > f64::EPSILON,
        "inspection_frequency_reduction_rate should be non-zero with full xlsx rows, got {inspection_value}"
    );

    let penalty_schema = datasets
        .get("penalty_result_list")
        .expect("penalty_result_list dataset")
        .schema
        .clone();
    let penalty_path = app_root.join("upload/8.行政处罚结果清单.xlsx");
    let penalty_snapshot = load_xlsx_table_snapshot(
        &penalty_path,
        "upload/8.行政处罚结果清单.xlsx",
        None,
        1,
        None,
    )
    .expect("load full penalty xlsx");
    if let Some(dataset) = datasets.get_mut("penalty_result_list") {
        dataset.rows = coerce_rows_to_schema(penalty_snapshot.rows, &penalty_schema);
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&["penalty_revenue_growth_rate".to_string()]),
    )
    .expect("evaluate penalty revenue growth");
    let penalty_rate = metrics
        .get("penalty_revenue_growth_rate")
        .expect("penalty_revenue_growth_rate metric");
    let penalty_value = penalty_rate
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| penalty_rate.value.as_f64())
        .unwrap_or(0.0);
    assert!(
        penalty_value.is_finite() && penalty_value.abs() > f64::EPSILON,
        "penalty_revenue_growth_rate should be non-zero with full penalty rows, got {penalty_value}"
    );
}

#[test]
fn spbjw_home_scene_compile_includes_administrative_inspection_dataset() {
    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("home".to_string()),
            preview_target: None,
        },
    )
    .expect("compile home scene (access-style)");
    let inspection = compiled
        .resources
        .iter()
        .find(|r| {
            r.id == "administrative_inspection"
                || r.dataset
                    .as_ref()
                    .is_some_and(|d| d.id == "administrative_inspection")
        })
        .and_then(|r| r.dataset.as_ref());
    assert!(
        inspection.is_some(),
        "home scene compile must include administrative_inspection for indicator metrics; dataset ids: {:?}",
        compiled
            .resources
            .iter()
            .filter(|r| r.dataset.is_some())
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
    );
    let inspection = inspection.unwrap();
    assert!(
        !inspection.rows.is_empty(),
        "administrative_inspection should have preview rows on home compile"
    );
    assert!(
        inspection.schema.iter().any(|c| c.name == "检查日期"),
        "schema must include 检查日期"
    );
}

#[test]
fn spbjw_home_preview_imported_indicator_metrics_nonzero() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::resolve_runtime_metric_def_key;
    use ws_spbjw_integration_tests::MetricShape;
    use ws_spbjw_integration_tests::{coerce_rows_to_schema, load_xlsx_table_snapshot};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile home preview");

    let resource_id = "__world_metrics__::scenes/03-指标体系.mei::metrics";
    let metric_key = "scenes/03-指标体系.mei::inspection_frequency_reduction_rate".to_string();
    let owner_dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "home preview should include `{resource_id}`, resources: {:?}",
                compiled
                    .resources
                    .iter()
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    let resolved = resolve_runtime_metric_def_key(
        resource_id,
        "inspection_frequency_reduction_rate",
        &owner_dataset.runtime_metric_defs,
    )
    .unwrap_or_else(|| panic!("resolve imported metric key"));
    assert_eq!(resolved, metric_key.as_str());

    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    datasets
        .entry("administrative_inspection".to_string())
        .or_insert_with(|| {
            panic!("home compile should include administrative_inspection in datasets map")
        });

    let preview_metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[metric_key.clone()]),
    )
    .expect("evaluate on home preview rows without hydrate");
    let preview_value = preview_metrics
        .get(&metric_key)
        .and_then(|metric| {
            metric
                .value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| metric.value.as_f64())
        })
        .unwrap_or(0.0);
    assert!(
        preview_value.abs() > f64::EPSILON,
        "home preview rows alone should yield non-zero imported metric, got {preview_value}"
    );

    let xlsx_path = app_root.join("upload/5.行政检查结果清单.xlsx");
    let snapshot = load_xlsx_table_snapshot(
        &xlsx_path,
        "upload/5.行政检查结果清单.xlsx",
        Some("总表"),
        1,
        None,
    )
    .expect("load inspection xlsx");
    let schema = datasets
        .get("administrative_inspection")
        .expect("administrative_inspection")
        .schema
        .clone();
    if let Some(dataset) = datasets.get_mut("administrative_inspection") {
        dataset.rows = coerce_rows_to_schema(snapshot.rows, &schema);
    }

    let metric_ids = vec![metric_key.clone()];
    let metrics = evaluate_runtime_metric_defs(
        &owner_dataset.runtime_metric_defs,
        &[],
        &datasets,
        Some(metric_ids.as_slice()),
    )
    .expect("evaluate imported home metric");
    let metric = metrics
        .get(&metric_key)
        .or_else(|| metrics.get("inspection_frequency_reduction_rate"))
        .expect("imported metric result");
    assert_eq!(metric.shape, MetricShape::Scalar);
    let value = metric
        .value
        .get("value")
        .and_then(|v| v.as_f64())
        .or_else(|| metric.value.as_f64())
        .unwrap_or(0.0);
    assert!(
        value.is_finite() && value.abs() > f64::EPSILON,
        "home imported inspection_frequency_reduction_rate should be non-zero, got {value}"
    );
}

#[test]
fn compile_spbjw_enforcement_elements_personnel_rowset_evaluates_nonempty() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::resolve_runtime_metric_def_key;
    use ws_spbjw_integration_tests::MetricShape;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let resource_id = "__world_metrics__";
    let owner = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| {
            panic!(
                "native preview should expose `{resource_id}`, got: {:?}",
                compiled
                    .resources
                    .iter()
                    .filter(|r| r.id.contains("world_metrics"))
                    .map(|r| r.id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    let rowset_key = "enforcement_personnel_count::__scalar_rowset__";
    let resolved =
        resolve_runtime_metric_def_key(resource_id, rowset_key, &owner.runtime_metric_defs)
            .unwrap_or_else(|| panic!("resolve `{rowset_key}` on `{resource_id}`"));
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    let dataset_aliases: Vec<_> = datasets
        .values()
        .map(|dataset| (dataset.id.clone(), dataset.clone()))
        .collect();
    for (dataset_id, dataset) in dataset_aliases {
        datasets.entry(dataset_id).or_insert(dataset);
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[resolved.clone()]),
    )
    .unwrap_or_else(|error| panic!("evaluate `{resolved}` failed: {error}"));
    let metric = metrics
        .get(&resolved)
        .unwrap_or_else(|| panic!("missing metric `{resolved}`"));
    assert_eq!(metric.shape, MetricShape::Dataframe);
    let row_count = metric.value.as_array().map(|rows| rows.len()).unwrap_or(0);
    assert!(
        row_count > 0,
        "personnel rowset should materialize rows, got {row_count}"
    );
}

#[test]
fn compile_spbjw_home_imported_personnel_rowset_evaluates_nonempty() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::resolve_runtime_metric_def_key;
    use ws_spbjw_integration_tests::MetricShape;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile home preview");
    let resource_id = "__world_metrics__::scenes/01-执法要素.mei::metrics";
    let metric_key = "scenes/01-执法要素.mei::enforcement_personnel_count::__scalar_rowset__";
    let owner = compiled
        .resources
        .iter()
        .find(|resource| resource.id == resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("missing imported world metrics `{resource_id}`"));
    let resolved =
        resolve_runtime_metric_def_key(resource_id, metric_key, &owner.runtime_metric_defs)
            .unwrap_or_else(|| panic!("resolve imported rowset `{metric_key}`"));
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    let dataset_aliases: Vec<_> = datasets
        .values()
        .map(|dataset| (dataset.id.clone(), dataset.clone()))
        .collect();
    for (dataset_id, dataset) in dataset_aliases {
        datasets.entry(dataset_id).or_insert(dataset);
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[resolved.clone()]),
    )
    .unwrap_or_else(|error| panic!("evaluate imported rowset failed: {error}"));
    let metric = metrics.get(&resolved).expect("imported rowset metric");
    assert_eq!(metric.shape, MetricShape::Dataframe);
    let row_count = metric.value.as_array().map(|rows| rows.len()).unwrap_or(0);
    assert!(
        row_count > 0,
        "imported personnel rowset should materialize rows, got {row_count}"
    );
}

#[test]
#[ignore = "历史数据口径：park_migration_yearly scoped metric 已迁移，待单独恢复断言"]
fn spbjw_park_migration_yearly_table_evaluates_nonempty_rows() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::MetricShape;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));
    let world_metrics = compiled
        .resources
        .iter()
        .find(|resource| resource.id == "__world_metrics__")
        .and_then(|resource| resource.dataset.as_ref())
        .unwrap_or_else(|| panic!("`{target}` direct preview should include __world_metrics__"));
    let yearly_key = world_metrics
        .runtime_metric_defs
        .keys()
        .find(|key| key.contains("park_migration_yearly"))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "missing park_migration_yearly scoped metric, keys: {:?}",
                world_metrics.runtime_metric_defs.keys().collect::<Vec<_>>()
            )
        });
    let datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    let metrics = evaluate_runtime_metric_defs(
        &world_metrics.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[yearly_key.clone()]),
    )
    .unwrap_or_else(|error| panic!("evaluate park migration yearly failed: {error}"));
    let metric = metrics
        .get(&yearly_key)
        .unwrap_or_else(|| panic!("missing evaluated metric `{yearly_key}`"));
    assert_eq!(metric.shape, MetricShape::Dataframe);
    let row_count = metric.value.as_array().map(|rows| rows.len()).unwrap_or(0);
    assert!(
        row_count > 0,
        "park migration yearly wide table should have rows, got {row_count}; value={}",
        serde_json::to_string(&metric.value).unwrap_or_default()
    );
}

#[test]
fn compile_spbjw_qunfu_home_scene_succeeds() {
    let source_root = source_root();
    let app_root = source_root.join("qunfu");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile qunfu home failed: {error}"));
    assert!(
        compiled
            .diagnostics
            .iter()
            .all(|diag| !matches!(diag.severity, mei_lang_kernel::Severity::Error)),
        "qunfu home should not produce error diagnostics: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_contract.is_some(),
        "qunfu home should produce scene contract"
    );
}

#[test]
fn eval_spbjw_park_relocation_summary_and_charts_nonempty() {
    use std::collections::BTreeMap;

    use ws_spbjw_integration_tests::{coerce_rows_to_schema, load_xlsx_table_snapshot};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/01-执法要素.board.mei".to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile enforcement board failed: {error}"));
    let resource_id = "__world_metrics__::scenes/01-执法要素.world.mei::metrics";
    let owner = compiled
        .resources
        .iter()
        .find(|r| r.id == resource_id)
        .and_then(|r| r.dataset.as_ref())
        .or_else(|| {
            compiled
                .resources
                .iter()
                .find(|r| r.id.starts_with("__world_metrics__") && r.dataset.is_some())
                .and_then(|r| r.dataset.as_ref())
        })
        .unwrap_or_else(|| {
            let ids = compiled
                .resources
                .iter()
                .filter(|r| r.id.contains("world_metrics"))
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>();
            panic!("world metrics missing; candidates: {ids:?}");
        });
    let mut datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let relocation_key = if datasets.contains_key("enterprise_relocation") {
        "enterprise_relocation".to_string()
    } else {
        "scenes/01-执法要素.mei::enterprise_relocation".to_string()
    };
    let xlsx_path = app_root.join("upload/迁入迁出企业.xlsx");
    let snapshot = load_xlsx_table_snapshot(
        &xlsx_path,
        "upload/迁入迁出企业.xlsx",
        Some("企业迁入迁出记录"),
        1,
        None,
    )
    .expect("load relocation xlsx");
    {
        let relocation_dataset = datasets
            .get_mut(&relocation_key)
            .expect("enterprise_relocation dataset");
        let schema = relocation_dataset.schema.clone();
        relocation_dataset.rows = coerce_rows_to_schema(snapshot.rows, &schema);
        relocation_dataset.columns = schema.iter().map(|column| column.name.clone()).collect();
    }
    let metrics = evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "park_count::relocation_summary".to_string(),
            "park_count::relocation_by_month".to_string(),
            "park_count::relocation_by_park".to_string(),
        ]),
    )
    .expect("evaluate park relocation metrics");
    let summary_rows = metrics
        .get("park_count::relocation_summary")
        .and_then(|metric| metric.value.as_array())
        .map(|rows| rows.len())
        .unwrap_or(0);
    let month_rows = metrics
        .get("park_count::relocation_by_month")
        .and_then(|metric| metric.value.as_array())
        .map(|rows| rows.len())
        .unwrap_or(0);
    assert!(
        summary_rows > 0,
        "relocation_summary should have rows, got {summary_rows}"
    );
    assert!(
        month_rows > 0,
        "relocation_by_month should have rows, got {month_rows}"
    );
    let month_sample = metrics
        .get("park_count::relocation_by_month")
        .and_then(|metric| metric.value.as_array())
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("年月"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    assert!(
        month_sample.len() == 7 && month_sample.chars().nth(4) == Some('-'),
        "年月 should be yyyy-mm, got {month_sample}"
    );
    assert!(
        month_rows < 84,
        "bucket_date should collapse day-level rows, got {month_rows}"
    );
}

fn assert_calendar_field_is_date_only(row: &Value, field: &str) {
    let Some(value) = row.get(field) else {
        return;
    };
    if value.is_null() {
        return;
    }
    let text = value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string());
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "？" || trimmed == "?" || trimmed == "——" || trimmed == "--"
    {
        return;
    }
    assert!(
        !trimmed.contains(':'),
        "field `{field}` should be calendar date without time, got `{trimmed}`"
    );
    assert!(
        trimmed.len() >= 10
            && trimmed.as_bytes().get(4) == Some(&b'-')
            && trimmed.as_bytes().get(7) == Some(&b'-'),
        "field `{field}` should look like yyyy-mm-dd, got `{trimmed}`"
    );
}

#[test]
fn spbjw_warning_and_issue_result_metric_dataframe_dates_are_calendar_only() {
    use mei_lang_datasets::{query_dataset_rows, query_metric_dataframe, DatasetQueryOptions};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/05-监督预警.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{board_target}` failed: {error}"));

    let warning_metric = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "warning_list",
        "warnings_count::__scalar_rowset__",
        Some("warnings_analytics_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 50,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("warning_list metric dataframe query");
    assert!(
        !warning_metric.rows.is_empty(),
        "warnings_count detail should return rows"
    );
    for row in &warning_metric.rows {
        for field in ["预警时间", "分办时间", "办结时间"] {
            assert_calendar_field_is_date_only(row, field);
        }
    }

    let issue_board = "scenes/08-监督成效.board.mei";
    let issue_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(issue_board.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{issue_board}` failed: {error}"));
    let issue_metric = query_metric_dataframe(
        &issue_compiled,
        app_root.as_path(),
        "mechanism_documents",
        "effectiveness_mechanism_item_count::mechanism_documents_list",
        Some("effect_mechanism_documents_board"),
        Some("scenes/_shared/mechanism-documents.board.mei"),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 50,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("mechanism_documents metric dataframe query");
    assert_eq!(
        issue_metric.rows.len(),
        10,
        "mechanism documents list should expose 10 mapped mechanism rows"
    );
    for row in &issue_metric.rows {
        let name = row.get("机制名称").and_then(Value::as_str).unwrap_or("");
        assert!(
            !name.trim().is_empty(),
            "mechanism document row should include 机制名称, got: {row:?}"
        );
    }

    let warning_list = issue_compiled
        .resources
        .iter()
        .find_map(|resource| {
            resource
                .dataset
                .as_ref()
                .filter(|dataset| dataset.id == "warning_list")
                .cloned()
        })
        .expect("warning_list dataset view");
    let warning_rows = query_dataset_rows(
        app_root.as_path(),
        &warning_list,
        DatasetQueryOptions {
            page: 1,
            page_size: 20,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
    )
    .expect("warning_list direct query");
    assert!(!warning_rows.rows.is_empty(), "warning_list rows query");
    for row in warning_rows.rows.iter().take(20) {
        for field in ["预警时间", "分办时间", "办结时间"] {
            assert_calendar_field_is_date_only(row, field);
        }
    }

    let realtime_target = "scenes/06-实时预警.mei";
    let realtime_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(realtime_target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{realtime_target}` failed: {error}"));
    let realtime_metric = query_metric_dataframe(
        &realtime_compiled,
        app_root.as_path(),
        "__world_metrics__",
        "warnings_realtime_cockpit_table",
        Some("realtime_warnings"),
        Some(realtime_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 10,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("warnings_realtime_cockpit_table metric dataframe query");
    assert!(
        !realtime_metric.rows.is_empty(),
        "realtime cockpit table should return rows"
    );
    for row in &realtime_metric.rows {
        assert_calendar_field_is_date_only(row, "预警时间");
    }
}

#[test]
fn spbjw_enforcement_personnel_composition_by_agency_returns_grouped_rows() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let target = "scenes/01-执法要素.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{target}` failed: {error}"));

    let composition = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "enforcement_officers",
        "scenes/01-执法要素.mei::enforcement_personnel_count::composition_by_agency",
        Some("enforcement_elements"),
        Some(target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 16,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("enforcement_personnel_count composition_by_agency");
    assert!(
        composition.total > 0,
        "composition_by_agency should group officers by 所属部门, got total={} rows={:?}",
        composition.total,
        composition.rows
    );
}

#[test]
fn spbjw_penalty_total_rowset_query_returns_more_than_preview_rows() {
    use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};
    use ws_spbjw_integration_tests::load_xlsx_table_snapshot;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let board_target = "scenes/04-行政处罚.board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(board_target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{board_target}` failed: {error}"));

    let assembly = compiled
        .scene_projection_assembly_by_id
        .get("penalty_total_analytics_board")
        .and_then(Value::as_object)
        .expect("penalty_total_analytics_board assembly");
    let detail_metric_id = assembly
        .get("projection_slots")
        .and_then(Value::as_array)
        .and_then(|slots| {
            slots.iter().find(|slot| {
                slot.as_object()
                    .and_then(|map| map.get("layout_zone"))
                    .and_then(Value::as_str)
                    == Some("detail")
            })
        })
        .and_then(Value::as_object)
        .and_then(|slot| slot.get("metric_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .expect("detail slot metric_id");
    assert!(
        detail_metric_id.ends_with("::__scalar_rowset__"),
        "penalty detail slot should bind scalar rowset, got `{detail_metric_id}`"
    );

    let penalty_snapshot = load_xlsx_table_snapshot(
        &app_root.join("upload/8.行政处罚结果清单.xlsx"),
        "upload/8.行政处罚结果清单.xlsx",
        None,
        1,
        None,
    )
    .expect("load full penalty xlsx");
    assert!(
        penalty_snapshot.rows.len() > 1000,
        "fixture should contain more than preview_rows=1000"
    );

    let rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        &detail_metric_id,
        Some("penalty_total_analytics_board"),
        Some(board_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 20,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("penalties_total_count rowset query");
    assert_eq!(
        rowset.total,
        penalty_snapshot.rows.len(),
        "penalty detail query should materialize full xlsx rows, not compile-time preview cap"
    );
}

#[test]
fn spbjw_penalty_filter_prefetch_does_not_cap_rowset_materialization() {
    use mei_lang_datasets::{
        clear_dataset_rows_cache, query_dataset_rows, query_metric_dataframe, DatasetQueryOptions,
    };
    use ws_spbjw_integration_tests::load_xlsx_table_snapshot;

    clear_dataset_rows_cache();

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let scene_target = "scenes/04-行政处罚.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(scene_target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile `{scene_target}` failed: {error}"));

    let penalty_snapshot = load_xlsx_table_snapshot(
        &app_root.join("upload/8.行政处罚结果清单.xlsx"),
        "upload/8.行政处罚结果清单.xlsx",
        None,
        1,
        None,
    )
    .expect("load full penalty xlsx");
    assert!(penalty_snapshot.rows.len() > 1000);

    // 模拟 filter-bar 首次拉取 rowset 选项：page_size=1000、非 collect_all。
    let prefetch = query_dataset_rows(
        app_root.as_path(),
        compiled
            .resources
            .iter()
            .find(|resource| resource.id == "penalty_result_dashboard_ds")
            .and_then(|resource| resource.dataset.as_ref())
            .expect("penalty_result_dashboard_ds"),
        DatasetQueryOptions {
            page: 1,
            page_size: 1000,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
    )
    .expect("penalty filter prefetch");
    assert_eq!(prefetch.rows.len(), 1000);
    assert_eq!(prefetch.total, penalty_snapshot.rows.len());

    let rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        "scenes/04-行政处罚.mei::penalties_total_count::__scalar_rowset__",
        Some("penalty_dashboard"),
        Some(scene_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 8,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("penalty rowset after filter prefetch");
    assert_eq!(
        rowset.total,
        penalty_snapshot.rows.len(),
        "filter prefetch must not poison rowset materialization to preview/page cap"
    );

    clear_dataset_rows_cache();

    let week_rowset = query_metric_dataframe(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        "scenes/04-行政处罚.mei::penalties_week_count::__scalar_rowset__",
        Some("penalty_dashboard"),
        Some(scene_target),
        "integration-test",
        DatasetQueryOptions {
            page: 1,
            page_size: 8,
            collect_all: false,
            ..DatasetQueryOptions::default()
        },
        None,
        Vec::new(),
    )
    .expect("penalty week rowset");
    let week_metric = mei_lang_datasets::evaluate_runtime_metrics(
        &compiled,
        app_root.as_path(),
        "penalty_result_dashboard_ds",
        &["penalties_week_count".to_string()],
        "penalty_dashboard",
        Some(scene_target),
        &Default::default(),
        &[],
        mei_lang_datasets::RuntimeMetricEvalMode::WithDag,
    )
    .expect("penalties_week_count metric");
    let week_count = week_metric
        .metrics
        .iter()
        .find(|metric| metric.id == "penalties_week_count")
        .and_then(|metric| metric.value.get("value").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    assert_eq!(
        week_rowset.total as f64, week_count,
        "week detail rowset total should match card metric value"
    );
}

#[test]
fn spbjw_shell_and_scene_theme_injection_use_separate_css_var_tracks() {
    use mei_lang_app::{page_body_theme_style, scene_viewport_theme_style};
    use mei_lang_kernel::load_workspace_config;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let workspace = load_workspace_config(&source_root);
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile spbjw home preview");
    let body_style = page_body_theme_style(&workspace, Some(&compiled));
    assert!(
        body_style.contains("--mei-shell-color-"),
        "workspace shell theme should inject --mei-shell-color-* on body"
    );
    assert!(
        body_style.contains("--mei-color-"),
        "page body should inject scene vars for overlays"
    );
    let scene_style = scene_viewport_theme_style(&compiled);
    assert!(
        scene_style.contains("--mei-color-"),
        "viewport scene theme should inject --mei-color-*"
    );
    assert!(
        !scene_style.contains("--mei-shell-color-"),
        "viewport must not inject shell color vars"
    );
}
