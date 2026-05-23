use serde_json::{json, Value};

use crate::compile::entry_payload::clone_merge::deep_merge_json;
use crate::model::{BlockDecl, Diagnostic, PanelDecl, Severity, UiNodeDecl};

use super::constants::SLOT_HEAD;
use super::nodes::{blocks_touch_slot, ensure_node_area, node_area};

pub(super) fn hoist_heading_to_head_props(
    panel: &mut PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(props_map) = panel.props.as_object() else {
        return;
    };
    let Some(heading) = props_map.get("heading").cloned() else {
        return;
    };
    let head_has_content = panel
        .head_props
        .as_object()
        .is_some_and(|map| !map.is_empty());
    if head_has_content {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            code: "heading_migrated_to_head_props".to_string(),
            message: format!(
                "panel `{}`: props.heading is ignored when head_props is set; use head_props only",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    } else {
        panel.head_props = deep_merge_json(&panel.head_props, &heading);
    }
    let mut map = props_map.clone();
    map.remove("heading");
    panel.props = Value::Object(map);
}
pub(super) fn merge_head_slot(panel: &mut PanelDecl) {
    let Some(head) = panel.head.take() else {
        return;
    };
    let mut node = *head;
    ensure_node_area(&mut node, SLOT_HEAD);
    if !blocks_touch_slot(&panel.blocks, SLOT_HEAD) {
        panel.blocks.insert(0, node);
    }
}
pub(super) fn resolve_has_head(panel: &PanelDecl, _extra: &[()]) -> bool {
    if let Some(show) = panel
        .props
        .as_object()
        .and_then(|map| map.get("show_heading"))
        .and_then(Value::as_bool)
    {
        return show;
    }
    let title = panel
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if title.is_some() {
        return true;
    }
    if panel.head.as_ref().is_some() {
        return true;
    }
    blocks_touch_slot(&panel.blocks, SLOT_HEAD)
}

pub(super) fn panel_has_body_blocks(blocks: &[UiNodeDecl], has_head: bool) -> bool {
    if !has_head {
        return !blocks.is_empty();
    }
    blocks.iter().any(|node| {
        node_area(node)
            .map(|area| area != SLOT_HEAD)
            .unwrap_or(true)
    })
}

pub(super) fn materialize_title_head_block(panel: &mut PanelDecl) {
    if blocks_touch_slot(&panel.blocks, SLOT_HEAD) {
        return;
    }
    let Some(title) = panel
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    panel.blocks.insert(
        0,
        UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: None,
            title: None,
            area: Some(SLOT_HEAD.to_string()),
            props: json!({ "content": title }),
            base: None,
            layout: None,
            blocks: vec![],
            component: None,
            placement: None,
            interactions: vec![],
            lifecycle: None,
            constraints: None,
            data: None,
        }),
    );
}
