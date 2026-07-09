use super::super::*;
use super::helpers::*;

#[test]
fn normalize_injects_head_block_from_title_and_default_layout() {
    let mut panels = vec![panel_with_title("标题")];
    let mut diagnostics = Vec::new();
    normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
    let panel = &panels[0];
    assert!(panel_resolved_has_head(panel));
    assert!(
        blocks_touch_slot(&panel.blocks, TITLE_ZONE),
        "expected synthetic head block"
    );
    let head = panel.blocks.first().expect("head block");
    if let UiTreeNode::Block(block) = head {
        assert_eq!(block.area.as_deref(), Some(TITLE_ZONE));
        assert_eq!(
            block.props.get("content").and_then(Value::as_str),
            Some("标题")
        );
    } else {
        panic!("expected block head");
    }
    let layout = panel.layout.as_ref().expect("layout");
    assert!(layout_has_slot(Some(layout), TITLE_ZONE));
    assert!(layout_has_slot(Some(layout), CONTENT_ZONE));
    let body = panel.blocks.get(1).expect("body block");
    if let UiTreeNode::Block(block) = body {
        assert_eq!(block.area.as_deref(), Some(CONTENT_ZONE));
    } else {
        panic!("expected body block");
    }
}

#[test]
fn normalize_uses_head_height_track_in_default_layout() {
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: Some("标题".to_string()),
        head: None::<Box<UiTreeNode>>,
        area: Some("auto".to_string()),
        layout: None,
        blocks: vec![UiTreeNode::Block(BlockDecl {
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
    let mut panels = vec![UiNodeDecl {
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
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "titled".to_string(),
        title: Some("执法要素".to_string()),
        head: None::<Box<UiTreeNode>>,
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
            UiTreeNode::Block(block) if block.area.as_deref() == Some(TITLE_ZONE) => Some(block),
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
    let mut panels = vec![UiNodeDecl {
        slot: None,
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: None,
        head: None::<Box<UiTreeNode>>,
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
    assert!(!blocks_touch_slot(&panels[0].blocks, TITLE_ZONE));
}

