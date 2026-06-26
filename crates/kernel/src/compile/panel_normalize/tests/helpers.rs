use super::super::*;
pub use crate::model::{BlockDecl, LayoutDecl, PanelDecl, UiNodeDecl};
pub use serde_json::{json, Value};

pub(super) use super::super::nodes::node_area;

pub(super) fn panel_with_title(title: &str) -> PanelDecl {
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

pub(super) fn metric_card_panel(id: &str) -> UiNodeDecl {
    metric_card_panel_with_height(id, None)
}

pub(super) fn metric_card_panel_with_height(id: &str, height: Option<&str>) -> UiNodeDecl {
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

pub(super) fn metric_card_panel_with_extra_props(
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

