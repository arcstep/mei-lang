use super::style::{block_style, panel_card_layout_style};
use mei_lang_kernel::LayoutDecl;
use serde_json::json;

fn grid_layout() -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string(), "2fr".to_string()]),
        rows: None,
        areas: Some(vec![vec!["doc".to_string(), "table".to_string()]]),
        gap: Some("16px".to_string()),
        padding: Some("20px".to_string()),
        align: None,
        justify: None,
    }
}

#[test]
fn panel_card_layout_style_preserves_gap_for_content_only_grid() {
    let mut layout = grid_layout();
    layout.gap = Some("8px".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("gap:8px;"));
    assert!(!style.contains("gap:0;"));
}

#[test]
fn panel_card_layout_style_zeroes_gap_for_head_body_layout() {
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string(), "1fr".to_string()]),
        areas: Some(vec![vec!["head".to_string()], vec!["body".to_string()]]),
        gap: Some("8px".to_string()),
        padding: Some("0".to_string()),
        align: Some("stretch".to_string()),
        justify: None,
    };
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("gap:0;"));
}

#[test]
fn panel_card_layout_style_applies_heading_height_only_for_head_slot_layouts() {
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string(), "1fr".to_string()]),
        areas: Some(vec![vec!["head".to_string()], vec!["body".to_string()]]),
        gap: Some("0".to_string()),
        padding: Some("0".to_string()),
        align: Some("stretch".to_string()),
        justify: None,
    };
    let style = panel_card_layout_style(
        Some(&layout),
        &json!({
            "height": "54px"
        }),
    );
    assert!(
        style.contains("grid-template-rows:54px minmax(0, 1fr);")
            || style.contains("grid-template-rows:54px 1fr;")
    );
}

#[test]
fn panel_card_layout_style_preserves_non_head_grid_rows() {
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec![
            "1fr".to_string(),
            "1fr".to_string(),
            "1fr".to_string(),
        ]),
        rows: Some(vec!["68px".to_string(), "54px".to_string()]),
        areas: Some(vec![
            vec!["top".to_string(), "top".to_string(), "top".to_string()],
            vec!["b0".to_string(), "b1".to_string(), "b2".to_string()],
        ]),
        gap: Some("2px".to_string()),
        padding: Some("2px 4px".to_string()),
        align: Some("stretch".to_string()),
        justify: None,
    };
    let style = panel_card_layout_style(
        Some(&layout),
        &json!({
            "height": "44px"
        }),
    );
    assert!(style.contains("grid-template-rows:68px 54px;"));
    assert!(!style.contains("grid-template-rows:44px;"));
    assert!(!style.contains("grid-template-rows:44px 54px;"));
}

#[test]
fn block_style_uses_full_span_in_grid() {
    let layout = grid_layout();
    assert_eq!(
        block_style(Some("full"), Some(&layout)),
        "grid-column:1 / -1;"
    );
}
