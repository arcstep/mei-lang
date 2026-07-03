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
use crate::types::GraphNodeKind;

const PRIMITIVE_CALLS: &[&str] = &[
    "ground", "pool", "green", "route", "road", "building", "floor", "wall_ring", "wall", "roof",
    "ceiling", "opening", "prop",
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

fn load_world_payloads(
    app_root: &Path,
    registry: &McgRegistry,
) -> Result<BTreeMap<String, Value>> {
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
            let expanded = expand_world_v2_file(&file, &catalog)
                .map_err(|error| anyhow::anyhow!("expand world file {}: {error}", path.display()))?;
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
                let world_id =
                    string_field_value(&block.payload, &["id"]).unwrap_or_else(|| block.block_id.clone());
                if !out.contains_key(&world_id) {
                    out.insert(world_id, block.payload);
                }
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
            "spatial_source" => spatial_sources.push(lower_spatial_source(value, app_root, app_id)?),
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

    let mut plan = json!({
        "schema": "mei-world-plan-v1",
        "id": world_id,
        "spatialSources": spatial_sources,
        "site": site,
        "primitives": primitives,
        "viewLayers": view_layers,
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
    let fill_color = if prim_kind == "building" {
        ssot_color
            .clone()
            .unwrap_or_else(|| map_view.get("fillColor").cloned().unwrap_or(json!("#5d8fd6")))
    } else {
        map_view
            .get("fillColor")
            .or_else(|| material.get("color"))
            .cloned()
            .unwrap_or_else(|| json!("#5d8fd6"))
    };
    let fill_opacity = if prim_kind == "building" {
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
            } else {
                map_view.get("height").cloned().unwrap_or(json!(8.6))
            };
            layer.insert("extrusionHeight".to_string(), extrusion_height);
            layer.insert(
                "style".to_string(),
                json!({
                    "fillColor": fill_color,
                    "fillOpacity": fill_opacity,
                }),
            );
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
        || prim.get("worldEnterViewpoint").and_then(|v| v.as_str()).is_some();
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
    if let Some(world_view) = building.get_mut("worldView").and_then(|v| v.as_object_mut()) {
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
                    if let Some(map_view) = building.get_mut("mapView").and_then(|v| v.as_object_mut()) {
                        map_view.insert("height".to_string(), json!(expected));
                    }
                    if let Some(world_view) = building.get_mut("worldView").and_then(|v| v.as_object_mut())
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
    let profile = building.get("interiorProfile").cloned().or_else(|| {
        collect_interior_profile(&building_id, by_id)
    });
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
    let roof_y = roof_elevation.or_else(|| {
        wall_height.map(|h| h + roof_thickness.unwrap_or(0.18) * 0.5)
    });
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
    let feature_entity_id = resolve_feature_entity_id(&geometry)
        .or_else(|| {
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
    if call == "roof" {
        if let Some(obj) = out.as_object_mut() {
            if let Some(slab) = args.get("slab") {
                obj.insert("slab".to_string(), lower_slab(slab));
            }
        }
    }
    if args.get("world_enterable").and_then(|v| v.as_bool()) == Some(true)
        || args.get("worldEnterable").and_then(|v| v.as_bool()) == Some(true)
        || resolve_ref_string(args.get("world_enter").or_else(|| args.get("worldEnter")))
            .is_some()
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
                resolve_ref_string(
                    args.get("world_enter").or_else(|| args.get("worldEnter")),
                )
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
            let lng = arr[0].as_f64().or_else(|| arr[0].as_i64().map(|n| n as f64))?;
            let lat = arr[1].as_f64().or_else(|| arr[1].as_i64().map(|n| n as f64))?;
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
    let mut collection = load_imported_footprint_collection(&Value::Object(plan.clone()), app_root, app_id)?;
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

    plan.insert(
        "emittedFootprint".to_string(),
        collection,
    );
    plan.insert(
        "footprintSource".to_string(),
        json!(if plan.get("spatialSources").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty()) {
            "world-merge-import"
        } else {
            "world-native"
        }),
    );
    if let Some(url) = plan
        .get("spatialSources")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|s| s.get("url"))
        .cloned()
    {
        plan.insert("emittedFootprintUrl".to_string(), url);
    }
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
            return Ok(normalize_feature_collection(value));
        }
    }
    Ok(json!({ "type": "FeatureCollection", "features": [] }))
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
        .or_else(|| feature.get("id").and_then(|v| v.as_str()).map(str::to_string))
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
        assert_eq!(building.get("hasInterior").and_then(|v| v.as_bool()), Some(true));
        let map_view = building.get("mapView").expect("mapView");
        assert_eq!(map_view.get("height").and_then(|v| v.as_f64()), Some(3.38));
        assert_eq!(
            map_view.get("fillColor").and_then(|v| v.as_str()),
            Some("#f5f0e6")
        );
        assert_eq!(map_view.get("ssotDerived").and_then(|v| v.as_bool()), Some(true));
        let world_view = building.get("worldView").expect("worldView");
        assert_eq!(world_view.get("shellHeight").and_then(|v| v.as_f64()), Some(3.38));
        let profile = building.get("interiorProfile").expect("interiorProfile");
        assert_eq!(profile.get("wallHeight").and_then(|v| v.as_f64()), Some(3.2));
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
        assert_eq!(layer.get("extrusionHeight").and_then(|v| v.as_f64()), Some(3.38));
        assert_eq!(
            layer.pointer("/style/fillColor")
                .and_then(|v| v.as_str()),
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
        assert_eq!(feature.get("worldEnterable").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            feature.get("enterViewpoint").and_then(|v| v.as_str()),
            Some("lake_pavilion_world_entry")
        );
        assert_eq!(plan.get("footprintSource").and_then(|v| v.as_str()), Some("world-merge-import"));
    }

    #[test]
    fn mini_park_load_world_payloads_from_world_mei() {
        use std::path::PathBuf;
        use crate::mcg::registry::McgRegistryWriter;

        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../workspaces/ws-demo-v2")
            .canonicalize()
            .expect("ws-demo-v2");
        let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "mini-park");
        let registry = McgRegistryWriter::load(workspace.as_path(), "mini-park");
        let exchange = build_world_exchange(app_root.as_path(), &registry, "mini-park").expect("exchange");
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
        assert!(prims.len() >= 9, "got {}", prims.len());
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
        assert_eq!(plan.get("footprintSource").and_then(|v| v.as_str()), Some("world-native"));
        let emitted = plan.get("emittedFootprint").expect("emittedFootprint");
        let features = emitted.get("features").and_then(|v| v.as_array()).expect("features");
        assert_eq!(features.len(), 2);
        let kiosk = features
            .iter()
            .find(|f| f.pointer("/properties/entityId").and_then(|v| v.as_str()) == Some("kiosk"))
            .expect("kiosk feature");
        assert_eq!(
            kiosk.pointer("/geometry/type").and_then(|v| v.as_str()),
            Some("Polygon")
        );
        assert!(
            kiosk
                .pointer("/geometry/coordinates/0")
                .and_then(|v| v.as_array())
                .is_some_and(|ring| ring.len() >= 4)
        );
        let projection = build_map_projection(&plan, "mini-park").expect("projection");
        assert!(projection.get("emittedFootprint").is_some());
        assert!(
            projection
                .get("layers")
                .and_then(|v| v.as_array())
                .is_some_and(|layers| layers.len() >= 2)
        );
    }

    #[test]
    fn plaza_native_world_mei_compiles_without_import() {
        use std::path::PathBuf;
        use crate::mcg::registry::McgRegistryWriter;

        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../workspaces/ws-demo-v2")
            .canonicalize()
            .expect("ws-demo-v2");
        let app_root = mei_lang_kernel::resolve_app_root(workspace.as_path(), "mini-park");
        let world_path = app_root.join("src/world/plaza-native.world.mei");
        assert!(world_path.is_file(), "missing {}", world_path.display());
        let file = mei_syntax::v2::parse_v2_source_file(&world_path).expect("parse");
        let catalog = mei_graph::WorldContextCatalog::load_from_app(app_root.as_path());
        let expanded = mei_graph::expand_world_v2_file(&file, &catalog).expect("expand");
        let rel = "src/world/plaza-native.world.mei";
        let outcome = mei_graph::lower_v2_file(rel, &expanded).expect("lower");
        let payload = outcome
            .blocks
            .iter()
            .find(|b| b.kind == "world")
            .map(|b| b.payload.clone())
            .expect("world block");
        let plan = build_world_plan(&payload, app_root.as_path(), "mini-park").expect("plan");
        assert_eq!(plan.get("footprintSource").and_then(|v| v.as_str()), Some("world-native"));
        assert_eq!(
            plan.get("emittedFootprint")
                .and_then(|v| v.get("features"))
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(2)
        );
        let registry = McgRegistryWriter::load(workspace.as_path(), "mini-park");
        let exchange = build_world_exchange(app_root.as_path(), &registry, "mini-park").expect("exchange");
        let plaza = exchange
            .world_plan
            .pointer("/worlds/plaza_native")
            .expect("plaza_native in exchange");
        assert_eq!(
            plaza.get("footprintSource").and_then(|v| v.as_str()),
            Some("world-native")
        );
    }
}
