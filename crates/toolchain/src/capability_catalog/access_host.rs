use super::prelude::*;
use super::*;

pub fn access_host_bound_tool_descriptors() -> Vec<Value> {
    let Some(surface) = mcp_surface_descriptor("access") else {
        return Vec::new();
    };
    surface
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(host_bound_access_tool_descriptor)
        .collect()
}

pub fn access_host_bound_tool_names() -> Vec<String> {
    access_host_bound_tool_descriptors()
        .into_iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()
}

pub fn access_host_bound_query_tools() -> Vec<ResourceQueryToolSpec> {
    access_host_bound_tool_descriptors()
        .into_iter()
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?.to_string();
            let description = tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let input =
                format_host_bound_input_summary(tool.get("input_schema").unwrap_or(&Value::Null));
            Some(ResourceQueryToolSpec {
                id: name.clone(),
                status: access_query_tool_status(&name).to_string(),
                purpose: description,
                input,
                output: access_query_tool_output(&name).to_string(),
            })
        })
        .collect()
}

fn host_bound_access_tool_descriptor(tool: &Value) -> Option<Value> {
    let name = tool.get("name")?.as_str()?.to_string();
    let description = tool
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let input_schema = host_bound_access_input_schema(tool.get("input_schema")?);
    Some(json!({
        "name": name,
        "description": description,
        "input_schema": input_schema,
    }))
}

fn host_bound_access_input_schema(input_schema: &Value) -> Value {
    let mut schema = match input_schema.as_object() {
        Some(map) => map.clone(),
        None => return json!({ "type": "object", "properties": {} }),
    };
    let mut properties = schema
        .remove("properties")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    properties.remove("app");
    properties.remove("source_root");
    if let Some(scene) = properties.remove("scene") {
        let mut scene_prop = scene;
        if let Some(description) = scene_prop.get("description").and_then(Value::as_str) {
            let normalized = description.replace("scene", "scene id");
            if let Some(obj) = scene_prop.as_object_mut() {
                obj.insert("description".to_string(), Value::String(normalized));
            }
        }
        properties.insert("scene_id".to_string(), scene_prop);
    }
    schema.insert("properties".to_string(), Value::Object(properties));
    let required = schema
        .remove("required")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .filter(|name| name != "app" && name != "source_root")
        .map(|name| {
            if name == "scene" {
                Value::String("scene_id".to_string())
            } else {
                Value::String(name)
            }
        })
        .collect::<Vec<_>>();
    if !required.is_empty() {
        schema.insert("required".to_string(), Value::Array(required));
    }
    Value::Object(schema)
}

fn access_query_tool_status(name: &str) -> &'static str {
    match name {
        "dataset_query" | "dataset_metric" => "phase2_api_ready",
        "resource_list" | "resource_get" | "resource_runtime_peek" => "phase3_native_ready",
        "resource_runtime_trace_export" | "resource_business_summary" => "phase5_native_ready",
        _ => "catalog_bound",
    }
}

fn access_query_tool_output(name: &str) -> &'static str {
    match name {
        "dataset_query" => {
            "bounded: {dataset{schema_preview,filters,metric_ids,analysis_contracts_preview}, sample_rows, truncation, usage_hint}"
        }
        "dataset_metric" => {
            "bounded: {dataset_id, total_rows, metrics, analysis_contracts}; analysis_contracts mirrors host UI explain/popup contract"
        }
        "resource_list" => "bounded: WorldAssetListResponse JSON",
        "resource_get" => "bounded: WorldAssetGetResponse JSON",
        "resource_runtime_peek" => "bounded: WorldRuntimePeekResponse JSON",
        "resource_runtime_trace_export" => {
            "bounded: HeadlessArtifactEnvelope JSON for runtime_trace"
        }
        "resource_business_summary" => "bounded: WorldBusinessSummary JSON",
        _ => "bounded JSON result",
    }
}

fn format_host_bound_input_summary(schema: &Value) -> String {
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return "{}".to_string();
    };
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let preferred_order = [
        "dataset_id",
        "metric_ids",
        "resource_id",
        "kind",
        "search",
        "filters",
        "columns",
        "limit",
        "trace_limit",
        "scene_id",
        "target_file",
    ];
    let mut keys = preferred_order
        .iter()
        .filter(|key| props.contains_key(**key))
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    let mut extras = props
        .keys()
        .filter(|key| !preferred_order.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    extras.sort();
    keys.extend(extras);
    let parts = keys
        .into_iter()
        .filter_map(|name| {
            let ty = props
                .get(&name)
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("value");
            let optional = !required.iter().any(|item| *item == name);
            Some(format!(
                "{}{}: {}",
                name,
                if optional { "?" } else { "" },
                ty
            ))
        })
        .collect::<Vec<_>>();
    format!("{{{}}}", parts.join(", "))
}
