use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

const RECIPE_SCHEMA_VERSION: &str = "mei-stock-object-recipe-v1";
const STOCK_RECIPE_SOURCE: &str = "stock/templates/cockpit/object-recipes.mei";
const OVERRIDE_PRECEDENCE: &[&str] = &[
    "local",
    "domain",
    "app",
    "stock",
    "placeholder",
    "no_projection",
];

#[derive(Clone, Copy)]
struct SlotSpec {
    name: &'static str,
    required: bool,
    missing: &'static str,
}

#[derive(Clone, Copy)]
struct ProjectionSpec {
    role: &'static str,
    id: &'static str,
    required: &'static [&'static str],
    optional: &'static [&'static str],
    partial: &'static str,
    absent: &'static str,
    reuses: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct InteractionSpec {
    trigger: &'static str,
    intents: &'static [&'static str],
    subject_kind: &'static str,
    projection_role: &'static str,
    requires: &'static [&'static str],
    target_slot: Option<&'static str>,
    selection_mode: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct ResponderSpec {
    role: &'static str,
    intents: &'static [&'static str],
    projection_role: &'static str,
    requires: &'static [&'static str],
    target_slot: Option<&'static str>,
}

pub(crate) struct RecipeAssembly {
    pub contract: Value,
    pub projections: Vec<Value>,
    pub interaction_bindings: Vec<Value>,
    pub responders: Vec<Value>,
    pub diagnostics: Vec<Value>,
    pub effective_override: Value,
    pub override_sources: BTreeMap<String, String>,
}

pub(crate) fn known_slots(recipe: &str) -> &'static [&'static str] {
    match recipe {
        "alert" => &[
            "label",
            "severity",
            "occurredAt",
            "status",
            "place",
            "detail",
            "explain",
        ],
        "case" => &[
            "label",
            "status",
            "occurredAt",
            "attachments",
            "evidence",
            "result",
            "detail",
        ],
        "place" => &[
            "label",
            "entityId",
            "viewpoint",
            "world",
            "rough3d",
            "narration",
        ],
        "event" => &[
            "label",
            "occurredAt",
            "severity",
            "playbackAt",
            "media",
            "place",
            "chart",
            "t2",
            "detail",
        ],
        _ => &[],
    }
}

pub(crate) fn assemble(
    recipe: &str,
    object_type_id: &str,
    intent_id: &str,
    source_anchor: &str,
    slots: &BTreeMap<String, Value>,
    override_props: &Value,
) -> Result<RecipeAssembly, String> {
    let slots_contract = slot_specs(recipe);
    let projection_specs = projection_specs(recipe);
    let interaction_specs = interaction_specs(recipe);
    let responder_specs = responder_specs(recipe);
    let contract = recipe_contract(
        recipe,
        slots_contract,
        projection_specs,
        interaction_specs,
        responder_specs,
    );
    let projections = assemble_projections(projection_specs, slots);
    let mut diagnostics = Vec::new();
    let missing_required = slots_contract
        .iter()
        .filter(|slot| slot.required && !slots.contains_key(slot.name))
        .map(|slot| slot.name)
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        diagnostics.push(json!({
            "code": "object_recipe_required_slots_missing",
            "severity": "warning",
            "message": format!(
                "recipe `cockpit.{recipe}` is missing required slots {}; affected projections are degraded or placeholder-only",
                missing_required.join(", ")
            ),
            "source_anchor": source_anchor,
        }));
    }

    let interaction_bindings = interaction_specs
        .iter()
        .filter_map(|spec| {
            active_contract_target(spec.projection_role, spec.requires, slots, &projections).map(
                |projection| {
                    let target = spec
                        .target_slot
                        .and_then(|slot| slots.get(slot))
                        .cloned()
                        .unwrap_or_else(|| projection["projection"].clone());
                    json!({
                        "id": format!("{intent_id}:{}", spec.trigger),
                        "trigger": spec.trigger,
                        "intents": spec.intents,
                        "objectType": object_type_id,
                        "subjectKind": spec.subject_kind,
                        "target": target,
                        "targetRole": spec.projection_role,
                        "selectionMode": spec.selection_mode,
                        "priority": if spec.target_slot.is_some() { "explicit_link" } else { "derived" },
                        "derived": true,
                        "legacyDoubleFire": true,
                        "source_anchor": source_anchor,
                    })
                },
            )
        })
        .collect();
    let responders = responder_specs
        .iter()
        .filter_map(|spec| {
            active_contract_target(spec.projection_role, spec.requires, slots, &projections).map(
                |projection| {
                    let target = spec
                        .target_slot
                        .and_then(|slot| slots.get(slot))
                        .cloned()
                        .unwrap_or_else(|| projection["projection"].clone());
                    json!({
                        "id": format!("{intent_id}:{}", spec.role),
                        "objectType": object_type_id,
                        "role": spec.role,
                        "intents": spec.intents,
                        "target": target,
                        "derived": true,
                        "source_anchor": source_anchor,
                    })
                },
            )
        })
        .collect();
    let (effective_override, override_sources) =
        resolve_overrides(recipe, override_props).map_err(str::to_string)?;

    Ok(RecipeAssembly {
        contract,
        projections,
        interaction_bindings,
        responders,
        diagnostics,
        effective_override,
        override_sources,
    })
}

fn recipe_contract(
    recipe: &str,
    slots: &[SlotSpec],
    projections: &[ProjectionSpec],
    interactions: &[InteractionSpec],
    responders: &[ResponderSpec],
) -> Value {
    json!({
        "schema_version": RECIPE_SCHEMA_VERSION,
        "id": format!("cockpit.{recipe}"),
        "slots": slots.iter().map(|slot| json!({
            "name": slot.name,
            "requirement": if slot.required { "required" } else { "optional" },
            "missing": slot.missing,
        })).collect::<Vec<_>>(),
        "projections": projections.iter().map(|projection| json!({
            "role": projection.role,
            "id": projection.id,
            "required_slots": projection.required,
            "optional_slots": projection.optional,
            "partial_behavior": projection.partial,
            "absent_behavior": projection.absent,
            "reuses": reuse_refs(projection.reuses),
        })).collect::<Vec<_>>(),
        "interactions": interactions.iter().map(|interaction| json!({
            "trigger": interaction.trigger,
            "intents": interaction.intents,
            "subject_kind": interaction.subject_kind,
            "projection_role": interaction.projection_role,
            "requires_slots": interaction.requires,
            "selection_mode": interaction.selection_mode,
        })).collect::<Vec<_>>(),
        "responders": responders.iter().map(|responder| json!({
            "role": responder.role,
            "intents": responder.intents,
            "projection_role": responder.projection_role,
            "requires_slots": responder.requires,
        })).collect::<Vec<_>>(),
        "override_precedence": OVERRIDE_PRECEDENCE,
        "identity_locked": true,
        "privacy_notice": (recipe == "case").then_some(
            "attachments and evidence are PII-redacted by default; owners must opt in to reveal"
        ),
        "source_anchor": STOCK_RECIPE_SOURCE,
    })
}

fn assemble_projections(specs: &[ProjectionSpec], slots: &BTreeMap<String, Value>) -> Vec<Value> {
    specs
        .iter()
        .map(|spec| {
            let expected = spec
                .required
                .iter()
                .chain(spec.optional.iter())
                .copied()
                .collect::<Vec<_>>();
            let inputs = expected
                .iter()
                .filter_map(|slot| {
                    slots
                        .get(*slot)
                        .map(|value| ((*slot).to_string(), value.clone()))
                })
                .collect::<Map<_, _>>();
            let present_required = spec
                .required
                .iter()
                .filter(|slot| slots.contains_key(**slot))
                .count();
            let state = if present_required == spec.required.len()
                && (!spec.required.is_empty() || !inputs.is_empty())
            {
                "ready"
            } else if present_required == 0 {
                spec.absent
            } else {
                spec.partial
            };
            let missing_slots = spec
                .required
                .iter()
                .filter(|slot| !slots.contains_key(**slot))
                .copied()
                .collect::<Vec<_>>();
            json!({
                "role": spec.role,
                "projection": projection_ref("stock_projection_ref", spec.id),
                "state": state,
                "inputs": inputs,
                "missing_slots": missing_slots,
                "reuses": reuse_refs(spec.reuses),
            })
        })
        .collect()
}

fn active_contract_target<'a>(
    projection_role: &str,
    requires: &[&str],
    slots: &BTreeMap<String, Value>,
    projections: &'a [Value],
) -> Option<&'a Value> {
    if !requires.iter().all(|slot| slots.contains_key(*slot)) {
        return None;
    }
    projections.iter().find(|projection| {
        projection["role"] == projection_role
            && matches!(projection["state"].as_str(), Some("ready" | "degraded"))
    })
}

fn resolve_overrides(
    recipe: &str,
    authored: &Value,
) -> Result<(Value, BTreeMap<String, String>), &'static str> {
    let mut effective = stock_defaults(recipe);
    let mut sources = BTreeMap::new();
    record_leaf_sources("", &effective, "stock", &mut sources);
    let Some(authored) = authored.as_object() else {
        return Ok((effective, sources));
    };
    reject_identity_override(authored)?;

    for layer in ["app", "domain"] {
        if let Some(value) = authored.get(layer) {
            let Some(value) = value.as_object() else {
                return Err("override app/domain/local layers must be maps");
            };
            merge_layer(&mut effective, value, layer, &mut sources);
        }
    }
    let local_flat = authored
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "app" | "domain" | "local"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Map<_, _>>();
    merge_layer(&mut effective, &local_flat, "local", &mut sources);
    if let Some(value) = authored.get("local") {
        let Some(value) = value.as_object() else {
            return Err("override app/domain/local layers must be maps");
        };
        merge_layer(&mut effective, value, "local", &mut sources);
    }
    Ok((effective, sources))
}

fn reject_identity_override(value: &Map<String, Value>) -> Result<(), &'static str> {
    for (key, value) in value {
        let normalized = key.replace(['_', '-'], "").to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "identity" | "identityfield" | "objectid" | "objecttype" | "source"
        ) {
            return Err("object recipe override cannot change identity, object type, or source");
        }
        match value {
            Value::Object(child) => reject_identity_override(child)?,
            Value::Array(items) => {
                for item in items {
                    if let Value::Object(child) = item {
                        reject_identity_override(child)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn merge_layer(
    target: &mut Value,
    layer: &Map<String, Value>,
    layer_name: &str,
    sources: &mut BTreeMap<String, String>,
) {
    let target = target
        .as_object_mut()
        .expect("stock defaults are an object");
    for (key, value) in layer {
        if let (Some(Value::Object(target_child)), Value::Object(source_child)) =
            (target.get_mut(key), value)
        {
            merge_object(target_child, source_child, key, layer_name, sources);
        } else {
            target.insert(key.clone(), value.clone());
            record_leaf_sources(key, value, layer_name, sources);
        }
    }
}

fn merge_object(
    target: &mut Map<String, Value>,
    source: &Map<String, Value>,
    prefix: &str,
    layer_name: &str,
    sources: &mut BTreeMap<String, String>,
) {
    for (key, value) in source {
        let path = format!("{prefix}.{key}");
        if let (Some(Value::Object(target_child)), Value::Object(source_child)) =
            (target.get_mut(key), value)
        {
            merge_object(target_child, source_child, &path, layer_name, sources);
        } else {
            target.insert(key.clone(), value.clone());
            record_leaf_sources(&path, value, layer_name, sources);
        }
    }
}

fn record_leaf_sources(
    prefix: &str,
    value: &Value,
    layer: &str,
    sources: &mut BTreeMap<String, String>,
) {
    if let Value::Object(entries) = value {
        for (key, value) in entries {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            record_leaf_sources(&path, value, layer, sources);
        }
    } else if !prefix.is_empty() {
        sources.insert(prefix.to_string(), layer.to_string());
    }
}

fn stock_defaults(recipe: &str) -> Value {
    match recipe {
        "alert" => json!({"projectionPolicy": "hide_missing"}),
        "case" => json!({"privacyMode": "pii_redacted"}),
        "place" => json!({"spatialMode": "poi_first"}),
        "event" => json!({"selectionMode": "secondary"}),
        _ => json!({}),
    }
}

fn projection_ref(kind: &str, id: &str) -> Value {
    json!({
        "role": "stock",
        "kind": kind,
        "id": id,
        "source_anchor": STOCK_RECIPE_SOURCE,
    })
}

fn reuse_refs(ids: &[&str]) -> Vec<Value> {
    ids.iter()
        .map(|id| {
            id.strip_prefix("template:")
                .map(|id| projection_ref("stock_template_ref", id))
                .unwrap_or_else(|| projection_ref("component_ref", id))
        })
        .collect()
}

fn slot_specs(recipe: &str) -> &'static [SlotSpec] {
    match recipe {
        "alert" => &[
            SlotSpec {
                name: "label",
                required: true,
                missing: "degrade_label",
            },
            SlotSpec {
                name: "severity",
                required: true,
                missing: "hide_severity",
            },
            SlotSpec {
                name: "occurredAt",
                required: true,
                missing: "hide_timestamp",
            },
            SlotSpec {
                name: "status",
                required: true,
                missing: "hide_status",
            },
            SlotSpec {
                name: "place",
                required: false,
                missing: "hide_map",
            },
            SlotSpec {
                name: "detail",
                required: false,
                missing: "hide_detail",
            },
            SlotSpec {
                name: "explain",
                required: false,
                missing: "hide_explain",
            },
        ],
        "case" => &[
            SlotSpec {
                name: "label",
                required: true,
                missing: "degrade_label",
            },
            SlotSpec {
                name: "status",
                required: true,
                missing: "hide_status",
            },
            SlotSpec {
                name: "occurredAt",
                required: true,
                missing: "hide_timestamp",
            },
            SlotSpec {
                name: "attachments",
                required: false,
                missing: "hide_attachments",
            },
            SlotSpec {
                name: "evidence",
                required: false,
                missing: "hide_evidence",
            },
            SlotSpec {
                name: "result",
                required: false,
                missing: "hide_result",
            },
            SlotSpec {
                name: "detail",
                required: false,
                missing: "hide_detail",
            },
        ],
        "place" => &[
            SlotSpec {
                name: "label",
                required: true,
                missing: "degrade_label",
            },
            SlotSpec {
                name: "entityId",
                required: true,
                missing: "degrade_to_viewpoint",
            },
            SlotSpec {
                name: "viewpoint",
                required: true,
                missing: "hide_spatial_entry",
            },
            SlotSpec {
                name: "world",
                required: false,
                missing: "hide_world_entry",
            },
            SlotSpec {
                name: "rough3d",
                required: false,
                missing: "hide_rough3d_proxy",
            },
            SlotSpec {
                name: "narration",
                required: false,
                missing: "hide_narration_card",
            },
        ],
        "event" => &[
            SlotSpec {
                name: "label",
                required: true,
                missing: "degrade_label",
            },
            SlotSpec {
                name: "occurredAt",
                required: true,
                missing: "hide_timestamp",
            },
            SlotSpec {
                name: "severity",
                required: false,
                missing: "hide_severity",
            },
            SlotSpec {
                name: "playbackAt",
                required: false,
                missing: "hide_timeline",
            },
            SlotSpec {
                name: "media",
                required: false,
                missing: "hide_media",
            },
            SlotSpec {
                name: "place",
                required: false,
                missing: "hide_map",
            },
            SlotSpec {
                name: "chart",
                required: false,
                missing: "hide_chart",
            },
            SlotSpec {
                name: "t2",
                required: false,
                missing: "hide_t2",
            },
            SlotSpec {
                name: "detail",
                required: false,
                missing: "hide_detail",
            },
        ],
        _ => &[],
    }
}

fn projection_specs(recipe: &str) -> &'static [ProjectionSpec] {
    match recipe {
        "alert" => &[
            ProjectionSpec {
                role: "list",
                id: "cockpit.alert.list",
                required: &["label", "severity", "occurredAt", "status"],
                optional: &[],
                partial: "degraded",
                absent: "placeholder",
                reuses: &["cockpit.data-table"],
            },
            ProjectionSpec {
                role: "map",
                id: "cockpit.alert.map",
                required: &["place"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["map.maplibre"],
            },
            ProjectionSpec {
                role: "detail",
                id: "cockpit.alert.detail",
                required: &["detail"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["mei.text"],
            },
            ProjectionSpec {
                role: "explain",
                id: "cockpit.alert.explain",
                required: &["explain"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["dataset.summary-cards"],
            },
        ],
        "case" => &[
            ProjectionSpec {
                role: "list",
                id: "cockpit.case.list",
                required: &["label", "status", "occurredAt"],
                optional: &[],
                partial: "degraded",
                absent: "placeholder",
                reuses: &["cockpit.data-table"],
            },
            ProjectionSpec {
                role: "detail",
                id: "cockpit.case.detail",
                required: &["detail"],
                optional: &["result"],
                partial: "hidden",
                absent: "hidden",
                reuses: &["mei.text"],
            },
            ProjectionSpec {
                role: "attachments",
                id: "cockpit.case.attachments",
                required: &["attachments"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["dataset.table"],
            },
            ProjectionSpec {
                role: "evidence",
                id: "cockpit.case.evidence",
                required: &["evidence"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["cockpit.opinion-panel"],
            },
            ProjectionSpec {
                role: "result",
                id: "cockpit.case.result",
                required: &["result"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["mei.text"],
            },
        ],
        "place" => &[
            ProjectionSpec {
                role: "poi",
                id: "cockpit.place.poi",
                required: &["label", "entityId", "viewpoint"],
                optional: &[],
                partial: "degraded",
                absent: "placeholder",
                reuses: &["map.maplibre"],
            },
            ProjectionSpec {
                role: "world",
                id: "cockpit.place.world",
                required: &["world", "viewpoint"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["cockpit.world-stage"],
            },
            ProjectionSpec {
                role: "rough3d",
                id: "cockpit.place.rough3d",
                required: &["rough3d", "viewpoint"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["cockpit.world-stage"],
            },
            ProjectionSpec {
                role: "narration",
                id: "cockpit.place.narration",
                required: &["narration"],
                optional: &["viewpoint"],
                partial: "hidden",
                absent: "hidden",
                reuses: &["mei.text"],
            },
        ],
        "event" => &[
            ProjectionSpec {
                role: "list",
                id: "cockpit.event.list",
                required: &["label", "occurredAt"],
                optional: &["severity"],
                partial: "degraded",
                absent: "placeholder",
                reuses: &["cockpit.data-table", "thunder.event-summary"],
            },
            ProjectionSpec {
                role: "timeline",
                id: "cockpit.event.timeline",
                required: &["playbackAt"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["thunder.playback-strip"],
            },
            ProjectionSpec {
                role: "media",
                id: "cockpit.event.media",
                required: &["media"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["mei.text"],
            },
            ProjectionSpec {
                role: "map",
                id: "cockpit.event.map",
                required: &["place"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["map.maplibre", "thunder.map-sync"],
            },
            ProjectionSpec {
                role: "chart",
                id: "cockpit.event.chart",
                required: &["chart"],
                optional: &[],
                partial: "hidden",
                absent: "hidden",
                reuses: &["thunder.event-charts", "chart.line"],
            },
            ProjectionSpec {
                role: "t2",
                id: "cockpit.event.t2",
                required: &["t2"],
                optional: &["detail"],
                partial: "hidden",
                absent: "hidden",
                reuses: &[
                    "template:cockpit/t2/t2-link",
                    "template:cockpit/t2/t2-nav",
                    "mei.text",
                ],
            },
        ],
        _ => &[],
    }
}

fn interaction_specs(recipe: &str) -> &'static [InteractionSpec] {
    match recipe {
        "alert" => &[
            InteractionSpec {
                trigger: "row_click",
                intents: &["select"],
                subject_kind: "object_focus",
                projection_role: "list",
                requires: &[],
                target_slot: None,
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "detail_click",
                intents: &["select", "open_projection"],
                subject_kind: "object_focus",
                projection_role: "detail",
                requires: &["detail"],
                target_slot: Some("detail"),
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "map_world_pick",
                intents: &["select", "focus_viewpoint"],
                subject_kind: "object_focus",
                projection_role: "map",
                requires: &["place"],
                target_slot: Some("place"),
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "explain_click",
                intents: &["explain_metric"],
                subject_kind: "object_set",
                projection_role: "explain",
                requires: &["explain"],
                target_slot: Some("explain"),
                selection_mode: None,
            },
        ],
        "case" => &[
            InteractionSpec {
                trigger: "row_click",
                intents: &["select"],
                subject_kind: "object_focus",
                projection_role: "list",
                requires: &[],
                target_slot: None,
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "detail_click",
                intents: &["select", "open_projection"],
                subject_kind: "object_focus",
                projection_role: "detail",
                requires: &["detail"],
                target_slot: Some("detail"),
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "evidence_click",
                intents: &["select", "open_projection"],
                subject_kind: "object_focus",
                projection_role: "evidence",
                requires: &["evidence"],
                target_slot: Some("evidence"),
                selection_mode: None,
            },
        ],
        "place" => &[
            InteractionSpec {
                trigger: "map_world_pick",
                intents: &["select", "focus_viewpoint"],
                subject_kind: "object_focus",
                projection_role: "poi",
                requires: &["viewpoint"],
                target_slot: Some("viewpoint"),
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "world_entry_click",
                intents: &["select", "focus_viewpoint"],
                subject_kind: "object_focus",
                projection_role: "world",
                requires: &["world", "viewpoint"],
                target_slot: Some("viewpoint"),
                selection_mode: None,
            },
            InteractionSpec {
                trigger: "narration_click",
                intents: &["open_projection"],
                subject_kind: "object_focus",
                projection_role: "narration",
                requires: &["narration"],
                target_slot: Some("narration"),
                selection_mode: None,
            },
        ],
        "event" => &[
            InteractionSpec {
                trigger: "row_click",
                intents: &["select"],
                subject_kind: "object_focus",
                projection_role: "list",
                requires: &[],
                target_slot: None,
                selection_mode: Some("secondary"),
            },
            InteractionSpec {
                trigger: "timeline_pick",
                intents: &["select"],
                subject_kind: "object_focus",
                projection_role: "timeline",
                requires: &["playbackAt"],
                target_slot: None,
                selection_mode: Some("secondary"),
            },
            InteractionSpec {
                trigger: "map_world_pick",
                intents: &["select", "focus_viewpoint"],
                subject_kind: "object_focus",
                projection_role: "map",
                requires: &["place"],
                target_slot: Some("place"),
                selection_mode: Some("secondary"),
            },
            InteractionSpec {
                trigger: "chart_click",
                intents: &["select"],
                subject_kind: "object_focus",
                projection_role: "chart",
                requires: &["chart"],
                target_slot: Some("chart"),
                selection_mode: Some("secondary"),
            },
            InteractionSpec {
                trigger: "media_click",
                intents: &["open_projection"],
                subject_kind: "object_focus",
                projection_role: "media",
                requires: &["media"],
                target_slot: Some("media"),
                selection_mode: Some("secondary"),
            },
            InteractionSpec {
                trigger: "t2_click",
                intents: &["open_projection"],
                subject_kind: "object_focus",
                projection_role: "t2",
                requires: &["t2"],
                target_slot: Some("t2"),
                selection_mode: Some("secondary"),
            },
        ],
        _ => &[],
    }
}

fn responder_specs(recipe: &str) -> &'static [ResponderSpec] {
    match recipe {
        "alert" => &[
            ResponderSpec {
                role: "list",
                intents: &["select"],
                projection_role: "list",
                requires: &[],
                target_slot: None,
            },
            ResponderSpec {
                role: "detail",
                intents: &["select", "open_projection"],
                projection_role: "detail",
                requires: &["detail"],
                target_slot: Some("detail"),
            },
            ResponderSpec {
                role: "map",
                intents: &["select", "focus_viewpoint"],
                projection_role: "map",
                requires: &["place"],
                target_slot: Some("place"),
            },
            ResponderSpec {
                role: "explain",
                intents: &["explain_metric"],
                projection_role: "explain",
                requires: &["explain"],
                target_slot: Some("explain"),
            },
        ],
        "case" => &[
            ResponderSpec {
                role: "list",
                intents: &["select"],
                projection_role: "list",
                requires: &[],
                target_slot: None,
            },
            ResponderSpec {
                role: "detail",
                intents: &["select", "open_projection"],
                projection_role: "detail",
                requires: &["detail"],
                target_slot: Some("detail"),
            },
            ResponderSpec {
                role: "evidence",
                intents: &["select", "open_projection"],
                projection_role: "evidence",
                requires: &["evidence"],
                target_slot: Some("evidence"),
            },
        ],
        "place" => &[
            ResponderSpec {
                role: "map",
                intents: &["select", "focus_viewpoint"],
                projection_role: "poi",
                requires: &["viewpoint"],
                target_slot: Some("viewpoint"),
            },
            ResponderSpec {
                role: "world",
                intents: &["select", "focus_viewpoint"],
                projection_role: "world",
                requires: &["world", "viewpoint"],
                target_slot: Some("viewpoint"),
            },
            ResponderSpec {
                role: "narration",
                intents: &["open_projection"],
                projection_role: "narration",
                requires: &["narration"],
                target_slot: Some("narration"),
            },
        ],
        "event" => &[
            ResponderSpec {
                role: "list",
                intents: &["select"],
                projection_role: "list",
                requires: &[],
                target_slot: None,
            },
            ResponderSpec {
                role: "timeline",
                intents: &["select"],
                projection_role: "timeline",
                requires: &["playbackAt"],
                target_slot: None,
            },
            ResponderSpec {
                role: "map",
                intents: &["select", "focus_viewpoint"],
                projection_role: "map",
                requires: &["place"],
                target_slot: Some("place"),
            },
            ResponderSpec {
                role: "chart",
                intents: &["select"],
                projection_role: "chart",
                requires: &["chart"],
                target_slot: Some("chart"),
            },
            ResponderSpec {
                role: "media",
                intents: &["open_projection"],
                projection_role: "media",
                requires: &["media"],
                target_slot: Some("media"),
            },
            ResponderSpec {
                role: "t2",
                intents: &["open_projection"],
                projection_role: "t2",
                requires: &["t2"],
                target_slot: Some("t2"),
            },
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_domain_app_stock_override_precedence_is_deterministic() {
        let authored = json!({
            "app": {"density": "app", "nested": {"owner": "app"}},
            "domain": {"density": "domain", "nested": {"owner": "domain"}},
            "local": {"density": "local"}
        });
        let (effective, sources) = resolve_overrides("alert", &authored).expect("resolve");
        assert_eq!(effective["density"], "local");
        assert_eq!(effective["nested"]["owner"], "domain");
        assert_eq!(effective["projectionPolicy"], "hide_missing");
        assert_eq!(sources["density"], "local");
        assert_eq!(sources["nested.owner"], "domain");
        assert_eq!(sources["projectionPolicy"], "stock");
    }

    #[test]
    fn identity_override_is_rejected_at_every_layer() {
        for authored in [
            json!({"identity": "other"}),
            json!({"local": {"objectId": "manual"}}),
            json!({"domain": {"nested": {"identity_field": "other"}}}),
        ] {
            assert!(resolve_overrides("alert", &authored).is_err());
        }
    }
}
