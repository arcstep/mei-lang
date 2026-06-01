use super::{compile_app_from_root_with_options, workspace_root, CompileOptions};
use serde_json::Value;

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
            preview_target: Some("scenes/6_典型案例/监督典型案例.mei".to_string()),
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
            r.scene_id == "typical_cases" && r.target_file == "scenes/6_典型案例/监督典型案例.mei"
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
        "scenes/6_典型案例/监督典型案例.mei"
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
            scene: Some("administrative_inspection".to_string()),
            preview_target: None,
        },
    )
    .expect("compile spbjw with administrative_inspection scene (discovered route)");
    assert_eq!(
        compiled.active_target_file.as_str(),
        "scenes/2_行政检查/行政检查.mei"
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
        dataset_resources.len() <= 32,
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layouts/中栏.mei".to_string()),
        },
    )
    .expect("compile spbjw layout center preview");
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "layout center preview errors: {:?}",
        errors
    );
    assert_eq!(compiled.active_target_file, "scenes/layouts/中栏.mei");
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督预警/监督预警.mei".to_string()),
        },
    )
    .expect("compile spbjw supervision warning preview");
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
        "scenes/4_监督预警/监督预警.mei"
    );
    assert!(
        compiled.resources.iter().any(|r| r.id == "warning_models"),
        "expected warning_models dataset in resources"
    );
}

#[test]
fn spbjw_supervision_models_count_is_eighteen() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督预警/监督预警.mei".to_string()),
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督预警/监督预警.mei".to_string()),
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
fn spbjw_warnings_count_sums_warning_entry_column() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/4_监督预警/监督预警.mei".to_string()),
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
            preview_target: Some("scenes/6_典型案例/监督典型案例.mei".to_string()),
        },
    )
    .expect("compile spbjw typical cases preview");
    let elapsed = started.elapsed();
    assert_eq!(
        compiled.active_target_file,
        "scenes/6_典型案例/监督典型案例.mei"
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
        !dataset.columns.is_empty() && !dataset.rows.is_empty(),
        "imported dataset should at least materialize basic tabular payload"
    );
    assert!(
        dataset.schema.len() >= 1,
        "expected imported dataset schema to be materialized"
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
    let issue_metrics_owner = "__world_metrics__::scenes/5_问题办理/问题办理.mei::metrics";
    let issue_metrics = compiled
        .resources
        .iter()
        .find(|r| r.id == issue_metrics_owner)
        .and_then(|r| r.dataset.as_ref())
        .expect("home should import 问题办理 world metrics resource");
    assert!(
        crate::resolve_runtime_metric_def_key(
            issue_metrics_owner,
            "warnings_pending_count::__scalar_rowset__",
            &issue_metrics.runtime_metric_defs,
        )
        .is_some(),
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
fn compile_spbjw_admin_inspection_switches_to_popup_scene_contracts() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/2_行政检查/行政检查.mei".to_string()),
        },
    )
    .expect("compile spbjw 行政检查 preview");
    let contract = compiled
        .scene_contract
        .as_ref()
        .expect("行政检查 preview scene contract");
    let encoded = serde_json::to_string(contract).expect("encode scene contract");

    assert!(
        encoded.contains("inspection-total-popup.mei")
            && encoded.contains("inspection_total_popup")
            && encoded.contains("inspection-today-popup.mei")
            && encoded.contains("inspection_today_popup")
            && encoded.contains("inspection-week-popup.mei")
            && encoded.contains("inspection_week_popup")
            && encoded.contains("inspection-complaint-popup.mei")
            && encoded.contains("inspection_complaint_popup")
            && encoded.contains("inspection-no-violation-popup.mei")
            && encoded.contains("inspection_no_violation_popup")
            && encoded.contains("inspection-ai-main-popup.mei")
            && encoded.contains("inspection_ai_main_popup")
            && encoded.contains("inspection-ai-top-popup.mei")
            && encoded.contains("inspection_ai_top_popup")
            && encoded.contains("inspection-ai-bottom-popup.mei")
            && encoded.contains("inspection_ai_bottom_popup"),
        "行政检查入口卡应全部指向独立 popup scene，got: {encoded}"
    );
    assert!(
        !encoded
            .contains("scene_file\":\"../templates/cockpit/drilldown/metric-explain-board.mei\""),
        "第二批后行政检查不应再直连模板壳 scene_file，got: {encoded}"
    );
}

#[test]
fn compile_spbjw_admin_inspection_popup_scenes_are_previewable() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let popup_targets = [
        (
            "scenes/2_行政检查/inspection-total-popup.mei",
            "composition_by_agency",
        ),
        ("scenes/2_行政检查/inspection-today-popup.mei", "detail"),
        ("scenes/2_行政检查/inspection-week-popup.mei", "detail"),
        ("scenes/2_行政检查/inspection-complaint-popup.mei", "detail"),
        (
            "scenes/2_行政检查/inspection-no-violation-popup.mei",
            "composition_by_park",
        ),
        ("scenes/2_行政检查/inspection-ai-main-popup.mei", "detail"),
        (
            "scenes/2_行政检查/inspection-ai-top-popup.mei",
            "composition_by_unit",
        ),
        ("scenes/2_行政检查/inspection-ai-bottom-popup.mei", "detail"),
    ];

    for (target, expected_entry) in popup_targets {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|error| panic!("compile popup preview `{target}` failed: {error}"));
        let errors: Vec<_> = compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, crate::Severity::Error))
            .collect();
        assert!(
            errors.is_empty(),
            "popup preview `{target}` should have no error diagnostics: {:?}",
            errors
        );
        let contract = compiled
            .scene_contract
            .as_ref()
            .unwrap_or_else(|| panic!("popup preview `{target}` should yield scene contract"));
        assert_eq!(
            contract
                .scene
                .local_nav
                .get("default_entry")
                .and_then(|value| value.as_str()),
            Some(expected_entry),
            "popup preview `{target}` should keep local_nav.default_entry"
        );
        assert!(
            contract.scene.bindings.is_object(),
            "popup preview `{target}` should keep scene.bindings for assembly defaults"
        );
        assert!(
            contract
                .scene
                .examples
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "popup preview `{target}` should keep scene.examples for bare preview"
        );
    }
}

#[test]
fn compile_spbjw_home_chain_new_popup_targets_replace_template_links() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let cases = [
        (
            "scenes/1_执法要素/执法要素.mei",
            vec![
                "enforcement-key-enterprises-popup.mei",
                "enforcement-park-popup.mei",
                "enforcement-whitelist-popup.mei",
                "enforcement-units-popup.mei",
                "enforcement-officers-popup.mei",
                "enforcement-matters-popup.mei",
            ],
        ),
        (
            "scenes/4_监督预警/监督预警.mei",
            vec![
                "supervision-warning-items-popup.mei",
                "supervision-warning-models-popup.mei",
                "supervision-warning-total-popup.mei",
            ],
        ),
        (
            "scenes/5_问题办理/问题办理.mei",
            vec![
                "issue-pending-popup.mei",
                "issue-doing-popup.mei",
                "issue-done-popup.mei",
                "issue-rate-popup.mei",
            ],
        ),
        (
            "scenes/5_问题办理/监督成效.mei",
            vec![
                "effect-transfer-clue-popup.mei",
                "effect-filing-popup.mei",
                "effect-sanction-popup.mei",
                "effect-handled-popup.mei",
                "effect-recovered-popup.mei",
                "effect-mechanism-popup.mei",
            ],
        ),
    ];

    for (target, expected_popup_files) in cases {
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

        for expected in expected_popup_files {
            assert!(
                encoded.contains(expected),
                "`{target}` should reference `{expected}` after popup migration, got: {encoded}"
            );
        }
        assert!(
            !encoded.contains("scene_file\":\"../templates/cockpit/drilldown/metric-explain-board.mei\""),
            "`{target}` should not keep direct metric-explain-board scene_file links, got: {encoded}"
        );
    }
}

#[test]
fn compile_spbjw_home_chain_batch_three_popup_scenes_are_previewable() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let popup_targets = [
        (
            "scenes/1_执法要素/enforcement-key-enterprises-popup.mei",
            "composition_by_street",
        ),
        (
            "scenes/1_执法要素/enforcement-park-popup.mei",
            "composition_by_geometry",
        ),
        (
            "scenes/1_执法要素/enforcement-whitelist-popup.mei",
            "detail",
        ),
        (
            "scenes/1_执法要素/enforcement-units-popup.mei",
            "composition_by_category",
        ),
        (
            "scenes/1_执法要素/enforcement-officers-popup.mei",
            "composition_by_agency",
        ),
        (
            "scenes/1_执法要素/enforcement-matters-popup.mei",
            "composition_by_domain",
        ),
        (
            "scenes/4_监督预警/supervision-warning-items-popup.mei",
            "detail",
        ),
        (
            "scenes/4_监督预警/supervision-warning-models-popup.mei",
            "detail",
        ),
        (
            "scenes/4_监督预警/supervision-warning-total-popup.mei",
            "detail",
        ),
        ("scenes/5_问题办理/issue-pending-popup.mei", "detail"),
        ("scenes/5_问题办理/issue-doing-popup.mei", "detail"),
        ("scenes/5_问题办理/issue-done-popup.mei", "detail"),
        ("scenes/5_问题办理/issue-rate-popup.mei", "detail"),
        ("scenes/5_问题办理/effect-transfer-clue-popup.mei", "detail"),
        ("scenes/5_问题办理/effect-filing-popup.mei", "detail"),
        ("scenes/5_问题办理/effect-sanction-popup.mei", "detail"),
        ("scenes/5_问题办理/effect-handled-popup.mei", "detail"),
        ("scenes/5_问题办理/effect-recovered-popup.mei", "detail"),
        ("scenes/5_问题办理/effect-mechanism-popup.mei", "detail"),
    ];

    for (target, expected_entry) in popup_targets {
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                scene: None,
                preview_target: Some(target.to_string()),
            },
        )
        .unwrap_or_else(|error| panic!("compile popup preview `{target}` failed: {error}"));
        let errors: Vec<_> = compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, crate::Severity::Error))
            .collect();
        assert!(
            errors.is_empty(),
            "popup preview `{target}` should have no error diagnostics: {:?}",
            errors
        );
        let contract = compiled
            .scene_contract
            .as_ref()
            .unwrap_or_else(|| panic!("popup preview `{target}` should yield scene contract"));
        assert_eq!(
            contract
                .scene
                .local_nav
                .get("default_entry")
                .and_then(|value| value.as_str()),
            Some(expected_entry),
            "popup preview `{target}` should keep local_nav.default_entry"
        );
        assert!(
            contract.scene.bindings.is_object(),
            "popup preview `{target}` should keep scene.bindings for assembly defaults"
        );
        assert!(
            contract
                .scene
                .examples
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "popup preview `{target}` should keep scene.examples for bare preview"
        );
    }
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
            preview_target: Some("scenes/7_物流园区/园区统计.mei".to_string()),
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
    let preview_targets = [
        "scenes/2_行政检查/指标体系.mei",
        "scenes/5_问题办理/问题办理.mei",
        "scenes/1_执法要素/执法要素.mei",
        "scenes/2_行政检查/行政检查.mei",
        "scenes/3_行政处罚/行政处罚.mei",
        "scenes/4_监督预警/监督预警.mei",
        "scenes/5_问题办理/监督成效.mei",
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
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let preview_targets = [
        (
            "scenes/5_问题办理/监督成效.mei",
            "effectiveness_handled_person_times",
        ),
        (
            "scenes/5_问题办理/问题办理.mei",
            "effectiveness_issue_verification_rate",
        ),
        (
            "scenes/2_行政检查/指标体系.mei",
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
    .expect("compile home preview");
    let cases = [
        (
            "scenes/1_执法要素/执法要素.mei",
            "enforcement_units_count",
        ),
        (
            "scenes/5_问题办理/监督成效.mei",
            "effectiveness_handled_person_times",
        ),
        (
            "scenes/2_行政检查/指标体系.mei",
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
                panic!("home preview should include imported world metrics resource `{resource_id}`")
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
fn compile_spbjw_enforcement_elements_direct_preview_world_metrics_have_analysis_contracts() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/执法要素.mei";
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
            dataset.runtime_analysis_contracts.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_enforcement_elements_enforcement_units_resource_has_hydratable_source() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/执法要素.mei";
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
fn compile_spbjw_enforcement_elements_direct_preview_inferred_rowset_materializes_enforcement_units() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/执法要素.mei";
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
    eprintln!("rowset def: {}", serde_json::to_string_pretty(rowset_def).unwrap());
    let metric = dataset.metrics.get(rowset_key).or_else(|| {
        dataset.metrics.get("enforcement_units_count")
    });
    if let Some(m) = metric {
        eprintln!("metric value shape: {:?}", m.shape);
        eprintln!("metric value: {}", serde_json::to_string(&m.value).unwrap_or_default().chars().take(200).collect::<String>());
    }
}

#[test]
fn compile_spbjw_home_preview_imported_enforcement_units_composition_tab_uses_real_rowset() {
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
    .expect("compile home preview");
    let resource_id = "__world_metrics__::scenes/1_执法要素/执法要素.mei::metrics";
    let metric_key = "scenes/1_执法要素/执法要素.mei::enforcement_units_count";
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
    eprintln!("home imported composition metric_id: {composition_metric_id}");
    assert!(
        !composition_metric_id.contains("__scalar_rowset__"),
        "imported composition tab should bind to a real rowset metric, got `{composition_metric_id}`"
    );
}

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_composition_tab_uses_rowset_not_dataset() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/1_执法要素/执法要素.mei";
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
        .get("enforcement_units_count")
        .and_then(Value::as_object)
        .expect("enforcement_units_count contract");
    let composition_metric_id = contract
        .get("tab_metrics")
        .and_then(|value| value.get("composition"))
        .and_then(|value| value.get("metric_id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        !composition_metric_id.contains("__scalar_rowset__"),
        "composition tab should bind to a real rowset metric, got `{composition_metric_id}`"
    );
}

#[test]
fn compile_spbjw_runtime_metric_defs_expand_explain_scope_metric_nodes() {
    let root = workspace_root();
    let source_root = root.join("workspaces");
    let app_root = source_root.join("spbjw");
    let target = "scenes/5_问题办理/监督成效.mei";
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
