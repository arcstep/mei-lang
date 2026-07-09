use super::super::*;
use super::helpers::*;

#[test]
fn normalize_injects_metrics_strip_layout_for_metric_children() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel("a"),
            metric_card_panel("b"),
            metric_card_panel("c"),
        ],
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    let layout = panel.layout.as_ref().expect("metrics strip layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![vec![
            "m0".to_string(),
            "m1".to_string(),
            "m2".to_string()
        ]])
    );
    assert_eq!(
        layout.columns.as_ref(),
        Some(&vec![
            "1fr".to_string(),
            "1fr".to_string(),
            "1fr".to_string()
        ])
    );
    assert_eq!(layout.gap.as_deref(), Some("8px"));
    assert_eq!(layout.padding.as_deref(), Some("12px"));
    assert_eq!(
        panel
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_strip")
    );
    for (idx, node) in panel.blocks.iter().enumerate() {
        assert_eq!(
            node_area(node),
            Some(match idx {
                0 => "m0",
                1 => "m1",
                _ => "m2",
            })
        );
    }
}

#[test]
fn normalize_injects_metrics_2_1_layout_when_policy_matches() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_2_1".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel("a"),
            metric_card_panel("b"),
            metric_card_panel("c"),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_2_1",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    let layout = panel.layout.as_ref().expect("metrics 2+1 layout");
    assert_eq!(
        layout.columns.as_ref(),
        Some(&vec![
            "114px".to_string(),
            "114px".to_string(),
            "234px".to_string()
        ])
    );
    assert_eq!(layout.gap.as_deref(), Some("8px"));
    assert_eq!(layout.padding.as_deref(), Some("12px 14px"));
    assert_eq!(
        panel
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_2_1")
    );
}

#[test]
fn normalize_injects_metrics_2x2_layout_when_policy_matches() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_2x2".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel_with_height("a", Some("102px")),
            metric_card_panel_with_height("b", Some("102px")),
            metric_card_panel_with_height("c", Some("102px")),
            metric_card_panel_with_height("d", Some("102px")),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_2x2",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    let layout = panel.layout.as_ref().expect("metrics 2x2 layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![
            vec!["m0".to_string(), "m1".to_string()],
            vec!["m2".to_string(), "m3".to_string()],
        ])
    );
    assert_eq!(
        layout.rows.as_ref(),
        Some(&vec!["102px".to_string(), "102px".to_string()])
    );
    assert_eq!(layout.gap.as_deref(), Some("8px"));
    assert_eq!(layout.padding.as_deref(), Some("12px"));
    assert_eq!(
        panel
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_2x2")
    );
}

#[test]
fn normalize_injects_metrics_auto_layout_for_six_cards() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel_with_height("a", Some("98px")),
            metric_card_panel_with_height("b", Some("98px")),
            metric_card_panel_with_height("c", Some("98px")),
            metric_card_panel_with_height("d", Some("104px")),
            metric_card_panel_with_height("e", Some("104px")),
            metric_card_panel_with_height("f", Some("104px")),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_auto",
            "layout_columns_prefer": 4,
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    let layout = panel.layout.as_ref().expect("metrics auto layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![
            vec!["m0".to_string(), "m1".to_string(), "m2".to_string()],
            vec!["m3".to_string(), "m4".to_string(), "m5".to_string()],
        ])
    );
    assert_eq!(
        layout.rows.as_ref(),
        Some(&vec!["98px".to_string(), "104px".to_string()])
    );
    assert_eq!(layout.gap.as_deref(), Some("8px"));
    assert_eq!(layout.padding.as_deref(), Some("12px"));
    assert_eq!(
        panel
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_auto")
    );
}

#[test]
fn normalize_warns_when_metrics_2_1_policy_shape_is_invalid() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "invalid".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![metric_card_panel("a"), metric_card_panel("b")],
        props: json!({
            "__mei_layout_policy": "metrics_2_1",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == "layout_policy_metrics_2_1_conflict"));
    assert!(
        panels[0].layout.is_some(),
        "fallback layout should still be injected"
    );
}

#[test]
fn normalize_warns_when_metrics_2x2_policy_shape_is_invalid() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "invalid_2x2".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel("a"),
            metric_card_panel("b"),
            metric_card_panel("c"),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_2x2",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == "layout_policy_metrics_2x2_conflict"));
}

#[test]
fn normalize_warns_when_metrics_auto_policy_shape_is_invalid() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "invalid_auto".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![metric_card_panel("a")],
        props: json!({
            "__mei_layout_policy": "metrics_auto",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == "layout_policy_metrics_auto_conflict"));
}

#[test]
fn normalize_injects_metrics_auto_layout_for_full_span_footer() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto_full".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel_with_height("a", Some("88px")),
            metric_card_panel_with_height("b", Some("88px")),
            metric_card_panel_with_height("c", Some("88px")),
            metric_card_panel_with_height("d", Some("88px")),
            metric_card_panel_with_extra_props(
                "wide",
                Some("96px"),
                json!({"layout_span": "full"}),
            ),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_auto",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    let layout = panel.layout.as_ref().expect("metrics auto full-row layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![
            vec![
                "m0".to_string(),
                "m1".to_string(),
                "m2".to_string(),
                "m3".to_string(),
            ],
            vec![
                "m4".to_string(),
                "m4".to_string(),
                "m4".to_string(),
                "m4".to_string(),
            ],
        ])
    );
    assert_eq!(
        layout.rows.as_ref(),
        Some(&vec!["88px".to_string(), "96px".to_string()])
    );
    assert_eq!(layout.gap.as_deref(), Some("8px"));
    assert_eq!(layout.padding.as_deref(), Some("12px"));
    assert_eq!(
        panel
            .props
            .get("__mei_layout_policy")
            .and_then(Value::as_str),
        Some("metrics_auto")
    );
}

#[test]
fn normalize_uses_fixed_metric_widths_and_centers_singleton_tail() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto_fixed".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel_with_extra_props("a", Some("88px"), json!({"width": "114px"})),
            metric_card_panel_with_extra_props("b", Some("88px"), json!({"width": "114px"})),
            metric_card_panel_with_extra_props("c", Some("88px"), json!({"width": "114px"})),
            metric_card_panel_with_extra_props("d", Some("88px"), json!({"width": "114px"})),
            metric_card_panel_with_extra_props("e", Some("88px"), json!({"width": "114px"})),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_auto",
            "width": "520px",
            "layout_columns_prefer": 4,
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let layout = panels[0]
        .layout
        .as_ref()
        .expect("metrics auto fixed layout");
    assert_eq!(
        layout.columns.as_ref(),
        Some(&vec![
            "114px".to_string(),
            "114px".to_string(),
            "114px".to_string(),
            "114px".to_string(),
        ])
    );
    assert_eq!(layout.gap.as_deref(), Some("8px 9px"));
    assert_eq!(layout.padding.as_deref(), Some("12px 18px 12px 18px"));
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![
            vec![
                "m0".to_string(),
                "m1".to_string(),
                "m2".to_string(),
                "m3".to_string(),
            ],
            vec![
                "m4".to_string(),
                "m4".to_string(),
                "m4".to_string(),
                "m4".to_string(),
            ],
        ])
    );
}

#[test]
fn normalize_clamps_metrics_strip_spacing_into_cockpit_budget() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "strip".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel("a"),
            metric_card_panel("b"),
            metric_card_panel("c"),
            metric_card_panel("d"),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_strip",
            "__mei_layout_gap": "20px",
            "__mei_layout_padding": "40px 40px",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let layout = panels[0].layout.as_ref().expect("strip layout");
    assert_eq!(layout.gap.as_deref(), Some("12px"));
    assert_eq!(layout.padding.as_deref(), Some("24px 24px"));
}

#[test]
fn normalize_clamps_metrics_auto_spacing_into_cockpit_budget() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "auto".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel("a"),
            metric_card_panel("b"),
            metric_card_panel("c"),
            metric_card_panel("d"),
        ],
        props: json!({
            "__mei_layout_policy": "metrics_auto",
            "__mei_layout_gap": "20px",
            "__mei_layout_padding": "4px 40px",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let layout = panels[0].layout.as_ref().expect("auto layout");
    assert_eq!(layout.gap.as_deref(), Some("12px"));
    assert_eq!(layout.padding.as_deref(), Some("12px 24px"));
}

