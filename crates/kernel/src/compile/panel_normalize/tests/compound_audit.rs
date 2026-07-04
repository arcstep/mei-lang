use super::super::*;
use super::helpers::*;

#[test]
fn normalize_injects_metrics_auto_layout_for_compound_wide_card() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto_compound".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel("a"),
            metric_card_panel("b"),
            UiNodeDecl::Panel(PanelDecl {
                slot: None,
                kind: "panel".to_string(),
                id: "compound".to_string(),
                title: None,
                head: None::<Box<UiNodeDecl>>,
                area: Some("auto".to_string()),
                layout: None,
                blocks: vec![
                    metric_card_panel_with_height("top", Some("68px")),
                    metric_card_panel_with_height("b0", Some("54px")),
                    metric_card_panel_with_height("b1", Some("54px")),
                    metric_card_panel_with_height("b2", Some("54px")),
                ],
                props: json!({
                    "__mei_layout_policy": "metric_compound_2_1",
                    "height": "128px",
                }),
                head_props: json!({}),
                body_props: json!({}),
                base: None,
                import_scope: None,
            }),
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
    let layout = panel.layout.as_ref().expect("metrics auto compound layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![vec![
            "m0".to_string(),
            "m1".to_string(),
            "m2".to_string(),
            "m2".to_string(),
        ]])
    );
}

#[test]
fn normalize_emits_metric_inline_baseline_risk_when_row_card_is_not_bottom_aligned() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string()]),
            rows: Some(vec!["auto".to_string()]),
            areas: Some(vec![vec!["m0".to_string()]]),
            gap: Some("8px".to_string()),
            padding: Some("12px".to_string()),
            align: Some("stretch".to_string()),
            justify: Some("center".to_string()),
        }),
        blocks: vec![UiNodeDecl::Panel(PanelDecl {
            slot: None,
            kind: "panel".to_string(),
            id: "m0".to_string(),
            title: None,
            head: None::<Box<UiNodeDecl>>,
            area: Some("m0".to_string()),
            layout: Some(LayoutDecl {
                layout_type: "grid".to_string(),
                direction: None,
                columns: Some(vec![
                    "auto".to_string(),
                    "auto".to_string(),
                    "auto".to_string(),
                ]),
                rows: Some(vec!["1fr".to_string()]),
                areas: Some(vec![vec![
                    "label".to_string(),
                    "value".to_string(),
                    "unit".to_string(),
                ]]),
                gap: Some("4px".to_string()),
                padding: Some("0 4px".to_string()),
                align: Some("center".to_string()),
                justify: Some("center".to_string()),
            }),
            blocks: vec![],
            props: json!({
                "__mei_metric_card": true,
                "__mei_metric_template": "row",
                "__mei_metric_inline_align": "compact",
                "height": "88px",
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        })],
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == "layout_eval_metric_inline_baseline_risk"));
}

#[test]
fn normalize_injects_metric_compound_2_1_layout_when_policy_matches() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "compound".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel_with_height("top", Some("68px")),
            metric_card_panel_with_height("b0", Some("54px")),
            metric_card_panel_with_height("b1", Some("54px")),
            metric_card_panel_with_height("b2", Some("54px")),
        ],
        props: json!({
            "__mei_layout_policy": "metric_compound_2_1",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    let layout = panel.layout.as_ref().expect("compound layout");
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![
            vec!["top".to_string(), "top".to_string(), "top".to_string()],
            vec!["b0".to_string(), "b1".to_string(), "b2".to_string()]
        ])
    );
    assert_eq!(
        layout.rows.as_ref(),
        Some(&vec!["112fr".to_string(), "144fr".to_string()])
    );
    assert_eq!(layout.gap.as_deref(), Some("2px"));
}

#[test]
fn normalize_injects_metric_compound_2_1_with_variable_bottom_count() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "compound_two_bottom".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![
            metric_card_panel_with_height("top", Some("68px")),
            metric_card_panel_with_height("b0", Some("54px")),
            metric_card_panel_with_height("b1", Some("54px")),
        ],
        props: json!({
            "__mei_layout_policy": "metric_compound_2_1",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(
        !diagnostics
            .iter()
            .any(|diag| diag.code == "layout_policy_metric_compound_2_1_conflict"),
        "3-card compound should inject layout: {diagnostics:?}"
    );
    let layout = panels[0].layout.as_ref().expect("compound layout");
    assert_eq!(
        layout.columns.as_ref(),
        Some(&vec!["1fr".to_string(), "1fr".to_string()])
    );
    assert_eq!(
        layout.areas.as_ref(),
        Some(&vec![
            vec!["top".to_string(), "top".to_string()],
            vec!["b0".to_string(), "b1".to_string()],
        ])
    );
}

#[test]
fn normalize_metric_compound_respects_top_band_ratio_props() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "compound_ratio".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![metric_card_panel("top"), metric_card_panel("b0")],
        props: json!({
            "__mei_layout_policy": "metric_compound_2_1",
            "height": "100px",
            "__mei_compound_top_band_ratio": "0.5",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let layout = panels[0].layout.as_ref().expect("compound layout");
    let rows = layout.rows.as_ref().expect("rows");
    assert_eq!(rows.len(), 2);
    assert!(
        rows[0].ends_with("fr"),
        "compound rows should use fractional tracks, got {:?}",
        rows
    );
    let top_w = rows[0].trim_end_matches("fr").parse::<u32>().unwrap();
    let bottom_w = rows[1].trim_end_matches("fr").parse::<u32>().unwrap();
    assert_eq!(
        top_w, bottom_w,
        "0.5 top band should yield equal fr weights, got {top_w} / {bottom_w}"
    );
}

#[test]
fn normalize_warns_when_metric_compound_2_1_policy_shape_is_invalid() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "compound_invalid".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![metric_card_panel("only_top")],
        props: json!({
            "__mei_layout_policy": "metric_compound_2_1",
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
        .any(|diag| diag.code == "layout_policy_metric_compound_2_1_conflict"));
}

#[test]
fn normalize_emits_layout_eval_for_unknown_block_area() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "audit".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string()]),
            rows: Some(vec!["20px".to_string()]),
            areas: Some(vec![vec!["body".to_string()]]),
            gap: Some("0".to_string()),
            padding: Some("0".to_string()),
            align: None,
            justify: None,
        }),
        blocks: vec![UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: None,
            title: None,
            area: Some("ghost".to_string()),
            props: json!({"content": "x"}),
            base: None,
            layout: None,
            blocks: vec![],
            component: None,
            placement: None,
            interactions: vec![],
            lifecycle: None,
            constraints: None,
            data: None,
        })],
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == "layout_eval_unknown_block_area"));
}

#[test]
fn normalize_emits_body_clip_risk_for_head_body_metrics_conflict() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "outer".to_string(),
        title: Some("标题".to_string()),
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Panel(PanelDecl {
            slot: None,
            kind: "panel".to_string(),
            id: "body".to_string(),
            title: None,
            head: None::<Box<UiNodeDecl>>,
            area: Some("body".to_string()),
            layout: None,
            blocks: vec![
                metric_card_panel_with_height("m0", Some("128px")),
                metric_card_panel_with_height("m1", Some("128px")),
                metric_card_panel_with_height("m2", Some("128px")),
            ],
            props: json!({
                "__mei_layout_policy": "metrics_2_1",
                "__mei_layout_padding": "24px 21px",
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        })],
        props: json!({ "height": "180px" }),
        head_props: json!({ "height": "54px" }),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(
        diagnostics
            .iter()
            .any(|diag| diag.code == "layout_policy_budget_overflow"),
        "expected layout policy budget overflow diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn normalize_audits_metric_group_off_center_rows() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "off_center".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec![
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
                "1fr".to_string(),
            ]),
            rows: Some(vec!["96px".to_string(), "96px".to_string()]),
            areas: Some(vec![
                vec![
                    "m0".to_string(),
                    "m1".to_string(),
                    ".".to_string(),
                    ".".to_string(),
                ],
                vec![
                    "m2".to_string(),
                    "m2".to_string(),
                    "m2".to_string(),
                    ".".to_string(),
                ],
            ]),
            gap: Some("8px".to_string()),
            padding: Some("12px".to_string()),
            align: Some("stretch".to_string()),
            justify: Some("center".to_string()),
        }),
        blocks: vec![
            metric_card_panel_with_height("a", Some("96px")),
            metric_card_panel_with_height("b", Some("96px")),
            metric_card_panel_with_extra_props("c", Some("96px"), json!({"layout_span": "full"})),
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
    assert!(diagnostics
        .iter()
        .any(|diag| diag.code == "layout_eval_metric_group_off_center"));
}

#[test]
fn normalize_emits_stack_desc_overlap_risk_for_short_metric_card() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: Some(LayoutDecl {
            layout_type: "grid".to_string(),
            direction: None,
            columns: Some(vec!["1fr".to_string()]),
            rows: Some(vec!["auto".to_string()]),
            areas: Some(vec![vec!["m0".to_string()]]),
            gap: Some("8px".to_string()),
            padding: Some("12px".to_string()),
            align: Some("stretch".to_string()),
            justify: Some("stretch".to_string()),
        }),
        blocks: vec![UiNodeDecl::Panel(PanelDecl {
            slot: None,
            kind: "panel".to_string(),
            id: "m0".to_string(),
            title: None,
            head: None::<Box<UiNodeDecl>>,
            area: Some("m0".to_string()),
            layout: None,
            blocks: vec![],
            props: json!({
                "__mei_metric_card": true,
                "__mei_metric_template": "stack_desc",
                "height": "88px",
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        })],
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(diagnostics
        .iter()
        .any(|diag| { diag.code == "layout_eval_metric_stack_desc_overlap_risk" }));
}

