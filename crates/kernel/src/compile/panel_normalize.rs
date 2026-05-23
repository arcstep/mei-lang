use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::compile::entry_payload::clone_merge::deep_merge_json;
use crate::model::{BlockDecl, Diagnostic, LayoutDecl, PanelDecl, Severity, UiNodeDecl};

const SLOT_HEAD: &str = "head";
const SLOT_BODY: &str = "body";
const PROP_HAS_HEAD: &str = "__mei_has_head";
const PROP_METRIC_CARD: &str = "__mei_metric_card";
const PROP_LAYOUT_POLICY: &str = "__mei_layout_policy";
const PROP_LAYOUT_GAP: &str = "__mei_layout_gap";
const PROP_LAYOUT_PADDING: &str = "__mei_layout_padding";
const PROP_LAYOUT_COLUMNS: &str = "__mei_layout_columns";
const LAYOUT_POLICY_METRICS_STRIP: &str = "metrics_strip";
const LAYOUT_POLICY_METRICS_2_1: &str = "metrics_2_1";
const LAYOUT_POLICY_METRIC_COMPOUND_2_1: &str = "metric_compound_2_1";
const DEFAULT_METRICS_STRIP_GAP: &str = "10px";
const DEFAULT_METRICS_STRIP_PADDING: &str = "10px";
const DEFAULT_METRICS_2_1_GAP: &str = "8px";
const DEFAULT_METRICS_2_1_PADDING: &str = "24px 21px";
const DEFAULT_METRICS_2_1_COLUMNS: [&str; 3] = ["114px", "114px", "234px"];
const DEFAULT_METRIC_COMPOUND_2_1_GAP: &str = "2px";

#[derive(Debug, Clone)]
struct PolicySpacing {
    gap: String,
    padding: String,
}

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

    let has_body = panel_has_body_blocks(&panel.blocks, has_head);
    if panel.layout.is_none() {
        let requested_policy = panel_layout_policy(panel);
        match requested_policy.as_deref() {
            Some(LAYOUT_POLICY_METRICS_STRIP) => {
                if should_inject_metrics_strip(panel, has_head) {
                    let spacing = policy_spacing(
                        panel,
                        DEFAULT_METRICS_STRIP_GAP,
                        DEFAULT_METRICS_STRIP_PADDING,
                    );
                    inject_default_metrics_strip_layout(panel, &spacing);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRICS_STRIP);
                } else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "layout_policy_metrics_strip_conflict".to_string(),
                        message: format!(
                            "panel `{}`: layout_policy=metrics_strip requires at least 2 metric_card children and no head slot",
                            panel.id
                        ),
                        source_path: Some(source_path.to_string()),
                    });
                    inject_default_layout(panel, has_head, has_body);
                }
            }
            Some(LAYOUT_POLICY_METRICS_2_1) => {
                if should_inject_metrics_2_1(panel, has_head) {
                    inject_default_metrics_2_1_layout(panel);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRICS_2_1);
                } else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "layout_policy_metrics_2_1_conflict".to_string(),
                        message: format!(
                            "panel `{}`: layout_policy=metrics_2_1 requires exactly 3 metric_card children and no head slot",
                            panel.id
                        ),
                        source_path: Some(source_path.to_string()),
                    });
                    inject_default_layout(panel, has_head, has_body);
                }
            }
            Some(LAYOUT_POLICY_METRIC_COMPOUND_2_1) => {
                if should_inject_metric_compound_2_1(panel, has_head) {
                    inject_default_metric_compound_2_1_layout(panel);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRIC_COMPOUND_2_1);
                } else {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "layout_policy_metric_compound_2_1_conflict".to_string(),
                        message: format!(
                            "panel `{}`: layout_policy=metric_compound_2_1 requires exactly 4 metric_card children and no head slot",
                            panel.id
                        ),
                        source_path: Some(source_path.to_string()),
                    });
                    inject_default_layout(panel, has_head, has_body);
                }
            }
            Some(policy) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_layout_policy".to_string(),
                    message: format!(
                        "panel `{}`: unknown layout_policy `{policy}`, fallback to default panel layout",
                        panel.id
                    ),
                    source_path: Some(source_path.to_string()),
                });
                inject_default_layout(panel, has_head, has_body);
            }
            None => {
                if should_inject_metrics_strip(panel, has_head) {
                    let spacing = policy_spacing(
                        panel,
                        DEFAULT_METRICS_STRIP_GAP,
                        DEFAULT_METRICS_STRIP_PADDING,
                    );
                    inject_default_metrics_strip_layout(panel, &spacing);
                    stamp_layout_policy(panel, LAYOUT_POLICY_METRICS_STRIP);
                } else {
                    inject_default_layout(panel, has_head, has_body);
                }
            }
        }
    }

    if layout_has_slot(panel.layout.as_ref(), SLOT_BODY)
        || panel
            .layout
            .as_ref()
            .is_none_or(|layout| layout.areas.is_none())
    {
        remap_block_areas_to_body(&mut panel.blocks);
    }
    emit_layout_audit_diagnostics(panel, diagnostics, source_path);

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
        (true, true) => Some(default_layout_head_body(panel_head_height_track(panel))),
        (true, false) => Some(default_layout_single_slot(SLOT_HEAD)),
        (false, true) => Some(default_layout_single_slot(SLOT_BODY)),
        (false, false) => None,
    };
}

fn default_layout_head_body(head_track: Option<String>) -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec![
            head_track.unwrap_or_else(|| "auto".to_string()),
            "1fr".to_string(),
        ]),
        areas: Some(vec![
            vec![SLOT_HEAD.to_string()],
            vec![SLOT_BODY.to_string()],
        ]),
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

fn default_metrics_strip_layout(count: usize, spacing: &PolicySpacing) -> LayoutDecl {
    let mut areas = Vec::with_capacity(count);
    let mut columns = Vec::with_capacity(count);
    for idx in 0..count {
        areas.push(format!("m{idx}"));
        columns.push("1fr".to_string());
    }
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(columns),
        rows: Some(vec!["auto".to_string()]),
        areas: Some(vec![areas]),
        gap: Some(spacing.gap.clone()),
        padding: Some(spacing.padding.clone()),
        align: Some("stretch".to_string()),
        justify: None,
    }
}

fn default_metrics_2_1_layout(panel: &PanelDecl) -> LayoutDecl {
    let columns = panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_COLUMNS))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| values.len() == 3)
        .unwrap_or_else(|| {
            DEFAULT_METRICS_2_1_COLUMNS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        });
    let spacing = policy_spacing(panel, DEFAULT_METRICS_2_1_GAP, DEFAULT_METRICS_2_1_PADDING);
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(columns),
        rows: Some(vec!["auto".to_string()]),
        areas: Some(vec![vec![
            "m0".to_string(),
            "m1".to_string(),
            "m2".to_string(),
        ]]),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
        justify: None,
    }
}

fn default_metric_compound_2_1_layout(panel: &PanelDecl) -> LayoutDecl {
    let spacing = policy_spacing(panel, DEFAULT_METRIC_COMPOUND_2_1_GAP, "0");
    let top_row = panel
        .blocks
        .first()
        .and_then(node_height_track)
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string());
    let bottom_row = panel
        .blocks
        .iter()
        .skip(1)
        .filter_map(node_height_track)
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(existing) => Some(existing.max(value)),
            None => Some(value),
        })
        .map(px_track)
        .unwrap_or_else(|| "auto".to_string());
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec![
            "1fr".to_string(),
            "1fr".to_string(),
            "1fr".to_string(),
        ]),
        rows: Some(vec![top_row, bottom_row]),
        areas: Some(vec![
            vec!["top".to_string(), "top".to_string(), "top".to_string()],
            vec!["b0".to_string(), "b1".to_string(), "b2".to_string()],
        ]),
        gap: Some(spacing.gap),
        padding: Some(spacing.padding),
        align: Some("stretch".to_string()),
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

fn should_inject_metrics_strip(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() < 2 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

fn should_inject_metrics_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 3 {
        return false;
    }
    panel.blocks.iter().all(node_is_metrics_2_1_item_like)
}

fn should_inject_metric_compound_2_1(panel: &PanelDecl, has_head: bool) -> bool {
    if has_head || panel.blocks.len() != 4 {
        return false;
    }
    panel.blocks.iter().all(node_is_metric_card_like)
}

fn inject_default_metrics_strip_layout(panel: &mut PanelDecl, spacing: &PolicySpacing) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_strip_layout(panel.blocks.len(), spacing));
}

fn inject_default_metrics_2_1_layout(panel: &mut PanelDecl) {
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, &format!("m{idx}"));
    }
    panel.layout = Some(default_metrics_2_1_layout(panel));
}

fn inject_default_metric_compound_2_1_layout(panel: &mut PanelDecl) {
    let areas = ["top", "b0", "b1", "b2"];
    for (idx, node) in panel.blocks.iter_mut().enumerate() {
        set_node_area(node, areas[idx]);
    }
    panel.layout = Some(default_metric_compound_2_1_layout(panel));
}

fn node_is_metric_card_like(node: &UiNodeDecl) -> bool {
    match node {
        UiNodeDecl::Panel(panel) => panel
            .props
            .as_object()
            .and_then(|map| map.get(PROP_METRIC_CARD))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        _ => false,
    }
}

fn node_is_metrics_2_1_item_like(node: &UiNodeDecl) -> bool {
    if node_is_metric_card_like(node) {
        return true;
    }
    match node {
        UiNodeDecl::Panel(panel) => panel_layout_policy(panel)
            .as_deref()
            .is_some_and(|policy| policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1),
        _ => false,
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

fn set_node_area(node: &mut UiNodeDecl, area: &str) {
    match node {
        UiNodeDecl::Block(block) => block.area = Some(area.to_string()),
        UiNodeDecl::Panel(panel) => panel.area = Some(area.to_string()),
        UiNodeDecl::PanelRefEmbed(embed) => embed.area = Some(area.to_string()),
    }
}

fn stamp_has_head_prop(panel: &mut PanelDecl, has_head: bool) {
    let map = panel.props.as_object().cloned().unwrap_or_default();
    let mut map = map;
    map.insert(PROP_HAS_HEAD.to_string(), Value::Bool(has_head));
    panel.props = Value::Object(map);
}

fn stamp_layout_policy(panel: &mut PanelDecl, policy: &str) {
    let map = panel.props.as_object().cloned().unwrap_or_default();
    let mut map = map;
    map.insert(
        PROP_LAYOUT_POLICY.to_string(),
        Value::String(policy.to_string()),
    );
    panel.props = Value::Object(map);
}

fn panel_layout_policy(panel: &PanelDecl) -> Option<String> {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_POLICY))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn policy_spacing(panel: &PanelDecl, default_gap: &str, default_padding: &str) -> PolicySpacing {
    let gap = panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_GAP))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_gap)
        .to_string();
    let padding = panel
        .props
        .as_object()
        .and_then(|map| map.get(PROP_LAYOUT_PADDING))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_padding)
        .to_string();
    PolicySpacing { gap, padding }
}

fn emit_layout_audit_diagnostics(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(layout) = panel.layout.as_ref() else {
        return;
    };
    audit_layout_matrix(panel, layout, diagnostics, source_path);
    audit_layout_area_mapping(panel, layout, diagnostics, source_path);
    audit_layout_spacing(layout, panel, diagnostics, source_path);
    audit_fixed_track_budget(panel, layout, diagnostics, source_path);
    audit_head_body_balance(panel, layout, diagnostics, source_path);
}

fn audit_layout_matrix(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(areas) = layout.areas.as_ref() else {
        if panel.blocks.iter().any(node_has_explicit_area) {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_audit_missing_areas".to_string(),
                message: format!(
                    "panel `{}`: blocks declare explicit area but layout.areas is missing",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
        return;
    };
    if areas.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_audit_empty_areas".to_string(),
            message: format!("panel `{}`: layout.areas is empty", panel.id),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let width = areas.first().map(Vec::len).unwrap_or(0);
    if width == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_audit_empty_area_row".to_string(),
            message: format!("panel `{}`: first areas row has zero columns", panel.id),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    for (row_idx, row) in areas.iter().enumerate() {
        if row.len() != width {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_irregular_area_matrix".to_string(),
                message: format!(
                    "panel `{}`: areas row {} has {} columns, expected {}",
                    panel.id,
                    row_idx + 1,
                    row.len(),
                    width
                ),
                source_path: Some(source_path.to_string()),
            });
            break;
        }
    }
    if let Some(columns) = layout.columns.as_ref() {
        if !columns.is_empty() && columns.len() != width {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_audit_columns_area_mismatch".to_string(),
                message: format!(
                    "panel `{}`: columns count ({}) differs from area columns ({width})",
                    panel.id,
                    columns.len()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(rows) = layout.rows.as_ref() {
        if !rows.is_empty() && rows.len() != areas.len() {
            diagnostics.push(Diagnostic {
                severity: Severity::Info,
                code: "layout_audit_rows_area_mismatch".to_string(),
                message: format!(
                    "panel `{}`: rows count ({}) differs from area rows ({})",
                    panel.id,
                    rows.len(),
                    areas.len()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

fn audit_layout_area_mapping(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(area_rows) = layout.areas.as_ref() else {
        return;
    };
    let mut declared = BTreeSet::new();
    for row in area_rows {
        for cell in row {
            let cell = cell.trim();
            if cell.is_empty() || cell == "." {
                continue;
            }
            declared.insert(cell.to_string());
        }
    }
    if declared.is_empty() {
        return;
    }
    for node in &panel.blocks {
        let Some(area) = node_area(node) else {
            continue;
        };
        let area = area.trim();
        if area.is_empty() || area.eq_ignore_ascii_case("auto") {
            continue;
        }
        if !declared.contains(area) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_unknown_block_area".to_string(),
                message: format!(
                    "panel `{}`: block area `{area}` not declared in layout.areas",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

fn audit_layout_spacing(
    layout: &LayoutDecl,
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if let Some(gap) = layout.gap.as_deref() {
        let numbers = css_scalar_numbers(gap);
        if numbers.iter().any(|value| *value < 0.0) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_negative_gap".to_string(),
                message: format!(
                    "panel `{}`: layout.gap has negative value `{gap}`",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(padding) = layout.padding.as_deref() {
        let numbers = css_scalar_numbers(padding);
        if numbers.iter().any(|value| *value < 0.0) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_negative_padding".to_string(),
                message: format!(
                    "panel `{}`: layout.padding has negative value `{padding}`",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(rows) = layout.rows.as_ref() {
        if rows.iter().any(|row| is_degenerate_track(row)) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_degenerate_rows".to_string(),
                message: format!(
                    "panel `{}`: layout.rows contains zero-sized track",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let Some(columns) = layout.columns.as_ref() {
        if columns.iter().any(|col| is_degenerate_track(col)) {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_degenerate_columns".to_string(),
                message: format!(
                    "panel `{}`: layout.columns contains zero-sized track",
                    panel.id
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

fn audit_fixed_track_budget(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let panel_width = panel_px_prop(panel, "width");
    let panel_height = panel_px_prop(panel, "height");
    let row_budget = layout
        .rows
        .as_ref()
        .and_then(|rows| sum_fixed_px_tracks(rows));
    let col_budget = layout
        .columns
        .as_ref()
        .and_then(|columns| sum_fixed_px_tracks(columns));
    if let (Some(height), Some(rows_px)) = (panel_height, row_budget) {
        if rows_px > height + 1.0 {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_row_budget_overflow".to_string(),
                message: format!(
                    "panel `{}`: fixed rows {}px exceed panel height {}px",
                    panel.id,
                    rows_px.round(),
                    height.round()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
    if let (Some(width), Some(cols_px)) = (panel_width, col_budget) {
        if cols_px > width + 1.0 {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "layout_audit_column_budget_overflow".to_string(),
                message: format!(
                    "panel `{}`: fixed columns {}px exceed panel width {}px",
                    panel.id,
                    cols_px.round(),
                    width.round()
                ),
                source_path: Some(source_path.to_string()),
            });
        }
    }
}

fn audit_head_body_balance(
    panel: &PanelDecl,
    layout: &LayoutDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if !layout_has_slot(Some(layout), SLOT_HEAD) || !layout_has_slot(Some(layout), SLOT_BODY) {
        return;
    }
    let Some(panel_height) = panel_px_prop(panel, "height") else {
        return;
    };
    let Some(head_height) = panel_head_height_track(panel).and_then(|value| parse_px(&value))
    else {
        return;
    };
    if panel_height <= head_height + 1.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_audit_head_body_height_conflict".to_string(),
            message: format!(
                "panel `{}`: panel height {}px is not enough for head height {}px",
                panel.id,
                panel_height.round(),
                head_height.round()
            ),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let available_body = panel_height - head_height - layout_gap_y_px(layout);
    let Some(required_body) = estimate_body_required_height(panel) else {
        return;
    };
    if required_body > available_body + 1.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "layout_audit_body_clip_risk".to_string(),
            message: format!(
                "panel `{}`: body available {}px is smaller than inferred content {}px (may clip)",
                panel.id,
                available_body.round(),
                required_body.round()
            ),
            source_path: Some(source_path.to_string()),
        });
        return;
    }
    let slack = available_body - required_body;
    if slack > 24.0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            code: "layout_audit_body_spacing_loose".to_string(),
            message: format!(
                "panel `{}`: body has {}px extra slack over inferred content (may look too loose)",
                panel.id,
                slack.round()
            ),
            source_path: Some(source_path.to_string()),
        });
    }
}

fn estimate_body_required_height(panel: &PanelDecl) -> Option<f64> {
    let body_panel = panel.blocks.iter().find_map(|node| match node {
        UiNodeDecl::Panel(value) if node_area(node) == Some(SLOT_BODY) => Some(value),
        _ => None,
    })?;
    let body_layout = body_panel.layout.as_ref()?;
    let policy = panel_layout_policy(body_panel)?;
    if policy == LAYOUT_POLICY_METRICS_2_1 || policy == LAYOUT_POLICY_METRICS_STRIP {
        let card_height = body_panel
            .blocks
            .iter()
            .filter_map(node_height_track)
            .fold(None, |acc: Option<f64>, value| match acc {
                Some(existing) => Some(existing.max(value)),
                None => Some(value),
            })?;
        let padding_vertical = layout_padding_vertical_px(body_layout);
        return Some(card_height + padding_vertical);
    }
    if policy == LAYOUT_POLICY_METRIC_COMPOUND_2_1 {
        let rows = body_layout.rows.as_ref()?;
        let row_budget = sum_fixed_px_tracks(rows)?;
        let padding_vertical = layout_padding_vertical_px(body_layout);
        let gap = layout_gap_y_px(body_layout);
        return Some(row_budget + padding_vertical + gap);
    }
    None
}

fn layout_gap_y_px(layout: &LayoutDecl) -> f64 {
    layout
        .gap
        .as_deref()
        .and_then(first_css_scalar_px)
        .unwrap_or(0.0)
}

fn layout_padding_vertical_px(layout: &LayoutDecl) -> f64 {
    layout
        .padding
        .as_deref()
        .map(padding_vertical_px)
        .unwrap_or(0.0)
}

fn node_has_explicit_area(node: &UiNodeDecl) -> bool {
    node_area(node)
        .map(str::trim)
        .is_some_and(|area| !area.is_empty() && !area.eq_ignore_ascii_case("auto"))
}

fn panel_px_prop(panel: &PanelDecl, key: &str) -> Option<f64> {
    panel
        .props
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(value_as_px)
}

fn panel_head_height_track(panel: &PanelDecl) -> Option<String> {
    panel
        .head_props
        .as_object()
        .and_then(|map| map.get("height"))
        .and_then(value_as_px)
        .map(px_track)
}

fn node_height_track(node: &UiNodeDecl) -> Option<f64> {
    match node {
        UiNodeDecl::Panel(panel) => panel_px_prop(panel, "height"),
        _ => None,
    }
}

fn px_track(value: f64) -> String {
    format!("{}px", value.round())
}

fn sum_fixed_px_tracks(tracks: &[String]) -> Option<f64> {
    let mut sum = 0.0;
    for track in tracks {
        let value = track.trim();
        if let Some(px) = parse_px(value) {
            sum += px.max(0.0);
            continue;
        }
        return None;
    }
    Some(sum)
}

fn is_degenerate_track(token: &str) -> bool {
    let token = token.trim().to_ascii_lowercase();
    token == "0" || token == "0px" || token.starts_with("minmax(0")
}

fn css_scalar_numbers(value: &str) -> Vec<f64> {
    value
        .split_whitespace()
        .filter_map(|token| {
            let token = token.trim().trim_end_matches(',');
            parse_px(token).or_else(|| token.parse::<f64>().ok())
        })
        .collect()
}

fn first_css_scalar_px(value: &str) -> Option<f64> {
    value
        .split_whitespace()
        .find_map(|token| parse_px(token.trim().trim_end_matches(',')))
}

fn padding_vertical_px(value: &str) -> f64 {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.is_empty() {
        return 0.0;
    }
    let top = parse_px(tokens[0]).unwrap_or(0.0);
    let bottom = if tokens.len() >= 3 {
        parse_px(tokens[2]).unwrap_or(top)
    } else {
        top
    };
    top + bottom
}

fn parse_px(value: &str) -> Option<f64> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(px) = raw.strip_suffix("px") {
        return px.trim().parse::<f64>().ok();
    }
    if raw == "0" {
        return Some(0.0);
    }
    None
}

fn value_as_px(value: &Value) -> Option<f64> {
    if let Some(raw) = value.as_str() {
        return parse_px(raw);
    }
    value.as_f64()
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

    fn metric_card_panel(id: &str) -> UiNodeDecl {
        metric_card_panel_with_height(id, None)
    }

    fn metric_card_panel_with_height(id: &str, height: Option<&str>) -> UiNodeDecl {
        UiNodeDecl::Panel(PanelDecl {
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

    #[test]
    fn normalize_injects_metrics_strip_layout_for_metric_children() {
        let mut panels = vec![PanelDecl {
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
        assert_eq!(layout.gap.as_deref(), Some("10px"));
        assert_eq!(layout.padding.as_deref(), Some("10px"));
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
        assert_eq!(layout.padding.as_deref(), Some("24px 21px"));
        assert_eq!(
            panel
                .props
                .get("__mei_layout_policy")
                .and_then(Value::as_str),
            Some("metrics_2_1")
        );
    }

    #[test]
    fn normalize_warns_when_metrics_2_1_policy_shape_is_invalid() {
        let mut panels = vec![PanelDecl {
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
    fn normalize_injects_metric_compound_2_1_layout_when_policy_matches() {
        let mut panels = vec![PanelDecl {
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
            Some(&vec!["68px".to_string(), "54px".to_string()])
        );
        assert_eq!(layout.gap.as_deref(), Some("2px"));
    }

    #[test]
    fn normalize_warns_when_metric_compound_2_1_policy_shape_is_invalid() {
        let mut panels = vec![PanelDecl {
            kind: "panel".to_string(),
            id: "compound_invalid".to_string(),
            title: None,
            head: None::<Box<UiNodeDecl>>,
            area: Some("auto".to_string()),
            layout: None,
            blocks: vec![
                metric_card_panel("top"),
                metric_card_panel("b0"),
                metric_card_panel("b1"),
            ],
            props: json!({
                "__mei_layout_policy": "metric_compound_2_1",
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
        }];
        let mut diagnostics = Vec::new();
        normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "layout_policy_metric_compound_2_1_conflict"));
    }

    #[test]
    fn normalize_emits_layout_audit_for_unknown_block_area() {
        let mut panels = vec![PanelDecl {
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
        }];
        let mut diagnostics = Vec::new();
        normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == "layout_audit_unknown_block_area"));
    }

    #[test]
    fn normalize_emits_body_clip_risk_for_head_body_metrics_conflict() {
        let mut panels = vec![PanelDecl {
            kind: "panel".to_string(),
            id: "outer".to_string(),
            title: Some("标题".to_string()),
            head: None::<Box<UiNodeDecl>>,
            area: Some("auto".to_string()),
            layout: None,
            blocks: vec![UiNodeDecl::Panel(PanelDecl {
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
            })],
            props: json!({ "height": "180px" }),
            head_props: json!({ "height": "54px" }),
            body_props: json!({}),
            base: None,
        }];
        let mut diagnostics = Vec::new();
        normalize_panel_slots(&mut panels, &mut diagnostics, "main.mei");
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.code == "layout_audit_body_clip_risk"),
            "expected body clip risk diagnostic, got: {:?}",
            diagnostics
        );
    }
}
