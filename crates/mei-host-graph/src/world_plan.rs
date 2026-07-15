//! Lower `world(...)` Mei payloads into `world_plan` and `map_projection` exchange JSON.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use mei_graph::{expand_world_v2_file, lower_v2_file, WorldContextCatalog};
use mei_syntax::v2::parse_v2_source_file;
use serde_json::{json, Map, Value};
use walkdir::WalkDir;

use crate::import::load_block_artifact;
use crate::mcg::registry::McgRegistry;
use crate::semantic_scene::collect_world_payloads_from_scene;
use crate::types::GraphNodeKind;

const PRIMITIVE_CALLS: &[&str] = &[
    "ground",
    "pool",
    "green",
    "route",
    "road",
    "building",
    "building_import",
    "floor",
    "wall_ring",
    "wall",
    "roof",
    "ceiling",
    "opening",
    "prop",
];

#[derive(Debug, Clone, Default)]
pub struct WorldCompileOutcome {
    pub world_plan: Value,
    pub map_projection: Value,
}

pub fn build_world_exchange(
    app_root: &Path,
    registry: &McgRegistry,
    app_id: &str,
) -> Result<WorldCompileOutcome> {
    let payloads = load_world_payloads(app_root, registry)?;
    let mut worlds = Map::new();
    let mut projections = Map::new();
    for (world_id, payload) in payloads {
        let plan = build_world_plan(&payload, app_root, app_id)?;
        let projection = build_map_projection(&plan, app_id)?;
        projections.insert(world_id.clone(), projection);
        worlds.insert(world_id, plan);
    }
    Ok(WorldCompileOutcome {
        world_plan: json!({ "worlds": worlds }),
        map_projection: json!({ "worlds": projections }),
    })
}

fn load_world_payloads(app_root: &Path, registry: &McgRegistry) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::WorldModel)
    {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(app_root, pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        let world_id = string_field_value(&payload, &["id"]).unwrap_or_else(|| node.id.key.clone());
        out.insert(world_id, payload);
    }
    for node in registry
        .nodes
        .iter()
        .filter(|n| n.id.kind == GraphNodeKind::SemanticGraph)
    {
        let Some(pref) = node.payload_ref.as_ref() else {
            continue;
        };
        let Some(artifact) = load_block_artifact(app_root, pref)? else {
            continue;
        };
        let payload = artifact.get("payload").cloned().unwrap_or(Value::Null);
        for (world_id, world_payload) in collect_world_payloads_from_scene(&payload) {
            out.insert(world_id, world_payload);
        }
    }
    let world_dir = app_root.join("src/world");
    if world_dir.is_dir() {
        for entry in WalkDir::new(&world_dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".world.mei") {
                continue;
            }
            let file = parse_v2_source_file(path)
                .with_context(|| format!("parse world file {}", path.display()))?;
            let catalog = WorldContextCatalog::load_from_app(app_root);
            let expanded = expand_world_v2_file(&file, &catalog).map_err(|error| {
                anyhow::anyhow!("expand world file {}: {error}", path.display())
            })?;
            let rel = path
                .strip_prefix(app_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let outcome = lower_v2_file(rel.as_str(), &expanded)?;
            for block in outcome.blocks {
                if block.kind != "world" {
                    continue;
                }
                let world_id = string_field_value(&block.payload, &["id"])
                    .unwrap_or_else(|| block.block_id.clone());
                // 作者态 `src/world/*.world.mei` 优先于 MCG 缓存 artifact。
                out.insert(world_id, block.payload);
            }
        }
    }
    Ok(out)
}

pub fn build_world_plan(payload: &Value, app_root: &Path, app_id: &str) -> Result<Value> {
    let world_id = string_field_value(payload, &["id"]).unwrap_or_else(|| "park_world".to_string());
    let mut spatial_sources = Vec::new();
    let mut site = Value::Null;
    let mut primitives = Vec::new();
    let mut view_layers = Vec::new();

    for (_key, value) in iter_world_entries(payload) {
        let Some(call) = call_name(value) else {
            continue;
        };
        match call {
            "spatial_source" => {
                spatial_sources.push(lower_spatial_source(value, app_root, app_id)?)
            }
            "site" => site = lower_site(value),
            "view_layer" => view_layers.push(lower_view_layer(value)?),
            _ if PRIMITIVE_CALLS.contains(&call) => {
                primitives.push(lower_primitive(call, value)?);
            }
            _ => {}
        }
    }

    if let Some(layers_value) = payload.get("view_layers").and_then(|v| v.as_array()) {
        for item in layers_value {
            if call_name(item) == Some("view_layer") {
                view_layers.push(lower_view_layer(item)?);
            }
        }
    }

    finalize_world_plan_ssot(&mut primitives);

    let world_stage_entities = collect_world_stage_entities(&primitives);

    let mut plan = json!({
        "schema": "mei-world-plan-v1",
        "id": world_id,
        "spatialSources": spatial_sources,
        "site": site,
        "primitives": primitives,
        "viewLayers": view_layers,
        "worldStageEntities": world_stage_entities,
    });
    if let Some(obj) = plan.as_object_mut() {
        emit_footprint_exchange(obj, app_root, app_id)?;
    }
    Ok(plan)
}

pub fn build_map_projection(world_plan: &Value, app_id: &str) -> Result<Value> {
    let world_id = world_plan
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("park_world");
    let footprint_url = world_plan
        .get("emittedFootprintUrl")
        .and_then(|v| v.as_str())
        .or_else(|| {
            world_plan
                .get("spatialSources")
                .and_then(|v| v.as_array())
                .and_then(|items| items.first())
                .and_then(|s| s.get("url"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("");
    let emitted_footprint = world_plan.get("emittedFootprint").cloned();
    let mut layers = Vec::new();
    if let Some(primitives) = world_plan.get("primitives").and_then(|v| v.as_array()) {
        for prim in primitives {
            let prim_kind = prim.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if prim_kind == "building_import" {
                let id = prim
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("building_import");
                let label = prim.get("label").and_then(|v| v.as_str()).unwrap_or(id);
                let map_view = prim
                    .get("mapView")
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or_else(|| json!({ "kind": "fill_extrusion", "fillOpacity": 0.78 }));
                let mv_kind = map_view
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("fill_extrusion");
                let height_property = prim
                    .get("heightProperty")
                    .and_then(|v| v.as_str())
                    .unwrap_or("height");
                let feature_match = prim
                    .get("featureMatch")
                    .cloned()
                    .unwrap_or_else(|| json!({ "featureKind": "building" }));
                let mut layer = json!({
                    "id": id,
                    "label": label,
                    "type": map_layer_type(mv_kind),
                    "url": footprint_url,
                    "visible": true,
                    "featureMatch": feature_match,
                    "extrusionHeightProperty": height_property,
                    "extrusionHeight": 1.0,
                });
                if let Some(obj) = layer.as_object_mut() {
                    merge_map_style(obj, &map_view, prim, mv_kind);
                }
                layers.push(layer);
                continue;
            }
            let Some(map_view) = prim.get("mapView").filter(|v| !v.is_null()) else {
                continue;
            };
            let id = prim.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let feature_entity = prim
                .get("featureEntityId")
                .and_then(|v| v.as_str())
                .unwrap_or(id);
            let kind = map_view.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let label = prim.get("label").and_then(|v| v.as_str()).unwrap_or(id);
            let mut layer = json!({
                "id": id,
                "label": label,
                "type": map_layer_type(kind),
                "url": footprint_url,
                "visible": true,
                "featureMatch": { "entityId": feature_entity },
            });
            if let Some(obj) = layer.as_object_mut() {
                merge_map_style(obj, map_view, prim, kind);
            }
            layers.push(layer);
        }
    }
    Ok(json!({
        "schema": "mei-map-projection-v1",
        "worldRef": world_id,
        "appId": app_id,
        "emittedFootprint": emitted_footprint,
        "layers": layers,
    }))
}

fn map_layer_type(kind: &str) -> &'static str {
    match kind {
        "polyline" => "line",
        _ => "polygon",
    }
}

fn merge_map_style(layer: &mut Map<String, Value>, map_view: &Value, prim: &Value, kind: &str) {
    let prim_kind = prim.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let shell_material = prim
        .get("shellMaterial")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or_else(|| prim.get("material").cloned().unwrap_or(Value::Null));
    let material = prim.get("material").cloned().unwrap_or(Value::Null);
    let ssot_color = shell_material
        .get("color")
        .or_else(|| material.get("color"))
        .cloned();
    let ssot_opacity = shell_material
        .get("opacity")
        .or_else(|| material.get("opacity"))
        .cloned();
    let fill_color = if prim_kind == "building" || prim_kind == "building_import" {
        ssot_color.clone().unwrap_or_else(|| {
            map_view
                .get("fillColor")
                .cloned()
                .unwrap_or(json!("#5d8fd6"))
        })
    } else {
        map_view
            .get("fillColor")
            .or_else(|| material.get("color"))
            .cloned()
            .unwrap_or_else(|| json!("#5d8fd6"))
    };
    let fill_opacity = if prim_kind == "building" || prim_kind == "building_import" {
        ssot_opacity
            .clone()
            .unwrap_or_else(|| map_view.get("fillOpacity").cloned().unwrap_or(json!(0.84)))
    } else {
        map_view
            .get("fillOpacity")
            .or_else(|| material.get("opacity"))
            .cloned()
            .unwrap_or(json!(0.72))
    };
    match kind {
        "polygon_outline" => {
            layer.insert("outlineOnly".to_string(), json!(true));
            layer.insert(
                "style".to_string(),
                json!({
                    "fillOpacity": 0,
                    "lineColor": map_view.get("lineColor").cloned().unwrap_or(json!("#93c5fd")),
                    "lineWidth": map_view.get("lineWidth").cloned().unwrap_or(json!(2.2)),
                }),
            );
        }
        "fill_extrusion" => {
            let extrusion_height = if prim_kind == "building" {
                json!(prim.get("height").and_then(|v| v.as_f64()).unwrap_or(8.6))
            } else if prim_kind == "building_import" {
                json!(1.0)
            } else {
                map_view.get("height").cloned().unwrap_or(json!(8.6))
            };
            layer.insert("extrusionHeight".to_string(), extrusion_height);
            let height_property = layer
                .get("extrusionHeightProperty")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mut style = json!({
                "fillColor": fill_color,
                "fillOpacity": fill_opacity,
            });
            if !height_property.is_empty() {
                style["extrusionHeightProperty"] = json!(height_property);
            }
            layer.insert("style".to_string(), style);
            merge_world_enter_fields(layer, prim);
        }
        "polyline" => {
            layer.insert(
                "style".to_string(),
                json!({
                    "lineColor": map_view.get("lineColor").cloned().unwrap_or(fill_color),
                    "lineWidth": map_view.get("lineWidth").cloned().unwrap_or(json!(4.4)),
                    "lineOpacity": map_view.get("lineOpacity").cloned().unwrap_or(json!(0.92)),
                }),
            );
            merge_world_enter_fields(layer, prim);
        }
        _ => {
            layer.insert(
                "style".to_string(),
                json!({
                    "fillColor": fill_color,
                    "fillOpacity": fill_opacity,
                }),
            );
        }
    }
}

fn merge_world_enter_fields(layer: &mut Map<String, Value>, prim: &Value) {
    let enterable = prim.get("worldEnterable").and_then(|v| v.as_bool()) == Some(true)
        || prim
            .get("worldEnterViewpoint")
            .and_then(|v| v.as_str())
            .is_some();
    if !enterable {
        return;
    }
    layer.insert("worldEnterable".to_string(), json!(true));
    if let Some(label) = prim.get("worldEnterLabel").filter(|v| !v.is_null()) {
        layer.insert("worldEnterLabel".to_string(), label.clone());
    }
    if let Some(vp) = prim.get("worldEnterViewpoint").filter(|v| !v.is_null()) {
        layer.insert("enterViewpoint".to_string(), vp.clone());
    }
}

/// Resolve dual projections and interior semantics from primitive SSOT (`height`, `shell`, children).
fn collect_world_stage_entities(primitives: &[Value]) -> Vec<Value> {
    let snapshot: Vec<Value> = primitives.to_vec();
    let by_id: BTreeMap<String, Value> = snapshot
        .iter()
        .filter_map(|prim| {
            let id = prim.get("id").and_then(|v| v.as_str())?;
            Some((id.to_string(), prim.clone()))
        })
        .collect();
    let mut entities = Vec::new();
    for prim in primitives {
        let Some(kind) = prim.get("kind").and_then(|v| v.as_str()) else {
            continue;
        };
        if kind != "building" {
            continue;
        }
        if prim.get("mapOnly").and_then(|v| v.as_bool()) == Some(true) {
            continue;
        }
        let has_world_view = prim.get("worldView").map(|v| !v.is_null()).unwrap_or(false);
        let enterable = prim.get("worldEnterable").and_then(|v| v.as_bool()) == Some(true);
        let has_interior = prim.get("hasInterior").and_then(|v| v.as_bool()) == Some(true);
        if !(has_world_view || enterable || has_interior) {
            continue;
        }
        let entity_id = prim
            .get("featureEntityId")
            .and_then(|v| v.as_str())
            .or_else(|| prim.get("id").and_then(|v| v.as_str()))
            .unwrap_or("");
        if entity_id.is_empty() {
            continue;
        }
        let mut members = vec![format!("{entity_id}:shell"), entity_id.to_string()];
        for child in by_id.values() {
            if resolve_building_id_for_primitive(child, &by_id).as_deref() != Some(entity_id) {
                continue;
            }
            let child_kind = child.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if child_kind == "building" || child_kind == "building_import" {
                continue;
            }
            if let Some(child_id) = child.get("id").and_then(|v| v.as_str()) {
                members.push(child_id.to_string());
            }
        }
        members.sort();
        members.dedup();
        entities.push(json!({
            "entityId": entity_id,
            "buildingId": prim.get("id").and_then(|v| v.as_str()).unwrap_or(entity_id),
            "worldEnterable": enterable,
            "members": members,
        }));
    }
    entities
}

fn finalize_world_plan_ssot(primitives: &mut [Value]) {
    let snapshot: Vec<Value> = primitives.to_vec();
    let by_id: BTreeMap<String, Value> = snapshot
        .iter()
        .filter_map(|prim| {
            let id = prim.get("id").and_then(|v| v.as_str())?;
            Some((id.to_string(), prim.clone()))
        })
        .collect();

    for prim in primitives.iter_mut() {
        let Some(obj) = prim.as_object_mut() else {
            continue;
        };
        let kind = obj
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if kind == "building" {
            enrich_building_ssot(obj, &by_id);
        }
        if kind == "roof" {
            enrich_roof_ssot(obj, &by_id);
        }
        if kind == "floor" {
            enrich_floor_ssot(obj);
        }
    }
}

fn enrich_building_ssot(building: &mut Map<String, Value>, by_id: &BTreeMap<String, Value>) {
    let building_id = building
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let height = building.get("height").and_then(|v| v.as_f64());
    let shell = building
        .get("shellMaterial")
        .cloned()
        .unwrap_or_else(|| building.get("material").cloned().unwrap_or(Value::Null));
    let shell_color = shell.get("color").cloned();
    let shell_opacity = shell.get("opacity").cloned();

    if let Some(map_view) = building.get_mut("mapView").and_then(|v| v.as_object_mut()) {
        if map_view.get("kind").and_then(|v| v.as_str()) == Some("fill_extrusion") {
            if let Some(h) = height {
                map_view.insert("height".to_string(), json!(h));
            }
            if let Some(color) = shell_color.clone() {
                map_view.insert("fillColor".to_string(), color);
            }
            if let Some(opacity) = shell_opacity.clone() {
                map_view.insert("fillOpacity".to_string(), opacity);
            }
            map_view.insert("ssotDerived".to_string(), json!(true));
        }
    }
    if let Some(world_view) = building
        .get_mut("worldView")
        .and_then(|v| v.as_object_mut())
    {
        if world_view.get("kind").and_then(|v| v.as_str()) == Some("footprint_shell") {
            if let Some(h) = height {
                world_view.insert("shellHeight".to_string(), json!(h));
            }
            if let Some(color) = shell_color {
                world_view.insert("shellColor".to_string(), color);
            }
            if let Some(opacity) = shell_opacity {
                world_view.insert("shellOpacity".to_string(), opacity);
            }
            world_view.insert("ssotDerived".to_string(), json!(true));
        }
    }

    if let Some(profile) = collect_interior_profile(&building_id, by_id) {
        building.insert("hasInterior".to_string(), json!(true));
        building.insert("interiorProfile".to_string(), profile.clone());
        if let Some(wall_height) = profile.get("wallHeight").and_then(|v| v.as_f64()) {
            if let Some(envelope) = height {
                let roof_thickness = profile
                    .get("roofThickness")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.18);
                let expected = wall_height + roof_thickness;
                if (envelope - expected).abs() > 0.05 {
                    building.insert("height".to_string(), json!(expected));
                    if let Some(map_view) =
                        building.get_mut("mapView").and_then(|v| v.as_object_mut())
                    {
                        map_view.insert("height".to_string(), json!(expected));
                    }
                    if let Some(world_view) = building
                        .get_mut("worldView")
                        .and_then(|v| v.as_object_mut())
                    {
                        world_view.insert("shellHeight".to_string(), json!(expected));
                    }
                }
            }
        }
    }
}

fn enrich_floor_ssot(floor: &mut Map<String, Value>) {
    if floor.get("elevation").is_none() {
        let elevation = floor
            .get("worldView")
            .and_then(|v| v.get("elevation"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.05);
        floor.insert("elevation".to_string(), json!(elevation));
    }
}

fn enrich_roof_ssot(roof: &mut Map<String, Value>, by_id: &BTreeMap<String, Value>) {
    let roof_value = Value::Object(roof.clone());
    let building_id = resolve_building_id_for_primitive(&roof_value, by_id);
    let Some(building_id) = building_id else {
        return;
    };
    let Some(building) = by_id.get(&building_id) else {
        return;
    };
    let profile = building
        .get("interiorProfile")
        .cloned()
        .or_else(|| collect_interior_profile(&building_id, by_id));
    let Some(profile) = profile else {
        return;
    };
    let wall_height = profile
        .get("wallHeight")
        .and_then(|v| v.as_f64())
        .unwrap_or(3.2);
    let roof_thickness = roof
        .get("slab")
        .and_then(|v| v.get("thickness"))
        .and_then(|v| v.as_f64())
        .or_else(|| {
            roof.get("worldView")
                .and_then(|v| v.get("thickness"))
                .and_then(|v| v.as_f64())
        })
        .unwrap_or(0.18);
    let derived_elevation = wall_height + roof_thickness * 0.5;
    if let Some(world_view) = roof.get_mut("worldView").and_then(|v| v.as_object_mut()) {
        if world_view.get("elevation").is_none() {
            world_view.insert("elevation".to_string(), json!(derived_elevation));
        }
        world_view.insert("ssotDerived".to_string(), json!(true));
    }
}

fn collect_interior_profile(building_id: &str, by_id: &BTreeMap<String, Value>) -> Option<Value> {
    let mut wall_height = None;
    let mut wall_thickness = None;
    let mut floor_elevation = None;
    let mut roof_elevation = None;
    let mut roof_thickness = None;

    for prim in by_id.values() {
        if resolve_building_id_for_primitive(prim, by_id).as_deref() != Some(building_id) {
            continue;
        }
        let kind = prim.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        match kind {
            "wall_ring" => {
                wall_height = prim
                    .get("height")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        prim.get("worldView")
                            .and_then(|v| v.get("height"))
                            .and_then(|v| v.as_f64())
                    })
                    .or(wall_height);
                wall_thickness = prim
                    .get("thickness")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        prim.get("worldView")
                            .and_then(|v| v.get("thickness"))
                            .and_then(|v| v.as_f64())
                    })
                    .or(wall_thickness);
            }
            "floor" => {
                floor_elevation = prim
                    .get("elevation")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        prim.get("worldView")
                            .and_then(|v| v.get("elevation"))
                            .and_then(|v| v.as_f64())
                    })
                    .or(floor_elevation);
            }
            "roof" => {
                roof_elevation = prim
                    .get("worldView")
                    .and_then(|v| v.get("elevation"))
                    .and_then(|v| v.as_f64())
                    .or(roof_elevation);
                roof_thickness = prim
                    .get("slab")
                    .and_then(|v| v.get("thickness"))
                    .and_then(|v| v.as_f64())
                    .or(roof_thickness);
            }
            _ => {}
        }
    }

    if wall_height.is_none() && floor_elevation.is_none() && roof_elevation.is_none() {
        return None;
    }

    let mut profile = Map::new();
    if let Some(v) = wall_height {
        profile.insert("wallHeight".to_string(), json!(v));
    }
    if let Some(v) = wall_thickness {
        profile.insert("wallThickness".to_string(), json!(v));
    }
    if let Some(v) = floor_elevation {
        profile.insert("floorElevation".to_string(), json!(v));
    }
    if let Some(v) = roof_thickness {
        profile.insert("roofThickness".to_string(), json!(v));
    }
    let roof_y =
        roof_elevation.or_else(|| wall_height.map(|h| h + roof_thickness.unwrap_or(0.18) * 0.5));
    if let Some(v) = roof_y {
        profile.insert("roofElevation".to_string(), json!(v));
    }
    Some(Value::Object(profile))
}

fn resolve_building_id_for_primitive(
    prim: &Value,
    by_id: &BTreeMap<String, Value>,
) -> Option<String> {
    let parent_id = prim.get("parent").and_then(|v| v.as_str())?;
    let parent = by_id.get(parent_id)?;
    let parent_kind = parent.get("kind").and_then(|v| v.as_str())?;
    if parent_kind == "building" {
        return Some(parent_id.to_string());
    }
    if parent_kind == "floor" {
        return parent
            .get("parent")
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    None
}

fn lower_spatial_source(value: &Value, app_root: &Path, app_id: &str) -> Result<Value> {
    let args = call_args(value);
    let id = string_field(args, &["id"]).unwrap_or_else(|| "footprint".to_string());
    let kind = string_field(args, &["kind"]).unwrap_or_else(|| "geojson".to_string());
    let src = args.get("src").cloned().unwrap_or(Value::Null);
    let url = resolve_asset_url(&src, app_root, app_id)?;
    Ok(json!({
        "id": id,
        "kind": kind,
        "src": src,
        "url": url,
    }))
}

fn lower_site(value: &Value) -> Value {
    let args = call_args(value);
    let id = string_field(args, &["id"]).unwrap_or_else(|| "park_site".to_string());
    let origin = args.get("origin").cloned().unwrap_or(Value::Null);
    let mut lng = 106.38224;
    let mut lat = 29.62396;
    if call_name(&origin) == Some("geo") {
        let o = call_args(&origin);
        lng = number_field(o, &["lng"]).unwrap_or(lng);
        lat = number_field(o, &["lat"]).unwrap_or(lat);
    }
    json!({
        "id": id,
        "origin": { "lng": lng, "lat": lat },
    })
}

fn lower_primitive(call: &str, value: &Value) -> Result<Value> {
    let args = call_args(value);
    let id = string_field(args, &["id"]).unwrap_or_default();
    let geometry = args
        .get("geometry")
        .or_else(|| args.get("footprint"))
        .cloned();
    let feature_entity_id = resolve_feature_entity_id(&geometry).or_else(|| {
        geometry
            .as_ref()
            .and_then(lower_inline_geometry)
            .map(|_| id.clone())
    });
    let inline_geometry = geometry.as_ref().and_then(lower_inline_geometry);
    let material = lower_material(
        args.get("material")
            .or_else(|| args.get("shell"))
            .or_else(|| args.get("slab"))
            .cloned()
            .as_ref(),
    );
    let map_view = lower_projection(args.get("map_view"), "map");
    let world_view = lower_projection(args.get("world_view"), "world");
    let mut out = json!({
        "kind": call,
        "id": id,
        "featureEntityId": feature_entity_id,
        "material": material,
        "mapView": map_view,
        "worldView": world_view,
        "parent": string_field(args, &["parent"]),
        "height": number_field(args, &["height"]),
        "label": string_field(args, &["label"]).unwrap_or_else(|| id.clone()),
    });
    if call == "prop" {
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "component".to_string(),
                string_field(args, &["component"])
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            obj.insert(
                "props".to_string(),
                args.get("props").cloned().unwrap_or(json!({})),
            );
            obj.insert("at".to_string(), lower_local_at(args.get("at")));
        }
    }
    if call == "wall_ring" {
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "thickness".to_string(),
                json!(number_field(args, &["thickness"]).unwrap_or(0.12)),
            );
        }
    }
    if call == "floor" {
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "elevation".to_string(),
                json!(number_field(args, &["elevation"]).unwrap_or(0.0)),
            );
            obj.insert("slab".to_string(), lower_material(args.get("slab")));
        }
    }
    if call == "building" {
        if let Some(obj) = out.as_object_mut() {
            obj.insert(
                "shellMaterial".to_string(),
                lower_material(args.get("shell")),
            );
        }
    }
    if call == "building_import" {
        if let Some(obj) = out.as_object_mut() {
            obj.insert("kind".to_string(), json!("building_import"));
            obj.insert("importBatch".to_string(), json!(true));
            obj.insert("mapOnly".to_string(), json!(true));
            obj.insert(
                "featureMatch".to_string(),
                lower_feature_match(
                    args.get("feature_match")
                        .or_else(|| args.get("featureMatch")),
                ),
            );
            obj.insert(
                "heightProperty".to_string(),
                json!(string_field(args, &["height_property", "heightProperty"])
                    .unwrap_or_else(|| "height".to_string())),
            );
            obj.insert(
                "shellMaterial".to_string(),
                lower_material(args.get("shell")),
            );
            obj.remove("featureEntityId");
            obj.remove("worldView");
        }
    }
    if call == "roof" {
        if let Some(obj) = out.as_object_mut() {
            if let Some(slab) = args.get("slab") {
                obj.insert("slab".to_string(), lower_slab(slab));
            }
        }
    }
    if args.get("world_enterable").and_then(|v| v.as_bool()) == Some(true)
        || args.get("worldEnterable").and_then(|v| v.as_bool()) == Some(true)
        || resolve_ref_string(args.get("world_enter").or_else(|| args.get("worldEnter"))).is_some()
    {
        if let Some(obj) = out.as_object_mut() {
            obj.insert("worldEnterable".to_string(), json!(true));
            obj.insert(
                "worldEnterLabel".to_string(),
                args.get("world_enter_label")
                    .or_else(|| args.get("worldEnterLabel"))
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            obj.insert(
                "worldEnterViewpoint".to_string(),
                resolve_ref_string(args.get("world_enter").or_else(|| args.get("worldEnter")))
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
        }
    }
    if let Some(inline_geometry) = inline_geometry {
        if let Some(obj) = out.as_object_mut() {
            obj.insert("inlineGeometry".to_string(), inline_geometry);
        }
    }
    Ok(out)
}

fn lower_slab(value: &Value) -> Value {
    if call_name(value) == Some("slab") {
        let args = call_args(value);
        return json!({
            "thickness": number_field(args, &["thickness"]).unwrap_or(0.18),
            "material": lower_material(args.get("material")),
        });
    }
    Value::Null
}

fn lower_view_layer(value: &Value) -> Result<Value> {
    let args = call_args(value);
    Ok(json!({
        "id": string_field(args, &["id"]),
        "label": string_field(args, &["label"]),
        "members": args
            .get("members")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    }))
}

fn lower_material(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return json!({ "kind": "surface" });
    };
    if call_name(value) == Some("surface") {
        let args = call_args(value);
        return json!({
            "kind": "surface",
            "color": string_field(args, &["color"]),
            "opacity": number_field(args, &["opacity"]),
        });
    }
    if let Some(obj) = value.as_object() {
        if obj.contains_key("material") {
            return lower_material(obj.get("material"));
        }
    }
    json!({ "kind": "surface" })
}

fn lower_projection(value: Option<&Value>, family: &str) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    let call = call_name(value).unwrap_or("");
    let args = call_args(value);
    let mut map = Map::new();
    map.insert("family".to_string(), json!(family));
    map.insert("kind".to_string(), json!(call));
    for (key, val) in args {
        if key.starts_with("arg") && key.len() > 3 {
            continue;
        }
        let normalized = camel_case_key(key);
        map.insert(normalized, val.clone());
    }
    Value::Object(map)
}

fn lower_local_at(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return json!([0, 0, 0]);
    };
    if call_name(value) == Some("local") {
        let args = call_args(value);
        return json!([
            number_field(args, &["arg0"])
                .or_else(|| number_field(args, &["x"]))
                .unwrap_or(0.0),
            number_field(args, &["arg1"])
                .or_else(|| number_field(args, &["y"]))
                .unwrap_or(0.0),
            number_field(args, &["arg2"])
                .or_else(|| number_field(args, &["z"]))
                .unwrap_or(0.0),
        ]);
    }
    if let Some(arr) = value.as_array() {
        return Value::Array(arr.clone());
    }
    Value::Null
}

fn resolve_feature_entity_id(geometry: &Option<Value>) -> Option<String> {
    let geometry = geometry.as_ref()?;
    let is_feature_ref = matches!(
        call_name(geometry),
        Some("feature_ref") | Some("source_feature_ref")
    );
    if is_feature_ref {
        let args = call_args(geometry);
        return string_field(args, &["entity_id", "entityId", "feature_id", "arg1"]);
    }
    None
}

fn lower_inline_geometry(value: &Value) -> Option<Value> {
    match call_name(value) {
        Some("geo_polygon") => {
            let args = call_args(value);
            let ring = args
                .get("ring")
                .or_else(|| args.get("arg0"))
                .and_then(parse_geo_ring)?;
            Some(json!({
                "type": "Polygon",
                "coordinates": [ring],
            }))
        }
        Some("geo_linestring") => {
            let args = call_args(value);
            let path = args
                .get("path")
                .or_else(|| args.get("ring"))
                .or_else(|| args.get("arg0"))
                .and_then(parse_geo_ring_open)?;
            Some(json!({
                "type": "LineString",
                "coordinates": path,
            }))
        }
        _ => None,
    }
}

fn parse_geo_ring(value: &Value) -> Option<Vec<Vec<f64>>> {
    let mut ring = parse_geo_ring_open(value)?;
    if ring.len() < 3 {
        return None;
    }
    if ring.first() != ring.last() {
        ring.push(ring[0].clone());
    }
    Some(ring)
}

fn parse_geo_ring_open(value: &Value) -> Option<Vec<Vec<f64>>> {
    let items = value.as_array()?;
    let mut ring = Vec::with_capacity(items.len());
    for item in items {
        ring.push(parse_geo_point(item)?);
    }
    if ring.len() < 2 {
        return None;
    }
    Some(ring)
}

fn parse_geo_point(value: &Value) -> Option<Vec<f64>> {
    if call_name(value) == Some("geo") {
        let args = call_args(value);
        let lng = number_field(args, &["lng", "lon", "longitude"])?;
        let lat = number_field(args, &["lat", "latitude"])?;
        return Some(vec![lng, lat]);
    }
    if let Some(arr) = value.as_array() {
        if arr.len() >= 2 {
            let lng = arr[0]
                .as_f64()
                .or_else(|| arr[0].as_i64().map(|n| n as f64))?;
            let lat = arr[1]
                .as_f64()
                .or_else(|| arr[1].as_i64().map(|n| n as f64))?;
            return Some(vec![lng, lat]);
        }
    }
    if let Some(obj) = value.as_object() {
        let lng = obj
            .get("lng")
            .or_else(|| obj.get("lon"))
            .and_then(|v| v.as_f64())?;
        let lat = obj.get("lat").and_then(|v| v.as_f64())?;
        return Some(vec![lng, lat]);
    }
    None
}

fn resolve_asset_url(src: &Value, app_root: &Path, app_id: &str) -> Result<String> {
    if let Some(path) = resolve_ref_string(Some(src)) {
        if path.starts_with("assets/") {
            return Ok(format!("/workspace-app-assets/{app_id}/{path}"));
        }
        return Ok(path);
    }
    let config_path = app_root.join("app.config.json");
    if config_path.is_file() {
        let text = std::fs::read_to_string(&config_path)?;
        let config: Value = serde_json::from_str(&text)?;
        if let Some(url) = config
            .get("ops")
            .and_then(|v| v.get("params"))
            .and_then(|v| v.get("park_footprint_geojson_url"))
            .and_then(|v| v.as_str())
        {
            return Ok(url.to_string());
        }
    }
    Ok(format!(
        "/workspace-app-assets/{app_id}/assets/park-footprint.geojson"
    ))
}

fn resolve_ref_string(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if value.get("__ref").and_then(|v| v.as_str()) == Some("asset_ref") {
        return value
            .get("__args")
            .and_then(|a| a.get("arg0"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    if value.get("__ref").and_then(|v| v.as_str()) == Some("asset_ref") {
        return value
            .get("__args")
            .and_then(|a| a.get("arg0"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    if value.get("__ref").and_then(|v| v.as_str()) == Some("viewpoint_ref") {
        return value
            .get("__args")
            .and_then(|a| a.get("arg0"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    if call_name(value) == Some("asset_ref") || call_name(value) == Some("viewpoint_ref") {
        return value
            .get("__args")
            .and_then(|a| a.get("arg0"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }
    None
}

fn iter_world_entries(payload: &Value) -> Vec<(String, &Value)> {
    let Some(obj) = payload.as_object() else {
        return Vec::new();
    };
    obj.iter().map(|(k, v)| (k.clone(), v)).collect()
}

fn call_name(value: &Value) -> Option<&str> {
    value.get("__call").and_then(|v| v.as_str())
}

fn call_args(value: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    value
        .get("__args")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}

fn string_field_value(value: &Value, keys: &[&str]) -> Option<String> {
    value.as_object().and_then(|map| string_field(map, keys))
}

fn string_field(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = map.get(*key).and_then(|v| v.as_str()) {
            return Some(v.to_string());
        }
    }
    None
}

fn number_field(map: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(n) = value.as_f64() {
                return Some(n);
            }
            if let Some(n) = value.as_i64() {
                return Some(n as f64);
            }
        }
    }
    None
}

fn camel_case_key(key: &str) -> String {
    if !key.contains('_') {
        return key.to_string();
    }
    let mut parts = key.split('_').filter(|p| !p.is_empty());
    let Some(first) = parts.next() else {
        return key.to_string();
    };
    let mut out = first.to_string();
    for part in parts {
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.push(c.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

/// Merge imported geometry (if any) with world primitive semantics into `emittedFootprint`.
fn emit_footprint_exchange(
    plan: &mut Map<String, Value>,
    app_root: &Path,
    app_id: &str,
) -> Result<()> {
    let primitives = plan
        .get("primitives")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut collection =
        load_imported_footprint_collection(&Value::Object(plan.clone()), app_root, app_id)?;
    let features = collection
        .as_object_mut()
        .and_then(|o| o.get_mut("features"))
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("footprint collection missing features array"))?;

    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, feature) in features.iter().enumerate() {
        if let Some(id) = feature_entity_id(feature) {
            index.insert(id, idx);
        }
    }

    for prim in &primitives {
        let entity_id = prim
            .get("featureEntityId")
            .and_then(|v| v.as_str())
            .or_else(|| prim.get("id").and_then(|v| v.as_str()))
            .unwrap_or("");
        if entity_id.is_empty() {
            continue;
        }
        if let Some(&idx) = index.get(entity_id) {
            enrich_feature_from_primitive(&mut features[idx], prim);
        } else if let Some(inline) = prim.get("inlineGeometry") {
            let mut feature = json!({
                "type": "Feature",
                "id": entity_id,
                "properties": { "entityId": entity_id },
                "geometry": inline,
            });
            enrich_feature_from_primitive(&mut feature, prim);
            index.insert(entity_id.to_string(), features.len());
            features.push(feature);
        }
    }

    let needs_play_zone = primitives.iter().any(|p| {
        p.get("id").and_then(|v| v.as_str()) == Some("play_zone")
            || p.get("featureEntityId").and_then(|v| v.as_str()) == Some("play_zone")
    });
    if needs_play_zone && !index.contains_key("play_zone") {
        append_play_zone_supplement(features, &mut index);
    }
    if needs_play_zone {
        if let Some(&idx) = index.get("play_zone") {
            if let Some(play_zone) = primitives
                .iter()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some("play_zone"))
            {
                enrich_feature_from_primitive(&mut features[idx], play_zone);
            }
        }
    }

    plan.insert("emittedFootprint".to_string(), collection.clone());
    if let Some(url) = persist_emitted_footprint_asset(
        &collection,
        app_root,
        app_id,
        plan.get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("park_world"),
    )? {
        plan.insert("emittedFootprintUrl".to_string(), json!(url));
    }
    plan.insert(
        "footprintSource".to_string(),
        json!(if plan
            .get("spatialSources")
            .and_then(|v| v.as_array())
            .is_some_and(|a| !a.is_empty())
        {
            "world-merge-import"
        } else {
            "world-native"
        }),
    );
    Ok(())
}

fn load_imported_footprint_collection(
    plan: &Value,
    app_root: &Path,
    app_id: &str,
) -> Result<Value> {
    let url = plan
        .get("spatialSources")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|s| s.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let prefix = format!("/workspace-app-assets/{app_id}/");
    if !url.is_empty() && url.starts_with(&prefix) {
        let rel = url.strip_prefix(&prefix).unwrap_or("");
        let path = app_root.join(rel);
        if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read footprint {}", path.display()))?;
            let value: Value = serde_json::from_str(&text)?;
            return Ok(normalize_imported_footprint_collection(
                normalize_feature_collection(value),
            ));
        }
    }
    Ok(json!({ "type": "FeatureCollection", "features": [] }))
}

fn lower_feature_match(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return json!({ "featureKind": "building" });
    };
    if let Some(obj) = value.as_object() {
        return Value::Object(obj.clone());
    }
    if call_name(value) == Some("object") || value.get("__args").is_some() {
        return Value::Object(call_args(value).clone());
    }
    json!({ "featureKind": "building" })
}

fn web_mercator_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    const EARTH_RADIUS: f64 = 6378137.0;
    let lng = x / EARTH_RADIUS * 180.0 / std::f64::consts::PI;
    let lat = (2.0 * (y / EARTH_RADIUS).exp().atan() - std::f64::consts::PI / 2.0) * 180.0
        / std::f64::consts::PI;
    (lng, lat)
}

fn coordinate_needs_mercator_reproject(coord: &[Value]) -> bool {
    let x = coord.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = coord.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
    x.abs() > 180.0 || y.abs() > 90.0
}

fn reproject_coordinate_pair(coord: &mut [Value], reproject: bool) {
    if !reproject || coord.len() < 2 {
        return;
    }
    let x = coord[0].as_f64().unwrap_or(0.0);
    let y = coord[1].as_f64().unwrap_or(0.0);
    if !coordinate_needs_mercator_reproject(coord) {
        return;
    }
    let (lng, lat) = web_mercator_to_wgs84(x, y);
    coord[0] = json!(lng);
    coord[1] = json!(lat);
}

fn reproject_ring(ring: &mut [Value], reproject: bool) {
    for coord in ring.iter_mut() {
        if let Some(pair) = coord.as_array_mut() {
            reproject_coordinate_pair(pair, reproject);
        }
    }
}

fn reproject_geometry_inplace(geometry: &mut Value, reproject: bool) {
    if !reproject {
        return;
    }
    let Some(geom_type) = geometry.get("type").and_then(|v| v.as_str()) else {
        return;
    };
    match geom_type {
        "Polygon" => {
            if let Some(rings) = geometry
                .get_mut("coordinates")
                .and_then(|v| v.as_array_mut())
            {
                for ring in rings.iter_mut() {
                    if let Some(ring_arr) = ring.as_array_mut() {
                        reproject_ring(ring_arr, reproject);
                    }
                }
            }
        }
        "MultiPolygon" => {
            if let Some(polys) = geometry
                .get_mut("coordinates")
                .and_then(|v| v.as_array_mut())
            {
                for poly in polys.iter_mut() {
                    if let Some(rings) = poly.as_array_mut() {
                        for ring in rings.iter_mut() {
                            if let Some(ring_arr) = ring.as_array_mut() {
                                reproject_ring(ring_arr, reproject);
                            }
                        }
                    }
                }
            }
        }
        "LineString" => {
            if let Some(coords) = geometry
                .get_mut("coordinates")
                .and_then(|v| v.as_array_mut())
            {
                for coord in coords.iter_mut() {
                    if let Some(pair) = coord.as_array_mut() {
                        reproject_coordinate_pair(pair, reproject);
                    }
                }
            }
        }
        _ => {}
    }
}

fn footprint_collection_needs_mercator_reproject(collection: &Value) -> bool {
    if collection
        .get("crs")
        .and_then(|v| v.get("properties"))
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .is_some_and(|name| name.contains("3857"))
    {
        return true;
    }
    let Some(features) = collection.get("features").and_then(|v| v.as_array()) else {
        return false;
    };
    for feature in features {
        if let Some(coords) = feature
            .pointer("/geometry/coordinates/0/0/0")
            .and_then(|v| v.as_array())
        {
            if coordinate_needs_mercator_reproject(coords) {
                return true;
            }
        }
    }
    false
}

fn normalize_imported_feature_properties(feature: &mut Value) {
    let geom_type = feature
        .pointer("/geometry/type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let existing_id = feature_entity_id(feature);
    if !feature
        .get("properties")
        .and_then(|v| v.as_object())
        .is_some()
    {
        if let Some(obj) = feature.as_object_mut() {
            obj.insert("properties".to_string(), json!({}));
        }
    }
    let Some(props) = feature
        .get_mut("properties")
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };
    let resolved_id = existing_id.or_else(|| {
        props
            .get("Id")
            .or_else(|| props.get("id"))
            .map(|v| format!("shixi_{}", v))
            .or_else(|| {
                props
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|name| format!("shixi_{}", name.trim()))
            })
    });
    if let Some(id) = &resolved_id {
        props.insert("entityId".to_string(), json!(id));
    }
    let is_area = geom_type == "Polygon" || geom_type == "MultiPolygon";
    if is_area && props.get("featureKind").is_none() {
        if props.get("height").and_then(|v| v.as_f64()).is_some() {
            props.insert("featureKind".to_string(), json!("building"));
        }
    }
    if props.get("name").is_none() {
        if let Some(label) = props.get("entityId").cloned() {
            props.insert("name".to_string(), label);
        }
    }
    let _ = props;
    if let Some(id) = resolved_id {
        if feature.get("id").is_none() {
            if let Some(obj) = feature.as_object_mut() {
                obj.insert("id".to_string(), json!(id));
            }
        }
    }
}

fn flatten_multipolygon_geometry(geometry: &mut Value) {
    if geometry.get("type").and_then(|v| v.as_str()) != Some("MultiPolygon") {
        return;
    }
    let Some(poly) = geometry
        .get("coordinates")
        .and_then(|v| v.as_array())
        .and_then(|polys| polys.first())
        .cloned()
    else {
        return;
    };
    if let Some(obj) = geometry.as_object_mut() {
        obj.insert("type".to_string(), json!("Polygon"));
        obj.insert("coordinates".to_string(), poly);
    }
}

fn persist_emitted_footprint_asset(
    collection: &Value,
    app_root: &Path,
    app_id: &str,
    world_id: &str,
) -> Result<Option<String>> {
    let safe_id = world_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let rel = format!("assets/{safe_id}-emitted-footprint.geojson");
    let path = app_root.join(&rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        serde_json::to_vec(collection).with_context(|| format!("write {}", path.display()))?,
    )?;
    Ok(Some(format!("/workspace-app-assets/{app_id}/{rel}")))
}

fn normalize_imported_footprint_collection(mut collection: Value) -> Value {
    let reproject = footprint_collection_needs_mercator_reproject(&collection);
    if let Some(obj) = collection.as_object_mut() {
        obj.remove("crs");
    }
    if let Some(features) = collection
        .get_mut("features")
        .and_then(|v| v.as_array_mut())
    {
        for feature in features.iter_mut() {
            if let Some(geometry) = feature.get_mut("geometry") {
                reproject_geometry_inplace(geometry, reproject);
                flatten_multipolygon_geometry(geometry);
            }
            normalize_imported_feature_properties(feature);
        }
    }
    collection
}

fn play_zone_supplement_ring() -> Vec<Vec<f64>> {
    vec![
        vec![113.28378, 23.07192],
        vec![113.28458, 23.07192],
        vec![113.28458, 23.07252],
        vec![113.28378, 23.07252],
        vec![113.28378, 23.07192],
    ]
}

fn append_play_zone_supplement(features: &mut Vec<Value>, index: &mut BTreeMap<String, usize>) {
    if index.contains_key("play_zone") {
        return;
    }
    let ring: Vec<Vec<Value>> = play_zone_supplement_ring()
        .into_iter()
        .map(|pair| vec![json!(pair[0]), json!(pair[1])])
        .collect();
    let feature = json!({
        "type": "Feature",
        "id": "play_zone",
        "properties": {
            "entityId": "play_zone",
            "name": "游乐区",
            "height": 14.0,
            "featureKind": "author",
            "shellColor": "#f472b6",
            "enterViewpoint": "play_zone_world_entry",
            "worldEnterable": true,
            "worldEnterLabel": "游乐区 3D"
        },
        "geometry": {
            "type": "Polygon",
            "coordinates": [ring],
        }
    });
    index.insert("play_zone".to_string(), features.len());
    features.push(feature);
}

fn normalize_feature_collection(value: Value) -> Value {
    if value.get("type").and_then(|v| v.as_str()) == Some("FeatureCollection") {
        return value;
    }
    if let Some(features) = value.as_array() {
        return json!({ "type": "FeatureCollection", "features": features });
    }
    json!({ "type": "FeatureCollection", "features": [] })
}

fn feature_entity_id(feature: &Value) -> Option<String> {
    feature
        .get("properties")
        .and_then(|p| p.get("entityId").or_else(|| p.get("entity_id")))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            feature
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
}

fn enrich_feature_from_primitive(feature: &mut Value, prim: &Value) {
    let Some(props) = feature
        .get_mut("properties")
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };
    let entity_id = prim
        .get("featureEntityId")
        .and_then(|v| v.as_str())
        .or_else(|| prim.get("id").and_then(|v| v.as_str()));
    if let Some(id) = entity_id {
        props.insert("entityId".to_string(), json!(id));
    }
    if let Some(label) = prim.get("label").and_then(|v| v.as_str()) {
        props.insert("name".to_string(), json!(label));
    }
    if prim.get("worldEnterable").and_then(|v| v.as_bool()) == Some(true) {
        props.insert("worldEnterable".to_string(), json!(true));
    }
    if let Some(label) = prim.get("worldEnterLabel").filter(|v| !v.is_null()) {
        props.insert("worldEnterLabel".to_string(), label.clone());
    }
    if let Some(vp) = prim
        .get("worldEnterViewpoint")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        props.insert("enterViewpoint".to_string(), json!(vp));
    }
    if let Some(height) = prim.get("height").and_then(|v| v.as_f64()) {
        props.insert("height".to_string(), json!(height));
    }
    if let Some(shell) = prim.get("shellMaterial").and_then(|v| v.get("color")) {
        props.insert("shellColor".to_string(), shell.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_world_plan_from_minimal_payload() {
        let payload = json!({
            "id": "park_world",
            "arg0": {
                "__call": "pool",
                "__args": {
                    "id": "central_lake",
                    "geometry": {
                        "__call": "feature_ref",
                        "__args": { "arg0": "footprint", "entity_id": "central_lake" }
                    },
                    "material": {
                        "__call": "surface",
                        "__args": { "color": "#2d7fb0", "opacity": 0.72 }
                    },
                    "map_view": {
                        "__call": "polygon_fill",
                        "__args": { "fill_opacity": 0.68 }
                    },
                    "world_view": {
                        "__call": "flat_fill",
                        "__args": { "lift": 0.028 }
                    }
                }
            }
        });
        let plan = build_world_plan(&payload, Path::new("."), "mini-park").expect("plan");
        assert_eq!(plan.get("id").and_then(|v| v.as_str()), Some("park_world"));
        let prims = plan.get("primitives").and_then(|v| v.as_array()).unwrap();
        assert_eq!(prims.len(), 1);
        assert_eq!(
            prims[0].get("featureEntityId").and_then(|v| v.as_str()),
            Some("central_lake")
        );
    }

    #[test]
    fn building_ssot_derives_map_and_world_projection_from_primitive() {
        let payload = json!({
            "id": "park_world",
            "arg0": {
                "__call": "building",
                "__args": {
                    "id": "lake_pavilion",
                    "footprint": {
                        "__call": "feature_ref",
                        "__args": { "arg0": "footprint", "entity_id": "lake_pavilion" }
                    },
                    "height": 3.38,
                    "shell": {
                        "__call": "surface",
                        "__args": { "color": "#f5f0e6", "opacity": 0.88 }
                    },
                    "map_view": {
                        "__call": "fill_extrusion",
                        "__args": { "fill_opacity": 0.84 }
                    },
                    "world_view": {
                        "__call": "footprint_shell",
                        "__args": { "lift": 0.034 }
                    }
                }
            },
            "arg1": {
                "__call": "wall_ring",
                "__args": {
                    "id": "pavilion_walls",
                    "parent": "floor_1",
                    "height": 3.2,
                    "thickness": 0.12,
                    "material": {
                        "__call": "surface",
                        "__args": { "color": "#f5f0e6", "opacity": 0.82 }
                    },
                    "world_view": {
                        "__call": "wall_ring",
                        "__args": { "height": 3.2, "thickness": 0.12 }
                    }
                }
            },
            "arg2": {
                "__call": "floor",
                "__args": {
                    "id": "floor_1",
                    "parent": "lake_pavilion",
                    "elevation": 0,
                    "slab": {
                        "__call": "surface",
                        "__args": { "color": "#d9c7a2" }
                    },
                    "world_view": {
                        "__call": "slab",
                        "__args": { "elevation": 0.05 }
                    }
                }
            },
            "arg3": {
                "__call": "roof",
                "__args": {
                    "id": "pavilion_roof",
                    "parent": "lake_pavilion",
                    "slab": {
                        "__call": "slab",
                        "__args": {
                            "thickness": 0.18,
                            "material": {
                                "__call": "surface",
                                "__args": { "color": "#8b5e34", "opacity": 0.88 }
                            }
                        }
                    },
                    "world_view": {
                        "__call": "slab",
                        "__args": { "elevation": 3.29 }
                    }
                }
            }
        });
        let plan = build_world_plan(&payload, Path::new("."), "mini-park").expect("plan");
        let building = plan
            .get("primitives")
            .and_then(|v| v.as_array())
            .and_then(|items| {
                items
                    .iter()
                    .find(|p| p.get("id").and_then(|v| v.as_str()) == Some("lake_pavilion"))
            })
            .expect("lake_pavilion primitive");
        assert_eq!(
            building.get("hasInterior").and_then(|v| v.as_bool()),
            Some(true)
        );
        let map_view = building.get("mapView").expect("mapView");
        assert_eq!(map_view.get("height").and_then(|v| v.as_f64()), Some(3.38));
        assert_eq!(
            map_view.get("fillColor").and_then(|v| v.as_str()),
            Some("#f5f0e6")
        );
        assert_eq!(
            map_view.get("ssotDerived").and_then(|v| v.as_bool()),
            Some(true)
        );
        let world_view = building.get("worldView").expect("worldView");
        assert_eq!(
            world_view.get("shellHeight").and_then(|v| v.as_f64()),
            Some(3.38)
        );
        let profile = building.get("interiorProfile").expect("interiorProfile");
        assert_eq!(
            profile.get("wallHeight").and_then(|v| v.as_f64()),
            Some(3.2)
        );
        assert_eq!(
            profile.get("roofElevation").and_then(|v| v.as_f64()),
            Some(3.29)
        );

        let projection = build_map_projection(&plan, "mini-park").expect("projection");
        let layer = projection
            .get("layers")
            .and_then(|v| v.as_array())
            .and_then(|items| {
                items
                    .iter()
                    .find(|l| l.get("id").and_then(|v| v.as_str()) == Some("lake_pavilion"))
            })
            .expect("lake_pavilion layer");
        assert_eq!(
            layer.get("extrusionHeight").and_then(|v| v.as_f64()),
            Some(3.38)
        );
        assert_eq!(
            layer.pointer("/style/fillColor").and_then(|v| v.as_str()),
            Some("#f5f0e6")
        );
    }

    #[test]
    fn emit_footprint_merges_primitive_semantics() {
        use std::fs;
        use tempfile::tempdir;

        let dir = tempdir().expect("tempdir");
        let app_root = dir.path();
        let assets = app_root.join("assets");
        fs::create_dir_all(&assets).expect("assets dir");
        fs::write(
            assets.join("park-footprint.geojson"),
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","id":"lake_pavilion","properties":{"entityId":"lake_pavilion","name":"湖心亭"},"geometry":{"type":"Polygon","coordinates":[[[106.38,29.62],[106.381,29.62],[106.381,29.621],[106.38,29.621],[106.38,29.62]]]}}]}"#,
        )
        .expect("write geojson");

        let payload = json!({
            "id": "park_world",
            "arg0": {
                "__call": "spatial_source",
                "__args": {
                    "id": "footprint",
                    "kind": "geojson",
                    "src": { "__ref": "asset_ref", "__args": { "arg0": "assets/park-footprint.geojson" } }
                }
            },
            "arg1": {
                "__call": "building",
                "__args": {
                    "id": "lake_pavilion",
                    "footprint": {
                        "__call": "feature_ref",
                        "__args": { "arg0": "footprint", "entity_id": "lake_pavilion" }
                    },
                    "height": 3.38,
                    "shell": { "__call": "surface", "__args": { "color": "#f5f0e6", "opacity": 0.88 } },
                    "map_view": { "__call": "fill_extrusion", "__args": { "fill_opacity": 0.84 } },
                    "world_view": { "__call": "footprint_shell", "__args": { "lift": 0.034 } },
                    "world_enterable": true,
                    "world_enter_label": "湖心亭 3D",
                    "world_enter": { "__ref": "viewpoint_ref", "__args": { "arg0": "lake_pavilion_world_entry" } }
                }
            }
        });
        let plan = build_world_plan(&payload, app_root, "mini-park").expect("plan");
        let emitted = plan.get("emittedFootprint").expect("emittedFootprint");
        let feature = emitted
            .pointer("/features/0/properties")
            .expect("feature properties");
        assert_eq!(
            feature.get("worldEnterable").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            feature.get("enterViewpoint").and_then(|v| v.as_str()),
            Some("lake_pavilion_world_entry")
        );
        assert_eq!(
            plan.get("footprintSource").and_then(|v| v.as_str()),
            Some("world-merge-import")
        );
    }

    #[test]
    fn mini_park_load_world_payloads_from_world_mei() {
        use crate::mcg::registry::McgRegistryWriter;
        use std::path::PathBuf;

        fn optional_external_workspace() -> Option<PathBuf> {
            let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
            let path = PathBuf::from(raw.trim());
            if path.as_os_str().is_empty() || !path.is_dir() {
                return None;
            }
            Some(path.canonicalize().unwrap_or(path))
        }

        let Some(workspace) = optional_external_workspace() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "mini-park");
        let registry = McgRegistryWriter::load(workspace.as_path(), "mini-park");
        let exchange =
            build_world_exchange(app_root.as_path(), &registry, "mini-park").expect("exchange");
        let worlds = exchange
            .world_plan
            .get("worlds")
            .and_then(|v| v.as_object())
            .expect("world_plan worlds");
        let park_world = worlds.get("park_world").expect("park_world");
        let prims = park_world
            .get("primitives")
            .and_then(|v| v.as_array())
            .expect("primitives");
        assert!(prims.len() >= 10, "got {}", prims.len());
        let lake_pavilion = prims
            .iter()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some("lake_pavilion"))
            .expect("lake_pavilion primitive");
        assert_eq!(
            lake_pavilion.get("hasInterior").and_then(|v| v.as_bool()),
            Some(true)
        );
        let stage = park_world
            .get("worldStageEntities")
            .and_then(|v| v.as_array())
            .expect("worldStageEntities");
        assert!(
            stage
                .iter()
                .any(|e| e.get("entityId").and_then(|v| v.as_str()) == Some("lake_pavilion")),
            "lake_pavilion should be a dedicated world-stage building"
        );
        assert!(
            stage
                .iter()
                .any(|e| e.get("entityId").and_then(|v| v.as_str()) == Some("play_zone")),
            "play_zone should be a dedicated world-stage building"
        );
        let emitted = park_world
            .get("emittedFootprint")
            .and_then(|v| v.get("features"))
            .and_then(|v| v.as_array())
            .expect("emittedFootprint");
        assert!(
            emitted.iter().any(|f| {
                f.pointer("/properties/entityId").and_then(|v| v.as_str()) == Some("lake_pavilion")
            }),
            "emittedFootprint should include lake_pavilion"
        );
    }

    #[test]
    fn emit_footprint_from_geo_polygon_without_spatial_source() {
        let payload = json!({
            "id": "plaza_native",
            "arg0": {
                "__call": "site",
                "__args": {
                    "id": "plaza_site",
                    "origin": {
                        "__call": "geo",
                        "__args": { "lng": 106.38224, "lat": 29.62396 }
                    }
                }
            },
            "arg1": {
                "__call": "ground",
                "__args": {
                    "id": "plaza_ground",
                    "geometry": {
                        "__call": "geo_polygon",
                        "__args": {
                            "ring": [
                                { "__call": "geo", "__args": { "lng": 106.3821, "lat": 29.6239 } },
                                { "__call": "geo", "__args": { "lng": 106.3824, "lat": 29.6239 } },
                                { "__call": "geo", "__args": { "lng": 106.3824, "lat": 29.6241 } },
                                { "__call": "geo", "__args": { "lng": 106.3821, "lat": 29.6241 } }
                            ]
                        }
                    },
                    "material": { "__call": "surface", "__args": { "color": "#1e3a5f" } },
                    "map_view": { "__call": "polygon_outline", "__args": { "line_color": "#93c5fd" } },
                    "world_view": { "__call": "site_outline", "__args": { "opacity": 0.15 } }
                }
            },
            "arg2": {
                "__call": "building",
                "__args": {
                    "id": "kiosk",
                    "footprint": {
                        "__call": "geo_polygon",
                        "__args": {
                            "ring": [
                                { "__call": "geo", "__args": { "lng": 106.38218, "lat": 29.62398 } },
                                { "__call": "geo", "__args": { "lng": 106.38228, "lat": 29.62398 } },
                                { "__call": "geo", "__args": { "lng": 106.38228, "lat": 29.62406 } },
                                { "__call": "geo", "__args": { "lng": 106.38218, "lat": 29.62406 } }
                            ]
                        }
                    },
                    "height": 4.2,
                    "shell": { "__call": "surface", "__args": { "color": "#e8dcc8", "opacity": 0.9 } },
                    "map_view": { "__call": "fill_extrusion", "__args": { "fill_opacity": 0.82 } },
                    "world_view": { "__call": "footprint_shell", "__args": { "lift": 0.02 } }
                }
            }
        });
        let plan = build_world_plan(&payload, Path::new("."), "mini-park").expect("plan");
        assert_eq!(
            plan.get("footprintSource").and_then(|v| v.as_str()),
            Some("world-native")
        );
        let emitted = plan.get("emittedFootprint").expect("emittedFootprint");
        let features = emitted
            .get("features")
            .and_then(|v| v.as_array())
            .expect("features");
        assert_eq!(features.len(), 2);
        let kiosk = features
            .iter()
            .find(|f| f.pointer("/properties/entityId").and_then(|v| v.as_str()) == Some("kiosk"))
            .expect("kiosk feature");
        assert_eq!(
            kiosk.pointer("/geometry/type").and_then(|v| v.as_str()),
            Some("Polygon")
        );
        assert!(kiosk
            .pointer("/geometry/coordinates/0")
            .and_then(|v| v.as_array())
            .is_some_and(|ring| ring.len() >= 4));
        let projection = build_map_projection(&plan, "mini-park").expect("projection");
        assert!(projection.get("emittedFootprint").is_some());
        assert!(projection
            .get("layers")
            .and_then(|v| v.as_array())
            .is_some_and(|layers| layers.len() >= 2));
    }

    #[test]
    fn plaza_native_world_mei_compiles_without_import() {
        use crate::mcg::registry::McgRegistryWriter;
        use std::path::PathBuf;

        fn optional_external_workspace() -> Option<PathBuf> {
            let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
            let path = PathBuf::from(raw.trim());
            if path.as_os_str().is_empty() || !path.is_dir() {
                return None;
            }
            Some(path.canonicalize().unwrap_or(path))
        }

        let Some(workspace) = optional_external_workspace() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "mini-park");
        let world_path = app_root.join("src/world/3d/plaza-native.world.mei");
        assert!(world_path.is_file(), "missing {}", world_path.display());
        let file = mei_syntax::v2::parse_v2_source_file(&world_path).expect("parse");
        let catalog = mei_graph::WorldContextCatalog::load_from_app(app_root.as_path());
        let expanded = mei_graph::expand_world_v2_file(&file, &catalog).expect("expand");
        let rel = "src/world/3d/plaza-native.world.mei";
        let outcome = mei_graph::lower_v2_file(rel, &expanded).expect("lower");
        let payload = outcome
            .blocks
            .iter()
            .find(|b| b.kind == "world")
            .map(|b| b.payload.clone())
            .expect("world block");
        let plan = build_world_plan(&payload, app_root.as_path(), "mini-park").expect("plan");
        assert_eq!(
            plan.get("footprintSource").and_then(|v| v.as_str()),
            Some("world-native")
        );
        assert_eq!(
            plan.get("emittedFootprint")
                .and_then(|v| v.get("features"))
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(2)
        );
        let registry = McgRegistryWriter::load(workspace.as_path(), "mini-park");
        let exchange =
            build_world_exchange(app_root.as_path(), &registry, "mini-park").expect("exchange");
        let plaza = exchange
            .world_plan
            .pointer("/worlds/plaza_native")
            .expect("plaza_native in exchange");
        assert_eq!(
            plaza.get("footprintSource").and_then(|v| v.as_str()),
            Some("world-native")
        );
    }

    #[test]
    fn shixi_building_import_compiles_with_batch_extrusion() {
        use std::path::PathBuf;

        fn optional_external_workspace() -> Option<PathBuf> {
            let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
            let path = PathBuf::from(raw.trim());
            if path.as_os_str().is_empty() || !path.is_dir() {
                return None;
            }
            Some(path.canonicalize().unwrap_or(path))
        }

        let Some(workspace) = optional_external_workspace() else {
            eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
            return;
        };
        let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "mini-park");
        let payload = json!({
            "id": "shixi_overlay",
            "arg0": {
                "__call": "spatial_source",
                "__args": {
                    "id": "footprint",
                    "kind": "geojson",
                    "src": { "__ref": "asset_ref", "__args": { "arg0": "assets/shixi.geojson" } }
                }
            },
            "arg1": {
                "__call": "site",
                "__args": {
                    "id": "shixi_site",
                    "origin": { "__call": "geo", "__args": { "lng": 113.2795, "lat": 23.06748 } }
                }
            },
            "arg2": {
                "__call": "building_import",
                "__args": {
                    "id": "shixi_buildings",
                    "feature_match": { "featureKind": "building" },
                    "height_property": "height",
                    "shell": { "__call": "surface", "__args": { "color": "#c8d4e0", "opacity": 0.86 } },
                    "map_view": { "__call": "fill_extrusion", "__args": { "fill_opacity": 0.78 } }
                }
            }
        });
        let plan = build_world_plan(&payload, app_root.as_path(), "mini-park").expect("plan");
        assert_eq!(
            plan.get("footprintSource").and_then(|v| v.as_str()),
            Some("world-merge-import")
        );
        let features = plan
            .get("emittedFootprint")
            .and_then(|v| v.get("features"))
            .and_then(|v| v.as_array())
            .expect("emitted features");
        assert!(
            features.len() > 400,
            "shixi import should include hundreds of buildings, got {}",
            features.len()
        );
        assert!(
            features.iter().any(|f| {
                f.pointer("/properties/featureKind")
                    .and_then(|v| v.as_str())
                    == Some("building")
            }),
            "shixi import should tag building features"
        );
        let first = features
            .iter()
            .find(|f| {
                f.pointer("/properties/featureKind")
                    .and_then(|v| v.as_str())
                    == Some("building")
            })
            .expect("building feature");
        let lng = first
            .pointer("/geometry/coordinates/0/0/0/0")
            .or_else(|| first.pointer("/geometry/coordinates/0/0/0"))
            .and_then(|v| v.as_f64())
            .expect("wgs lng");
        assert!(
            lng > 110.0 && lng < 115.0,
            "coordinates should be WGS84, got {lng}"
        );
        let projection = build_map_projection(&plan, "mini-park").expect("projection");
        let layers = projection
            .get("layers")
            .and_then(|v| v.as_array())
            .expect("projection layers");
        assert!(
            layers
                .iter()
                .any(|l| l.get("id").and_then(|v| v.as_str()) == Some("shixi_buildings")),
            "batch building layer missing"
        );
    }
}
