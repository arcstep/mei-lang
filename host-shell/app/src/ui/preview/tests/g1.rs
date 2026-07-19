use super::style::{panel_card_layout_style, panel_style, surface_layout_style};
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
fn surface_layout_style_emits_grid_template_areas() {
    let layout = grid_layout();
    let style = surface_layout_style(Some(&layout));
    assert!(style.contains("grid-template-areas:'doc table';"));
}

#[test]
fn panel_style_requires_named_grid_areas() {
    let mut layout = grid_layout();
    layout.areas = None;
    assert_eq!(panel_style(Some("doc"), Some(&layout), &json!({})), "");
}

#[test]
fn panel_card_layout_style_applies_grid_columns() {
    let layout = grid_layout();
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("display:grid;"));
    assert!(style.contains("grid-template-columns:1fr 2fr;"));
}

#[test]
fn panel_card_layout_style_emits_grid_align_items() {
    let mut layout = grid_layout();
    layout.align = Some("stretch".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("align-items:stretch;"));
}

#[test]
fn panel_card_layout_style_emits_grid_justify_content() {
    let mut layout = grid_layout();
    layout.justify = Some("center".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("justify-content:center;"));
}
