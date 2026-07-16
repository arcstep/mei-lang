use std::collections::BTreeMap;

use mei_lang_kernel::{UiNodeDecl, UiTreeNode, PRESENTATION_MAP_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tier::DEFAULT_PANEL_TIER;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewpointMapEntry {
    pub tier: String,
    #[serde(rename = "panelId")]
    pub panel_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "blockPath")]
    pub block_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "viewFamily"
    )]
    pub view_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "stageKind")]
    pub stage_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "worldRef")]
    pub world_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "entityId")]
    pub entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "cameraPreset"
    )]
    pub camera_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresentationDeckSlide {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresentationDeck {
    #[serde(rename = "stageKind")]
    pub stage_kind: String,
    pub slides: Vec<PresentationDeckSlide>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "activeSlideId"
    )]
    pub active_slide_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresentationMapDocument {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub scene: String,
    pub viewpoints: BTreeMap<String, ViewpointMapEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck: Option<PresentationDeck>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "defaultScript"
    )]
    pub default_script: Option<Value>,
}

fn panel_tier(panel: &UiNodeDecl) -> String {
    panel
        .props
        .get("__mei_tier")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PANEL_TIER)
        .to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ViewpointHints {
    view_family: Option<String>,
    stage_kind: Option<String>,
    world_ref: Option<String>,
    entity_id: Option<String>,
    group_id: Option<String>,
    camera_preset: Option<String>,
}

impl ViewpointHints {
    fn from_value(value: &Value) -> Self {
        let Some(obj) = value.as_object() else {
            return Self::default();
        };
        let mut hints = Self {
            view_family: read_string(obj, &["__mei_view_family", "viewFamily", "view_family"]),
            stage_kind: read_string(obj, &["__mei_stage_kind", "stageKind", "stage_kind"]),
            world_ref: read_string(obj, &["__mei_world_ref", "worldRef", "world_ref"]),
            entity_id: read_string(obj, &["entityId", "entity_id"]),
            group_id: read_string(obj, &["groupId", "group_id"]),
            camera_preset: read_string(obj, &["cameraPreset", "camera_preset"]),
        };
        match obj.get("__call").and_then(|v| v.as_str()) {
            Some("map_view") => {
                hints.view_family.get_or_insert_with(|| "map".to_string());
            }
            Some("world_view") => {
                hints.view_family.get_or_insert_with(|| "world".to_string());
            }
            _ => {}
        }
        if let Some(args) = obj.get("__args").and_then(|v| v.as_object()) {
            if hints.world_ref.is_none() {
                hints.world_ref = read_string(args, &["worldRef", "world_ref", "arg0"]);
            }
            if hints.entity_id.is_none() {
                hints.entity_id = read_string(args, &["entityId", "entity_id"]);
            }
            if hints.group_id.is_none() {
                hints.group_id = read_string(args, &["groupId", "group_id"]);
            }
            if hints.camera_preset.is_none() {
                hints.camera_preset = read_string(args, &["cameraPreset", "camera_preset"]);
            }
        }
        hints
    }

    fn with_fallbacks(mut self, fallback: &Self) -> Self {
        if self.view_family.is_none() {
            self.view_family = fallback.view_family.clone();
        }
        if self.stage_kind.is_none() {
            self.stage_kind = fallback.stage_kind.clone();
        }
        if self.world_ref.is_none() {
            self.world_ref = fallback.world_ref.clone();
        }
        if self.entity_id.is_none() {
            self.entity_id = fallback.entity_id.clone();
        }
        if self.group_id.is_none() {
            self.group_id = fallback.group_id.clone();
        }
        if self.camera_preset.is_none() {
            self.camera_preset = fallback.camera_preset.clone();
        }
        self
    }
}

fn read_string(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn panel_viewpoint_hints(panel: &UiNodeDecl) -> ViewpointHints {
    ViewpointHints::from_value(&panel.props)
}

fn entry_from_hints(
    tier: String,
    panel_id: String,
    block_path: Option<String>,
    label: Option<String>,
    hints: ViewpointHints,
) -> ViewpointMapEntry {
    ViewpointMapEntry {
        tier,
        panel_id,
        block_path,
        label,
        view_family: hints.view_family,
        stage_kind: hints.stage_kind,
        world_ref: hints.world_ref,
        entity_id: hints.entity_id,
        group_id: hints.group_id,
        camera_preset: hints.camera_preset,
    }
}

fn merge_viewpoint_entry(
    existing: Option<ViewpointMapEntry>,
    candidate: ViewpointMapEntry,
) -> ViewpointMapEntry {
    let Some(existing) = existing else {
        return candidate;
    };
    let existing_score = viewpoint_entry_specificity(&existing);
    let candidate_score = viewpoint_entry_specificity(&candidate);
    let preserve_existing_carrier = existing_score > candidate_score;
    ViewpointMapEntry {
        tier: if preserve_existing_carrier {
            existing.tier.clone()
        } else {
            candidate.tier.clone()
        },
        panel_id: if preserve_existing_carrier {
            existing.panel_id.clone()
        } else {
            candidate.panel_id.clone()
        },
        block_path: if preserve_existing_carrier {
            existing.block_path.clone().or(candidate.block_path)
        } else {
            candidate.block_path.or(existing.block_path)
        },
        label: candidate.label.or(existing.label),
        view_family: candidate.view_family.or(existing.view_family),
        stage_kind: candidate.stage_kind.or(existing.stage_kind),
        world_ref: candidate.world_ref.or(existing.world_ref),
        entity_id: candidate.entity_id.or(existing.entity_id),
        group_id: candidate.group_id.or(existing.group_id),
        camera_preset: candidate.camera_preset.or(existing.camera_preset),
    }
}

fn viewpoint_entry_specificity(entry: &ViewpointMapEntry) -> usize {
    let mut score = 0usize;
    if entry.view_family.is_some() {
        score += 2;
    }
    if entry.stage_kind.is_some() {
        score += 2;
    }
    if entry.world_ref.is_some() {
        score += 2;
    }
    if entry.entity_id.is_some() {
        score += 1;
    }
    if entry.group_id.is_some() {
        score += 1;
    }
    if entry.camera_preset.is_some() {
        score += 1;
    }
    score
}

fn upsert_viewpoint(
    out: &mut BTreeMap<String, ViewpointMapEntry>,
    viewpoint_id: String,
    entry: ViewpointMapEntry,
) {
    let merged = merge_viewpoint_entry(out.remove(viewpoint_id.as_str()), entry);
    out.insert(viewpoint_id, merged);
}

fn collect_block_viewpoints(
    nodes: &[UiTreeNode],
    panel: &UiNodeDecl,
    path_prefix: &str,
    inherited_hints: &ViewpointHints,
    panel_payloads: &BTreeMap<String, Value>,
    out: &mut BTreeMap<String, ViewpointMapEntry>,
) {
    for (index, node) in nodes.iter().enumerate() {
        let block_path = if path_prefix.is_empty() {
            format!("{index}")
        } else {
            format!("{path_prefix}/{index}")
        };
        match node {
            UiTreeNode::Block(block) => {
                if let Some(vp) = block
                    .props
                    .get("viewpoint")
                    .or_else(|| block.props.get("__mei_viewpoint"))
                {
                    if let Some(id) = resolve_viewpoint_id(vp) {
                        let hints = ViewpointHints::from_value(&block.props)
                            .with_fallbacks(inherited_hints);
                        upsert_viewpoint(
                            out,
                            id.clone(),
                            entry_from_hints(
                                panel_tier(panel),
                                panel.id.clone(),
                                Some(block_path.clone()),
                                block
                                    .props
                                    .get("label")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                                hints,
                            ),
                        );
                    }
                }
            }
            UiTreeNode::Panel(nested) => {
                if let Some(vp) = nested
                    .props
                    .get("viewpoint")
                    .or_else(|| nested.props.get("__mei_viewpoint"))
                {
                    if let Some(id) = resolve_viewpoint_id(vp) {
                        let hints = panel_viewpoint_hints(nested).with_fallbacks(inherited_hints);
                        upsert_viewpoint(
                            out,
                            id.clone(),
                            entry_from_hints(
                                panel_tier(nested),
                                nested.id.clone(),
                                Some(block_path.clone()),
                                nested.title.clone(),
                                hints,
                            ),
                        );
                    }
                }
                let nested_hints = panel_viewpoint_hints(nested).with_fallbacks(inherited_hints);
                if let Some(payload) = panel_payloads.get(nested.id.as_str()) {
                    merge_content_panel_viewpoints(
                        payload,
                        nested.id.as_str(),
                        panel_tier(nested).as_str(),
                        &nested_hints,
                        out,
                    );
                }
                collect_block_viewpoints(
                    &nested.blocks,
                    nested,
                    &block_path,
                    &nested_hints,
                    panel_payloads,
                    out,
                );
            }
            _ => {}
        }
    }
}

pub fn resolve_viewpoint_id(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    let obj = value.as_object()?;
    if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
        return Some(id.to_string());
    }
    if obj.get("__ref").and_then(|v| v.as_str()) == Some("viewpoint_ref") {
        return obj
            .get("__args")
            .and_then(|args| args.get("arg0"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    if obj.get("__call").and_then(|v| v.as_str()) == Some("viewpoint_ref") {
        return obj
            .get("__args")
            .and_then(|args| args.get("arg0"))
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("id").and_then(|v| v.as_str()))
            .or_else(|| obj.get("key").and_then(|v| v.as_str()))
            .map(str::to_string);
    }
    None
}

fn viewpoints_array<'a>(payload: &'a Value) -> Option<&'a Vec<Value>> {
    let viewpoints = payload.get("viewpoints")?;
    viewpoints.as_array().or_else(|| {
        viewpoints
            .as_object()
            .and_then(|obj| obj.get("value"))
            .and_then(Value::as_array)
    })
}

fn viewpoint_entry_args(entry: &Value) -> Option<&Value> {
    match entry.get("__call").and_then(Value::as_str) {
        Some("viewpoint") => Some(entry.get("__args").unwrap_or(entry)),
        _ => {
            let obj = entry.as_object()?;
            if obj
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
            {
                Some(entry)
            } else {
                None
            }
        }
    }
}

fn merge_content_panel_viewpoints(
    payload: &Value,
    panel_id: &str,
    tier: &str,
    inherited_hints: &ViewpointHints,
    out: &mut BTreeMap<String, ViewpointMapEntry>,
) {
    let Some(viewpoints) = viewpoints_array(payload) else {
        return;
    };
    for entry in viewpoints {
        let Some(args) = viewpoint_entry_args(entry) else {
            continue;
        };
        let id = args.get("id").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let hints = ViewpointHints::from_value(args).with_fallbacks(inherited_hints);
        let blocks = args
            .get("blocks")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|s| !s.is_empty());
        upsert_viewpoint(
            out,
            id.to_string(),
            entry_from_hints(
                tier.to_string(),
                panel_id.to_string(),
                blocks,
                args.get("label")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                hints,
            ),
        );
    }
}

pub fn build_presentation_map(
    scene_id: &str,
    panels: &[UiNodeDecl],
    panel_payloads: &BTreeMap<String, Value>,
) -> PresentationMapDocument {
    build_presentation_map_with_default_script(scene_id, panels, panel_payloads, None)
}

pub fn build_presentation_map_with_default_script(
    scene_id: &str,
    panels: &[UiNodeDecl],
    panel_payloads: &BTreeMap<String, Value>,
    default_script: Option<Value>,
) -> PresentationMapDocument {
    let mut viewpoints = BTreeMap::new();
    for panel in panels {
        let tier = panel_tier(panel);
        let panel_hints = panel_viewpoint_hints(panel);
        if let Some(vp) = panel.props.get("__mei_viewpoint") {
            if let Some(id) = resolve_viewpoint_id(vp) {
                upsert_viewpoint(
                    &mut viewpoints,
                    id.clone(),
                    entry_from_hints(
                        tier.clone(),
                        panel.id.clone(),
                        None,
                        panel.title.clone(),
                        panel_hints.clone(),
                    ),
                );
            }
        }
        if let Some(payload) = panel_payloads.get(panel.id.as_str()) {
            merge_content_panel_viewpoints(
                payload,
                panel.id.as_str(),
                tier.as_str(),
                &panel_hints,
                &mut viewpoints,
            );
        }
        collect_block_viewpoints(
            &panel.blocks,
            panel,
            "",
            &panel_hints,
            panel_payloads,
            &mut viewpoints,
        );
    }
    let deck = build_presentation_deck(panels);
    PresentationMapDocument {
        schema_version: PRESENTATION_MAP_SCHEMA_VERSION.to_string(),
        scene: scene_id.to_string(),
        viewpoints,
        deck,
        default_script,
    }
}

fn build_presentation_deck(panels: &[UiNodeDecl]) -> Option<PresentationDeck> {
    let mut slides = Vec::new();
    collect_deck_slides(panels, &mut slides);
    if slides.is_empty() {
        return None;
    }
    for (order, slide) in slides.iter_mut().enumerate() {
        slide.order = order;
    }
    let active_slide_id = slides.first().map(|slide| slide.id.clone());
    Some(PresentationDeck {
        stage_kind: "presentation".to_string(),
        slides,
        active_slide_id,
    })
}

fn collect_deck_slides(panels: &[UiNodeDecl], out: &mut Vec<PresentationDeckSlide>) {
    let mut seen = std::collections::BTreeSet::new();
    collect_deck_slides_inner(panels, out, &mut seen);
}

fn collect_deck_slides_inner(
    panels: &[UiNodeDecl],
    out: &mut Vec<PresentationDeckSlide>,
    seen: &mut std::collections::BTreeSet<String>,
) {
    for panel in panels {
        if panel.props.get("__mei_ui_role").and_then(Value::as_str) == Some("slide") {
            if seen.insert(panel.id.clone()) {
                out.push(PresentationDeckSlide {
                    id: panel.id.clone(),
                    title: panel
                        .props
                        .get("__mei_slide_title")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| panel.title.clone()),
                    chapter: panel
                        .props
                        .get("__mei_slide_chapter")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    pattern: panel
                        .props
                        .get("__mei_slide_pattern")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    order: out.len(),
                });
            }
        }
        let nested: Vec<UiNodeDecl> = panel
            .blocks
            .iter()
            .filter_map(|node| match node {
                UiTreeNode::Panel(child) => Some(child.clone()),
                _ => None,
            })
            .collect();
        if !nested.is_empty() {
            collect_deck_slides_inner(&nested, out, seen);
        }
    }
}

pub fn presentation_map_to_value(map: &PresentationMapDocument) -> Value {
    serde_json::to_value(map).unwrap_or(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{UiNodeDecl, UiTreeNode};

    #[test]
    fn resolve_viewpoint_id_handles_compiler_viewpoint_ref_call() {
        let vp = json!({
            "__call": "viewpoint_ref",
            "__args": { "arg0": "warnings_total" }
        });
        assert_eq!(resolve_viewpoint_id(&vp).as_deref(), Some("warnings_total"));
    }

    #[test]
    fn resolve_viewpoint_id_handles_v2_viewpoint_ref() {
        let vp = json!({
            "__ref": "viewpoint_ref",
            "__args": { "arg0": "warnings_total" }
        });
        assert_eq!(resolve_viewpoint_id(&vp).as_deref(), Some("warnings_total"));
    }

    #[test]
    fn collect_metric_card_viewpoint_from_nested_panel() {
        let card = UiNodeDecl {
            kind: "panel".to_string(),
            id: "warnings_total_card".to_string(),
            title: None,
            head: None,
            area: Some("warnings".to_string()),
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({
                "__mei_viewpoint": "warnings_total",
                "__mei_metric_card": true,
                "__mei_view_family": "map",
                "__mei_world_ref": "park_world",
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let parent = UiNodeDecl {
            kind: "panel".to_string(),
            id: "supervision-stats".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiTreeNode::Panel(card)],
            slot: None,
            props: json!({}),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let map = build_presentation_map("home", std::slice::from_ref(&parent), &BTreeMap::new());
        assert!(map.viewpoints.contains_key("warnings_total"));
        assert_eq!(
            map.viewpoints
                .get("warnings_total")
                .and_then(|entry| entry.view_family.as_deref()),
            Some("map")
        );
        assert_eq!(
            map.viewpoints
                .get("warnings_total")
                .and_then(|entry| entry.world_ref.as_deref()),
            Some("park_world")
        );
    }

    #[test]
    fn merge_content_panel_viewpoints_preserves_world_target_hints() {
        let payload = json!({
            "viewpoints": [{
                "__call": "viewpoint",
                "__args": {
                    "id": "park_overview_stage",
                    "label": "总览",
                    "worldRef": "park_world",
                    "entityId": "lake_pavilion",
                    "groupId": "overview",
                    "cameraPreset": "orbit"
                }
            }]
        });
        let mut out = BTreeMap::new();
        merge_content_panel_viewpoints(
            &payload,
            "basemap",
            "t0",
            &ViewpointHints {
                view_family: Some("map".to_string()),
                ..ViewpointHints::default()
            },
            &mut out,
        );
        let entry = out.get("park_overview_stage").expect("viewpoint entry");
        assert_eq!(entry.view_family.as_deref(), Some("map"));
        assert_eq!(entry.world_ref.as_deref(), Some("park_world"));
        assert_eq!(entry.entity_id.as_deref(), Some("lake_pavilion"));
        assert_eq!(entry.group_id.as_deref(), Some("overview"));
        assert_eq!(entry.camera_preset.as_deref(), Some("orbit"));
    }

    #[test]
    fn build_presentation_map_merges_panel_viewpoint_hints_with_block_focus_target() {
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "basemap".to_string(),
            title: Some("迷你公园总览".to_string()),
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiTreeNode::Block(mei_lang_kernel::BlockDecl {
                kind: "block".to_string(),
                use_key: "cockpit.basemap-stage".to_string(),
                id: Some("basemap_stage".to_string()),
                title: None,
                area: Some("auto".to_string()),
                props: json!({
                    "__mei_viewpoint": "park_point_1_entry"
                }),
                base: None,
                layout: None,
                blocks: Vec::new(),
                component: None,
                placement: None,
                interactions: Vec::new(),
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            slot: None,
            props: json!({
                "__mei_tier": "t0",
                "__mei_view_family": "map"
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let payloads = BTreeMap::from([(
            "basemap".to_string(),
            json!({
                "viewpoints": [{
                    "__call": "viewpoint",
                    "__args": {
                        "id": "park_point_1_entry",
                        "worldRef": "park_world",
                        "entityId": "lake_pavilion",
                        "groupId": "lake_pavilion_story",
                        "cameraPreset": "lake_pavilion_focus"
                    }
                }]
            }),
        )]);
        let map = build_presentation_map("home", &[panel], &payloads);
        let entry = map
            .viewpoints
            .get("park_point_1_entry")
            .expect("merged viewpoint entry");
        assert_eq!(entry.view_family.as_deref(), Some("map"));
        assert_eq!(entry.world_ref.as_deref(), Some("park_world"));
        assert_eq!(entry.entity_id.as_deref(), Some("lake_pavilion"));
        assert_eq!(entry.group_id.as_deref(), Some("lake_pavilion_story"));
        assert_eq!(entry.camera_preset.as_deref(), Some("lake_pavilion_focus"));
        assert_eq!(entry.panel_id, "basemap");
        assert_eq!(entry.block_path.as_deref(), Some("0"));
    }

    #[test]
    fn merge_viewpoint_entry_keeps_more_specific_stage_carrier() {
        let existing = ViewpointMapEntry {
            tier: "t0".to_string(),
            panel_id: "basemap".to_string(),
            block_path: Some("0".to_string()),
            label: Some("湖心亭".to_string()),
            view_family: Some("map".to_string()),
            stage_kind: Some("map-stage".to_string()),
            world_ref: Some("park_world".to_string()),
            entity_id: Some("lake_pavilion".to_string()),
            group_id: Some("lake_pavilion_story".to_string()),
            camera_preset: Some("lake_pavilion_focus".to_string()),
        };
        let candidate = ViewpointMapEntry {
            tier: "t1".to_string(),
            panel_id: "lake_visitors_card".to_string(),
            block_path: Some("0/1".to_string()),
            label: None,
            view_family: None,
            stage_kind: None,
            world_ref: None,
            entity_id: None,
            group_id: None,
            camera_preset: None,
        };
        let merged = merge_viewpoint_entry(Some(existing), candidate);
        assert_eq!(merged.panel_id, "basemap");
        assert_eq!(merged.tier, "t0");
        assert_eq!(merged.world_ref.as_deref(), Some("park_world"));
    }

    #[test]
    fn presentation_map_serializes_default_script_as_camel_case() {
        let default_script = json!({
            "id": "deck-default",
            "title": "Deck 默认讲稿",
            "steps": [{
                "id": "cover",
                "title": "封面",
                "caption": "开场",
                "speaker_notes": "欢迎",
                "actions": [
                    {"type": "show_page", "pageId": "slide-01-cover"},
                    {"type": "highlight", "viewpoint": "cover-title"}
                ]
            }]
        });
        let map = build_presentation_map_with_default_script(
            "intro",
            &[],
            &BTreeMap::new(),
            Some(default_script.clone()),
        );
        let value = presentation_map_to_value(&map);
        assert_eq!(value.get("defaultScript"), Some(&default_script));
        assert!(value.get("default_script").is_none());
        assert!(value.get("deck").is_none());
    }
}
