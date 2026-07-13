use crate::compile::layout_budget::{
    materialize_fill_section_derived_heights, resolve_layout_budgets,
    validate_layout_budget_policy_with_options, LayoutBudgetValidateOptions,
};
use crate::model::{Diagnostic, LayoutDecl, Severity, UiNodeDecl, UiTreeNode};
use serde_json::{json, Value};

fn empty_panel(id: &str) -> UiNodeDecl {
    UiNodeDecl {
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None,
        area: None,
        layout: None,
        blocks: vec![],
        slot: None,
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }
}

fn panel_with_height(id: &str, height: &str) -> UiNodeDecl {
    let mut panel = empty_panel(id);
    panel.title = Some("Section".to_string());
    panel.props = json!({
        "__mei_ui_role": "section",
        "height": height,
    });
    panel
}

fn region_with_px_rows(id: &str) -> UiNodeDecl {
    let mut panel = empty_panel(id);
    panel.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(vec!["100px".to_string()]),
        areas: None,
        gap: None,
        padding: None,
        align: None,
        justify: None,
    });
    panel.props = json!({
        "__mei_ui_role": "region",
        "__mei_chrome_role": "rail",
        "viewport": {"design_height": 500},
    });
    panel
}

fn fill_content_panel(id: &str, rows: &[&str]) -> UiNodeDecl {
    let mut panel = empty_panel(id);
    panel.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(rows.iter().map(|r| r.to_string()).collect()),
        areas: None,
        gap: None,
        padding: None,
        align: None,
        justify: None,
    });
    panel.props = json!({
        "__mei_layout_fill": true,
    });
    panel
}

fn content_panel(id: &str, row_budgets: &[i64], rows: &[&str]) -> UiNodeDecl {
    let mut panel = empty_panel(id);
    panel.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(rows.iter().map(|r| r.to_string()).collect()),
        areas: None,
        gap: None,
        padding: None,
        align: None,
        justify: None,
    });
    panel.props = json!({
        "__mei_content_budget": {
            "rows": row_budgets,
            "gap": "0",
        },
    });
    panel
}

fn section_with_body(id: &str, body: UiNodeDecl, extra_props: serde_json::Value) -> UiNodeDecl {
    let mut panel = empty_panel(id);
    panel.title = Some("Section".to_string());
    panel.props = extra_props;
    panel.blocks = vec![UiTreeNode::Panel(body)];
    panel
}

fn strict_fill_options() -> LayoutBudgetValidateOptions {
    LayoutBudgetValidateOptions {
        strict_t1_fill_down: true,
        strict_t2_fill_down: true,
    }
}

fn resolve_with_strict(
    panels: &mut [UiNodeDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    validate_layout_budget_policy_with_options(
        panels,
        diagnostics,
        source_path,
        &strict_fill_options(),
    );
    materialize_fill_section_derived_heights(panels, diagnostics, source_path);
}

fn assert_has_code(diagnostics: &[crate::model::Diagnostic], code: &str) {
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == code && d.severity == Severity::Error),
        "expected {code} error, got {diagnostics:?}"
    );
}

#[test]
fn layout_policy_section_height_forbidden_emits_error() {
    let mut panels = vec![panel_with_height("sec", "200px")];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_section_height_forbidden");
}

#[test]
fn layout_policy_region_px_track_forbidden_emits_error() {
    let mut panels = vec![region_with_px_rows("rail")];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_region_px_track_forbidden");
}

#[test]
fn layout_policy_t2_page_region_allows_analytics_page_grid_minmax() {
    let mut region = empty_panel("t2_warnings");
    region.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec![
            "minmax(180px, 1fr)".to_string(),
            "minmax(0, 5fr)".to_string(),
        ]),
        rows: Some(vec!["minmax(0, 1fr)".to_string()]),
        areas: Some(vec![vec!["filter".to_string(), "main".to_string()]]),
        gap: Some("4px".to_string()),
        padding: None,
        align: None,
        justify: None,
    });
    region.props = json!({
        "__mei_ui_role": "region",
        "__mei_t2_page": true,
    });
    region.blocks = vec![UiTreeNode::Panel(empty_panel("s-filter"))];
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    resolve_with_strict(&mut panels, &mut diagnostics, "test.mei");
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "layout_policy_region_px_track_forbidden"),
        "T2 page regions use analytics_page_grid minmax by design: {diagnostics:?}"
    );
}

fn slot_without_stretch_bg(id: &str) -> UiNodeDecl {
    let mut panel = empty_panel(id);
    panel.props = json!({
        "__mei_slot_frame_bg": true,
        "background": {
            "image": "url(test.svg)",
            "size": "contain",
        },
    });
    panel
}

#[test]
fn layout_policy_slot_background_incomplete_emits_error() {
    let mut panels = vec![slot_without_stretch_bg("slot_a")];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_slot_background_incomplete");
}

#[test]
fn layout_policy_slot_background_accepts_multilayer_stretch_and_shorthand() {
    let mut multilayer = empty_panel("status_slot");
    multilayer.props = json!({
        "__mei_slot_frame_bg": true,
        "background": {
            "color": "rgba(98,190,235,0.10)",
            "image": ["url(icon.png)", "url(fill.svg)"],
            "size": ["48px 48px", "100% 100%"],
            "origin": "border-box",
            "clip": "border-box",
        },
    });
    let mut shorthand = empty_panel("corner_slot");
    shorthand.props = json!({
        "__mei_slot_frame_bg": true,
        "background": "linear-gradient(#71F1EA,#71F1EA) left top / 4px 2px no-repeat, rgba(98,190,235,0.10)",
    });
    let mut panels = vec![multilayer, shorthand];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code == "layout_policy_slot_background_incomplete"),
        "unexpected slot background errors: {diagnostics:?}"
    );
}

#[test]
fn layout_policy_placement_absolute_forbidden_emits_error() {
    let mut panel = empty_panel("biz_section");
    panel.props = json!({
        "__mei_ui_role": "section",
        "position": "absolute",
        "top": "0",
        "left": "0",
    });
    let mut panels = vec![panel];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_placement_absolute_forbidden");
}

#[test]
fn layout_policy_content_without_fill_emits_error() {
    let mut panel = empty_panel("enforcement-stats");
    panel.props = json!({});
    let mut panels = vec![panel];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_body_not_fill");
}

#[test]
fn layout_policy_content_auto_row_forbidden_emits_error() {
    let mut panels = vec![fill_content_panel("strip", &["auto"])];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_content_auto_row_forbidden");
}

#[test]
fn layout_policy_duplicate_dimension_emits_error() {
    let mut panel = empty_panel("dup_panel");
    panel.props = json!({
        "__mei_placement_dimension_conflicts": ["height"],
    });
    let mut panels = vec![panel];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_duplicate_dimension");
}

#[test]
fn layout_policy_budget_overflow_path_deleted_forbids_content_budget() {
    let body = content_panel("body", &[400], &["1fr"]);
    let section = section_with_body(
        "sec_overflow",
        body,
        json!({
            "__mei_ui_role": "section",
            "__mei_padding_profile": "dense",
            "viewport": {"design_height": 120},
        }),
    );
    let mut panels = vec![section];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_content_budget_px_forbidden");
}

#[test]
fn layout_policy_region_overflow_emits_error() {
    let sec_a = section_with_body(
        "sec_a",
        content_panel("body_a", &[300], &["1fr"]),
        json!({
            "__mei_ui_role": "section",
            "__mei_padding_profile": "dense",
        }),
    );
    let sec_b = section_with_body(
        "sec_b",
        content_panel("body_b", &[300], &["1fr"]),
        json!({
            "__mei_ui_role": "section",
            "__mei_padding_profile": "dense",
        }),
    );
    let mut region = empty_panel("right_rail");
    region.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(vec!["1fr".to_string(), "1fr".to_string()]),
        areas: None,
        gap: Some("12px".to_string()),
        padding: None,
        align: None,
        justify: None,
    });
    region.props = json!({
        "__mei_ui_role": "region",
        "viewport": {"design_height": 200},
    });
    region.blocks = vec![UiTreeNode::Panel(sec_a), UiTreeNode::Panel(sec_b)];
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_region_overflow");
}

#[test]
fn layout_policy_fill_down_compliant_tree_emits_no_errors() {
    let body = content_panel("content_strip", &[], &["1fr"]);
    let mut body = body;
    body.props = json!({
        "__mei_layout_fill": true,
        "height": "100%",
    });
    let section = section_with_body(
        "enforcement",
        body,
        json!({
            "__mei_ui_role": "section",
            "__mei_padding_profile": "dense_strip_100",
        }),
    );
    let mut region = empty_panel("left_rail");
    region.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(vec!["1fr".to_string(), "2fr".to_string()]),
        areas: None,
        gap: Some("12px".to_string()),
        padding: None,
        align: None,
        justify: None,
    });
    region.props = json!({
        "__mei_ui_role": "region",
        "viewport": {"design_height": 520},
    });
    region.blocks = vec![
        UiTreeNode::Panel(section),
        UiTreeNode::Panel(section_with_body(
            "inspection",
            content_panel("inspection_body", &[200], &["1fr"]),
            json!({
                "__mei_ui_role": "section",
                "__mei_padding_profile": "compact_ai",
            }),
        )),
    ];
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    let layout_errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.starts_with("layout_policy_") && d.severity == Severity::Error)
        .collect();
    assert!(
        layout_errors.is_empty(),
        "expected no layout_policy errors, got {layout_errors:?}"
    );
    let enforcement = find_panel_by_id_in_tree(&panels, "enforcement").expect("enforcement");
    let derived = enforcement
        .props
        .as_object()
        .and_then(|m: &serde_json::Map<String, Value>| m.get("__mei_section_derived_height_px"))
        .and_then(Value::as_f64)
        .expect("fill section derived height");
    assert!(
        (derived - 169.0).abs() < 2.0,
        "fill section should derive from fr row (~169px), got {derived}"
    );
}

#[test]
fn layout_policy_content_fill_with_budget_forbidden() {
    let mut body = content_panel("strip", &[100], &["1fr"]);
    if let Some(map) = body.props.as_object_mut() {
        map.insert("__mei_layout_fill".to_string(), json!(true));
    }
    let mut panels = vec![body];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_content_budget_px_forbidden");
}

#[test]
fn layout_policy_content_budget_px_forbidden_on_t1_emits_error() {
    let mut body = content_panel("penalty-stats", &[144], &["1fr"]);
    body.props = json!({
        "__mei_content_budget": {"rows": [144], "gap": "0"},
        "__mei_tier": "t1",
    });
    let mut panels = vec![body];
    let mut diagnostics = Vec::new();
    resolve_with_strict(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_content_budget_px_forbidden");
}

#[test]
fn layout_policy_body_not_fill_on_t1_section_emits_error() {
    let body = content_panel("inspection-stats", &[100], &["1fr"]);
    let section = section_with_body(
        "inspection",
        body,
        json!({
            "__mei_ui_role": "section",
            "__mei_tier": "t1",
            "__mei_padding_profile": "compact_ai",
        }),
    );
    let mut panels = vec![section];
    let mut diagnostics = Vec::new();
    resolve_with_strict(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_body_not_fill");
}

#[test]
fn layout_policy_t2_fill_down_requires_explicit_opt_out() {
    let mut body = empty_panel("drilldown-content");
    body.props = json!({
        "__mei_ui_role": "content",
        "__mei_tier": "t2",
    });
    let section = section_with_body(
        "drilldown",
        body,
        json!({
            "__mei_ui_role": "section",
            "__mei_tier": "t2",
        }),
    );

    let mut strict_panels = vec![section.clone()];
    let mut strict_diagnostics = Vec::new();
    resolve_with_strict(&mut strict_panels, &mut strict_diagnostics, "strict-t2.mei");
    assert_has_code(&strict_diagnostics, "layout_policy_body_not_fill");

    let mut opted_out_panels = vec![section];
    let mut opted_out_diagnostics = Vec::new();
    validate_layout_budget_policy_with_options(
        &mut opted_out_panels,
        &mut opted_out_diagnostics,
        "content-driven-t2.mei",
        &LayoutBudgetValidateOptions {
            strict_t1_fill_down: true,
            strict_t2_fill_down: false,
        },
    );
    assert!(
        !opted_out_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "layout_policy_body_not_fill"),
        "explicit T2 opt-out should disable fill-down errors: {opted_out_diagnostics:?}"
    );
}

#[test]
fn layout_policy_slot_height_px_forbidden_under_fill_emits_error() {
    let mut slot = empty_panel("metric_slot");
    slot.props = json!({
        "__mei_slot_frame_bg": true,
        "height": "104px",
        "background": {
            "size": "100% 100%",
            "origin": "border-box",
            "clip": "border-box",
        },
    });
    let mut body = empty_panel("enforcement-stats");
    body.props = json!({
        "__mei_layout_fill": true,
        "height": "100%",
    });
    body.blocks = vec![UiTreeNode::Panel(slot)];
    let mut panels = vec![body];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_slot_height_px_forbidden");
}

#[test]
fn layout_policy_inline_font_forbidden_emits_error() {
    let block = crate::model::BlockDecl {
        kind: "metric_card".to_string(),
        use_key: "row".to_string(),
        id: Some("card_a".to_string()),
        title: None,
        area: None,
        props: json!({"font_size": "13px"}),
        base: None,
        layout: None,
        blocks: vec![],
        component: None,
        placement: None,
        interactions: vec![],
        lifecycle: None,
        constraints: None,
        data: None,
    };
    let mut body = empty_panel("strip");
    body.props = json!({"__mei_layout_fill": true});
    body.blocks = vec![UiTreeNode::Block(block)];
    let section = section_with_body(
        "enforcement",
        body,
        json!({"__mei_ui_role": "section", "__mei_tier": "t1"}),
    );
    let mut region = empty_panel("left_rail");
    region.blocks = vec![UiTreeNode::Panel(section)];
    region.props = json!({"__mei_ui_role": "region", "__mei_tier": "t1"});
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    resolve_with_strict(&mut panels, &mut diagnostics, "test.mei");
    assert_has_code(&diagnostics, "layout_policy_inline_font_forbidden");
}

#[test]
fn materialize_fill_section_derived_heights_stamps_fill_section() {
    use super::validate::materialize_fill_section_derived_heights;

    let mut body = content_panel("enforcement-stats", &[], &["1fr"]);
    body.props = json!({"__mei_layout_fill": true, "height": "100%"});
    let section = section_with_body(
        "enforcement",
        body,
        json!({
            "__mei_ui_role": "section",
            "__mei_padding_profile": "dense_strip_100",
        }),
    );
    let mut region = empty_panel("left_rail");
    region.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(vec!["1fr".to_string()]),
        areas: None,
        gap: None,
        padding: None,
        align: None,
        justify: None,
    });
    region.props = json!({
        "__mei_ui_role": "region",
        "viewport": {"design_height": 520},
    });
    region.blocks = vec![UiTreeNode::Panel(section)];
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    materialize_fill_section_derived_heights(&mut panels, &mut diagnostics, "test.mei");
    let enforcement = find_panel_by_id_in_tree(&panels, "enforcement").expect("enforcement");
    let derived = enforcement
        .props
        .get("__mei_section_derived_height_px")
        .and_then(Value::as_f64);
    assert!(
        derived.is_some(),
        "expected derived height, diagnostics={diagnostics:?}"
    );
}

fn find_panel_by_id_in_tree<'a>(panels: &'a [UiNodeDecl], id: &str) -> Option<&'a UiNodeDecl> {
    for panel in panels {
        if let Some(found) = find_panel_by_id_recursive(panel, id) {
            return Some(found);
        }
    }
    None
}

fn find_panel_by_id_recursive<'a>(panel: &'a UiNodeDecl, id: &str) -> Option<&'a UiNodeDecl> {
    if panel.id == id {
        return Some(panel);
    }
    for node in &panel.blocks {
        if let UiTreeNode::Panel(child) = node {
            if let Some(found) = find_panel_by_id_recursive(child, id) {
                return Some(found);
            }
        }
    }
    None
}

#[test]
fn layout_policy_budget_compliant_tree_emits_no_errors() {
    let body = content_panel("content_strip", &[100, 80], &["1fr", "1fr"]);
    let section = section_with_body(
        "enforcement",
        body,
        json!({
            "__mei_ui_role": "section",
            "__mei_padding_profile": "dense_strip_100",
        }),
    );
    let mut region = empty_panel("left_rail");
    region.layout = Some(LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: None,
        rows: Some(vec!["1fr".to_string()]),
        areas: None,
        gap: None,
        padding: None,
        align: None,
        justify: None,
    });
    region.props = json!({
        "__mei_ui_role": "region",
        "viewport": {"design_height": 520},
    });
    region.blocks = vec![UiTreeNode::Panel(section)];
    let mut panels = vec![region];
    let mut diagnostics = Vec::new();
    resolve_layout_budgets(&mut panels, &mut diagnostics, "test.mei");
    let layout_errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.starts_with("layout_policy_") && d.severity == Severity::Error)
        .collect();
    assert!(
        layout_errors.is_empty(),
        "expected no layout_policy errors, got {layout_errors:?}"
    );
}
