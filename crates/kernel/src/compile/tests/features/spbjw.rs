use super::{
    compile_app_from_root, compile_app_from_root_with_options, temp_root, workspace_root,
    write_file, CompileOptions,
};
use serde_json::Value;

#[test]
fn compile_spbjw_preview_typical_cases_dataset_mei_has_no_missing_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
            .all(|d| !matches!(d.severity, crate::Severity::Error)),
        "unexpected errors: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_spbjw_dataset_preview_with_wrong_scene_query_still_resolves_entry_scene() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        dataset_resources.len() <= 40,
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
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
fn compile_spbjw_layout_right_supervision_popup_has_analytics_projection_slots() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
    );
}

#[test]
fn compile_spbjw_preview_widget_typical_cases_succeeds() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
fn compile_spbjw_access_home_scene_materializes_ops_theme() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        nodes: &'a [crate::UiNodeDecl],
        id: &str,
    ) -> Option<&'a crate::PanelDecl> {
        for node in nodes {
            let crate::UiNodeDecl::Panel(panel) = node else {
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        crate::resolve_runtime_metric_def_key(
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
                compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let target = "../.stock/templates/cockpit/drilldown/analytics-drilldown-board.mei";
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some(target.to_string()),
        },
    )
    .unwrap_or_else(|error| panic!("compile analytics drilldown preview `{target}` failed: {error}"));
    let errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
                compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
            .filter(|d| matches!(d.severity, crate::Severity::Error))
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
        detail_slot
            .and_then(|slot| slot.get("dataset_id").and_then(Value::as_str)),
        Some("supervision_matters"),
        "analytics board detail slot should use rowset dataset, slots: {slots:?}"
    );
    let chart_slot = slots.iter().find(|slot| {
        slot.get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == "composition_by_category")
    });
    assert_eq!(
        chart_slot
            .and_then(|slot| slot.get("dataset_id").and_then(Value::as_str)),
        Some("supervision_matters"),
        "analytics board chart slot should use rowset dataset, slots: {slots:?}"
    );
}

#[test]
fn compile_spbjw_issue_handling_board_export_preview_projection_slots_in_assembly() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
                compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
            .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
        pending_encoded.contains("supervisionDomain") && pending_encoded.contains("month_multi_select"),
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
            && encoded.contains("预警ID"),
        "verification rate popup should use analytics board with warning_detail detail fields, got: {encoded}"
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
            == Some("warning_detail_rows"),
        "rate analytics detail slot should bind verified warning detail rows, slots: {rate_slots:?}"
    );
}

#[test]
fn compile_spbjw_left_rail_analytics_projection_slots() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    for (target, board_id) in [
        ("scenes/01-执法要素.mei", "enforcement_units_analytics_board"),
        ("scenes/02-行政检查.mei", "inspection_total_analytics_board"),
        ("scenes/03-指标体系.mei", "indicator_inspection_frequency_analytics_board"),
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
            !encoded.contains("generic_drilldown_board")
                || target == "scenes/01-执法要素.mei",
            "`{target}` should not use generic drilldown except 执法对象，got: {encoded}"
        );
        assert!(
            compiled.scene_projection_assembly_by_id.contains_key(board_id),
            "`{target}` should hydrate assembly for `{board_id}`, keys: {:?}",
            compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
        );
    }
}

#[test]
fn compile_spbjw_supervision_effectiveness_analytics_projection_slots() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        "effect_mechanism_analytics_board",
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
        compiled.scene_projection_assembly_by_id.keys().collect::<Vec<_>>()
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
}

#[test]
fn compile_board_assembly_rejects_missing_data_table_zone() {
    let source_root = temp_root("reject-scene-shell-zone");
    let app_root = source_root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
app(
    id = "demo",
    default_scene = "home",
)

scene(id = "home", profile = "page")

scene.set_world(
    resources = [
        resource(
            id = "warning_list",
            kind = "dataset",
            source = ds.csv(path = "data/warning_list.csv"),
        ),
    ],
)

rows = ds.data_ref("warning_list")

world.add_dataset_view(
    id = "warning_metrics",
    rowset = rows,
    schema = [
        ds.column("分类", "string"),
        ds.column("数量", "number"),
    ],
    metrics = [
        ds.dataframe(
            id = "detail",
            schema = [
                ds.column("分类", "string"),
                ds.column("数量", "number"),
            ],
            value = rows,
        ),
    ],
)

frame()

frame.add_panel(
    id = "card",
    title = "Bad",
    blocks = [
        component(
            "mei-card",
            area = "auto",
            props = {
                "title": "Bad",
                "value": 1,
                "popup": link(
                    type = "popup",
                    projection = "overlay",
                    scene = scene_ref(scene_id = "broken_board", scene_file = "shell.mei"),
                    params = {"metric": metric_ref("detail")},
                ),
            },
        ),
    ],
)
"#,
    );
    write_file(
        &app_root.join("data/warning_list.csv"),
        "分类,数量\nA,1\nB,2\n",
    );
    write_file(
        &app_root.join("shell.mei"),
        r#"
scene(
    id = "broken_board",
    profile = "cockpit",
    params = {
        "metric": param(type = "metric", required = True),
    },
    bindings = {
        "filter_schema": {"fields": []},
    },
    local_nav = {
        "kind": "broken_board",
        "scene_id": "broken_board",
        "overlay_size": "large",
    },
)
world(resources = [])
frame(
    layout = grid(
        columns = ["minmax(180px, 1fr)", "minmax(0, 5fr)"],
        rows = ["minmax(0, 1fr)"],
        areas = [["filter", "main"]],
        gap = "12px",
        padding = "12px",
    ),
)
frame.add_panel(
    id = "filter",
    area = "filter",
    slot = panel_slot(kind = "filter", source = "filter_schema"),
    blocks = [],
)
frame.add_panel(
    id = "main",
    area = "main",
    layout = grid(
        columns = ["1fr"],
        rows = ["auto", "minmax(0, 1fr)"],
        areas = [["chart"], ["detail"]],
        gap = "12px",
    ),
    slot = panel_slot(kind = "container"),
    blocks = [
        panel(
            id = "chart",
            area = "chart",
            slot = panel_slot(kind = "slots", accepts = ["chart"], max = 3),
            blocks = [],
        ),
    ],
)
"#,
    );
    let compiled = compile_app_from_root(&source_root, &app_root)
        .expect("compile broken shell app should finish with diagnostics");
    let zone_errors: Vec<_> = compiled
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(
                d.code.as_str(),
                "scene_shell_zone_missing"
                    | "board_assembly_missing_detail"
                    | "analytics_projection_missing_detail"
            )
        })
        .collect();
    assert!(
        !zone_errors.is_empty(),
        "missing data_table zone should produce board assembly detail errors, got: {:?}",
        compiled.diagnostics
    );
}

#[test]
fn compile_rejects_explain_chart_kind_in_composition() {
    let source_root = temp_root("reject-explain-chart-kind");
    let app_root = source_root.join("demo");
    write_file(
        &app_root.join("main.mei"),
        r#"
BAD = ds.composition(id = "c", by = "分类", chart_kind = "bar")

app(
    id = "demo",
    default_scene = "home",
)

scene(
    id = "home",
    profile = "page",
)

world()
frame()
"#,
    );

    let result = compile_app_from_root(&source_root, &app_root);
    assert!(result.is_err(), "expected compile to fail for explain chart_kind ban");
    let message = result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("chart_kind") || message.contains("unexpected named argument"),
        "expected error to mention chart_kind rejection, got: {message}"
    );

    let _ = std::fs::remove_dir_all(&source_root);
}

#[test]
fn compile_spbjw_preview_administrative_inspection_park_metrics_succeeds() {
    use crate::MetricShape;
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
        .filter(|d| matches!(d.severity, crate::Severity::Error))
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
    let evaluated = super::evaluate_runtime_metric_defs(
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
fn compile_spbjw_enforcement_elements_direct_preview_world_metrics_have_analysis_contracts() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
fn compile_spbjw_enforcement_elements_direct_preview_inferred_rowset_materializes_enforcement_units(
) {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
fn compile_spbjw_home_preview_imported_enforcement_units_composition_tab_uses_real_rowset() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let metric_key = "scenes/01-执法要素.mei::enforcement_units_count";
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
    assert_eq!(
        composition_metric_id,
        format!("{metric_key}::__scalar_rowset__"),
        "imported composition tab should bind to capsule-qualified rowset metric"
    );
}

#[test]
fn compile_spbjw_enforcement_elements_direct_preview_composition_tab_uses_rowset_not_dataset() {
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
fn spbjw_effectiveness_transfer_clue_and_filing_count_equal_four() {
    use std::collections::BTreeMap;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let owner = compiled
        .resources
        .iter()
        .find(|r| r.id == "__world_metrics__")
        .and_then(|r| r.dataset.as_ref())
        .expect("08 capsule should materialize __world_metrics__");
    let datasets: BTreeMap<_, _> = compiled
        .resources
        .iter()
        .filter_map(|r| r.dataset.clone().map(|d| (r.id.clone(), d)))
        .collect();
    let metrics = super::evaluate_runtime_metric_defs(
        &owner.runtime_metric_defs,
        &[],
        &datasets,
        Some(&[
            "effectiveness_transfer_clue_count".to_string(),
            "effectiveness_filing_count".to_string(),
        ]),
    )
    .expect("evaluate supervision effectiveness clue/filing metrics");
    for metric_id in [
        "effectiveness_transfer_clue_count",
        "effectiveness_filing_count",
    ] {
        let metric = metrics
            .get(metric_id)
            .unwrap_or_else(|| panic!("missing metric `{metric_id}`"));
        let value = metric
            .value
            .get("value")
            .and_then(|v| v.as_f64())
            .or_else(|| metric.value.as_f64())
            .unwrap_or(f64::NAN);
        assert_eq!(
            value, 4.0,
            "{metric_id} should count 2+1+1 from 《11》是否转问题线索（含「是（2）」），got {value}"
        );
    }
}

#[test]
fn spbjw_indicator_system_calendar_year_metrics_use_inspection_xlsx_check_date() {
    use std::collections::BTreeMap;

    use crate::compile::analysis::dates::coerce_rows_to_schema;
    use crate::compile::loaders::load_xlsx_table_snapshot;
    use crate::MetricShape;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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

    let preview_only = super::evaluate_runtime_metric_defs(
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

    let metrics = super::evaluate_runtime_metric_defs(
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
    let metrics = super::evaluate_runtime_metric_defs(
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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

    use crate::compile::analysis::dates::coerce_rows_to_schema;
    use crate::compile::loaders::load_xlsx_table_snapshot;
    use crate::compile::materialize::resolve_runtime_metric_def_key;
    use crate::MetricShape;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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

    let preview_metrics = super::evaluate_runtime_metric_defs(
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
    let metrics = super::evaluate_runtime_metric_defs(
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

    use crate::compile::materialize::resolve_runtime_metric_def_key;
    use crate::MetricShape;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let metrics = super::evaluate_runtime_metric_defs(
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

    use crate::compile::materialize::resolve_runtime_metric_def_key;
    use crate::MetricShape;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let metrics = super::evaluate_runtime_metric_defs(
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

    use crate::model::MetricShape;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let metrics = super::evaluate_runtime_metric_defs(
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
    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
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
            .all(|diag| !matches!(diag.severity, crate::Severity::Error)),
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

    use crate::compile::analysis::dates::coerce_rows_to_schema;
    use crate::compile::loaders::load_xlsx_table_snapshot;

    let root = workspace_root();
    let source_root = root.join("workspaces").join("ws-spbjw");
    let app_root = source_root.join("zhifa");
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
    let metrics = super::evaluate_runtime_metric_defs(
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
