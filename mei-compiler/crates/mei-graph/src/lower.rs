use mei_syntax::v2::{CallArgs, V2Item, V2SourceFile};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use thiserror::Error;

use crate::artifact_expand;

#[derive(Debug, Error)]
pub enum LowerGraphError {
    #[error("{0}")]
    Lower(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBlock {
    pub kind: String,
    pub block_id: String,
    pub schema: String,
    pub payload: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphOutcome {
    pub graph_schema_version: String,
    pub source_file: String,
    pub blocks: Vec<GraphBlock>,
}

pub fn lower_v2_file(
    source_file: &str,
    file: &V2SourceFile,
) -> Result<GraphOutcome, LowerGraphError> {
    let mut blocks = Vec::new();
    for item in &file.items {
        if let V2Item::TopLevel { name, args } = item {
            blocks.push(lower_top_level(source_file, name, args)?);
        }
    }
    Ok(GraphOutcome {
        graph_schema_version: "mei-compiler-graph-v2".to_string(),
        source_file: source_file.to_string(),
        blocks,
    })
}

fn lower_top_level(
    source_file: &str,
    name: &str,
    args: &CallArgs,
) -> Result<GraphBlock, LowerGraphError> {
    let mut payload = call_args_to_json(args)?;
    if matches!(
        name,
        "scene"
            | "plane_layout"
            | "region_layout"
            | "section_layout"
            | "map_spec"
            | "view_spec"
            | "assembly_view"
            | "board_assembly"
    ) {
        if let Some(obj) = payload.as_object_mut() {
            obj.entry("source_file".to_string())
                .or_insert(JsonValue::String(source_file.to_string()));
            if obj.get("key").is_none() {
                let block_id = obj
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        LowerGraphError::Lower(
                            format!("{name} top-level must declare non-empty `id`"),
                        )
                    })?;
                obj.insert(
                    "key".to_string(),
                    JsonValue::String(format!("{block_id}@{source_file}")),
                );
            }
        }
    }
    let block_id = derive_block_id(name, source_file, &payload)?;
    let schema = schema_for_constructor(name);
    Ok(GraphBlock {
        kind: name.to_string(),
        block_id,
        schema: schema.to_string(),
        payload,
    })
}

fn schema_for_constructor(name: &str) -> &'static str {
    match name {
        "app_skeleton" => "mei-app-skeleton-artifact-v1",
        "scene" => "mei-scene-semantic-v1",
        "plane_layout" | "region_layout" | "section_layout" => "mei-scene-layout-fragment-v1",
        "map_spec" => "mei-map-spec-v1",
        "view_spec" => "mei-view-spec-v1",
        "assembly_view" | "board_assembly" => "mei-projection-assembly-v1",
        "panel_contract" => "mei-panel-contract-artifact-v1",
        "metric_def_bundle" => "mei-metric-def-bundle-artifact-v1",
        "navigation" | "link_decl" => "mei-navigation-artifact-v1",
        "warmup_policy" => "mei-warmup-policy-artifact-v1",
        _ => "mei-graph-block-v2",
    }
}

fn derive_block_id(
    name: &str,
    source_file: &str,
    payload: &JsonValue,
) -> Result<String, LowerGraphError> {
    let obj = payload
        .as_object()
        .ok_or_else(|| LowerGraphError::Lower("payload must be object".into()))?;
    match name {
        "app_skeleton" => kw_string(obj, "id").map(|id| format!("app_skeleton:{id}")),
        "scene" => {
            let scene_id = kw_string(obj, "id")?;
            let key = obj
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{scene_id}@{source_file}"));
            Ok(format!("scene:{key}"))
        }
        "plane_layout" | "region_layout" | "section_layout" | "map_spec" | "view_spec" => {
            let id = kw_string(obj, "id")?;
            let key = obj
                .get("key")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{id}@{source_file}"));
            Ok(format!("{name}:{key}"))
        }
        "navigation" | "link_decl" => kw_string(obj, "key").map(|key| format!("{name}:{key}")),
        "assembly_view" | "board_assembly" => kw_string(obj, "key").map(|key| format!("assembly_view:{key}")),
        "panel_contract" => {
            let id = kw_string(obj, "id")?;
            if let Some(scope) = obj.get("scope").and_then(|v| v.as_str()) {
                Ok(format!("panel_contract:{scope}:{id}"))
            } else {
                Ok(format!("panel_contract:{id}"))
            }
        }
        "metric_def_bundle" => kw_string(obj, "key").map(|key| format!("metric_def_bundle:{key}")),
        "world" => kw_string(obj, "id").map(|id| format!("world_model:{id}")),
        "warmup_policy" => {
            let scope = obj.get("scope").cloned().unwrap_or(JsonValue::Null);
            Ok(format!("warmup_policy:{scope}"))
        }
        other => Ok(format!("{other}:anonymous")),
    }
}

fn kw_string(obj: &Map<String, JsonValue>, key: &str) -> Result<String, LowerGraphError> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| LowerGraphError::Lower(format!("missing string field `{key}`")))
}

fn call_args_to_json(args: &CallArgs) -> Result<JsonValue, LowerGraphError> {
    let mut map = Map::new();
    for (idx, expr) in args.positional.iter().enumerate() {
        map.insert(
            format!("arg{idx}"),
            artifact_expand::expr_to_json(expr).map_err(|e| LowerGraphError::Lower(e.to_string()))?,
        );
    }
    for (name, expr) in &args.keywords {
        map.insert(
            name.clone(),
            artifact_expand::expr_to_json(expr).map_err(|e| LowerGraphError::Lower(e.to_string()))?,
        );
    }
    Ok(JsonValue::Object(map))
}
