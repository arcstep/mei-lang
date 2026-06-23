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

fn title_block_typography_from_head_props(head_props: &Value) -> Value {
    let Some(map) = head_props.as_object() else {
        return json!({});
    };
    const KEYS: &[&str] = &[
        "font",
        "font_size",
        "font_family",
        "font_weight",
        "letter_spacing",
        "color",
        "text_align",
        "align",
        "line_height",
    ];
    let mut out = serde_json::Map::new();
    let has_font = map.get("font").is_some_and(|value| {
        value.as_str().is_some_and(|raw| !raw.trim().is_empty()) || value.as_i64().is_some()
    });
    for key in KEYS {
        if has_font && matches!(*key, "font_size" | "fontSize") {
            continue;
        }
        if let Some(value) = map.get(*key) {
            if value.is_string() {
                if value.as_str().is_some_and(|raw| !raw.trim().is_empty()) {
                    out.insert((*key).to_string(), value.clone());
                }
            } else if !value.is_null() {
                out.insert((*key).to_string(), value.clone());
            }
        }
    }
    Value::Object(out)
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
    let mut props = serde_json::Map::new();
    props.insert("content".to_string(), Value::String(title.to_string()));
    if let Some(typography) = title_block_typography_from_head_props(&panel.head_props).as_object()
    {
        for (key, value) in typography {
            props.insert(key.clone(), value.clone());
        }
    }
    panel.blocks.insert(
        0,
        UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: None,
            title: None,
            area: Some(SLOT_HEAD.to_string()),
            props: Value::Object(props),
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
