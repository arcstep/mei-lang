//! Phase 3 ABI projector: legacy scene_contract / ui_layout / presentation_map → ABI.
//!
//! Also computes structure_digest / narration_digest and emits Gate 3 diagnostics.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use serde_json::Value;

use super::content_capability_abi::ContentCapability;
use super::contract::SceneContract;
use super::diagnostic::{Diagnostic, Severity};
use super::narration_abi::NarrationCatalog;
use super::presentation_map_schema::accept_presentation_map;
use super::scene_slot_abi::{
    SceneSlotModule, SceneSlotModuleId, SemanticSlotDecl, SlotCardinality,
};
use super::stage_program::StageProgramIndex;
use super::stage_registry::StageProfile;
use super::ui::UiTreeNode;
use super::ui_layout_index::{UiLayoutIndex, UiScopeRole};
use super::ui_node::UiNodeDecl;

/// Inputs for projecting ABI for the active assemble/compile scope.
#[derive(Debug, Clone, Default)]
pub struct AbiProjectionInput<'a> {
    pub stage_id: Option<&'a str>,
    pub stage_source_anchor: Option<&'a str>,
    pub profile: Option<StageProfile>,
    pub scene_contract: Option<&'a SceneContract>,
    pub ui_layout_index: Option<&'a UiLayoutIndex>,
    /// Full presentation_map Value (deck + viewpoints + defaultScript).
    pub presentation_map: Option<&'a Value>,
}

/// Projected ABI bundles keyed for CompiledApp.
#[derive(Debug, Clone, Default)]
pub struct AbiProjection {
    pub scene_slot_modules: BTreeMap<String, SceneSlotModule>,
    pub content_capabilities: BTreeMap<String, ContentCapability>,
    pub narration_catalogs: BTreeMap<String, NarrationCatalog>,
    pub diagnostics: Vec<Diagnostic>,
}

fn hash_stable(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Structure digest: slot modules + capabilities (excludes narration caption/notes/timing).
pub fn compute_structure_digest(
    slot_modules: &BTreeMap<String, SceneSlotModule>,
    capabilities: &BTreeMap<String, ContentCapability>,
) -> String {
    let mut parts = Vec::new();
    for (id, module) in slot_modules {
        let mut slot_ids: Vec<_> = module.slot_ids();
        slot_ids.sort();
        parts.push(format!(
            "module:{id}:{}:{}",
            module.version,
            slot_ids.join(",")
        ));
        for slot in &module.slots {
            let mut caps = slot.accepted_capability_ids.clone();
            caps.sort();
            parts.push(format!(
                "slot:{}:req={}:card={}-{:?}:caps={}",
                slot.slot_id,
                slot.required,
                slot.cardinality.min,
                slot.cardinality.max,
                caps.join(",")
            ));
        }
    }
    for (id, cap) in capabilities {
        let mut private = cap.private_child_ids.clone();
        private.sort();
        parts.push(format!(
            "cap:{id}:v{}:fill={}:private={}",
            cap.version,
            cap.supports_fill,
            private.join(",")
        ));
    }
    parts.sort();
    hash_stable(&parts.join("\n"))
}

/// Narration digest: catalog tracks/cues including caption/notes/timing.
pub fn compute_narration_digest(catalogs: &BTreeMap<String, NarrationCatalog>) -> String {
    let mut parts = Vec::new();
    for (id, catalog) in catalogs {
        parts.push(format!("catalog:{id}"));
        for track in &catalog.tracks {
            parts.push(format!(
                "track:{}:{}:{:?}:{:?}:{}",
                track.id, track.title, track.default_for, track.default_timing_ms, track.digest
            ));
            for cue in &track.cues {
                parts.push(format!(
                    "cue:{}:{}:body={:?}:cap={:?}:notes={:?}:actions={:?}:t={:?}:anchor={}",
                    cue.id,
                    cue.target_ref,
                    cue.body,
                    cue.caption,
                    cue.speaker_notes,
                    cue.actions,
                    cue.timing,
                    cue.source_anchor,
                ));
            }
        }
    }
    parts.sort();
    hash_stable(&parts.join("\n"))
}

fn is_t2_or_overlay_scope(preview_scope: &str) -> bool {
    let s = preview_scope.replace('\\', "/").to_ascii_lowercase();
    s.contains("/t2/") || s.contains("/overlay/") || s.contains("/t2") || s.contains("/plane-")
}

fn content_key_from_preview(preview_scope: &str) -> String {
    preview_scope
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(preview_scope)
        .to_string()
}

fn collect_panel_children(panel: &UiNodeDecl, out: &mut Vec<String>) {
    for node in &panel.blocks {
        if let UiTreeNode::Panel(child) = node {
            out.push(child.id.clone());
            collect_panel_children(child, out);
        }
    }
}

fn walk_panels_for_content(panel: &UiNodeDecl, out: &mut Vec<(String, String, Vec<String>)>) {
    let role = panel
        .props
        .get("__mei_ui_role")
        .or_else(|| panel.props.get("ui_role"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let looks_content = role == "content"
        || panel
            .props
            .get("__mei_layout_fill")
            .and_then(|v| v.as_bool())
            == Some(true)
        || panel.kind == "content_panel"
        || panel.id.contains("-compound")
        || panel.id.ends_with("-metric")
        || panel.id.starts_with("mini-");

    if looks_content {
        let mut private = Vec::new();
        collect_panel_children(panel, &mut private);
        // Drop self id if present
        private.retain(|id| id != &panel.id);
        let anchor = panel
            .props
            .get("__mei_source_file")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        out.push((panel.id.clone(), anchor, private));
    }
    for node in &panel.blocks {
        if let UiTreeNode::Panel(child) = node {
            walk_panels_for_content(child, out);
        }
    }
}

/// Project ABI from layout index + optional scene_contract / presentation_map.
pub fn project_abi(input: &AbiProjectionInput<'_>) -> AbiProjection {
    let mut out = AbiProjection::default();
    let stage_id = input.stage_id.unwrap_or("default");
    let source_anchor = input.stage_source_anchor.unwrap_or("").replace('\\', "/");
    let module_id = SceneSlotModuleId::for_stage(stage_id);
    let profile = input.profile.unwrap_or(StageProfile::Cockpit);

    let presentation_map = match input.presentation_map {
        Some(value) => match accept_presentation_map(value) {
            Ok(accepted) => accepted,
            Err(message) => {
                out.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "presentation_map.schema".to_string(),
                    message,
                    source_path: if source_anchor.is_empty() {
                        None
                    } else {
                        Some(source_anchor.clone())
                    },
                });
                None
            }
        },
        None => None,
    };

    let mut slots = Vec::new();
    let mut capabilities: BTreeMap<String, ContentCapability> = BTreeMap::new();
    let mut seen_slots = BTreeSet::new();

    // Prefer ui_layout_index Content nodes (public boundary).
    if let Some(index) = input.ui_layout_index {
        for node in index.nodes.values() {
            if node.role != UiScopeRole::Content {
                continue;
            }
            if is_t2_or_overlay_scope(&node.preview_scope) {
                continue;
            }
            // Nested Content under Content = private card/field; not a public Stage slot.
            // Content under Slot = metric-card field / layout cell — also private.
            if let Some(parent_id) = node.parent_id.as_deref() {
                if index
                    .nodes
                    .get(parent_id)
                    .is_some_and(|p| matches!(p.role, UiScopeRole::Content | UiScopeRole::Slot))
                {
                    continue;
                }
            }
            let slot_id = content_key_from_preview(&node.preview_scope);
            if slot_id.is_empty() || !seen_slots.insert(slot_id.clone()) {
                continue;
            }
            let def_anchor = node
                .source_anchors
                .first()
                .map(|a| {
                    if a.symbol_id.is_empty() {
                        a.file.clone()
                    } else {
                        format!("{}#{}", a.file, a.symbol_id)
                    }
                })
                .unwrap_or_else(|| source_anchor.clone());
            let call_site = node.parent_id.clone();
            // Nested cards under compound content: if content_kind suggests compound
            // and label has children in index under same parent, keep private via
            // scene_contract walk below.
            slots.push(SemanticSlotDecl {
                slot_id: slot_id.clone(),
                required: true,
                cardinality: SlotCardinality::required_one(),
                accepted_capability_ids: vec![slot_id.clone()],
                anchors: Vec::new(),
                call_site_anchor: call_site,
                source_anchor: def_anchor.clone(),
                slide_unit_id: if profile == StageProfile::Slides {
                    // Bind to nearest slide ancestor if present.
                    index
                        .ancestor_chain(&node.node_id)
                        .into_iter()
                        .rev()
                        .find(|n| n.role == UiScopeRole::Slide)
                        .map(|n| content_key_from_preview(&n.preview_scope))
                } else {
                    None
                },
            });
            capabilities.entry(slot_id.clone()).or_insert_with(|| {
                ContentCapability::from_content_panel(&slot_id, &def_anchor, Vec::new())
            });
        }
    }

    // Enrich private children + ensure content panels from scene_contract.
    if let Some(contract) = input.scene_contract {
        let mut found = Vec::new();
        for panel in &contract.panels {
            walk_panels_for_content(panel, &mut found);
        }
        // Pass 1: ensure capabilities for exports and record private child sets.
        let mut private_of: BTreeSet<String> = BTreeSet::new();
        for (id, anchor, private) in &found {
            if is_t2_or_overlay_scope(id) || id.contains("/t2/") {
                continue;
            }
            let anchor = if anchor.is_empty() {
                source_anchor.clone()
            } else {
                anchor.clone()
            };
            for p in private {
                private_of.insert(p.clone());
            }
            capabilities
                .entry(id.clone())
                .and_modify(|cap| {
                    if cap.private_child_ids.is_empty() && !private.is_empty() {
                        cap.private_child_ids = private.clone();
                    }
                })
                .or_insert_with(|| {
                    ContentCapability::from_content_panel(id, &anchor, private.clone())
                });
        }
        // Pass 2: only promote non-private panels as public slots.
        for (id, anchor, private) in found {
            if is_t2_or_overlay_scope(&id) || id.contains("/t2/") {
                continue;
            }
            if private_of.contains(&id) {
                capabilities.remove(&id);
                continue;
            }
            let anchor = if anchor.is_empty() {
                source_anchor.clone()
            } else {
                anchor
            };
            if seen_slots.insert(id.clone()) {
                slots.push(SemanticSlotDecl {
                    slot_id: id.clone(),
                    required: true,
                    cardinality: SlotCardinality::required_one(),
                    accepted_capability_ids: vec![id.clone()],
                    anchors: Vec::new(),
                    call_site_anchor: None,
                    source_anchor: anchor.clone(),
                    slide_unit_id: None,
                });
                capabilities.entry(id.clone()).or_insert_with(|| {
                    ContentCapability::from_content_panel(&id, &anchor, private)
                });
            } else if let Some(cap) = capabilities.get_mut(&id) {
                if cap.private_child_ids.is_empty() && !private.is_empty() {
                    cap.private_child_ids = private;
                }
            }
        }
        // Drop layout-promoted slots that are private children of a content export.
        if !private_of.is_empty() {
            slots.retain(|s| !private_of.contains(&s.slot_id));
            for id in &private_of {
                capabilities.remove(id);
            }
            seen_slots.retain(|id| !private_of.contains(id));
        }
    }

    // Slides: project viewpoints as slide-local slots when layout had none.
    if profile == StageProfile::Slides {
        if let Some(map) = presentation_map {
            if let Some(viewpoints) = map.get("viewpoints").and_then(|v| v.as_object()) {
                for vp_id in viewpoints.keys() {
                    if !seen_slots.insert(vp_id.clone()) {
                        continue;
                    }
                    let panel_id = viewpoints
                        .get(vp_id)
                        .and_then(|v| v.get("panelId").or_else(|| v.get("panel_id")))
                        .and_then(|v| v.as_str())
                        .unwrap_or(vp_id.as_str());
                    slots.push(SemanticSlotDecl {
                        slot_id: vp_id.clone(),
                        required: false,
                        cardinality: SlotCardinality::default(),
                        accepted_capability_ids: vec![panel_id.to_string()],
                        anchors: vec![vp_id.clone()],
                        call_site_anchor: Some(format!("viewpoint:{vp_id}")),
                        source_anchor: source_anchor.clone(),
                        slide_unit_id: None,
                    });
                    capabilities.entry(panel_id.to_string()).or_insert_with(|| {
                        ContentCapability::from_content_panel(panel_id, &source_anchor, Vec::new())
                    });
                }
            }
        }
    }

    // Stable slot order by id.
    slots.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));

    // Compound content: private nested cards must not remain public slots/capabilities.
    let mut private_ids: BTreeSet<String> = BTreeSet::new();
    for cap in capabilities.values() {
        for id in &cap.private_child_ids {
            private_ids.insert(id.clone());
        }
    }
    if !private_ids.is_empty() {
        slots.retain(|s| !private_ids.contains(&s.slot_id));
        for id in &private_ids {
            // Keep capability entry only if it is itself a public export (not only a child).
            // Children that were mistakenly promoted as capabilities are dropped.
            if slots.iter().all(|s| &s.slot_id != id) {
                capabilities.remove(id);
            }
        }
        seen_slots.retain(|id| !private_ids.contains(id));
    }

    let module = SceneSlotModule {
        module_id: module_id.clone(),
        version: "1".to_string(),
        slots,
        compatible_surfaces: vec!["access".to_string()],
        source_anchor: source_anchor.clone(),
    };
    out.scene_slot_modules
        .insert(module_id.as_str().to_string(), module);
    for cap in capabilities.values_mut() {
        cap.maybe_mark_world_from_id();
    }
    out.content_capabilities = capabilities;

    out
}

/// Validate fills implied by StageProgram units against Slot ABI (Gate 3 diagnostics).
pub fn validate_abi_against_programs(
    programs: &StageProgramIndex,
    slot_modules: &BTreeMap<String, SceneSlotModule>,
    capabilities: &BTreeMap<String, ContentCapability>,
    _narration_catalogs: &BTreeMap<String, NarrationCatalog>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for program in programs.programs.values() {
        let module_key = format!("scene:{}", program.stage_id.as_str());
        let Some(module) = slot_modules.get(&module_key) else {
            // Cockpit without layout yet is ok; Slides may only have viewpoint slots.
            continue;
        };
        // Cardinality: each required slot must have ≥ min accepted fills.
        // Projection treats each slot as self-filled once via accepted_capability_ids.
        for slot in &module.slots {
            let fill_count = if capabilities.contains_key(&slot.slot_id)
                || slot
                    .accepted_capability_ids
                    .iter()
                    .any(|c| capabilities.contains_key(c))
            {
                1u32
            } else {
                0
            };
            if !slot.cardinality.allows_count(fill_count) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "slot_cardinality".to_string(),
                    message: format!(
                        "slot `{}` fill count {fill_count} outside cardinality {}-{:?}",
                        slot.slot_id, slot.cardinality.min, slot.cardinality.max
                    ),
                    source_path: Some(slot.source_anchor.clone()),
                });
            }
            for cap_id in &slot.accepted_capability_ids {
                if !capabilities.contains_key(cap_id) && fill_count > 0 {
                    // accepted lists the intended capability; missing capability is mismatch
                    // only when we claim a fill.
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "capability_mismatch".to_string(),
                        message: format!(
                            "slot `{}` accepts capability `{cap_id}` which is not projected",
                            slot.slot_id
                        ),
                        source_path: Some(slot.source_anchor.clone()),
                    });
                }
            }
            if slot.source_anchor.is_empty() && slot.call_site_anchor.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "abi_source_anchor_incomplete".to_string(),
                    message: format!(
                        "slot `{}` missing call-site or definition source anchor",
                        slot.slot_id
                    ),
                    source_path: None,
                });
            }
        }
    }
    diagnostics
}

/// Apply projection onto StagePrograms (refs + digests).
pub fn bind_programs_to_abi(programs: &mut StageProgramIndex, projection: &AbiProjection) {
    let structure = compute_structure_digest(
        &projection.scene_slot_modules,
        &projection.content_capabilities,
    );
    for (stage_id, program) in programs.programs.iter_mut() {
        let module_key = format!("scene:{stage_id}");
        program.slot_module_ref = projection
            .scene_slot_modules
            .contains_key(&module_key)
            .then_some(module_key.clone());
        let cap_ids: Vec<_> = projection.content_capabilities.keys().cloned().collect();
        program.capability_ref = if cap_ids.is_empty() {
            None
        } else {
            Some(format!("caps:{}", hash_stable(&cap_ids.join(","))))
        };
        let narr_key = format!("narration:{stage_id}");
        program.narration_ref = projection
            .narration_catalogs
            .contains_key(&narr_key)
            .then_some(narr_key);
        program.structure_digest = Some(structure.clone());
        let narr_digest = if let Some(cat) = projection
            .narration_catalogs
            .get(&format!("narration:{stage_id}"))
        {
            let mut one = BTreeMap::new();
            one.insert(format!("narration:{stage_id}"), cat.clone());
            Some(compute_narration_digest(&one))
        } else {
            Some(compute_narration_digest(&BTreeMap::new()))
        };
        program.narration_digest = narr_digest;
    }
}

/// Emit `slot_missing` when a referenced slot_id is absent (for explicit checks).
pub fn diagnose_slot_missing(
    module: &SceneSlotModule,
    referenced_slot_id: &str,
    source_path: Option<&str>,
) -> Option<Diagnostic> {
    if module.get_slot(referenced_slot_id).is_some() {
        return None;
    }
    Some(Diagnostic {
        severity: Severity::Error,
        code: "slot_missing".to_string(),
        message: format!(
            "referenced slot `{referenced_slot_id}` is not in module `{}`",
            module.module_id.as_str()
        ),
        source_path: source_path.map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::stage_program::StageProgramIndex;
    use crate::model::stage_registry::{StageDescriptor, StageId, StageProfile, StageRegistry};
    use crate::model::{NarrationCue, NarrationTrack};

    #[test]
    fn structure_digest_ignores_narration_caption_changes() {
        let mut slots = BTreeMap::new();
        slots.insert(
            "scene:home".to_string(),
            SceneSlotModule {
                module_id: SceneSlotModuleId::for_stage("home"),
                version: "1".to_string(),
                slots: vec![SemanticSlotDecl {
                    slot_id: "metric".to_string(),
                    required: true,
                    cardinality: SlotCardinality::required_one(),
                    accepted_capability_ids: vec!["metric".to_string()],
                    anchors: Vec::new(),
                    call_site_anchor: None,
                    source_anchor: "src/scene/home.mei".to_string(),
                    slide_unit_id: None,
                }],
                compatible_surfaces: vec!["access".to_string()],
                source_anchor: "src/scene/home.mei".to_string(),
            },
        );
        let mut caps = BTreeMap::new();
        caps.insert(
            "metric".to_string(),
            ContentCapability::from_content_panel("metric", "src/x.mei", Vec::new()),
        );
        let s1 = compute_structure_digest(&slots, &caps);
        let s2 = compute_structure_digest(&slots, &caps);
        assert_eq!(s1, s2);

        let mut narr_a = BTreeMap::new();
        narr_a.insert(
            "narration:home".to_string(),
            NarrationCatalog {
                catalog_id: "narration:home".to_string(),
                tracks: vec![NarrationTrack {
                    id: "t".to_string(),
                    title: "Track".to_string(),
                    scope: "app".to_string(),
                    cues: vec![NarrationCue {
                        id: "c1".to_string(),
                        target_ref: "stage:home/viewpoint:metric".to_string(),
                        caption: Some("A".to_string()),
                        source_anchor: "x".to_string(),
                        ..NarrationCue::default()
                    }],
                    source_anchor: "x".to_string(),
                    digest: "track-digest".to_string(),
                    ..NarrationTrack::default()
                }],
                ..NarrationCatalog::default()
            },
        );
        let mut narr_b = narr_a.clone();
        narr_b.get_mut("narration:home").unwrap().tracks[0].cues[0].caption = Some("B".to_string());
        let n1 = compute_narration_digest(&narr_a);
        let n2 = compute_narration_digest(&narr_b);
        assert_ne!(n1, n2);
        // structure unchanged by narration mutation
        assert_eq!(
            compute_structure_digest(&slots, &caps),
            compute_structure_digest(&slots, &caps)
        );
    }

    #[test]
    fn slot_missing_diagnostic() {
        let module = SceneSlotModule {
            module_id: SceneSlotModuleId::for_stage("home"),
            version: "1".to_string(),
            slots: vec![],
            compatible_surfaces: vec![],
            source_anchor: "x".to_string(),
        };
        let d = diagnose_slot_missing(&module, "metric", Some("x.mei")).unwrap();
        assert_eq!(d.code, "slot_missing");
    }

    #[test]
    fn capability_mismatch_when_accepted_cap_absent() {
        let mut modules = BTreeMap::new();
        modules.insert(
            "scene:home".to_string(),
            SceneSlotModule {
                module_id: SceneSlotModuleId::for_stage("home"),
                version: "1".to_string(),
                slots: vec![SemanticSlotDecl {
                    slot_id: "metric".to_string(),
                    required: true,
                    cardinality: SlotCardinality::required_one(),
                    accepted_capability_ids: vec!["missing-cap".to_string()],
                    anchors: Vec::new(),
                    call_site_anchor: Some("panel_ref".to_string()),
                    source_anchor: "src/scene/home.mei".to_string(),
                    slide_unit_id: None,
                }],
                compatible_surfaces: vec![],
                source_anchor: "src/scene/home.mei".to_string(),
            },
        );
        let mut caps = BTreeMap::new();
        // Slot is filled by same-id capability, but accepted list points elsewhere → mismatch.
        caps.insert(
            "metric".to_string(),
            ContentCapability::from_content_panel("metric", "src/x.mei", Vec::new()),
        );
        let desc = StageDescriptor {
            id: StageId::new("home"),
            profile: StageProfile::Cockpit,
            title: None,
            short_title: None,
            source_anchor: "src/scene/home.mei".to_string(),
            is_default: true,
            legacy_scene_id: "home".to_string(),
        };
        let mut programs = StageProgramIndex::default();
        let mut program = crate::model::stage_program::StageProgram::from_cockpit(&desc);
        program.slot_module_ref = Some("scene:home".to_string());
        programs.programs.insert("home".to_string(), program);
        let diags = validate_abi_against_programs(&programs, &modules, &caps, &BTreeMap::new());
        assert!(
            diags.iter().any(|d| d.code == "capability_mismatch"),
            "expected capability_mismatch, got {diags:?}"
        );
    }

    #[test]
    fn bind_programs_sets_digests() {
        let registry = StageRegistry {
            stages: vec![StageDescriptor {
                id: StageId::new("home"),
                profile: StageProfile::Cockpit,
                title: None,
                short_title: None,
                source_anchor: "src/scene/home.mei".to_string(),
                is_default: true,
                legacy_scene_id: "home".to_string(),
            }],
            default_stage_id: Some(StageId::new("home")),
        };
        let mut programs = StageProgramIndex::from_registry(&registry, &BTreeMap::new());
        let mut projection = AbiProjection::default();
        projection.scene_slot_modules.insert(
            "scene:home".to_string(),
            SceneSlotModule {
                module_id: SceneSlotModuleId::for_stage("home"),
                version: "1".to_string(),
                slots: vec![],
                compatible_surfaces: vec![],
                source_anchor: "src/scene/home.mei".to_string(),
            },
        );
        bind_programs_to_abi(&mut programs, &projection);
        let p = programs.get("home").unwrap();
        assert_eq!(p.slot_module_ref.as_deref(), Some("scene:home"));
        assert!(p.structure_digest.is_some());
        assert!(p.narration_digest.is_some());
    }
}
