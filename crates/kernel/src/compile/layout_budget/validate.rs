use serde_json::Value;

use crate::model::{Diagnostic, PanelDecl, Severity, UiNodeDecl};
use crate::theme_tokens::is_literal_font_size;

use super::padding::{padding_profile_vertical_px, TITLE_BAR_HEIGHT_PX};

#[derive(Debug, Clone, Default)]
pub struct LayoutBudgetValidateOptions {
    pub strict_t1_fill_down: bool,
}

#[derive(Debug, Clone, Default)]
struct ValidateContext {
    tier: Option<String>,
    in_fill_down: bool,
}

pub fn emit_layout_budget_policy_diagnostics(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    validate_layout_budget_policy(panels, diagnostics, source_path);
    materialize_layout_budget_px(panels, diagnostics, source_path);
}

/// Compile-time policy validation only (no px materialization).
pub fn validate_layout_budget_policy(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    validate_layout_budget_policy_with_options(
        panels,
        diagnostics,
        source_path,
        &LayoutBudgetValidateOptions::default(),
    )
}

pub fn validate_layout_budget_policy_with_options(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
    options: &LayoutBudgetValidateOptions,
) {
    let mut flat = Vec::new();
    for panel in panels.iter() {
        collect_panels(panel, &mut flat);
    }
    let panel_map: std::collections::HashMap<&str, &PanelDecl> =
        flat.iter().map(|p| (p.id.as_str(), *p)).collect();
    let fill_descendants = collect_fill_down_descendant_ids(&flat);
    for panel in flat.iter() {
        let tier = panel_tier(panel);
        let in_fill_down = fill_descendants.contains(panel.id.as_str()) || is_layout_fill_panel(panel);
        let ctx = ValidateContext {
            tier,
            in_fill_down,
        };
        validate_panel(panel, &panel_map, &ctx, diagnostics, source_path, options);
    }
}

fn collect_fill_down_descendant_ids(flat: &[&PanelDecl]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for panel in flat {
        if is_layout_fill_panel(panel) {
            mark_panel_subtree_ids(panel, &mut ids);
        }
    }
    ids
}

fn mark_panel_subtree_ids(panel: &PanelDecl, out: &mut std::collections::HashSet<String>) {
    out.insert(panel.id.clone());
    for node in &panel.blocks {
        if let UiNodeDecl::Panel(child) = node {
            mark_panel_subtree_ids(child, out);
        }
    }
}

/// Stamp `__mei_section_derived_height_px` on fill-down sections from region fr projection
/// without materializing region row tracks to px (Build index / preview path).
pub fn materialize_fill_section_derived_heights(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let (mut derived_map, region_ids) = {
        let mut flat = Vec::new();
        for panel in panels.iter() {
            collect_panels(panel, &mut flat);
        }
        let panel_map: std::collections::HashMap<&str, &PanelDecl> =
            flat.iter().map(|p| (p.id.as_str(), *p)).collect();

        let mut derived_map: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for panel in flat.iter() {
            if ui_role(panel) != Some("section") {
                continue;
            }
            if section_body_uses_fill(panel, &panel_map) {
                continue;
            }
            if let Some(h) = compute_section_derived_height(panel, &panel_map) {
                derived_map.insert(panel.id.clone(), h);
            }
        }

        let region_ids: Vec<String> = flat
            .iter()
            .filter(|p| ui_role(p) == Some("region"))
            .map(|p| p.id.clone())
            .collect();

        (derived_map, region_ids)
    };

    let fill_section_ids = collect_fill_section_ids(panels);

    materialize_regions_on_tree_fill_only(
        panels,
        &mut derived_map,
        &fill_section_ids,
        diagnostics,
        source_path,
    );
    stamp_derived_on_tree(panels, &derived_map);

    let _ = region_ids;
}

fn materialize_regions_on_tree_fill_only(
    panels: &mut [PanelDecl],
    derived: &mut std::collections::HashMap<String, f64>,
    fill_section_ids: &std::collections::HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for panel in panels.iter_mut() {
        materialize_regions_fill_only_recursive(
            panel,
            derived,
            fill_section_ids,
            diagnostics,
            source_path,
        );
    }
}

fn materialize_regions_fill_only_recursive(
    panel: &mut PanelDecl,
    derived: &mut std::collections::HashMap<String, f64>,
    fill_section_ids: &std::collections::HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if ui_role(panel) == Some("region") {
        materialize_region_fr_rows_fill_only(
            panel,
            derived,
            fill_section_ids,
            diagnostics,
            source_path,
        );
    }
    for node in panel.blocks.iter_mut() {
        if let UiNodeDecl::Panel(child) = node {
            materialize_regions_fill_only_recursive(
                child,
                derived,
                fill_section_ids,
                diagnostics,
                source_path,
            );
        }
    }
}

fn materialize_region_fr_rows_fill_only(
    region: &mut PanelDecl,
    derived: &mut std::collections::HashMap<String, f64>,
    fill_section_ids: &std::collections::HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(viewport_h) = section_viewport_inner_height(region) else {
        return;
    };
    let section_ids: Vec<String> = region
        .blocks
        .iter()
        .filter_map(|n| match n {
            UiNodeDecl::Panel(p) => Some(p.id.clone()),
            _ => None,
        })
        .collect();
    let Some(layout) = region.layout.as_ref() else {
        return;
    };
    let Some(rows) = layout.rows.as_ref() else {
        return;
    };
    if section_ids.len() != rows.len() {
        return;
    }
    let fr_weights: Vec<f64> = rows.iter().filter_map(|r| parse_fr_weight(r)).collect();
    if fr_weights.len() != rows.len() {
        return;
    }
    let gap_px = layout
        .gap
        .as_deref()
        .and_then(parse_px_str)
        .unwrap_or(0.0);
    let gap_total = if section_ids.len() > 1 {
        gap_px * (section_ids.len() as f64 - 1.0)
    } else {
        0.0
    };
    let inner = (viewport_h - gap_total).max(0.0);
    let fr_sum: f64 = fr_weights.iter().sum();
    if fr_sum <= 0.0 {
        return;
    }
    for (i, sid) in section_ids.iter().enumerate() {
        if !fill_section_ids.contains(sid) {
            continue;
        }
        let row_px = inner * fr_weights[i] / fr_sum;
        derived.insert(sid.clone(), row_px);
        let _ = diagnostics;
        let _ = source_path;
    }
}

/// Optional px materialization for baseline SSR / transition paths.
pub fn materialize_layout_budget_px(
    panels: &mut [PanelDecl],
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let (mut derived_map, region_ids) = {
        let mut flat = Vec::new();
        for panel in panels.iter() {
            collect_panels(panel, &mut flat);
        }
        let panel_map: std::collections::HashMap<&str, &PanelDecl> =
            flat.iter().map(|p| (p.id.as_str(), *p)).collect();

        let mut derived_map: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for panel in flat.iter() {
            if ui_role(panel) != Some("section") {
                continue;
            }
            if section_body_uses_fill(panel, &panel_map) {
                continue;
            }
            if let Some(h) = compute_section_derived_height(panel, &panel_map) {
                derived_map.insert(panel.id.clone(), h);
            }
        }

        let region_ids: Vec<String> = flat
            .iter()
            .filter(|p| ui_role(p) == Some("region"))
            .map(|p| p.id.clone())
            .collect();

        (derived_map, region_ids)
    };

    let fill_section_ids = collect_fill_section_ids(panels);

    materialize_regions_on_tree(
        panels,
        &mut derived_map,
        &fill_section_ids,
        diagnostics,
        source_path,
    );
    stamp_derived_on_tree(panels, &derived_map);

    for region_id in &region_ids {
        if let Some(region) = find_panel_by_id(panels, region_id.as_str()) {
            validate_region_overflow(region, diagnostics, source_path);
        }
    }
}

fn find_panel_by_id<'a>(panels: &'a [PanelDecl], id: &str) -> Option<&'a PanelDecl> {
    for panel in panels {
        if panel.id == id {
            return Some(panel);
        }
        if let Some(found) = find_panel_by_id_in_blocks(&panel.blocks, id) {
            return Some(found);
        }
    }
    None
}

fn find_panel_by_id_in_blocks<'a>(blocks: &'a [UiNodeDecl], id: &str) -> Option<&'a PanelDecl> {
    for node in blocks {
        if let UiNodeDecl::Panel(panel) = node {
            if panel.id == id {
                return Some(panel);
            }
            if let Some(found) = find_panel_by_id_in_blocks(&panel.blocks, id) {
                return Some(found);
            }
        }
    }
    None
}

fn collect_panels<'a>(panel: &'a PanelDecl, out: &mut Vec<&'a PanelDecl>) {
    out.push(panel);
    for node in &panel.blocks {
        if let UiNodeDecl::Panel(child) = node {
            collect_panels(child, out);
        }
    }
}

fn stamp_derived_on_tree(
    panels: &mut [PanelDecl],
    derived: &std::collections::HashMap<String, f64>,
) {
    for panel in panels.iter_mut() {
        stamp_derived_recursive(panel, derived);
    }
}

fn stamp_derived_recursive(
    panel: &mut PanelDecl,
    derived: &std::collections::HashMap<String, f64>,
) {
    if let Some(h) = derived.get(&panel.id) {
        stamp_section_derived(panel, *h);
    }
    for node in panel.blocks.iter_mut() {
        if let UiNodeDecl::Panel(child) = node {
            stamp_derived_recursive(child, derived);
        }
    }
}

fn collect_fill_section_ids(
    panels: &[PanelDecl],
) -> std::collections::HashSet<String> {
    let mut flat = Vec::new();
    for panel in panels {
        collect_panels(panel, &mut flat);
    }
    let panel_map: std::collections::HashMap<&str, &PanelDecl> =
        flat.iter().map(|p| (p.id.as_str(), *p)).collect();
    flat.iter()
        .filter(|p| ui_role(p) == Some("section"))
        .filter(|p| section_body_uses_fill(p, &panel_map))
        .map(|p| p.id.clone())
        .collect()
}

fn materialize_regions_on_tree(
    panels: &mut [PanelDecl],
    derived: &mut std::collections::HashMap<String, f64>,
    fill_section_ids: &std::collections::HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    for panel in panels.iter_mut() {
        materialize_regions_recursive(
            panel,
            derived,
            fill_section_ids,
            diagnostics,
            source_path,
        );
    }
}

fn materialize_regions_recursive(
    panel: &mut PanelDecl,
    derived: &mut std::collections::HashMap<String, f64>,
    fill_section_ids: &std::collections::HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if ui_role(panel) == Some("region") {
        materialize_region_fr_rows(
            panel,
            derived,
            fill_section_ids,
            diagnostics,
            source_path,
        );
    }
    for node in panel.blocks.iter_mut() {
        if let UiNodeDecl::Panel(child) = node {
            materialize_regions_recursive(
                child,
                derived,
                fill_section_ids,
                diagnostics,
                source_path,
            );
        }
    }
}

fn ui_role(panel: &PanelDecl) -> Option<&str> {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_ui_role"))
        .and_then(Value::as_str)
}

fn chrome_role(panel: &PanelDecl) -> Option<&str> {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_chrome_role"))
        .and_then(Value::as_str)
}

fn is_platform_placement(panel: &PanelDecl) -> bool {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_platform_placement"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn is_parent_fill_absolute(panel: &PanelDecl) -> bool {
    let Some(map) = panel.props.as_object() else {
        return false;
    };
    if map.get("position").and_then(Value::as_str) != Some("absolute") {
        return false;
    }
    if map.get("inset").and_then(Value::as_str) == Some("0") {
        return true;
    }
    let top = map.get("top").and_then(Value::as_str).unwrap_or("");
    let left = map.get("left").and_then(Value::as_str).unwrap_or("");
    top.is_empty()
        && left.is_empty()
        && map.get("width").and_then(Value::as_str) == Some("100%")
        && map.get("height").and_then(Value::as_str) == Some("100%")
}

fn is_forbidden_author_absolute(panel: &PanelDecl) -> bool {
    let Some(map) = panel.props.as_object() else {
        return false;
    };
    if map.get("position").and_then(Value::as_str) != Some("absolute") {
        return false;
    }
    if is_platform_placement(panel) || is_parent_fill_absolute(panel) {
        return false;
    }
    true
}

fn has_structural_children(panel: &PanelDecl) -> bool {
    panel.blocks.iter().any(|node| match node {
        UiNodeDecl::Panel(child) => matches!(ui_role(child), Some("region") | Some("section")),
        _ => false,
    })
}

fn is_author_section_height(panel: &PanelDecl) -> bool {
    let Some(map) = panel.props.as_object() else {
        return false;
    };
    if map
        .get("__mei_section_derived_height")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return false;
    }
    map.get("height")
        .and_then(Value::as_str)
        .is_some_and(|h| !h.trim().is_empty() && h != "auto" && h != "100%")
}

fn track_is_forbidden_region(track: &str) -> bool {
    let t = track.trim().to_ascii_lowercase();
    if t == "auto" || t.starts_with("minmax") {
        return true;
    }
    if t.ends_with("px") || t == "0" {
        return true;
    }
    if let Some(stripped) = t.strip_suffix("px") {
        return stripped.parse::<f64>().is_ok();
    }
    false
}

fn track_is_fr_only(track: &str) -> bool {
    if track_is_forbidden_region(track) {
        return false;
    }
    let t = track.trim();
    t.ends_with("fr") && t[..t.len().saturating_sub(2)].trim().parse::<f64>().is_ok()
}

fn push_error(
    diagnostics: &mut Vec<Diagnostic>,
    code: &str,
    message: String,
    source_path: &str,
) {
    diagnostics.push(Diagnostic {
        severity: Severity::Error,
        code: code.to_string(),
        message,
        source_path: Some(source_path.to_string()),
    });
}

fn panel_tier(panel: &PanelDecl) -> Option<String> {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_tier").or_else(|| m.get("tier")))
        .and_then(Value::as_str)
        .map(|tier| tier.to_ascii_lowercase())
}

fn is_t1_tier(tier: Option<&str>) -> bool {
    tier == Some("t1")
}

fn is_author_px_height(height: &str) -> bool {
    let t = height.trim();
    if t.is_empty() || t == "auto" || t == "100%" {
        return false;
    }
    if let Some(stripped) = t.strip_suffix("px") {
        return stripped.trim().parse::<f64>().is_ok();
    }
    false
}

fn validate_panel(
    panel: &PanelDecl,
    panel_map: &std::collections::HashMap<&str, &PanelDecl>,
    ctx: &ValidateContext,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
    options: &LayoutBudgetValidateOptions,
) {
    let role = ui_role(panel);
    let tier = panel_tier(panel).or_else(|| ctx.tier.clone());
    let in_fill_down = ctx.in_fill_down;

    if role == Some("section") && is_author_section_height(panel) {
        push_error(
            diagnostics,
            "layout_policy_section_height_forbidden",
            format!(
                "section `{}`: remove manual height; use section_shell + content_budget derivation",
                panel.id
            ),
            source_path,
        );
    }

    if role == Some("region") {
        let should_enforce_region_tracks =
            chrome_role(panel) == Some("rail") || has_structural_children(panel);
        if should_enforce_region_tracks {
            if panel
                .props
                .as_object()
                .and_then(|m| m.get("__mei_region_rows_materialized"))
                .and_then(Value::as_bool)
                != Some(true)
            {
                if let Some(rows) = panel.layout.as_ref().and_then(|l| l.rows.as_ref()) {
                    for row in rows {
                        if !track_is_fr_only(row) {
                            push_error(
                                diagnostics,
                                "layout_policy_region_px_track_forbidden",
                                format!(
                                    "region `{}`: row track `{row}` must be Nfr only (no px/minmax/auto)",
                                    panel.id
                                ),
                                source_path,
                            );
                        }
                    }
                }
            }
        }
    }

    if is_forbidden_author_absolute(panel) {
        push_error(
            diagnostics,
            "layout_policy_placement_absolute_forbidden",
            format!(
                "panel `{}`: author position:absolute is forbidden; use plane/region grid layout",
                panel.id
            ),
            source_path,
        );
    }

    if is_content_panel(panel) {
        if is_layout_fill_panel(panel) && content_budget_present(panel) {
            push_error(
                diagnostics,
                "layout_policy_content_budget_px_forbidden",
                format!(
                    "content panel `{}`: __mei_layout_fill and __mei_content_budget are mutually exclusive (0327 fill-down)",
                    panel.id
                ),
                source_path,
            );
        } else if content_budget_present(panel)
            && !is_layout_fill_panel(panel)
            && options.strict_t1_fill_down
            && is_t1_tier(tier.as_deref())
        {
            push_error(
                diagnostics,
                "layout_policy_content_budget_px_forbidden",
                format!(
                    "content panel `{}`: T1 fill-down forbids __mei_content_budget px rows (use content_fill_props)",
                    panel.id
                ),
                source_path,
            );
        }
        if content_budget_missing(panel, tier.as_deref()) {
            push_error(
                diagnostics,
                "layout_policy_content_budget_missing",
                format!(
                    "content panel `{}`: missing __mei_content_budget (use content_strip or semantic macro) or __mei_layout_fill (use content_fill_props)",
                    panel.id
                ),
                source_path,
            );
        }
        if let Some(rows) = panel.layout.as_ref().and_then(|l| l.rows.as_ref()) {
            for row in rows {
                if row.trim().eq_ignore_ascii_case("auto") {
                    push_error(
                        diagnostics,
                        "layout_policy_content_auto_row_forbidden",
                        format!(
                            "content panel `{}`: layout.rows must not use auto (use 1fr + row_budgets or fill-down)",
                            panel.id
                        ),
                        source_path,
                    );
                }
            }
        }
    }

    if role == Some("section") {
        validate_section_content_link(panel, panel_map, diagnostics, source_path);
        if is_t1_tier(tier.as_deref()) && options.strict_t1_fill_down {
            if let Some(body) = find_body_content_panel(panel, panel_map) {
                if !is_layout_fill_panel(body) {
                    push_error(
                        diagnostics,
                        "layout_policy_body_not_fill",
                        format!(
                            "section `{}`: T1 body must use __mei_layout_fill (content_fill_props)",
                            panel.id
                        ),
                        source_path,
                    );
                }
            }
        }
    }

    validate_duplicate_dimension(panel, diagnostics, source_path);
    validate_slot_background(panel, diagnostics, source_path);
    validate_slot_height_px(panel, in_fill_down, diagnostics, source_path);

    for node in &panel.blocks {
        walk_nodes_validate(node, ctx, diagnostics, source_path);
    }
}

fn is_content_panel(panel: &PanelDecl) -> bool {
    if is_layout_fill_panel(panel) {
        return true;
    }
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_content_budget"))
        .is_some()
        || panel.id.contains("-stats")
        || panel.id.contains("indicator-system")
        || panel.id.contains("realtime-table")
        || panel.id.contains("typical-cases")
}

fn is_layout_fill_panel(panel: &PanelDecl) -> bool {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_layout_fill"))
        .and_then(Value::as_bool)
        == Some(true)
}

fn content_budget_present(panel: &PanelDecl) -> bool {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_content_budget"))
        .is_some()
}

fn section_body_uses_fill(
    section: &PanelDecl,
    panel_map: &std::collections::HashMap<&str, &PanelDecl>,
) -> bool {
    find_body_content_panel(section, panel_map)
        .map(is_layout_fill_panel)
        .unwrap_or(false)
}

fn content_budget_missing(panel: &PanelDecl, tier: Option<&str>) -> bool {
    if is_layout_fill_panel(panel) {
        return false;
    }
    if is_t1_tier(tier) {
        return false;
    }
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_content_budget"))
        .is_none()
}

fn content_budget_sum(panel: &PanelDecl) -> Option<f64> {
    let budget = panel.props.as_object()?.get("__mei_content_budget")?;
    let rows = budget.get("rows")?.as_array()?;
    let mut sum = 0.0;
    for row in rows {
        let px = row.as_f64().or_else(|| row.as_i64().map(|n| n as f64))?;
        sum += px;
    }
    let gap = budget
        .get("gap")
        .and_then(Value::as_str)
        .and_then(parse_px_str)
        .unwrap_or(0.0);
    let n = rows.len();
    if n > 1 {
        sum += gap * (n as f64 - 1.0);
    }
    Some(sum)
}

fn parse_px_str(s: &str) -> Option<f64> {
    let t = s.trim();
    if t == "0" {
        return Some(0.0);
    }
    t.strip_suffix("px")?.parse().ok()
}

fn validate_section_content_link(
    section: &PanelDecl,
    panel_map: &std::collections::HashMap<&str, &PanelDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(body_panel) = find_body_content_panel(section, panel_map) else {
        return;
    };
    if is_layout_fill_panel(body_panel) {
        return;
    }
    let Some(content_sum) = content_budget_sum(body_panel) else {
        return;
    };
    let profile = section
        .props
        .as_object()
        .and_then(|m| m.get("__mei_padding_profile"))
        .and_then(Value::as_str)
        .unwrap_or("dense");
    let (pad_top, pad_bottom) = padding_profile_vertical_px(profile).unwrap_or((8.0, 4.0));
    let derived = TITLE_BAR_HEIGHT_PX + pad_top + pad_bottom + content_sum;

    if let Some(viewport_h) = section_viewport_inner_height(section) {
        if derived > viewport_h + 0.5 {
            push_error(
                diagnostics,
                "layout_policy_budget_overflow",
                format!(
                    "section `{}`: derived height {derived}px exceeds region viewport inner {viewport_h}px",
                    section.id
                ),
                source_path,
            );
        }
    }
}

fn section_viewport_inner_height(panel: &PanelDecl) -> Option<f64> {
    let map = panel.props.as_object()?;
    let viewport = map.get("viewport")?.as_object()?;
    let design_h = viewport
        .get("design_height")
        .and_then(Value::as_f64)
        .or_else(|| {
            viewport
                .get("design_height")
                .and_then(Value::as_u64)
                .map(|n| n as f64)
        })?;
    Some(design_h)
}

fn find_body_content_panel<'a>(
    section: &'a PanelDecl,
    panel_map: &std::collections::HashMap<&str, &'a PanelDecl>,
) -> Option<&'a PanelDecl> {
    for node in &section.blocks {
        if let UiNodeDecl::Panel(p) = node {
            if is_content_panel(p) {
                return Some(p);
            }
            if let Some(found) = panel_map.get(p.id.as_str()) {
                if is_content_panel(found) {
                    return Some(found);
                }
            }
            if let Some(nested) = find_body_content_panel(p, panel_map) {
                return Some(nested);
            }
        }
        if let UiNodeDecl::Block(block) = node {
            if let Some(id) = block.id.as_ref() {
                if let Some(found) = panel_map.get(id.as_str()) {
                    if is_content_panel(found) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn validate_slot_height_px(
    panel: &PanelDecl,
    in_fill_down: bool,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if !in_fill_down {
        return;
    }
    let Some(map) = panel.props.as_object() else {
        return;
    };
    if map
        .get("__mei_slot_frame_bg")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return;
    }
    if let Some(height) = map.get("height").and_then(Value::as_str) {
        if is_author_px_height(height) {
            push_error(
                diagnostics,
                "layout_policy_slot_height_px_forbidden",
                format!(
                    "panel `{}`: slot shell_height must be 100% under fill-down, not `{height}`",
                    panel.id
                ),
                source_path,
            );
        }
    }
}

fn walk_nodes_validate(
    node: &UiNodeDecl,
    ctx: &ValidateContext,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    match node {
        UiNodeDecl::Panel(p) => {
            validate_slot_background(p, diagnostics, source_path);
            validate_slot_height_px(p, ctx.in_fill_down, diagnostics, source_path);
            if ctx.in_fill_down {
                validate_inline_font_in_props(&p.props, &p.id, diagnostics, source_path);
            }
            for child in &p.blocks {
                walk_nodes_validate(child, ctx, diagnostics, source_path);
            }
        }
        UiNodeDecl::Block(block) => {
            if ctx.in_fill_down {
                validate_inline_font_in_props(
                    &block.props,
                    block.id.as_deref().unwrap_or("block"),
                    diagnostics,
                    source_path,
                );
            }
        }
        UiNodeDecl::PanelRefEmbed(_) => {}
    }
}
fn validate_inline_font_in_props(
    props: &Value,
    panel_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(map) = props.as_object() else {
        return;
    };
    if let Some(raw) = map.get("font_size").and_then(Value::as_str) {
        if is_literal_font_size(raw) {
            push_error(
                diagnostics,
                "layout_policy_inline_font_forbidden",
                format!(
                    "panel/block `{panel_id}`: literal font_size `{raw}` forbidden (use font token tier)",
                ),
                source_path,
            );
        }
    }
}

fn validate_duplicate_dimension(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(map) = panel.props.as_object() else {
        return;
    };
    let Some(conflicts) = map
        .get("__mei_placement_dimension_conflicts")
        .and_then(Value::as_array)
    else {
        return;
    };
    for key in conflicts {
        let Some(dim) = key.as_str() else {
            continue;
        };
        push_error(
            diagnostics,
            "layout_policy_duplicate_dimension",
            format!(
                "panel `{}`: placement and shell both set conflicting `{dim}`",
                panel.id
            ),
            source_path,
        );
    }
}

fn validate_slot_background(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(map) = panel.props.as_object() else {
        return;
    };
    if map
        .get("__mei_slot_frame_bg")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return;
    }
    let bg = map.get("background").and_then(Value::as_object);
    let size_ok = bg
        .and_then(|b| b.get("size"))
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim() == "100% 100%");
    let origin_ok = bg
        .and_then(|b| b.get("origin"))
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim() == "border-box");
    let clip_ok = bg
        .and_then(|b| b.get("clip"))
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim() == "border-box");
    if size_ok && origin_ok && clip_ok {
        return;
    }
    let mut missing = Vec::new();
    if !size_ok {
        missing.push("background.size: 100% 100%");
    }
    if !origin_ok {
        missing.push("background.origin: border-box");
    }
    if !clip_ok {
        missing.push("background.clip: border-box");
    }
    push_error(
        diagnostics,
        "layout_policy_slot_background_incomplete",
        format!(
            "panel `{}`: slot chrome missing {}",
            panel.id,
            missing.join(", ")
        ),
        source_path,
    );
}

fn validate_region_overflow(
    panel: &PanelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    if ui_role(panel) != Some("region") {
        return;
    }
    let Some(viewport_h) = section_viewport_inner_height(panel) else {
        return;
    };
    let gap_px = panel
        .layout
        .as_ref()
        .and_then(|l| l.gap.as_deref())
        .and_then(parse_px_str)
        .unwrap_or(0.0);

    let section_panels: Vec<&PanelDecl> = panel
        .blocks
        .iter()
        .filter_map(|n| match n {
            UiNodeDecl::Panel(p) => Some(p),
            _ => None,
        })
        .collect();

    let mut sum = 0.0;
    for sec in &section_panels {
        if let Some(h) = section_derived_height_px(sec) {
            sum += h;
        }
    }
    if section_panels.len() > 1 {
        sum += gap_px * (section_panels.len() as f64 - 1.0);
    }
    if sum > viewport_h + 0.5 {
        push_error(
            diagnostics,
            "layout_policy_region_overflow",
            format!(
                "region `{}`: sections total {sum}px exceeds viewport inner {viewport_h}px",
                panel.id
            ),
            source_path,
        );
    }
}

fn section_derived_height_px(panel: &PanelDecl) -> Option<f64> {
    panel
        .props
        .as_object()
        .and_then(|m| m.get("__mei_section_derived_height_px"))
        .and_then(Value::as_f64)
}

fn compute_section_derived_height(
    section: &PanelDecl,
    panel_map: &std::collections::HashMap<&str, &PanelDecl>,
) -> Option<f64> {
    let body = find_body_content_panel(section, panel_map)?;
    let content_sum = content_budget_sum(body)?;
    let profile = section
        .props
        .as_object()
        .and_then(|m| m.get("__mei_padding_profile"))
        .and_then(Value::as_str)
        .unwrap_or("dense");
    let (pad_top, pad_bottom) = padding_profile_vertical_px(profile).unwrap_or((8.0, 4.0));
    Some(TITLE_BAR_HEIGHT_PX + pad_top + pad_bottom + content_sum)
}

fn stamp_section_derived(panel: &mut PanelDecl, height_px: f64) {
    let height_str = format!("{}px", height_px.round() as i64);
    if let Some(map) = panel.props.as_object_mut() {
        map.insert(
            "__mei_section_derived_height".to_string(),
            Value::Bool(true),
        );
        map.insert(
            "__mei_section_derived_height_px".to_string(),
            serde_json::json!(height_px),
        );
        map.insert("height".to_string(), Value::String(height_str));
    }
}

fn parse_fr_weight(track: &str) -> Option<f64> {
    let t = track.trim();
    if !t.ends_with("fr") {
        return None;
    }
    t[..t.len().saturating_sub(2)].trim().parse().ok()
}

fn materialize_region_fr_rows(
    region: &mut PanelDecl,
    derived: &mut std::collections::HashMap<String, f64>,
    fill_section_ids: &std::collections::HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(viewport_h) = section_viewport_inner_height(region) else {
        return;
    };
    let section_ids: Vec<String> = region
        .blocks
        .iter()
        .filter_map(|n| match n {
            UiNodeDecl::Panel(p) => Some(p.id.clone()),
            _ => None,
        })
        .collect();
    let Some(layout) = region.layout.as_mut() else {
        return;
    };
    let Some(rows) = layout.rows.as_ref() else {
        return;
    };
    if section_ids.len() != rows.len() {
        return;
    }
    let fr_weights: Vec<f64> = rows.iter().filter_map(|r| parse_fr_weight(r)).collect();
    if fr_weights.len() != rows.len() {
        return;
    }
    let gap_px = layout
        .gap
        .as_deref()
        .and_then(parse_px_str)
        .unwrap_or(0.0);
    let gap_total = if section_ids.len() > 1 {
        gap_px * (section_ids.len() as f64 - 1.0)
    } else {
        0.0
    };
    let inner = (viewport_h - gap_total).max(0.0);
    let fr_sum: f64 = fr_weights.iter().sum();
    if fr_sum <= 0.0 {
        return;
    }
    let mut px_rows = Vec::new();
    for (i, sid) in section_ids.iter().enumerate() {
        let row_px = inner * fr_weights[i] / fr_sum;
        px_rows.push(format!("{}px", row_px.round() as i64));
        if fill_section_ids.contains(sid) {
            derived.insert(sid.clone(), row_px);
            continue;
        }
        if let Some(sec_h) = derived.get(sid) {
            if *sec_h > row_px + 0.5 {
                push_error(
                    diagnostics,
                    "layout_policy_budget_overflow",
                    format!(
                        "region `{}` section `{sid}`: derived {sec_h}px exceeds fr row allocation {row_px}px",
                        region.id
                    ),
                    source_path,
                );
            }
        }
    }
    layout.rows = Some(px_rows);
    if let Some(map) = region.props.as_object_mut() {
        map.insert(
            "__mei_region_rows_materialized".to_string(),
            Value::Bool(true),
        );
    }
}
