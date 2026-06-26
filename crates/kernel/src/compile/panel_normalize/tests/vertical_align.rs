use super::super::*;
use super::helpers::*;

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
            blocks: vec![UiNodeDecl::Block(BlockDecl {
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
        blocks: vec![UiNodeDecl::Block(BlockDecl {
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
        blocks: vec![UiNodeDecl::Block(BlockDecl {
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
