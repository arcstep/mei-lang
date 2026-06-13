use super::*;
use crate::model::{BlockDecl, LayoutDecl, PanelDecl, UiNodeDecl};
use serde_json::json;

use super::nodes::node_area;

fn panel_with_title(title: &str) -> PanelDecl {
    PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: Some(title.to_string()),
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: None,
            title: None,
            area: Some("auto".to_string()),
            props: json!({ "content": "body" }),
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
    }
}

fn metric_card_panel(id: &str) -> UiNodeDecl {
    metric_card_panel_with_height(id, None)
}

fn metric_card_panel_with_height(id: &str, height: Option<&str>) -> UiNodeDecl {
    UiNodeDecl::Panel(PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![],
        props: json!({
            "__mei_metric_card": true,
            "chrome": "bare",
            "height": height,
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    })
}

fn metric_card_panel_with_extra_props(
    id: &str,
    height: Option<&str>,
    extra_props: serde_json::Value,
) -> UiNodeDecl {
    let mut props = json!({
        "__mei_metric_card": true,
        "chrome": "bare",
        "height": height,
    });
    if let (Some(base), Some(extra)) = (props.as_object_mut(), extra_props.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    UiNodeDecl::Panel(PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: id.to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![],
        props,
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    })
}

#[test]
fn normalize_injects_head_block_from_title_and_default_layout() {
    let mut panels = vec![panel_with_title("标题")];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    assert!(panel_resolved_has_head(panel));
    assert!(
        blocks_touch_slot(&panel.blocks, SLOT_HEAD),
        "expected synthetic head block"
    );
    let head = panel.blocks.first().expect("head block");
    if let UiNodeDecl::Block(block) = head {
        assert_eq!(block.area.as_deref(), Some(SLOT_HEAD));
        assert_eq!(
            block.props.get("content").and_then(Value::as_str),
            Some("标题")
        );
    } else {
        panic!("expected block head");
    }
    let layout = panel.layout.as_ref().expect("layout");
    assert!(layout_has_slot(Some(layout), SLOT_HEAD));
    assert!(layout_has_slot(Some(layout), SLOT_BODY));
    let body = panel.blocks.get(1).expect("body block");
    if let UiNodeDecl::Block(block) = body {
        assert_eq!(block.area.as_deref(), Some(SLOT_BODY));
    } else {
        panic!("expected body block");
    }
}

#[test]
fn normalize_uses_head_height_track_in_default_layout() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: Some("标题".to_string()),
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: None,
            title: None,
            area: Some("auto".to_string()),
            props: json!({ "content": "body" }),
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
        props: json!({ "height": "230px" }),
        head_props: json!({ "height": "54px" }),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let rows = panels[0]
        .layout
        .as_ref()
        .and_then(|layout| layout.rows.as_ref())
        .expect("rows");
    assert_eq!(rows[0], "54px");
}

#[test]
fn normalize_hoists_props_heading_to_head_props() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: None,
        head: None,
        area: None,
        layout: None,
        blocks: vec![],
        props: json!({"heading": {"variant": "screen", "height": "40px"}}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    assert!(panel
        .props
        .as_object()
        .and_then(|m| m.get("heading"))
        .is_none());
    assert_eq!(
        panel.head_props.get("variant").and_then(Value::as_str),
        Some("screen")
    );
    assert_eq!(
        panel.head_props.get("height").and_then(Value::as_str),
        Some("40px")
    );
}

#[test]
fn normalize_title_head_block_inherits_head_props_typography() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "titled".to_string(),
        title: Some("执法要素".to_string()),
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![],
        props: json!({}),
        head_props: json!({
            "font_size": "30px",
            "font_family": "Microsoft YaHei",
            "letter_spacing": "20px",
            "color": "rgba(255,255,255,0.80)",
        }),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let head_block = panels[0]
        .blocks
        .iter()
        .find_map(|node| match node {
            UiNodeDecl::Block(block) if block.area.as_deref() == Some(SLOT_HEAD) => Some(block),
            _ => None,
        })
        .expect("title head block");
    assert_eq!(
        head_block.props.get("content").and_then(|v| v.as_str()),
        Some("执法要素")
    );
    assert_eq!(
        head_block.props.get("font_size").and_then(|v| v.as_str()),
        Some("30px")
    );
    assert_eq!(
        head_block
            .props
            .get("letter_spacing")
            .and_then(|v| v.as_str()),
        Some("20px")
    );
}

#[test]
fn normalize_no_head_without_title() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![],
        props: json!({}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    }];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    assert!(!panel_resolved_has_head(&panels[0]));
    assert!(!blocks_touch_slot(&panels[0].blocks, SLOT_HEAD));
}

#[test]
fn normalize_injects_metrics_strip_layout_for_metric_children() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_2_1".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_2x2".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "invalid".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "invalid_2x2".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "invalid_auto".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto_full".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "metrics_auto_fixed".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
            .any(|diag| diag.code == "layout_eval_body_clip_risk"),
        "expected body clip risk diagnostic, got: {:?}",
        diagnostics
    );
}

#[test]
fn normalize_clamps_metrics_strip_spacing_into_cockpit_budget() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "strip".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "auto".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
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

#[test]
fn normalize_metric_card_stack_applies_fractional_vertical_bands() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "wrap".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Panel(PanelDecl {
        slot: None,
            kind: "panel".to_string(),
            id: "m0".to_string(),
            title: None,
            head: None::<Box<UiNodeDecl>>,
            area: Some("auto".to_string()),
            layout: Some(LayoutDecl {
                layout_type: "grid".to_string(),
                direction: None,
                columns: Some(vec!["auto".to_string(), "auto".to_string()]),
                rows: Some(vec!["auto".to_string(), "auto".to_string()]),
                areas: Some(vec![
                    vec!["label".to_string(), "label".to_string()],
                    vec!["value".to_string(), "unit".to_string()],
                ]),
                gap: Some("4px".to_string()),
                padding: None,
                align: Some("end".to_string()),
                justify: Some("center".to_string()),
            }),
            blocks: vec![],
            props: json!({
                "__mei_metric_card": true,
                "__mei_metric_template": "stack",
                "height": "128px",
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
    let card = match &panels[0].blocks[0] {
        UiNodeDecl::Panel(panel) => panel,
        other => panic!("expected metric card panel, got {other:?}"),
    };
    let layout = card.layout.as_ref().expect("metric card layout");
    let rows = layout.rows.as_ref().expect("metric card rows");
    assert!(
        rows.iter().any(|track| track.contains("fr")),
        "expected fractional row tracks, got {rows:?}"
    );
    assert_eq!(layout.align.as_deref(), Some("stretch"));
    assert!(
        !diagnostics
            .iter()
            .any(|diag| diag.code == "layout_eval_metric_vertical_align_risk"),
        "normalize should fix align=end before audit: {diagnostics:?}"
    );
}

#[test]
fn normalize_applies_metric_slot_vertical_align_from_shell_props() {
    let mut panels = vec![PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "wrap".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Panel(PanelDecl {
        slot: None,
            kind: "panel".to_string(),
            id: "m0".to_string(),
            title: None,
            head: None::<Box<UiNodeDecl>>,
            area: Some("auto".to_string()),
            layout: None,
            blocks: vec![UiNodeDecl::Block(crate::BlockDecl {
                kind: "block".to_string(),
                use_key: "mei.text".to_string(),
                id: Some("value_slot".to_string()),
                title: None,
                area: Some("value".to_string()),
                props: json!({"content": "--", "metric_role": "value"}),
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
            props: json!({
                "__mei_metric_card": true,
                "__mei_metric_template": "stack",
                "__mei_metric_value_v_align": "center",
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
    let card = match &panels[0].blocks[0] {
        UiNodeDecl::Panel(panel) => panel,
        other => panic!("expected metric card panel, got {other:?}"),
    };
    let block = match &card.blocks[0] {
        UiNodeDecl::Block(block) => block,
        other => panic!("expected mei.text block, got {other:?}"),
    };
    assert_eq!(
        block.props.get("metric_v_align").and_then(Value::as_str),
        Some("center")
    );
}

#[test]
fn seed_metric_block_vertical_align_prefers_shell_over_base_template() {
    let base = PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "card_plain".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Block(crate::BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: Some("label".to_string()),
            title: None,
            area: Some("label".to_string()),
            props: json!({
                "content": "·",
                "metric_role": "label",
                "metric_v_align": "end",
            }),
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
        props: json!({"__mei_metric_card": true}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    let mut merged = PanelDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "live".to_string(),
        title: None,
        head: None::<Box<UiNodeDecl>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Block(crate::BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: Some("label".to_string()),
            title: None,
            area: Some("label".to_string()),
            props: json!({"content": "执法对象", "metric_role": "label"}),
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
        props: json!({
            "__mei_metric_card": true,
            "__mei_metric_label_v_align": "center",
        }),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
    };
    seed_metric_block_vertical_align_from_base(&base, &mut merged);
    let block = match &merged.blocks[0] {
        UiNodeDecl::Block(block) => block,
        other => panic!("expected block, got {other:?}"),
    };
    assert_eq!(
        block.props.get("metric_v_align").and_then(Value::as_str),
        Some("center"),
        "shell label_vertical_align must win over card_plain label end default"
    );
}
