use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::{
    BlockDecl, BuildNodeId, LayoutDecl, UiBudgetSummary, UiNodeDecl, UiScopeNode, UiScopeRole,
    UiSourceAnchor, UiTreeNode,
};

pub struct UiStructureBuildResult {
    pub nodes: BTreeMap<String, UiScopeNode>,
    pub duplicate_node_ids: Vec<String>,
}

struct Builder<'a> {
    scene_id: &'a str,
    _scene_label: &'a str,
    _app_id: &'a str,
    nodes: BTreeMap<String, UiScopeNode>,
    duplicate_node_ids: Vec<String>,
}

impl<'a> Builder<'a> {
    fn finish(self) -> UiStructureBuildResult {
        UiStructureBuildResult {
            nodes: self.nodes,
            duplicate_node_ids: self.duplicate_node_ids,
        }
    }

    fn insert_node(&mut self, node: UiScopeNode) -> String {
        let id = node.node_id.clone();
        if self.nodes.contains_key(&id) && !self.duplicate_node_ids.contains(&id) {
            self.duplicate_node_ids.push(id.clone());
        }
        self.nodes.insert(id.clone(), node);
        id
    }

    fn link_child(&mut self, parent_id: &str, child_id: &str) {
        if let Some(parent) = self.nodes.get_mut(parent_id) {
            if !parent.children.contains(&child_id.to_string()) {
                parent.children.push(child_id.to_string());
            }
        }
    }

    fn make_node(
        &self,
        role: UiScopeRole,
        label: String,
        scope_segments: &[String],
        preview_scope: String,
        parent_id: Option<String>,
        plane: Option<String>,
        budget: Option<UiBudgetSummary>,
        source_anchors: Vec<UiSourceAnchor>,
        content_kind: Option<String>,
    ) -> UiScopeNode {
        let key = scope_segments.join("/");
        let node_id = BuildNodeId::ui_scope(self.scene_id, key).encode();
        UiScopeNode {
            node_id,
            role,
            label,
            scope_path: scope_segments.to_vec(),
            plane,
            parent_id,
            children: Vec::new(),
            preview_scope,
            budget,
            source_anchors,
            content_kind,
            scene_id: Some(self.scene_id.to_string()),
        }
    }
}

pub fn build_scene_ui_structure(
    scene_id: &str,
    scene_label: &str,
    panels: &[UiNodeDecl],
    app_id: &str,
) -> UiStructureBuildResult {
    let mut builder = Builder {
        scene_id,
        _scene_label: scene_label,
        _app_id: app_id,
        nodes: BTreeMap::new(),
        duplicate_node_ids: Vec::new(),
    };

    let scene_segments = vec![scene_id.to_string()];
    let scene_node = builder.make_node(
        UiScopeRole::Scene,
        scene_label.to_string(),
        &scene_segments,
        String::new(),
        None,
        None,
        None,
        Vec::new(),
        None,
    );
    let scene_id_encoded = builder.insert_node(scene_node);

    let mut planes: BTreeMap<String, Vec<&UiNodeDecl>> = BTreeMap::new();
    for panel in panels {
        // 0335: index by plane_id when authored as plane; do not collapse all tier=t2 into one node.
        let key = panel
            .props
            .get("__mei_plane_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| panel_tier(panel));
        planes.entry(key).or_default().push(panel);
    }

    for (plane_key, top_panels) in planes {
        let authored_plane = top_panels
            .iter()
            .find(|panel| ui_role_from_props(&panel.props) == Some("plane"))
            .copied();
        let tier = authored_plane
            .and_then(|p| p.props.get("__mei_tier").and_then(|v| v.as_str()))
            .unwrap_or(plane_key.as_str())
            .to_string();
        let plane_label = authored_plane
            .map(region_label)
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| {
                if tier == "t2" && plane_key != "t2" {
                    format!("T2 · {plane_key}")
                } else {
                    plane_label_for_tier(tier.as_str())
                }
            });
        let plane_segments = vec![scene_id.to_string(), plane_key.clone()];
        let plane_budget = authored_plane.and_then(budget_from_panel);
        let plane_anchors = authored_plane
            .map(source_anchor_for_panel)
            .unwrap_or_default();
        // Plane preview_scope = plane_key（如 t1），供 layout_budget / DOM data-preview-scope 对齐。
        let plane_preview_scope = plane_key.trim().trim_matches('/').to_ascii_lowercase();
        let plane_node = builder.make_node(
            UiScopeRole::Plane,
            plane_label,
            &plane_segments,
            plane_preview_scope,
            Some(scene_id_encoded.clone()),
            Some(tier.clone()),
            plane_budget,
            plane_anchors,
            None,
        );
        let plane_id = builder.insert_node(plane_node);
        builder.link_child(&scene_id_encoded, &plane_id);

        let child_panels: Vec<&UiNodeDecl> = if let Some(plane) = authored_plane {
            plane
                .blocks
                .iter()
                .filter_map(|node| match node {
                    UiTreeNode::Panel(panel) => Some(panel),
                    _ => None,
                })
                .collect()
        } else {
            top_panels
                .iter()
                .copied()
                .filter(|panel| ui_role_from_props(&panel.props) != Some("plane"))
                .collect()
        };

        for child in child_panels {
            match ui_role_from_props(&child.props) {
                Some("slide") => walk_slide(&mut builder, child, tier.as_str(), &plane_id),
                _ => walk_region(&mut builder, child, tier.as_str(), &plane_id),
            }
        }
    }

    builder.finish()
}

fn tier_scoped_preview_scope(tier: &str, logical_path: &str) -> String {
    let tier_slug = tier.trim().trim_matches('/').to_ascii_lowercase();
    let path = logical_path.trim().trim_matches('/');
    if path.is_empty() {
        return tier_slug.to_string();
    }
    if tier_slug.is_empty() {
        return path.to_string();
    }
    if path.starts_with(&format!("{tier_slug}/")) {
        return path.to_string();
    }
    format!("{tier_slug}/{path}")
}

fn walk_region(builder: &mut Builder<'_>, region: &UiNodeDecl, tier: &str, plane_id: &str) {
    let region_id = region.id.clone();
    let region_label = region_label(region);
    let region_segments = vec![
        builder.scene_id.to_string(),
        tier.to_string(),
        region_id.clone(),
    ];
    let preview_scope = tier_scoped_preview_scope(tier, region_id.as_str());
    let region_budget = budget_from_panel(region);
    let region_node = builder.make_node(
        UiScopeRole::Region,
        region_label,
        &region_segments,
        preview_scope.clone(),
        Some(plane_id.to_string()),
        Some(tier.to_string()),
        region_budget,
        source_anchor_for_panel(region),
        None,
    );
    let region_node_id = builder.insert_node(region_node);
    builder.link_child(plane_id, &region_node_id);

    let sections = sections_in_region(region);
    if sections.is_empty() {
        walk_default_section(
            builder,
            region,
            tier,
            &region_node_id,
            &region_segments,
            preview_scope.as_str(),
        );
        return;
    }

    for (section_key, section_panel) in sections {
        if panel_is_nested_region(&section_panel) {
            walk_nested_region_under_region(
                builder,
                &section_panel,
                tier,
                &region_node_id,
                &region_segments,
                preview_scope.as_str(),
            );
            continue;
        }
        walk_section(
            builder,
            &section_panel,
            tier,
            &region_node_id,
            &region_segments,
            preview_scope.as_str(),
            section_key.as_str(),
        );
    }
}

fn walk_slide(builder: &mut Builder<'_>, slide: &UiNodeDecl, tier: &str, plane_id: &str) {
    let slide_id = slide.id.clone();
    let slide_label = slide
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| slide_id.clone());
    let slide_segments = vec![
        builder.scene_id.to_string(),
        tier.to_string(),
        slide_id.clone(),
    ];
    let preview_scope = tier_scoped_preview_scope(tier, slide_id.as_str());
    let slide_budget = budget_from_panel(slide);
    let slide_node = builder.make_node(
        UiScopeRole::Slide,
        slide_label,
        &slide_segments,
        preview_scope.clone(),
        Some(plane_id.to_string()),
        Some(tier.to_string()),
        slide_budget,
        source_anchor_for_panel(slide),
        None,
    );
    let slide_node_id = builder.insert_node(slide_node);
    builder.link_child(plane_id, &slide_node_id);

    let regions: Vec<UiNodeDecl> = slide
        .blocks
        .iter()
        .filter_map(|node| match node {
            UiTreeNode::Panel(panel)
                if ui_role_from_props(&panel.props) == Some("region")
                    || panel_is_nested_region(panel) =>
            {
                Some(panel.clone())
            }
            _ => None,
        })
        .collect();
    if regions.is_empty() {
        walk_default_section(
            builder,
            slide,
            tier,
            &slide_node_id,
            &slide_segments,
            preview_scope.as_str(),
        );
        return;
    }
    for region_panel in regions {
        walk_nested_region_under_region(
            builder,
            &region_panel,
            tier,
            &slide_node_id,
            &slide_segments,
            preview_scope.as_str(),
        );
    }
}

fn panel_is_nested_region(panel: &UiNodeDecl) -> bool {
    // `chrome_role = "header"` rewrites `__mei_ui_role` to "header" for z-index, but the
    // panel remains a region container (sections + screen_header brand live underneath).
    // Presentation slides are walked via `walk_slide`, not as nested regions.
    matches!(
        ui_role_from_props(&panel.props),
        Some("region") | Some("header") | Some("float_dock")
    )
}

fn walk_nested_region_under_region(
    builder: &mut Builder<'_>,
    nested_region: &UiNodeDecl,
    tier: &str,
    parent_region_node_id: &str,
    parent_region_segments: &[String],
    _parent_preview_prefix: &str,
) {
    let nested_id = nested_region.id.clone();
    let nested_label = region_label(nested_region);
    let mut nested_segments = parent_region_segments.to_vec();
    nested_segments.push(nested_id.clone());
    let nested_preview_scope = tier_scoped_preview_scope(tier, nested_id.as_str());
    let nested_budget = budget_from_panel(nested_region);
    let nested_node = builder.make_node(
        UiScopeRole::Region,
        nested_label,
        &nested_segments,
        nested_preview_scope.clone(),
        Some(parent_region_node_id.to_string()),
        Some(tier.to_string()),
        nested_budget,
        source_anchor_for_panel(nested_region),
        None,
    );
    let nested_node_id = builder.insert_node(nested_node);
    builder.link_child(parent_region_node_id, &nested_node_id);

    let subsections = sections_in_region(nested_region);
    let nested_regions: Vec<UiNodeDecl> = nested_region
        .blocks
        .iter()
        .filter_map(|node| match node {
            UiTreeNode::Panel(panel)
                if ui_role_from_props(&panel.props) == Some("region")
                    && !panel_is_section(panel) =>
            {
                Some(panel.clone())
            }
            _ => None,
        })
        .collect();
    if !nested_regions.is_empty() {
        for region_panel in nested_regions {
            walk_nested_region_under_region(
                builder,
                &region_panel,
                tier,
                &nested_node_id,
                &nested_segments,
                nested_preview_scope.as_str(),
            );
        }
        return;
    }
    if subsections.is_empty() {
        walk_default_section(
            builder,
            nested_region,
            tier,
            &nested_node_id,
            &nested_segments,
            nested_preview_scope.as_str(),
        );
        return;
    }
    for (subsection_key, subsection_panel) in subsections {
        walk_section(
            builder,
            &subsection_panel,
            tier,
            &nested_node_id,
            &nested_segments,
            nested_preview_scope.as_str(),
            subsection_key.as_str(),
        );
    }
}

const DEFAULT_SECTION_KEY: &str = "_default";

fn default_section_label(region: &UiNodeDecl) -> String {
    region
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("主体")
        .to_string()
}

fn walk_default_section(
    builder: &mut Builder<'_>,
    region: &UiNodeDecl,
    tier: &str,
    region_node_id: &str,
    region_segments: &[String],
    region_id: &str,
) {
    let section_key = DEFAULT_SECTION_KEY;
    let section_label = default_section_label(region);
    let mut section_segments = region_segments.to_vec();
    section_segments.push(section_key.to_string());
    let preview_scope = format!("{region_id}/{section_key}");
    let section_node = builder.make_node(
        UiScopeRole::Section,
        section_label,
        &section_segments,
        preview_scope.clone(),
        Some(region_node_id.to_string()),
        Some(tier.to_string()),
        None,
        source_anchor_for_panel(region),
        None,
    );
    let section_node_id = builder.insert_node(section_node);
    builder.link_child(region_node_id, &section_node_id);
    walk_section_body(
        builder,
        region,
        tier,
        &section_node_id,
        &section_segments,
        preview_scope.as_str(),
    );
}

fn walk_section(
    builder: &mut Builder<'_>,
    section: &UiNodeDecl,
    tier: &str,
    region_node_id: &str,
    region_segments: &[String],
    region_id: &str,
    section_key: &str,
) {
    let section_label = section
        .title
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| section_key.to_string());
    let mut section_segments = region_segments.to_vec();
    section_segments.push(section_key.to_string());
    let preview_scope = format!("{region_id}/{section_key}");
    let section_budget = merged_section_budget(section);
    let section_node = builder.make_node(
        UiScopeRole::Section,
        section_label,
        &section_segments,
        preview_scope.clone(),
        Some(region_node_id.to_string()),
        Some(tier.to_string()),
        section_budget,
        source_anchor_for_panel(section),
        None,
    );
    let section_node_id = builder.insert_node(section_node);
    builder.link_child(region_node_id, &section_node_id);
    walk_section_body(
        builder,
        section,
        tier,
        &section_node_id,
        &section_segments,
        preview_scope.as_str(),
    );
}

fn walk_section_body(
    builder: &mut Builder<'_>,
    panel: &UiNodeDecl,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
) {
    let file_hint = panel.import_scope.as_deref().filter(|v| !v.is_empty());
    for layout_panel in slotted_layout_panels_in_deep(panel) {
        walk_slotted_layout(
            builder,
            layout_panel,
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
        );
    }
    for layout_panel in layout_content_group_panels_in_deep(panel) {
        if let Some(kind) = content_group_kind(layout_panel) {
            walk_layout_content_group(
                builder,
                layout_panel,
                tier,
                parent_id,
                parent_segments,
                preview_prefix,
                kind,
            );
        }
    }
    for metric_panel in metric_card_panels_exclusive(panel) {
        walk_content_panel(
            builder,
            metric_panel,
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
            None,
        );
    }
    for (block, content_label) in content_blocks_in(panel) {
        walk_content_block(
            builder,
            block,
            content_label.as_str(),
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
            file_hint,
        );
    }
    walk_contract_level_content_in_panel(
        builder,
        panel,
        tier,
        parent_id,
        parent_segments,
        preview_prefix,
        file_hint,
    );
    for chrome_panel in viewport_chrome_panels_in_deep(panel) {
        let label = chrome_panel
            .props
            .get("__mei_chrome_role")
            .and_then(|v| v.as_str())
            .map(|role| format!("viewport:{role}"))
            .unwrap_or_else(|| chrome_panel.id.clone());
        walk_content_panel(
            builder,
            chrome_panel,
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
            Some(label.as_str()),
        );
    }
}

fn walk_slotted_layout(
    builder: &mut Builder<'_>,
    layout_panel: &UiNodeDecl,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
) {
    let layout_key = slotted_layout_key(layout_panel);
    let layout_label = slotted_layout_label(layout_panel);
    let mut layout_segments = parent_segments.to_vec();
    layout_segments.push(layout_key.clone());
    let preview_scope = format!("{preview_prefix}/{layout_key}");
    let budget = budget_from_panel(layout_panel);
    let layout_node = builder.make_node(
        UiScopeRole::Slot,
        layout_label,
        &layout_segments,
        preview_scope.clone(),
        Some(parent_id.to_string()),
        Some(tier.to_string()),
        budget.clone(),
        source_anchor_for_panel(layout_panel),
        layout_macro_hint(layout_panel).map(str::to_string),
    );
    let layout_node_id = builder.insert_node(layout_node);
    builder.link_child(parent_id, &layout_node_id);

    if let Some(budget) = budget {
        if !budget_is_empty(&budget) {
            let budget_label = budget_label_from_summary(&budget);
            let mut budget_segments = layout_segments.clone();
            budget_segments.push("budget".to_string());
            let budget_preview = format!("{preview_scope}/budget");
            let budget_node = builder.make_node(
                UiScopeRole::Budget,
                budget_label,
                &budget_segments,
                budget_preview,
                Some(layout_node_id.clone()),
                Some(tier.to_string()),
                Some(budget),
                Vec::new(),
                None,
            );
            let budget_id = builder.insert_node(budget_node);
            builder.link_child(&layout_node_id, &budget_id);
        }
    }

    for slot in slot_nodes_in_layout(layout_panel) {
        walk_slot(
            builder,
            &slot,
            tier,
            &layout_node_id,
            &layout_segments,
            preview_scope.as_str(),
        );
    }
}

fn walk_slot(
    builder: &mut Builder<'_>,
    slot: &SlotWalkItem,
    tier: &str,
    parent_node_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
) {
    let slot_key = slot.area.clone();
    let slot_label = slot
        .nested_panel
        .as_ref()
        .filter(|panel| is_slot_shell_panel(panel))
        .map(|panel| panel.id.clone())
        .unwrap_or_else(|| slot.label.clone());
    let mut slot_segments = parent_segments.to_vec();
    slot_segments.push(slot_key.clone());
    let preview_scope = format!("{preview_prefix}/{slot_key}");
    let slot_node = builder.make_node(
        UiScopeRole::Slot,
        slot_label,
        &slot_segments,
        preview_scope.clone(),
        Some(parent_node_id.to_string()),
        Some(tier.to_string()),
        None,
        Vec::new(),
        None,
    );
    let slot_node_id = builder.insert_node(slot_node);
    builder.link_child(parent_node_id, &slot_node_id);

    if let Some(panel) = &slot.nested_panel {
        if let Some(kind) = content_group_kind(panel) {
            walk_layout_content_group(
                builder,
                panel,
                tier,
                &slot_node_id,
                &slot_segments,
                preview_scope.as_str(),
                kind,
            );
        } else if is_metric_card_panel(panel) {
            walk_content_panel(
                builder,
                panel,
                tier,
                &slot_node_id,
                &slot_segments,
                preview_scope.as_str(),
                Some(slot.label.as_str()),
            );
        } else if is_slot_shell_panel(panel) {
            let mut walked_compound_group = false;
            for child in child_panels(panel) {
                if let Some(kind) = content_group_kind(child) {
                    walk_layout_content_group(
                        builder,
                        child,
                        tier,
                        &slot_node_id,
                        &slot_segments,
                        preview_scope.as_str(),
                        kind,
                    );
                    walked_compound_group = true;
                }
            }
            if walked_compound_group {
                return;
            }
            for group in layout_content_group_panels_in_deep(panel) {
                if let Some(kind) = content_group_kind(group) {
                    walk_layout_content_group(
                        builder,
                        group,
                        tier,
                        &slot_node_id,
                        &slot_segments,
                        preview_scope.as_str(),
                        kind,
                    );
                    walked_compound_group = true;
                }
            }
            if walked_compound_group {
                return;
            }
            for metric_panel in metric_card_panels_exclusive(panel) {
                walk_content_panel(
                    builder,
                    metric_panel,
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                    None,
                );
            }
            let file_hint = panel.import_scope.as_deref().filter(|v| !v.is_empty());
            for (block, content_label) in content_blocks_in(panel) {
                walk_content_block(
                    builder,
                    block,
                    content_label.as_str(),
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                    file_hint,
                );
            }
            // Single-content slot shells place leaf blocks in the `content` area.
            for (block, content_label) in contract_level_content_blocks(panel) {
                walk_content_block(
                    builder,
                    block,
                    content_label.as_str(),
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                    file_hint,
                );
            }
            if slot.area == "chart" {
                for (block, content_label) in chart_blocks_in_deep(panel) {
                    walk_content_block(
                        builder,
                        block,
                        content_label.as_str(),
                        tier,
                        &slot_node_id,
                        &slot_segments,
                        preview_scope.as_str(),
                        file_hint,
                    );
                }
            }
        } else if is_slotted_layout_panel(panel) {
            walk_slotted_layout(
                builder,
                panel,
                tier,
                &slot_node_id,
                &slot_segments,
                preview_scope.as_str(),
            );
        } else {
            let file_hint = panel.import_scope.as_deref().filter(|v| !v.is_empty());
            for layout_panel in slotted_layout_panels_in_deep(panel) {
                walk_slotted_layout(
                    builder,
                    layout_panel,
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                );
            }
            for group in layout_content_group_panels_in_deep(panel) {
                if let Some(kind) = content_group_kind(group) {
                    walk_layout_content_group(
                        builder,
                        group,
                        tier,
                        &slot_node_id,
                        &slot_segments,
                        preview_scope.as_str(),
                        kind,
                    );
                }
            }
            for metric_panel in metric_card_panels_exclusive(panel) {
                walk_content_panel(
                    builder,
                    metric_panel,
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                    None,
                );
            }
            for (block, content_label) in content_blocks_in_deep(panel) {
                walk_content_block(
                    builder,
                    block,
                    content_label.as_str(),
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                    file_hint,
                );
            }
            for (block, content_label) in contract_level_content_blocks(panel) {
                walk_content_block(
                    builder,
                    block,
                    content_label.as_str(),
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                    file_hint,
                );
            }
            if slot.area == "chart" {
                for (block, content_label) in chart_blocks_in_deep(panel) {
                    walk_content_block(
                        builder,
                        block,
                        content_label.as_str(),
                        tier,
                        &slot_node_id,
                        &slot_segments,
                        preview_scope.as_str(),
                        file_hint,
                    );
                }
            }
        }
    }
    if let Some(block) = &slot.block {
        walk_content_block(
            builder,
            block,
            slot.label.as_str(),
            tier,
            &slot_node_id,
            &slot_segments,
            preview_scope.as_str(),
            None,
        );
    }
}

/// Join slot prefix with optional block/panel area and content key without duplicating area.
/// Grid cell names like `content` are layout plumbing, not logical preview scopes.
fn join_content_preview_scope(
    preview_prefix: &str,
    area: Option<&str>,
    content_key: &str,
) -> String {
    let prefix = preview_prefix.trim().trim_end_matches('/');
    let area = area
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto" && *value != "content");
    match area {
        Some(area) if prefix.ends_with(&format!("/{area}")) || prefix == area => {
            format!("{prefix}/{content_key}")
        }
        Some(area) => format!("{prefix}/{area}/{content_key}"),
        None => format!("{prefix}/{content_key}"),
    }
}

/// Build stable node scope keys aligned with preview_scope (scene/tier + logical path).
fn scope_segments_from_preview(scene_id: &str, tier: &str, preview_scope: &str) -> Vec<String> {
    let mut segments = vec![scene_id.to_string(), tier.to_string()];
    let prefix = preview_scope.trim().trim_matches('/');
    if !prefix.is_empty() {
        segments.extend(
            prefix
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(str::to_string),
        );
    }
    segments
}

fn walk_contract_level_content_in_panel(
    builder: &mut Builder<'_>,
    host_panel: &UiNodeDecl,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
    file_hint: Option<&str>,
) {
    // Metric cards are surfaced as a single content node via `walk_content_panel`;
    // emitting contract-level label/value/unit slots here duplicates client mounts.
    if is_metric_card_panel(host_panel) {
        return;
    }
    for (block, content_label) in contract_level_content_blocks(host_panel) {
        walk_contract_level_content_block(
            builder,
            block,
            content_label.as_str(),
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
            file_hint,
            host_panel,
        );
    }
    for child in child_panels(host_panel) {
        if !is_content_group_panel(child) && !is_slotted_layout_panel(child) {
            walk_contract_level_content_in_panel(
                builder,
                child,
                tier,
                parent_id,
                parent_segments,
                preview_prefix,
                file_hint,
            );
        }
    }
}

fn walk_contract_level_content_block(
    builder: &mut Builder<'_>,
    block: &BlockDecl,
    label: &str,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
    file_hint: Option<&str>,
    host_panel: &UiNodeDecl,
) {
    if is_slot_area_block(block, host_panel) {
        let area = block
            .area
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "auto")
            .unwrap_or("slot");
        let slot_preview = format!("{}/{}", preview_prefix.trim().trim_end_matches('/'), area);
        let slot_segments =
            scope_segments_from_preview(builder.scene_id, tier, slot_preview.as_str());
        let slot_label = contract_grid_slot_label(area, block);
        let slot_budget = budget_from_chart_block(block);
        let slot_node = builder.make_node(
            UiScopeRole::Slot,
            slot_label,
            &slot_segments,
            slot_preview.clone(),
            Some(parent_id.to_string()),
            Some(tier.to_string()),
            slot_budget,
            source_anchor_for_block(block, file_hint),
            Some(block_content_use_key(block)),
        );
        let slot_node_id = builder.insert_node(slot_node);
        builder.link_child(parent_id, &slot_node_id);
        walk_content_block(
            builder,
            block,
            label,
            tier,
            slot_node_id.as_str(),
            parent_segments,
            slot_preview.as_str(),
            file_hint,
        );
        return;
    }
    walk_content_block(
        builder,
        block,
        label,
        tier,
        parent_id,
        parent_segments,
        preview_prefix,
        file_hint,
    );
}

fn contract_grid_slot_label(area: &str, block: &BlockDecl) -> String {
    if let Some(title) = block
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return title.to_string();
    }
    let content_label = content_label_from_block(block);
    if block_content_use_key(block).starts_with("chart.")
        && content_label != block_content_use_key(block)
    {
        return content_label;
    }
    area.to_string()
}

fn walk_content_block(
    builder: &mut Builder<'_>,
    block: &BlockDecl,
    label: &str,
    tier: &str,
    parent_id: &str,
    _parent_segments: &[String],
    preview_prefix: &str,
    file_hint: Option<&str>,
) {
    let content_key = block
        .id
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| block_content_use_key(block));
    let content_label = if label.trim().is_empty() {
        content_key.clone()
    } else {
        label.to_string()
    };
    let preview_scope =
        join_content_preview_scope(preview_prefix, block.area.as_deref(), content_key.as_str());
    let content_segments =
        scope_segments_from_preview(builder.scene_id, tier, preview_scope.as_str());
    let content_kind = Some(block_content_use_key(block));
    let content_node = builder.make_node(
        UiScopeRole::Content,
        content_label,
        &content_segments,
        preview_scope,
        Some(parent_id.to_string()),
        Some(tier.to_string()),
        None,
        source_anchor_for_block(block, file_hint),
        content_kind,
    );
    let content_id = builder.insert_node(content_node);
    builder.link_child(parent_id, &content_id);
}

fn walk_content_panel(
    builder: &mut Builder<'_>,
    panel: &UiNodeDecl,
    tier: &str,
    parent_id: &str,
    _parent_segments: &[String],
    preview_prefix: &str,
    label_hint: Option<&str>,
) {
    let content_key = panel.id.clone();
    let content_label = label_hint
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| metric_card_label(panel));
    let preview_scope =
        join_content_preview_scope(preview_prefix, panel.area.as_deref(), content_key.as_str());
    let content_segments =
        scope_segments_from_preview(builder.scene_id, tier, preview_scope.as_str());
    let content_kind = if is_viewport_chrome_panel(panel) {
        None
    } else {
        Some(
            panel
                .props
                .get("__mei_metric_template")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| "metric-card".to_string()),
        )
    };
    let content_node = builder.make_node(
        UiScopeRole::Content,
        content_label,
        &content_segments,
        preview_scope,
        Some(parent_id.to_string()),
        Some(tier.to_string()),
        None,
        source_anchor_for_panel(panel),
        content_kind,
    );
    let content_id = builder.insert_node(content_node);
    builder.link_child(parent_id, &content_id);
}

fn walk_layout_content_group(
    builder: &mut Builder<'_>,
    panel: &UiNodeDecl,
    tier: &str,
    parent_id: &str,
    _parent_segments: &[String],
    preview_prefix: &str,
    content_kind: &str,
) {
    let group_key = panel.id.clone();
    let group_label = layout_content_group_label(panel);
    let group_preview = format!("{}/{}", preview_prefix.trim_end_matches('/'), group_key);
    let group_segments =
        scope_segments_from_preview(builder.scene_id, tier, group_preview.as_str());
    let budget = budget_from_panel(panel);
    let group_node = builder.make_node(
        UiScopeRole::Content,
        group_label,
        &group_segments,
        group_preview.clone(),
        Some(parent_id.to_string()),
        Some(tier.to_string()),
        budget,
        source_anchor_for_panel(panel),
        Some(content_kind.to_string()),
    );
    let group_id = builder.insert_node(group_node);
    builder.link_child(parent_id, &group_id);

    for slot in slot_nodes_in_layout(panel) {
        walk_slot(
            builder,
            &slot,
            tier,
            &group_id,
            &group_segments,
            group_preview.as_str(),
        );
    }
}

fn layout_content_group_label(panel: &UiNodeDecl) -> String {
    if let Some(label) = panel
        .props
        .get("__mei_content_group_label")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return label.to_string();
    }
    if let Some(title) = panel
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return title.to_string();
    }
    for (id_hint, label) in GROUP_LABEL_OVERRIDES {
        if panel.id == *id_hint || panel.id.contains(id_hint) {
            return (*label).to_string();
        }
    }
    for metric in metric_card_panels_in_deep(panel) {
        let label = metric_card_label(metric);
        if label != metric.id && label != "metric_card" && !label.trim().is_empty() {
            return label;
        }
    }
    panel.id.clone()
}

const GROUP_LABEL_OVERRIDES: &[(&str, &str)] = &[
    ("penalty_count_summary", "处罚统计"),
    ("inspection_counts_layout", "检查统计"),
    ("inspection_no_violation_layout", "无违规分析"),
    ("issue_status_flow", "办理状态"),
    ("realtime_warning_layout", "预警汇总"),
    ("park_penalty_amounts", "园区处罚"),
];

fn chart_blocks_in_deep<'a>(panel: &'a UiNodeDecl) -> Vec<(&'a BlockDecl, String)> {
    let mut blocks = Vec::new();
    collect_chart_blocks(panel, &mut blocks);
    blocks
}

fn collect_chart_blocks<'a>(panel: &'a UiNodeDecl, out: &mut Vec<(&'a BlockDecl, String)>) {
    for ui_node in &panel.blocks {
        match ui_node {
            UiTreeNode::Block(block)
                if block_content_use_key(block).starts_with("chart.")
                    || block.area.as_deref() == Some("chart")
                    || block.id.as_deref() == Some("chart") =>
            {
                out.push((block, content_label_from_block(block)));
            }
            UiTreeNode::Panel(child) => collect_chart_blocks(child, out),
            _ => {}
        }
    }
}

fn content_blocks_in_deep<'a>(panel: &'a UiNodeDecl) -> Vec<(&'a BlockDecl, String)> {
    let mut blocks = content_blocks_in(panel);
    for ui_node in &panel.blocks {
        if let UiTreeNode::Panel(child) = ui_node {
            if is_slotted_layout_panel(child)
                || is_content_group_panel(child)
                || is_metric_card_panel(child)
            {
                continue;
            }
            blocks.extend(content_blocks_in_deep(child));
        }
    }
    blocks
}

struct SlotWalkItem {
    area: String,
    label: String,
    nested_panel: Option<UiNodeDecl>,
    block: Option<BlockDecl>,
}

fn sections_in_region(region: &UiNodeDecl) -> Vec<(String, UiNodeDecl)> {
    let mut section_panels: Vec<UiNodeDecl> = Vec::new();
    for ui_node in &region.blocks {
        if let UiTreeNode::Panel(panel) = ui_node {
            if panel_is_section(panel) {
                section_panels.push(panel.clone());
            }
        }
    }

    // Prefer explicit section/slide children. When several pages share one grid area
    // (presentation deck slides), key by panel.id so later pages are not dropped.
    if !section_panels.is_empty() {
        let mut area_counts: BTreeMap<String, usize> = BTreeMap::new();
        for panel in &section_panels {
            if let Some(area) = panel
                .area
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *area_counts.entry(area.to_string()).or_default() += 1;
            }
        }
        return section_panels
            .into_iter()
            .map(|panel| {
                let area = panel
                    .area
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                let shared = area
                    .as_ref()
                    .is_some_and(|value| area_counts.get(value).copied().unwrap_or(0) > 1);
                let key = if shared {
                    panel.id.clone()
                } else {
                    area.unwrap_or_else(|| panel.id.clone())
                };
                (key, panel)
            })
            .collect();
    }

    let mut sections = Vec::new();
    if let Some(areas) = region
        .layout
        .as_ref()
        .and_then(|layout| layout.areas.as_ref())
    {
        for area_row in areas {
            for area in area_row {
                if let Some(panel) = find_nested_panel_by_area(region, area.as_str()) {
                    sections.push((area.clone(), panel.clone()));
                }
            }
        }
    }
    sections
}

fn panel_is_section(panel: &UiNodeDecl) -> bool {
    if matches!(ui_role_from_props(&panel.props), Some("section")) {
        return true;
    }
    // slides are page containers, never section keys inside a region.
    if ui_role_from_props(&panel.props) == Some("slide") {
        return false;
    }
    if panel
        .props
        .get("__mei_section_title")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty())
    {
        return true;
    }
    if panel
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty())
    {
        return true;
    }
    if panel
        .props
        .get("__mei_shell")
        .and_then(|value| value.as_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("titled_shell"))
    {
        return true;
    }
    layout_macro_hint(panel)
        .is_some_and(|macro_name| macro_name.ends_with("_body") && !is_slotted_layout_panel(panel))
}

fn find_nested_panel_by_area<'a>(region: &'a UiNodeDecl, area: &str) -> Option<&'a UiNodeDecl> {
    for ui_node in &region.blocks {
        match ui_node {
            UiTreeNode::Panel(panel) if panel.area.as_deref() == Some(area) => return Some(panel),
            UiTreeNode::Panel(panel) if panel.id == area => return Some(panel),
            _ => {}
        }
    }
    None
}

fn slotted_layout_panels_in(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    let mut result = Vec::new();
    for ui_node in &panel.blocks {
        if let UiTreeNode::Panel(nested) = ui_node {
            if is_slotted_layout_panel(nested) {
                result.push(nested);
            }
        }
    }
    result
}

fn slotted_layout_panels_in_deep(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    let mut result = slotted_layout_panels_in(panel);
    for child in child_panels(panel) {
        if !is_slotted_layout_panel(child) && !is_content_group_panel(child) {
            result.extend(slotted_layout_panels_in_deep(child));
        }
    }
    result
}

fn layout_content_group_panels_in_deep(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    let mut result = Vec::new();
    collect_layout_content_group_panels(panel, &mut result);
    result
}

fn collect_layout_content_group_panels<'a>(panel: &'a UiNodeDecl, out: &mut Vec<&'a UiNodeDecl>) {
    for child in child_panels(panel) {
        if is_content_group_panel(child) {
            out.push(child);
            continue;
        }
        if !is_slotted_layout_panel(child) && !is_metric_card_panel(child) {
            collect_layout_content_group_panels(child, out);
        }
    }
}

fn child_panels(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    panel
        .blocks
        .iter()
        .filter_map(|ui_node| match ui_node {
            UiTreeNode::Panel(nested) => Some(nested),
            _ => None,
        })
        .collect()
}

const VIEWPORT_CHROME_ROLES: &[&str] = &[
    "viewport",
    "viewport_frame",
    "map_tools",
    "map_interaction_surface",
    "stage_aperture",
];

fn is_viewport_chrome_panel(panel: &UiNodeDecl) -> bool {
    if panel
        .props
        .get("__mei_chrome_role")
        .and_then(|v| v.as_str())
        .is_some_and(|role| VIEWPORT_CHROME_ROLES.contains(&role))
    {
        return true;
    }
    matches!(
        panel.id.as_str(),
        "map-viewport"
            | "map-interaction-surface"
            | "map-tools-slot"
            | "stage-aperture-frame"
            | "stage-aperture-hint"
    )
}

fn viewport_chrome_panels_in_deep(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    let mut result = Vec::new();
    collect_viewport_chrome_panels(panel, &mut result);
    result
}

fn collect_viewport_chrome_panels<'a>(panel: &'a UiNodeDecl, out: &mut Vec<&'a UiNodeDecl>) {
    for child in child_panels(panel) {
        if is_viewport_chrome_panel(child) {
            out.push(child);
        }
        if !is_slotted_layout_panel(child) && !is_metric_card_panel(child) {
            collect_viewport_chrome_panels(child, out);
        }
    }
}

fn is_metric_card_panel(panel: &UiNodeDecl) -> bool {
    panel
        .props
        .get("__mei_metric_card")
        .map(|value| match value {
            Value::Bool(enabled) => *enabled,
            Value::String(text) => matches!(text.as_str(), "1" | "true" | "yes"),
            _ => false,
        })
        .unwrap_or(false)
}

fn metric_card_panels_in(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    child_panels(panel)
        .into_iter()
        .filter(|nested| is_metric_card_panel(nested))
        .collect()
}

fn metric_card_panels_in_deep(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    let mut result = metric_card_panels_in(panel);
    for child in child_panels(panel) {
        if !is_slotted_layout_panel(child) && !is_metric_card_panel(child) {
            result.extend(metric_card_panels_in_deep(child));
        }
    }
    result
}

fn metric_card_panels_exclusive(panel: &UiNodeDecl) -> Vec<&UiNodeDecl> {
    let mut result = Vec::new();
    collect_metric_card_panels_exclusive(panel, &mut result);
    result
}

fn collect_metric_card_panels_exclusive<'a>(panel: &'a UiNodeDecl, out: &mut Vec<&'a UiNodeDecl>) {
    if is_slotted_layout_panel(panel) || is_content_group_panel(panel) {
        return;
    }
    out.extend(metric_card_panels_in(panel));
    for child in child_panels(panel) {
        collect_metric_card_panels_exclusive(child, out);
    }
}

fn metric_card_label(panel: &UiNodeDecl) -> String {
    if let Some(title) = panel.title.as_deref().filter(|v| !v.trim().is_empty()) {
        return title.to_string();
    }
    if let Some(label) = panel
        .props
        .get("source")
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
    {
        return label.to_string();
    }
    for ui_node in &panel.blocks {
        if let UiTreeNode::Block(block) = ui_node {
            if matches!(block.use_key.as_str(), "mei.text" | "label") {
                let label = content_label_from_block(block);
                if !label.is_empty() && label != block.use_key {
                    return label;
                }
            }
        }
    }
    panel.id.clone()
}

fn is_slot_shell_panel(panel: &UiNodeDecl) -> bool {
    if is_metric_card_panel(panel) || is_content_group_panel(panel) {
        return false;
    }
    if panel
        .layout
        .as_ref()
        .map(flat_areas)
        .is_some_and(|areas| areas.len() >= 2)
    {
        return false;
    }
    if panel
        .props
        .get("__mei_slot_frame_bg")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    // Generic slot shell: a single `content` area hosting nested panels or blocks.
    if is_single_content_slot_shell(panel) {
        return true;
    }
    if metric_card_panels_in(panel).len() == 1 {
        let areas = panel.layout.as_ref().map(flat_areas).unwrap_or_default();
        if areas.is_empty() || areas == ["content"] {
            return true;
        }
    }
    false
}

fn is_single_content_slot_shell(panel: &UiNodeDecl) -> bool {
    let areas = panel.layout.as_ref().map(flat_areas).unwrap_or_default();
    if areas != ["content"] {
        return false;
    }
    !child_panels(panel).is_empty()
        || panel
            .blocks
            .iter()
            .any(|node| matches!(node, UiTreeNode::Block(_)))
}

fn is_slotted_layout_panel(panel: &UiNodeDecl) -> bool {
    if is_content_group_panel(panel) {
        return false;
    }
    let areas = panel.layout.as_ref().map(flat_areas).unwrap_or_default();
    // Legacy chrome wrappers: single `content` area + slot frame bg. Author panels with
    // surface=compound also set __mei_slot_frame_bg but keep real multi-area layouts.
    if panel
        .props
        .get("__mei_slot_frame_bg")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && (areas.is_empty() || areas == ["content"])
    {
        return false;
    }
    if metric_card_panels_in(panel).len() == 1 {
        if areas.is_empty() || areas == ["content"] {
            return false;
        }
    }
    if layout_macro_hint(panel).is_some_and(macro_implies_slotted_layout) {
        return true;
    }
    let id = panel.id.as_str();
    if SLOTTED_LAYOUT_PANEL_ID_HINTS
        .iter()
        .any(|prefix| id.starts_with(prefix) || id.contains(prefix))
    {
        return true;
    }
    if let Some(layout) = &panel.layout {
        let areas = flat_areas(layout);
        if areas.contains(&"first".to_string())
            && areas.contains(&"second".to_string())
            && (areas.contains(&"third".to_string()) || areas.contains(&"compound".to_string()))
        {
            return true;
        }
        if areas.contains(&"main".to_string())
            && (areas.contains(&"sub_a".to_string()) || areas.contains(&"sub_b".to_string()))
        {
            return true;
        }
        if areas.len() >= 2 && panel_has_slotted_layout_children(panel, &areas) {
            return true;
        }
    }
    false
}

fn panel_has_slotted_layout_children(panel: &UiNodeDecl, areas: &[String]) -> bool {
    if areas.is_empty() {
        return false;
    }
    panel.blocks.iter().any(|ui_node| match ui_node {
        UiTreeNode::Block(block) => {
            let area = block
                .area
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "auto")
                .or_else(|| block.id.as_deref().map(str::trim).filter(|v| !v.is_empty()));
            area.is_some_and(|area| areas.iter().any(|slot| slot == area))
        }
        UiTreeNode::Panel(nested) => nested
            .area
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or(Some(nested.id.as_str()))
            .is_some_and(|area| areas.iter().any(|slot| slot == area)),
        _ => false,
    })
}

fn is_compound_metric_panel(panel: &UiNodeDecl) -> bool {
    if panel.props.get("__mei_compound_top_band_ratio").is_some()
        || panel.props.get("__mei_compound_top_ratio").is_some()
        || panel.props.get("__mei_compound_bottom_ratio").is_some()
    {
        return true;
    }
    if layout_macro_hint(panel).is_some_and(|macro_name| {
        macro_name.contains("wide_metric_compound") || macro_name.contains("long_metric_compound")
    }) {
        return true;
    }
    layout_has_areas(panel, &["top", "b0", "b1", "b2"])
        || layout_has_areas(panel, &["main", "rtop", "rbottom"])
}

fn is_chart_summary_panel(panel: &UiNodeDecl) -> bool {
    layout_macro_hint(panel).is_some_and(|macro_name| macro_name.contains("chart_with_summary"))
        || layout_has_areas(panel, &["summary", "chart"])
}

fn is_table_summary_panel(panel: &UiNodeDecl) -> bool {
    layout_macro_hint(panel).is_some_and(|macro_name| macro_name.contains("table_with_summary"))
        || layout_has_areas(panel, &["summary", "table"])
}

fn is_metric_summary_panel(panel: &UiNodeDecl) -> bool {
    layout_macro_hint(panel).is_some_and(|macro_name| macro_name.contains("summary_stack"))
        || layout_has_areas(panel, &["primary", "secondary_a", "secondary_b"])
}

fn is_status_flow_panel(panel: &UiNodeDecl) -> bool {
    layout_macro_hint(panel).is_some_and(|macro_name| macro_name.contains("status_triptych"))
        || layout_has_areas(panel, &["pending", "doing", "done", "summary"])
}

fn is_progress_triptych_panel(panel: &UiNodeDecl) -> bool {
    layout_macro_hint(panel)
        .is_some_and(|macro_name| macro_name.contains("primary_progress_triptych"))
        || layout_has_areas(panel, &["primary", "triptych"])
}

fn is_metric_list_panel(panel: &UiNodeDecl) -> bool {
    layout_macro_hint(panel).is_some_and(|macro_name| macro_name.contains("metric_list"))
        || panel.id.contains("metric_list")
}

fn content_group_kind(panel: &UiNodeDecl) -> Option<&'static str> {
    if is_compound_metric_panel(panel) {
        return Some("compound-metric");
    }
    if is_chart_summary_panel(panel) {
        return Some("chart-summary");
    }
    if is_table_summary_panel(panel) {
        return Some("table-summary");
    }
    if is_metric_summary_panel(panel) {
        return Some("metric-summary");
    }
    if is_status_flow_panel(panel) {
        return Some("status-flow");
    }
    if is_progress_triptych_panel(panel) {
        return Some("progress-triptych");
    }
    if is_metric_list_panel(panel) {
        return Some("metric-list");
    }
    None
}

fn is_content_group_panel(panel: &UiNodeDecl) -> bool {
    content_group_kind(panel).is_some()
}

fn layout_macro_hint(panel: &UiNodeDecl) -> Option<&str> {
    panel
        .props
        .get("__mei_layout_macro")
        .or_else(|| panel.props.get("__mei_macro"))
        .and_then(|value| value.as_str())
}

fn macro_implies_slotted_layout(macro_name: &str) -> bool {
    macro_name == "micro_panel" || macro_name.ends_with("_body")
}

fn layout_has_areas(panel: &UiNodeDecl, required: &[&str]) -> bool {
    let areas = panel.layout.as_ref().map(flat_areas).unwrap_or_default();
    required
        .iter()
        .all(|area| areas.iter().any(|value| value == area))
}

const SLOTTED_LAYOUT_PANEL_ID_HINTS: &[&str] = &[
    "metric_triptych",
    "wide_metric_compound",
    "enforcement_strip_layout",
    "supervision_triptych",
    "inspection_triptych",
    "penalty_triptych",
    "issue_triptych",
    "effectiveness_triptych",
    "chart_table_split",
    "semantic_layout",
    "supervision-stats",
];

fn slotted_layout_key(panel: &UiNodeDecl) -> String {
    layout_macro_hint(panel)
        .map(str::to_string)
        .unwrap_or_else(|| panel.id.clone())
}

fn slotted_layout_label(panel: &UiNodeDecl) -> String {
    layout_macro_hint(panel)
        .map(|macro_name| macro_name.replace("_body", ""))
        .unwrap_or_else(|| panel.id.clone())
}

fn slot_nodes_in_layout(layout_panel: &UiNodeDecl) -> Vec<SlotWalkItem> {
    let mut slots = Vec::new();
    let layout_areas = layout_panel
        .layout
        .as_ref()
        .map(flat_areas)
        .unwrap_or_default();

    for ui_node in &layout_panel.blocks {
        match ui_node {
            UiTreeNode::Block(block) => {
                let area = block
                    .area
                    .clone()
                    .filter(|v| !v.is_empty() && v != "auto")
                    .unwrap_or_else(|| block.id.clone().unwrap_or_else(|| block.use_key.clone()));
                if layout_areas.is_empty() || layout_areas.contains(&area) {
                    slots.push(SlotWalkItem {
                        area: area.clone(),
                        label: slot_label_from_block(block, area.as_str()),
                        nested_panel: None,
                        block: Some(block.clone()),
                    });
                }
            }
            UiTreeNode::Panel(panel) => {
                let area = panel
                    .area
                    .clone()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| panel.id.clone());
                if layout_areas.is_empty() || layout_areas.contains(&area) {
                    let label = if is_metric_card_panel(panel) {
                        metric_card_label(panel)
                    } else {
                        slot_label_from_panel(panel, area.as_str())
                    };
                    slots.push(SlotWalkItem {
                        area: area.clone(),
                        label,
                        nested_panel: Some(panel.clone()),
                        block: None,
                    });
                }
            }
            _ => {}
        }
    }
    slots
}

fn content_blocks_in(panel: &UiNodeDecl) -> Vec<(&BlockDecl, String)> {
    let mut blocks = Vec::new();
    for ui_node in &panel.blocks {
        if let UiTreeNode::Block(block) = ui_node {
            if is_content_block(block) && !is_slot_area_block(block, panel) {
                blocks.push((block, content_label_from_block(block)));
            }
        }
    }
    blocks
}

fn contract_level_content_blocks(panel: &UiNodeDecl) -> Vec<(&BlockDecl, String)> {
    let mut blocks = Vec::new();
    for ui_node in &panel.blocks {
        if let UiTreeNode::Block(block) = ui_node {
            if is_content_block(block) && is_slot_area_block(block, panel) {
                blocks.push((block, content_label_from_block(block)));
            }
        }
    }
    blocks
}

fn is_slot_area_block(block: &BlockDecl, panel: &UiNodeDecl) -> bool {
    let area = block
        .area
        .as_deref()
        .filter(|v| !v.is_empty() && *v != "auto")
        .or_else(|| block.id.as_deref().filter(|v| !v.is_empty()));
    let Some(area) = area else {
        return false;
    };
    panel
        .layout
        .as_ref()
        .map(flat_areas)
        .is_some_and(|areas| areas.contains(&area.to_string()))
}

fn is_content_block(block: &BlockDecl) -> bool {
    let key = block_content_use_key(block);
    if key.starts_with("chart.") {
        return true;
    }
    if key.contains("data-table") {
        return true;
    }
    !matches!(
        key.as_str(),
        "label" | "value" | "unit" | "icon" | "component"
    )
}

fn block_content_use_key(block: &BlockDecl) -> String {
    if !block.use_key.trim().is_empty() && block.use_key != "component" {
        return block.use_key.clone();
    }
    if let Some(value) = &block.component {
        if let Some(key) = value.as_str().filter(|text| !text.is_empty()) {
            return key.to_string();
        }
        if let Some(key) = value
            .get("use_key")
            .or_else(|| value.get("id"))
            .and_then(|entry| entry.as_str())
            .filter(|text| !text.is_empty())
        {
            return key.to_string();
        }
    }
    block.use_key.clone()
}

fn content_label_from_block(block: &BlockDecl) -> String {
    if let Some(title) = block.title.as_deref().filter(|v| !v.trim().is_empty()) {
        return title.to_string();
    }
    if block_content_use_key(block).starts_with("chart.") {
        if let Some(title) = block
            .props
            .get("title")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        {
            return title.to_string();
        }
        return chart_kind_label(block_content_use_key(block).as_str());
    }
    for pointer in [
        "/source/label",
        "/content/label",
        "/label",
        "/text",
        "/content",
    ] {
        if let Some(label) = block
            .props
            .pointer(pointer)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return label.to_string();
        }
    }
    block
        .props
        .get("source")
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            block
                .id
                .clone()
                .unwrap_or_else(|| block_content_use_key(block))
        })
}

fn slot_label_from_block(block: &BlockDecl, area: &str) -> String {
    let label = content_label_from_block(block);
    if label != block.use_key && !label.is_empty() {
        format!("{area} · {label}")
    } else {
        area.to_string()
    }
}

fn slot_label_from_panel(panel: &UiNodeDecl, area: &str) -> String {
    if let Some(title) = panel.title.as_deref().filter(|v| !v.trim().is_empty()) {
        format!("{area} · {title}")
    } else {
        area.to_string()
    }
}

fn chart_kind_label(use_key: &str) -> String {
    match use_key {
        "chart.column" => "分组柱图".to_string(),
        "chart.bar" => "条形图".to_string(),
        "chart.line" => "折线图".to_string(),
        "chart.ranking" => "排名图".to_string(),
        other => other
            .rsplit('.')
            .next()
            .filter(|value| !value.is_empty())
            .unwrap_or("chart")
            .to_string(),
    }
}

fn flat_areas(layout: &LayoutDecl) -> Vec<String> {
    layout
        .areas
        .as_ref()
        .map(|areas| areas.iter().flat_map(|row| row.iter().cloned()).collect())
        .unwrap_or_default()
}

fn panel_tier(panel: &UiNodeDecl) -> String {
    panel
        .props
        .get("__mei_tier")
        .and_then(|v| v.as_str())
        .map(|tier| tier.to_ascii_uppercase())
        .or_else(|| {
            panel
                .props
                .get("tier")
                .and_then(|v| v.as_str())
                .map(|tier| tier.to_ascii_uppercase())
        })
        .unwrap_or_else(|| "T1".to_string())
}

fn plane_label_for_tier(tier: &str) -> String {
    match tier.to_ascii_uppercase().as_str() {
        "T0" => "T0 · 底图".to_string(),
        "T2" => "T2 · 二层".to_string(),
        "P" => "P · 演说".to_string(),
        _ => "T1 · 首层".to_string(),
    }
}

fn region_label(panel: &UiNodeDecl) -> String {
    panel
        .title
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            panel
                .props
                .get("__mei_chrome_role")
                .and_then(|v| v.as_str())
                .map(|role| format!("region:{role}"))
                .unwrap_or_else(|| panel.id.clone())
        })
}

fn ui_role_from_props(props: &Value) -> Option<&str> {
    props.get("__mei_ui_role").and_then(|v| v.as_str())
}

fn budget_from_chart_block(_block: &BlockDecl) -> Option<UiBudgetSummary> {
    // chartHeight no longer drives section/slot height (content-budget path deleted).
    None
}

fn merged_section_budget(section: &UiNodeDecl) -> Option<UiBudgetSummary> {
    let mut budget = budget_from_panel(section).unwrap_or_default();
    if let Some(content_panel) = fill_content_panel_in_deep(section) {
        if let Some(content_budget) = budget_from_panel(content_panel) {
            budget = merge_section_shell_budget(budget, content_budget);
        }
    }
    if section
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty())
        && budget.grid_template_rows.is_none()
    {
        budget.grid_template_rows = Some("auto minmax(0, 1fr)".to_string());
    }
    if budget_is_empty(&budget) {
        None
    } else {
        Some(budget)
    }
}

fn merge_section_shell_budget(
    mut base: UiBudgetSummary,
    overlay: UiBudgetSummary,
) -> UiBudgetSummary {
    if overlay.padding.is_some() {
        base.padding = overlay.padding;
    }
    if overlay.section_derived_height_px.is_some() {
        base.section_derived_height_px = overlay.section_derived_height_px;
    }
    if overlay.padding_profile.is_some() {
        base.padding_profile = overlay.padding_profile;
    }
    base
}

fn fill_content_panel_in_deep(panel: &UiNodeDecl) -> Option<&UiNodeDecl> {
    if panel
        .props
        .as_object()
        .is_some_and(|map| map.get("__mei_layout_fill").and_then(Value::as_bool) == Some(true))
    {
        return Some(panel);
    }
    for child in child_panels(panel) {
        if let Some(found) = fill_content_panel_in_deep(child) {
            return Some(found);
        }
    }
    None
}

fn budget_from_panel(panel: &UiNodeDecl) -> Option<UiBudgetSummary> {
    let mut budget = UiBudgetSummary::default();
    if let Some(layout) = &panel.layout {
        if layout.layout_type != "flex" {
            if let Some(columns) = layout.columns.as_ref().filter(|cols| !cols.is_empty()) {
                budget.grid_template_columns = Some(columns.join(" "));
            }
            if let Some(rows) = layout.rows.as_ref().filter(|rows| !rows.is_empty()) {
                budget.grid_template_rows = Some(rows.join(" "));
            }
            if let Some(areas) = grid_template_areas_css(layout) {
                budget.grid_template_areas = Some(areas);
            }
            let slot_areas = flat_areas(layout)
                .into_iter()
                .filter(|area| !is_css_null_grid_area(area))
                .collect::<Vec<_>>();
            if !slot_areas.is_empty() {
                budget.slot_areas = Some(slot_areas);
            }
        }
        if let Some(gap) = layout.gap.as_deref().filter(|v| !v.is_empty()) {
            budget.gap = Some(gap.to_string());
        }
    }
    if let Some(map) = panel.props.as_object() {
        if let Some(h) = map
            .get("__mei_section_derived_height_px")
            .and_then(Value::as_f64)
        {
            budget.section_derived_height_px = Some(h);
        }
        if let Some(profile) = map
            .get("__mei_padding_profile")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
        {
            budget.padding_profile = Some(profile.to_string());
        }
    }
    for key in [
        "first_width",
        "second_width",
        "third_width",
        "compound_width",
        "width",
    ] {
        if let Some(value) = panel.props.get(key) {
            let text = value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_i64().map(|n| n.to_string()));
            if let Some(text) = text.filter(|v| !v.is_empty()) {
                if key == "width" {
                    budget.widths.insert("width".to_string(), text);
                } else {
                    budget.widths.insert(key.to_string(), text);
                }
            }
        }
    }
    if let Some(padding) = panel
        .body_props
        .get("padding")
        .or_else(|| panel.props.get("padding"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
    {
        budget.padding = Some(padding.to_string());
    }
    if let Some(overflow) = panel
        .props
        .get("overflow")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
    {
        budget.overflow = Some(overflow.to_string());
    }
    if budget_is_empty(&budget) {
        None
    } else {
        Some(budget)
    }
}

fn budget_is_empty(budget: &UiBudgetSummary) -> bool {
    budget.gap.is_none()
        && budget.padding.is_none()
        && budget.widths.is_empty()
        && budget.section_derived_height_px.is_none()
        && budget.padding_profile.is_none()
        && budget.grid_template_columns.is_none()
        && budget.grid_template_rows.is_none()
        && budget.grid_template_areas.is_none()
        && budget.slot_areas.is_none()
        && budget.overflow.is_none()
}

fn css_grid_area_token(area: &str) -> &str {
    let area = area.trim();
    // MeiLang uses `_` as an empty/discard cell; CSS null cells must be `.`.
    // Emitting `_` as a named area breaks when the same token appears in
    // non-contiguous cells (e.g. `'_ main _'` → browser drops the whole property).
    if area.is_empty() || area == "_" {
        "."
    } else {
        area
    }
}

fn is_css_null_grid_area(area: &str) -> bool {
    let area = area.trim();
    area.is_empty() || area == "_" || area == "."
}

fn grid_template_areas_css(layout: &LayoutDecl) -> Option<String> {
    let rows = layout.areas.as_ref()?;
    let formatted = rows
        .iter()
        .filter(|row| !row.is_empty())
        .map(|row| {
            let template = row
                .iter()
                .map(|area| css_grid_area_token(area))
                .collect::<Vec<_>>()
                .join(" ");
            format!("'{template}'")
        })
        .collect::<Vec<_>>();
    if formatted.is_empty() {
        None
    } else {
        Some(formatted.join(" "))
    }
}

fn budget_label_from_summary(budget: &UiBudgetSummary) -> String {
    let mut parts = Vec::new();
    if let Some(gap) = budget.gap.as_deref() {
        parts.push(format!("gap={gap}"));
    }
    if let Some(padding) = budget.padding.as_deref() {
        parts.push(format!("padding={padding}"));
    }
    if let Some(h) = budget.section_derived_height_px {
        parts.push(format!("section_derived_height_px={h:.0}"));
    }
    if let Some(profile) = budget.padding_profile.as_deref() {
        parts.push(format!("padding_profile={profile}"));
    }
    for (key, value) in &budget.widths {
        parts.push(format!("{key}={value}"));
    }
    if parts.is_empty() {
        "budget".to_string()
    } else {
        parts.join(", ")
    }
}

fn source_anchor_for_panel(panel: &UiNodeDecl) -> Vec<UiSourceAnchor> {
    panel
        .import_scope
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(|file| {
            vec![UiSourceAnchor {
                file: file.to_string(),
                symbol_id: panel.id.clone(),
            }]
        })
        .unwrap_or_default()
}

fn source_anchor_for_block(block: &BlockDecl, file_hint: Option<&str>) -> Vec<UiSourceAnchor> {
    block
        .id
        .as_ref()
        .map(|id| UiSourceAnchor {
            file: file_hint.unwrap_or("").to_string(),
            symbol_id: id.clone(),
        })
        .into_iter()
        .collect()
}
