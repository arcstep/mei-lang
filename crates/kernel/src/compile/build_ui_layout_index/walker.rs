use std::collections::BTreeMap;

use serde_json::Value;

use crate::model::{
    BlockDecl, BuildNodeId, LayoutDecl, PanelDecl, UiBudgetSummary, UiScopeNode, UiScopeRole,
    UiSourceAnchor, UiNodeDecl,
};

pub struct UiStructureBuildResult {
    pub nodes: BTreeMap<String, UiScopeNode>,
}

struct Builder<'a> {
    scene_id: &'a str,
    _scene_label: &'a str,
    _app_id: &'a str,
    nodes: BTreeMap<String, UiScopeNode>,
}

impl<'a> Builder<'a> {
    fn finish(self) -> UiStructureBuildResult {
        UiStructureBuildResult { nodes: self.nodes }
    }

    fn insert_node(&mut self, node: UiScopeNode) -> String {
        let id = node.node_id.clone();
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
    panels: &[PanelDecl],
    app_id: &str,
) -> UiStructureBuildResult {
    let mut builder = Builder {
        scene_id,
        _scene_label: scene_label,
        _app_id: app_id,
        nodes: BTreeMap::new(),
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

    let mut planes: BTreeMap<String, Vec<&PanelDecl>> = BTreeMap::new();
    for panel in panels {
        let tier = panel_tier(panel);
        planes.entry(tier).or_default().push(panel);
    }

    for (tier, region_panels) in planes {
        let plane_label = plane_label_for_tier(tier.as_str());
        let plane_segments = vec![scene_id.to_string(), tier.clone()];
        let plane_node = builder.make_node(
            UiScopeRole::Plane,
            plane_label,
            &plane_segments,
            String::new(),
            Some(scene_id_encoded.clone()),
            Some(tier.clone()),
            None,
            Vec::new(),
            None,
        );
        let plane_id = builder.insert_node(plane_node);
        builder.link_child(&scene_id_encoded, &plane_id);

        for region in region_panels {
            walk_region(&mut builder, region, tier.as_str(), &plane_id);
        }
    }

    builder.finish()
}

fn walk_region(builder: &mut Builder<'_>, region: &PanelDecl, tier: &str, plane_id: &str) {
    let region_id = region.id.clone();
    let region_label = region_label(region);
    let region_segments = vec![
        builder.scene_id.to_string(),
        tier.to_string(),
        region_id.clone(),
    ];
    let preview_scope = region_id.clone();
    let region_node = builder.make_node(
        UiScopeRole::Region,
        region_label,
        &region_segments,
        preview_scope,
        Some(plane_id.to_string()),
        Some(tier.to_string()),
        None,
        source_anchor_for_panel(region),
        None,
    );
    let region_node_id = builder.insert_node(region_node);
    builder.link_child(plane_id, &region_node_id);

    let sections = sections_in_region(region);
    if sections.is_empty() {
        walk_micro_and_content(builder, region, tier, &region_node_id, &region_segments, &region_id);
        return;
    }

    for (section_key, section_panel) in sections {
        walk_section(
            builder,
            &section_panel,
            tier,
            &region_node_id,
            &region_segments,
            &region_id,
            section_key.as_str(),
        );
    }
}

fn walk_section(
    builder: &mut Builder<'_>,
    section: &PanelDecl,
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
    let section_node = builder.make_node(
        UiScopeRole::Section,
        section_label,
        &section_segments,
        preview_scope.clone(),
        Some(region_node_id.to_string()),
        Some(tier.to_string()),
        None,
        source_anchor_for_panel(section),
        None,
    );
    let section_node_id = builder.insert_node(section_node);
    builder.link_child(region_node_id, &section_node_id);
    walk_micro_and_content(
        builder,
        section,
        tier,
        &section_node_id,
        &section_segments,
        preview_scope.as_str(),
    );
}

fn walk_micro_and_content(
    builder: &mut Builder<'_>,
    panel: &PanelDecl,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
) {
    let file_hint = panel
        .import_scope
        .as_deref()
        .filter(|v| !v.is_empty());
    for micro in micro_layout_panels_in_deep(panel) {
        walk_micro_layout(
            builder,
            micro,
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
        );
    }
    for metric_panel in metric_card_panels_in_deep(panel) {
        walk_content_panel(
            builder,
            metric_panel,
            tier,
            parent_id,
            parent_segments,
            preview_prefix,
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
}

fn walk_micro_layout(
    builder: &mut Builder<'_>,
    micro: &PanelDecl,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
) {
    let micro_key = micro_layout_key(micro);
    let micro_label = micro_layout_label(micro);
    let mut micro_segments = parent_segments.to_vec();
    micro_segments.push(micro_key.clone());
    let preview_scope = format!("{preview_prefix}/{micro_key}");
    let budget = budget_from_panel(micro);
    let micro_node = builder.make_node(
        UiScopeRole::MicroLayout,
        micro_label,
        &micro_segments,
        preview_scope.clone(),
        Some(parent_id.to_string()),
        Some(tier.to_string()),
        budget.clone(),
        source_anchor_for_panel(micro),
        Some(micro_macro_name(micro)),
    );
    let micro_node_id = builder.insert_node(micro_node);
    builder.link_child(parent_id, &micro_node_id);

    if let Some(budget) = budget {
        if !budget_is_empty(&budget) {
            let budget_label = budget_label_from_summary(&budget);
            let mut budget_segments = micro_segments.clone();
            budget_segments.push("budget".to_string());
            let budget_preview = format!("{preview_scope}/budget");
            let budget_node = builder.make_node(
                UiScopeRole::Budget,
                budget_label,
                &budget_segments,
                budget_preview,
                Some(micro_node_id.clone()),
                Some(tier.to_string()),
                Some(budget),
                Vec::new(),
                None,
            );
            let budget_id = builder.insert_node(budget_node);
            builder.link_child(&micro_node_id, &budget_id);
        }
    }

    for slot in slot_nodes_in_micro(micro) {
        walk_slot(
            builder,
            &slot,
            tier,
            &micro_node_id,
            &micro_segments,
            preview_scope.as_str(),
        );
    }
}

fn walk_slot(
    builder: &mut Builder<'_>,
    slot: &SlotWalkItem,
    tier: &str,
    micro_node_id: &str,
    micro_segments: &[String],
    preview_prefix: &str,
) {
    let slot_key = slot.area.clone();
    let slot_label = slot.label.clone();
    let mut slot_segments = micro_segments.to_vec();
    slot_segments.push(slot_key.clone());
    let preview_scope = format!("{preview_prefix}/{slot_key}");
    let slot_node = builder.make_node(
        UiScopeRole::Slot,
        slot_label,
        &slot_segments,
        preview_scope.clone(),
        Some(micro_node_id.to_string()),
        Some(tier.to_string()),
        None,
        Vec::new(),
        None,
    );
    let slot_node_id = builder.insert_node(slot_node);
    builder.link_child(micro_node_id, &slot_node_id);

    if let Some(panel) = &slot.nested_panel {
        if is_metric_card_panel(panel) {
            walk_content_panel(
                builder,
                panel,
                tier,
                &slot_node_id,
                &slot_segments,
                preview_scope.as_str(),
            );
        } else if is_micro_layout_panel(panel) {
            walk_micro_layout(
                builder,
                panel,
                tier,
                &slot_node_id,
                &slot_segments,
                preview_scope.as_str(),
            );
        } else {
            let file_hint = panel
                .import_scope
                .as_deref()
                .filter(|v| !v.is_empty());
            for micro in micro_layout_panels_in_deep(panel) {
                walk_micro_layout(
                    builder,
                    micro,
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                );
            }
            for metric_panel in metric_card_panels_in_deep(panel) {
                walk_content_panel(
                    builder,
                    metric_panel,
                    tier,
                    &slot_node_id,
                    &slot_segments,
                    preview_scope.as_str(),
                );
            }
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

fn walk_content_block(
    builder: &mut Builder<'_>,
    block: &BlockDecl,
    label: &str,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
    file_hint: Option<&str>,
) {
    let content_key = block
        .id
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| block.use_key.clone());
    let content_label = if label.trim().is_empty() {
        content_key.clone()
    } else {
        label.to_string()
    };
    let mut content_segments = parent_segments.to_vec();
    content_segments.push(content_key.clone());
    let area_suffix = block
        .area
        .as_deref()
        .filter(|v| !v.is_empty() && *v != "auto")
        .map(|area| format!("/{area}"))
        .unwrap_or_default();
    let preview_scope = format!("{preview_prefix}{area_suffix}/{content_key}");
    let content_kind = Some(block.use_key.clone());
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
    panel: &PanelDecl,
    tier: &str,
    parent_id: &str,
    parent_segments: &[String],
    preview_prefix: &str,
) {
    let content_key = panel.id.clone();
    let content_label = metric_card_label(panel);
    let mut content_segments = parent_segments.to_vec();
    content_segments.push(content_key.clone());
    let area_suffix = panel
        .area
        .as_deref()
        .filter(|v| !v.is_empty() && *v != "auto")
        .map(|area| format!("/{area}"))
        .unwrap_or_default();
    let preview_scope = format!("{preview_prefix}{area_suffix}/{content_key}");
    let content_kind = Some(
        panel
            .props
            .get("__mei_metric_template")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| "metric-card".to_string()),
    );
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

struct SlotWalkItem {
    area: String,
    label: String,
    nested_panel: Option<PanelDecl>,
    block: Option<BlockDecl>,
}

fn sections_in_region(region: &PanelDecl) -> Vec<(String, PanelDecl)> {
    let mut sections = Vec::new();
    if let Some(areas) = region.layout.as_ref().and_then(|layout| layout.areas.as_ref()) {
        for area_row in areas {
            for area in area_row {
                if let Some(panel) = find_nested_panel_by_area(region, area.as_str()) {
                    sections.push((area.clone(), panel.clone()));
                }
            }
        }
    }
    for ui_node in &region.blocks {
        if let UiNodeDecl::Panel(panel) = ui_node {
            if panel.title.as_deref().is_some_and(|t| !t.trim().is_empty()) {
                let key = panel
                    .area
                    .clone()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| panel.id.clone());
                if !sections.iter().any(|(k, _)| k == &key) {
                    sections.push((key, panel.clone()));
                }
            }
        }
    }
    sections
}

fn find_nested_panel_by_area<'a>(region: &'a PanelDecl, area: &str) -> Option<&'a PanelDecl> {
    for ui_node in &region.blocks {
        match ui_node {
            UiNodeDecl::Panel(panel) if panel.area.as_deref() == Some(area) => return Some(panel),
            UiNodeDecl::Panel(panel) if panel.id == area => return Some(panel),
            _ => {}
        }
    }
    None
}

fn micro_layout_panels_in(panel: &PanelDecl) -> Vec<&PanelDecl> {
    let mut result = Vec::new();
    for ui_node in &panel.blocks {
        if let UiNodeDecl::Panel(nested) = ui_node {
            if is_micro_layout_panel(nested) {
                result.push(nested);
            }
        }
    }
    result
}

fn micro_layout_panels_in_deep(panel: &PanelDecl) -> Vec<&PanelDecl> {
    let mut result = micro_layout_panels_in(panel);
    for child in child_panels(panel) {
        if !is_micro_layout_panel(child) {
            result.extend(micro_layout_panels_in_deep(child));
        }
    }
    result
}

fn child_panels(panel: &PanelDecl) -> Vec<&PanelDecl> {
    panel
        .blocks
        .iter()
        .filter_map(|ui_node| match ui_node {
            UiNodeDecl::Panel(nested) => Some(nested),
            _ => None,
        })
        .collect()
}

fn is_metric_card_panel(panel: &PanelDecl) -> bool {
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

fn metric_card_panels_in(panel: &PanelDecl) -> Vec<&PanelDecl> {
    child_panels(panel)
        .into_iter()
        .filter(|nested| is_metric_card_panel(nested))
        .collect()
}

fn metric_card_panels_in_deep(panel: &PanelDecl) -> Vec<&PanelDecl> {
    let mut result = metric_card_panels_in(panel);
    for child in child_panels(panel) {
        if !is_micro_layout_panel(child) && !is_metric_card_panel(child) {
            result.extend(metric_card_panels_in_deep(child));
        }
    }
    result
}

fn metric_card_label(panel: &PanelDecl) -> String {
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
        if let UiNodeDecl::Block(block) = ui_node {
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

fn is_micro_layout_panel(panel: &PanelDecl) -> bool {
    if ui_role_from_props(&panel.props) == Some("micro_layout") {
        return true;
    }
    if panel.props.get("__mei_macro").is_some() {
        return true;
    }
    let id = panel.id.as_str();
    if KNOWN_MICRO_LAYOUT_IDS
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
    }
    false
}

const KNOWN_MICRO_LAYOUT_IDS: &[&str] = &[
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
];

fn micro_layout_key(panel: &PanelDecl) -> String {
    panel
        .props
        .get("__mei_macro")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| panel.id.clone())
}

fn micro_layout_label(panel: &PanelDecl) -> String {
    panel
        .props
        .get("__mei_macro")
        .and_then(|v| v.as_str())
        .map(|macro_name| macro_name.replace("_body", ""))
        .unwrap_or_else(|| panel.id.clone())
}

fn micro_macro_name(panel: &PanelDecl) -> String {
    panel
        .props
        .get("__mei_macro")
        .and_then(|v| v.as_str())
        .unwrap_or("micro_layout")
        .to_string()
}

fn slot_nodes_in_micro(micro: &PanelDecl) -> Vec<SlotWalkItem> {
    let mut slots = Vec::new();
    let layout_areas = micro
        .layout
        .as_ref()
        .map(flat_areas)
        .unwrap_or_default();

    for ui_node in &micro.blocks {
        match ui_node {
            UiNodeDecl::Block(block) => {
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
            UiNodeDecl::Panel(panel) => {
                let area = panel
                    .area
                    .clone()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| panel.id.clone());
                if layout_areas.is_empty() || layout_areas.contains(&area) {
                    slots.push(SlotWalkItem {
                        area: area.clone(),
                        label: slot_label_from_panel(panel, area.as_str()),
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

fn content_blocks_in(panel: &PanelDecl) -> Vec<(&BlockDecl, String)> {
    let mut blocks = Vec::new();
    for ui_node in &panel.blocks {
        if let UiNodeDecl::Block(block) = ui_node {
            if is_content_block(block) && !is_slot_area_block(block, panel) {
                blocks.push((block, content_label_from_block(block)));
            }
        }
    }
    blocks
}

fn is_slot_area_block(block: &BlockDecl, panel: &PanelDecl) -> bool {
    let Some(area) = block.area.as_deref().filter(|v| !v.is_empty() && *v != "auto") else {
        return false;
    };
    panel
        .layout
        .as_ref()
        .map(flat_areas)
        .is_some_and(|areas| areas.contains(&area.to_string()))
}

fn is_content_block(block: &BlockDecl) -> bool {
    let key = block.use_key.as_str();
    !matches!(key, "label" | "value" | "unit" | "icon")
}

fn content_label_from_block(block: &BlockDecl) -> String {
    if let Some(title) = block.title.as_deref().filter(|v| !v.trim().is_empty()) {
        return title.to_string();
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
                .unwrap_or_else(|| block.use_key.clone())
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

fn slot_label_from_panel(panel: &PanelDecl, area: &str) -> String {
    if let Some(title) = panel.title.as_deref().filter(|v| !v.trim().is_empty()) {
        format!("{area} · {title}")
    } else {
        area.to_string()
    }
}

fn flat_areas(layout: &LayoutDecl) -> Vec<String> {
    layout
        .areas
        .as_ref()
        .map(|areas| {
            areas
                .iter()
                .flat_map(|row| row.iter().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn panel_tier(panel: &PanelDecl) -> String {
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

fn region_label(panel: &PanelDecl) -> String {
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
    props
        .get("__mei_ui_role")
        .and_then(|v| v.as_str())
}

fn budget_from_panel(panel: &PanelDecl) -> Option<UiBudgetSummary> {
    let mut budget = UiBudgetSummary::default();
    if let Some(layout) = &panel.layout {
        if let Some(gap) = layout.gap.as_deref().filter(|v| !v.is_empty()) {
            budget.gap = Some(gap.to_string());
        }
    }
    for key in [
        "first_width",
        "second_width",
        "third_width",
        "compound_width",
        "width",
        "card_height",
    ] {
        if let Some(value) = panel.props.get(key) {
            let text = value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_i64().map(|n| n.to_string()));
            if let Some(text) = text.filter(|v| !v.is_empty()) {
                if key == "card_height" {
                    budget.card_height = text.parse().ok();
                } else if key == "width" {
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
    if budget_is_empty(&budget) {
        None
    } else {
        Some(budget)
    }
}

fn budget_is_empty(budget: &UiBudgetSummary) -> bool {
    budget.gap.is_none()
        && budget.padding.is_none()
        && budget.card_height.is_none()
        && budget.widths.is_empty()
}

fn budget_label_from_summary(budget: &UiBudgetSummary) -> String {
    let mut parts = Vec::new();
    if let Some(gap) = budget.gap.as_deref() {
        parts.push(format!("gap={gap}"));
    }
    if let Some(padding) = budget.padding.as_deref() {
        parts.push(format!("padding={padding}"));
    }
    if let Some(height) = budget.card_height {
        parts.push(format!("card_height={height}"));
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

fn source_anchor_for_panel(panel: &PanelDecl) -> Vec<UiSourceAnchor> {
    panel
        .import_scope
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(|file| vec![UiSourceAnchor {
            file: file.to_string(),
            symbol_id: panel.id.clone(),
        }])
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
