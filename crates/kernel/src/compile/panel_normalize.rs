use serde_json::{json, Value};

use crate::compile::entry_payload::clone_merge::deep_merge_json;
use crate::model::{
    BlockDecl, Diagnostic, LayoutDecl, PanelDecl, Severity, UiNodeDecl,
};

const SLOT_HEAD: &str = "head";
const SLOT_BODY: &str = "body";
const PROP_HAS_HEAD: &str = "__mei_has_head";

pub fn normalize_panel_slots(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for panel in panels.iter_mut() {
        normalize_panel(panel, diagnostics, source_path);
    }
}

pub fn panel_resolved_has_head(panel: &PanelDecl) -> bool {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_HAS_HEAD))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| resolve_has_head(panel, &[]))
}

fn normalize_panel(panel: &mut PanelDecl, diagnostics: &mut Vec<Diagnostic>, source_path: &str) {
    merge_head_slot(panel);
    for block in &mut panel.blocks {
        if let UiNodeDecl::Panel(nested) = block {
            normalize_panel(nested, diagnostics, source_path);
        }
    }

    let had_title = panel
        .title
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let had_head_slot = panel.head.is_some();
    let had_head_block = blocks_touch_slot(&panel.blocks, SLOT_HEAD);

    let has_head = resolve_has_head(panel, &[]);
    emit_panel_head_diagnostics(
        panel,
        has_head,
        had_title,
        had_head_slot,
        had_head_block,
        diagnostics,
        source_path,
    );

    if has_head {
        materialize_title_head_block(panel);
    }

    if panel.layout.is_none() {
        inject_default_layout(panel, has_head, panel_has_body_blocks(&panel.blocks, has_head));
    }

    if layout_has_slot(panel.layout.as_ref(), SLOT_BODY)
        || panel
            .layout
            .as_ref()
            .is_none_or(|layout| layout.areas.is_none())
    {
        remap_block_areas_to_body(&mut panel.blocks);
    }

    hoist_heading_to_head_props(panel, diagnostics, source_path);
    stamp_has_head_prop(panel, has_head);
    panel.head = None;
}

fn hoist_heading_to_head_props(
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

fn merge_head_slot(panel: &mut PanelDecl) {
    let Some(head) = panel.head.take() else {
        return;
    };
    let mut node = *head;
    ensure_node_area(&mut node, SLOT_HEAD);
    if !blocks_touch_slot(&panel.blocks, SLOT_HEAD) {
        panel.blocks.insert(0, node);
    }
}

fn resolve_has_head(panel: &PanelDecl, _extra: &[()]) -> bool {
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

fn panel_has_body_blocks(blocks: &[UiNodeDecl], has_head: bool) -> bool {
    if !has_head {
        return !blocks.is_empty();
    }
    blocks.iter().any(|node| {
        node_area(node)
            .map(|area| area != SLOT_HEAD)
            .unwrap_or(true)
    })
}

fn materialize_title_head_block(panel: &mut PanelDecl) {
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

fn inject_default_layout(panel: &mut PanelDecl, has_head: bool, has_body: bool) {
    panel.layout = match (has_head, has_body) {
        (true, true) => Some(default_layout_head_body()),
        (true, false) => Some(default_layout_single_slot(SLOT_HEAD)),
        (false, true) => Some(default_layout_single_slot(SLOT_BODY)),
        (false, false) => None,
    };
}

fn default_layout_head_body() -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string(), "1fr".to_string()]),
        areas: Some(vec![vec![SLOT_HEAD.to_string()], vec![SLOT_BODY.to_string()]]),
        gap: Some("0".to_string()),
        padding: Some("0".to_string()),
        align: None,
        justify: None,
    }
}

fn default_layout_single_slot(slot: &str) -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string()]),
        areas: Some(vec![vec![slot.to_string()]]),
        gap: Some("0".to_string()),
        padding: Some("0".to_string()),
        align: None,
        justify: None,
    }
}

fn layout_has_slot(layout: Option<&LayoutDecl>, slot: &str) -> bool {
    layout
        .and_then(|value| value.areas.as_ref())
        .is_some_and(|rows| {
            rows.iter()
                .flat_map(|row| row.iter())
                .any(|cell| cell == slot)
        })
}

fn remap_block_areas_to_body(blocks: &mut [UiNodeDecl]) {
    for node in blocks {
        match node {
            UiNodeDecl::Block(block) => {
                let area = block.area.as_deref().map(str::trim).unwrap_or("");
                if area.is_empty() || area.eq_ignore_ascii_case("auto") {
                    block.area = Some(SLOT_BODY.to_string());
                }
            }
            UiNodeDecl::Panel(panel) => remap_block_areas_to_body(&mut panel.blocks),
            UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

fn blocks_touch_slot(blocks: &[UiNodeDecl], slot: &str) -> bool {
    blocks
        .iter()
        .any(|node| node_area(node).is_some_and(|area| area == slot))
}

fn node_area(node: &UiNodeDecl) -> Option<&str> {
    match node {
        UiNodeDecl::Block(block) => block.area.as_deref(),
        UiNodeDecl::Panel(panel) => panel.area.as_deref(),
        UiNodeDecl::PanelRefEmbed(embed) => embed.area.as_deref(),
    }
}

fn ensure_node_area(node: &mut UiNodeDecl, slot: &str) {
    match node {
        UiNodeDecl::Block(block) => {
            if block
                .area
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("auto"))
            {
                block.area = Some(slot.to_string());
            }
        }
        UiNodeDecl::Panel(panel) => {
            if panel
                .area
                .as_deref()
                .map(str::trim)
                .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("auto"))
            {
                panel.area = Some(slot.to_string());
            }
        }
        UiNodeDecl::PanelRefEmbed(_) => {}
    }
}

fn stamp_has_head_prop(panel: &mut PanelDecl, has_head: bool) {
    let map = panel
        .props
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut map = map;
    map.insert(PROP_HAS_HEAD.to_string(), Value::Bool(has_head));
    panel.props = Value::Object(map);
}

fn emit_panel_head_diagnostics(
    panel: &PanelDecl,
    has_head: bool,
    had_title: bool,
    had_head_slot: bool,
    had_head_block: bool,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let show_heading = panel
        .props
        .as_object()
        .and_then(|map| map.get("show_heading"))
        .and_then(Value::as_bool);

    if show_heading == Some(false) && (had_title || had_head_slot || had_head_block) {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "redundant_show_heading".to_string(),
            message: format!(
                "panel `{}`: show_heading=False ignores title/head content",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }

    if show_heading == Some(true) && !has_head {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "empty_panel_head".to_string(),
            message: format!(
                "panel `{}`: show_heading=True but no title, head slot, or area=head block",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }

    if had_title && had_head_block && !had_head_slot {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            code: "panel_head_block_overrides_title".to_string(),
            message: format!(
                "panel `{}`: area=head block overrides title string for display",
                panel.id
            ),
            source_path: Some(source_path.to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn panel_with_title(title: &str) -> PanelDecl {
        PanelDecl {
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
        }
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
    fn normalize_hoists_props_heading_to_head_props() {
        let mut panels = vec![PanelDecl {
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
        }];
        let mut diagnostics = Vec::new();
        normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
        let panel = &panels[0];
        assert!(panel.props.as_object().and_then(|m| m.get("heading")).is_none());
        assert_eq!(
            panel
                .head_props
                .get("variant")
                .and_then(Value::as_str),
            Some("screen")
        );
        assert_eq!(
            panel.head_props.get("height").and_then(Value::as_str),
            Some("40px")
        );
    }

    #[test]
    fn normalize_no_head_without_title() {
        let mut panels = vec![PanelDecl {
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
        }];
        let mut diagnostics = Vec::new();
        normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
        assert!(!panel_resolved_has_head(&panels[0]));
        assert!(!blocks_touch_slot(&panels[0].blocks, SLOT_HEAD));
    }
}
