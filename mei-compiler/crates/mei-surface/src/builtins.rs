use mei_syntax::CallArgs;

use crate::eval::keyword_map;
use crate::value::{clean_object, empty_object, optional, ObjectMap, Value};

pub fn app(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    let id = take_string(&mut map, "id").ok_or_else(|| "app requires `id`".to_string())?;
    Ok(Value::Object(clean_object(vec![
        ("kind", Value::string("app")),
        ("id", id),
        ("title", optional(take_string(&mut map, "title"))),
        (
            "default_scene",
            optional(take_string(&mut map, "default_scene")),
        ),
        ("scene", optional(take_value(&mut map, "scene"))),
    ])))
}

pub fn scene(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    let access_export = take_bool(&mut map, "access_export");
    let pairs = vec![
        ("kind", Value::string("scene")),
        ("id", optional(take_string(&mut map, "id"))),
        ("world", optional(take_value(&mut map, "world"))),
        ("flow", optional(take_value(&mut map, "flow"))),
        ("frame", optional(take_value(&mut map, "frame"))),
        ("profile", optional(take_string(&mut map, "profile"))),
        ("theme", optional(take_value(&mut map, "theme"))),
        ("summary", optional(take_string(&mut map, "summary"))),
        ("goal", optional(take_string(&mut map, "goal"))),
        (
            "state",
            take_value(&mut map, "state").unwrap_or_else(empty_object),
        ),
        (
            "shared",
            take_value(&mut map, "shared").unwrap_or_else(empty_object),
        ),
        ("local_nav", optional(take_value(&mut map, "local_nav"))),
        (
            "params",
            take_value(&mut map, "params").unwrap_or_else(empty_object),
        ),
        (
            "bindings",
            take_value(&mut map, "bindings").unwrap_or_else(empty_object),
        ),
        (
            "examples",
            take_value(&mut map, "examples").unwrap_or_else(|| Value::Array(vec![])),
        ),
        ("base", optional(take_value(&mut map, "base"))),
    ];
    let mut object = clean_object(pairs);
    if access_export == Some(false) {
        object.insert("access_export".to_string(), Value::Bool(false));
    }
    Ok(Value::Object(object))
}

pub fn world(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    Ok(Value::Object(clean_object(vec![
        ("kind", Value::string("world")),
        ("id", optional(take_string(&mut map, "id"))),
        ("topology", optional(take_value(&mut map, "topology"))),
        (
            "resources",
            take_value(&mut map, "resources").unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "entities",
            take_value(&mut map, "entities").unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "datasets",
            take_value(&mut map, "datasets").unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "metrics",
            take_value(&mut map, "metrics").unwrap_or_else(|| Value::Array(vec![])),
        ),
        (
            "metric_packs",
            take_value(&mut map, "metric_packs").unwrap_or_else(|| Value::Array(vec![])),
        ),
        ("base", optional(take_value(&mut map, "base"))),
    ])))
}

pub fn frame(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    Ok(Value::Object(clean_object(vec![
        ("kind", Value::string("frame")),
        ("id", optional(take_string(&mut map, "id"))),
        ("title", optional(take_string(&mut map, "title"))),
        ("layout", optional(take_value(&mut map, "layout"))),
        ("profile", optional(take_string(&mut map, "profile"))),
        (
            "props",
            take_value(&mut map, "props").unwrap_or_else(empty_object),
        ),
        ("blocks", optional(take_value(&mut map, "blocks"))),
        ("panels", optional(take_value(&mut map, "panels"))),
        ("base", optional(take_value(&mut map, "base"))),
    ])))
}

pub fn panel_decl(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    let id = map.remove("id");
    let area = map.remove("area");
    let panel_id = match id {
        Some(value @ Value::String(_)) => Some(value),
        Some(other) => Some(other),
        None => area.clone(),
    };
    Ok(Value::Object(clean_object(vec![
        ("kind", Value::string("panel")),
        ("id", optional(panel_id)),
        ("title", optional(take_string(&mut map, "title"))),
        ("head", optional(take_value(&mut map, "head"))),
        ("area", optional(area)),
        ("layout", optional(take_value(&mut map, "layout"))),
        ("data_ref", optional(take_value(&mut map, "data_ref"))),
        (
            "props",
            take_value(&mut map, "props").unwrap_or_else(empty_object),
        ),
        (
            "slot",
            take_value(&mut map, "slot").unwrap_or_else(empty_object),
        ),
        (
            "head_props",
            take_value(&mut map, "head_props").unwrap_or_else(empty_object),
        ),
        (
            "body_props",
            take_value(&mut map, "body_props").unwrap_or_else(empty_object),
        ),
        ("blocks", optional(take_value(&mut map, "blocks"))),
        ("data", optional(take_value(&mut map, "data"))),
        ("base", optional(take_value(&mut map, "base"))),
    ])))
}

pub fn scene_ref(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    let id = take_string(&mut map, "id");
    let scene_id = take_string(&mut map, "scene_id").or_else(|| id.clone());
    Ok(Value::Object(clean_object(vec![
        ("__ref", Value::string("scene")),
        ("id", optional(id)),
        ("scene_id", optional(scene_id)),
        ("scene_file", optional(take_string(&mut map, "scene_file"))),
        ("entry", optional(take_value(&mut map, "entry"))),
    ])))
}

pub fn flex(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    let direction = take_string(&mut map, "direction")
        .ok_or_else(|| "flex requires `direction`".to_string())?;
    Ok(Value::Object(clean_object(vec![
        ("type", Value::string("flex")),
        ("direction", direction),
        ("wrap", optional(take_string(&mut map, "wrap"))),
        ("gap", optional(take_string(&mut map, "gap"))),
        ("padding", optional(take_string(&mut map, "padding"))),
        ("align", optional(take_string(&mut map, "align"))),
        ("justify", optional(take_string(&mut map, "justify"))),
    ])))
}

pub fn markdown(args: &CallArgs) -> Result<Value, String> {
    let mut map = keyword_map(args)?;
    let props_value = Value::Object(clean_object(vec![
        ("path", optional(take_string(&mut map, "path"))),
        ("content", optional(take_string(&mut map, "content"))),
        ("source", optional(take_string(&mut map, "source"))),
        ("resource", optional(take_value(&mut map, "resource"))),
    ]));
    let component = Value::Object(clean_object(vec![
        ("use", Value::string("doc.markdown")),
        ("pack", Value::string("cockpit-default")),
        ("props", props_value.clone()),
    ]));
    Ok(Value::Object(clean_object(vec![
        ("kind", Value::string("block")),
        ("use_key", Value::string("doc.markdown")),
        ("id", optional(take_string(&mut map, "id"))),
        ("title", optional(take_string(&mut map, "title"))),
        ("area", optional(take_string(&mut map, "area"))),
        ("layout", optional(take_value(&mut map, "layout"))),
        ("props", props_value),
        ("component", component),
        ("placement", optional(take_value(&mut map, "placement"))),
        ("interactions", Value::Array(vec![])),
        ("lifecycle", optional(take_value(&mut map, "lifecycle"))),
        ("constraints", optional(take_value(&mut map, "constraints"))),
        ("data", optional(take_value(&mut map, "data"))),
    ])))
}

pub struct SurfaceDescriptor {
    pub name: &'static str,
    pub detail: &'static str,
}

pub fn surface_descriptors() -> Vec<SurfaceDescriptor> {
    vec![
        SurfaceDescriptor {
            name: "app",
            detail: "application root declaration",
        },
        SurfaceDescriptor {
            name: "scene",
            detail: "scene shell declaration",
        },
        SurfaceDescriptor {
            name: "world",
            detail: "world resources declaration",
        },
        SurfaceDescriptor {
            name: "frame",
            detail: "frame layout declaration",
        },
        SurfaceDescriptor {
            name: "panel_decl",
            detail: "panel block container",
        },
        SurfaceDescriptor {
            name: "scene_ref",
            detail: "scene reference helper",
        },
        SurfaceDescriptor {
            name: "flex",
            detail: "flex layout value",
        },
        SurfaceDescriptor {
            name: "markdown",
            detail: "doc.markdown block",
        },
    ]
}

fn take_string(map: &mut ObjectMap, key: &str) -> Option<Value> {
    match map.remove(key)? {
        Value::String(text) => Some(Value::String(text)),
        Value::Null => None,
        other => Some(other),
    }
}

fn take_bool(map: &mut ObjectMap, key: &str) -> Option<bool> {
    match map.remove(key)? {
        Value::Bool(value) => Some(value),
        _ => None,
    }
}

fn take_value(map: &mut ObjectMap, key: &str) -> Option<Value> {
    map.remove(key)
}
