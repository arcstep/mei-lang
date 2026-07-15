use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn compile_spbjw_preview_widget_supervision_warning_succeeds() {
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-右栏.mei".to_string()),
            ..Default::default()
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
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/layout-右栏.mei".to_string()),
            ..Default::default()
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
    let Some(source_root) = source_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let Some(app_root) = zhifa_app_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/09-监督典型案例.mei".to_string()),
            ..Default::default()
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
            mei_lang_kernel::UiTreeNode::Block(decl) if decl.use_key == "cockpit.data-table" => {
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
