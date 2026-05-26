use super::{compile_app_from_root_with_options, workspace_root, CompileOptions};

#[test]
fn compile_spbjw_preview_typical_cases_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/5_典型案例/监督典型案例.mei".to_string()),
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
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.scene_routes.iter().any(|r| {
            r.scene_id == "typical_cases" && r.target_file == "scenes/5_典型案例/监督典型案例.mei"
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
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
        "scenes/5_典型案例/监督典型案例.mei"
    );
    assert_eq!(compiled.active_scene.as_deref(), Some("typical_cases"));
}

#[test]
fn compile_spbjw_select_enterprise_complaints_scene_resolves_dataset_entry() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("enterprise_complaints".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with enterprise_complaints scene (discovered route)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/2_行政检查/企业投诉.mei"
    );
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enterprise_complaints")
    );
}

#[test]
fn compile_spbjw_preview_enforcement_whitelist_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw enterprise whitelist preview");
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
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_dataset_preview_with_wrong_scene_query_still_resolves_entry_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("企业白名单".to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw whitelist with filename-like scene query");
    assert_eq!(compiled.active_target_file, target);
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enterprise_whitelist")
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/企业白名单.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: Some("enterprise_whitelist".to_string()),
            preview_target: Some(target.to_string()),
        },
    )
    .expect("compile spbjw whitelist scene+focus");
    assert_eq!(
        compiled.active_scene.as_deref(),
        Some("enterprise_whitelist")
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layouts/左栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout left preview");
    let elapsed = started.elapsed();
    assert_eq!(compiled.active_target_file, "scenes/layouts/左栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
        contract.panels.len() >= 3,
        "layout left should resolve frame.panels panel_ref slots, got {}",
        contract.panels.len()
    );
    assert!(
        contract.panels.iter().any(|p| !p.blocks.is_empty()),
        "layout left panels should carry blocks from external panel lookup"
    );
    let stats = contract
        .panels
        .iter()
        .find(|p| p.id == "enforcement_elements_stats")
        .expect("enforcement stats panel from panel_ref");
    let panel_layout = stats
        .layout
        .as_ref()
        .expect("panel_ref must preserve panel.layout from source");
    assert_eq!(panel_layout.layout_type, "grid");
    assert!(
        !stats.blocks.is_empty(),
        "stats panel should carry title + metrics body blocks"
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
        dataset_resources.len() <= 14,
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
fn compile_spbjw_preview_widget_metrics_system_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督和问题办理/预警模型.mei".to_string()),
        },
    )
    .expect("compile spbjw warning models preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "metrics widget preview errors: {:?}",
        errors
    );
    assert_eq!(
        compiled.active_target_file,
        "scenes/4_监督和问题办理/预警模型.mei"
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_models"),
        "expected warning_models dataset in resources"
    );
}

#[test]
fn compile_spbjw_preview_widget_supervision_warning_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layouts/右栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout right preview");
    assert_eq!(compiled.active_target_file, "scenes/layouts/右栏.mei");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
        contract.panels.len() >= 4,
        "layout right should resolve multiple panel_ref slots, got {}",
        contract.panels.len()
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
fn compile_spbjw_preview_widget_typical_cases_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let started = std::time::Instant::now();
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/5_典型案例/监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw typical cases preview");
    let elapsed = started.elapsed();
    assert_eq!(
        compiled.active_target_file,
        "scenes/5_典型案例/监督典型案例.mei"
    );
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/layouts/左栏.mei";
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
        !dataset.runtime_metric_defs.is_empty(),
        "imported dataset should carry runtime metric defs"
    );
    assert!(
        dataset.metrics.contains_key("enforcement_units_count"),
        "expected enforcement_units_count metric"
    );
}

#[test]
fn compile_spbjw_preview_home_scene_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
        contract.panels.len() >= 10,
        "home should flatten panel_ref slots into scene panels, got {}",
        contract.panels.len()
    );
    for area in [
        "header",
        "left_1",
        "left_2",
        "left_3",
        "center_top",
        "center_bottom",
        "right_1",
        "right_4",
    ] {
        assert!(
            contract
                .panels
                .iter()
                .any(|panel| panel.area.as_deref() == Some(area)),
            "missing grid area panel: {area}"
        );
    }
    let overview = contract
        .panels
        .iter()
        .find(|p| p.id == "enforcement_elements_stats")
        .expect("enforcement elements stats from panel(base=panel_ref)");
    assert!(
        !overview.blocks.is_empty(),
        "home panel(base=panel_ref) should inherit blocks from external panel"
    );
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
    let inspection = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection from 行政检查.mei");
    assert!(inspection.metrics.contains_key("inspections_total_count"));
    assert!(inspection.metrics.contains_key("park_inspection_count"));
}

#[test]
fn compile_spbjw_preview_main_mei_keeps_inspection_and_penalty_cockpit_metrics() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("main.mei".to_string()),
        },
    )
    .expect("compile spbjw main preview");
    let inspection = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection");
    assert!(inspection.metrics.contains_key("inspections_total_count"));
    assert!(inspection
        .metrics
        .contains_key("inspections_6m_count_trend"));
    let penalty = compiled
        .resources
        .iter()
        .find(|r| r.id == "penalty_result_list")
        .and_then(|r| r.dataset.as_ref())
        .expect("penalty_result_list");
    assert!(penalty.metrics.contains_key("penalties_today_count"));
    assert!(penalty.metrics.contains_key("penalties_6m_amount_trend"));
    assert!(
        inspection.metrics.contains_key("park_inspection_count"),
        "catalog should merge park metrics without dropping cockpit defs"
    );
}

#[test]
fn compile_spbjw_preview_logistics_park_vector_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/6_物流园区/园区统计.mei".to_string()),
        },
    )
    .expect("compile spbjw logistics preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "logistics_park_vector preview errors: {:?}",
        errors
    );
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("preview scene contract");
    assert!(
        !contract.panels.is_empty(),
        "expected stats/charts/table panels"
    );
    let logistics = compiled
        .resources
        .iter()
        .find(|r| r.id == "logistics_park_vector")
        .and_then(|r| r.dataset.as_ref())
        .expect("logistics_park_vector dataset");
    assert!(logistics.metrics.contains_key("logistics_parks_count"));
    assert_eq!(
        logistics.rows.len(),
        3,
        "geojson FeatureCollection should yield 3 park rows"
    );
    let inspection = compiled
        .resources
        .iter()
        .find(|r| r.id == "administrative_inspection")
        .and_then(|r| r.dataset.as_ref())
        .expect("administrative_inspection dataset");
    assert!(
        inspection.metrics.contains_key("park_inspection_count"),
        "catalog should merge 园区统计 park metrics into administrative_inspection"
    );
    let inspection_by_park = inspection
        .metrics
        .get("park_inspection_count")
        .expect("park_inspection_count metric");
    let by_park_rows = inspection_by_park
        .value
        .as_array()
        .or_else(|| {
            inspection_by_park
                .value
                .get("value")
                .and_then(|v| v.as_array())
        })
        .unwrap_or_else(|| {
            panic!(
                "dataframe metric rows expected array, got: {}",
                inspection_by_park.value
            );
        });
    assert!(
        !by_park_rows.is_empty(),
        "park_inspection_count should have grouped rows, got {by_park_rows:?}"
    );
    assert!(
        by_park_rows[0]
            .get("园区名称")
            .and_then(|v| v.as_str())
            .is_some(),
        "group_by should use 园区名称 field, not label: {:?}",
        by_park_rows[0]
    );
    let total = inspection
        .metrics
        .get("park_inspection_total")
        .and_then(|m| {
            m.value
                .get("value")
                .and_then(|v| v.as_f64())
                .or_else(|| m.value.as_f64())
        })
        .unwrap_or(-1.0);
    assert!(
        total > 0.0 && total < 100.0,
        "park_inspection_total should be enterprise-matched inspections on preview rows, got {total}"
    );
}

#[test]
fn compile_spbjw_runtime_metric_defs_keep_drilldown_object_metadata() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");

    let indicator = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/2_行政检查/指标体系.mei".to_string()),
        },
    )
    .expect("compile indicator_system preview");
    let warning_dataset = indicator
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("warnings_verification_rate")
                .map(|metric| (resource.id.clone(), metric))
        })
        .unwrap_or_else(|| {
            let resources: Vec<_> = indicator.resources.iter().map(|r| r.id.as_str()).collect();
            panic!(
                "warnings_verification_rate runtime def should exist, resources: {resources:?}"
            )
        });
    let (warning_dataset_id, warning_metric) = warning_dataset;
    let inspection_metric = indicator
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("inspection_frequency_reduction_rate")
        })
        .expect("inspection_frequency_reduction_rate runtime def should exist");
    let inspection_drilldown = inspection_metric
        .get("drilldown_dataset")
        .or_else(|| inspection_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("inspection_frequency_reduction_rate drilldown should remain object");
    assert_eq!(
        inspection_drilldown
            .get("kind")
            .and_then(|value| value.as_str()),
        Some("mom")
    );
    assert_eq!(
        inspection_drilldown
            .get("basis_refs")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(2)
    );
    let inspection_ratio_parts = inspection_drilldown
        .get("ratio_parts")
        .and_then(|value| value.as_object())
        .expect("inspection_frequency_reduction_rate ratio_parts should exist");
    assert_eq!(
        inspection_ratio_parts
            .get("formula")
            .and_then(|value| value.as_str()),
        Some("(最近月检查次数 - 上月检查次数) / 上月检查次数")
    );
    let inspection_tab_metrics = inspection_drilldown
        .get("tab_metrics")
        .or_else(|| inspection_drilldown.get("tabMetrics"))
        .and_then(|value| value.as_object())
        .expect("inspection_frequency_reduction_rate tab_metrics should exist");
    let inspection_trend_tab = inspection_tab_metrics
        .get("trend")
        .and_then(|value| value.as_object())
        .expect("inspection_frequency_reduction_rate trend tab should exist");
    assert_eq!(
        inspection_trend_tab
            .get("table_metric_id")
            .and_then(|value| value.as_str()),
        Some("inspections_6m_count_trend")
    );
    let warning_drilldown = warning_metric
        .get("drilldown_dataset")
        .or_else(|| warning_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("warnings_verification_rate drilldown should remain object");
    assert_eq!(
        warning_drilldown
            .get("target_scene_id")
            .and_then(|value| value.as_str()),
        Some("warning_list")
    );
    assert_eq!(
        warning_drilldown
            .get("table_metric_id")
            .and_then(|value| value.as_str()),
        Some("warnings_verification_breakdown_table")
    );
    assert_eq!(
        warning_drilldown
            .get("basis_refs")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(3)
    );
    assert_eq!(
        warning_drilldown
            .get("detail_fields")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(6)
    );
    let warning_ratio_parts = warning_drilldown
        .get("ratio_parts")
        .and_then(|value| value.as_object())
        .expect("warnings_verification_rate ratio_parts should exist");
    assert_eq!(
        warning_ratio_parts
            .get("numerator")
            .and_then(|value| value.as_str()),
        Some("已查实预警数")
    );
    assert_eq!(
        warning_ratio_parts
            .get("denominator")
            .and_then(|value| value.as_str()),
        Some("预警总数（按预警ID去重）")
    );
    let rectification_metric = indicator
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("effectiveness_verified_rectification_rate")
        })
        .expect("effectiveness_verified_rectification_rate runtime def should exist");
    let rectification_drilldown = rectification_metric
        .get("drilldown_dataset")
        .or_else(|| rectification_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("effectiveness_verified_rectification_rate drilldown should remain object");
    assert_eq!(
        rectification_drilldown
            .get("table_metric_id")
            .and_then(|value| value.as_str()),
        Some("effectiveness_verified_rectification_breakdown_table")
    );
    let rectification_ratio_parts = rectification_drilldown
        .get("ratio_parts")
        .and_then(|value| value.as_object())
        .expect("effectiveness_verified_rectification_rate ratio_parts should exist");
    assert_eq!(
        rectification_ratio_parts
            .get("denominator")
            .and_then(|value| value.as_str()),
        Some("已查实问题总数")
    );
    let rectification_tab_metrics = rectification_drilldown
        .get("tab_metrics")
        .or_else(|| rectification_drilldown.get("tabMetrics"))
        .and_then(|value| value.as_object())
        .expect("effectiveness_verified_rectification_rate tab_metrics should exist");
    let rectification_detail_tab = rectification_tab_metrics
        .get("detail")
        .and_then(|value| value.as_object())
        .expect("effectiveness_verified_rectification_rate detail tab should exist");
    assert_eq!(
        rectification_detail_tab
            .get("dataset_id")
            .and_then(|value| value.as_str()),
        Some("issue_result_list")
    );
    assert!(
        !warning_dataset_id.is_empty(),
        "warnings_verification_rate should resolve to a concrete dataset id"
    );

    let issue = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督和问题办理/问题办理.mei".to_string()),
        },
    )
    .expect("compile issue_handling preview");
    let issue_warning_dataset = issue
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("warnings_pending_count")
                .map(|metric| (resource.id.clone(), metric))
        })
        .unwrap_or_else(|| {
            let resources: Vec<_> = issue.resources.iter().map(|r| r.id.as_str()).collect();
            panic!(
                "warnings_pending_count runtime def should exist, resources: {resources:?}"
            )
        });
    let (issue_warning_dataset_id, issue_warning_metric) = issue_warning_dataset;
    let issue_warning_drilldown = issue_warning_metric
        .get("drilldown_dataset")
        .or_else(|| issue_warning_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("warnings_pending_count drilldown should remain object");
    assert_eq!(
        issue_warning_drilldown
            .get("target_scene_id")
            .and_then(|value| value.as_str()),
        Some("warning_list")
    );
    assert_eq!(
        issue_warning_drilldown
            .get("layout_preset")
            .and_then(|value| value.as_str()),
        Some("drilldown_warnings")
    );
    assert_eq!(
        issue_warning_drilldown
            .get("basis_refs")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(3)
    );
    assert_eq!(
        issue_warning_drilldown
            .get("detail_fields")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(6)
    );
    assert!(
        !issue_warning_dataset_id.is_empty(),
        "warnings_pending_count should resolve to a concrete dataset id"
    );

    let enforcement = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/1_执法要素/执法要素.mei".to_string()),
        },
    )
    .expect("compile enforcement_elements preview");
    let enterprise_dataset = enforcement
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("key_enterprises_count")
                .map(|metric| (resource.id.clone(), metric))
        })
        .unwrap_or_else(|| {
            let resources: Vec<_> = enforcement.resources.iter().map(|r| r.id.as_str()).collect();
            panic!(
                "key_enterprises_count runtime def should exist, resources: {resources:?}"
            )
        });
    let (enterprise_dataset_id, enterprise_metric) = enterprise_dataset;
    let enterprise_drilldown = enterprise_metric
        .get("drilldown_dataset")
        .or_else(|| enterprise_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("key_enterprises_count drilldown should remain object");
    assert_eq!(
        enterprise_drilldown
            .get("target_scene_id")
            .and_then(|value| value.as_str()),
        Some("key_enterprises")
    );
    assert!(
        !enterprise_dataset_id.is_empty(),
        "key_enterprises_count should resolve to a concrete dataset id"
    );
    let matters_metric = enforcement
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("enforcement_items_count")
        })
        .expect("enforcement_items_count runtime def should exist");
    let matters_drilldown = matters_metric
        .get("drilldown_dataset")
        .or_else(|| matters_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("enforcement_items_count drilldown should remain object");
    assert_eq!(
        matters_drilldown
            .get("target_scene_id")
            .and_then(|value| value.as_str()),
        Some("enforcement_matters")
    );
    let matters_tab_metrics = matters_drilldown
        .get("tab_metrics")
        .or_else(|| matters_drilldown.get("tabMetrics"))
        .and_then(|value| value.as_object())
        .expect("enforcement_items_count tab_metrics should exist");
    assert_eq!(
        matters_tab_metrics
            .get("composition")
            .and_then(|value| value.as_object())
            .and_then(|value| value.get("table_metric_id"))
            .and_then(|value| value.as_str()),
        Some("enforcement_items_by_domain_ranking")
    );

    let inspection = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/2_行政检查/行政检查.mei".to_string()),
        },
    )
    .expect("compile administrative_inspection preview");
    let inspection_today_metric = inspection
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset.runtime_metric_defs.get("inspections_today_count")
        })
        .expect("inspections_today_count runtime def should exist");
    let inspection_total_metric = inspection
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset.runtime_metric_defs.get("inspections_total_count")
        })
        .expect("inspections_total_count runtime def should exist");
    let inspection_total_drilldown = inspection_total_metric
        .get("drilldown_dataset")
        .or_else(|| inspection_total_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("inspections_total_count drilldown should remain object");
    assert_eq!(
        inspection_total_drilldown
            .get("tab_metrics")
            .or_else(|| inspection_total_drilldown.get("tabMetrics"))
            .and_then(|value| value.as_object())
            .and_then(|tabs| tabs.get("trend"))
            .and_then(|value| value.as_object())
            .and_then(|value| value.get("table_metric_id"))
            .and_then(|value| value.as_str()),
        Some("inspections_6m_count_trend")
    );
    let inspection_today_drilldown = inspection_today_metric
        .get("drilldown_dataset")
        .or_else(|| inspection_today_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("inspections_today_count drilldown should remain object");
    assert_eq!(
        inspection_today_drilldown
            .get("table_metric_id")
            .and_then(|value| value.as_str()),
        Some("inspections_today_detail_table")
    );
    assert_eq!(
        inspection_today_drilldown
            .get("detail_fields")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(11)
    );

    let penalty = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/3_行政处罚/行政处罚.mei".to_string()),
        },
    )
    .expect("compile penalty_dashboard preview");
    let penalty_today_metric = penalty
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset.runtime_metric_defs.get("penalties_today_count")
        })
        .expect("penalties_today_count runtime def should exist");
    let penalty_today_drilldown = penalty_today_metric
        .get("drilldown_dataset")
        .or_else(|| penalty_today_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("penalties_today_count drilldown should remain object");
    assert_eq!(
        penalty_today_drilldown
            .get("table_metric_id")
            .and_then(|value| value.as_str()),
        Some("penalties_today_detail_table")
    );
    let reconsideration_metric = penalty
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("administrative_reconsiderations_count")
        })
        .expect("administrative_reconsiderations_count runtime def should exist");
    let reconsideration_drilldown = reconsideration_metric
        .get("drilldown_dataset")
        .or_else(|| reconsideration_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("administrative_reconsiderations_count drilldown should remain object");
    assert_eq!(
        reconsideration_drilldown
            .get("target_scene_id")
            .and_then(|value| value.as_str()),
        Some("admin_reconsideration_register")
    );
    let penalty_growth_metric = penalty
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset.runtime_metric_defs.get("penalty_revenue_growth_rate")
        })
        .expect("penalty_revenue_growth_rate runtime def should exist");
    let penalty_growth_drilldown = penalty_growth_metric
        .get("drilldown_dataset")
        .or_else(|| penalty_growth_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("penalty_revenue_growth_rate drilldown should remain object");
    assert_eq!(
        penalty_growth_drilldown
            .get("tab_metrics")
            .or_else(|| penalty_growth_drilldown.get("tabMetrics"))
            .and_then(|value| value.as_object())
            .and_then(|tabs| tabs.get("composition"))
            .and_then(|value| value.as_object())
            .and_then(|value| value.get("table_metric_id"))
            .and_then(|value| value.as_str()),
        Some("park_penalty_amount_by_park")
    );

    let warning = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督和问题办理/监督预警.mei".to_string()),
        },
    )
    .expect("compile supervision_warning preview");
    let supervision_metric = warning
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset.runtime_metric_defs.get("supervision_items_count")
        })
        .expect("supervision_items_count runtime def should exist");
    let supervision_drilldown = supervision_metric
        .get("drilldown_dataset")
        .or_else(|| supervision_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("supervision_items_count drilldown should remain object");
    assert_eq!(
        supervision_drilldown
            .get("target_scene_id")
            .and_then(|value| value.as_str()),
        Some("supervision_matters")
    );
    assert_eq!(
        supervision_drilldown
            .get("layout_preset")
            .and_then(|value| value.as_str()),
        Some("drilldown_matters")
    );
    assert_eq!(
        supervision_drilldown
            .get("detail_fields")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(5)
    );

    let effectiveness = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督和问题办理/监督成效.mei".to_string()),
        },
    )
    .expect("compile supervision_effectiveness preview");
    let transfer_metric = effectiveness
        .resources
        .iter()
        .find_map(|resource| {
            let dataset = resource.dataset.as_ref()?;
            dataset
                .runtime_metric_defs
                .get("effectiveness_transfer_clue_count")
        })
        .expect("effectiveness_transfer_clue_count runtime def should exist");
    let transfer_drilldown = transfer_metric
        .get("drilldown_dataset")
        .or_else(|| transfer_metric.get("drilldown"))
        .and_then(|value| value.as_object())
        .expect("effectiveness_transfer_clue_count drilldown should remain object");
    assert_eq!(
        transfer_drilldown
            .get("table_metric_id")
            .and_then(|value| value.as_str()),
        Some("issue_results_transfer_clue_table")
    );
    assert_eq!(
        transfer_drilldown
            .get("basis_refs")
            .and_then(|value| value.as_array())
            .map(|items| items.len()),
        Some(3)
    );
}
